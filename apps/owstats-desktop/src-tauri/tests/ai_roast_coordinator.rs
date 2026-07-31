#![allow(clippy::too_many_arguments, dead_code)]

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use anyhow::Result;
use owstats_desktop_lib::{
    ai_roast::{
        client::{DeepSeekRoastClient, MODEL, PROMPT_VERSION},
        coordinator::{AiRoastCoordinator, AiRoastCoordinatorError, CapturedRoastRequest},
        knowledge::{embedded_knowledge, knowledge_for_match},
        report::{
            AiRoastNarrative, AiRoastReport, ClaimType, NarrativeAnalysis, RoastClaim,
            RoastInsight, RoastVerdict, TeamInsight, TeamInsightKind, REPORT_SCHEMA_VERSION,
        },
        snapshot::{build_roast_snapshot, INPUT_SCHEMA_VERSION},
        store::{roast_for_match, save_roast_once, AiRoastRecord},
    },
    deepseek_key::{DeepSeekCredentialStore, DeepSeekKeyManager},
    get_match_detail, OwDatabase,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::tempdir;

struct KeyStore {
    value: Mutex<Option<String>>,
    reads: AtomicUsize,
}
impl KeyStore {
    fn new(value: Option<&str>) -> Self {
        Self {
            value: Mutex::new(value.map(str::to_string)),
            reads: AtomicUsize::new(0),
        }
    }
}
impl DeepSeekCredentialStore for KeyStore {
    fn read(&self) -> Result<Option<String>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.value.lock().unwrap().clone())
    }
    fn write(&self, key: &str) -> Result<()> {
        *self.value.lock().unwrap() = Some(key.into());
        Ok(())
    }
}
fn keys(store: Arc<KeyStore>) -> DeepSeekKeyManager {
    DeepSeekKeyManager::with_store(reqwest::Client::new(), "http://127.0.0.1:9", store).unwrap()
}
fn client(url: &str) -> DeepSeekRoastClient {
    DeepSeekRoastClient::with_client(reqwest::Client::new(), url, Duration::from_secs(3)).unwrap()
}

fn database(name: &str) -> Result<(tempfile::TempDir, OwDatabase)> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join(name));
    db.initialize()?;
    Ok((dir, db))
}
fn player_json(id: &str, hero: &str, kills: i64, damage: i64, healing: i64) -> Value {
    json!({"bnetId":id,"name":"private-name","heroGuid":hero,"kill":kills,"assist":5,"death":4,"heroDamage":damage,"cure":healing,"resistDamage":0,"healingTaken":1000,"damageTaken":5000,"finalHit":2,"targetCompetingTime":20,"endWorth":7050})
}
fn seed_player(
    conn: &Connection,
    match_id: &str,
    side: &str,
    id: &str,
    hero_guid: &str,
    hero_name: &str,
    role: &str,
    current: bool,
    kills: i64,
    damage: i64,
    healing: i64,
) -> Result<()> {
    conn.execute("INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,hero_name,is_current_player,is_friend,kill,assist,death,hero_damage,cure,resist_damage,healing_taken,damage_taken,final_hit,target_competing_time,role_type,raw_json,updated_at) VALUES(?1,23,?2,?3,'private-name',?4,?5,?6,0,?7,5,4,?8,?9,0,1000,5000,2,20,?10,'{}','now')",params![match_id,side,id,hero_guid,hero_name,i64::from(current),kills,damage,healing,role])?;
    Ok(())
}
fn seed_match(conn: &Connection, match_id: &str, arena: bool) -> Result<()> {
    for (guid, name, role) in [
        ("207165582859043131", "安娜", "healer"),
        ("207165582859042926", "士兵：76", "dps"),
        ("207165582859043328", "艾什", "dps"),
        ("207165582859042820", "天使", "healer"),
    ] {
        conn.execute(
            "INSERT OR REPLACE INTO hero_dict(guid,name,role,updated_at) VALUES(?1,?2,?3,'now')",
            params![guid, name, role],
        )?;
    }
    let kind = if arena { "fight" } else { "normal" };
    conn.execute("INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,map_name,hero_guid,hero_name,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,23,'preset','SportPreset',?2,'loss','map','伊利奥斯','207165582859043131','安娜',1000,1,2,0,0,0,0,0,0,0,600,'{}','now')",params![match_id,kind])?;
    seed_player(
        conn,
        match_id,
        "teammate",
        "current",
        "207165582859043131",
        "安娜",
        "support",
        true,
        7,
        5000,
        12000,
    )?;
    seed_player(
        conn,
        match_id,
        "teammate",
        "soldier",
        "207165582859042926",
        "士兵：76",
        "damage",
        false,
        10,
        10000,
        0,
    )?;
    seed_player(
        conn,
        match_id,
        "enemy",
        "ashe",
        "207165582859043328",
        "艾什",
        "damage",
        false,
        12,
        12000,
        0,
    )?;
    seed_player(
        conn,
        match_id,
        "enemy",
        "mercy",
        "207165582859042820",
        "天使",
        "support",
        false,
        2,
        1000,
        13000,
    )?;
    let detail = if arena {
        let team = vec![
            player_json("current", "207165582859043131", 7, 5000, 12000),
            player_json("soldier", "207165582859042926", 10, 10000, 0),
        ];
        let enemy = vec![
            player_json("ashe", "207165582859043328", 12, 12000, 0),
            player_json("mercy", "207165582859042820", 2, 1000, 13000),
        ];
        json!({"match_id":match_id,"match_kind":"fight","payload":{"code":0,"data":{"totalCount":{"teammateList":team,"enemyList":enemy},"roundCountList":[{"result":"loss","teamScore":0,"opponentScore":1,"teammateList":team,"enemyList":enemy},{"result":"win","teamScore":1,"opponentScore":1,"teammateList":team,"enemyList":enemy},{"result":"loss","teamScore":1,"opponentScore":2,"teammateList":team,"enemyList":enemy}]}}})
    } else {
        json!({"match_id":match_id,"match_kind":"normal","payload":{"code":0,"data":{
            "teammateList":[player_json("current","207165582859043131",7,5000,12000),player_json("soldier","207165582859042926",10,10000,0)],
            "enemyList":[player_json("ashe","207165582859043328",12,12000,0),player_json("mercy","207165582859042820",2,1000,13000)]
        }}})
    };
    conn.execute("INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES(?1,?2,?3,'now')",params![match_id,kind,detail.to_string()])?;
    Ok(())
}

fn insight(title: &str, path: &str) -> RoastInsight {
    RoastInsight {
        title: title.into(),
        commentary: "合成事实分析。".into(),
        claim_type: ClaimType::DirectFact,
        evidence_refs: vec![path.into()],
    }
}
fn report(title: &str) -> AiRoastReport {
    AiRoastReport {
        verdict: RoastVerdict {
            title: title.into(),
            summary: "这场合成对局的关键产出没有接上。".into(),
        },
        decisive_factors: vec![
            insight("续航在线", "我方安娜.stats.healing"),
            insight("敌方火力", "敌方艾什.stats.damage"),
        ],
        current_player: RoastInsight {
            title: "你这局".into(),
            commentary: "治疗事实与目标时间一并核验。".into(),
            claim_type: ClaimType::StatisticalInterpretation,
            evidence_refs: vec![
                "我方安娜.stats.healing".into(),
                "我方安娜.derived.teamObjectiveTimeRank".into(),
            ],
        },
        team_and_matchups: vec![TeamInsight {
            kind: TeamInsightKind::RoleMatchup,
            insight: insight("职责对位", "roleMatchups.damage.deltas.damage"),
        }],
        closing_roast: "药箱准时到岗，扳机集体请假。".into(),
    }
}
fn narrative(title: &str) -> AiRoastNarrative {
    let claim = |judgment: &str, refs: Vec<&str>| RoastClaim {
        judgment: judgment.into(),
        claim_type: ClaimType::StatisticalInterpretation,
        evidence_refs: refs.into_iter().map(str::to_string).collect(),
    };
    AiRoastNarrative {
        analysis: NarrativeAnalysis {
            match_thesis: claim("治疗在线但敌方火力更有效", vec!["我方安娜.stats.healing", "敌方艾什.stats.damage"]),
            current_player: claim("你的治疗是亮点", vec!["我方安娜.stats.healing"]),
            key_actors: Vec::new(),
            supporting_factors: Vec::new(),
        },
        title: title.into(),
        commentary: "你的治疗确实撑住了场面，至少不是站在后排看热闹；但敌方艾什的输出更有存在感，我方火力没把治疗创造的空间换成结果。这局输得像药箱准时到岗，扳机集体请假。".into(),
    }
}
fn completion(report: &AiRoastNarrative) -> String {
    let output = serde_json::to_string(report).unwrap();
    format!(
        "event: response.created\ndata: {}\n\nevent: response.output_text.delta\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
        json!({"type":"response.created","sequence_number":0}),
        json!({"type":"response.output_text.delta","sequence_number":1,"delta":output}),
        json!({"type":"response.completed","sequence_number":2,"response":{"usage":{}}}),
    )
}
fn legacy(match_id: &str) -> AiRoastRecord {
    AiRoastRecord {
        match_id: match_id.into(),
        award: "旧标题".into(),
        commentary: "旧正文".into(),
        report: None,
        used_fact_ids: vec!["old".into()],
        model: "old".into(),
        evaluation_rule_version: 1,
        material_version: 1,
        prompt_version: 1,
        report_schema_version: None,
        input_schema_version: None,
        knowledge_version: None,
        created_at: "now".into(),
    }
}

fn server(status: u16, body: String, count: usize) -> (String, Arc<Mutex<Vec<String>>>, Server) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let capture = requests.clone();
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for _ in 0..count {
            let (mut stream, _) = listener.accept().unwrap();
            capture.lock().unwrap().push(read(&mut stream));
            let response=format!("HTTP/1.1 {status} X\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
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
fn read(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let mut expected = None;
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
                if expected.is_none() {
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let len = headers
                            .lines()
                            .find_map(|line| {
                                line.split_once(':').and_then(|(name, value)| {
                                    name.eq_ignore_ascii_case("content-length")
                                        .then(|| value.trim().parse::<usize>().unwrap())
                                })
                            })
                            .unwrap_or(0);
                        expected = Some(end + 4 + len);
                    }
                }
                if expected.is_some_and(|len| bytes.len() >= len) {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8(bytes).unwrap()
}
fn request_body(request: &str) -> Value {
    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

#[tokio::test]
async fn unified_standard_and_arena_requests_include_complete_context_and_versions() -> Result<()> {
    for arena in [false, true] {
        let (_dir, db) = database(if arena {
            "arena.sqlite3"
        } else {
            "normal.sqlite3"
        })?;
        let conn = db.connect()?;
        seed_match(&conn, "match", arena)?;
        drop(conn);
        let detail = get_match_detail(&db, "match")?;
        let knowledge = embedded_knowledge()?;
        let context = knowledge_for_match(
            &knowledge,
            detail
                .teammates
                .iter()
                .chain(&detail.enemies)
                .map(|row| row.hero_name.clone()),
            detail.map_name.as_deref(),
        );
        let _snapshot = build_roast_snapshot(&db.connect()?, "match", &detail, context)
            .map_err(|error| anyhow::anyhow!("arena={arena}: {error}"))?;
        let generated = narrative(if arena {
            "角斗新标题"
        } else {
            "普通新标题"
        });
        let (url, requests, server) = server(200, completion(&generated), 1);
        let result = AiRoastCoordinator::default()
            .generate(
                CapturedRoastRequest::new(db.clone(), "match".into())
                    .map_err(anyhow::Error::msg)?,
                &keys(Arc::new(KeyStore::new(Some("synthetic-key")))),
                &client(&url),
            )
            .await
            .map_err(anyhow::Error::msg)?;
        server.join();
        assert_eq!(result.ai_roast.award, generated.title);
        assert_eq!(result.ai_roast.report, None);
        let request = request_body(&requests.lock().unwrap()[0]);
        let prompt = request["input"][0]["content"].as_str().unwrap();
        assert!(prompt.contains("scoreboard"));
        assert!(prompt.contains("opponent"));
        assert!(!prompt.contains("selectedFacts"));
        if arena {
            assert!(prompt.contains("roundResults"));
        } else {
            assert!(prompt.contains("\"arena\":null"));
        }
        assert!(prompt.contains("安娜"));
        assert!(!prompt.contains("禅雅塔"));
        let conn = db.connect()?;
        let versions: (i64,i64,i64)=conn.query_row("SELECT report_schema_version,input_schema_version,knowledge_version FROM match_ai_roasts WHERE match_id='match'",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
        assert_eq!(versions, (REPORT_SCHEMA_VERSION, INPUT_SCHEMA_VERSION, 2));
    }
    Ok(())
}

#[tokio::test]
async fn cache_short_circuits_regeneration_replaces_and_failure_preserves_old() -> Result<()> {
    let (_dir, db) = database("cache.sqlite3")?;
    let conn = db.connect()?;
    seed_match(&conn, "match", false)?;
    save_roast_once(&conn, &legacy("match"))?;
    drop(conn);
    let no_key = Arc::new(KeyStore::new(None));
    let cached = AiRoastCoordinator::default()
        .generate(
            CapturedRoastRequest::new(db.clone(), "match".into()).map_err(anyhow::Error::msg)?,
            &keys(no_key.clone()),
            &client("http://127.0.0.1:9"),
        )
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(cached.ai_roast.award, "旧标题");
    assert_eq!(no_key.reads.load(Ordering::SeqCst), 0);
    let newer = narrative("新标题");
    let (url, _, success_server) = server(200, completion(&newer), 1);
    let replaced = AiRoastCoordinator::default()
        .generate(
            CapturedRoastRequest::regenerate(db.clone(), "match".into())
                .map_err(anyhow::Error::msg)?,
            &keys(Arc::new(KeyStore::new(Some("key")))),
            &client(&url),
        )
        .await
        .map_err(anyhow::Error::msg)?;
    success_server.join();
    assert_eq!(replaced.ai_roast.award, newer.title);
    assert_eq!(replaced.ai_roast.report, None);
    let (url, _, server) = server(503, "private body".into(), 1);
    let error = AiRoastCoordinator::default()
        .generate(
            CapturedRoastRequest::regenerate(db.clone(), "match".into())
                .map_err(anyhow::Error::msg)?,
            &keys(Arc::new(KeyStore::new(Some("key")))),
            &client(&url),
        )
        .await
        .unwrap_err();
    server.join();
    assert!(!error.contains("private body"));
    assert_eq!(
        roast_for_match(&db.connect()?, "match")?.unwrap().award,
        "新标题"
    );
    Ok(())
}

#[tokio::test]
async fn application_generation_lock_rejects_concurrent_work() {
    let coordinator = AiRoastCoordinator::default();
    let first = coordinator.clone();
    let (start_tx, start_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        first
            .run(|| async move {
                start_tx.send(()).unwrap();
                release_rx.await.unwrap();
                1
            })
            .await
    });
    start_rx.await.unwrap();
    assert_eq!(
        coordinator.run(|| async { 2 }).await,
        Err(AiRoastCoordinatorError::Busy)
    );
    release_tx.send(()).unwrap();
    assert_eq!(task.await.unwrap(), Ok(1));
}

#[test]
fn record_compatibility_constants_remain_expected() {
    assert_eq!(MODEL, "deepseek-v4-flash");
    assert_eq!(PROMPT_VERSION, 4);
}
