use crate::db::OwDatabase;
use crate::importer::{int_field, string_field};
use crate::models::{
    ComparisonMetrics, FilterOption, FilterOptions, MatchDetailView, MatchFilters, MatchSummary,
    PlayerRow, SameRoleComparison,
};
use crate::redaction::redact_sensitive_json;
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use rusqlite::{params, OptionalExtension, ToSql};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn list_matches(db: &OwDatabase, filters: MatchFilters) -> Result<Vec<MatchSummary>> {
    db.initialize()?;
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT
            match_id, season, bucket, game_mode, result, map_guid, COALESCE(map_name, ''),
            hero_guid, COALESCE(hero_name, ''), begin_ts, team_score, opponent_score,
            kills, assists, deaths, damage, healing, mitigation, duration_sec
        FROM matches
        WHERE bucket = 'preset'
        "#,
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    append_filters(&mut sql, &mut owned, &filters);
    sql.push_str(" ORDER BY begin_ts DESC, match_id");

    let params: Vec<&dyn ToSql> = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        let game_mode: String = row.get(3)?;
        let result: String = row.get(4)?;
        let begin_ts: i64 = row.get(9)?;
        let map_guid: String = row.get(5)?;
        let hero_guid: String = row.get(7)?;
        let map_name: String = row.get(6)?;
        let hero_name: String = row.get(8)?;
        Ok(MatchSummary {
            match_id: row.get(0)?,
            season: row.get(1)?,
            bucket: row.get(2)?,
            mode_label: mode_label(&game_mode).to_string(),
            game_mode,
            result_label: result_label(&result).to_string(),
            result,
            map_guid: map_guid.clone(),
            map_name: fallback_name(map_name, &map_guid, "未知地图"),
            hero_guid: hero_guid.clone(),
            hero_name: fallback_name(hero_name, &hero_guid, "未知英雄"),
            begin_ts,
            begin_label: format_begin_ts(begin_ts),
            team_score: row.get(10)?,
            opponent_score: row.get(11)?,
            kills: row.get(12)?,
            assists: row.get(13)?,
            deaths: row.get(14)?,
            damage: row.get(15)?,
            healing: row.get(16)?,
            mitigation: row.get(17)?,
            duration_sec: row.get(18)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub fn list_filter_options(db: &OwDatabase) -> Result<FilterOptions> {
    db.initialize()?;
    let conn = db.connect()?;
    Ok(FilterOptions {
        seasons: distinct_i64(&conn, "season")?,
        heroes: distinct_named_options(&conn, "hero_guid", "hero_name", "鏈煡鑻遍泟")?,
        maps: distinct_named_options(&conn, "map_guid", "map_name", "鏈煡鍦板浘")?,
        results: distinct_text(&conn, "result")?,
        modes: distinct_text(&conn, "game_mode")?,
    })
}

pub fn get_match_detail(db: &OwDatabase, match_id: &str) -> Result<MatchDetailView> {
    db.initialize()?;
    let conn = db.connect()?;
    let (entry_text, detail_text): (String, String) = conn
        .query_row(
            r#"
            SELECT matches.raw_entry_json, match_details.raw_detail_json
            FROM matches
            JOIN match_details ON match_details.match_id = matches.match_id
            WHERE matches.match_id = ?
            "#,
            params![match_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .with_context(|| format!("未找到对局详情：{match_id}"))?;
    let entry: Value = serde_json::from_str(&entry_text)?;
    let detail: Value = serde_json::from_str(&detail_text)?;
    let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
    let hero_names = hero_lookup(&conn)?;
    Ok(MatchDetailView {
        match_id: match_id.to_string(),
        teammates: player_rows(data.get("teammateList"), "teammate", &hero_names),
        enemies: player_rows(data.get("enemyList"), "enemy", &hero_names),
        same_role_comparison: same_role_comparison(&conn, match_id)?,
        raw_json: redact_sensitive_json(&json!({ "entry": entry, "detail": detail })),
    })
}

fn same_role_comparison(
    conn: &rusqlite::Connection,
    match_id: &str,
) -> Result<Option<SameRoleComparison>> {
    let current = conn
        .query_row(
            r#"
            SELECT season, role_type, kill, death, hero_damage, cure
            FROM match_players
            WHERE match_id=? AND is_current_player=1 AND TRIM(COALESCE(role_type, '')) <> ''
            LIMIT 1
            "#,
            params![match_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    ComparisonMetrics {
                        kills: Some(row.get::<_, i64>(2)? as f64),
                        deaths: Some(row.get::<_, i64>(3)? as f64),
                        damage: Some(row.get::<_, i64>(4)? as f64),
                        healing: Some(row.get::<_, i64>(5)? as f64),
                    },
                ))
            },
        )
        .optional()?;
    let Some((season, raw_role, current)) = current else {
        return Ok(None);
    };
    let average = conn.query_row(
        r#"
        SELECT AVG(kill), AVG(death), AVG(hero_damage), AVG(cure)
        FROM match_players
        WHERE season=? AND is_current_player=1 AND role_type=?
        "#,
        params![season, raw_role],
        |row| {
            Ok(ComparisonMetrics {
                kills: row.get(0)?,
                deaths: row.get(1)?,
                damage: row.get(2)?,
                healing: row.get(3)?,
            })
        },
    )?;
    Ok(Some(SameRoleComparison {
        role_label: role_label(&raw_role).map(str::to_string),
        raw_role: Some(raw_role),
        current,
        average,
    }))
}

fn role_label(value: &str) -> Option<&'static str> {
    match value {
        "tank" => Some("坦克"),
        "dps" => Some("输出"),
        "healer" => Some("支援"),
        _ => None,
    }
}

fn append_filters(sql: &mut String, owned: &mut Vec<Box<dyn ToSql>>, filters: &MatchFilters) {
    if let Some(season) = filters.season {
        sql.push_str(" AND season = ?");
        owned.push(Box::new(season));
    }
    if let Some(hero) = non_empty(&filters.hero) {
        sql.push_str(" AND hero_guid = ?");
        owned.push(Box::new(hero));
    }
    if let Some(map) = non_empty(&filters.map) {
        sql.push_str(" AND map_guid = ?");
        owned.push(Box::new(map));
    }
    if let Some(result) = non_empty(&filters.result) {
        sql.push_str(" AND result = ?");
        owned.push(Box::new(result));
    }
    if let Some(mode) = non_empty(&filters.mode) {
        sql.push_str(" AND game_mode = ?");
        owned.push(Box::new(mode));
    }
    if let Some(from) = non_empty(&filters.date_from).and_then(|date| begin_bound(&date, false)) {
        sql.push_str(" AND begin_ts >= ?");
        owned.push(Box::new(from));
    }
    if let Some(to) = non_empty(&filters.date_to).and_then(|date| begin_bound(&date, true)) {
        sql.push_str(" AND begin_ts <= ?");
        owned.push(Box::new(to));
    }
}

fn player_rows(
    value: Option<&Value>,
    side: &str,
    hero_names: &HashMap<String, String>,
) -> Vec<PlayerRow> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if !item.is_object() {
                        return None;
                    }
                    let hero_guid = string_field(item, "heroGuid").unwrap_or_default();
                    Some(PlayerRow {
                        side: side.to_string(),
                        name: string_field(item, "name").unwrap_or_else(|| {
                            string_field(item, "bnetId").unwrap_or_else(|| "未知玩家".to_string())
                        }),
                        bnet_id: string_field(item, "bnetId").unwrap_or_default(),
                        hero_guid: hero_guid.clone(),
                        hero_name: hero_names.get(&hero_guid).cloned().unwrap_or_else(|| {
                            fallback_name(String::new(), &hero_guid, "未知英雄")
                        }),
                        kills: int_field(item, "kill"),
                        assists: int_field(item, "assist"),
                        deaths: int_field(item, "death"),
                        damage: int_field(item, "heroDamage"),
                        healing: int_field(item, "cure"),
                        mitigation: int_field(item, "resistDamage"),
                        final_hits: int_field(item, "finalHit"),
                        target_time_sec: int_field(item, "targetCompetingTime"),
                        healing_taken: int_field(item, "healingTaken"),
                        damage_taken: int_field(item, "damageTaken"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn hero_lookup(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT guid, name FROM hero_dict")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(Into::into)
}

fn distinct_i64(conn: &rusqlite::Connection, expr: &str) -> Result<Vec<i64>> {
    let sql = format!("SELECT DISTINCT {expr} FROM matches WHERE bucket='preset' ORDER BY {expr}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn distinct_text(conn: &rusqlite::Connection, expr: &str) -> Result<Vec<String>> {
    let sql = format!(
        "SELECT DISTINCT {expr} FROM matches WHERE bucket='preset' AND TRIM({expr}) <> '' ORDER BY {expr}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn distinct_named_options(
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

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn begin_bound(date: &str, end_of_day: bool) -> Option<i64> {
    let suffix = if end_of_day { " 23:59:59" } else { " 00:00:00" };
    let naive =
        NaiveDateTime::parse_from_str(&format!("{date}{suffix}"), "%Y-%m-%d %H:%M:%S").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp())
}

fn format_begin_ts(value: i64) -> String {
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

fn fallback_name(value: String, guid: &str, fallback: &str) -> String {
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

fn result_label(result: &str) -> &'static str {
    match result {
        "win" => "胜利",
        "loss" => "失败",
        "draw" => "平局",
        _ => "未知",
    }
}

fn mode_label(mode: &str) -> &str {
    match mode {
        "LeisurePreset" => "快速预设",
        "Leisure6v6" => "快速 6v6",
        "SportPreset" => "竞技预设",
        "SportOpen" => "竞技开放",
        "leisure" => "快速",
        "sport" => "竞技",
        _ => mode,
    }
}
