#![allow(dead_code)]

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use owstats_desktop_lib::ai_roast::{
    client::{
        system_prompt, AiRoastStreamEvent, DeepSeekRoastClient, DeepSeekRoastError, MAX_TOKENS,
        MODEL, PRODUCTION_TIMEOUT, PROMPT_VERSION,
    },
    knowledge::KnowledgeContext,
    report::{
        AiRoastNarrative, AiRoastReport, ClaimType, NarrativeAnalysis, RoastClaim, RoastInsight,
        RoastVerdict, TeamInsight, TeamInsightKind,
    },
    snapshot::{
        DerivedMetrics, PlayerStats, RoastMatchContext, RoastMatchKind, RoastPlayer,
        RoastScoreboard, RoastSnapshot, INPUT_SCHEMA_VERSION,
    },
    task::TaskKnowledgeContext,
};
use reqwest::Client;
use serde_json::{json, Value};

fn snapshot() -> RoastSnapshot {
    let current = RoastPlayer {
        subject: "我方安娜".into(),
        is_current_player: true,
        hero: "安娜".into(),
        role: Some("support".into()),
        stats: PlayerStats {
            healing: Some(12000),
            deaths: Some(4),
            ..Default::default()
        },
        derived: DerivedMetrics::default(),
    };
    let enemy = RoastPlayer {
        subject: "敌方艾什".into(),
        is_current_player: false,
        hero: "艾什".into(),
        role: Some("damage".into()),
        stats: PlayerStats {
            damage: Some(10000),
            ..Default::default()
        },
        derived: DerivedMetrics::default(),
    };
    RoastSnapshot {
        schema_version: INPUT_SCHEMA_VERSION,
        match_kind: RoastMatchKind::Standard,
        match_context: RoastMatchContext {
            result: Some("loss".into()),
            ..Default::default()
        },
        current_player: "我方安娜".into(),
        scoreboard: RoastScoreboard {
            team: vec![current],
            opponent: vec![enemy],
        },
        role_matchups: Vec::new(),
        current_player_baseline: None,
        streak: None,
        game_knowledge: KnowledgeContext::default(),
        task_knowledge: TaskKnowledgeContext::default(),
        arena: None,
    }
}

fn insight(title: &str, path: &str) -> RoastInsight {
    RoastInsight {
        title: title.into(),
        commentary: "合成事实分析。".into(),
        claim_type: ClaimType::DirectFact,
        evidence_refs: vec![path.into()],
    }
}
fn report() -> AiRoastReport {
    AiRoastReport {
        verdict: RoastVerdict {
            title: "治疗有电，火力停机".into(),
            summary: "这场合成对局输在产出没有接上。".into(),
        },
        decisive_factors: vec![
            insight("治疗续航", "我方安娜.stats.healing"),
            insight("敌方火力", "敌方艾什.stats.damage"),
        ],
        current_player: insight("你这局", "我方安娜.stats.healing"),
        team_and_matchups: vec![TeamInsight {
            kind: TeamInsightKind::NegativePlayer,
            insight: insight("主要压力", "敌方艾什.stats.damage"),
        }],
        closing_roast: "药箱准时到岗，扳机集体请假。".into(),
    }
}
fn narrative() -> AiRoastNarrative {
    let claim = |judgment: &str, refs: Vec<&str>| RoastClaim {
        judgment: judgment.into(),
        claim_type: ClaimType::StatisticalInterpretation,
        evidence_refs: refs.into_iter().map(str::to_string).collect(),
    };
    AiRoastNarrative {
        analysis: NarrativeAnalysis {
            match_thesis: claim("治疗撑住了，但敌方火力更有效", vec!["我方安娜.stats.healing", "敌方艾什.stats.damage"]),
            current_player: claim("你的治疗是亮点", vec!["我方安娜.stats.healing"]),
            key_actors: Vec::new(),
            supporting_factors: Vec::new(),
        },
        title: "药箱到岗，扳机请假".into(),
        commentary: "你的治疗确实撑住了场面，至少不是站在后排看热闹；但敌方艾什的输出更有存在感，我方火力没把治疗创造的空间换成结果。这局输得像药箱准时到岗，扳机集体请假。".into(),
    }
}

fn sse_event(kind: &str, payload: Value) -> String {
    format!("event: {kind}\ndata: {payload}\n\n")
}

fn completion(report: &str, finish: &str) -> String {
    let mut body = sse_event(
        "response.created",
        json!({"type":"response.created","sequence_number":0}),
    );
    match finish {
        "stop" => {
            body.push_str(&sse_event(
                "response.output_text.delta",
                json!({"type":"response.output_text.delta","sequence_number":1,"delta":report}),
            ));
            body.push_str(&sse_event(
                "response.completed",
                json!({"type":"response.completed","sequence_number":2,"response":{"usage":{}}}),
            ));
        }
        "length" => body.push_str(&sse_event(
            "response.incomplete",
            json!({"type":"response.incomplete","sequence_number":1,"response":{"incomplete_details":{"reason":"max_output_tokens"}}}),
        )),
        "content_filter" | "insufficient_system_resource" => body.push_str(&sse_event(
            "response.failed",
            json!({"type":"response.failed","sequence_number":1,"response":{"error":{"code":finish}}}),
        )),
        other => panic!("unsupported finish reason: {other}"),
    }
    body
}

fn streaming_completion(deltas: &[&str]) -> String {
    let mut body = sse_event(
        "response.created",
        json!({"type":"response.created","sequence_number":0}),
    );
    for (index, delta) in deltas.iter().enumerate() {
        body.push_str(&sse_event(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","sequence_number":index + 1,"delta":delta}),
        ));
    }
    body.push_str(&sse_event(
        "response.completed",
        json!({"type":"response.completed","sequence_number":deltas.len() + 1,"response":{"usage":{}}}),
    ));
    body
}

#[derive(Clone)]
struct Reply {
    status: u16,
    body: String,
    delay: Duration,
}
fn server(replies: Vec<Reply>) -> (String, Arc<Mutex<Vec<String>>>, Server) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let count = replies.len();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            captured.lock().unwrap().push(read_request(&mut stream));
            thread::sleep(reply.delay);
            let reason = match reply.status {
                200 => "OK",
                401 => "Unauthorized",
                402 => "Payment Required",
                429 => "Too Many Requests",
                503 => "Service Unavailable",
                _ => "Synthetic",
            };
            let response=format!("HTTP/1.1 {} {}\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",reply.status,reason,reply.body.len(),reply.body);
            let _ = stream.write_all(response.as_bytes());
        }
        let _ = tx.send(());
    });
    (
        format!("http://{address}"),
        requests,
        Server {
            address,
            count,
            rx,
            handle,
        },
    )
}
struct Server {
    address: std::net::SocketAddr,
    count: usize,
    rx: mpsc::Receiver<()>,
    handle: thread::JoinHandle<()>,
}
impl Server {
    fn join(self) {
        if self.rx.recv_timeout(Duration::from_secs(2)).is_err() {
            for _ in 0..self.count {
                let _ = TcpStream::connect(self.address);
            }
        }
        self.handle.join().unwrap();
    }
}
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let mut expected = None;
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                bytes.extend_from_slice(&buffer[..read]);
                if expected.is_none() {
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.split_once(':').and_then(|(name, value)| {
                                    name.eq_ignore_ascii_case("content-length")
                                        .then(|| value.trim().parse::<usize>().unwrap())
                                })
                            })
                            .unwrap_or(0);
                        expected = Some(end + 4 + length);
                    }
                }
                if expected.is_some_and(|length| bytes.len() >= length) {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                break
            }
            Err(error) => panic!("{error}"),
        }
    }
    String::from_utf8(bytes).unwrap()
}
fn body(request: &str) -> Value {
    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}
fn client(url: &str, timeout: Duration) -> DeepSeekRoastClient {
    let client = Client::builder().no_proxy().build().unwrap();
    DeepSeekRoastClient::with_client(client, url, timeout).unwrap()
}

#[test]
fn public_v4_constants_are_fixed() {
    assert_eq!(MODEL, "deepseek-v4-flash");
    assert_eq!(PROMPT_VERSION, 4);
    assert_eq!(MAX_TOKENS, 4000);
    assert_eq!(PRODUCTION_TIMEOUT, Duration::from_secs(60));
}

#[tokio::test]
async fn responses_request_streams_json_and_uses_complete_snapshot() {
    let valid = serde_json::to_string(&narrative()).unwrap();
    let (url, requests, handle) = server(vec![Reply {
        status: 200,
        body: completion(&valid, "stop"),
        delay: Duration::ZERO,
    }]);
    assert_eq!(
        client(&url, Duration::from_secs(2))
            .generate("sk-synthetic", &snapshot())
            .await
            .unwrap(),
        narrative()
    );
    handle.join();
    let captured = requests.lock().unwrap();
    assert!(captured[0].starts_with("POST /responses HTTP/1.1"));
    let request = body(&captured[0]);
    assert_eq!(request["model"], "deepseek-v4-flash");
    assert!(request.get("thinking").is_none());
    assert_eq!(request["reasoning"]["effort"], "none");
    assert_eq!(request["temperature"], 0.8);
    assert_eq!(request["max_output_tokens"], 4000);
    assert_eq!(request["text"]["format"]["type"], "json_object");
    assert_eq!(request["stream"], true);
    assert!(request["instructions"]
        .as_str()
        .unwrap()
        .contains("严格按以下字段顺序"));
    let prompt = request["input"][0]["content"].as_str().unwrap();
    for required in [
        "scoreboard",
        "team",
        "opponent",
        "roleMatchups",
        "gameKnowledge",
    ] {
        assert!(prompt.contains(required));
    }
    for old in ["selectedFacts", "anonymousRelation", "usedFactIds"] {
        assert!(!prompt.contains(old));
    }
    assert!(prompt.contains("证据目录（路径映射到值和语义）"));
    assert!(prompt.contains("\"我方安娜.stats.healing\""));
    assert!(prompt.contains("\"value\":12000"));
    assert!(prompt.contains("\"semantic\":\"raw_stat\""));
}

#[tokio::test]
async fn visible_title_and_commentary_are_emitted_incrementally() {
    let expected = narrative();
    let analysis = serde_json::to_string(&expected.analysis).unwrap();
    let prefix = format!(
        "{{\"title\":{},\"commentary\":\"{}",
        serde_json::to_string(&expected.title).unwrap(),
        "你的治疗确实撑住了场面，"
    );
    let suffix = format!(
        "{}\",\"analysis\":{analysis}}}",
        "至少不是站在后排看热闹；但敌方艾什的输出更有存在感，我方火力没把治疗创造的空间换成结果。这局输得像药箱准时到岗，扳机集体请假。"
    );
    let (url, _, handle) = server(vec![Reply {
        status: 200,
        body: streaming_completion(&[&prefix, &suffix]),
        delay: Duration::ZERO,
    }]);
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);

    let actual = client(&url, Duration::from_secs(2))
        .generate_streaming("sk-synthetic", &snapshot(), move |event| {
            captured.lock().unwrap().push(event);
        })
        .await
        .unwrap();

    handle.join();
    assert_eq!(actual, expected);
    let events = events.lock().unwrap();
    assert_eq!(events.first(), Some(&AiRoastStreamEvent::AttemptStarted));
    assert!(events.iter().any(|event| matches!(
        event,
        AiRoastStreamEvent::Content { title, commentary }
            if title == &expected.title && commentary == "你的治疗确实撑住了场面，"
    )));
    assert_eq!(
        events.last(),
        Some(&AiRoastStreamEvent::Content {
            title: expected.title,
            commentary: expected.commentary,
        })
    );
}

#[test]
fn prompt_v3_encodes_style_structure_and_inference_boundaries() {
    let prompt = system_prompt();
    for required in [
        "如果只能告诉朋友一件事",
        "一个有趣的主标题",
        "一篇连续正文",
        "第一责任人",
        "胜利",
        "失败",
        "evidenceRefs",
    ] {
        assert!(prompt.contains(required), "missing {required}");
    }
    assert!(!prompt.contains("固定为五区块"));
    assert!(!prompt.contains("Grok"));
}

#[tokio::test]
async fn invalid_output_gets_exactly_one_anonymous_correction() {
    let invalid=json!({"verdict":{"title":"坏","summary":"坏"},"decisiveFactors":[],"currentPlayer":{"title":"坏","commentary":"private-first-output","claimType":"direct_fact","evidenceRefs":["missing"]},"teamAndMatchups":[],"closingRoast":"坏"}).to_string();
    let valid = serde_json::to_string(&narrative()).unwrap();
    let (url, requests, handle) = server(vec![
        Reply {
            status: 200,
            body: completion(&invalid, "stop"),
            delay: Duration::ZERO,
        },
        Reply {
            status: 200,
            body: completion(&valid, "stop"),
            delay: Duration::ZERO,
        },
    ]);
    client(&url, Duration::from_secs(2))
        .generate("sk-synthetic", &snapshot())
        .await
        .unwrap();
    handle.join();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let retry = body(&requests[1]);
    let correction = retry["input"].as_array().unwrap().last().unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(correction.contains("校验失败"));
    assert!(correction.contains("matchThesis"));
    assert!(correction.contains("\"keyActors\":[]"));
    assert!(correction.contains("不要在 judgment、title、commentary 中书写阿拉伯数字"));
    assert!(!correction.contains("五区块"));
    assert!(!correction.contains("closingRoast"));
    assert!(!correction.contains("private-first-output"));
    assert!(!correction.contains("我方安娜"));
}

#[tokio::test]
async fn structurally_valid_v3_output_is_not_blocked_by_content_validation() {
    let mut output = narrative();
    output.analysis.match_thesis.evidence_refs = vec!["model-chosen-reference".into()];
    output.analysis.current_player.evidence_refs = vec!["another-model-reference".into()];
    output.commentary = "这把肯定是他的问题，输出位坐牢坐得像来打卡。".into();
    let valid = serde_json::to_string(&output).unwrap();
    let (url, requests, handle) = server(vec![Reply {
        status: 200,
        body: completion(&valid, "stop"),
        delay: Duration::ZERO,
    }]);

    let actual = client(&url, Duration::from_secs(2))
        .generate("sk-synthetic", &snapshot())
        .await
        .unwrap();

    handle.join();
    assert_eq!(actual, output);
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn title_and_commentary_survive_hidden_analysis_shape_drift() {
    let output = json!({
        "analysis": {"modelOwnedReasoning": ["shape may change"]},
        "title": "标题照常回来",
        "commentary": "隐藏分析是模型自己的草稿，桌面端只需要稳定拿到这一篇连续正文。"
    });
    let (url, requests, handle) = server(vec![Reply {
        status: 200,
        body: completion(&output.to_string(), "stop"),
        delay: Duration::ZERO,
    }]);

    let actual = client(&url, Duration::from_secs(2))
        .generate("sk-synthetic", &snapshot())
        .await
        .unwrap();

    handle.join();
    assert_eq!(actual.title, "标题照常回来");
    assert_eq!(
        actual.commentary,
        "隐藏分析是模型自己的草稿，桌面端只需要稳定拿到这一篇连续正文。"
    );
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn second_invalid_output_stops_without_more_retries() {
    let invalid = completion("{}", "stop");
    let (url, requests, handle) = server(vec![
        Reply {
            status: 200,
            body: invalid.clone(),
            delay: Duration::ZERO,
        },
        Reply {
            status: 200,
            body: invalid,
            delay: Duration::ZERO,
        },
    ]);
    let error = client(&url, Duration::from_secs(2))
        .generate("sk-synthetic", &snapshot())
        .await
        .unwrap_err();
    handle.join();
    assert_eq!(error, DeepSeekRoastError::InvalidOutput);
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn status_finish_and_timeout_errors_remain_safe() {
    for (status, expected) in [
        (401, DeepSeekRoastError::InvalidKey),
        (402, DeepSeekRoastError::InsufficientBalance),
        (429, DeepSeekRoastError::TooFrequent),
        (503, DeepSeekRoastError::Busy),
    ] {
        let (url, requests, handle) = server(vec![Reply {
            status,
            body: "{}".into(),
            delay: Duration::ZERO,
        }]);
        assert_eq!(
            client(&url, Duration::from_secs(2))
                .generate("sk-synthetic", &snapshot())
                .await
                .unwrap_err(),
            expected
        );
        handle.join();
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
    for (reason, expected) in [
        ("length", DeepSeekRoastError::Length),
        ("content_filter", DeepSeekRoastError::ContentFilter),
        ("insufficient_system_resource", DeepSeekRoastError::Busy),
    ] {
        let (url, _, handle) = server(vec![Reply {
            status: 200,
            body: completion("{}", reason),
            delay: Duration::ZERO,
        }]);
        assert_eq!(
            client(&url, Duration::from_secs(2))
                .generate("sk-synthetic", &snapshot())
                .await
                .unwrap_err(),
            expected
        );
        handle.join();
    }
    let valid = serde_json::to_string(&narrative()).unwrap();
    let (url, _, handle) = server(vec![Reply {
        status: 200,
        body: completion(&valid, "stop"),
        delay: Duration::from_millis(150),
    }]);
    assert_eq!(
        client(&url, Duration::from_millis(30))
            .generate("sk-synthetic", &snapshot())
            .await
            .unwrap_err(),
        DeepSeekRoastError::Timeout
    );
    handle.join();
}

#[tokio::test]
async fn empty_key_is_rejected_without_network() {
    let client = DeepSeekRoastClient::with_client(
        Client::new(),
        "http://127.0.0.1:1",
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(
        client.generate(" ", &snapshot()).await.unwrap_err(),
        DeepSeekRoastError::InvalidKey
    );
}
