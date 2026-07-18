use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Tank,
    Damage,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeFamily {
    RoleCompetitive,
    RoleQuick,
    OpenCompetitive,
    OpenQuick,
    StadiumCompetitive,
    StadiumQuick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingCohort {
    RoleQueue,
    OpenQueue,
    StadiumCompetitive,
    OutcomeOnly,
}

impl ModeFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoleCompetitive => "role_competitive",
            Self::RoleQuick => "role_quick",
            Self::OpenCompetitive => "open_competitive",
            Self::OpenQuick => "open_quick",
            Self::StadiumCompetitive => "stadium_competitive",
            Self::StadiumQuick => "stadium_quick",
        }
    }

    pub fn from_stored_str(value: &str) -> Option<Self> {
        match value {
            "role_competitive" => Some(Self::RoleCompetitive),
            "role_quick" | "quick_play" => Some(Self::RoleQuick),
            "open_competitive" => Some(Self::OpenCompetitive),
            "open_quick" => Some(Self::OpenQuick),
            "stadium_competitive" => Some(Self::StadiumCompetitive),
            "stadium_quick" => Some(Self::StadiumQuick),
            _ => None,
        }
    }

    pub fn training_cohort(self) -> TrainingCohort {
        match self {
            Self::RoleCompetitive | Self::RoleQuick => TrainingCohort::RoleQueue,
            Self::OpenCompetitive | Self::OpenQuick => TrainingCohort::OpenQueue,
            Self::StadiumCompetitive => TrainingCohort::StadiumCompetitive,
            Self::StadiumQuick => TrainingCohort::OutcomeOnly,
        }
    }

    pub fn trains_performance(self) -> bool {
        self != Self::StadiumQuick
    }

    pub fn is_quick(self) -> bool {
        matches!(self, Self::RoleQuick | Self::OpenQuick | Self::StadiumQuick)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StadiumStatShape {
    RoundLocal,
    Cumulative,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StadiumFinalSnapshot {
    pub role: Role,
    pub hero_guid: String,
    pub statistics: Value,
}

pub struct ExtractionContext {
    summary_roles: HashMap<String, Role>,
    hero_roles: HashMap<String, Role>,
}

impl ExtractionContext {
    pub fn build(conn: &Connection) -> Result<Self> {
        let summary_roles = summary_role_mapping(conn)?;
        let mut hero_roles = summary_roles.clone();
        let mut statement =
            conn.prepare("SELECT guid, role FROM hero_dict WHERE role IS NOT NULL")?;
        for row in statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })? {
            let (hero, role) = row?;
            if let Some(role) = normalize_role(Some(&role)) {
                hero_roles.insert(hero, role);
            }
        }
        Ok(Self {
            summary_roles,
            hero_roles,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MmrMatchInput {
    pub match_id: String,
    pub season: i64,
    pub begin_ts: i64,
    pub mode_family: ModeFamily,
    pub result: f64,
    pub duration_sec: i64,
    pub role: Role,
    pub role_share: f64,
    pub role_source: &'static str,
    pub data_quality: f64,
    pub team_strength: Option<f64>,
    pub opponent_strength: Option<f64>,
    pub rank_coverage: f64,
    pub performance_features: Value,
}

pub fn normalize_mode_family(game_mode: &str, instance_type: Option<&str>) -> Option<ModeFamily> {
    match game_mode.trim().to_ascii_lowercase().as_str() {
        "sportpreset" => Some(ModeFamily::RoleCompetitive),
        "sportopen" => Some(ModeFamily::OpenCompetitive),
        "leisurepreset" | "leisure" => Some(ModeFamily::RoleQuick),
        "leisure6v6" => Some(ModeFamily::OpenQuick),
        "sportfight" => Some(ModeFamily::StadiumCompetitive),
        "leisurefight" | "quickfight" => Some(ModeFamily::StadiumQuick),
        "sport" => match instance_type?.trim().to_ascii_lowercase().as_str() {
            "it_ranked" => Some(ModeFamily::RoleCompetitive),
            "it_ruleset" => Some(ModeFamily::OpenCompetitive),
            _ => None,
        },
        _ => None,
    }
}

pub fn extract_ordinary_match(conn: &Connection, match_id: &str) -> Result<Option<MmrMatchInput>> {
    let context = ExtractionContext::build(conn)?;
    extract_ordinary_match_with_context(conn, match_id, &context)
}

pub fn extract_ordinary_match_with_context(
    conn: &Connection,
    match_id: &str,
    context: &ExtractionContext,
) -> Result<Option<MmrMatchInput>> {
    let row = conn.query_row(
        "SELECT match_id, season, begin_ts, game_mode, result, duration_sec, hero_guid, raw_entry_json FROM matches WHERE match_id=?1",
        [match_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, i64>(5)?, row.get::<_, String>(6)?, row.get::<_, String>(7)?)),
    ).optional()?;
    let Some((match_id, season, begin_ts, game_mode, result, duration_sec, hero_guid, raw)) = row
    else {
        return Ok(None);
    };
    let parsed = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
    let instance_type = parsed.get("instanceType").and_then(Value::as_str);
    let Some(mode_family) = normalize_mode_family(&game_mode, instance_type) else {
        return Ok(None);
    };
    let result = match result.trim().to_ascii_lowercase().as_str() {
        "win" => 1.0,
        "loss" => 0.0,
        "draw" => 0.5,
        _ => return Ok(None),
    };

    let entry_role = parsed
        .get("roleType")
        .and_then(Value::as_str)
        .and_then(|value| normalize_role(Some(value)));
    let (role, role_source, data_quality) = if let Some(role) = entry_role {
        (role, "entry_role", 1.0)
    } else {
        let Some(role) = context
            .summary_roles
            .get(&hero_guid)
            .or_else(|| context.hero_roles.get(&hero_guid))
            .copied()
        else {
            return Ok(None);
        };
        (role, "final_hero", 0.55)
    };
    let (team_strength, opponent_strength, rank_coverage) = ordinary_strengths(conn, &match_id)?;

    Ok(Some(MmrMatchInput {
        match_id,
        season,
        begin_ts,
        mode_family,
        result,
        duration_sec,
        role,
        role_share: 1.0,
        role_source,
        data_quality,
        team_strength,
        opponent_strength,
        rank_coverage,
        performance_features: Value::Object(Default::default()),
    }))
}

pub fn extract_stadium_match(conn: &Connection, match_id: &str) -> Result<Option<MmrMatchInput>> {
    let context = ExtractionContext::build(conn)?;
    extract_stadium_match_with_context(conn, match_id, &context)
}

pub fn extract_stadium_match_with_context(
    conn: &Connection,
    match_id: &str,
    context: &ExtractionContext,
) -> Result<Option<MmrMatchInput>> {
    let row = conn
        .query_row(
            r#"
            SELECT m.match_id, m.season, m.begin_ts, m.game_mode, m.result, m.duration_sec,
                   m.raw_entry_json, d.raw_detail_json
            FROM matches m
            LEFT JOIN match_details d ON d.match_id = m.match_id
            WHERE m.match_id=?1 AND m.bucket='stadium'
            "#,
            [match_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((match_id, season, begin_ts, game_mode, result, duration_sec, raw_entry, raw_detail)) =
        row
    else {
        return Ok(None);
    };
    let Some(result) = normalize_result(&result) else {
        return Ok(None);
    };
    let entry = serde_json::from_str::<Value>(&raw_entry).unwrap_or(Value::Null);
    let Some(mode_family) = normalize_mode_family(
        &game_mode,
        entry.get("instanceType").and_then(Value::as_str),
    ) else {
        return Ok(None);
    };
    let detail = raw_detail
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or(Value::Null);
    let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
    let rounds = data
        .get("roundCountList")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let Some(current_player_key) = unique_current_player_key(conn, &match_id)? else {
        return Ok(None);
    };
    let entry_role = entry
        .get("roleType")
        .and_then(Value::as_str)
        .and_then(|value| normalize_role(Some(value)));
    let (team_strength, opponent_strength, rank_coverage) = stadium_strengths(data, &entry);

    let selected = stadium_final_snapshot(rounds, &current_player_key, context);
    let (role, role_source, data_quality, performance_features) = if let Some(snapshot) = selected {
        (
            snapshot.role,
            "stadium_last_round",
            0.9,
            snapshot.statistics,
        )
    } else {
        let Some(role) = entry_role else {
            return Ok(None);
        };
        return Ok(Some(stadium_outcome_only(
            match_id,
            season,
            begin_ts,
            result,
            duration_sec,
            mode_family,
            role,
            team_strength,
            opponent_strength,
            rank_coverage,
        )));
    };

    Ok(Some(MmrMatchInput {
        match_id,
        season,
        begin_ts,
        mode_family,
        result,
        duration_sec,
        role,
        role_share: 1.0,
        role_source,
        data_quality,
        team_strength,
        opponent_strength,
        rank_coverage,
        performance_features,
    }))
}

fn stadium_outcome_only(
    match_id: String,
    season: i64,
    begin_ts: i64,
    result: f64,
    duration_sec: i64,
    mode_family: ModeFamily,
    role: Role,
    team_strength: Option<f64>,
    opponent_strength: Option<f64>,
    rank_coverage: f64,
) -> MmrMatchInput {
    MmrMatchInput {
        match_id,
        season,
        begin_ts,
        mode_family,
        result,
        duration_sec,
        role,
        role_share: 1.0,
        role_source: "entry_role",
        data_quality: 0.35,
        team_strength,
        opponent_strength,
        rank_coverage,
        performance_features: Value::Object(Default::default()),
    }
}

fn normalize_result(result: &str) -> Option<f64> {
    match result.trim().to_ascii_lowercase().as_str() {
        "win" => Some(1.0),
        "loss" => Some(0.0),
        "draw" => Some(0.5),
        _ => None,
    }
}

fn unique_current_player_key(conn: &Connection, match_id: &str) -> Result<Option<String>> {
    Ok(conn.query_row(
        "SELECT CASE WHEN COUNT(*)=1 AND MIN(bnet_id)=TRIM(MIN(bnet_id)) AND TRIM(MIN(bnet_id))<>'' THEN MIN(bnet_id) END FROM match_players WHERE match_id=?1 AND is_current_player=1",
        [match_id],
        |row| row.get(0),
    )?)
}

fn last_structurally_valid_stadium_round(rounds: &[Value]) -> Option<&Value> {
    ordered_structurally_valid_stadium_rounds(rounds).pop()
}

pub(crate) fn stadium_final_snapshot(
    rounds: &[Value],
    current_player_key: &str,
    context: &ExtractionContext,
) -> Option<StadiumFinalSnapshot> {
    let player = current_player_row(
        last_structurally_valid_stadium_round(rounds)?,
        current_player_key,
    )?;
    let hero_guid = scalar_text(player.get("heroGuid"))?;
    let role = context.hero_roles.get(&hero_guid).copied()?;
    Some(StadiumFinalSnapshot {
        role,
        hero_guid,
        statistics: stadium_performance_features(player),
    })
}

fn structurally_valid_stadium_round(round: &Value) -> bool {
    round.is_object()
        && ["teammateList", "enemyList"]
            .into_iter()
            .any(|list| round.get(list).is_some_and(Value::is_array))
}

fn current_player_row<'a>(round: &'a Value, current_key: &str) -> Option<&'a Value> {
    ["teammateList", "enemyList"]
        .into_iter()
        .filter_map(|list| round.get(list).and_then(Value::as_array))
        .flatten()
        .find(|player| {
            scalar_text(player.get("bnetId")).as_deref() == Some(current_key)
                && scalar_text(player.get("heroGuid")).is_some()
        })
}

fn scalar_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let text = match value {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

fn json_i64_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(json_i64_value)
}

const STADIUM_PERFORMANCE_FIELDS: &[&str] = &[
    "kill",
    "assist",
    "death",
    "heroDamage",
    "cure",
    "resistDamage",
    "finalHit",
    "targetCompetingTime",
    "healingTaken",
    "damageTaken",
];

fn stadium_performance_features(player: &Value) -> Value {
    let mut features = serde_json::Map::new();
    for key in STADIUM_PERFORMANCE_FIELDS {
        if let Some(value) = player.get(*key) {
            if value.is_number() {
                features.insert((*key).to_string(), value.clone());
            } else if let Some(number) = json_number(value) {
                features.insert((*key).to_string(), Value::from(number));
            }
        }
    }
    Value::Object(features)
}

pub fn classify_stadium_stat_shape(rounds: &[Value], current_key: &str) -> StadiumStatShape {
    let players = ordered_structurally_valid_stadium_rounds(rounds)
        .into_iter()
        .filter_map(|round| current_player_row(round, current_key))
        .collect::<Vec<_>>();
    if players.len() < 2 {
        return StadiumStatShape::Ambiguous;
    }
    let mut cumulative_signal = false;
    let mut local_signal = false;
    for key in STADIUM_PERFORMANCE_FIELDS {
        let values = players
            .iter()
            .filter_map(|player| player.get(*key).and_then(json_number))
            .collect::<Vec<_>>();
        if values.len() < 2 {
            continue;
        }
        if values.windows(2).all(|pair| pair[1] >= pair[0]) {
            cumulative_signal |= values.windows(2).any(|pair| pair[1] > pair[0]);
        } else {
            local_signal = true;
        }
    }
    match (local_signal, cumulative_signal) {
        (true, false) => StadiumStatShape::RoundLocal,
        (false, true) => StadiumStatShape::Cumulative,
        _ => StadiumStatShape::Ambiguous,
    }
}

fn ordered_structurally_valid_stadium_rounds(rounds: &[Value]) -> Vec<&Value> {
    let mut ordered = rounds
        .iter()
        .enumerate()
        .filter(|(_, round)| structurally_valid_stadium_round(round))
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(position, round)| {
        if let Some(index) = json_i64_field(round, "roundIndex") {
            (2_i8, index, *position as i64)
        } else if let Some(start_time) = json_i64_field(round, "startTime") {
            (1_i8, start_time, *position as i64)
        } else {
            (0_i8, *position as i64, *position as i64)
        }
    });
    ordered.into_iter().map(|(_, round)| round).collect()
}

fn stadium_strengths(data: &Value, entry: &Value) -> (Option<f64>, Option<f64>, f64) {
    let total = data.get("totalCount").unwrap_or(&Value::Null);
    let team = stadium_side_strengths(
        total
            .get("teammateList")
            .or_else(|| entry.get("teammateList")),
    );
    let opponent =
        stadium_side_strengths(total.get("enemyList").or_else(|| entry.get("enemyList")));
    let total_players = team.1 + opponent.1;
    let ranked = team.2 + opponent.2;
    let coverage = if total_players == 0 {
        0.0
    } else {
        ranked as f64 / total_players as f64
    };
    if coverage < 0.8 || team.0.is_empty() || opponent.0.is_empty() {
        (None, None, coverage)
    } else {
        (Some(mean(&team.0)), Some(mean(&opponent.0)), coverage)
    }
}

fn stadium_side_strengths(value: Option<&Value>) -> (Vec<f64>, u64, u64) {
    let Some(players) = value.and_then(Value::as_array) else {
        return (Vec::new(), 0, 0);
    };
    let values = players
        .iter()
        .filter_map(|player| player.get("rankInfo").and_then(rank_strength))
        .collect::<Vec<_>>();
    let ranked = values.len() as u64;
    (values, players.len() as u64, ranked)
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tank => "tank",
            Self::Damage => "damage",
            Self::Support => "support",
        }
    }
}

pub fn normalize_role(value: Option<&str>) -> Option<Role> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "tank" => Some(Role::Tank),
        "dps" | "damage" => Some(Role::Damage),
        "healer" | "support" => Some(Role::Support),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrInputAudit {
    pub matches: i64,
    pub ordinary_matches: i64,
    pub stadium_matches: i64,
    pub player_rows: i64,
    pub mapped_player_rows: i64,
    pub unmapped_player_rows: i64,
    pub final_hit_rows: i64,
    pub healing_taken_rows: i64,
    pub stadium_round_local_matches: i64,
    pub stadium_cumulative_matches: i64,
    pub stadium_ambiguous_matches: i64,
    pub discovered_seasons: Vec<i64>,
}

pub fn audit_career_mmr_inputs(conn: &Connection) -> Result<CareerMmrInputAudit> {
    let (matches, ordinary_matches, stadium_matches) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN bucket = 'stadium' THEN 0 ELSE 1 END), 0), COALESCE(SUM(CASE WHEN bucket = 'stadium' THEN 1 ELSE 0 END), 0) FROM matches",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    let mut discovered_seasons = Vec::new();
    let mut seasons = conn.prepare("SELECT DISTINCT season FROM matches ORDER BY season")?;
    for season in seasons.query_map([], |row| row.get(0))? {
        discovered_seasons.push(season?);
    }

    let summary_roles = summary_role_mapping(conn)?;
    let mut player_rows = 0;
    let mut mapped_player_rows = 0;
    let mut final_hit_rows = 0;
    let mut healing_taken_rows = 0;
    let mut players = conn.prepare(
        "SELECT p.hero_guid, p.role_type, d.role, p.final_hit, p.healing_taken FROM match_players p LEFT JOIN hero_dict d ON d.guid = p.hero_guid",
    )?;
    let rows = players.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;
    for row in rows {
        let (hero, explicit_role, dictionary_role, final_hit, healing_taken) = row?;
        player_rows += 1;
        let mapped = normalize_role(explicit_role.as_deref())
            .or_else(|| normalize_role(dictionary_role.as_deref()))
            .or_else(|| summary_roles.get(&hero).copied());
        if mapped.is_some() {
            mapped_player_rows += 1;
        }
        if final_hit > 0 {
            final_hit_rows += 1;
        }
        if healing_taken > 0 {
            healing_taken_rows += 1;
        }
    }

    let mut stadium_round_local_matches = 0;
    let mut stadium_cumulative_matches = 0;
    let mut stadium_ambiguous_matches = 0;
    let mut stadium = conn.prepare(
        r#"
        SELECT d.raw_detail_json, p.bnet_id
        FROM matches m
        LEFT JOIN match_details d ON d.match_id=m.match_id
        LEFT JOIN match_players p ON p.match_id=m.match_id AND p.is_current_player=1
        WHERE m.bucket='stadium'
        GROUP BY m.match_id
        "#,
    )?;
    for row in stadium.query_map([], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
        ))
    })? {
        let (raw_detail, current_key) = row?;
        let shape = raw_detail
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|detail| {
                let rounds = detail.pointer("/payload/data/roundCountList")?.as_array()?;
                Some(classify_stadium_stat_shape(
                    rounds,
                    current_key.as_deref().unwrap_or_default(),
                ))
            })
            .unwrap_or(StadiumStatShape::Ambiguous);
        match shape {
            StadiumStatShape::RoundLocal => stadium_round_local_matches += 1,
            StadiumStatShape::Cumulative => stadium_cumulative_matches += 1,
            StadiumStatShape::Ambiguous => stadium_ambiguous_matches += 1,
        }
    }

    Ok(CareerMmrInputAudit {
        matches,
        ordinary_matches,
        stadium_matches,
        player_rows,
        mapped_player_rows,
        unmapped_player_rows: player_rows - mapped_player_rows,
        final_hit_rows,
        healing_taken_rows,
        stadium_round_local_matches,
        stadium_cumulative_matches,
        stadium_ambiguous_matches,
        discovered_seasons,
    })
}

fn ordinary_strengths(
    conn: &Connection,
    match_id: &str,
) -> Result<(Option<f64>, Option<f64>, f64)> {
    let mut rows =
        conn.prepare("SELECT side, raw_json FROM match_players WHERE match_id=?1 ORDER BY side")?;
    let rows = rows.query_map([match_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut total = 0_u64;
    let mut ranked = 0_u64;
    let mut team = Vec::new();
    let mut opponent = Vec::new();
    let mut ranked_unknown_side = false;
    for row in rows {
        let (side, raw) = row?;
        total += 1;
        let parsed = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
        let rank_info = parsed.get("rankInfo").or_else(|| parsed.get("rank_info"));
        let normalized_side = side.trim().to_ascii_lowercase();
        if rank_info.is_some()
            && !matches!(
                normalized_side.as_str(),
                "teammate" | "team" | "enemy" | "opponent"
            )
        {
            ranked_unknown_side = true;
        }
        let score = rank_info.and_then(rank_strength);
        let Some(score) = score else { continue };
        ranked += 1;
        match normalized_side.as_str() {
            "teammate" | "team" => team.push(score),
            "enemy" | "opponent" => opponent.push(score),
            _ => ranked_unknown_side = true,
        }
    }
    let coverage = if total == 0 {
        0.0
    } else {
        ranked as f64 / total as f64
    };
    if coverage < 0.8 || team.is_empty() || opponent.is_empty() || ranked_unknown_side {
        return Ok((None, None, coverage));
    }
    Ok((Some(mean(&team)), Some(mean(&opponent)), coverage))
}

fn json_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .filter(|value| value.is_finite())
}

fn rank_strength(rank_info: &Value) -> Option<f64> {
    if let Some(score) = rank_info
        .get("rankScore")
        .or_else(|| rank_info.get("rank_score"))
        .and_then(json_number)
        .filter(|score| *score > 0.0)
    {
        return Some(score);
    }

    let name = rank_info
        .get("rankName")
        .or_else(|| rank_info.get("rank_name"))?
        .as_str()?
        .trim()
        .to_ascii_lowercase();
    let level = match name.as_str() {
        "bronze" => 0.0,
        "silver" => 100.0,
        "gold" => 200.0,
        "platinum" => 300.0,
        "diamond" => 400.0,
        "master" => 500.0,
        "grandmaster" => 600.0,
        "champion" => 700.0,
        "top 500" | "top500" => 800.0,
        _ => return None,
    };
    let sub_tier = rank_info
        .get("rankSubTier")
        .or_else(|| rank_info.get("rank_sub_tier"))
        .and_then(json_i64_value)?;
    if !(1..=5).contains(&sub_tier) {
        return None;
    }
    Some(level + (6 - sub_tier) as f64 * 20.0)
}

fn json_i64_value(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| {
            value
                .as_f64()
                .filter(|number| number.fract() == 0.0)
                .map(|number| number as i64)
        })
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn summary_role_mapping(conn: &Connection) -> Result<HashMap<String, Role>> {
    let mut candidates: HashMap<String, HashSet<Role>> = HashMap::new();
    let mut statement = conn.prepare("SELECT raw_entry_json FROM matches")?;
    for raw in statement.query_map([], |row| row.get::<_, String>(0))? {
        if let Ok(value) = serde_json::from_str::<Value>(&raw?) {
            collect_summary_role(&value, &mut candidates);
        }
    }
    Ok(candidates
        .into_iter()
        .filter_map(|(hero, roles)| {
            if roles.len() == 1 {
                roles.into_iter().next().map(|role| (hero, role))
            } else {
                None
            }
        })
        .collect())
}

fn collect_summary_role(value: &Value, candidates: &mut HashMap<String, HashSet<Role>>) {
    let Some(object) = value.as_object() else {
        return;
    };
    let hero = object.get("heroGuid").and_then(Value::as_str);
    let role = object
        .get("roleType")
        .and_then(Value::as_str)
        .and_then(|value| normalize_role(Some(value)));
    if let (Some(hero), Some(role)) = (hero, role) {
        candidates.entry(hero.to_owned()).or_default().insert(role);
    }
}
