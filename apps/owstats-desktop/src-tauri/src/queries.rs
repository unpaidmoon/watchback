use crate::db::OwDatabase;
use crate::importer::{int_field, string_field};
use crate::models::{
    ArenaDetailSlice, ArenaMatchDetail, ComparisonMetrics, FilterOption, FilterOptions, HeroStat,
    MatchDetailView, MatchEvaluationSummary, MatchFilters, MatchPerformanceAward, MatchSummary,
    PlayerPerk, PlayerRow, PlayerSearchOption, SameRoleComparison,
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
            e.grade, e.primary_tag, e.evidence_tags_json, e.confidence, m.match_kind, d.raw_detail_json
        FROM matches m
        LEFT JOIN match_evaluations e ON e.match_id = m.match_id
        LEFT JOIN match_details d ON d.match_id = m.match_id
        WHERE (m.bucket = 'preset' OR m.match_kind = 'fight' OR m.game_mode IN ('LeisureFight', 'SportFight'))
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
        let match_kind: String = row.get(23)?;
        let raw_detail_json: Option<String> = row.get(24)?;
        let fight_projection = if match_kind == "fight" {
            fight_list_projection(raw_detail_json.as_deref())
        } else {
            None
        };
        Ok(MatchSummary {
            match_id: match_id.clone(),
            season: row.get(1)?,
            bucket: row.get(2)?,
            match_kind: match_kind.clone(),
            mode_label: mode_label(&game_mode).to_string(),
            game_mode,
            result_label: result_label(&result).to_string(),
            result,
            map_guid: map_guid.clone(),
            map_preview_guid: if match_kind == "fight" {
                last_round_map_guid(raw_detail_json.as_deref())
            } else {
                None
            },
            map_name: fallback_name(map_name, &map_guid, "未知地图"),
            hero_guid: hero_guid.clone(),
            hero_name: fallback_name(hero_name, &hero_guid, "未知英雄"),
            begin_ts,
            begin_label: format_begin_ts(begin_ts),
            team_score: fight_projection
                .as_ref()
                .and_then(|projection| projection.team_score)
                .unwrap_or(row.get(10)?),
            opponent_score: fight_projection
                .as_ref()
                .and_then(|projection| projection.opponent_score)
                .unwrap_or(row.get(11)?),
            kills: fight_projection
                .as_ref()
                .and_then(|projection| projection.kills)
                .unwrap_or(row.get(12)?),
            assists: fight_projection
                .as_ref()
                .and_then(|projection| projection.assists)
                .unwrap_or(row.get(13)?),
            deaths: fight_projection
                .as_ref()
                .and_then(|projection| projection.deaths)
                .unwrap_or(row.get(14)?),
            damage: fight_projection
                .as_ref()
                .and_then(|projection| projection.damage)
                .unwrap_or(row.get(15)?),
            healing: fight_projection
                .as_ref()
                .and_then(|projection| projection.healing)
                .unwrap_or(row.get(16)?),
            mitigation: fight_projection
                .as_ref()
                .and_then(|projection| projection.mitigation)
                .unwrap_or(row.get(17)?),
            duration_sec: fight_projection
                .as_ref()
                .and_then(|projection| projection.duration_sec)
                .unwrap_or(row.get(18)?),
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

pub fn search_match_players(db: &OwDatabase, query: &str) -> Result<Vec<PlayerSearchOption>> {
    db.initialize()?;
    let needle = query.trim();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let conn = db.connect()?;
    let pattern = format!("%{}%", escape_like(needle));
    let mut stmt = conn.prepare(
        r#"
        SELECT mp.player_name, COUNT(DISTINCT mp.match_id) AS matches, MAX(m.begin_ts) AS latest_begin_ts
        FROM match_players mp
        JOIN matches m ON m.match_id = mp.match_id
        WHERE m.bucket = 'preset'
          AND TRIM(mp.player_name) <> ''
          AND mp.player_name LIKE ? ESCAPE '\'
        GROUP BY mp.player_name
        ORDER BY matches DESC, latest_begin_ts DESC, mp.player_name
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
        match_kind,
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
        String,
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
                matches.duration_sec,
                match_details.match_kind
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
                    row.get(12)?,
                ))
            },
        )
        .with_context(|| format!("未找到对局详情：{match_id}"))?;
    let entry: Value = serde_json::from_str(&entry_text)?;
    let detail: Value = serde_json::from_str(&detail_text)?;
    let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
    let hero_names = hero_lookup(&conn)?;
    let map_names = map_lookup(&conn)?;
    let arena_detail = if match_kind == "fight" {
        build_arena_detail(data, result.as_deref(), begin_ts, &hero_names, &map_names)
    } else {
        None
    };
    let teammates = if let Some(arena) = &arena_detail {
        arena.summary.teammates.clone()
    } else {
        player_rows(
            player_rows_for_side(data, "teammateList"),
            "teammate",
            &hero_names,
        )
    };
    let enemies = if let Some(arena) = &arena_detail {
        arena.summary.enemies.clone()
    } else {
        player_rows(
            player_rows_for_side(data, "enemyList"),
            "enemy",
            &hero_names,
        )
    };
    let team_awards = team_awards_from_match_players(&conn, match_id)?;
    let raw_json = if match_kind == "fight" {
        Value::Null
    } else {
        redact_sensitive_json(&json!({ "entry": entry, "detail": detail }))
    };
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
        arena_detail,
        evaluation: match_evaluation_summary(&conn, match_id)?,
        team_awards,
        same_role_comparison: same_role_comparison(&conn, match_id)?,
        raw_json,
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
            rank_label: None,
            rank_key: None,
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
            worth: None,
            end_worth: None,
            worth_max: false,
            mod_guids: Vec::new(),
            trait_guids: Vec::new(),
            has_source_max_flags: false,
            kill_max: false,
            hero_damage_max: false,
            cure_max: false,
            resist_damage_max: false,
            perks: Vec::new(),
            hero_stats: Vec::new(),
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
    push_award_if_highest(
        &mut awards,
        "kill",
        "击杀最多",
        "gold",
        current.kills,
        players.iter().map(|player| player.kills),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "damage",
        "伤害最高",
        "gold",
        current.damage,
        players.iter().map(|player| player.damage),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "healing",
        "治疗最高",
        "gold",
        current.healing,
        players.iter().map(|player| player.healing),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "mitigation",
        "格挡最多",
        "gold",
        current.mitigation,
        players.iter().map(|player| player.mitigation),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "death",
        "死亡最多",
        "danger",
        current.deaths,
        players.iter().map(|player| player.deaths),
        false,
    );
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
    if values
        .max()
        .map(|max| value >= max && max > 0)
        .unwrap_or(false)
    {
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
    if let Some(player_name) = non_empty(&filters.player_name) {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM match_players mp WHERE mp.match_id = m.match_id AND mp.player_name = ?)",
        );
        owned.push(Box::new(player_name));
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

fn last_round_map_guid(raw_detail_json: Option<&str>) -> Option<String> {
    let detail = raw_detail_json.and_then(|text| serde_json::from_str::<Value>(text).ok())?;
    detail
        .pointer("/payload/data/roundCountList")
        .and_then(Value::as_array)
        .and_then(|rounds| {
            rounds
                .iter()
                .rev()
                .find_map(|round| string_field(round, "mapGuid"))
        })
}

#[derive(Default)]
struct FightListProjection {
    team_score: Option<i64>,
    opponent_score: Option<i64>,
    kills: Option<i64>,
    assists: Option<i64>,
    deaths: Option<i64>,
    damage: Option<i64>,
    healing: Option<i64>,
    mitigation: Option<i64>,
    duration_sec: Option<i64>,
}

fn fight_list_projection(raw_detail_json: Option<&str>) -> Option<FightListProjection> {
    let detail = raw_detail_json.and_then(|text| serde_json::from_str::<Value>(text).ok())?;
    let last_round = detail
        .pointer("/payload/data/roundCountList")
        .and_then(Value::as_array)?
        .iter()
        .rev()
        .find(|round| round.is_object())?;
    let player = last_round
        .get("teammateList")
        .and_then(Value::as_array)
        .and_then(|players| players.first());
    Some(FightListProjection {
        team_score: optional_i64(last_round, "teamScore"),
        opponent_score: optional_i64(last_round, "opponentScore"),
        kills: player.and_then(|item| optional_i64(item, "kill")),
        assists: player.and_then(|item| optional_i64(item, "assist")),
        deaths: player.and_then(|item| optional_i64(item, "death")),
        damage: player.and_then(|item| optional_i64(item, "heroDamage")),
        healing: player.and_then(|item| optional_i64(item, "cure")),
        mitigation: player.and_then(|item| optional_i64(item, "resistDamage")),
        duration_sec: optional_i64(last_round, "gameTimeSec"),
    })
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
                        rank_label: None,
                        rank_key: None,
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
                        worth: None,
                        end_worth: None,
                        worth_max: false,
                        mod_guids: Vec::new(),
                        trait_guids: Vec::new(),
                        has_source_max_flags: false,
                        kill_max: false,
                        hero_damage_max: false,
                        cure_max: false,
                        resist_damage_max: false,
                        perks: parse_player_perks(item),
                        hero_stats: parse_hero_stats(item),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn player_rows_for_side<'a>(data: &'a Value, list_name: &str) -> Option<&'a Value> {
    data.pointer(&format!("/totalCount/{list_name}"))
        .filter(|value| {
            value
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        })
        .or_else(|| data.get(list_name))
}

fn build_arena_detail(
    data: &Value,
    match_result: Option<&str>,
    match_begin_ts: Option<i64>,
    hero_names: &HashMap<String, String>,
    map_names: &HashMap<String, String>,
) -> Option<ArenaMatchDetail> {
    let name_map = arena_name_map(data);
    let total = data.get("totalCount").unwrap_or(&Value::Null);
    let rounds_value = data.get("roundCountList");
    let summary_begin_ts = match_begin_ts
        .filter(|value| *value > 0)
        .or_else(|| first_round_start_time(rounds_value))
        .or_else(|| optional_i64(total, "startTime").map(normalize_timestamp));
    let summary_duration_sec =
        optional_i64(total, "gameTimeSec").or_else(|| round_duration_sum(rounds_value));
    let summary_source = last_arena_round(rounds_value).unwrap_or(total);
    let summary_map_guid = string_field(summary_source, "mapGuid")
        .or_else(|| string_field(total, "mapGuid"))
        .unwrap_or_else(|| "arena-victoriae".to_string());
    let summary = ArenaDetailSlice {
        key: "summary".to_string(),
        label: "角斗汇总".to_string(),
        result: match_result.map(str::to_string),
        result_label: match_result.map(result_label).map(str::to_string),
        team_score: optional_i64(summary_source, "teamScore")
            .or_else(|| optional_i64(total, "teamScore")),
        opponent_score: optional_i64(summary_source, "opponentScore")
            .or_else(|| optional_i64(total, "opponentScore")),
        duration_sec: summary_duration_sec,
        begin_ts: summary_begin_ts,
        begin_label: summary_begin_ts.map(format_begin_ts),
        map_name: Some(
            map_names
                .get(&summary_map_guid)
                .cloned()
                .unwrap_or_else(|| "未知地图".to_string()),
        ),
        map_guid: Some(summary_map_guid),
        teammates: arena_summary_player_rows(
            summary_source,
            total,
            rounds_value,
            "teammateList",
            "teammate",
            &name_map,
            hero_names,
        ),
        enemies: arena_summary_player_rows(
            summary_source,
            total,
            rounds_value,
            "enemyList",
            "enemy",
            &name_map,
            hero_names,
        ),
    };
    let rounds = rounds_value
        .and_then(Value::as_array)
        .map(|items| {
            let mut previous_team = 0;
            let mut previous_opponent = 0;
            items
                .iter()
                .enumerate()
                .map(|(index, round)| {
                    let team_score = optional_i64(round, "teamScore");
                    let opponent_score = optional_i64(round, "opponentScore");
                    let result = arena_round_result(
                        team_score,
                        opponent_score,
                        previous_team,
                        previous_opponent,
                    );
                    previous_team = team_score.unwrap_or(previous_team);
                    previous_opponent = opponent_score.unwrap_or(previous_opponent);
                    let begin_ts = optional_i64(round, "startTime").map(normalize_timestamp);
                    let map_guid = string_field(round, "mapGuid");
                    ArenaDetailSlice {
                        key: format!("round-{}", index + 1),
                        label: format!("回合 {}", index + 1),
                        result: result.map(str::to_string),
                        result_label: result.map(result_label).map(str::to_string),
                        team_score,
                        opponent_score,
                        duration_sec: optional_i64(round, "gameTimeSec"),
                        begin_ts,
                        begin_label: begin_ts.map(format_begin_ts),
                        map_name: map_guid.as_ref().map(|guid| {
                            map_names
                                .get(guid)
                                .cloned()
                                .unwrap_or_else(|| "未知地图".to_string())
                        }),
                        map_guid,
                        teammates: arena_player_rows(
                            round.get("teammateList"),
                            "teammate",
                            &name_map,
                            hero_names,
                        ),
                        enemies: arena_player_rows(
                            round.get("enemyList"),
                            "enemy",
                            &name_map,
                            hero_names,
                        ),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ArenaMatchDetail { summary, rounds })
}

fn last_arena_round(rounds_value: Option<&Value>) -> Option<&Value> {
    rounds_value
        .and_then(Value::as_array)
        .and_then(|items| items.last())
}

fn arena_summary_player_rows(
    summary_source: &Value,
    total: &Value,
    rounds_value: Option<&Value>,
    list_name: &str,
    side: &str,
    name_map: &HashMap<String, String>,
    hero_names: &HashMap<String, String>,
) -> Vec<PlayerRow> {
    let source_list = summary_source
        .get(list_name)
        .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()))
        .or_else(|| total.get(list_name));
    let mut rows = arena_player_rows(source_list, side, name_map, hero_names);
    let aggregates = arena_round_damage_healing_totals(rounds_value, list_name);
    if let Some(items) = source_list.and_then(Value::as_array) {
        for (row, item) in rows.iter_mut().zip(items) {
            let Some(bnet_id) = string_field(item, "bnetId") else {
                continue;
            };
            if let Some((damage_taken, healing_taken)) = aggregates.get(&bnet_id) {
                row.damage_taken = *damage_taken;
                row.healing_taken = *healing_taken;
            }
        }
    }
    rows
}

fn arena_round_damage_healing_totals(
    rounds_value: Option<&Value>,
    list_name: &str,
) -> HashMap<String, (i64, i64)> {
    let mut totals = HashMap::new();
    let Some(rounds) = rounds_value.and_then(Value::as_array) else {
        return totals;
    };
    for round in rounds {
        let Some(players) = round.get(list_name).and_then(Value::as_array) else {
            continue;
        };
        for player in players {
            let Some(bnet_id) = string_field(player, "bnetId") else {
                continue;
            };
            let entry = totals.entry(bnet_id).or_insert((0, 0));
            entry.0 += int_field(player, "damageTaken");
            entry.1 += int_field(player, "healingTaken");
        }
    }
    totals
}

fn arena_player_rows(
    value: Option<&Value>,
    side: &str,
    name_map: &HashMap<String, String>,
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
                    let bnet_id = string_field(item, "bnetId").unwrap_or_default();
                    Some(PlayerRow {
                        side: side.to_string(),
                        name: arena_player_name(item, &bnet_id, name_map),
                        bnet_id: String::new(),
                        rank_label: arena_rank_label(item),
                        rank_key: arena_rank_key(item),
                        hero_guid: hero_guid.clone(),
                        hero_name: hero_names
                            .get(&hero_guid)
                            .cloned()
                            .unwrap_or_else(|| "未知英雄".to_string()),
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
                        worth: optional_i64(item, "worth"),
                        end_worth: optional_i64(item, "endWorth"),
                        worth_max: bool_field(item, "worthMax"),
                        mod_guids: string_array_field(item, "modGuids"),
                        trait_guids: string_array_field(item, "traitGuids"),
                        has_source_max_flags: true,
                        kill_max: bool_field(item, "killMax"),
                        hero_damage_max: bool_field(item, "heroDamageMax"),
                        cure_max: bool_field(item, "cureMax"),
                        resist_damage_max: bool_field(item, "resistDamageMax"),
                        perks: Vec::new(),
                        hero_stats: parse_hero_stats(item),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn arena_name_map(data: &Value) -> HashMap<String, String> {
    data.get("nameMap")
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(key, value)| {
                    let name = value.as_str()?.trim();
                    if key.trim().is_empty() || name.is_empty() {
                        None
                    } else {
                        Some((key.to_string(), name.to_string()))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn arena_player_name(item: &Value, bnet_id: &str, name_map: &HashMap<String, String>) -> String {
    string_field(item, "name")
        .filter(|name| !looks_like_private_id(name))
        .or_else(|| {
            name_map
                .get(bnet_id)
                .filter(|name| !looks_like_private_id(name))
                .cloned()
        })
        .unwrap_or_else(|| "未知玩家".to_string())
}

fn first_round_start_time(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|round| optional_i64(round, "startTime"))
        .map(normalize_timestamp)
}

fn round_duration_sum(value: Option<&Value>) -> Option<i64> {
    let total = value
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|round| optional_i64(round, "gameTimeSec"))
        .sum::<i64>();
    if total > 0 {
        Some(total)
    } else {
        None
    }
}

fn arena_rank_label(item: &Value) -> Option<String> {
    let rank = item.get("rankInfo")?;
    let raw_name = string_field(rank, "rankName").or_else(|| string_field(rank, "rank_name"))?;
    let name = rank_name_label(&raw_name)?;
    let sub_tier =
        optional_i64(rank, "rankSubTier").or_else(|| optional_i64(rank, "rank_sub_tier"));
    Some(match sub_tier {
        Some(value) if value > 0 => format!("{name} {value}"),
        _ => name.to_string(),
    })
}

fn arena_rank_key(item: &Value) -> Option<String> {
    let rank = item.get("rankInfo")?;
    let raw_name = string_field(rank, "rankName").or_else(|| string_field(rank, "rank_name"))?;
    let key = normalize_rank_key(&raw_name)?;
    Some(key.to_string())
}

fn normalize_rank_key(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bronze" => Some("bronze"),
        "silver" => Some("silver"),
        "gold" => Some("gold"),
        "platinum" => Some("platinum"),
        "diamond" => Some("diamond"),
        "master" => Some("master"),
        "grandmaster" => Some("grandmaster"),
        "champion" => Some("champion"),
        "top 500" | "top500" => Some("top500"),
        _ => None,
    }
}

fn rank_name_label(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bronze" => Some("青铜"),
        "silver" => Some("白银"),
        "gold" => Some("黄金"),
        "platinum" => Some("白金"),
        "diamond" => Some("钻石"),
        "master" => Some("大师"),
        "grandmaster" => Some("宗师"),
        "champion" => Some("冠军"),
        "top 500" | "top500" => Some("五百强"),
        _ => None,
    }
}

fn bool_field(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(|item| {
            item.as_bool()
                .or_else(|| item.as_i64().map(|number| number != 0))
                .or_else(|| item.as_str().map(|text| matches!(text, "true" | "1")))
        })
        .unwrap_or(false)
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string)
                        .or_else(|| item.as_i64().map(|number| number.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn arena_round_result(
    team_score: Option<i64>,
    opponent_score: Option<i64>,
    previous_team: i64,
    previous_opponent: i64,
) -> Option<&'static str> {
    let team_delta = team_score.map(|score| score - previous_team);
    let opponent_delta = opponent_score.map(|score| score - previous_opponent);
    if team_delta.is_some_and(|delta| delta > 0) {
        Some("win")
    } else if opponent_delta.is_some_and(|delta| delta > 0) {
        Some("loss")
    } else {
        None
    }
}

fn optional_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|item| {
        item.as_i64()
            .or_else(|| item.as_f64().map(|value| value.round() as i64))
            .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
    })
}

fn normalize_timestamp(value: i64) -> i64 {
    if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    }
}

fn looks_like_private_id(value: &str) -> bool {
    value.chars().all(|item| item.is_ascii_digit()) && value.len() >= 4
}

fn parse_player_perks(item: &Value) -> Vec<PlayerPerk> {
    item.get("perks")
        .and_then(Value::as_array)
        .map(|perks| {
            perks
                .iter()
                .filter_map(|perk| {
                    let guid = string_field(perk, "guid")?;
                    Some(PlayerPerk {
                        guid,
                        perk_level: int_field(perk, "perkLevel"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_hero_stats(item: &Value) -> Vec<HeroStat> {
    item.get("statMap")
        .and_then(Value::as_object)
        .map(|stats| {
            let mut rows = stats
                .iter()
                .filter_map(|(guid, value)| {
                    stat_value(value).map(|number| HeroStat {
                        guid: guid.clone(),
                        value: number,
                    })
                })
                .collect::<Vec<_>>();
            rows.sort_by(|a, b| a.guid.cmp(&b.guid));
            rows
        })
        .unwrap_or_default()
}

fn stat_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|number| number as f64))
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
}

fn hero_lookup(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT guid, name FROM hero_dict")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(Into::into)
}

fn map_lookup(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT guid, name FROM map_dict")?;
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

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
