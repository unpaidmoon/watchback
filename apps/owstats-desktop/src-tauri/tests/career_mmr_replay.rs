use owstats_desktop_lib::{
    career_mmr::replay::rebuild_career_mmr, import_payload_value_embedded, OwDatabase,
};
use rusqlite::{params, Connection};
use tempfile::tempdir;

fn database() -> (tempfile::TempDir, OwDatabase) {
    let dir = tempdir().unwrap();
    let db = OwDatabase::new(dir.path().join("replay.sqlite3"));
    db.initialize().unwrap();
    (dir, db)
}

fn insert_match(conn: &Connection, id: &str, season: i64, ts: i64, role: &str, result: &str) {
    conn.execute(
        "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, ?2, 'preset', 'SportPreset', 'normal', ?3, 'map', ?4, ?5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, ?6, 'now')",
        params![id, season, result, format!("hero-{role}"), ts, serde_json::json!({"roleType":role}).to_string()],
    ).unwrap();
}

fn insert_mode_match(
    conn: &Connection,
    id: &str,
    season: i64,
    ts: i64,
    role: &str,
    result: &str,
    game_mode: &str,
) {
    insert_match(conn, id, season, ts, role, result);
    conn.execute(
        "UPDATE matches SET game_mode=?1 WHERE match_id=?2",
        params![game_mode, id],
    )
    .unwrap();
}

fn insert_stadium_match(conn: &Connection, id: &str, season: i64, ts: i64) {
    insert_mode_match(conn, id, season, ts, "tank", "win", "SportFight");
    conn.execute(
        "UPDATE matches SET bucket='stadium', match_kind='arena', duration_sec=900 WHERE match_id=?1",
        [id],
    )
    .unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO hero_dict (guid,name,role,updated_at) VALUES ('stadium-tank','重装','tank','now'),('stadium-support','支援','healer','now')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_players (match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,raw_json,updated_at) VALUES (?1,?2,'teammate','synthetic-current','匿名玩家','stadium-support',1,'{}','now')",
        params![id, season],
    )
    .unwrap();
    let detail = serde_json::json!({"payload":{"data":{"roundCountList":[
        {"roundIndex":1,"teammateList":[{"bnetId":"synthetic-current","heroGuid":"stadium-tank","finalHit":99}],"enemyList":[]},
        {"roundIndex":2,"teammateList":[{"bnetId":"synthetic-current","heroGuid":"stadium-support","finalHit":3}],"enemyList":[]}
    ]}}});
    conn.execute(
        "INSERT INTO match_details (match_id,match_kind,raw_detail_json,updated_at) VALUES (?1,'arena',?2,'now')",
        params![id, detail.to_string()],
    )
    .unwrap();
}

fn digest(db: &OwDatabase) -> String {
    rebuild_career_mmr(db).unwrap().event_digest
}

fn persisted_mmr_rows(conn: &Connection) -> Vec<String> {
    [
        "career_mmr_events",
        "career_role_mmr",
        "career_role_mode_summary",
        "career_role_season_summary",
    ]
    .into_iter()
    .map(|table| {
        let columns = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let expression = columns
            .iter()
            .map(|column| format!("quote(\"{column}\")"))
            .collect::<Vec<_>>()
            .join(" || '|' || ");
        conn.prepare(&format!("SELECT {expression} FROM {table} ORDER BY rowid"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
            .join("\n")
    })
    .collect()
}

#[test]
fn dynamically_discovers_seasons_and_orders_by_time_then_season_then_stable_hash() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "later-s15", 15, 300, "tank", "win");
    insert_match(&conn, "same-s23", 23, 100, "damage", "win");
    insert_match(&conn, "same-s20", 20, 100, "support", "loss");
    drop(conn);

    let summary = rebuild_career_mmr(&db).unwrap();
    assert_eq!(summary.discovered_seasons, vec![15, 20, 23]);
    let conn = db.connect().unwrap();
    let rows = conn
        .prepare(
            "SELECT begin_ts, season FROM career_mmr_events WHERE model_version=1 ORDER BY rowid",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(rows, vec![(100, 20), (100, 23), (300, 15)]);
    let hashes = conn
        .prepare("SELECT input_hash FROM mmr_match_inputs ORDER BY input_hash")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(hashes.iter().all(|hash| hash.len() == 64));
    assert_eq!(
        hashes
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    assert_eq!(conn.query_row("SELECT matches_count FROM career_role_mode_summary WHERE role='tank' AND mode_family='role_competitive'", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
    assert_eq!(
        conn.query_row(
            "SELECT matches_count FROM career_role_season_summary WHERE season=15 AND role='tank'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn replay_is_idempotent_and_older_backfill_changes_later_results() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "newer", 23, 200, "tank", "win");
    drop(conn);
    let first = digest(&db);
    assert_eq!(first, digest(&db));
    let conn = db.connect().unwrap();
    insert_match(&conn, "older", 15, 100, "tank", "loss");
    drop(conn);
    assert_ne!(first, digest(&db));
    let conn = db.connect().unwrap();
    let mu_before: f64 = conn
        .query_row(
            "SELECT mu_before FROM career_mmr_events WHERE match_id='newer'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(mu_before < 2500.0);
}

#[test]
fn mode_summary_counts_wins_losses_and_draws_as_discrete_results() {
    let (_temp, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "mode-win", 23, 100, "tank", "win");
    insert_match(&conn, "mode-loss", 23, 200, "tank", "loss");
    insert_match(&conn, "mode-draw", 23, 300, "tank", "draw");
    rebuild_career_mmr(&db).unwrap();
    let counts = conn.query_row("SELECT wins,losses,draws FROM career_role_mode_summary WHERE role='tank' AND mode_family='role_competitive'", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))).unwrap();
    assert_eq!(counts, (1, 1, 1));
}

#[test]
fn repeated_rebuild_keeps_every_persisted_event_and_summary_column_identical() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "stable", 23, 1_700_000_000, "tank", "win");
    drop(conn);
    rebuild_career_mmr(&db).unwrap();
    let before = persisted_mmr_rows(&db.connect().unwrap());
    rebuild_career_mmr(&db).unwrap();
    assert_eq!(before, persisted_mmr_rows(&db.connect().unwrap()));
}

#[test]
fn sha256_input_hash_changes_when_a_normalized_calculation_field_changes() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "same-match", 23, 1_700_000_000, "tank", "win");
    drop(conn);
    rebuild_career_mmr(&db).unwrap();
    let conn = db.connect().unwrap();
    let before: String = conn
        .query_row("SELECT input_hash FROM mmr_match_inputs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(before.len(), 64);
    conn.execute(
        "UPDATE matches SET result='loss' WHERE match_id='same-match'",
        [],
    )
    .unwrap();
    drop(conn);
    rebuild_career_mmr(&db).unwrap();
    let after: String = db
        .connect()
        .unwrap()
        .query_row("SELECT input_hash FROM mmr_match_inputs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_ne!(before, after);
}

#[test]
fn conflicting_model_version_is_rejected_without_mutating_the_existing_row() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "normal", 23, 1_700_000_000, "tank", "win");
    drop(conn);
    rebuild_career_mmr(&db).unwrap();
    let conn = db.connect().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT is_active FROM mmr_model_versions WHERE version=1",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    conn.execute(
        "UPDATE mmr_model_versions SET algorithm='conflicting_algorithm' WHERE version=1",
        [],
    )
    .unwrap();
    drop(conn);
    assert!(rebuild_career_mmr(&db).is_err());
    let conn = db.connect().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT algorithm FROM mmr_model_versions WHERE version=1",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "conflicting_algorithm"
    );
}

#[test]
fn roles_have_independent_rating_state() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "tank-win", 23, 100, "tank", "win");
    insert_match(&conn, "damage-loss", 23, 200, "damage", "loss");
    insert_match(&conn, "support-win", 23, 300, "support", "win");
    drop(conn);
    rebuild_career_mmr(&db).unwrap();
    let conn = db.connect().unwrap();
    let damage_before: f64 = conn
        .query_row(
            "SELECT mu_before FROM career_mmr_events WHERE match_id='damage-loss'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let support_before: f64 = conn
        .query_row(
            "SELECT mu_before FROM career_mmr_events WHERE match_id='support-win'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((damage_before, support_before), (2500.0, 2500.0));
}

#[test]
fn synthetic_end_to_end_replay_covers_dynamic_seasons_roles_modes_caps_and_stadium_final_round() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_mode_match(&conn, "role-tank", 15, 100, "tank", "win", "SportPreset");
    insert_mode_match(&conn, "open-damage", 29, 200, "dps", "loss", "SportOpen");
    insert_mode_match(
        &conn,
        "role-quick-support",
        42,
        300,
        "healer",
        "win",
        "LeisurePreset",
    );
    insert_mode_match(
        &conn,
        "open-quick-support",
        42,
        350,
        "healer",
        "win",
        "Leisure6v6",
    );
    insert_stadium_match(&conn, "stadium-final", 42, 400);
    insert_stadium_match(&conn, "stadium-quick", 42, 500);
    conn.execute(
        "UPDATE matches SET game_mode='QuickFight' WHERE match_id='stadium-quick'",
        [],
    )
    .unwrap();
    drop(conn);

    let summary = rebuild_career_mmr(&db).unwrap();
    assert_eq!(summary.discovered_seasons, vec![15, 29, 42]);

    let conn = db.connect().unwrap();
    let modes = conn
        .prepare("SELECT mode_family FROM career_role_mode_summary ORDER BY mode_family")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        modes,
        vec![
            "open_competitive",
            "open_quick",
            "role_competitive",
            "role_quick",
            "stadium_competitive",
            "stadium_quick"
        ]
    );
    let roles = conn
        .prepare("SELECT DISTINCT role FROM career_mmr_events ORDER BY role")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(roles, vec!["damage", "support", "tank"]);

    let mut statement = conn
        .prepare("SELECT mode_family,final_delta FROM career_mmr_events")
        .unwrap();
    for row in statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .unwrap()
    {
        let (mode, delta) = row.unwrap();
        let cap = if mode.ends_with("_quick") { 10.0 } else { 30.0 };
        assert!(delta.abs() <= cap);
    }
    let stadium_quick_mode: String = conn
        .query_row(
            "SELECT mode_family FROM career_mmr_events WHERE match_id='stadium-quick'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stadium_quick_mode, "stadium_quick");
    let stadium_role: String = conn
        .query_row(
            "SELECT role FROM career_mmr_events WHERE match_id='stadium-final'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stadium_role, "support");
}

#[test]
fn summaries_apply_global_inactivity_and_persist_prepared_event_sigma() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "tank-old", 22, 100, "tank", "win");
    insert_match(&conn, "tank-next-season", 23, 200, "tank", "win");
    insert_match(
        &conn,
        "damage-watermark",
        23,
        200 + 91 * 86_400,
        "damage",
        "win",
    );
    drop(conn);
    rebuild_career_mmr(&db).unwrap();
    let conn = db.connect().unwrap();
    let (first_after, second_before): (f64, f64) = conn
        .query_row(
            "SELECT a.sigma_after,b.sigma_before FROM career_mmr_events a JOIN career_mmr_events b ON a.match_id='tank-old' AND b.match_id='tank-next-season'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(second_before > first_after);
    let (event_mu, event_sigma, summary_mu, summary_sigma): (f64, f64, f64, f64) = conn
        .query_row(
            "SELECT e.mu_after,e.sigma_after,s.mu,s.sigma FROM career_mmr_events e JOIN career_role_mmr s ON s.role=e.role WHERE e.match_id='tank-next-season'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(summary_mu, event_mu);
    assert!(summary_sigma > event_sigma);
}

#[test]
fn all_persisted_display_summaries_are_clamped_to_zero_through_five_thousand() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    for index in 0..240 {
        insert_match(
            &conn,
            &format!("extreme-win-{index}"),
            99,
            100 + index,
            "tank",
            "win",
        );
    }
    drop(conn);
    rebuild_career_mmr(&db).unwrap();
    let conn = db.connect().unwrap();
    let career_outside: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_role_mmr WHERE mmr NOT BETWEEN 0 AND 5000 OR peak_mmr NOT BETWEEN 0 AND 5000",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let season_outside: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_role_season_summary WHERE start_mmr NOT BETWEEN 0 AND 5000 OR end_mmr NOT BETWEEN 0 AND 5000 OR peak_mmr NOT BETWEEN 0 AND 5000",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((career_outside, season_outside), (0, 0));
}

#[test]
fn transaction_failure_preserves_previous_complete_events() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "first", 23, 100, "tank", "win");
    drop(conn);
    let previous = digest(&db);
    let conn = db.connect().unwrap();
    insert_match(&conn, "second", 23, 200, "tank", "loss");
    conn.execute_batch("CREATE TRIGGER fail_mmr_summary BEFORE INSERT ON career_role_mmr BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    drop(conn);
    assert!(rebuild_career_mmr(&db).is_err());
    let conn = db.connect().unwrap();
    conn.execute_batch("DROP TRIGGER fail_mmr_summary").unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM career_mmr_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    drop(conn);
    assert_ne!(previous, digest(&db));
}

#[test]
fn import_commits_matches_and_returns_safe_warning_when_rebuild_fails() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    conn.execute_batch("CREATE TRIGGER fail_mmr_summary BEFORE INSERT ON career_role_mmr BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    drop(conn);
    let payload = serde_json::json!({
        "season": 31,
        "matches": {"entries": [{"matchId":"imported-safe","gameMode":"SportPreset","matchRet":1,"mapGuid":"map","heroGuid":"hero","beginTs":1700000000,"roleType":"tank"}], "details": [], "detail_errors": []}
    });
    let summary = import_payload_value_embedded(&db, &payload).unwrap();
    assert_eq!(
        summary.mmr_warning.as_deref(),
        Some("比赛已成功导入，但生涯分重建失败；已保留上一套完整结果。")
    );
    let conn = db.connect().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM matches WHERE match_id='imported-safe'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM career_mmr_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
