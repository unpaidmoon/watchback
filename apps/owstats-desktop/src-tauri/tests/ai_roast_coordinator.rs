use anyhow::Result;
use futures_util::task::noop_waker_ref;
use futures_util::FutureExt;
use owstats_desktop_lib::ai_roast::{
    client::{DeepSeekRoastClient, PRODUCTION_TIMEOUT},
    coordinator::{AiRoastCoordinator, AiRoastCoordinatorError, CapturedRoastRequest},
    material::build_roast_material,
    store::{roast_for_match, save_roast_once, AiRoastRecord},
    validator::{validate_roast_output, RoastValidationContext, RoastValidationVocabulary},
};
use owstats_desktop_lib::deepseek_key::{DeepSeekCredentialStore, DeepSeekKeyManager};
use owstats_desktop_lib::OwDatabase;
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::HashSet;
use std::future::{pending, Future};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

struct SyntheticKeyStore {
    key: Mutex<Option<String>>,
    reads: AtomicUsize,
}

impl SyntheticKeyStore {
    fn new(key: Option<&str>) -> Self {
        Self {
            key: Mutex::new(key.map(str::to_owned)),
            reads: AtomicUsize::new(0),
        }
    }
}

impl DeepSeekCredentialStore for SyntheticKeyStore {
    fn read(&self) -> Result<Option<String>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.key.lock().unwrap().clone())
    }

    fn write(&self, api_key: &str) -> Result<()> {
        *self.key.lock().unwrap() = Some(api_key.to_owned());
        Ok(())
    }
}

fn database(name: &str) -> Result<(tempfile::TempDir, OwDatabase)> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join(name));
    db.initialize()?;
    Ok((dir, db))
}

fn key_manager(store: Arc<SyntheticKeyStore>) -> DeepSeekKeyManager {
    DeepSeekKeyManager::with_store(reqwest::Client::new(), "http://127.0.0.1:9", store).unwrap()
}

fn client(base_url: &str) -> DeepSeekRoastClient {
    DeepSeekRoastClient::with_client(reqwest::Client::new(), base_url, PRODUCTION_TIMEOUT).unwrap()
}

fn seed_match(conn: &Connection, match_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,map_name,hero_guid,hero_name,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,23,'preset','SportPreset','normal','win','synthetic-map','\u{5408}\u{6210}\u{5730}\u{56fe}','synthetic-hero','\u{5408}\u{6210}\u{82f1}\u{96c4}',1000,2,1,0,0,0,0,0,0,0,600,'{}','now')",
        [match_id],
    )?;
    Ok(())
}

fn seed_complete_detail(conn: &Connection, match_id: &str) -> Result<()> {
    let detail = json!({
        "match_id": match_id,
        "match_kind": "normal",
        "payload": {"code": 0, "data": {}}
    });
    conn.execute(
        "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES(?1,'normal',?2,'now')",
        params![match_id, detail.to_string()],
    )?;
    seed_evaluation(conn, match_id)?;
    seed_team(conn, match_id)?;
    Ok(())
}

fn seed_evaluation(conn: &Connection, match_id: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO match_evaluations(match_id,mode,rule_version,grade,primary_tag,evidence_tags_json,reasons_json,confidence,computed_at) VALUES(?1,'SportPreset',7,'A','\u{7a33}\u{5065}\u{5c40}',?2,'[]',0.91,'now')",
        params![match_id, json!(["\u{4f24}\u{5bb3}\u{7a33}\u{5b9a}"]).to_string()],
    )?;
    Ok(())
}

fn seed_team(conn: &Connection, match_id: &str) -> Result<()> {
    let teammates = [
        ("tank", true, (8, 5, 4, 7000, 0, 9000)),
        ("dps", false, (12, 3, 4, 10000, 0, 0)),
        ("dps", false, (6, 2, 8, 5000, 0, 0)),
        ("healer", false, (3, 11, 5, 2000, 8000, 0)),
        ("healer", false, (4, 10, 6, 2500, 7000, 0)),
    ];
    for (index, (role, current, stats)) in teammates.into_iter().enumerate() {
        seed_player(conn, match_id, "teammate", index, role, current, stats)?;
    }
    for (index, (role, stats)) in [
        ("dps", (12, 5, 5, 8000, 0, 0)),
        ("dps", (10, 4, 6, 7000, 0, 0)),
        ("healer", (8, 8, 5, 4000, 5000, 0)),
        ("healer", (7, 9, 4, 3000, 6000, 0)),
        ("tank", (6, 3, 7, 4000, 0, 7000)),
    ]
    .into_iter()
    .enumerate()
    {
        seed_player(conn, match_id, "enemy", index, role, false, stats)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn seed_player(
    conn: &Connection,
    match_id: &str,
    side: &str,
    index: usize,
    role: &str,
    current: bool,
    stats: (i64, i64, i64, i64, i64, i64),
) -> Result<()> {
    conn.execute(
        "INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,hero_name,is_current_player,is_friend,kill,assist,death,hero_damage,cure,resist_damage,healing_taken,damage_taken,final_hit,target_competing_time,role_type,raw_json,updated_at) VALUES(?1,23,?2,?3,?4,?5,'\u{5408}\u{6210}\u{82f1}\u{96c4}',?6,0,?7,?8,?9,?10,?11,?12,0,0,0,0,?13,'{}','now')",
        params![
            match_id,
            side,
            format!("synthetic-account-{side}-{index}"),
            format!("\u{5408}\u{6210}\u{73a9}\u{5bb6}{index}"),
            format!("synthetic-hero-{side}-{index}"),
            i64::from(current),
            stats.0,
            stats.1,
            stats.2,
            stats.3,
            stats.4,
            stats.5,
            role,
        ],
    )?;
    Ok(())
}

fn saved_roast(match_id: &str, award: &str, commentary: &str) -> AiRoastRecord {
    AiRoastRecord {
        match_id: match_id.to_string(),
        award: award.to_string(),
        commentary: commentary.to_string(),
        used_fact_ids: vec!["synthetic:fact".to_string()],
        model: "synthetic-model".to_string(),
        evaluation_rule_version: 1,
        material_version: 1,
        prompt_version: 1,
        created_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn valid_completion(conn: &Connection, match_id: &str) -> Result<String> {
    let material = build_roast_material(conn, match_id)?;
    let ids = if let Some(relationship) = material.relationship.as_ref() {
        let relationship_id = relationship.facts[0].id.clone();
        let selected_id = material
            .selected_facts
            .iter()
            .find(|fact| fact.id != relationship_id)
            .unwrap()
            .id
            .clone();
        vec![selected_id, relationship_id]
    } else {
        vec![
            material.selected_facts[0].id.clone(),
            material.selected_facts[1].id.clone(),
        ]
    };
    let content = json!({
        "award": "\u{7a33}\u{5065}\u{62c5}\u{5f53}",
        "commentary": "\u{8fd9}\u{573a}\u{53d1}\u{6325}\u{6709}\u{636e}\u{53ef}\u{67e5}\u{ff0c}\u{8282}\u{594f}\u{7a33}\u{7a33}\u{62ff}\u{4f4f}\u{3002}\u{533f}\u{540d}\u{961f}\u{53cb}\u{4e5f}\u{6709}\u{4eae}\u{70b9}\u{ff0c}\u{6536}\u{5c3e}\u{5f88}\u{5e72}\u{51c0}\u{3002}",
        "usedFactIds": ids
    })
    .to_string();
    validate_roast_output(
        &content,
        &RoastValidationContext::for_match(
            &material,
            RoastValidationVocabulary::new(HashSet::new(), HashSet::new()),
        ),
    )?;
    Ok(json!({
        "choices": [{
            "message": {"content": content},
            "finish_reason": "stop"
        }]
    })
    .to_string())
}

fn loopback_server(
    status: u16,
    body: String,
    expected_request_count: usize,
    before_reply: Option<Box<dyn FnOnce() + Send>>,
) -> (String, LoopbackServer) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (finished_tx, finished_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut before_reply = before_reply;
        let reason = if status == 200 { "OK" } else { "Synthetic" };
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            reason,
            body.len()
            ,body
        );
        for _ in 0..expected_request_count {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            if let Some(action) = before_reply.take() {
                action();
            }
            let _ = stream.write_all(response.as_bytes());
        }
        let _ = finished_tx.send(());
    });
    (
        format!("http://{address}"),
        LoopbackServer {
            address,
            expected_request_count,
            finished: finished_rx,
            handle,
        },
    )
}

struct LoopbackServer {
    address: std::net::SocketAddr,
    expected_request_count: usize,
    finished: mpsc::Receiver<()>,
    handle: thread::JoinHandle<()>,
}

impl LoopbackServer {
    fn join(self) {
        match self.finished.recv_timeout(Duration::from_secs(2)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.handle.join().unwrap();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                for _ in 0..self.expected_request_count {
                    let _ = TcpStream::connect(self.address);
                }
                let _ = self.handle.join();
                panic!("timed out waiting for synthetic server completion");
            }
        }
    }
}

fn read_request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                bytes.extend_from_slice(&buffer[..read]);
                if let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..header_end + 4]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if bytes.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
}

#[tokio::test]
async fn run_rejects_a_second_operation_while_the_first_is_blocked() {
    let coordinator = AiRoastCoordinator::default();
    let first_coordinator = coordinator.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let first = tokio::spawn(async move {
        first_coordinator
            .run(|| async move {
                started_tx.send(()).unwrap();
                release_rx.await.unwrap();
                "first"
            })
            .await
    });

    started_rx.await.unwrap();
    assert_eq!(
        coordinator.run(|| async { "second" }).await,
        Err(AiRoastCoordinatorError::Busy)
    );
    release_tx.send(()).unwrap();
    assert_eq!(first.await.unwrap(), Ok("first"));
}

#[tokio::test]
async fn run_is_immediately_reusable_after_a_normal_error_result() {
    let coordinator = AiRoastCoordinator::default();
    assert_eq!(
        coordinator
            .run(|| async { Err::<(), _>("synthetic error") })
            .await,
        Ok(Err("synthetic error"))
    );
    assert_eq!(coordinator.run(|| async { "reused" }).await, Ok("reused"));
}

#[tokio::test(flavor = "current_thread")]
async fn run_is_immediately_reusable_after_panic_unwind() {
    let coordinator = AiRoastCoordinator::default();
    let panic = AssertUnwindSafe(coordinator.run(|| async {
        panic!("synthetic panic");
    }))
    .catch_unwind()
    .await;

    assert!(panic.is_err());
    assert_eq!(coordinator.run(|| async { "reused" }).await, Ok("reused"));
}

#[test]
fn dropping_a_pending_run_without_a_runtime_releases_the_lock_synchronously() {
    let coordinator = AiRoastCoordinator::default();
    let mut first = Box::pin(coordinator.run(|| async { pending::<()>().await }));
    let mut context = Context::from_waker(noop_waker_ref());
    assert!(matches!(first.as_mut().poll(&mut context), Poll::Pending));

    drop(first);

    let mut second = Box::pin(coordinator.run(|| async { "reused" }));
    assert_eq!(
        second.as_mut().poll(&mut context),
        Poll::Ready(Ok("reused"))
    );
}

#[tokio::test]
async fn existing_roast_short_circuits_without_reading_key_or_http() -> Result<()> {
    let (_dir, db) = database("existing.sqlite3")?;
    let conn = db.connect()?;
    seed_match(&conn, "synthetic-existing")?;
    save_roast_once(
        &conn,
        &saved_roast(
            "synthetic-existing",
            "\u{5df2}\u{5b58}\u{5956}",
            "\u{5df2}\u{5b58}\u{9510}\u{8bc4}",
        ),
    )?;
    drop(conn);
    let store = Arc::new(SyntheticKeyStore::new(None));
    let result = AiRoastCoordinator::default()
        .generate(
            CapturedRoastRequest::new(db, "synthetic-existing".into())
                .map_err(anyhow::Error::msg)?,
            &key_manager(store.clone()),
            &client("http://127.0.0.1:9"),
        )
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(result.ai_roast.award, "\u{5df2}\u{5b58}\u{5956}");
    assert_eq!(store.reads.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn missing_and_incomplete_details_fail_safely() -> Result<()> {
    for (match_id, raw_detail) in [
        ("synthetic-missing-detail", None),
        ("synthetic-incomplete-detail", Some("{}")),
    ] {
        let (_dir, db) = database("detail.sqlite3")?;
        let conn = db.connect()?;
        seed_match(&conn, match_id)?;
        if let Some(raw_detail) = raw_detail {
            conn.execute(
                "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES(?1,'normal',?2,'now')",
                params![match_id, raw_detail],
            )?;
        }
        drop(conn);
        let error = AiRoastCoordinator::default()
            .generate(
                CapturedRoastRequest::new(db, match_id.into()).map_err(anyhow::Error::msg)?,
                &key_manager(Arc::new(SyntheticKeyStore::new(Some("synthetic-key")))),
                &client("http://127.0.0.1:9"),
            )
            .await
            .unwrap_err();
        assert!(error.contains("\u{8be6}\u{60c5}"));
        assert!(!error.contains(match_id));
        assert!(!error.contains("127.0.0.1"));
        assert!(!error.contains("synthetic-key"));
    }
    Ok(())
}

#[tokio::test]
async fn captured_database_receives_generated_roast_after_external_switch() -> Result<()> {
    let (_first_dir, first_db) = database("first.sqlite3")?;
    let (_second_dir, second_db) = database("second.sqlite3")?;
    for db in [&first_db, &second_db] {
        let conn = db.connect()?;
        seed_match(&conn, "synthetic-origin")?;
        seed_complete_detail(&conn, "synthetic-origin")?;
    }
    let body = valid_completion(&first_db.connect()?, "synthetic-origin")?;
    let (base_url, server) = loopback_server(200, body, 1, None);
    let captured = CapturedRoastRequest::new(first_db.clone(), "synthetic-origin".into())
        .map_err(anyhow::Error::msg)?;
    let _switched_database = second_db.clone();
    AiRoastCoordinator::default()
        .generate(
            captured,
            &key_manager(Arc::new(SyntheticKeyStore::new(Some("synthetic-key")))),
            &client(&base_url),
        )
        .await
        .map_err(anyhow::Error::msg)?;
    server.join();
    assert!(roast_for_match(&first_db.connect()?, "synthetic-origin")?.is_some());
    assert!(roast_for_match(&second_db.connect()?, "synthetic-origin")?.is_none());
    Ok(())
}

#[tokio::test]
async fn save_is_first_write_wins_when_result_appears_during_http() -> Result<()> {
    let (_dir, db) = database("first-write.sqlite3")?;
    let conn = db.connect()?;
    seed_match(&conn, "synthetic-race")?;
    seed_complete_detail(&conn, "synthetic-race")?;
    let body = valid_completion(&conn, "synthetic-race")?;
    drop(conn);
    let competing_db = db.clone();
    let (base_url, server) = loopback_server(
        200,
        body,
        1,
        Some(Box::new(move || {
            let conn = competing_db.connect().unwrap();
            save_roast_once(
                &conn,
                &saved_roast(
                    "synthetic-race",
                    "\u{5148}\u{5230}\u{5956}",
                    "\u{5148}\u{5230}\u{9510}\u{8bc4}",
                ),
            )
            .unwrap();
        })),
    );
    let result = AiRoastCoordinator::default()
        .generate(
            CapturedRoastRequest::new(db, "synthetic-race".into()).map_err(anyhow::Error::msg)?,
            &key_manager(Arc::new(SyntheticKeyStore::new(Some("synthetic-key")))),
            &client(&base_url),
        )
        .await
        .map_err(anyhow::Error::msg)?;
    server.join();
    assert_eq!(result.ai_roast.award, "\u{5148}\u{5230}\u{5956}");
    assert_eq!(
        result.ai_roast.commentary,
        "\u{5148}\u{5230}\u{9510}\u{8bc4}"
    );
    Ok(())
}

#[tokio::test]
async fn api_error_mapping_never_returns_request_secrets_or_local_id() -> Result<()> {
    let (_dir, db) = database("safe-error.sqlite3")?;
    let conn = db.connect()?;
    seed_match(&conn, "synthetic-safe-error")?;
    seed_complete_detail(&conn, "synthetic-safe-error")?;
    drop(conn);
    let (base_url, server) = loopback_server(
        500,
        "synthetic-key https://private.invalid synthetic-safe-error".to_string(),
        1,
        None,
    );
    let error = AiRoastCoordinator::default()
        .generate(
            CapturedRoastRequest::new(db, "synthetic-safe-error".into())
                .map_err(anyhow::Error::msg)?,
            &key_manager(Arc::new(SyntheticKeyStore::new(Some("synthetic-key")))),
            &client(&base_url),
        )
        .await
        .unwrap_err();
    server.join();
    for forbidden in [
        "synthetic-key",
        "private.invalid",
        "synthetic-safe-error",
        "authorization",
        "header",
        "body",
    ] {
        assert!(!error.to_ascii_lowercase().contains(forbidden));
    }
    Ok(())
}
