use owstats_desktop_lib::ai_roast::{
    arena::ArenaRoastMaterial,
    client::{
        DeepSeekRoastClient, DeepSeekRoastError, RoastPromptMaterial, MAX_TOKENS, MODEL,
        PRODUCTION_TIMEOUT, PROMPT_VERSION,
    },
    material::{
        AnonymousOrdinal, AnonymousPlayerRole, FactSubject, RoastEvaluation, RoastFact,
        RoastFactEvidence, RoastMaterial, RoastRelationship, RoastRelationshipKind,
        RoastRelationshipSubject, RoastStreakContext, MATERIAL_VERSION,
    },
    validator::{validate_roast_output, RoastValidationContext, RoastValidationError},
};
use reqwest::Client;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const AWARD: &str = "\u{5e26}\u{98de}\u{5956}";
const COMMENTARY: &str = "\u{4f60}\u{8fd9}\u{5c40}\u{51fb}\u{6740}\u{6392}\u{7b2c}1\u{ff0c}\u{8f93}\u{51fa}\u{50cf}\u{5f00}\u{4e86}\u{5bfc}\u{822a}\u{3002}\u{533f}\u{540d}\u{961f}\u{53cb}\u{6cbb}\u{7597}\u{4e5f}\u{7a33}\u{ff0c}\u{8d62}\u{6cd5}\u{5199}\u{5f97}\u{660e}\u{660e}\u{767d}\u{767d}\u{3002}";
const UNKNOWN_HERO: &str = "\u{795e}\u{79d8}\u{82f1}\u{96c4}";

fn fact(id: &str, text: &str, subject: FactSubject) -> RoastFact {
    RoastFact {
        id: id.into(),
        subject,
        text: text.into(),
        strength: 0.9,
        evidence: RoastFactEvidence::Contribution,
    }
}

fn material() -> RoastMaterial {
    RoastMaterial {
        material_version: MATERIAL_VERSION,
        evaluation: RoastEvaluation {
            rule_version: 1,
            primary_tag: "\u{5e26}\u{98de}\u{5c40}".into(),
            evidence_tags: vec!["\u{51fb}\u{6740}\u{6700}\u{591a}".into()],
            confidence: 0.88,
        },
        selected_facts: vec![
            fact(
                "current_player:kills_rank",
                "\u{5f53}\u{524d}\u{73a9}\u{5bb6}\u{51fb}\u{6740}\u{6392}\u{540d}\u{7b2c}1",
                FactSubject::CurrentPlayer,
            ),
            fact(
                "current_player:damage_rank",
                "\u{5f53}\u{524d}\u{73a9}\u{5bb6}\u{4f24}\u{5bb3}\u{6392}\u{540d}\u{7b2c}2",
                FactSubject::CurrentPlayer,
            ),
        ],
        relationship: Some(RoastRelationship {
            kind: RoastRelationshipKind::Carry,
            subject: RoastRelationshipSubject::Teammate {
                role: AnonymousPlayerRole::Support,
                ordinal: AnonymousOrdinal::Second,
            },
            facts: vec![fact(
                "relationship_subject:healing_rank",
                "\u{533f}\u{540d}\u{5173}\u{7cfb}\u{5bf9}\u{8c61}\u{6cbb}\u{7597}\u{6392}\u{540d}\u{7b2c}1",
                FactSubject::RelationshipSubject,
            )],
            counter_facts: Vec::new(),
            confidence: 0.81,
        }),
        streak: RoastStreakContext {
            before_current: 2,
            previous_event: Some("\u{80dc}\u{5229}".into()),
            current_event: "\u{80dc}\u{5229}".into(),
        },
    }
}

fn arena_material() -> ArenaRoastMaterial {
    ArenaRoastMaterial {
        candidate_facts: vec![
            fact(
                "match:arena_result",
                "\u{89d2}\u{6597}\u{5bf9}\u{5c40}\u{4ee5}4\u{6bd4}3\u{80dc}\u{5229}",
                FactSubject::Match,
            ),
            fact(
                "current_player:arena_funds_rank",
                "\u{5f53}\u{524d}\u{73a9}\u{5bb6}\u{6700}\u{7ec8}\u{8d44}\u{91d1}\u{6392}\u{540d}\u{7b2c}2",
                FactSubject::CurrentPlayer,
            ),
        ],
    }
}

fn allowed_terms() -> HashSet<String> {
    [
        "\u{5e26}\u{98de}\u{5c40}",
        "\u{51fb}\u{6740}\u{6700}\u{591a}",
        "\u{80dc}\u{5229}",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn valid_output() -> String {
    json!({
        "award": AWARD,
        "commentary": COMMENTARY,
        "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
    })
    .to_string()
}

fn completion(content: &str, finish_reason: &str) -> String {
    json!({
        "choices": [{"message": {"content": content}, "finish_reason": finish_reason}]
    })
    .to_string()
}

#[derive(Clone)]
struct Reply {
    status: u16,
    body: String,
    delay: Duration,
}

fn loopback_server(
    replies: Vec<Reply>,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
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
                500 => "Internal Server Error",
                503 => "Service Unavailable",
                _ => "Synthetic",
            };
            let response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.status,
                reason,
                reply.body.len(),
                reply.body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{address}"), requests, handle)
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let mut expected_length = None;
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                bytes.extend_from_slice(&buffer[..read]);
                if expected_length.is_none() {
                    if let Some(header_end) = bytes.windows(4).position(|item| item == b"\r\n\r\n")
                    {
                        let headers = String::from_utf8_lossy(&bytes[..header_end]);
                        let content_length = headers
                            .lines()
                            .find_map(|line| {
                                line.split_once(':').and_then(|(name, value)| {
                                    name.eq_ignore_ascii_case("content-length")
                                        .then(|| value.trim().parse::<usize>().unwrap())
                                })
                            })
                            .unwrap_or(0);
                        expected_length = Some(header_end + 4 + content_length);
                    }
                }
                if expected_length.is_some_and(|length| bytes.len() >= length) {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                break;
            }
            Err(error) => panic!("loopback read failed: {error}"),
        }
    }
    String::from_utf8(bytes).unwrap()
}

fn client(base_url: &str, timeout: Duration) -> DeepSeekRoastClient {
    DeepSeekRoastClient::with_client(Client::new(), base_url, timeout).unwrap()
}

fn request_body(request: &str) -> Value {
    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

#[tokio::test]
async fn sends_fixed_contract_with_minimal_prompt() {
    let (url, requests, handle) = loopback_server(vec![Reply {
        status: 200,
        body: completion(&valid_output(), "stop"),
        delay: Duration::ZERO,
    }]);
    let output = client(&url, Duration::from_secs(5))
        .generate(
            "sk-synthetic-only",
            RoastPromptMaterial::Match(&material()),
            &allowed_terms(),
        )
        .await
        .unwrap();
    handle.join().unwrap();

    assert_eq!(output.award, AWARD);
    assert_eq!(MODEL, "deepseek-v4-flash");
    assert_eq!(PROMPT_VERSION, 1);
    assert_eq!(MAX_TOKENS, 500);
    assert_eq!(PRODUCTION_TIMEOUT, Duration::from_secs(60));

    let request = &requests.lock().unwrap()[0];
    assert!(request.starts_with("POST /chat/completions HTTP/1.1"));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer sk-synthetic-only"));
    let body = request_body(request);
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["max_tokens"], 500);
    assert_eq!(body["thinking"], json!({"type": "disabled"}));
    assert_eq!(body["response_format"], json!({"type": "json_object"}));
    assert_eq!(body["stream"], false);
    let prompt = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["content"].as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(prompt.contains("usedFactIds"));
    assert!(prompt.contains("promptVersion"));
    assert!(prompt.contains("anonymousRelation"));
    for forbidden in [
        "bnet_id",
        "bnetId",
        "player_name",
        "playerName",
        "raw_json",
        "credential",
        "cookie",
        "authorization",
        "\"confidence\":",
        "\"strength\":",
        "\"evidence\":",
        "materialVersion",
        "ruleVersion",
        "sk-synthetic-only",
    ] {
        assert!(!prompt.contains(forbidden), "prompt leaked {forbidden}");
    }
}

#[tokio::test]
async fn arena_prompt_contains_only_candidate_facts() {
    let output = json!({
        "award": "\u{89d2}\u{6597}\u{5956}",
        "commentary": "\u{8fd9}\u{573a}\u{89d2}\u{6597}\u{4ee5}4\u{6bd4}3\u{6536}\u{5c3e}\u{ff0c}\u{8282}\u{594f}\u{62c9}\u{6ee1}\u{3002}\u{6700}\u{7ec8}\u{8d44}\u{91d1}\u{6392}\u{7b2c}2\u{ff0c}\u{8be5}\u{82b1}\u{7684}\u{90fd}\u{82b1}\u{4e86}\u{3002}",
        "usedFactIds": ["match:arena_result", "current_player:arena_funds_rank"]
    })
    .to_string();
    let (url, requests, handle) = loopback_server(vec![Reply {
        status: 200,
        body: completion(&output, "stop"),
        delay: Duration::ZERO,
    }]);
    client(&url, Duration::from_secs(2))
        .generate(
            "sk-synthetic-only",
            RoastPromptMaterial::Arena(&arena_material()),
            &allowed_terms(),
        )
        .await
        .unwrap();
    handle.join().unwrap();

    let prompt = request_body(&requests.lock().unwrap()[0]).to_string();
    assert!(prompt.contains("candidateFacts"));
    assert!(!prompt.contains("anonymousRelation"));
    assert!(!prompt.contains("evaluation"));
    assert!(!prompt.contains("streak"));
}

#[tokio::test]
async fn status_errors_are_safe_and_not_retried() {
    for (status, expected) in [
        (401, DeepSeekRoastError::InvalidKey),
        (402, DeepSeekRoastError::InsufficientBalance),
        (429, DeepSeekRoastError::TooFrequent),
        (500, DeepSeekRoastError::Busy),
        (503, DeepSeekRoastError::Busy),
    ] {
        let secret = "synthetic-secret-body-id-path-key";
        let (url, requests, handle) = loopback_server(vec![Reply {
            status,
            body: secret.into(),
            delay: Duration::ZERO,
        }]);
        let error = client(&url, Duration::from_secs(2))
            .generate(
                "sk-synthetic-only",
                RoastPromptMaterial::Match(&material()),
                &allowed_terms(),
            )
            .await
            .unwrap_err();
        handle.join().unwrap();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains(secret));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn finish_errors_are_safe_and_not_retried() {
    for (finish_reason, expected) in [
        ("length", DeepSeekRoastError::Length),
        ("content_filter", DeepSeekRoastError::ContentFilter),
        ("insufficient_system_resource", DeepSeekRoastError::Busy),
        ("synthetic-unknown", DeepSeekRoastError::InvalidOutput),
    ] {
        let (url, requests, handle) = loopback_server(vec![Reply {
            status: 200,
            body: completion(&valid_output(), finish_reason),
            delay: Duration::ZERO,
        }]);
        let error = client(&url, Duration::from_secs(2))
            .generate(
                "sk-synthetic-only",
                RoastPromptMaterial::Match(&material()),
                &allowed_terms(),
            )
            .await
            .unwrap_err();
        handle.join().unwrap();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains(finish_reason));
        assert_eq!(requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn timeout_is_safe_and_not_retried() {
    let (url, requests, handle) = loopback_server(vec![Reply {
        status: 200,
        body: completion(&valid_output(), "stop"),
        delay: Duration::from_millis(150),
    }]);
    let error = client(&url, Duration::from_millis(30))
        .generate(
            "sk-synthetic-only",
            RoastPromptMaterial::Match(&material()),
            &allowed_terms(),
        )
        .await
        .unwrap_err();
    handle.join().unwrap();
    assert_eq!(error, DeepSeekRoastError::Timeout);
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_json_and_validation_retry_once() {
    let invalid_output = json!({
        "award": AWARD,
        "commentary": "\u{53ea}\u{6709}\u{4e00}\u{53e5}\u{3002}",
        "usedFactIds": ["current_player:kills_rank"]
    })
    .to_string();
    for first_body in ["not-json".into(), completion(&invalid_output, "stop")] {
        let (url, requests, handle) = loopback_server(vec![
            Reply {
                status: 200,
                body: first_body,
                delay: Duration::ZERO,
            },
            Reply {
                status: 200,
                body: completion(&valid_output(), "stop"),
                delay: Duration::ZERO,
            },
        ]);
        let output = client(&url, Duration::from_secs(2))
            .generate(
                "sk-synthetic-only",
                RoastPromptMaterial::Match(&material()),
                &allowed_terms(),
            )
            .await
            .unwrap();
        handle.join().unwrap();
        assert_eq!(output.award, AWARD);
        assert_eq!(requests.lock().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn a_second_invalid_response_stops_retrying() {
    let (url, requests, handle) = loopback_server(vec![
        Reply {
            status: 200,
            body: completion("", "stop"),
            delay: Duration::ZERO,
        },
        Reply {
            status: 200,
            body: completion(" ", "stop"),
            delay: Duration::ZERO,
        },
    ]);
    let error = client(&url, Duration::from_secs(2))
        .generate(
            "sk-synthetic-only",
            RoastPromptMaterial::Match(&material()),
            &allowed_terms(),
        )
        .await
        .unwrap_err();
    handle.join().unwrap();
    assert_eq!(error, DeepSeekRoastError::InvalidOutput);
    assert_eq!(requests.lock().unwrap().len(), 2);
}

fn match_context() -> RoastValidationContext {
    RoastValidationContext::for_match(&material(), allowed_terms())
}

fn assert_invalid(value: Value, expected: RoastValidationError) {
    assert_eq!(
        validate_roast_output(&value.to_string(), &match_context()).unwrap_err(),
        expected
    );
}

#[test]
fn strict_shape_award_and_commentary_validation() {
    assert_eq!(
        validate_roast_output("not-json", &match_context()).unwrap_err(),
        RoastValidationError::InvalidJson
    );
    assert_invalid(
        json!({"award": AWARD, "commentary": COMMENTARY}),
        RoastValidationError::MissingField("usedFactIds"),
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"],
            "extra": true
        }),
        RoastValidationError::ExtraFields,
    );
    assert_invalid(
        json!({
            "award": "",
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::EmptyAward,
    );
    assert_invalid(
        json!({
            "award": "MVP\u{5956}",
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::AwardTooLongOrNotChinese,
    );
    assert_invalid(
        json!({
            "award": "\u{5956}".repeat(17),
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::AwardTooLongOrNotChinese,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": "\u{53ea}\u{6709}\u{4e00}\u{53e5}\u{3002}",
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::SentenceCount,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": format!("{}\u{3002}{}\u{3002}", "\u{597d}".repeat(100), "\u{7a33}".repeat(100)),
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::CommentaryTooLong,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": "# \u{603b}\u{7ed3}\u{3002}- \u{51fb}\u{6740}\u{7b2c}1\u{3002}",
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::UnsupportedFormat,
    );
}

#[test]
fn facts_numbers_and_terms_must_be_grounded() {
    assert_eq!(
        validate_roast_output(&valid_output(), &match_context())
            .unwrap()
            .award,
        AWARD
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank", "unknown:fact"]
        }),
        RoastValidationError::UnknownFact,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank", "current_player:kills_rank"]
        }),
        RoastValidationError::DuplicateFact,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank"]
        }),
        RoastValidationError::InsufficientFacts,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": COMMENTARY,
            "usedFactIds": ["current_player:kills_rank", "current_player:damage_rank"]
        }),
        RoastValidationError::MissingRelationshipFact,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": "\u{4f60}\u{8fd9}\u{5c40}\u{51fb}\u{6740}\u{6392}\u{7b2c}99\u{ff0c}\u{53cd}\u{5411}\u{767b}\u{9876}\u{3002}\u{533f}\u{540d}\u{961f}\u{53cb}\u{6cbb}\u{7597}\u{4e5f}\u{7a33}\u{ff0c}\u{6ca1}\u{7559}\u{7f3a}\u{53e3}\u{3002}",
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::UnsupportedNumber,
    );
    assert_invalid(
        json!({
            "award": AWARD,
            "commentary": "\u{4f60}\u{8fd9}\u{5c40}\u{7edd}\u{5bf9}\u{5168}\u{573a}\u{6700}\u{5f3a}\u{ff0c}\u{8c01}\u{6765}\u{90fd}\u{4e0d}\u{884c}\u{3002}\u{533f}\u{540d}\u{961f}\u{53cb}\u{53ea}\u{80fd}\u{4ef0}\u{671b}\u{ff0c}\u{6beb}\u{65e0}\u{53cd}\u{4f8b}\u{3002}",
            "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
        }),
        RoastValidationError::UnsupportedClaim,
    );
}

#[test]
fn quoted_hero_terms_require_the_allowlist() {
    let value = json!({
        "award": AWARD,
        "commentary": format!("\u{4f60}\u{7528}\u{300c}{UNKNOWN_HERO}\u{300d}\u{6253}\u{51fa}\u{4e86}\u{8282}\u{594f}\u{3002}\u{533f}\u{540d}\u{961f}\u{53cb}\u{6cbb}\u{7597}\u{4e5f}\u{7a33}\u{ff0c}\u{8d62}\u{5f97}\u{5f88}\u{6e05}\u{695a}\u{3002}"),
        "usedFactIds": ["current_player:kills_rank", "relationship_subject:healing_rank"]
    });
    assert_eq!(
        validate_roast_output(&value.to_string(), &match_context()).unwrap_err(),
        RoastValidationError::UnknownTerm
    );
    let mut terms = allowed_terms();
    terms.insert(UNKNOWN_HERO.into());
    assert!(validate_roast_output(
        &value.to_string(),
        &RoastValidationContext::for_match(&material(), terms)
    )
    .is_ok());
}

#[test]
fn arena_requires_two_fact_ids() {
    let context = RoastValidationContext::for_arena(&arena_material(), allowed_terms());
    let value = json!({
        "award": "\u{89d2}\u{6597}\u{5956}",
        "commentary": "\u{8fd9}\u{573a}\u{89d2}\u{6597}\u{4ee5}4\u{6bd4}3\u{6536}\u{5c3e}\u{ff0c}\u{8282}\u{594f}\u{62c9}\u{6ee1}\u{3002}\u{51b3}\u{80dc}\u{8f6e}\u{6ca1}\u{624b}\u{8f6f}\u{ff0c}\u{7ed3}\u{679c}\u{5f88}\u{5e72}\u{51c0}\u{3002}",
        "usedFactIds": ["match:arena_result"]
    });
    assert_eq!(
        validate_roast_output(&value.to_string(), &context).unwrap_err(),
        RoastValidationError::InsufficientFacts
    );
}
