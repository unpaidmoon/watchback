use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    report::{evidence_catalog, AiRoastNarrative, AiRoastReport, EvidenceSemantic, EvidenceValue},
    snapshot::RoastSnapshot,
    validator::{normalize_json_input, validate_narrative_output, RoastValidationError},
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
            match validate_narrative_output(normalize_json_input(&content), snapshot) {
                Ok(report) => return Ok(report),
                Err(error) if attempt + 1 < MAX_ATTEMPTS => {
                    append_correction(&mut request, &error, &content, snapshot)
                }
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

fn append_correction(
    request: &mut ChatRequest,
    error: &RoastValidationError,
    content: &str,
    snapshot: &RoastSnapshot,
) {
    let category = match error {
        RoastValidationError::UnknownEvidence => "证据字段不存在",
        RoastValidationError::DuplicateEvidence => "证据字段重复",
        RoastValidationError::UnsupportedNumber => "数字没有对应证据",
        RoastValidationError::UnsupportedClaim => "比较或推断强度不受证据支持",
        RoastValidationError::InvalidSectionCount => "区块条目数量无效",
        RoastValidationError::TextTooLong => "文本长度超限",
        RoastValidationError::Unfriendly => "出现攻击性内容",
        RoastValidationError::RealIdentity => "出现身份或凭据内容",
        _ => "JSON 结构或必填字段无效",
    };
    let location =
        validation_location(error, content, snapshot).unwrap_or_else(|| "整份报告".to_string());
    let instruction = if matches!(
        error,
        RoastValidationError::InvalidJson
            | RoastValidationError::MissingField(_)
            | RoastValidationError::ExtraFields
    ) {
        "严格按系统提示中的 JSON 骨架返回；不得增加字段、注释、Markdown 或骨架外文字。"
    } else if *error == RoastValidationError::UnsupportedNumber {
        "检查并删除整份报告中除英雄或地图固有名称外的所有阿拉伯数字、中文数字、百分比和比分，尤其删除一杀、双杀、三杀等中文数词加杀的写法；改用有证据的最高、靠前、领先或偏低等定性表述。"
    } else if matches!(
        error,
        RoastValidationError::UnknownEvidence | RoastValidationError::DuplicateEvidence
    ) {
        "逐项检查整份报告的 evidenceRefs：每个值必须逐字复制自证据路径目录，同一条目不得重复，禁止使用数组下标路径或缩写。"
    } else if *error == RoastValidationError::UnsupportedClaim {
        "逐项检查整份报告：最高或第一只引用值为一的 Rank；领先、落后、高于、低于必须引用 Rank、Delta 或双方原始值；知识引用必须标为 game_context_inference 并同时引用统计；目标时间全队最高时 currentPlayer 必须引用 teamObjectiveTimeRank。删除无法满足这些条件的断言。"
    } else if matches!(
        error,
        RoastValidationError::EmptyField | RoastValidationError::TextTooLong
    ) {
        "保留所有必填区块并压缩整份报告：标题控制在十六字内，总评控制在七十字内，单条正文控制在一百字内，收尾控制在六十字内，所有可见字符控制在四百五十以内。"
    } else {
        "只修正无效引用或对应结论。"
    };
    request.messages.push(Message{role:"user",content:format!("上一次报告的 {location} 校验失败：{category}。{instruction}保持其他区块语义不变，并重新返回完整 JSON。不得复述输入、错误输出、身份、路径或凭据。")});
}

fn validation_location(
    error: &RoastValidationError,
    content: &str,
    snapshot: &RoastSnapshot,
) -> Option<String> {
    let report = serde_json::from_str::<AiRoastReport>(normalize_json_input(content)).ok()?;
    if *error == RoastValidationError::UnsupportedNumber {
        for (location, text) in [
            ("verdict.title", report.verdict.title.as_str()),
            ("verdict.summary", report.verdict.summary.as_str()),
            ("closingRoast", report.closing_roast.as_str()),
        ] {
            if !super::validator::numeric_mentions_for_diagnostic(text, snapshot).is_empty() {
                return Some(location.to_string());
            }
        }
    }
    if matches!(
        error,
        RoastValidationError::EmptyField | RoastValidationError::TextTooLong
    ) {
        if report.verdict.title.trim().is_empty() || report.verdict.title.chars().count() > 24 {
            return Some("verdict.title".to_string());
        }
        if report.verdict.summary.trim().is_empty() || report.verdict.summary.chars().count() > 100
        {
            return Some("verdict.summary".to_string());
        }
        if report.closing_roast.trim().is_empty() || report.closing_roast.chars().count() > 100 {
            return Some("closingRoast".to_string());
        }
    }
    let catalog = evidence_catalog(snapshot);
    let items = report
        .decisive_factors
        .iter()
        .enumerate()
        .map(|(index, item)| (format!("decisiveFactors[{index}]"), item))
        .chain(std::iter::once((
            "currentPlayer".to_string(),
            &report.current_player,
        )))
        .chain(
            report
                .team_and_matchups
                .iter()
                .enumerate()
                .map(|(index, item)| (format!("teamAndMatchups[{index}]"), &item.insight)),
        );
    for (location, item) in items {
        if matches!(
            error,
            RoastValidationError::EmptyField | RoastValidationError::TextTooLong
        ) && (item.title.trim().is_empty()
            || item.commentary.trim().is_empty()
            || item.title.chars().count() > 24
            || item.commentary.chars().count() > 160)
        {
            return Some(location);
        }
        if matches!(
            error,
            RoastValidationError::UnknownEvidence | RoastValidationError::DuplicateEvidence
        ) && item
            .evidence_refs
            .iter()
            .any(|path| !catalog.contains_key(path))
        {
            return Some(location);
        }
        if *error == RoastValidationError::UnsupportedNumber {
            let entries = item
                .evidence_refs
                .iter()
                .filter_map(|path| catalog.get(path))
                .collect::<Vec<_>>();
            let text = format!("{} {}", item.title, item.commentary);
            if super::validator::numeric_mentions_for_diagnostic(&text, snapshot)
                .into_iter()
                .any(|mention| {
                    !entries.iter().any(|entry| {
                        super::validator::evidence_supports_number_for_diagnostic(
                            &entry.value,
                            mention,
                        )
                    })
                })
            {
                return Some(location);
            }
        }
    }
    None
}

pub fn system_prompt() -> &'static str {
    return r#"你是《守望先锋》赛后损友：先真正理解这局，再说人话。不要写五区块报告，不要引用、扮演或自称任何外部产品人格。
先在 analysis 中回答“如果只能告诉朋友一件事，这局最关键的是什么”，形成唯一 matchThesis；currentPlayer 判断用户真实亮点与责任；keyActors 只选真正带飞或最该背锅的人；supportingFactors 最多两个。优先追究任务的第一责任人，再考虑协助者。涉及法老之鹰、回声等空中威胁时，若我方存在主要对空职责英雄，应结合 taskKnowledge 与结果统计判断其任务是否完成。
胜利时尽量夸用户，但只能夸证据支持的亮点，也要点名真正带飞的大爹。失败时先接住用户情绪，再根据证据找一至两位主要责任人；用户确实有锅也直说。可以口语化使用“坐牢”“头锅”“战犯”“大爹”“这把肯定是他的问题”，但只评价本局行为、数据与职责，不攻击人格、智力、身份或现实生活。
最终可见输出只能是一个有趣的主标题和一篇连续正文，正文建议三百五十至五百个中文字符、硬上限六百五十；不要分节、列清单或套模板。不得虚构站位、换英雄、技能命中、语音、故意送、具体击杀过程等快照没有的事件。
只返回严格 JSON，不得增加字段或骨架外文字：{"analysis":{"matchThesis":{"judgment":"","claimType":"statistical_interpretation","evidenceRefs":[""]},"currentPlayer":{"judgment":"","claimType":"statistical_interpretation","evidenceRefs":[""]},"keyActors":[],"supportingFactors":[]},"title":"","commentary":""}。keyActors 每项为 {"subject":"","impact":"positive|negative","judgment":"","claimType":"direct_fact|statistical_interpretation|game_context_inference","evidenceRefs":[""]}；其他 claim 同样使用 judgment、claimType、evidenceRefs。所有 evidenceRefs 必须逐字来自证据目录；游戏职责推断必须同时引用知识与统计。"#;
    r#"你是《守望先锋》赛后锐评助手。事实优先，使用冷面幽默，像真正懂游戏的损友复盘；不写客服或公关稿，不说教，不解释笑话，也不模仿或自称任何其他模型。

先阅读比赛信息、双方完整计分板、职责对位、当前玩家基准、连续状态、游戏知识和角斗扩展，再依次判断整体故事、主要胜负因素、当前玩家贡献与责任、关键队友或职责差距，以及最有节目效果的数据反差。本地没有预选主锅或大爹；证据不足时不要强行找锅。

解释边界：高承伤不自动等于坦克失职；高伤害要结合击杀、最后一击、参与率和存活；治疗低不自动等于支援失职；目标时间可以解释机会成本但不是万能免责；枪位面对空中威胁只能形成职责推断。不得推测站位、技能命中、具体伤害目标、语音交流、故意送、摆烂或主观意图。

只返回一个 JSON 对象，固定为五区块：verdict（title、summary）、decisiveFactors（2至3条）、currentPlayer、teamAndMatchups（1至3条）和 closingRoast。每个分析条目包含 title、commentary、claimType、evidenceRefs；teamAndMatchups 另含 type。claimType 只能是 direct_fact、statistical_interpretation、game_context_inference；type 只能是 positive_player、negative_player、role_matchup、arena_trend。每个分析条目附 evidenceRefs，不能自创。推断条目必须同时引用统计和游戏知识；引用游戏知识的条目必须标为 game_context_inference。verdict 和 closingRoast 没有 evidenceRefs，因此其中不得写数字、百分比或名次。

严格 JSON 骨架如下，不得增加任何字段或骨架外文字：{"verdict":{"title":"","summary":""},"decisiveFactors":[{"title":"","commentary":"","claimType":"direct_fact","evidenceRefs":[""]},{"title":"","commentary":"","claimType":"statistical_interpretation","evidenceRefs":[""]}],"currentPlayer":{"title":"","commentary":"","claimType":"statistical_interpretation","evidenceRefs":[""]},"teamAndMatchups":[{"type":"role_matchup","title":"","commentary":"","claimType":"statistical_interpretation","evidenceRefs":[""]}],"closingRoast":""}。空字符串必须替换为实际内容或目录中的证据路径。

证据路径只能逐字复制用户消息末尾“证据路径目录”中当前实际存在的字符串，系统提示中的任何说明、英雄名或字段名都不是可复制的证据路径。不得自行拼接路径，不得写 scoreboard.team[0]、roleMatchups[0] 等数组下标路径。

本场判词用强梗但不堆数字；胜负手事实为主；你这局直接诚实；队友与对位只选关键项且同一人不重复处刑；终审锐评一句收尾。除英雄或地图固有名称外，所有可见正文都不要写阿拉伯数字、中文数字、百分比或比分；改用有证据引用支持的最高、靠前、领先、偏低等定性表述。生成目标要明显短于硬上限：整份报告控制在三百五十至四百五十个可见字符，数据不足可缩短；标题控制在十六字内，总评控制在七十字内，每条 commentary 写六十至九十个汉字，收尾控制在六十字内。嘲讽行为、数据结果和职责完成情况，不攻击人格、智力、身份或现实生活。

反例“伤害高所以尽力了”；正例应比较伤害与击杀转化后再判断。反例“目标时间高所以免责”；正例应说明目标时间的机会成本，同时检查战斗数据是否全面垫底。反例“枪位完全没打过法老之鹰”；正例应写“对空任务没有形成结果”，并标为 game_context_inference。不要照抄这些例句。

禁止以“从数据来看”“综合来看”“整体而言”开头；避免“表现较为稳定”“节奏掌控良好”“压制力十足”“已经尽力”“这锅不背”和万能封号。不要为了玩梗篡改事实。"#
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
