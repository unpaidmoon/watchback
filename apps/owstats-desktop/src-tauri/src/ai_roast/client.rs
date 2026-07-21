use super::{
    arena::ArenaRoastMaterial,
    material::{
        AnonymousOrdinal, AnonymousPlayerRole, FactSubject, RoastFact, RoastMaterial,
        RoastRelationshipKind, RoastRelationshipSubject,
    },
    validator::{
        validate_roast_output, RoastOutput, RoastValidationContext, RoastValidationVocabulary,
    },
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

pub const MODEL: &str = "deepseek-v4-flash";
pub const PROMPT_VERSION: i64 = 1;
pub const MAX_TOKENS: u32 = 500;
pub const PRODUCTION_TIMEOUT: Duration = Duration::from_secs(60);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";

#[derive(Debug, Clone, Copy)]
pub enum RoastPromptMaterial<'a> {
    Match(&'a RoastMaterial),
    Arena(&'a ArenaRoastMaterial),
}

#[derive(Clone)]
pub struct DeepSeekRoastClient {
    client: Client,
    base_url: String,
    timeout: Duration,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DeepSeekRoastError {
    #[error("DeepSeek API Key \u{65e0}\u{6548}")]
    InvalidKey,
    #[error("DeepSeek \u{4f59}\u{989d}\u{4e0d}\u{8db3}")]
    InsufficientBalance,
    #[error("\u{8bf7}\u{6c42}\u{8fc7}\u{4e8e}\u{9891}\u{7e41}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}")]
    TooFrequent,
    #[error("DeepSeek \u{670d}\u{52a1}\u{6682}\u{65f6}\u{7e41}\u{5fd9}")]
    Busy,
    #[error("\u{751f}\u{6210}\u{8d85}\u{65f6}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}")]
    Timeout,
    #[error("\u{65e0}\u{6cd5}\u{8fde}\u{63a5} DeepSeek \u{670d}\u{52a1}")]
    Transport,
    #[error("\u{751f}\u{6210}\u{5185}\u{5bb9}\u{8d85}\u{51fa}\u{957f}\u{5ea6}\u{9650}\u{5236}")]
    Length,
    #[error(
        "\u{751f}\u{6210}\u{5185}\u{5bb9}\u{88ab}\u{5b89}\u{5168}\u{89c4}\u{5219}\u{62e6}\u{622a}"
    )]
    ContentFilter,
    #[error("DeepSeek \u{8fd4}\u{56de}\u{4e86}\u{65e0}\u{6548}\u{5185}\u{5bb9}")]
    InvalidOutput,
}

impl DeepSeekRoastClient {
    pub fn new() -> Result<Self, DeepSeekRoastError> {
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(PRODUCTION_TIMEOUT)
            .build()
            .map_err(|_| DeepSeekRoastError::Transport)?;
        Self::with_client(client, DEFAULT_BASE_URL, PRODUCTION_TIMEOUT)
    }

    #[doc(hidden)]
    pub fn with_client(
        client: Client,
        base_url: &str,
        timeout: Duration,
    ) -> Result<Self, DeepSeekRoastError> {
        let base_url = base_url.trim().trim_end_matches('/');
        if base_url.is_empty() || timeout.is_zero() {
            return Err(DeepSeekRoastError::Transport);
        }
        Ok(Self {
            client,
            base_url: base_url.to_string(),
            timeout,
        })
    }

    pub async fn generate(
        &self,
        api_key: &str,
        material: RoastPromptMaterial<'_>,
        vocabulary: &RoastValidationVocabulary,
    ) -> Result<RoastOutput, DeepSeekRoastError> {
        if api_key.trim().is_empty() {
            return Err(DeepSeekRoastError::InvalidKey);
        }
        if prompt_fact_count(material) < 2 {
            return Err(DeepSeekRoastError::InvalidOutput);
        }
        let context = match material {
            RoastPromptMaterial::Match(value) => {
                RoastValidationContext::for_match(value, vocabulary.clone())
            }
            RoastPromptMaterial::Arena(value) => {
                RoastValidationContext::for_arena(value, vocabulary.clone())
            }
        };
        let request = build_request(material);

        for attempt in 0..=1 {
            let response = self
                .client
                .post(format!("{}/chat/completions", self.base_url))
                .bearer_auth(api_key)
                .timeout(self.timeout)
                .json(&request)
                .send()
                .await
                .map_err(map_request_error)?;
            let status = response.status();
            if !status.is_success() {
                return Err(map_status(status));
            }
            let body = response.bytes().await.map_err(map_request_error)?;
            let completion = match serde_json::from_slice::<CompletionResponse>(&body) {
                Ok(value) => value,
                Err(_) if attempt == 0 => continue,
                Err(_) => return Err(DeepSeekRoastError::InvalidOutput),
            };
            let Some(choice) = completion.choices.into_iter().next() else {
                if attempt == 0 {
                    continue;
                }
                return Err(DeepSeekRoastError::InvalidOutput);
            };
            match choice.finish_reason.as_deref() {
                Some("stop") => {}
                Some("length") => return Err(DeepSeekRoastError::Length),
                Some("content_filter") => return Err(DeepSeekRoastError::ContentFilter),
                Some("insufficient_system_resource") => return Err(DeepSeekRoastError::Busy),
                _ => return Err(DeepSeekRoastError::InvalidOutput),
            }
            let content = choice.message.content.unwrap_or_default();
            match validate_roast_output(&content, &context) {
                Ok(output) => return Ok(output),
                Err(_) if attempt == 0 => continue,
                Err(_) => return Err(DeepSeekRoastError::InvalidOutput),
            }
        }
        Err(DeepSeekRoastError::InvalidOutput)
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: [Message; 2],
    max_tokens: u32,
    thinking: Thinking,
    response_format: ResponseFormat,
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct Thinking {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Deserialize)]
struct CompletionResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: CompletionMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct CompletionMessage {
    content: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchPrompt<'a> {
    kind: &'static str,
    prompt_version: i64,
    evaluation: PromptEvaluation<'a>,
    selected_facts: Vec<PromptFact<'a>>,
    anonymous_relation: Option<PromptRelationship<'a>>,
    streak: PromptStreak<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArenaPrompt<'a> {
    kind: &'static str,
    prompt_version: i64,
    candidate_facts: Vec<PromptFact<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptEvaluation<'a> {
    primary_tag: &'a str,
    evidence_tags: &'a [String],
}

#[derive(Serialize)]
struct PromptFact<'a> {
    id: &'a str,
    subject: &'static str,
    text: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptRelationship<'a> {
    kind: &'static str,
    anonymous_subject: String,
    facts: Vec<PromptFact<'a>>,
    counter_facts: Vec<PromptFact<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptStreak<'a> {
    before_current: usize,
    previous_event: Option<&'a str>,
    current_event: &'a str,
}

fn build_request(material: RoastPromptMaterial<'_>) -> ChatRequest {
    ChatRequest {
        model: MODEL,
        messages: [
            Message {
                role: "system",
                content: system_prompt().to_string(),
            },
            Message {
                role: "user",
                content: user_prompt(material),
            },
        ],
        max_tokens: MAX_TOKENS,
        thinking: Thinking { kind: "disabled" },
        response_format: ResponseFormat {
            kind: "json_object",
        },
        stream: false,
    }
}

fn system_prompt() -> &'static str {
    concat!(
        "\u{4f60}\u{662f}\u{5b88}\u{671b}\u{5148}\u{950b}\u{5bf9}\u{5c40}\u{9510}\u{8bc4}\u{52a9}\u{624b}\u{3002}",
        "\u{76ee}\u{6807}\u{662f}\u{8ba9}\u{7528}\u{6237}\u{770b}\u{5f97}\u{5f00}\u{5fc3}\u{3002}",
        "\u{80dc}\u{5229}\u{6216}\u{6709}\u{5e26}\u{98de}\u{8bc1}\u{636e}\u{65f6}\u{ff0c}\u{5148}\u{5938}\u{5f53}\u{524d}\u{73a9}\u{5bb6}\u{6709}\u{4e8b}\u{5b9e}\u{652f}\u{6301}\u{7684}\u{4f18}\u{70b9}\u{ff0c}\u{4e5f}\u{53ef}\u{6839}\u{636e}\u{672c}\u{5730}\u{533f}\u{540d}\u{5173}\u{7cfb}\u{4e8b}\u{5b9e}\u{4fcf}\u{76ae}\u{5730}\u{70b9}\u{51fa}\u{533f}\u{540d}\u{5f31}\u{70b9}\u{6216}\u{5927}\u{7239}\u{3002}",
        "\u{5931}\u{5229}\u{65f6}\u{ff0c}\u{53ea}\u{80fd}\u{4f9d}\u{636e}\u{672c}\u{5730}\u{533f}\u{540d}\u{5173}\u{7cfb}\u{4e8b}\u{5b9e}\u{8fdb}\u{884c}\u{5e7d}\u{9ed8}\u{7529}\u{9505}\u{3002}",
        "\u{59cb}\u{7ec8}\u{907f}\u{514d}\u{4ee4}\u{4eba}\u{6cc4}\u{6c14}\u{3001}\u{8fb1}\u{9a82}\u{3001}\u{4ec7}\u{6068}\u{6216}\u{9488}\u{5bf9}\u{8eab}\u{4efd}\u{7684}\u{8868}\u{8fbe}\u{3002}",
        "\u{8bc4}\u{8bed}\u{5fc5}\u{987b}\u{5199}\u{6210}\u{8fde}\u{8d2f}\u{81ea}\u{7136}\u{6bb5}\u{ff0c}\u{4e0d}\u{8981}\u{6807}\u{7b7e}\u{6216}\u{5217}\u{8868}\u{3002}",
        "\u{53ea}\u{80fd}\u{4f7f}\u{7528}\u{7528}\u{6237}\u{6d88}\u{606f}\u{4e2d}\u{7684}\u{533f}\u{540d}\u{4e8b}\u{5b9e}\u{ff0c}",
        "\u{4e0d}\u{5f97}\u{731c}\u{6d4b}\u{82f1}\u{96c4}\u{3001}\u{6570}\u{503c}\u{3001}\u{771f}\u{5b9e}\u{8eab}\u{4efd}\u{6216}\u{9690}\u{79c1}\u{4fe1}\u{606f}\u{3002}",
        "\u{53ea}\u{8fd4}\u{56de} JSON \u{5bf9}\u{8c61}\u{ff0c}\u{4e14}\u{53ea}\u{5305}\u{542b} award\u{3001}commentary\u{3001}usedFactIds\u{3002}",
        "award \u{53ea}\u{7528}\u{4e2d}\u{6587}\u{4e14}\u{4e0d}\u{8d85}\u{8fc7}16\u{5b57}\u{ff1b}",
        "commentary \u{7528}2\u{5230}3\u{53e5}\u{4e2d}\u{6587}\u{81ea}\u{7136}\u{6bb5}\u{ff0c}\u{4e0d}\u{8d85}\u{8fc7}180\u{5b57}\u{ff0c}\u{4e0d}\u{7528} Markdown\u{3002}",
        "usedFactIds \u{81f3}\u{5c11}\u{5f15}\u{7528}2\u{4e2a}\u{63d0}\u{4f9b}\u{7684}\u{4e8b}\u{5b9e} ID\u{ff1b}",
        "\u{5982}\u{679c}\u{5b58}\u{5728}\u{533f}\u{540d}\u{5173}\u{7cfb}\u{5bf9}\u{8c61}\u{ff0c}\u{5fc5}\u{987b}\u{5f15}\u{7528}\u{81f3}\u{5c11}1\u{4e2a}\u{5173}\u{7cfb}\u{4e8b}\u{5b9e}\u{3002}"
    )
}

fn prompt_fact_count(material: RoastPromptMaterial<'_>) -> usize {
    match material {
        RoastPromptMaterial::Match(value) => {
            value.selected_facts.len()
                + value.relationship.as_ref().map_or(0, |relationship| {
                    relationship.facts.len() + relationship.counter_facts.len()
                })
        }
        RoastPromptMaterial::Arena(value) => value.candidate_facts.len(),
    }
}

fn user_prompt(material: RoastPromptMaterial<'_>) -> String {
    let payload = match material {
        RoastPromptMaterial::Match(value) => serde_json::to_string(&match_prompt(value)),
        RoastPromptMaterial::Arena(value) => serde_json::to_string(&arena_prompt(value)),
    }
    .expect("roast prompt payload serializes");
    format!(
        "\u{4f7f}\u{7528}\u{4e0b}\u{5217}\u{533f}\u{540d}\u{6750}\u{6599}\u{751f}\u{6210}\u{4e00}\u{6b21}\u{5bf9}\u{5c40}\u{9510}\u{8bc4}\u{ff0c}\u{4e0d}\u{8981}\u{8f93}\u{51fa} Markdown\u{6216}\u{89e3}\u{91ca}\u{ff1a}{payload}"
    )
}

fn match_prompt(material: &RoastMaterial) -> MatchPrompt<'_> {
    MatchPrompt {
        kind: "match",
        prompt_version: PROMPT_VERSION,
        evaluation: PromptEvaluation {
            primary_tag: &material.evaluation.primary_tag,
            evidence_tags: &material.evaluation.evidence_tags,
        },
        selected_facts: prompt_facts(&material.selected_facts),
        anonymous_relation: material
            .relationship
            .as_ref()
            .map(|relationship| PromptRelationship {
                kind: relationship_kind(&relationship.kind),
                anonymous_subject: relationship_subject(&relationship.subject),
                facts: prompt_facts(&relationship.facts),
                counter_facts: prompt_facts(&relationship.counter_facts),
            }),
        streak: PromptStreak {
            before_current: material.streak.before_current,
            previous_event: material.streak.previous_event.as_deref(),
            current_event: &material.streak.current_event,
        },
    }
}

fn arena_prompt(material: &ArenaRoastMaterial) -> ArenaPrompt<'_> {
    ArenaPrompt {
        kind: "arena",
        prompt_version: PROMPT_VERSION,
        candidate_facts: prompt_facts(&material.candidate_facts),
    }
}

fn prompt_facts(facts: &[RoastFact]) -> Vec<PromptFact<'_>> {
    facts
        .iter()
        .map(|fact| PromptFact {
            id: &fact.id,
            subject: subject(fact.subject),
            text: &fact.text,
        })
        .collect()
}

fn subject(value: FactSubject) -> &'static str {
    match value {
        FactSubject::CurrentPlayer => "current_player",
        FactSubject::RelationshipSubject => "anonymous_relation",
        FactSubject::Team => "team",
        FactSubject::Match => "match",
    }
}

fn relationship_kind(value: &RoastRelationshipKind) -> &'static str {
    match value {
        RoastRelationshipKind::WeakLink => "weak_link",
        RoastRelationshipKind::Carry => "carry",
        RoastRelationshipKind::Scapegoat => "scapegoat",
        RoastRelationshipKind::SelfRoast => "self_roast",
    }
}

fn relationship_subject(value: &RoastRelationshipSubject) -> String {
    match value {
        RoastRelationshipSubject::CurrentPlayer => "\u{5f53}\u{524d}\u{73a9}\u{5bb6}".into(),
        RoastRelationshipSubject::Teammate { role, ordinal } => format!(
            "\u{533f}\u{540d}{}\u{53f7}{}\u{4f4d}",
            ordinal_label(*ordinal),
            role_label(*role)
        ),
    }
}

fn ordinal_label(value: AnonymousOrdinal) -> &'static str {
    match value {
        AnonymousOrdinal::First => "\u{4e00}",
        AnonymousOrdinal::Second => "\u{4e8c}",
        AnonymousOrdinal::Third => "\u{4e09}",
        AnonymousOrdinal::Fourth => "\u{56db}",
        AnonymousOrdinal::Fifth => "\u{4e94}",
    }
}

fn role_label(value: AnonymousPlayerRole) -> &'static str {
    match value {
        AnonymousPlayerRole::Tank => "\u{5766}\u{514b}",
        AnonymousPlayerRole::Damage => "\u{8f93}\u{51fa}",
        AnonymousPlayerRole::Support => "\u{652f}\u{63f4}",
    }
}

fn map_request_error(error: reqwest::Error) -> DeepSeekRoastError {
    if error.is_timeout() {
        DeepSeekRoastError::Timeout
    } else {
        DeepSeekRoastError::Transport
    }
}

fn map_status(status: StatusCode) -> DeepSeekRoastError {
    match status.as_u16() {
        401 => DeepSeekRoastError::InvalidKey,
        402 => DeepSeekRoastError::InsufficientBalance,
        429 => DeepSeekRoastError::TooFrequent,
        500 | 502 | 503 | 504 => DeepSeekRoastError::Busy,
        _ => DeepSeekRoastError::Transport,
    }
}
