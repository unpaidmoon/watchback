use anyhow::Result;
use owstats_desktop_lib::OwDatabase;
use rusqlite::{params, Connection};

const MMR_TABLES: [&str; 6] = [
    "mmr_model_versions",
    "mmr_match_inputs",
    "career_mmr_events",
    "career_role_mmr",
    "career_role_mode_summary",
    "career_role_season_summary",
];

fn assert_mmr_schema(conn: &Connection) -> Result<()> {
    for table in MMR_TABLES {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1, "missing or duplicated table {table}");
    }
    for index in [
        "idx_mmr_match_inputs_replay",
        "idx_career_mmr_events_role_history",
        "idx_career_role_season_summary_lookup",
    ] {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [index],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1, "missing or duplicated index {index}");
    }
    Ok(())
}

#[test]
fn creates_career_mmr_tables_and_indexes() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize()?;
    assert_mmr_schema(&db.connect()?)
}

#[test]
fn upgrades_legacy_database_and_repeat_initialize_is_idempotent() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("legacy.sqlite3");
    let db = OwDatabase::new(&path);
    db.initialize()?;
    let conn = db.connect()?;
    conn.execute_batch(
        r#"
        DROP TABLE career_role_mode_summary;
        DROP TABLE career_role_season_summary;
        DROP TABLE career_role_mmr;
        DROP TABLE career_mmr_events;
        DROP TABLE mmr_match_inputs;
        DROP TABLE mmr_model_versions;
        PRAGMA user_version = 4;
        "#,
    )?;
    drop(conn);

    db.initialize()?;
    db.initialize()?;
    let conn = db.connect()?;
    assert_mmr_schema(&conn)?;
    conn.execute(
        "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES ('upgraded-match', 22, 'career', 'competitive', 'normal', 'win', 'map', 'hero', 100, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, '{}', 'now')",
        [],
    )?;
    conn.execute(
        "INSERT INTO mmr_match_inputs (match_id, role, season, begin_ts, mode_family, result, duration_sec, role_share, role_source, data_quality, rank_coverage, input_hash) VALUES ('upgraded-match', 'tank', 22, 100, 'role_competitive', 1.0, 600, 1.0, 'hero_duration', 1.0, 1.0, 'hash')",
        [],
    )?;
    assert!(conn
        .execute(
            "INSERT INTO mmr_match_inputs (match_id, role, season, begin_ts, mode_family, result, duration_sec, role_share, role_source, data_quality, rank_coverage, input_hash) VALUES ('missing-match', 'tank', 22, 100, 'role_competitive', 1.0, 600, 1.0, 'hero_duration', 1.0, 1.0, 'hash')",
            [],
        )
        .is_err());
    let foreign_key_errors: i64 =
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(foreign_key_errors, 0);
    Ok(())
}

#[test]
fn upgrades_existing_v5_database_that_predates_career_mmr_tables() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("legacy-v5.sqlite3");
    let db = OwDatabase::new(&path);
    db.initialize()?;
    let conn = db.connect()?;
    conn.execute_batch(
        r#"
        DROP TABLE career_role_mode_summary;
        DROP TABLE career_role_season_summary;
        DROP TABLE career_role_mmr;
        DROP TABLE career_mmr_events;
        DROP TABLE mmr_match_inputs;
        DROP TABLE mmr_model_versions;
        PRAGMA user_version = 5;
        "#,
    )?;
    drop(conn);

    db.initialize()?;
    let conn = db.connect()?;
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
        7
    );
    assert_mmr_schema(&conn)
}

#[test]
fn rejects_invalid_roles_ranges_and_duplicate_events() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    conn.execute(
        "INSERT INTO mmr_model_versions (version, algorithm, config_json, is_active, created_at) VALUES (1, 'baseline', '{}', 1, 'now')",
        [],
    )?;
    assert!(conn
        .execute(
            "INSERT INTO mmr_model_versions (version, algorithm, config_json, is_active, created_at) VALUES (2, 'baseline', '{}', 2, 'now')",
            [],
        )
        .is_err());
    conn.execute(
        "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES ('match-1', 22, 'career', 'competitive', 'normal', 'win', 'map', 'hero', 100, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, '{}', 'now')",
        [],
    )?;

    let insert_input = |role: &str, result: f64, role_share: f64, quality: f64, coverage: f64| {
        conn.execute(
            "INSERT INTO mmr_match_inputs (match_id, role, season, begin_ts, mode_family, result, duration_sec, role_share, role_source, data_quality, rank_coverage, input_hash) VALUES ('match-1', ?1, 22, 100, 'role_competitive', ?2, 600, ?3, 'hero_duration', ?4, ?5, 'hash')",
            params![role, result, role_share, quality, coverage],
        )
    };
    assert!(insert_input("invalid", 1.0, 1.0, 1.0, 1.0).is_err());
    assert!(insert_input("tank", 1.1, 1.0, 1.0, 1.0).is_err());
    assert!(insert_input("tank", 1.0, 0.0, 1.0, 1.0).is_err());
    assert!(insert_input("tank", 1.0, 1.0, -0.1, 1.0).is_err());
    assert!(insert_input("tank", 1.0, 1.0, 1.0, 1.1).is_err());
    insert_input("tank", 1.0, 1.0, 1.0, 1.0)?;

    let insert_event = |model: i64,
                        match_id: &str,
                        role: &str,
                        role_share: f64,
                        result: f64,
                        expected: f64,
                        quality: f64,
                        confidence: f64| {
        conn.execute(
            "INSERT INTO career_mmr_events (model_version, match_id, role, season, begin_ts, mode_family, role_share, result, expected_win_rate, data_quality, performance_adjustment, mu_before, sigma_before, base_delta, final_delta, mu_after, sigma_after, confidence_after, input_hash, computed_at) VALUES (?1, ?2, ?3, 22, 100, 'role_competitive', ?4, ?5, ?6, ?7, 0.0, 2500.0, 350.0, 10.0, 10.0, 2510.0, 340.0, ?8, 'hash', 'now')",
            params![model, match_id, role, role_share, result, expected, quality, confidence],
        )
    };
    assert!(insert_event(1, "match-1", "invalid", 1.0, 1.0, 0.5, 1.0, 0.5).is_err());
    assert!(insert_event(1, "match-1", "tank", 0.0, 1.0, 0.5, 1.0, 0.5).is_err());
    assert!(insert_event(1, "match-1", "tank", 1.0, 1.1, 0.5, 1.0, 0.5).is_err());
    assert!(insert_event(1, "match-1", "tank", 1.0, 1.0, -0.1, 1.0, 0.5).is_err());
    assert!(insert_event(1, "match-1", "tank", 1.0, 1.0, 0.5, 1.1, 0.5).is_err());
    assert!(insert_event(1, "match-1", "tank", 1.0, 1.0, 0.5, 1.0, 1.1).is_err());
    assert!(insert_event(99, "match-1", "tank", 1.0, 1.0, 0.5, 1.0, 0.5).is_err());
    assert!(insert_event(1, "missing", "tank", 1.0, 1.0, 0.5, 1.0, 0.5).is_err());
    insert_event(1, "match-1", "tank", 1.0, 1.0, 0.5, 1.0, 0.5)?;
    assert!(insert_event(1, "match-1", "tank", 1.0, 1.0, 0.5, 1.0, 0.5).is_err());
    Ok(())
}
