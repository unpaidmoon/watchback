use super::*;
use rusqlite::OptionalExtension;

#[derive(Debug, Clone, Default)]
pub(super) struct RoleAggregate {
    matches: i64,
    wins: i64,
    kills: i64,
    assists: i64,
    deaths: i64,
    latest_begin_ts: i64,
    rank_score: i64,
    rank_sub_tier: i64,
}

#[derive(Debug, Clone, Default)]
struct StadiumRoleAggregate {
    matches: i64,
    wins: i64,
    kills: i64,
    assists: i64,
    deaths: i64,
    latest_begin_ts: i64,
    rank_key: String,
    rank_sub_tier: i64,
}

pub(super) fn role_summaries(db: &OwDatabase, season: Option<i64>) -> Result<Vec<RoleRankSummary>> {
    let from_matches = role_summaries_from_matches(db, season)?;
    if from_matches.values().any(|item| item.matches > 0) {
        return Ok(build_role_summaries(from_matches));
    }

    role_summaries_from_match_players(db, season).map(build_role_summaries)
}

pub(super) fn stadium_role_summaries(
    db: &OwDatabase,
    season: Option<i64>,
) -> Result<Vec<RoleRankSummary>> {
    let conn = db.connect()?;
    let current_bnet = conn
        .query_row(
            "SELECT bnet_id FROM players WHERE TRIM(COALESCE(bnet_id, '')) <> '' ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let mut sql = String::from(
        r#"
        SELECT m.result, m.begin_ts, m.raw_entry_json, d.raw_detail_json
        FROM matches m
        LEFT JOIN match_details d ON d.match_id = m.match_id
        WHERE m.bucket = 'stadium' AND m.game_mode = 'SportFight'
        "#,
    );
    if season.is_some() {
        sql.push_str(" AND m.season = ?");
    }
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    };
    let rows = if let Some(value) = season {
        stmt.query_map(params![value], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map([], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let hero_roles = embedded_hero_roles()?;
    let mut by_role = HashMap::<String, StadiumRoleAggregate>::new();
    for (result, begin_ts, raw_entry_json, raw_detail_json) in rows {
        let entry = serde_json::from_str::<Value>(&raw_entry_json).unwrap_or(Value::Null);
        let detail = raw_detail_json
            .as_deref()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or(Value::Null);
        let player = current_bnet.as_deref().and_then(|current_bnet| {
            detail
                .pointer("/payload/data/totalCount/teammateList")
                .and_then(Value::as_array)
                .and_then(|players| {
                    players.iter().find(|player| {
                        player
                            .get("bnetId")
                            .and_then(value_as_string)
                            .map(|value| value == current_bnet)
                            .unwrap_or(false)
                    })
                })
        });
        let entry_role =
            normalize_role_type(entry.get("roleType").and_then(Value::as_str).unwrap_or(""));
        let role = if entry_role.is_empty() {
            player
                .and_then(|player| player.get("heroGuid"))
                .and_then(value_as_string)
                .and_then(|hero_guid| hero_roles.get(&hero_guid))
                .map(|role| normalize_role_type(role))
                .unwrap_or("")
        } else {
            entry_role
        };
        if !STADIUM_ROLE_ORDER.iter().any(|(key, _)| *key == role) {
            continue;
        }
        let item = by_role.entry(role.to_string()).or_default();
        item.matches += 1;
        item.wins += if result == "win" { 1 } else { 0 };

        let Some(player) = player else {
            continue;
        };
        item.kills += json_i64(player.get("kill"));
        item.assists += json_i64(player.get("assist"));
        item.deaths += json_i64(player.get("death"));
        if begin_ts >= item.latest_begin_ts {
            if let Some(rank_key) = player
                .get("rankInfo")
                .and_then(|rank| rank.get("rankName"))
                .and_then(Value::as_str)
                .and_then(stadium_rank_key)
            {
                item.latest_begin_ts = begin_ts;
                item.rank_key = rank_key.to_string();
                item.rank_sub_tier = json_i64(
                    player
                        .get("rankInfo")
                        .and_then(|rank| rank.get("rankSubTier")),
                );
            }
        }
    }

    Ok(build_stadium_role_summaries(by_role))
}

pub(super) fn role_summaries_from_matches(
    db: &OwDatabase,
    season: Option<i64>,
) -> Result<HashMap<String, RoleAggregate>> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT
            game_mode,
            result,
            kills,
            assists,
            deaths,
            begin_ts,
            raw_entry_json
        FROM matches
        WHERE bucket = 'preset'
        "#,
    );
    if season.is_some() {
        sql.push_str(" AND season = ?");
    }
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, String>(6)?,
        ))
    };
    let rows = if let Some(value) = season {
        stmt.query_map(params![value], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map([], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut by_role = HashMap::<String, RoleAggregate>::new();
    for (game_mode, result, kills, assists, deaths, begin_ts, raw_json) in rows {
        let parsed = serde_json::from_str::<Value>(&raw_json).unwrap_or(Value::Null);
        let role = role_type_from_match_player(&game_mode, &parsed, "");
        if role.is_empty() {
            continue;
        }
        add_role_match(
            &mut by_role,
            role,
            &result,
            kills,
            assists,
            deaths,
            begin_ts,
            &raw_json,
        );
    }
    Ok(by_role)
}

pub(super) fn role_summaries_from_match_players(
    db: &OwDatabase,
    season: Option<i64>,
) -> Result<HashMap<String, RoleAggregate>> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT
            COALESCE(mp.role_type, ''),
            m.game_mode,
            m.result,
            mp.kill,
            mp.assist,
            mp.death,
            m.begin_ts,
            mp.raw_json,
            m.raw_entry_json
        FROM match_players mp
        JOIN matches m ON m.match_id = mp.match_id
        WHERE mp.is_current_player = 1
          AND m.bucket = 'preset'
        "#,
    );
    if season.is_some() {
        sql.push_str(" AND m.season = ?");
    }
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
        ))
    };
    let rows = if let Some(value) = season {
        stmt.query_map(params![value], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map([], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut by_role = HashMap::<String, RoleAggregate>::new();
    for (raw_role, game_mode, result, kills, assists, deaths, begin_ts, raw_json, raw_entry_json) in
        rows
    {
        let raw_entry = serde_json::from_str::<Value>(&raw_entry_json).unwrap_or(Value::Null);
        let role = role_type_from_match_player(&game_mode, &raw_entry, &raw_role);
        if !ROLE_ORDER.iter().any(|(key, _)| *key == role) {
            continue;
        }
        add_role_match(
            &mut by_role,
            role,
            &result,
            kills,
            assists,
            deaths,
            begin_ts,
            &raw_json,
        );
    }

    Ok(by_role)
}

pub(super) fn build_role_summaries(
    by_role: HashMap<String, RoleAggregate>,
) -> Vec<RoleRankSummary> {
    ROLE_ORDER
        .iter()
        .map(|(role_type, role_label)| {
            let item = by_role.get(*role_type).cloned().unwrap_or_default();
            let rank_level = rank_level_for_score(item.rank_score);
            RoleRankSummary {
                role_type: (*role_type).to_string(),
                role_label: (*role_label).to_string(),
                matches: item.matches,
                wins: item.wins,
                win_rate: round2(ratio(item.wins, item.matches)),
                kda: round2((item.kills + item.assists) as f64 / item.deaths.max(1) as f64),
                rank_level,
                rank_sub_tier: if rank_level > 0 {
                    item.rank_sub_tier
                } else {
                    0
                },
                rank_key: rank_key_for_level(rank_level).to_string(),
            }
        })
        .collect()
}

fn build_stadium_role_summaries(
    by_role: HashMap<String, StadiumRoleAggregate>,
) -> Vec<RoleRankSummary> {
    STADIUM_ROLE_ORDER
        .iter()
        .map(|(role_type, role_label)| {
            let item = by_role.get(*role_type).cloned().unwrap_or_default();
            let rank_key = if item.rank_key.is_empty() {
                "unranked".to_string()
            } else {
                item.rank_key
            };
            RoleRankSummary {
                role_type: (*role_type).to_string(),
                role_label: (*role_label).to_string(),
                matches: item.matches,
                wins: item.wins,
                win_rate: round2(ratio(item.wins, item.matches)),
                kda: round2((item.kills + item.assists) as f64 / item.deaths.max(1) as f64),
                rank_level: stadium_rank_level(&rank_key),
                rank_sub_tier: if rank_key == "unranked" {
                    0
                } else {
                    item.rank_sub_tier
                },
                rank_key,
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)] // Aggregation inputs mirror the selected SQL columns.
pub(super) fn add_role_match(
    by_role: &mut HashMap<String, RoleAggregate>,
    role: &str,
    result: &str,
    kills: i64,
    assists: i64,
    deaths: i64,
    begin_ts: i64,
    raw_json: &str,
) {
    let item = by_role.entry(role.to_string()).or_default();
    item.matches += 1;
    item.wins += if result == "win" { 1 } else { 0 };
    item.kills += kills;
    item.assists += assists;
    item.deaths += deaths;
    let (rank_score, rank_sub_tier) = parse_rank_info(raw_json);
    if rank_score > 0 && begin_ts >= item.latest_begin_ts {
        item.latest_begin_ts = begin_ts;
        item.rank_score = rank_score;
        item.rank_sub_tier = rank_sub_tier;
    }
}

pub(super) fn role_type_from_match_player(
    game_mode: &str,
    raw_entry: &Value,
    fallback_role_type: &str,
) -> &'static str {
    let instance_type = raw_entry
        .get("instanceType")
        .and_then(Value::as_str)
        .unwrap_or("");
    if game_mode == "SportOpen" || (game_mode == "sport" && instance_type == "IT_RULESET") {
        return "open";
    }
    if game_mode == "SportPreset" || (game_mode == "sport" && instance_type == "IT_RANKED") {
        return normalize_role_type(
            raw_entry
                .get("roleType")
                .and_then(Value::as_str)
                .unwrap_or(fallback_role_type),
        );
    }
    ""
}

pub(crate) fn normalize_role_type(role_type: &str) -> &'static str {
    match role_type.trim().to_ascii_lowercase().as_str() {
        "tank" => "tank",
        "dps" | "damage" => "dps",
        "healer" | "support" => "healer",
        "open" => "open",
        _ => "",
    }
}

const STADIUM_ROLE_ORDER: [(&str, &str); 3] =
    [("tank", "重装"), ("dps", "输出"), ("healer", "支援")];

pub(crate) fn embedded_hero_roles() -> Result<HashMap<String, String>> {
    let lookup: Value =
        serde_json::from_str(include_str!("../../../runtime-config/ow-lookups.json"))
            .context("内置英雄职责映射无效")?;
    Ok(lookup
        .get("heroRoles")
        .and_then(Value::as_object)
        .map(|roles| {
            roles
                .iter()
                .filter_map(|(guid, role)| {
                    role.as_str()
                        .map(str::trim)
                        .filter(|role| !role.is_empty())
                        .map(|role| (guid.clone(), role.to_string()))
                })
                .collect()
        })
        .unwrap_or_default())
}

fn value_as_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.as_i64().map(|item| item.to_string()))
        .or_else(|| value.as_u64().map(|item| item.to_string()))
}

fn stadium_rank_key(rank_name: &str) -> Option<&'static str> {
    match rank_name.trim().to_ascii_lowercase().as_str() {
        "bronze" => Some("bronze"),
        "silver" => Some("silver"),
        "gold" => Some("gold"),
        "platinum" => Some("platinum"),
        "diamond" => Some("diamond"),
        "master" => Some("master"),
        "grandmaster" => Some("grandmaster"),
        _ => None,
    }
}

fn stadium_rank_level(rank_key: &str) -> i64 {
    match rank_key {
        "bronze" => 1,
        "silver" => 2,
        "gold" => 3,
        "platinum" => 4,
        "diamond" => 5,
        "master" => 6,
        "grandmaster" => 7,
        _ => 0,
    }
}

pub(super) fn parse_rank_info(raw_json: &str) -> (i64, i64) {
    let parsed = serde_json::from_str::<Value>(raw_json).unwrap_or(Value::Null);
    let rank_info = parsed.get("rankInfo").unwrap_or(&Value::Null);
    (
        json_i64(rank_info.get("rankScore")),
        json_i64(rank_info.get("rankSubTier")),
    )
}

pub(super) fn json_i64(value: Option<&Value>) -> i64 {
    value
        .and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_f64().map(|value| value.round() as i64))
                .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
        })
        .unwrap_or(0)
}

pub(super) fn rank_level_for_score(rank_score: i64) -> i64 {
    if rank_score <= 0 {
        0
    } else {
        ((rank_score / 100) + 1).clamp(1, 8)
    }
}

pub(super) fn rank_key_for_level(rank_level: i64) -> &'static str {
    match rank_level {
        1 => "bronze",
        2 => "silver",
        3 => "gold",
        4 => "platinum",
        5 => "diamond",
        6 => "master",
        7 => "grandmaster",
        8 => "champion",
        _ => "unranked",
    }
}

pub(super) fn phase5_highlights(
    db: &OwDatabase,
    season: Option<i64>,
    streaks: &StreakSummary,
) -> Result<Phase5ReviewHighlights> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT e.primary_tag, e.grade, e.evidence_tags_json
        FROM match_evaluations e
        JOIN matches m ON m.match_id = e.match_id
        WHERE m.bucket='preset'
        "#,
    );
    let mut rows = if let Some(season) = season {
        sql.push_str(" AND m.season=?");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![season], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    } else {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };

    let carry_count = rows.iter().filter(|row| row.0 == "带飞局").count() as i64;
    let try_hard_count = rows.iter().filter(|row| row.0 == "尽力局").count() as i64;
    let s_grade_count = rows.iter().filter(|row| row.1 == "S").count() as i64;
    let mut highlight_counts = HashMap::<String, i64>::new();
    for (_, _, evidence_text) in rows.drain(..) {
        for tag in serde_json::from_str::<Vec<String>>(&evidence_text).unwrap_or_default() {
            if matches!(
                tag.as_str(),
                "KDA最高" | "击杀最多" | "伤害最高" | "治疗最高" | "格挡最多"
            ) {
                *highlight_counts.entry(tag).or_default() += 1;
            }
        }
    }
    let most_common_highlight = highlight_counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|item| item.0);

    Ok(Phase5ReviewHighlights {
        carry_count,
        try_hard_count,
        s_grade_count,
        longest_win_streak: streaks.longest_win_streak,
        most_common_highlight,
    })
}
