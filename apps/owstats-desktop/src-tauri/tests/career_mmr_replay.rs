#![allow(clippy::too_many_arguments)]

use owstats_desktop_lib::{
    career_mmr::{
        input::{ModeFamily, PerformanceTier, Role, TrainingCohort},
        performance_selection::SelectedRoleModel,
        replay::{
            rebuild_career_mmr, rebuild_career_mmr_active, rebuild_career_mmr_v3,
            BASELINE_MODEL_VERSION, PACKAGED_V3_ARTIFACT_JSON, PERFORMANCE_MODEL_VERSION,
        },
        training_report::{
            recompute_v3_config_hash, validate_parsed_v3_artifact, AggregateEvidenceArtifact,
            CareerMmrV3Artifact, PerformanceModelArtifact, SharedFamilyArtifact,
            V3_ARTIFACT_ALGORITHM, V3_SCHEMA_VERSION,
        },
    },
    import_payload_value_embedded, OwDatabase,
};
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::tempdir;

fn database() -> (tempfile::TempDir, OwDatabase) {
    let dir = tempdir().unwrap();
    let db = OwDatabase::new(dir.path().join("replay.sqlite3"));
    db.initialize().unwrap();
    (dir, db)
}

#[test]
fn packaged_v3_artifact_is_valid_and_product_entrypoint_targets_v3() {
    let packaged: PerformanceModelArtifact =
        serde_json::from_str(PACKAGED_V3_ARTIFACT_JSON).unwrap();
    let artifact = packaged.v3_runtime.unwrap();
    validate_parsed_v3_artifact(&artifact).unwrap();
    assert_eq!(artifact.schema_version, V3_SCHEMA_VERSION);
    assert_eq!(artifact.selected_models.len(), 3);
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    insert_match(&conn, "packaged-tank", 23, 100, "tank", "win");
    insert_match(&conn, "packaged-damage", 23, 200, "damage", "win");
    insert_match(&conn, "packaged-support", 23, 300, "support", "win");
    drop(conn);
    let summary = rebuild_career_mmr_active(&db).unwrap();
    assert_eq!(summary.model_version, PERFORMANCE_MODEL_VERSION);
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
            "SELECT begin_ts, season FROM career_mmr_events WHERE model_version=?1 ORDER BY rowid",
        )
        .unwrap()
        .query_map([BASELINE_MODEL_VERSION], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
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
fn rebuild_creates_v2_without_mutating_the_legacy_v1_definition() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO mmr_model_versions(version,algorithm,config_json,is_active,created_at) VALUES(1,'role_bayesian_outcome_v1','{\"performanceWeight\":0.0}',1,'legacy')",
        [],
    )
    .unwrap();
    insert_match(&conn, "v2-match", 23, 100, "tank", "win");
    drop(conn);

    let summary = rebuild_career_mmr(&db).unwrap();
    assert_eq!(summary.model_version, 2);
    assert_eq!(BASELINE_MODEL_VERSION, 2);

    let conn = db.connect().unwrap();
    let legacy: (String, String, i64) = conn
        .query_row(
            "SELECT algorithm,config_json,is_active FROM mmr_model_versions WHERE version=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        legacy,
        (
            "role_bayesian_outcome_v1".into(),
            r#"{"performanceWeight":0.0}"#.into(),
            0,
        )
    );
    let current: (String, String, i64) = conn
        .query_row(
            "SELECT algorithm,config_json,is_active FROM mmr_model_versions WHERE version=2",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(current.0, "role_bayesian_outcome_six_mode_v2");
    assert_eq!(
        current.1,
        r#"{"performanceWeight":0.0,"trainingCohorts":{"role_competitive":"role_queue","role_quick":"role_queue","open_competitive":"open_queue","open_quick":"open_queue","stadium_competitive":"stadium_competitive","stadium_quick":"outcome_only"},"modeWeights":{"role_competitive":1.0,"role_quick":0.3,"open_competitive":1.0,"open_quick":0.3,"stadium_competitive":0.5,"stadium_quick":0.1},"modeCaps":{"competitive":30.0,"quick":10.0}}"#
    );
    assert_eq!(current.2, 1);
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
            "SELECT is_active FROM mmr_model_versions WHERE version=?1",
            [BASELINE_MODEL_VERSION],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    conn.execute(
        "UPDATE mmr_model_versions SET algorithm='conflicting_algorithm' WHERE version=?1",
        [BASELINE_MODEL_VERSION],
    )
    .unwrap();
    drop(conn);
    assert!(rebuild_career_mmr(&db).is_err());
    let conn = db.connect().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT algorithm FROM mmr_model_versions WHERE version=?1",
            [BASELINE_MODEL_VERSION],
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
fn import_commits_matches_without_rebuilding_disabled_career_mmr() {
    let (_dir, db) = database();
    let conn = db.connect().unwrap();
    conn.execute_batch("CREATE TRIGGER fail_mmr_summary BEFORE INSERT ON career_role_mmr BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    drop(conn);
    let payload = serde_json::json!({
        "season": 31,
        "matches": {"entries": [{"matchId":"imported-safe","gameMode":"SportPreset","matchRet":1,"mapGuid":"map","heroGuid":"hero","beginTs":1700000000,"roleType":"tank"}], "details": [], "detail_errors": []}
    });
    let summary = import_payload_value_embedded(&db, &payload).unwrap();
    assert!(summary.mmr_warning.is_none());
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

fn stadium_match_for_role(
    conn: &Connection,
    id: &str,
    season: i64,
    ts: i64,
    hero_guid: &str,
    hero_role: &str,
    final_hit: f64,
    kill: f64,
    assist: f64,
    death: f64,
) {
    conn.execute(
        "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, ?2, 'stadium', 'SportFight', 'arena', 'win', 'map', ?3, ?4, 1, 0, 0, 0, 0, 0, 0, 0, 0, 900, ?5, 'now')",
        params![id, season, hero_guid, ts, json!({"roleType": hero_role}).to_string()],
    ).unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO hero_dict (guid,name,role,updated_at) VALUES (?1,?2,?3,'now')",
        params![hero_guid, hero_role, hero_role],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_players (match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,raw_json,updated_at) VALUES (?1,?2,'teammate','synthetic-current','????',?3,1,'{}','now')",
        params![id, season, hero_guid],
    ).unwrap();
    let player = json!({
        "bnetId": "synthetic-current",
        "heroGuid": hero_guid,
        "finalHit": final_hit,
        "kill": kill,
        "assist": assist,
        "death": death
    });
    let detail = json!({"payload":{"data":{"roundCountList":[
        {"roundIndex":1,"teammateList":[player],"enemyList":[]}
    ]}}});
    conn.execute(
        "INSERT INTO match_details (match_id,match_kind,raw_detail_json,updated_at) VALUES (?1,'arena',?2,'now')",
        params![id, detail.to_string()],
    ).unwrap();
}

fn selected_model(role: Role, tier: PerformanceTier) -> SelectedRoleModel {
    let mut weights = BTreeMap::new();
    // A small, valid signed weight: final_hit is non-negative, death stays non-positive.
    weights.insert("final_hit".to_string(), 1.0);
    weights.insert("death".to_string(), -0.5);
    SelectedRoleModel {
        role,
        tier,
        weights,
        base_offset: 0.0,
        shrink: 0.3,
        training_cohort: TrainingCohort::StadiumCompetitive,
        config_hash: format!("test:{}:{}", tier.as_str(), role.as_str()),
    }
}

fn synthetic_v3_artifact_json() -> String {
    let mut selected_models = BTreeMap::new();
    selected_models.insert(
        Role::Tank,
        selected_model(Role::Tank, PerformanceTier::Learned),
    );
    selected_models.insert(
        Role::Damage,
        selected_model(Role::Damage, PerformanceTier::Shrunk),
    );
    selected_models.insert(
        Role::Support,
        selected_model(Role::Support, PerformanceTier::Percentile),
    );
    let aggregate = AggregateEvidenceArtifact {
        total_rows: 3,
        role_rows: [(Role::Tank, 1), (Role::Damage, 1), (Role::Support, 1)]
            .into_iter()
            .collect(),
        mode_rows: [(ModeFamily::StadiumCompetitive, 3)].into_iter().collect(),
        account_count: 1,
        season_count: 1,
        tier_selection: selected_models
            .iter()
            .map(|(role, m)| (*role, m.tier.as_str().to_string()))
            .collect(),
    };
    let mut artifact = CareerMmrV3Artifact {
        schema_version: V3_SCHEMA_VERSION,
        algorithm: V3_ARTIFACT_ALGORITHM.to_string(),
        ordinary_shared_v3: SharedFamilyArtifact {
            cohort: TrainingCohort::RoleQueue,
            shared_parameter_version: "ordinary-shared-v3".to_string(),
            modes: vec![
                ModeFamily::RoleCompetitive,
                ModeFamily::RoleQuick,
                ModeFamily::OpenCompetitive,
                ModeFamily::OpenQuick,
            ],
        },
        stadium_shared_v3: SharedFamilyArtifact {
            cohort: TrainingCohort::StadiumCompetitive,
            shared_parameter_version: "stadium-shared-v3".to_string(),
            modes: vec![ModeFamily::StadiumCompetitive, ModeFamily::StadiumQuick],
        },
        mode_scales: [(ModeFamily::StadiumCompetitive, 0.5)]
            .into_iter()
            .collect(),
        selected_models,
        aggregate_evidence: aggregate,
        config_hash: String::new(),
    };
    artifact.config_hash = recompute_v3_config_hash(&artifact).unwrap();
    serde_json::to_string(&artifact).unwrap()
}

fn seed_v3_stadium_matches(conn: &Connection) {
    stadium_match_for_role(
        conn,
        "tank-match",
        22,
        100,
        "stadium-tank",
        "tank",
        99.0,
        10.0,
        5.0,
        1.0,
    );
    stadium_match_for_role(
        conn,
        "damage-match",
        22,
        200,
        "stadium-dps",
        "dps",
        50.0,
        20.0,
        8.0,
        2.0,
    );
    stadium_match_for_role(
        conn,
        "support-match",
        22,
        300,
        "stadium-healer",
        "healer",
        12.0,
        2.0,
        30.0,
        3.0,
    );
}

fn active_version(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT version FROM mmr_model_versions WHERE is_active=1",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn v3_events_contain_nonzero_bounded_corrections_for_one_role_per_tier() {
    let (_dir, db) = database();
    {
        let conn = db.connect().unwrap();
        seed_v3_stadium_matches(&conn);
    }
    rebuild_career_mmr(&db).unwrap();
    let v2_count: i64;
    {
        let conn = db.connect().unwrap();
        v2_count = conn
            .query_row(
                "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=2",
                [],
                |r| r.get(0),
            )
            .unwrap();
    }
    assert!(v2_count > 0);

    let artifact = synthetic_v3_artifact_json();
    let summary = rebuild_career_mmr_v3(&db, &artifact).unwrap();
    assert_eq!(summary.model_version, PERFORMANCE_MODEL_VERSION);
    let conn = db.connect().unwrap();
    assert_eq!(active_version(&conn), PERFORMANCE_MODEL_VERSION);

    let rows: Vec<(String, f64, String)> = conn
        .prepare("SELECT role, performance_adjustment, performance_tier FROM career_mmr_events WHERE model_version=?1 ORDER BY role")
        .unwrap()
        .query_map([PERFORMANCE_MODEL_VERSION], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let mut tiers = std::collections::BTreeSet::new();
    let mut nonzero = 0;
    for (role, adj, tier) in &rows {
        assert_eq!(
            tier.as_str(),
            match role.as_str() {
                "tank" => "learned",
                "damage" => "shrunk",
                "support" => "percentile",
                _ => panic!("unexpected role {role}"),
            }
        );
        tiers.insert(tier.clone());
        let cap = ModeFamily::StadiumCompetitive.runtime_correction_cap(match tier.as_str() {
            "learned" => PerformanceTier::Learned,
            "shrunk" => PerformanceTier::Shrunk,
            _ => PerformanceTier::Percentile,
        });
        assert!(
            *adj <= cap + 1e-9 && *adj >= -cap - 1e-9,
            "adjustment {adj} exceeds tier cap {cap}"
        );
        if adj.abs() > 1e-9 {
            nonzero += 1;
        }
    }
    assert_eq!(tiers.len(), 3, "exactly one role per tier must appear");
    assert!(
        nonzero >= 3,
        "each role must receive a non-zero bounded correction"
    );

    // v2 rows are retained untouched.
    let v2_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(v2_after, v2_count);
    let v2_tier_null: i64 = conn.query_row("SELECT COUNT(*) FROM career_mmr_events WHERE model_version=2 AND performance_tier IS NOT NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(v2_tier_null, 0);
}

#[test]
fn v3_rebuild_is_byte_for_byte_identical() {
    let (_dir, db) = database();
    {
        let conn = db.connect().unwrap();
        seed_v3_stadium_matches(&conn);
    }
    let artifact = synthetic_v3_artifact_json();
    let first = rebuild_career_mmr_v3(&db, &artifact).unwrap();
    let first_rows = {
        let conn = db.connect().unwrap();
        persisted_mmr_rows(&conn)
    };
    let second = rebuild_career_mmr_v3(&db, &artifact).unwrap();
    let second_rows = {
        let conn = db.connect().unwrap();
        persisted_mmr_rows(&conn)
    };
    assert_eq!(first.event_digest, second.event_digest);
    assert_eq!(
        first_rows, second_rows,
        "v3 persisted rows must be byte-for-byte identical across rebuilds"
    );
}

#[test]
fn v2_rows_remain_queryable_after_v3_activation() {
    let (_dir, db) = database();
    {
        let conn = db.connect().unwrap();
        seed_v3_stadium_matches(&conn);
    }
    rebuild_career_mmr(&db).unwrap();
    let artifact = synthetic_v3_artifact_json();
    rebuild_career_mmr_v3(&db, &artifact).unwrap();
    let conn = db.connect().unwrap();
    assert_eq!(active_version(&conn), PERFORMANCE_MODEL_VERSION);
    let v2_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(v2_rows > 0, "v2 rows must still be present for rollback");
    let v2_role: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_role_mmr WHERE model_version=2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(v2_role, 3, "v2 role summaries must remain");
}

#[test]
fn injected_mid_transaction_failure_leaves_v2_active_and_rolls_back_v3() {
    let (_dir, db) = database();
    {
        let conn = db.connect().unwrap();
        seed_v3_stadium_matches(&conn);
    }
    rebuild_career_mmr(&db).unwrap();
    let v2_events: i64 = {
        let conn = db.connect().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=2",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    {
        let conn = db.connect().unwrap();
        assert_eq!(active_version(&conn), BASELINE_MODEL_VERSION);
    }
    // Inject a failure during v3 summary persistence.
    {
        let conn = db.connect().unwrap();
        conn.execute_batch("CREATE TRIGGER fail_v3_summary BEFORE INSERT ON career_role_mode_summary WHEN NEW.model_version=3 BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    }
    let artifact = synthetic_v3_artifact_json();
    let result = rebuild_career_mmr_v3(&db, &artifact);
    assert!(
        result.is_err(),
        "v3 activation must fail when injected fault fires"
    );
    let conn = db.connect().unwrap();
    conn.execute_batch("DROP TRIGGER fail_v3_summary").unwrap();
    assert_eq!(
        active_version(&conn),
        BASELINE_MODEL_VERSION,
        "v2 must remain active after rollback"
    );
    let v3_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=3",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(v3_events, 0, "rolled-back v3 events must not persist");
    let v2_after: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        v2_after, v2_events,
        "v2 events must be untouched by the failed v3 activation"
    );
}

#[test]
fn v3_activation_rejects_an_invalid_artifact_without_writing() {
    let (_dir, db) = database();
    let bad = serde_json::json!({
        "schema_version": 3,
        "algorithm": "career-mmr-v3-guaranteed-performance",
        "ordinary_shared_v3": {"cohort":"role_queue","shared_parameter_version":"x","modes":[]},
        "stadium_shared_v3": {"cohort":"stadium_competitive","shared_parameter_version":"x","modes":[]},
        "mode_scales": {},
        "selected_models": {},
        "aggregate_evidence": {"total_rows":0,"role_rows":{},"mode_rows":{},"account_count":0,"season_count":0,"tier_selection":{}},
        "config_hash": "deadbeef"
    })
    .to_string();
    assert!(rebuild_career_mmr_v3(&db, &bad).is_err());
    let conn = db.connect().unwrap();
    let v3_events: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=3",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(v3_events, 0);
}
