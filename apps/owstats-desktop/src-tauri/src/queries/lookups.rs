use super::*;

pub(super) fn list_filter_options_impl(db: &OwDatabase) -> Result<FilterOptions> {
    db.initialize()?;
    let conn = db.connect()?;
    Ok(FilterOptions {
        seasons: distinct_i64(&conn, "season")?,
        heroes: distinct_named_options(&conn, "hero_guid", "hero_name", "未知英雄")?,
        maps: distinct_named_options(&conn, "map_guid", "map_name", "未知地图")?,
        results: distinct_text(&conn, "result")?,
        modes: distinct_text(&conn, "game_mode")?,
    })
}

pub(super) fn search_match_players_impl(
    db: &OwDatabase,
    query: &str,
) -> Result<Vec<PlayerSearchOption>> {
    db.initialize()?;
    ensure_arena_player_index(db, None)?;
    let needle = query.trim();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let conn = db.connect()?;
    let pattern = format!("%{}%", escape_like(needle));
    let mut stmt = conn.prepare(
        r#"
        WITH searchable_players AS (
            SELECT mp.match_id, mp.player_name, mp.bnet_id
            FROM match_players mp
            JOIN matches m ON m.match_id = mp.match_id
            WHERE m.bucket = 'preset' AND TRIM(mp.player_name) <> ''
            UNION
            SELECT amp.match_id, amp.player_name, amp.bnet_id
            FROM arena_match_players amp
            WHERE TRIM(amp.player_name) <> ''
        )
        SELECT sp.player_name, COUNT(DISTINCT sp.match_id) AS matches, MAX(m.begin_ts) AS latest_begin_ts
        FROM searchable_players sp
        JOIN matches m ON m.match_id = sp.match_id
        WHERE sp.player_name LIKE ?1 ESCAPE '\' OR sp.bnet_id LIKE ?1 ESCAPE '\'
        GROUP BY sp.player_name
        ORDER BY matches DESC, latest_begin_ts DESC, sp.player_name
        LIMIT 10
        "#,
    )?;
    let rows = stmt.query_map(params![pattern], |row| {
        Ok(PlayerSearchOption {
            player_name: row.get(0)?,
            matches: row.get(1)?,
            latest_begin_ts: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn ensure_arena_player_index(db: &OwDatabase, season: Option<i64>) -> Result<()> {
    let conn = db.connect()?;
    let missing_season = conn
        .query_row(
            r#"
            SELECT DISTINCT m.season
            FROM matches m
            JOIN match_details d ON d.match_id = m.match_id
            WHERE m.bucket = 'stadium'
              AND (m.match_kind = 'fight' OR d.match_kind = 'fight')
              AND (?1 IS NULL OR m.season = ?1)
              AND NOT EXISTS (
                  SELECT 1 FROM app_settings s
                  WHERE s.key = 'arena_player_index_v1:' || m.season
              )
            LIMIT 1
            "#,
            params![season],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    drop(conn);
    if missing_season.is_some() {
        rebuild_match_players_from_existing_details(db)?;
    }
    Ok(())
}

pub(super) fn hero_lookup(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT guid, name FROM hero_dict")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(Into::into)
}

pub(super) fn map_lookup(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT guid, name FROM map_dict")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(Into::into)
}

pub(super) fn distinct_i64(conn: &rusqlite::Connection, expr: &str) -> Result<Vec<i64>> {
    let sql = format!("SELECT DISTINCT {expr} FROM matches WHERE bucket='preset' ORDER BY {expr}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn distinct_text(conn: &rusqlite::Connection, expr: &str) -> Result<Vec<String>> {
    let sql = format!(
        "SELECT DISTINCT {expr} FROM matches WHERE bucket='preset' AND TRIM({expr}) <> '' ORDER BY {expr}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn distinct_named_options(
    conn: &rusqlite::Connection,
    id_column: &str,
    label_column: &str,
    fallback: &str,
) -> Result<Vec<FilterOption>> {
    let sql = format!(
        r#"
        SELECT {id_column}, COALESCE(MAX(NULLIF(TRIM({label_column}), '')), '')
        FROM matches
        WHERE bucket='preset' AND TRIM(COALESCE({id_column}, '')) <> ''
        GROUP BY {id_column}
        ORDER BY COALESCE(MAX(NULLIF(TRIM({label_column}), '')), {id_column})
        "#
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let label: String = row.get(1)?;
        Ok(FilterOption {
            label: fallback_name(label, &id, fallback),
            id,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub(super) fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub(super) fn begin_bound(date: &str, end_of_day: bool) -> Option<i64> {
    let suffix = if end_of_day { " 23:59:59" } else { " 00:00:00" };
    let naive =
        NaiveDateTime::parse_from_str(&format!("{date}{suffix}"), "%Y-%m-%d %H:%M:%S").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp())
}

pub(super) fn format_begin_ts(value: i64) -> String {
    if value <= 0 {
        return String::new();
    }
    let seconds = if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    };
    DateTime::from_timestamp(seconds, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

pub(super) fn fallback_name(value: String, guid: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        if guid.trim().is_empty() {
            fallback.to_string()
        } else {
            guid.to_string()
        }
    } else {
        value
    }
}

pub(super) fn result_label(result: &str) -> &'static str {
    match result {
        "win" => "胜利",
        "loss" => "失败",
        "draw" => "平局",
        _ => "未知",
    }
}

pub(super) fn mode_label(mode: &str) -> &str {
    match mode {
        "LeisureFight" => "角斗快速",
        "SportFight" => "角斗竞技",
        "LeisurePreset" => "快速预设",
        "Leisure6v6" => "快速 6v6",
        "SportPreset" => "竞技预设",
        "SportOpen" => "竞技开放",
        "leisure" => "快速",
        "sport" => "竞技",
        _ => mode,
    }
}
