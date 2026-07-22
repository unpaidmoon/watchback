use anyhow::Result;
use owstats_desktop_lib::season_fetch::{QuickListModeProbe, QuickListProbe};
use owstats_desktop_lib::{
    commit_quick_list_probe, ModeWork, OwDatabase, QuickListCommitSummary, RequestShape,
    SeasonSyncResult, SeasonSyncStore,
};
use rusqlite::{params, Connection};
use std::time::{Duration, Instant};

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )?)
}

fn insert_match(
    conn: &Connection,
    match_id: &str,
    season: i64,
    match_kind: &str,
    game_mode: &str,
    begin_ts: i64,
) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result,
            map_guid, hero_guid, begin_ts, team_score, opponent_score,
            kills, assists, deaths, damage, healing, mitigation,
            total_worth, duration_sec, raw_entry_json, updated_at
        ) VALUES (
            ?1, ?2, 'normal', ?3, ?4, 'win',
            'map', 'hero', ?5, 1, 0,
            0, 0, 0, 0, 0, 0,
            0, 600, '{}', '2026-07-17T00:00:00Z'
        )
        "#,
        params![match_id, season, game_mode, match_kind, begin_ts],
    )?;
    Ok(())
}

#[test]
fn upgrades_v8_with_idempotent_season_sync_state() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("upgrade.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    insert_match(&conn, "s22-sport-old", 22, "normal", "sport", 1_720_000_000)?;
    insert_match(
        &conn,
        "s22-sport-new",
        22,
        "normal",
        "SportPreset",
        1_720_000_100,
    )?;
    insert_match(&conn, "s23-fight", 23, "fight", "QuickFight", 1_730_000_000)?;
    conn.pragma_update(None, "user_version", 8)?;
    drop(conn);

    db.initialize()?;
    db.initialize()?;

    let conn = db.connect()?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
    assert_eq!(version, 12);
    for table in [
        "season_sync_state",
        "season_sync_cursors",
        "season_sync_jobs",
        "season_sync_job_modes",
        "season_sync_job_matches",
    ] {
        assert!(table_exists(&conn, table)?, "missing {table}");
    }

    let (latest, shape): (Option<i64>, String) = conn.query_row(
        r#"
        SELECT latest_begin_ts, request_shape
        FROM season_sync_cursors
        WHERE season=22 AND match_kind='normal' AND game_mode='sport'
        "#,
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(latest, Some(1_720_000_100));
    assert_eq!(shape, "explicitSeason");

    let current_shape: String = conn.query_row(
        r#"
        SELECT request_shape
        FROM season_sync_cursors
        WHERE season=23 AND match_kind='fight' AND game_mode='QuickFight'
        "#,
        [],
        |row| row.get(0),
    )?;
    assert_eq!(current_shape, "implicitCurrentSeason");

    let absent_mode_rows: i64 = conn.query_row(
        r#"
        SELECT COUNT(*)
        FROM season_sync_cursors
        WHERE season=22 AND match_kind='fight' AND game_mode='SportFight'
        "#,
        [],
        |row| row.get(0),
    )?;
    assert_eq!(absent_mode_rows, 0);
    Ok(())
}

#[test]
fn season_job_store_claims_once_merges_and_recovers_orphans() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("jobs.sqlite3"));
    db.initialize()?;
    let store = SeasonSyncStore::new(db.clone());
    let work = ModeWork {
        match_kind: "normal".to_string(),
        game_mode: "sport".to_string(),
        request_shape: RequestShape::ExplicitSeason,
        next_page: 2,
        max_page: 5,
    };

    let first_id = store.enqueue_or_merge(22, 4, &[work.clone()], &[])?;
    let merged_id = store.enqueue_or_merge(22, 6, &[work], &[])?;
    assert_eq!(first_id, merged_id);
    assert_eq!(store.active_job_count(22)?, 1);

    let claimed = store.claim_next(22)?.expect("queued job");
    assert_eq!(claimed.job_id, first_id);
    assert_eq!(claimed.target_revision, 6);
    assert!(store.claim_next(22)?.is_none());

    assert_eq!(store.requeue_orphaned_running()?, 1);
    let recovered = store.claim_next(22)?.expect("recovered job");
    assert_eq!(recovered.job_id, first_id);
    assert!(store.claim_next(22)?.is_none());
    Ok(())
}

fn quick_mode(match_kind: &str, game_mode: &str, payload: serde_json::Value) -> QuickListModeProbe {
    QuickListModeProbe {
        match_kind: match_kind.to_string(),
        game_mode: game_mode.to_string(),
        request_shape: RequestShape::ExplicitSeason,
        payload: Some(payload),
        error_class: None,
    }
}

#[test]
fn foreground_commit_inserts_new_lists_without_details_or_snapshot_work() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("foreground.sqlite3"));
    let probe = QuickListProbe {
        season: 22,
        list_duration_ms: 12,
        modes: vec![
            quick_mode(
                "normal",
                "sport",
                serde_json::json!({"code": 0, "data": {"matchList": [
                    {"matchId":"new-a","beginTs":1720000200,"gameMode":"sport","matchRet":1},
                    {"matchId":"new-b","beginTs":1720000100,"gameMode":"sport","matchRet":-1}
                ]}}),
            ),
            quick_mode(
                "normal",
                "leisure",
                serde_json::json!({"code":0,"data":{"matchList":[]}}),
            ),
            quick_mode(
                "fight",
                "QuickFight",
                serde_json::json!({"code":0,"data":{"matchList":[]}}),
            ),
            quick_mode(
                "fight",
                "LeisureFight",
                serde_json::json!({"code":0,"data":{"matchList":[]}}),
            ),
            quick_mode(
                "fight",
                "SportFight",
                serde_json::json!({"code":0,"data":{"matchList":[]}}),
            ),
        ],
    };

    let summary = commit_quick_list_probe(&db, probe)?;

    assert_eq!(summary.new_list_matches, 2);
    assert!(summary.data_changed);
    assert!(!summary.partial_failure);
    assert_eq!(summary.pending_details, 2);
    let conn = db.connect()?;
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM matches WHERE season=22", [], |row| {
            row.get::<_, i64>(0)
        })?,
        2
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM match_details", [], |row| row
            .get::<_, i64>(0))?,
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT season_revision FROM season_sync_state WHERE season=22",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    assert_eq!(store_active_jobs(&conn, 22)?, 1);
    Ok(())
}

#[test]
fn foreground_local_commit_stays_below_250_ms_on_s22_scale_fixture() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("foreground-performance.sqlite3"));
    db.initialize()?;
    let mut conn = db.connect()?;
    let tx = conn.transaction()?;
    for index in 0..1_000 {
        insert_match(
            &tx,
            &format!("known-{index}"),
            22,
            "normal",
            "sport",
            1_720_000_000 + index,
        )?;
    }
    tx.commit()?;
    let new_entries = (0..20)
        .map(|index| {
            serde_json::json!({
                "matchId": format!("new-{index}"),
                "beginTs": 1_730_000_000 + index,
                "gameMode": "sport",
                "matchRet": 1
            })
        })
        .collect::<Vec<_>>();
    let probe = QuickListProbe {
        season: 22,
        list_duration_ms: 0,
        modes: vec![quick_mode(
            "normal",
            "sport",
            serde_json::json!({"code":0,"data":{"matchList":new_entries}}),
        )],
    };

    let started = Instant::now();
    let summary = commit_quick_list_probe(&db, probe)?;
    let elapsed = started.elapsed();

    assert_eq!(summary.new_list_matches, 20);
    assert!(
        elapsed <= Duration::from_millis(250),
        "anonymous foreground SQLite commit took {elapsed:?}"
    );
    Ok(())
}

#[test]
fn foreground_partial_failure_preserves_cursor_and_queues_only_failed_mode() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("partial-failure.sqlite3"));
    db.initialize()?;
    db.connect()?.execute(
        "INSERT INTO season_sync_cursors (season, match_kind, game_mode, request_shape, latest_begin_ts, last_checked_at, last_stop_reason) VALUES (22, 'normal', 'sport', 'explicitSeason', 1720000200, 'old-check', 'watermarkReached')",
        [],
    )?;
    let probe = QuickListProbe {
        season: 22,
        list_duration_ms: 0,
        modes: vec![QuickListModeProbe {
            match_kind: "normal".to_string(),
            game_mode: "sport".to_string(),
            request_shape: RequestShape::ExplicitSeason,
            payload: None,
            error_class: Some("network".to_string()),
        }],
    };

    let summary = commit_quick_list_probe(&db, probe)?;

    assert!(summary.partial_failure);
    assert!(!summary.data_changed);
    let conn = db.connect()?;
    assert_eq!(
        conn.query_row(
            "SELECT latest_begin_ts FROM season_sync_cursors WHERE season=22 AND match_kind='normal' AND game_mode='sport'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1_720_000_200
    );
    assert_eq!(
        conn.query_row("SELECT next_page FROM season_sync_job_modes", [], |row| row
            .get::<_, i64>(0))?,
        1
    );
    Ok(())
}

fn store_active_jobs(conn: &Connection, season: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM season_sync_jobs WHERE season=?1 AND status IN ('queued','running','retryable')",
        [season],
        |row| row.get(0),
    )?)
}

#[test]
fn season_sync_result_serializes_as_a_flat_discriminated_union() -> Result<()> {
    let value = serde_json::to_value(SeasonSyncResult::QuickList(QuickListCommitSummary {
        season: 22,
        new_list_matches: 0,
        data_changed: false,
        partial_failure: false,
        pending_details: 0,
        background_status: "idle".to_string(),
        checked_at: "2026-07-17T00:00:00Z".to_string(),
        modes: vec![],
    }))?;
    assert_eq!(value["kind"], "quickList");
    assert_eq!(value["season"], 22);
    assert!(value.get("QuickList").is_none());
    Ok(())
}

#[test]
fn retry_season_requeues_only_retryable_job() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("retry.sqlite3"));
    db.initialize()?;
    let store = SeasonSyncStore::new(db.clone());
    let id = store.enqueue_or_merge(22, 0, &[], &[])?;
    db.connect()?.execute(
        "UPDATE season_sync_jobs SET status='retryable', error_class='network' WHERE job_id=?1",
        [&id],
    )?;

    assert!(store.retry_season(22)?);
    assert_eq!(
        db.connect()?.query_row(
            "SELECT status FROM season_sync_jobs WHERE job_id=?1",
            [&id],
            |row| row.get::<_, String>(0)
        )?,
        "queued"
    );
    assert!(!store.retry_season(23)?);
    Ok(())
}

#[test]
fn full_fetch_reconciliation_supersedes_stale_jobs_and_aligns_revisions() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("full-reconcile.sqlite3"));
    db.initialize()?;
    let store = SeasonSyncStore::new(db.clone());
    db.connect()?.execute(
        "INSERT INTO season_sync_state (season, season_revision, snapshot_revision, updated_at) VALUES (22, 4, 2, ?1)",
        ["2026-07-17T00:00:00Z"],
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
    let revisions = conn.query_row(
        "SELECT season_revision, snapshot_revision, background_status, pending_detail_count FROM season_sync_state WHERE season=22",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?)),
    )?;
    assert_eq!(revisions, (5, 5, "completed".to_string(), 0));
    Ok(())
}
