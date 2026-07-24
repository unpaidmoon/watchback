use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    report::{
        evidence_catalog, AiRoastNarrative, ClaimType, EvidenceSemantic, EvidenceValue,
        NarrativeAnalysis, RoastClaim,
    },
    snapshot::RoastSnapshot,
    validator::{normalize_json_input, RoastValidationError},
};

pub const MODEL: &str = "deepseek-v4-flash";
pub const PROMPT_VERSION: i64 = 3;
pub const MAX_TOKENS: u32 = 4_000;
pub const PRODUCTION_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
const MAX_ATTEMPTS: usize = 2;
static LAST_VALIDATION_FAILURE: AtomicU8 = AtomicU8::new(0);

#[doc(hidden)]
pub fn take_validation_failure_category() -> Option<&'static str> {
    match LAST_VALIDATION_FAILURE.swap(0, Ordering::AcqRel) {
        1 => Some("json"),
        2 => Some("sections"),
        3 => Some("text"),
        4 => Some("evidence"),
        5 => Some("number"),
        6 => Some("claim"),
        7 => Some("safety"),
        8 => Some("text_total"),
        9 => Some("text_field"),
        10 => Some("text_empty"),
        11 => Some("number_percent"),
        12 => Some("number_other"),
        13 => Some("response_shape"),
        14 => Some("response_length"),
        15 => Some("response_filter"),
        16 => Some("finish_reason"),
        17 => Some("number_ascii"),
        18 => Some("number_chinese"),
        _ => None,
    }
}

#[derive(Clone)]
pub struct DeepSeekRoastClient {
    client: Client,
    base_url: String,
    timeout: Duration,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DeepSeekRoastError {
    #[error("DeepSeek API Key 无效")]
    InvalidKey,
    #[error("DeepSeek 余额不足")]
    InsufficientBalance,
    #[error("请求过于频繁，请稍后再试")]
    TooFrequent,
    #[error("DeepSeek 服务暂时繁忙")]
    Busy,
    #[error("生成超时，请稍后再试")]
    Timeout,
    #[error("无法连接 DeepSeek 服务")]
    Transport,
    #[error("生成内容超出长度限制")]
    Length,
    #[error("生成内容被安全规则拦截")]
    ContentFilter,
    #[error("DeepSeek 返回了无效内容")]
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
        snapshot: &RoastSnapshot,
    ) -> Result<AiRoastNarrative, DeepSeekRoastError> {
        LAST_VALIDATION_FAILURE.store(0, Ordering::Release);
        if api_key.trim().is_empty() {
            return Err(DeepSeekRoastError::InvalidKey);
        }
        let request = build_request(snapshot);
        tokio::time::timeout(
            self.timeout,
            self.generate_with_correction(api_key, request, snapshot),
        )
        .await
        .map_err(|_| DeepSeekRoastError::Timeout)?
    }

    async fn generate_with_correction(
        &self,
        api_key: &str,
        mut request: ChatRequest,
        snapshot: &RoastSnapshot,
    ) -> Result<AiRoastNarrative, DeepSeekRoastError> {
        for attempt in 0..MAX_ATTEMPTS {
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
            let completion = serde_json::from_slice::<CompletionResponse>(&body).map_err(|_| {
                LAST_VALIDATION_FAILURE.store(13, Ordering::Release);
                DeepSeekRoastError::InvalidOutput
            })?;
            let choice = completion.choices.into_iter().next().ok_or_else(|| {
                LAST_VALIDATION_FAILURE.store(13, Ordering::Release);
                DeepSeekRoastError::InvalidOutput
            })?;
            match choice.finish_reason.as_deref() {
                Some("stop") => {}
                Some("length") => {
                    LAST_VALIDATION_FAILURE.store(14, Ordering::Release);
                    return Err(DeepSeekRoastError::Length);
                }
                Some("content_filter") => {
                    LAST_VALIDATION_FAILURE.store(15, Ordering::Release);
                    return Err(DeepSeekRoastError::ContentFilter);
                }
                Some("insufficient_system_resource") => return Err(DeepSeekRoastError::Busy),
                _ => {
                    LAST_VALIDATION_FAILURE.store(16, Ordering::Release);
                    return Err(DeepSeekRoastError::InvalidOutput);
                }
            }
            let content = choice.message.content.unwrap_or_default();
            match parse_visible_narrative(&content) {
                Ok(report) => return Ok(report),
                Err(error) if attempt + 1 < MAX_ATTEMPTS => append_correction(&mut request, &error),
                Err(error) => {
                    LAST_VALIDATION_FAILURE.store(
                        detailed_validation_failure_code(&error, &content, snapshot),
                        Ordering::Release,
                    );
                    return Err(DeepSeekRoastError::InvalidOutput);
                }
            }
        }
        Err(DeepSeekRoastError::InvalidOutput)
    }
}

#[derive(Deserialize)]
struct VisibleNarrative {
    title: String,
    commentary: String,
}

fn parse_visible_narrative(content: &str) -> Result<AiRoastNarrative, RoastValidationError> {
    if let Ok(report) = serde_json::from_str::<AiRoastNarrative>(normalize_json_input(content)) {
        return Ok(report);
    }
    let visible = serde_json::from_str::<VisibleNarrative>(normalize_json_input(content))
        .map_err(|_| RoastValidationError::InvalidJson)?;
    let empty_claim = || RoastClaim {
        judgment: String::new(),
        claim_type: ClaimType::StatisticalInterpretation,
        evidence_refs: Vec::new(),
    };
    Ok(AiRoastNarrative {
        analysis: NarrativeAnalysis {
            match_thesis: empty_claim(),
            current_player: empty_claim(),
            key_actors: Vec::new(),
            supporting_factors: Vec::new(),
        },
        title: visible.title,
        commentary: visible.commentary,
    })
}

fn validation_failure_code(error: &RoastValidationError) -> u8 {
    match error {
        RoastValidationError::InvalidJson
        | RoastValidationError::MissingField(_)
        | RoastValidationError::ExtraFields => 1,
        RoastValidationError::InvalidSectionCount => 2,
        RoastValidationError::EmptyField | RoastValidationError::TextTooLong => 3,
        RoastValidationError::UnknownEvidence | RoastValidationError::DuplicateEvidence => 4,
        RoastValidationError::UnsupportedNumber => 5,
        RoastValidationError::UnsupportedClaim => 6,
        RoastValidationError::Unfriendly | RoastValidationError::RealIdentity => 7,
    }
}

fn detailed_validation_failure_code(
    error: &RoastValidationError,
    content: &str,
    snapshot: &RoastSnapshot,
) -> u8 {
    if *error == RoastValidationError::InvalidJson
        && serde_json::from_str::<serde_json::Value>(normalize_json_input(content)).is_ok()
    {
        return 13;
    }
    if *error == RoastValidationError::UnsupportedNumber {
        if let Ok(report) = serde_json::from_str::<AiRoastNarrative>(normalize_json_input(content))
        {
            let visible = [report.title.as_str(), report.commentary.as_str()];
            if visible
                .iter()
                .flat_map(|text| super::validator::numeric_mentions_for_diagnostic(text, snapshot))
                .any(|mention| mention.percent)
            {
                return 11;
            }
            if visible
                .iter()
                .any(|text| super::validator::has_ascii_number_for_diagnostic(text, snapshot))
            {
                return 17;
            }
            return 18;
        }
    }
    if matches!(
        error,
        RoastValidationError::EmptyField | RoastValidationError::TextTooLong
    ) {
        if let Ok(report) = serde_json::from_str::<AiRoastNarrative>(normalize_json_input(content))
        {
            let visible = [report.title.as_str(), report.commentary.as_str()];
            if visible.iter().any(|text| text.trim().is_empty()) {
                return 10;
            }
            if report.title.chars().count() > 36 || report.commentary.chars().count() > 650 {
                return 9;
            }
            return 8;
        }
    }
    validation_failure_code(error)
}

#[derive(Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
    thinking: Thinking,
    reasoning_effort: &'static str,
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

fn build_request(snapshot: &RoastSnapshot) -> ChatRequest {
    let evidence_directory = evidence_catalog(snapshot)
        .into_iter()
        .map(|(path, entry)| {
            let value = match entry.value {
                EvidenceValue::Integer(value) => serde_json::json!(value),
                EvidenceValue::Decimal(value) => serde_json::json!(value),
                EvidenceValue::Text(value) => serde_json::json!(value),
            };
            let semantic = match entry.semantic {
                EvidenceSemantic::RawStat => "raw_stat",
                EvidenceSemantic::Rank => "rank",
                EvidenceSemantic::Share => "share",
                EvidenceSemantic::Delta => "delta",
                EvidenceSemantic::RoundResult => "round_result",
                EvidenceSemantic::FinalWorth => "final_worth",
                EvidenceSemantic::GameKnowledge => "game_knowledge",
            };
            (path, serde_json::json!({"value":value,"semantic":semantic}))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let current_objective_requirement = snapshot
        .scoreboard
        .team
        .iter()
        .find(|player| player.subject == snapshot.current_player)
        .and_then(|player| {
            (player.derived.team_objective_time_rank == Some(1)).then(|| {
                format!(
                    "currentPlayer.evidenceRefs 必须包含 {}.derived.teamObjectiveTimeRank。",
                    player.subject
                )
            })
        })
        .unwrap_or_default();
    let mut request = ChatRequest{
        model:MODEL,
        messages:vec![Message{role:"system",content:system_prompt().to_string()},Message{role:"user",content:format!("以下是匿名、结构化的完整对局快照，以及由同一快照生成的证据目录（路径映射到值和语义）。只依据这些内容生成五区块 JSON 报告；evidenceRefs 必须逐字选自目录键，正文数字必须与目录值一致，claimType 必须符合目录语义。{current_objective_requirement}凡使用最高、第一、最低、垫底、领先、落后、高于或低于，必须在同一条目的 evidenceRefs 中引用支持该比较的 rank、delta 或双方 raw_stat；拿不准就改写为不含比较词的事实描述。所有可见文本禁止出现击杀数量写法，包括“一杀”至“百杀”、“双杀”和任何其他中文数词加“杀”；只写击杀表现靠前、偏低或转化不足，并引用对应证据。不要使用因为……所以……、整局没打过、失职、拖累、背锅、免责、洗白等因果或归责套话。每条 commentary 写到六十至九十个汉字，完整报告目标三百五十至四百五十个可见字符。\n快照：{}\n证据目录（路径映射到值和语义）：{}",serde_json::to_string(snapshot).expect("snapshot serializes"),serde_json::to_string(&evidence_directory).expect("evidence directory serializes"))}],
        max_tokens:MAX_TOKENS,temperature:0.8,thinking:Thinking{kind:"enabled"},reasoning_effort:"high",response_format:ResponseFormat{kind:"json_object"},stream:false,
    };
    request.messages[1].content = format!(
        "以下是匿名完整对局快照及证据目录（路径映射到值和语义）。只依据它们生成 V3 JSON。evidenceRefs 必须逐字选自目录键；比较结论需引用 rank、delta 或双方原始统计；涉及英雄任务与克制关系时，必须同时引用 taskKnowledge/gameKnowledge 与相关统计。不得虚构站位、换人、技能使用、语音交流或具体击杀过程。\n快照：{}\n证据目录（路径映射到值和语义）：{}",
        serde_json::to_string(snapshot).expect("snapshot serializes"),
        serde_json::to_string(&evidence_directory).expect("evidence directory serializes")
    );
    request
}

fn append_correction(request: &mut ChatRequest, error: &RoastValidationError) {
    let category = match error {
        RoastValidationError::UnknownEvidence => "evidenceRefs 中存在证据目录之外的路径",
        RoastValidationError::DuplicateEvidence => "同一条判断重复引用了证据",
        RoastValidationError::UnsupportedNumber => "可见正文中的数字缺少对应证据",
        RoastValidationError::UnsupportedClaim => "判断强度、比较关系或职责推断不受引用证据支持",
        RoastValidationError::InvalidSectionCount => "analysis 中的关键人物或次要因素数量超限",
        RoastValidationError::TextTooLong => "标题或连续正文超过长度上限",
        RoastValidationError::Unfriendly => "出现了人格攻击或不允许的过程断言",
        RoastValidationError::RealIdentity => "出现了真实身份或凭据内容",
        _ => "JSON 结构或必填字段无效",
    };
    request.messages.push(Message {
        role: "user",
        content: format!(
            "上一次 V3 锐评校验失败：{category}。只修正对应问题，并重新返回完整 JSON。严格照此骨架，不增加字段或外层文字：{{\"analysis\":{{\"matchThesis\":{{\"judgment\":\"\",\"claimType\":\"statistical_interpretation\",\"evidenceRefs\":[\"\"]}},\"currentPlayer\":{{\"judgment\":\"\",\"claimType\":\"statistical_interpretation\",\"evidenceRefs\":[\"\"]}},\"keyActors\":[],\"supportingFactors\":[]}},\"title\":\"\",\"commentary\":\"\"}}。matchThesis 至少引用两条证据，并且来自两个不同玩家或不同维度；currentPlayer 至少引用一条以当前玩家 subject 开头的证据。凡 evidenceRefs 包含 taskKnowledge 或 gameKnowledge，claimType 必须是 game_context_inference 且还要引用至少一条玩家统计；不含知识引用时不要使用 game_context_inference。保留一个主线，supportingFactors 最多两个，negative keyActors 最多两个。不要在 judgment、title、commentary 中书写阿拉伯数字、百分比或比分，用有证据支持的定性比较表达。evidenceRefs 必须逐字取自原证据目录。不要复述上一次输出、输入数据、身份、路径或凭据。"
        ),
    });
}

pub fn system_prompt() -> &'static str {
    r#"你是《守望先锋》赛后损友：先真正理解这局，再说人话。不要写五区块报告，不要引用、扮演或自称任何外部产品人格。
先在 analysis 中回答“如果只能告诉朋友一件事，这局最关键的是什么”，形成唯一 matchThesis；currentPlayer 判断用户真实亮点与责任；keyActors 只选真正带飞或最该背锅的人；supportingFactors 最多两个。优先追究任务的第一责任人，再考虑协助者。涉及法老之鹰、回声等空中威胁时，若我方存在主要对空职责英雄，应结合 taskKnowledge 与结果统计判断其任务是否完成。
胜利时尽量夸用户，但只能夸证据支持的亮点，也要点名真正带飞的大爹。失败时先接住用户情绪，再根据证据找一至两位主要责任人；用户确实有锅也直说。可以口语化使用“坐牢”“头锅”“战犯”“大爹”“这把肯定是他的问题”，但只评价本局行为、数据与职责，不攻击人格、智力、身份或现实生活。
最终可见输出只能是一个有趣的主标题和一篇连续正文，正文建议三百五十至五百个中文字符、硬上限六百五十；不要分节、列清单或套模板。judgment、title、commentary 不书写阿拉伯数字、百分比或比分，用有证据支持的定性比较表达。不得虚构站位、换英雄、技能命中、语音、故意送、具体击杀过程等快照没有的事件。
只返回严格 JSON，不得增加字段或骨架外文字：{"analysis":{"matchThesis":{"judgment":"","claimType":"statistical_interpretation","evidenceRefs":[""]},"currentPlayer":{"judgment":"","claimType":"statistical_interpretation","evidenceRefs":[""]},"keyActors":[],"supportingFactors":[]},"title":"","commentary":""}。keyActors 每项为 {"subject":"","impact":"positive|negative","judgment":"","claimType":"direct_fact|statistical_interpretation|game_context_inference","evidenceRefs":[""]}；其他 claim 同样使用 judgment、claimType、evidenceRefs。matchThesis 至少引用两条证据，并且来自两个不同玩家或不同维度；currentPlayer 至少引用一条以当前玩家 subject 开头的证据。凡 evidenceRefs 包含 taskKnowledge 或 gameKnowledge，claimType 必须是 game_context_inference 且还要引用至少一条玩家统计；不含知识引用时不要使用 game_context_inference。所有 evidenceRefs 必须逐字来自证据目录。"#
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
