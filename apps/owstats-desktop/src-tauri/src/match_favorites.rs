use crate::{MatchFavoriteView, OwDatabase};
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

pub fn get_match_favorite(
    database: &OwDatabase,
    match_id: &str,
) -> Result<Option<MatchFavoriteView>> {
    database.initialize()?;
    let connection = database.connect()?;
    connection
        .query_row(
            "SELECT note FROM match_favorites WHERE match_id = ?1",
            [match_id],
            |row| Ok(MatchFavoriteView { note: row.get(0)? }),
        )
        .optional()
        .map_err(Into::into)
}

pub fn save_match_favorite(
    database: &OwDatabase,
    match_id: &str,
    note: &str,
) -> Result<MatchFavoriteView> {
    if note.chars().count() > 500 {
        return Err(anyhow!("备注最多 500 字"));
    }

    database.initialize()?;
    let connection = database.connect()?;
    let match_exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM matches WHERE match_id = ?1)",
        [match_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !match_exists {
        return Err(anyhow!("未找到对应对局，无法收藏。"));
    }

    let now = Utc::now().to_rfc3339();
    connection.execute(
        r#"
        INSERT INTO match_favorites (match_id, note, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?3)
        ON CONFLICT(match_id) DO UPDATE SET
            note = excluded.note,
            updated_at = excluded.updated_at
        "#,
        params![match_id, note, now],
    )?;

    Ok(MatchFavoriteView {
        note: note.to_string(),
    })
}

pub fn delete_match_favorite(database: &OwDatabase, match_id: &str) -> Result<()> {
    database.initialize()?;
    database.connect()?.execute(
        "DELETE FROM match_favorites WHERE match_id = ?1",
        [match_id],
    )?;
    Ok(())
}
