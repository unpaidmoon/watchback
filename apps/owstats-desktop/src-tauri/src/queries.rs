use crate::db::OwDatabase;
use crate::importer::{int_field, string_field};
use crate::models::{
    ComparisonMetrics, FilterOption, FilterOptions, MatchDetailView, MatchEvaluationSummary,
    MatchFilters, MatchPerformanceAward, MatchSummary, PlayerRow, SameRoleComparison,
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
            m.match_id, m.season, m.bucket, m.game_mode, m.result, m.map_guid, COALESCE(m.map_name, ''),
            m.hero_guid, COALESCE(m.hero_name, ''), m.begin_ts, m.team_score, m.opponent_score,
            m.kills, m.assists, m.deaths, m.damage, m.healing, m.mitigation, m.duration_sec,
            e.grade, e.primary_tag, e.evidence_tags_json, e.confidence
        FROM matches m
        LEFT JOIN match_evaluations e ON e.match_id = m.match_id
        WHERE m.bucket = 'preset'
        "#,
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    append_filters(&mut sql, &mut owned, &filters);
    sql.push_str(" ORDER BY m.begin_ts DESC, m.match_id");

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
        let match_id: String = row.get(0)?;
        Ok(MatchSummary {
            match_id: match_id.clone(),
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
            evaluation: evaluation_from_row(row, 19)?,
            team_awards: Vec::new(),
        })
    })?;
    let mut matches = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for item in &mut matches {
        item.team_awards = team_awards_from_match_players(&conn, &item.match_id)?;
    }
    Ok(matches)
}

pub fn list_filter_options(db: &OwDatabase) -> Result<FilterOptions> {
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

pub fn get_match_detail(db: &OwDatabase, match_id: &str) -> Result<MatchDetailView> {
    db.initialize()?;
    let conn = db.connect()?;
    let (
        entry_text,
        detail_text,
        game_mode,
        result,
        map_guid,
        map_name,
        hero_guid,
        hero_name,
        begin_ts,
        team_score,
        opponent_score,
        duration_sec,
    ): (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    ) = conn
        .query_row(
            r#"
            SELECT
                matches.raw_entry_json,
                match_details.raw_detail_json,
                NULLIF(matches.game_mode, ''),
                NULLIF(matches.result, ''),
                NULLIF(matches.map_guid, ''),
                NULLIF(matches.map_name, ''),
                NULLIF(matches.hero_guid, ''),
                NULLIF(matches.hero_name, ''),
                matches.begin_ts,
                matches.team_score,
                matches.opponent_score,
                matches.duration_sec
            FROM matches
            JOIN match_details ON match_details.match_id = matches.match_id
            WHERE matches.match_id = ?
            "#,
            params![match_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )
        .with_context(|| format!("未找到对局详情：{match_id}"))?;
    let entry: Value = serde_json::from_str(&entry_text)?;
    let detail: Value = serde_json::from_str(&detail_text)?;
    let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
    let hero_names = hero_lookup(&conn)?;
    let teammates = player_rows(data.get("teammateList"), "teammate", &hero_names);
    let enemies = player_rows(data.get("enemyList"), "enemy", &hero_names);
    let team_awards = team_awards_from_match_players(&conn, match_id)?;
    Ok(MatchDetailView {
        match_id: match_id.to_string(),
        mode_label: game_mode.as_deref().map(mode_label).map(str::to_string),
        result_label: result.as_deref().map(result_label).map(str::to_string),
        game_mode,
        result,
        map_guid,
        map_name,
        hero_guid,
        hero_name,
        begin_label: begin_ts.map(format_begin_ts),
        begin_ts,
        team_score,
        opponent_score,
        duration_sec,
        teammates,
        enemies,
        evaluation: match_evaluation_summary(&conn, match_id)?,
        team_awards,
        same_role_comparison: same_role_comparison(&conn, match_id)?,
        raw_json: redact_sensitive_json(&json!({ "entry": entry, "detail": detail })),
    })
}

fn team_awards_from_match_players(
    conn: &rusqlite::Connection,
    match_id: &str,
) -> Result<Vec<MatchPerformanceAward>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT side, player_name, bnet_id, hero_guid, hero_name, kill, assist, death,
               hero_damage, cure, resist_damage, final_hit, target_competing_time,
               healing_taken, damage_taken
        FROM match_players
        WHERE match_id=? AND side='teammate'
        ORDER BY is_current_player DESC, rowid ASC
        "#,
    )?;
    let rows = stmt.query_map(params![match_id], |row| {
        Ok(PlayerRow {
            side: row.get(0)?,
            name: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            bnet_id: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            hero_guid: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            hero_name: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            kills: row.get(5)?,
            assists: row.get(6)?,
            deaths: row.get(7)?,
            damage: row.get(8)?,
            healing: row.get(9)?,
            mitigation: row.get(10)?,
            final_hits: row.get(11)?,
            target_time_sec: row.get(12)?,
            healing_taken: row.get(13)?,
            damage_taken: row.get(14)?,
        })
    })?;
    let players = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(team_awards_from_players(&players))
}

fn team_awards_from_players(players: &[PlayerRow]) -> Vec<MatchPerformanceAward> {
    let Some(current) = players.first() else {
        return Vec::new();
    };
    let mut awards = Vec::new();
    push_award_if_highest(&mut awards, "kill", "击杀最多", "gold", current.kills, players.iter().map(|player| player.kills), true);
    push_award_if_highest(&mut awards, "damage", "伤害最高", "gold", current.damage, players.iter().map(|player| player.damage), true);
    push_award_if_highest(&mut awards, "healing", "治疗最高", "gold", current.healing, players.iter().map(|player| player.healing), true);
    push_award_if_highest(&mut awards, "mitigation", "格挡最多", "gold", current.mitigation, players.iter().map(|player| player.mitigation), true);
    push_award_if_highest(&mut awards, "death", "死亡最多", "danger", current.deaths, players.iter().map(|player| player.deaths), false);
    awards
}

fn push_award_if_highest(
    awards: &mut Vec<MatchPerformanceAward>,
    code: &str,
    label: &str,
    tone: &str,
    value: i64,
    values: impl Iterator<Item = i64>,
    require_positive: bool,
) {
    if require_positive && value <= 0 {
        return;
    }
    if values.max().map(|max| value >= max && max > 0).unwrap_or(false) {
        awards.push(MatchPerformanceAward {
            code: code.to_string(),
            label: label.to_string(),
            tone: tone.to_string(),
        });
    }
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
        sql.push_str(" AND m.season = ?");
        owned.push(Box::new(season));
    }
    if let Some(hero) = non_empty(&filters.hero) {
        sql.push_str(" AND m.hero_guid = ?");
        owned.push(Box::new(hero));
    }
    if let Some(map) = non_empty(&filters.map) {
        sql.push_str(" AND m.map_guid = ?");
        owned.push(Box::new(map));
    }
    if let Some(result) = non_empty(&filters.result) {
        sql.push_str(" AND m.result = ?");
        owned.push(Box::new(result));
    }
    if let Some(mode) = non_empty(&filters.mode) {
        sql.push_str(" AND m.game_mode = ?");
        owned.push(Box::new(mode));
    }
    if let Some(primary_tag) =
        non_empty(&filters.primary_tag).filter(|tag| filterable_primary_tag(tag))
    {
        sql.push_str(" AND e.primary_tag = ?");
        owned.push(Box::new(primary_tag));
    }
    if let Some(from) = non_empty(&filters.date_from).and_then(|date| begin_bound(&date, false)) {
        sql.push_str(" AND m.begin_ts >= ?");
        owned.push(Box::new(from));
    }
    if let Some(to) = non_empty(&filters.date_to).and_then(|date| begin_bound(&date, true)) {
        sql.push_str(" AND m.begin_ts <= ?");
        owned.push(Box::new(to));
    }
}

fn filterable_primary_tag(value: &str) -> bool {
    matches!(value, "带飞局" | "尽力局" | "躺赢局" | "背锅局")
}

fn match_evaluation_summary(
    conn: &rusqlite::Connection,
    match_id: &str,
) -> Result<Option<MatchEvaluationSummary>> {
    conn.query_row(
        r#"
        SELECT grade, primary_tag, evidence_tags_json, confidence
        FROM match_evaluations
        WHERE match_id=?
        "#,
        params![match_id],
        |row| evaluation_from_row(row, 0),
    )
    .optional()
    .map(|value| value.flatten())
    .map_err(Into::into)
}

fn evaluation_from_row(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<Option<MatchEvaluationSummary>> {
    let grade: Option<String> = row.get(offset)?;
    let Some(grade) = grade else {
        return Ok(None);
    };
    let evidence_text: String = row.get(offset + 2)?;
    let evidence_tags = serde_json::from_str::<Vec<String>>(&evidence_text).unwrap_or_default();
    Ok(Some(MatchEvaluationSummary {
        grade,
        primary_tag: row.get(offset + 1)?,
        evidence_tags,
        confidence: row.get(offset + 3)?,
    }))
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
