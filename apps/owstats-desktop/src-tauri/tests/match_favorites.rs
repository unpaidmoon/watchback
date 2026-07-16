use anyhow::Result;
use owstats_desktop_lib::{
    delete_match_favorite, get_match_detail, get_match_favorite, list_matches, save_match_favorite,
    MatchFilters, OwDatabase,
};
use rusqlite::params;

fn database(filename: &str) -> Result<(tempfile::TempDir, OwDatabase)> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join(filename));
    db.initialize()?;
    Ok((temp, db))
}

fn seed_match(db: &OwDatabase, match_id: &str) -> Result<()> {
    seed_match_record(db, match_id, 23, "normal", "SportPreset", 100)
}

fn seed_match_record(
    db: &OwDatabase,
    match_id: &str,
    season: i64,
    match_kind: &str,
    game_mode: &str,
    begin_ts: i64,
) -> Result<()> {
    let conn = db.connect()?;
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json,
            updated_at
        )
        VALUES (
            ?1, ?2, 'preset', ?3, ?4, 'win', 'test-map', '测试地图',
            'test-hero', '测试英雄', ?5, 2, 1, 10, 5, 2, 5000, 1000, 500, 0, 600,
            '{}', '2026-07-16T00:00:00Z'
        )
        "#,
        params![match_id, season, game_mode, match_kind, begin_ts],
    )?;
    conn.execute(
        r#"
        INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
        VALUES (
            ?1, ?2,
            '{"payload":{"data":{"teammateList":[],"enemyList":[],"roundCountList":[]}}}',
            '2026-07-16T00:00:00Z'
        )
        "#,
        params![match_id, match_kind],
    )?;
    Ok(())
}

#[test]
fn projects_favorite_state_filters_with_and_and_keeps_note_out_of_raw_json() -> Result<()> {
    let (_temp, db) = database("query.sqlite3")?;
    seed_match_record(&db, "normal-favorite", 23, "normal", "SportPreset", 300)?;
    seed_match_record(&db, "normal-plain", 23, "normal", "SportPreset", 200)?;
    seed_match_record(&db, "arena-favorite", 22, "fight", "LeisureFight", 100)?;
    save_match_favorite(&db, "normal-favorite", "本地私密备注")?;
    save_match_favorite(&db, "arena-favorite", "")?;

    let default_rows = list_matches(&db, MatchFilters::default())?;
    assert!(default_rows
        .iter()
        .find(|row| row.match_id == "normal-favorite")
        .is_some_and(|row| row.is_favorite));
    assert!(default_rows
        .iter()
        .find(|row| row.match_id == "normal-plain")
        .is_some_and(|row| !row.is_favorite));

    let favorite_rows = list_matches(
        &db,
        MatchFilters {
            favorite_only: Some(true),
            ..MatchFilters::default()
        },
    )?;
    assert_eq!(
        favorite_rows
            .iter()
            .map(|row| row.match_id.as_str())
            .collect::<Vec<_>>(),
        vec!["normal-favorite", "arena-favorite"]
    );

    let season_favorite_rows = list_matches(
        &db,
        MatchFilters {
            season: Some(23),
            favorite_only: Some(true),
            ..MatchFilters::default()
        },
    )?;
    assert_eq!(season_favorite_rows.len(), 1);
    assert_eq!(season_favorite_rows[0].match_id, "normal-favorite");

    let detail = get_match_detail(&db, "normal-favorite")?;
    assert_eq!(
        detail
            .favorite
            .as_ref()
            .map(|favorite| favorite.note.as_str()),
        Some("本地私密备注")
    );
    assert!(!detail.raw_json.to_string().contains("本地私密备注"));
    Ok(())
}

#[test]
fn upgrades_schema_v7_to_v8_without_losing_matches_and_repeat_initialize_is_idempotent(
) -> Result<()> {
    let (_temp, db) = database("upgrade.sqlite3")?;
    seed_match(&db, "match-before-upgrade")?;
    let conn = db.connect()?;
    conn.execute_batch(
        r#"
        DROP TABLE match_favorites;
        PRAGMA user_version = 7;
        "#,
    )?;
    let version_before_upgrade: i64 =
        conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
    let favorites_before_upgrade: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'match_favorites'",
        [],
        |row| row.get(0),
    )?;
    let matches_before_upgrade: i64 = conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE match_id = 'match-before-upgrade'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(version_before_upgrade, 7);
    assert_eq!(favorites_before_upgrade, 0);
    assert_eq!(matches_before_upgrade, 1);
    drop(conn);

    db.initialize()?;
    db.initialize()?;

    let conn = db.connect()?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
    let table_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'match_favorites'",
        [],
        |row| row.get(0),
    )?;
    let index_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_match_favorites_updated'",
        [],
        |row| row.get(0),
    )?;
    let preserved_matches: i64 = conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE match_id = 'match-before-upgrade'",
        [],
        |row| row.get(0),
    )?;
    let cascade_foreign_keys: i64 = conn.query_row(
        r#"
        SELECT COUNT(*)
        FROM pragma_foreign_key_list('match_favorites')
        WHERE "table" = 'matches'
          AND "from" = 'match_id'
          AND "to" = 'match_id'
          AND on_delete = 'CASCADE'
        "#,
        [],
        |row| row.get(0),
    )?;

    assert_eq!(version, 8);
    assert_eq!(preserved_matches, 1);
    assert_eq!(table_count, 1);
    assert_eq!(index_count, 1);
    assert_eq!(cascade_foreign_keys, 1);
    Ok(())
}

#[test]
fn saves_empty_note_and_updates_multiline_chinese_without_replacing_created_at() -> Result<()> {
    let (_temp, db) = database("save.sqlite3")?;
    seed_match(&db, "match-save")?;

    let saved = save_match_favorite(&db, "match-save", "")?;
    assert_eq!(saved.note, "");
    let (created_at, first_updated_at): (String, String) = db.connect()?.query_row(
        "SELECT created_at, updated_at FROM match_favorites WHERE match_id = 'match-save'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(created_at, first_updated_at);
    let timestamp_sentinel = "2000-01-01T00:00:00Z";
    db.connect()?.execute(
        r#"
        UPDATE match_favorites
        SET created_at = ?1, updated_at = ?1
        WHERE match_id = 'match-save'
        "#,
        [timestamp_sentinel],
    )?;

    let note = "第一行：配合很好\n第二行：注意返场";
    let updated = save_match_favorite(&db, "match-save", note)?;
    assert_eq!(updated.note, note);
    assert_eq!(
        get_match_favorite(&db, "match-save")?,
        Some(updated.clone())
    );
    let (created_after_update, updated_at): (String, String) = db.connect()?.query_row(
        "SELECT created_at, updated_at FROM match_favorites WHERE match_id = 'match-save'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    assert_eq!(created_after_update, timestamp_sentinel);
    assert_ne!(updated_at, timestamp_sentinel);
    Ok(())
}

#[test]
fn delete_is_idempotent_and_match_can_be_favorited_again() -> Result<()> {
    let (_temp, db) = database("delete.sqlite3")?;
    seed_match(&db, "match-delete")?;
    save_match_favorite(&db, "match-delete", "首次收藏")?;

    delete_match_favorite(&db, "match-delete")?;
    delete_match_favorite(&db, "match-delete")?;
    assert_eq!(get_match_favorite(&db, "match-delete")?, None);

    let saved = save_match_favorite(&db, "match-delete", "重新收藏")?;
    assert_eq!(saved.note, "重新收藏");
    Ok(())
}

#[test]
fn note_limit_counts_unicode_characters() -> Result<()> {
    let (_temp, db) = database("limit.sqlite3")?;
    seed_match(&db, "match-limit")?;
    let valid = "好".repeat(500);

    assert_eq!(save_match_favorite(&db, "match-limit", &valid)?.note, valid);

    let error = save_match_favorite(&db, "match-limit", &"好".repeat(501))
        .unwrap_err()
        .to_string();
    assert_eq!(error, "备注最多 500 字");
    Ok(())
}

#[test]
fn missing_match_returns_stable_error() -> Result<()> {
    let (_temp, db) = database("missing.sqlite3")?;

    let error = save_match_favorite(&db, "missing-match", "")
        .unwrap_err()
        .to_string();

    assert_eq!(error, "未找到对应对局，无法收藏。");
    Ok(())
}

#[test]
fn deleting_match_cascades_to_favorite() -> Result<()> {
    let (_temp, db) = database("cascade.sqlite3")?;
    seed_match(&db, "match-cascade")?;
    save_match_favorite(&db, "match-cascade", "应随对局删除")?;

    db.connect()?
        .execute("DELETE FROM matches WHERE match_id = 'match-cascade'", [])?;

    assert_eq!(get_match_favorite(&db, "match-cascade")?, None);
    Ok(())
}

#[test]
fn separate_database_files_isolate_same_match_id() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let first = OwDatabase::new(temp.path().join("account-one.sqlite3"));
    let second = OwDatabase::new(temp.path().join("account-two.sqlite3"));
    first.initialize()?;
    second.initialize()?;
    seed_match(&first, "shared-match")?;
    seed_match(&second, "shared-match")?;

    save_match_favorite(&first, "shared-match", "账号一的备注")?;

    assert_eq!(
        get_match_favorite(&first, "shared-match")?.map(|favorite| favorite.note),
        Some("账号一的备注".to_string())
    );
    assert_eq!(get_match_favorite(&second, "shared-match")?, None);
    Ok(())
}
