use anyhow::Result;
use owstats_desktop_lib::{
    delete_match_favorite, get_match_favorite, save_match_favorite, OwDatabase,
};
use rusqlite::params;
use std::thread;
use std::time::Duration;

fn database(filename: &str) -> Result<(tempfile::TempDir, OwDatabase)> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join(filename));
    db.initialize()?;
    Ok((temp, db))
}

fn seed_match(db: &OwDatabase, match_id: &str) -> Result<()> {
    db.connect()?.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json,
            updated_at
        )
        VALUES (
            ?1, 23, 'career', 'SportPreset', 'normal', 'win', 'test-map', '测试地图',
            'test-hero', '测试英雄', 100, 2, 1, 10, 5, 2, 5000, 1000, 500, 0, 600,
            '{}', '2026-07-16T00:00:00Z'
        )
        "#,
        params![match_id],
    )?;
    Ok(())
}

#[test]
fn repeat_initialize_creates_schema_v8_once() -> Result<()> {
    let (_temp, db) = database("schema.sqlite3")?;

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

    assert_eq!(version, 8);
    assert_eq!(table_count, 1);
    assert_eq!(index_count, 1);
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

    thread::sleep(Duration::from_millis(2));
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

    assert_eq!(created_after_update, created_at);
    assert!(updated_at > first_updated_at);
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
