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
        1
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
async fn failed_detail_keeps_job_retryable_and_pending_count() -> Result<()> {
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
    assert_eq!(store.season_status(22)?.retryable, true);
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
