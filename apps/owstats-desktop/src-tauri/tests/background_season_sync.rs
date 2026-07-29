use anyhow::Result;
use owstats_desktop_lib::credential_store::save_credentials_atomic;
use owstats_desktop_lib::dashen_client::DashenCredentials;
use owstats_desktop_lib::player_target::{save_player_target_for_path, PlayerTarget};
use owstats_desktop_lib::season_fetch::FullSeasonRequest;
use owstats_desktop_lib::{
    run_background_job_once, BackgroundSeasonSyncManager, OwDatabase, SeasonSyncStore,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

fn spawn_detail_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 8192];
        let _ = stream.read(&mut buffer).unwrap();
        let body = r#"{"code":0,"data":{"gameTimeSec":600,"teammateList":[],"enemyList":[]}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), body
        );
        stream.write_all(response.as_bytes()).unwrap();
    });
    (format!("http://{address}/customer"), handle)
}

fn spawn_multi_detail_server(
    count: usize,
) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        for _ in 0..count {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8192];
            let read = stream.read(&mut buffer).unwrap();
            recorded.lock().unwrap().push(
                String::from_utf8_lossy(&buffer[..read])
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string(),
            );
            let body = r#"{"code":0,"data":{"gameTimeSec":600,"teammateList":[],"enemyList":[]}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}/customer"), requests, handle)
}

fn spawn_blocked_tail_detail_server(
    count: usize,
    block_after: usize,
) -> (
    String,
    Arc<AtomicUsize>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let server_accepted = Arc::clone(&accepted);
    let server_release = Arc::clone(&release);
    let handle = thread::spawn(move || {
        for index in 0..count {
            let (mut stream, _) = listener.accept().unwrap();
            server_accepted.store(index + 1, Ordering::SeqCst);
            let mut buffer = [0_u8; 8192];
            let _ = stream.read(&mut buffer).unwrap();
            if index >= block_after {
                while !server_release.load(Ordering::SeqCst) {
                    thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            let body = r#"{"code":0,"data":{"gameTimeSec":600,"teammateList":[],"enemyList":[]}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (
        format!("http://{address}/customer"),
        accepted,
        release,
        handle,
    )
}

fn spawn_failing_detail_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 8192];
        let _ = stream.read(&mut buffer).unwrap();
        let body = r#"{"error":"temporary"}"#;
        let response = format!(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(), body
        );
        stream.write_all(response.as_bytes()).unwrap();
    });
    (format!("http://{address}/customer"), handle)
}

fn spawn_pagination_server() -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8192];
            let read = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]);
            let first_line = request.lines().next().unwrap_or_default().to_string();
            recorded.lock().unwrap().push(first_line.clone());
            let body = if first_line.contains("page=2") {
                r#"{"code":0,"data":{"matchList":[{"matchId":"page-two-new","beginTs":1720000300,"gameMode":"sport","matchRet":1}]}}"#
            } else if first_line.contains("page=3") {
                r#"{"code":0,"data":{"matchList":[]}}"#
            } else {
                r#"{"code":0,"data":{"gameTimeSec":500,"teammateList":[],"enemyList":[]}}"#
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}/customer"), requests, handle)
}

fn spawn_interrupted_pagination_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        for response_index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8192];
            let _ = stream.read(&mut buffer).unwrap();
            let (status, body) = if response_index == 0 {
                (
                    "200 OK",
                    r#"{"code":0,"data":{"matchList":[{"matchId":"recovery-page-two","beginTs":1720000400,"gameMode":"sport","matchRet":1}]}}"#,
                )
            } else {
                ("503 Service Unavailable", r#"{"error":"temporary"}"#)
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}/customer"), handle)
}

fn spawn_pagination_recovery_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        for response_index in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 8192];
            let _ = stream.read(&mut buffer).unwrap();
            let body = if response_index == 0 {
                r#"{"code":0,"data":{"matchList":[]}}"#
            } else {
                r#"{"code":0,"data":{"gameTimeSec":500,"teammateList":[],"enemyList":[]}}"#
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}/customer"), handle)
}

fn background_request(base_url: String) -> FullSeasonRequest {
    FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 1,
            token: "synthetic-account-secret".to_string(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "synthetic-player-key".to_string(),
        input_bnet: "匿名账号".to_string(),
        resolved_bnet: None,
        season: 22,
        customer_base_url: base_url,
        pages_per_batch: 5,
        detail_concurrency: 2,
        include_fight: true,
    }
}

fn seed_list_match(db: &OwDatabase) -> Result<()> {
    db.initialize()?;
    db.connect()?.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result,
            map_guid, hero_guid, begin_ts, team_score, opponent_score,
            kills, assists, deaths, damage, healing, mitigation,
            total_worth, duration_sec, raw_entry_json, updated_at
        ) VALUES (
            'background-new', 22, 'preset', 'sport', 'normal', 'win',
            '', '', 1720000200, 1, 0,
            0, 0, 0, 0, 0, 0,
            0, 0, '{"matchId":"background-new","gameMode":"sport","beginTs":1720000200}',
            '2026-07-17T00:00:00Z'
        )
        "#,
        [],
    )?;
    Ok(())
}

fn seed_named_list_match(db: &OwDatabase, match_id: &str, begin_ts: i64) -> Result<()> {
    db.initialize()?;
    db.connect()?.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result,
            map_guid, hero_guid, begin_ts, team_score, opponent_score,
            kills, assists, deaths, damage, healing, mitigation,
            total_worth, duration_sec, raw_entry_json, updated_at
        ) VALUES (?1, 22, 'preset', 'sport', 'normal', 'win',
            '', '', ?2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ?3, '2026-07-17T00:00:00Z')
        "#,
        rusqlite::params![
            match_id,
            begin_ts,
            serde_json::json!({"matchId": match_id, "gameMode": "sport", "beginTs": begin_ts})
                .to_string()
        ],
    )?;
    Ok(())
}

#[tokio::test]
async fn background_job_fetches_missing_detail_and_rebuilds_snapshot() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("background.sqlite3"));
    seed_list_match(&db)?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 1, ?1)",
        ["2026-07-17T00:00:00Z"],
    )?;
    store.enqueue_or_merge(22, 1, &[], &["background-new".to_string()])?;
    let (base_url, server) = spawn_detail_server();
    let request = FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 1,
            token: "synthetic-account-secret".to_string(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "synthetic-player-key".to_string(),
        input_bnet: "匿名账号".to_string(),
        resolved_bnet: None,
        season: 22,
        customer_base_url: base_url,
        pages_per_batch: 5,
        detail_concurrency: 2,
        include_fight: true,
    };

    let summary = run_background_job_once(&store, &request, &temp.path().join("seasons")).await?;
    server.join().unwrap();

    assert_eq!(summary.details_written, 1);
    assert_eq!(summary.pages_scanned, 0);
    let conn = db.connect()?;
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM match_details", [], |row| row
            .get::<_, i64>(0))?,
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT snapshot_revision FROM season_sync_state WHERE season=22",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        2
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM season_sync_jobs WHERE season=22 AND status='completed'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    assert!(temp
        .path()
        .join("seasons/local_s22_full_season.json")
        .is_file());
    Ok(())
}

#[tokio::test]
async fn failed_detail_preserves_original_error_and_keeps_job_retryable() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("retryable.sqlite3"));
    seed_list_match(&db)?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 1, ?1)",
        ["2026-07-17T00:00:00Z"],
    )?;
    store.enqueue_or_merge(22, 1, &[], &["background-new".to_string()])?;
    let (base_url, server) = spawn_failing_detail_server();
    let request = FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 1,
            token: "synthetic-account-secret".to_string(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "synthetic-player-key".to_string(),
        input_bnet: "匿名账号".to_string(),
        resolved_bnet: None,
        season: 22,
        customer_base_url: base_url,
        pages_per_batch: 5,
        detail_concurrency: 2,
        include_fight: true,
    };

    let summary = run_background_job_once(&store, &request, &temp.path().join("seasons")).await?;
    server.join().unwrap();

    assert_eq!(summary.failed_details, 1);
    let conn = db.connect()?;
    assert_eq!(
        conn.query_row(
            "SELECT status FROM season_sync_jobs WHERE season=22",
            [],
            |row| row.get::<_, String>(0)
        )?,
        "retryable"
    );
    assert_eq!(
        conn.query_row(
            "SELECT pending_detail_count FROM season_sync_state WHERE season=22",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    assert!(store.season_status(22)?.retryable);
    let message: String = conn.query_row(
        "SELECT message FROM sync_errors WHERE season=22 AND match_id='background-new'",
        [],
        |row| row.get(0),
    )?;
    assert!(message.contains("HTTP status server error (503 Service Unavailable)"));
    assert!(message.contains("token=synthetic-player-key"));
    Ok(())
}

#[tokio::test]
async fn detail_worker_fetches_only_job_matches_missing_valid_details() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("detail-selection.sqlite3"));
    for (match_id, begin_ts) in [
        ("missing-detail", 1_720_000_300),
        ("invalid-detail", 1_720_000_200),
        ("already-complete", 1_720_000_100),
    ] {
        seed_named_list_match(&db, match_id, begin_ts)?;
    }
    let conn = db.connect()?;
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES ('invalid-detail', 'normal', '{}', 'now')",
        [],
    )?;
    conn.execute(
        r#"INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
           VALUES ('already-complete', 'normal',
           '{"match_id":"already-complete","match_kind":"normal","payload":{"code":0,"data":{}}}', 'now')"#,
        [],
    )?;
    conn.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 1, 'now')",
        [],
    )?;
    drop(conn);
    let store = SeasonSyncStore::new(db.clone());
    store.enqueue_or_merge(
        22,
        1,
        &[],
        &[
            "missing-detail".to_string(),
            "invalid-detail".to_string(),
            "already-complete".to_string(),
        ],
    )?;
    let (base_url, requests, server) = spawn_multi_detail_server(2);

    let summary = run_background_job_once(
        &store,
        &background_request(base_url),
        &temp.path().join("seasons"),
    )
    .await?;
    server.join().unwrap();

    assert_eq!(summary.details_written, 2);
    let requests = requests.lock().unwrap().join("\n");
    assert!(requests.contains("missing-detail"));
    assert!(requests.contains("invalid-detail"));
    assert!(!requests.contains("already-complete"));
    assert_eq!(
        db.connect()?.query_row(
            "SELECT COUNT(*) FROM season_sync_job_matches WHERE detail_status='completed'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        3
    );
    Ok(())
}

#[tokio::test]
async fn detail_worker_persists_each_ten_item_batch_before_fetching_tail() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("detail-batches.sqlite3"));
    let mut ids = Vec::new();
    for index in 0..12 {
        let match_id = format!("batch-{index:02}");
        seed_named_list_match(&db, &match_id, 1_720_001_000 + index)?;
        ids.push(match_id);
    }
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 1, 'now')",
        [],
    )?;
    let store = SeasonSyncStore::new(db.clone());
    store.enqueue_or_merge(22, 1, &[], &ids)?;
    let (base_url, accepted, release, server) = spawn_blocked_tail_detail_server(12, 10);
    let worker_store = store.clone();
    let output_dir = temp.path().join("seasons");
    let worker = tokio::spawn(async move {
        run_background_job_once(&worker_store, &background_request(base_url), &output_dir).await
    });

    for _ in 0..200 {
        if accepted.load(Ordering::SeqCst) >= 11 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(accepted.load(Ordering::SeqCst), 11);
    assert_eq!(
        db.connect()?
            .query_row("SELECT COUNT(*) FROM match_details", [], |row| {
                row.get::<_, i64>(0)
            })?,
        10
    );

    release.store(true, Ordering::SeqCst);
    let summary = worker.await??;
    server.join().unwrap();
    assert_eq!(summary.details_written, 12);
    assert_eq!(
        db.connect()?
            .query_row("SELECT COUNT(*) FROM match_details", [], |row| {
                row.get::<_, i64>(0)
            })?,
        12
    );
    Ok(())
}

#[tokio::test]
async fn background_job_continues_pages_and_commits_each_new_list() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("pages.sqlite3"));
    seed_list_match(&db)?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 0, ?1)",
        ["2026-07-17T00:00:00Z"],
    )?;
    db.connect()?.execute(
        r#"
        INSERT INTO season_sync_cursors (
            season, match_kind, game_mode, request_shape, latest_begin_ts,
            empty_confirmed, last_stop_reason
        ) VALUES (22, 'normal', 'sport', 'explicitSeason', 1720000200, 0, 'continue')
        "#,
        [],
    )?;
    store.enqueue_or_merge(
        22,
        0,
        &[owstats_desktop_lib::ModeWork {
            match_kind: "normal".to_string(),
            game_mode: "sport".to_string(),
            request_shape: owstats_desktop_lib::RequestShape::ExplicitSeason,
            next_page: 2,
            max_page: 5,
        }],
        &[],
    )?;
    let (base_url, requests, server) = spawn_pagination_server();
    let request = FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 1,
            token: "synthetic-account-secret".to_string(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "synthetic-player-key".to_string(),
        input_bnet: "匿名账号".to_string(),
        resolved_bnet: None,
        season: 22,
        customer_base_url: base_url,
        pages_per_batch: 5,
        detail_concurrency: 2,
        include_fight: true,
    };

    let summary = run_background_job_once(&store, &request, &temp.path().join("seasons")).await?;
    server.join().unwrap();

    assert_eq!(summary.pages_scanned, 2);
    assert_eq!(summary.new_list_matches, 1);
    assert_eq!(summary.details_written, 1);
    let recorded = requests.lock().unwrap();
    assert!(recorded.iter().any(|line| line.contains("page=2")));
    assert!(recorded.iter().any(|line| line.contains("page=3")));
    assert!(!recorded.iter().any(|line| line.contains("page=4")));
    assert_eq!(
        db.connect()?.query_row(
            "SELECT COUNT(*) FROM matches WHERE match_id='page-two-new'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn manager_deduplicates_wakes_for_the_same_account_and_season() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let shared = owstats_desktop_lib::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = owstats_desktop_lib::account_store::AccountId::parse(
        "11111111-1111-4111-8111-111111111111",
    )?;
    let paths = shared.account(&account_id);
    std::fs::create_dir_all(paths.root())?;
    save_credentials_atomic(
        &paths.credentials(),
        &owstats_desktop_lib::credential_store::DashenCredentials {
            role_id: 1,
            token: "synthetic-account-secret".to_string(),
            customer_token: None,
            dts: Some("2026".to_string()),
            server: 1,
            saved_at: "2026-07-17T00:00:00Z".to_string(),
        },
    )?;
    save_player_target_for_path(
        &paths.player_target(),
        &PlayerTarget {
            input_bnet: "匿名账号".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-17T00:00:00Z".to_string(),
            source_role_id: Some(1),
        },
    )?;
    let db = OwDatabase::for_account(&paths);
    seed_list_match(&db)?;
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 1, ?1)",
        ["2026-07-17T00:00:00Z"],
    )?;
    SeasonSyncStore::new(db.clone()).enqueue_or_merge(
        22,
        1,
        &[],
        &["background-new".to_string()],
    )?;
    let (base_url, server) = spawn_detail_server();
    let manager = BackgroundSeasonSyncManager::default();

    assert!(manager.wake_headless(account_id.as_str(), paths.clone(), 22, base_url.clone()));
    assert!(!manager.wake_headless(account_id.as_str(), paths.clone(), 22, base_url));
    manager.wait_for_idle(account_id.as_str(), 22).await;
    server.join().unwrap();

    assert_eq!(manager.active_count(), 0);
    assert_eq!(
        db.connect()?.query_row(
            "SELECT COUNT(*) FROM season_sync_jobs WHERE status='completed'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    Ok(())
}

#[test]
fn manager_wake_can_start_outside_a_tokio_runtime() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let shared = owstats_desktop_lib::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = owstats_desktop_lib::account_store::AccountId::parse(
        "22222222-2222-4222-8222-222222222222",
    )?;
    let paths = shared.account(&account_id);
    std::fs::create_dir_all(paths.root())?;
    let manager = BackgroundSeasonSyncManager::default();

    assert!(manager.wake_headless(
        account_id.as_str(),
        paths,
        22,
        "http://127.0.0.1:9/customer".to_string(),
    ));
    tauri::async_runtime::block_on(manager.wait_for_idle(account_id.as_str(), 22));

    assert_eq!(manager.active_count(), 0);
    Ok(())
}

#[tokio::test]
async fn recovery_pagination_resumes_at_first_uncommitted_page() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("recovery-pages.sqlite3"));
    seed_list_match(&db)?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 0, 'now')",
        [],
    )?;
    db.connect()?.execute(
        "INSERT INTO season_sync_cursors (season, match_kind, game_mode, request_shape, latest_begin_ts, last_stop_reason) VALUES (22, 'normal', 'sport', 'explicitSeason', 1720000200, 'continue')",
        [],
    )?;
    store.enqueue_or_merge(
        22,
        0,
        &[owstats_desktop_lib::ModeWork {
            match_kind: "normal".to_string(),
            game_mode: "sport".to_string(),
            request_shape: owstats_desktop_lib::RequestShape::ExplicitSeason,
            next_page: 2,
            max_page: 5,
        }],
        &[],
    )?;
    let (first_url, first_server) = spawn_interrupted_pagination_server();

    assert!(run_background_job_once(
        &store,
        &background_request(first_url),
        &temp.path().join("seasons")
    )
    .await
    .is_err());
    first_server.join().unwrap();
    let conn = db.connect()?;
    assert_eq!(
        conn.query_row(
            "SELECT next_page FROM season_sync_job_modes WHERE season_sync_job_modes.job_id=(SELECT job_id FROM season_sync_jobs WHERE season=22)",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        3
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM matches WHERE match_id='recovery-page-two'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    drop(conn);

    let (second_url, second_server) = spawn_pagination_recovery_server();
    let recovered = run_background_job_once(
        &store,
        &background_request(second_url),
        &temp.path().join("seasons"),
    )
    .await?;
    second_server.join().unwrap();

    assert_eq!(recovered.pages_scanned, 1);
    assert_eq!(recovered.details_written, 1);
    assert_eq!(store.active_job_count(22)?, 0);
    assert_eq!(
        db.connect()?.query_row(
            "SELECT COUNT(*) FROM matches WHERE match_id='recovery-page-two'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    Ok(())
}

#[tokio::test]
async fn recovery_detail_retries_only_missing_detail() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("recovery-detail.sqlite3"));
    seed_list_match(&db)?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, updated_at) VALUES (22, 1, 'now')",
        [],
    )?;
    store.enqueue_or_merge(22, 1, &[], &["background-new".to_string()])?;
    let (failed_url, failed_server) = spawn_failing_detail_server();
    let failed = run_background_job_once(
        &store,
        &background_request(failed_url),
        &temp.path().join("seasons"),
    )
    .await?;
    failed_server.join().unwrap();
    assert_eq!(failed.failed_details, 1);

    let (success_url, success_server) = spawn_detail_server();
    let recovered = run_background_job_once(
        &store,
        &background_request(success_url),
        &temp.path().join("seasons"),
    )
    .await?;
    success_server.join().unwrap();

    assert_eq!(recovered.pages_scanned, 0);
    assert_eq!(recovered.details_written, 1);
    assert_eq!(store.active_job_count(22)?, 0);
    assert_eq!(
        db.connect()?.query_row(
            "SELECT COUNT(*) FROM match_details WHERE match_id='background-new'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&std::fs::read(
        temp.path().join("seasons/local_s22_full_season.json"),
    )?)?;
    assert!(snapshot["matches"]["details"]
        .as_array()
        .is_some_and(|details| details
            .iter()
            .any(|detail| detail["match_id"] == "background-new")));
    assert!(db.connect()?.query_row(
        "SELECT season_revision=snapshot_revision FROM season_sync_state WHERE season=22",
        [],
        |row| row.get::<_, bool>(0)
    )?);
    Ok(())
}

#[tokio::test]
async fn recovery_snapshot_phase_rebuilds_without_network_or_duplicate_rows() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("recovery-snapshot.sqlite3"));
    seed_list_match(&db)?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, snapshot_revision, updated_at) VALUES (22, 1, 0, 'now')",
        [],
    )?;
    let job_id = store.enqueue_or_merge(22, 1, &[], &[])?;
    db.connect()?.execute(
        "UPDATE season_sync_jobs SET status='running', phase='rebuildSnapshot' WHERE job_id=?1",
        [&job_id],
    )?;

    assert_eq!(store.requeue_orphaned_running()?, 1);
    let summary = run_background_job_once(
        &store,
        &background_request("http://127.0.0.1:9/customer".to_string()),
        &temp.path().join("seasons"),
    )
    .await?;

    assert_eq!(summary.pages_scanned, 0);
    assert_eq!(summary.details_written, 0);
    assert!(summary.snapshot_rebuilt);
    assert_eq!(store.active_job_count(22)?, 0);
    assert_eq!(
        db.connect()?
            .query_row("SELECT COUNT(*) FROM matches", [], |row| row
                .get::<_, i64>(0))?,
        1
    );
    Ok(())
}

#[test]
fn full_fetch_supersedes_stale_jobs() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("full-supersede.sqlite3"));
    db.initialize()?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, snapshot_revision, updated_at) VALUES (22, 4, 2, 'now')",
        [],
    )?;
    store.enqueue_or_merge(22, 4, &[], &[])?;
    db.connect()?.execute(
        "UPDATE season_sync_jobs SET status='retryable' WHERE season=22",
        [],
    )?;

    store.reconcile_full_fetch(22)?;

    let conn = db.connect()?;
    assert_eq!(
        conn.query_row(
            "SELECT status FROM season_sync_jobs WHERE season=22",
            [],
            |row| row.get::<_, String>(0)
        )?,
        "superseded"
    );
    assert!(conn.query_row(
        "SELECT season_revision=snapshot_revision FROM season_sync_state WHERE season=22",
        [],
        |row| row.get::<_, bool>(0)
    )?);
    Ok(())
}
