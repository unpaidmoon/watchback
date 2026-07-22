use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::knowledge::KnowledgeContext;
use super::task::TaskKnowledgeContext;
use crate::models::{MatchDetailView, PlayerRow};
use crate::review::roles::{embedded_hero_roles, normalize_role_type};

pub const INPUT_SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastSnapshot {
    pub schema_version: i64,
    pub match_kind: RoastMatchKind,
    #[serde(rename = "match")]
    pub match_context: RoastMatchContext,
    pub current_player: String,
    pub scoreboard: RoastScoreboard,
    pub role_matchups: Vec<RoleMatchup>,
    pub current_player_baseline: Option<CurrentPlayerBaseline>,
    pub streak: Option<RoastStreak>,
    pub game_knowledge: KnowledgeContext,
    pub task_knowledge: TaskKnowledgeContext,
    pub arena: Option<ArenaContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoastMatchKind {
    Standard,
    Arena,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoastMatchContext {
    pub result: Option<String>,
    pub mode: Option<String>,
    pub map: Option<String>,
    pub team_score: Option<i64>,
    pub opponent_score: Option<i64>,
    pub duration_seconds: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastScoreboard {
    pub team: Vec<RoastPlayer>,
    pub opponent: Vec<RoastPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastPlayer {
    pub subject: String,
    pub is_current_player: bool,
    pub hero: String,
    pub role: Option<String>,
    pub stats: PlayerStats,
    pub derived: DerivedMetrics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub kills: Option<i64>,
    pub assists: Option<i64>,
    pub deaths: Option<i64>,
    pub damage: Option<i64>,
    pub healing: Option<i64>,
    pub mitigation: Option<i64>,
    pub final_hits: Option<i64>,
    pub objective_time_seconds: Option<i64>,
    pub healing_received: Option<i64>,
    pub damage_taken: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DerivedMetrics {
    pub team_kills_rank: Option<i64>,
    pub match_kills_rank: Option<i64>,
    pub team_assists_rank: Option<i64>,
    pub match_assists_rank: Option<i64>,
    pub team_deaths_rank: Option<i64>,
    pub match_deaths_rank: Option<i64>,
    pub team_damage_rank: Option<i64>,
    pub match_damage_rank: Option<i64>,
    pub team_healing_rank: Option<i64>,
    pub match_healing_rank: Option<i64>,
    pub team_mitigation_rank: Option<i64>,
    pub match_mitigation_rank: Option<i64>,
    pub team_final_hits_rank: Option<i64>,
    pub match_final_hits_rank: Option<i64>,
    pub team_objective_time_rank: Option<i64>,
    pub match_objective_time_rank: Option<i64>,
    pub team_healing_received_rank: Option<i64>,
    pub match_healing_received_rank: Option<i64>,
    pub team_damage_taken_rank: Option<i64>,
    pub match_damage_taken_rank: Option<i64>,
    pub kill_participation_rate: Option<f64>,
    pub team_damage_share: Option<f64>,
    pub team_healing_share: Option<f64>,
    pub team_death_share: Option<f64>,
    pub team_damage_taken_share: Option<f64>,
    pub team_healing_received_share: Option<f64>,
    pub damage_per_death: Option<f64>,
    pub healing_per_death: Option<f64>,
    pub kill_participation_per_death: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleMatchup {
    pub role: String,
    pub team_subjects: Vec<String>,
    pub opponent_subjects: Vec<String>,
    pub team_totals: PlayerStats,
    pub opponent_totals: PlayerStats,
    pub deltas: PlayerStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentPlayerBaseline {
    pub role: Option<String>,
    pub averages: PlayerStats,
    pub difference_rates: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoastStreak {
    pub result: Option<String>,
    pub count_before_match: Option<i64>,
    pub continued: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArenaContext {
    pub round_results: Vec<ArenaRoundResult>,
    pub final_worth: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArenaRoundResult {
    pub round: i64,
    pub result: String,
    pub score_after: String,
}

#[derive(Debug, Clone)]
struct RowIdentity {
    role: Option<String>,
    current: bool,
}

pub fn build_roast_snapshot(
    conn: &Connection,
    match_id: &str,
    detail: &MatchDetailView,
    game_knowledge: KnowledgeContext,
) -> Result<RoastSnapshot> {
    let mut identities = load_identities(conn, match_id)?;
    let hero_roles = embedded_hero_roles()?;
    augment_current_identity_from_detail(&mut identities, detail, &hero_roles);
    let (match_kind, team_source, opponent_source, arena) =
        if let Some(arena_detail) = &detail.arena_detail {
            let selected = arena_detail
                .rounds
                .iter()
                .rev()
                .find(|round| {
                    !round.teammates.is_empty()
                        && !round.enemies.is_empty()
                        && round.team_score.is_some_and(|score| score >= 0)
                        && round.opponent_score.is_some_and(|score| score >= 0)
                        && round
                            .teammates
                            .iter()
                            .filter(|row| {
                                identity_for_row(&identities, "teammate", row)
                                    .is_some_and(|item| item.current)
                            })
                            .count()
                            == 1
                })
                .ok_or_else(|| anyhow!("当前对局详情尚未完整同步"))?;
            let preview_team = map_side(
                "我方",
                "teammate",
                &selected.teammates,
                &identities,
                &hero_roles,
            );
            let preview_opponent =
                map_side("敌方", "enemy", &selected.enemies, &identities, &hero_roles);
            let mut final_worth = BTreeMap::new();
            for (source, mapped) in selected.teammates.iter().zip(&preview_team) {
                if let Some(value) = source.end_worth {
                    final_worth.insert(mapped.subject.clone(), value);
                }
            }
            for (source, mapped) in selected.enemies.iter().zip(&preview_opponent) {
                if let Some(value) = source.end_worth {
                    final_worth.insert(mapped.subject.clone(), value);
                }
            }
            let round_results = arena_detail
                .rounds
                .iter()
                .enumerate()
                .filter_map(|(index, round)| {
                    Some(ArenaRoundResult {
                        round: index as i64 + 1,
                        result: round.result.clone()?,
                        score_after: format!("{}:{}", round.team_score?, round.opponent_score?),
                    })
                })
                .collect();
            (
                RoastMatchKind::Arena,
                selected.teammates.as_slice(),
                selected.enemies.as_slice(),
                Some(ArenaContext {
                    round_results,
                    final_worth,
                }),
            )
        } else {
            (
                RoastMatchKind::Standard,
                detail.teammates.as_slice(),
                detail.enemies.as_slice(),
                None,
            )
        };
    let mut team = map_side("我方", "teammate", team_source, &identities, &hero_roles);
    let mut opponent = map_side("敌方", "enemy", opponent_source, &identities, &hero_roles);
    let current_count = team.iter().filter(|row| row.is_current_player).count();
    if current_count != 1 {
        return Err(anyhow!("当前对局无法唯一识别当前玩家"));
    }
    calculate_derived(&mut team, &mut opponent);
    let current_player = team
        .iter()
        .find(|row| row.is_current_player)
        .map(|row| row.subject.clone())
        .ok_or_else(|| anyhow!("当前对局无法唯一识别当前玩家"))?;
    let current = team.iter().find(|row| row.is_current_player).unwrap();
    let season = match_season(conn, match_id)?;
    let current_player_baseline = current
        .role
        .as_deref()
        .map(|role| baseline(conn, season, role, current))
        .transpose()?
        .flatten();
    let streak = if match_kind == RoastMatchKind::Standard {
        build_streak(conn, match_id, season, detail.result.as_deref())?
    } else {
        None
    };
    let role_matchups = build_role_matchups(&team, &opponent);

    Ok(RoastSnapshot {
        schema_version: INPUT_SCHEMA_VERSION,
        match_kind,
        match_context: RoastMatchContext {
            result: detail.result.clone(),
            mode: detail
                .mode_label
                .clone()
                .or_else(|| detail.game_mode.clone()),
            map: detail.map_name.clone(),
            team_score: detail.team_score,
            opponent_score: detail.opponent_score,
            duration_seconds: detail.duration_sec,
        },
        current_player,
        scoreboard: RoastScoreboard { team, opponent },
        role_matchups,
        current_player_baseline,
        streak,
        game_knowledge,
        task_knowledge: TaskKnowledgeContext::default(),
        arena,
    })
}

fn augment_current_identity_from_detail(
    identities: &mut HashMap<(String, String), RowIdentity>,
    detail: &MatchDetailView,
    hero_roles: &HashMap<String, String>,
) {
    let Some(current_hero_guid) = detail.hero_guid.as_deref() else {
        return;
    };
    let candidates = detail
        .teammates
        .iter()
        .filter(|row| row.hero_guid == current_hero_guid)
        .collect::<Vec<_>>();
    if candidates.len() != 1 || candidates[0].name.trim().is_empty() {
        return;
    }
    let current = candidates[0];
    if detail
        .teammates
        .iter()
        .filter(|row| row.name == current.name)
        .count()
        == 1
    {
        identities.insert(
            ("teammate".to_string(), format!("name:{}", current.name)),
            RowIdentity {
                role: hero_roles
                    .get(current_hero_guid)
                    .and_then(|value| canonical_role(value)),
                current: true,
            },
        );
    }
}

fn load_identities(
    conn: &Connection,
    match_id: &str,
) -> Result<HashMap<(String, String), RowIdentity>> {
    let mut hero_counts = HashMap::<(String, String), usize>::new();
    let mut name_counts = HashMap::<(String, String), usize>::new();
    let mut hero_stmt =
        conn.prepare("SELECT side,hero_guid FROM match_players WHERE match_id=? ORDER BY rowid")?;
    for row in hero_stmt.query_map(params![match_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        *hero_counts.entry(row?).or_default() += 1;
    }
    let mut hero_rows = conn.prepare("SELECT side,bnet_id,hero_guid,role_type,is_current_player,player_name FROM match_players WHERE match_id=? ORDER BY rowid")?;
    let rows = hero_rows
        .query_map(params![match_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)? == 1,
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (side, _, _, _, _, name) in &rows {
        if !name.trim().is_empty() {
            *name_counts.entry((side.clone(), name.clone())).or_default() += 1;
        }
    }
    let mut identities = HashMap::new();
    for (side, id, hero_guid, role, current, player_name) in rows {
        let identity = RowIdentity {
            role: role.and_then(|value| canonical_role(&value)),
            current,
        };
        identities.insert((side.clone(), id), identity.clone());
        if hero_counts.get(&(side.clone(), hero_guid.clone())) == Some(&1) {
            identities.insert(
                (side.clone(), format!("hero:{hero_guid}")),
                identity.clone(),
            );
        }
        if name_counts.get(&(side.clone(), player_name.clone())) == Some(&1) {
            identities.insert((side, format!("name:{player_name}")), identity);
        }
    }
    let current_bnet_ids = identities
        .iter()
        .filter_map(|((_, key), identity)| {
            (identity.current && !key.starts_with("hero:") && !key.starts_with("name:"))
                .then_some(key.clone())
        })
        .collect::<std::collections::HashSet<_>>();
    let mut arena_stmt = conn.prepare(
        "SELECT side,bnet_id,player_name FROM arena_match_players WHERE match_id=? ORDER BY rowid",
    )?;
    let arena_rows = arena_stmt
        .query_map(params![match_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut arena_name_counts = HashMap::<(String, String), usize>::new();
    for (side, _, name) in &arena_rows {
        if !name.trim().is_empty() {
            *arena_name_counts
                .entry((side.clone(), name.clone()))
                .or_default() += 1;
        }
    }
    for (side, bnet_id, player_name) in arena_rows {
        if arena_name_counts.get(&(side.clone(), player_name.clone())) == Some(&1) {
            identities.insert(
                (side, format!("name:{player_name}")),
                RowIdentity {
                    role: None,
                    current: current_bnet_ids.contains(&bnet_id),
                },
            );
        }
    }
    Ok(identities)
}

fn canonical_role(value: &str) -> Option<String> {
    match normalize_role_type(value) {
        "tank" => Some("tank".to_string()),
        "dps" => Some("damage".to_string()),
        "healer" => Some("support".to_string()),
        _ => None,
    }
}

fn map_side(
    prefix: &str,
    db_side: &str,
    rows: &[PlayerRow],
    identities: &HashMap<(String, String), RowIdentity>,
    hero_roles: &HashMap<String, String>,
) -> Vec<RoastPlayer> {
    let mut counts = HashMap::<&str, usize>::new();
    for row in rows {
        *counts.entry(row.hero_name.as_str()).or_default() += 1;
    }
    let mut ordinals = HashMap::<&str, usize>::new();
    rows.iter()
        .map(|row| {
            let ordinal = ordinals.entry(row.hero_name.as_str()).or_default();
            *ordinal += 1;
            let suffix = if counts[row.hero_name.as_str()] > 1 {
                chinese_ordinal(*ordinal)
            } else {
                ""
            };
            let identity = identity_for_row(identities, db_side, row);
            let role = identity.and_then(|item| item.role.clone()).or_else(|| {
                hero_roles
                    .get(&row.hero_guid)
                    .and_then(|value| canonical_role(value))
            });
            RoastPlayer {
                subject: format!("{prefix}{}{suffix}", stable_hero_name(&row.hero_name)),
                is_current_player: identity.is_some_and(|item| item.current),
                hero: stable_hero_name(&row.hero_name),
                role,
                stats: stats(row),
                derived: DerivedMetrics::default(),
            }
        })
        .collect()
}

fn identity_for_row<'a>(
    identities: &'a HashMap<(String, String), RowIdentity>,
    db_side: &str,
    row: &PlayerRow,
) -> Option<&'a RowIdentity> {
    identities
        .get(&(db_side.to_string(), row.bnet_id.clone()))
        .or_else(|| identities.get(&(db_side.to_string(), format!("hero:{}", row.hero_guid))))
        .or_else(|| identities.get(&(db_side.to_string(), format!("name:{}", row.name))))
}

fn stable_hero_name(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "未知英雄".to_string()
    } else {
        value.to_string()
    }
}

fn chinese_ordinal(value: usize) -> &'static str {
    match value {
        1 => "一号",
        2 => "二号",
        3 => "三号",
        4 => "四号",
        _ => "五号",
    }
}

fn stats(row: &PlayerRow) -> PlayerStats {
    PlayerStats {
        kills: Some(row.kills),
        assists: Some(row.assists),
        deaths: Some(row.deaths),
        damage: Some(row.damage),
        healing: Some(row.healing),
        mitigation: Some(row.mitigation),
        final_hits: Some(row.final_hits),
        objective_time_seconds: Some(row.target_time_sec),
        healing_received: Some(row.healing_taken),
        damage_taken: Some(row.damage_taken),
    }
}

fn calculate_derived(team: &mut [RoastPlayer], opponent: &mut [RoastPlayer]) {
    let all = team
        .iter()
        .chain(opponent.iter())
        .map(|row| row.stats.clone())
        .collect::<Vec<_>>();
    let team_stats = team.iter().map(|row| row.stats.clone()).collect::<Vec<_>>();
    let team_kills = sum(&team_stats, |s| s.kills);
    let damage_total = sum(&team_stats, |s| s.damage);
    let healing_total = sum(&team_stats, |s| s.healing);
    let death_total = sum(&team_stats, |s| s.deaths);
    let taken_total = sum(&team_stats, |s| s.damage_taken);
    let received_total = sum(&team_stats, |s| s.healing_received);
    for row in team.iter_mut() {
        fill_derived(
            row,
            &team_stats,
            &all,
            team_kills,
            damage_total,
            healing_total,
            death_total,
            taken_total,
            received_total,
        );
    }
    let opponent_stats = opponent
        .iter()
        .map(|row| row.stats.clone())
        .collect::<Vec<_>>();
    let opponent_kills = sum(&opponent_stats, |s| s.kills);
    let opponent_damage = sum(&opponent_stats, |s| s.damage);
    let opponent_healing = sum(&opponent_stats, |s| s.healing);
    let opponent_deaths = sum(&opponent_stats, |s| s.deaths);
    let opponent_taken = sum(&opponent_stats, |s| s.damage_taken);
    let opponent_received = sum(&opponent_stats, |s| s.healing_received);
    for row in opponent.iter_mut() {
        fill_derived(
            row,
            &opponent_stats,
            &all,
            opponent_kills,
            opponent_damage,
            opponent_healing,
            opponent_deaths,
            opponent_taken,
            opponent_received,
        );
    }
}

fn sum(stats: &[PlayerStats], get: fn(&PlayerStats) -> Option<i64>) -> i64 {
    stats.iter().filter_map(get).sum()
}

fn competition_rank(
    value: Option<i64>,
    rows: &[PlayerStats],
    get: fn(&PlayerStats) -> Option<i64>,
    ascending: bool,
) -> Option<i64> {
    let value = value?;
    Some(
        1 + rows
            .iter()
            .filter_map(get)
            .filter(|other| {
                if ascending {
                    *other < value
                } else {
                    *other > value
                }
            })
            .count() as i64,
    )
}

fn share(value: Option<i64>, total: i64) -> Option<f64> {
    if total > 0 {
        value.map(|value| value as f64 / total as f64)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_derived(
    row: &mut RoastPlayer,
    team: &[PlayerStats],
    all: &[PlayerStats],
    team_kills: i64,
    damage: i64,
    healing: i64,
    deaths: i64,
    taken: i64,
    received: i64,
) {
    let s = &row.stats;
    let mut d = DerivedMetrics::default();
    macro_rules! ranks {
        ($team:ident,$match_:ident,$field:ident,$asc:expr) => {{
            d.$team = competition_rank(s.$field, team, |v| v.$field, $asc);
            d.$match_ = competition_rank(s.$field, all, |v| v.$field, $asc);
        }};
    }
    ranks!(team_kills_rank, match_kills_rank, kills, false);
    ranks!(team_assists_rank, match_assists_rank, assists, false);
    ranks!(team_deaths_rank, match_deaths_rank, deaths, true);
    ranks!(team_damage_rank, match_damage_rank, damage, false);
    ranks!(team_healing_rank, match_healing_rank, healing, false);
    ranks!(
        team_mitigation_rank,
        match_mitigation_rank,
        mitigation,
        false
    );
    ranks!(
        team_final_hits_rank,
        match_final_hits_rank,
        final_hits,
        false
    );
    ranks!(
        team_objective_time_rank,
        match_objective_time_rank,
        objective_time_seconds,
        false
    );
    ranks!(
        team_healing_received_rank,
        match_healing_received_rank,
        healing_received,
        false
    );
    ranks!(
        team_damage_taken_rank,
        match_damage_taken_rank,
        damage_taken,
        false
    );
    let participation = s.kills.zip(s.assists).and_then(|(kills, assists)| {
        let numerator = kills + assists;
        (team_kills > 0 && numerator <= team_kills).then(|| numerator as f64 / team_kills as f64)
    });
    d.kill_participation_rate = participation;
    d.team_damage_share = share(s.damage, damage);
    d.team_healing_share = share(s.healing, healing);
    d.team_death_share = share(s.deaths, deaths);
    d.team_damage_taken_share = share(s.damage_taken, taken);
    d.team_healing_received_share = share(s.healing_received, received);
    if let Some(death) = s.deaths.filter(|value| *value > 0) {
        d.damage_per_death = s.damage.map(|value| value as f64 / death as f64);
        d.healing_per_death = s.healing.map(|value| value as f64 / death as f64);
        d.kill_participation_per_death = participation.map(|value| value / death as f64);
    }
    row.derived = d;
}

fn build_role_matchups(team: &[RoastPlayer], opponent: &[RoastPlayer]) -> Vec<RoleMatchup> {
    ["tank", "damage", "support"]
        .into_iter()
        .filter_map(|role| {
            let team_rows = team
                .iter()
                .filter(|row| row.role.as_deref() == Some(role))
                .collect::<Vec<_>>();
            let opponent_rows = opponent
                .iter()
                .filter(|row| row.role.as_deref() == Some(role))
                .collect::<Vec<_>>();
            if team_rows.is_empty() && opponent_rows.is_empty() {
                return None;
            }
            let team_totals = totals(&team_rows);
            let opponent_totals = totals(&opponent_rows);
            Some(RoleMatchup {
                role: role.to_string(),
                team_subjects: team_rows.iter().map(|row| row.subject.clone()).collect(),
                opponent_subjects: opponent_rows
                    .iter()
                    .map(|row| row.subject.clone())
                    .collect(),
                deltas: subtract(&team_totals, &opponent_totals),
                team_totals,
                opponent_totals,
            })
        })
        .collect()
}

fn totals(rows: &[&RoastPlayer]) -> PlayerStats {
    let values = rows.iter().map(|row| row.stats.clone()).collect::<Vec<_>>();
    PlayerStats {
        kills: Some(sum(&values, |s| s.kills)),
        assists: Some(sum(&values, |s| s.assists)),
        deaths: Some(sum(&values, |s| s.deaths)),
        damage: Some(sum(&values, |s| s.damage)),
        healing: Some(sum(&values, |s| s.healing)),
        mitigation: Some(sum(&values, |s| s.mitigation)),
        final_hits: Some(sum(&values, |s| s.final_hits)),
        objective_time_seconds: Some(sum(&values, |s| s.objective_time_seconds)),
        healing_received: Some(sum(&values, |s| s.healing_received)),
        damage_taken: Some(sum(&values, |s| s.damage_taken)),
    }
}

fn subtract(left: &PlayerStats, right: &PlayerStats) -> PlayerStats {
    fn diff(a: Option<i64>, b: Option<i64>) -> Option<i64> {
        a.zip(b).map(|(a, b)| a - b)
    }
    PlayerStats {
        kills: diff(left.kills, right.kills),
        assists: diff(left.assists, right.assists),
        deaths: diff(left.deaths, right.deaths),
        damage: diff(left.damage, right.damage),
        healing: diff(left.healing, right.healing),
        mitigation: diff(left.mitigation, right.mitigation),
        final_hits: diff(left.final_hits, right.final_hits),
        objective_time_seconds: diff(left.objective_time_seconds, right.objective_time_seconds),
        healing_received: diff(left.healing_received, right.healing_received),
        damage_taken: diff(left.damage_taken, right.damage_taken),
    }
}

fn match_season(conn: &Connection, match_id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT season FROM matches WHERE match_id=?",
        params![match_id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn baseline(
    conn: &Connection,
    season: i64,
    role: &str,
    current: &RoastPlayer,
) -> Result<Option<CurrentPlayerBaseline>> {
    let roles: &[&str] = match role {
        "tank" => &["tank"],
        "damage" => &["dps", "damage"],
        "support" => &["healer", "support"],
        _ => return Ok(None),
    };
    let sql = format!("SELECT AVG(mp.kill),AVG(mp.death),AVG(mp.hero_damage),AVG(mp.cure) FROM match_players mp JOIN matches m ON m.match_id=mp.match_id WHERE m.season=? AND mp.is_current_player=1 AND LOWER(mp.role_type) IN ({}) AND m.match_kind='normal'", std::iter::repeat_n("?",roles.len()).collect::<Vec<_>>().join(","));
    let mut params_values: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(season)];
    params_values.extend(
        roles
            .iter()
            .map(|role| Box::new((*role).to_string()) as Box<dyn rusqlite::ToSql>),
    );
    let refs = params_values
        .iter()
        .map(|value| value.as_ref())
        .collect::<Vec<_>>();
    let averages = conn.query_row(&sql, refs.as_slice(), |row| {
        Ok((
            row.get::<_, Option<f64>>(0)?,
            row.get::<_, Option<f64>>(1)?,
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<f64>>(3)?,
        ))
    })?;
    let (Some(kills), Some(deaths), Some(damage), Some(healing)) = averages else {
        return Ok(None);
    };
    let avg = PlayerStats {
        kills: Some(kills.round() as i64),
        deaths: Some(deaths.round() as i64),
        damage: Some(damage.round() as i64),
        healing: Some(healing.round() as i64),
        ..Default::default()
    };
    let mut difference_rates = BTreeMap::new();
    for (name, value, base) in [
        ("kills", current.stats.kills, kills),
        ("deaths", current.stats.deaths, deaths),
        ("damage", current.stats.damage, damage),
        ("healing", current.stats.healing, healing),
    ] {
        if base != 0.0 {
            if let Some(value) = value {
                difference_rates.insert(name.to_string(), (value as f64 - base) / base);
            }
        }
    }
    Ok(Some(CurrentPlayerBaseline {
        role: Some(role.to_string()),
        averages: avg,
        difference_rates,
    }))
}

fn build_streak(
    conn: &Connection,
    match_id: &str,
    season: i64,
    current_result: Option<&str>,
) -> Result<Option<RoastStreak>> {
    let Some(current_result) = current_result else {
        return Ok(None);
    };
    let mut stmt=conn.prepare("SELECT match_id,result FROM matches WHERE season=? AND match_kind='normal' ORDER BY begin_ts,match_id")?;
    let rows = stmt
        .query_map(params![season], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let Some(index) = rows.iter().position(|(id, _)| id == match_id) else {
        return Ok(None);
    };
    let previous = index
        .checked_sub(1)
        .and_then(|i| rows.get(i))
        .map(|(_, result)| result.as_str());
    let count = previous
        .filter(|value| *value != "draw")
        .map(|value| {
            rows[..index]
                .iter()
                .rev()
                .take_while(|(_, result)| result == value)
                .count() as i64
        })
        .unwrap_or(0);
    Ok(Some(RoastStreak {
        result: Some(current_result.to_string()),
        count_before_match: Some(count),
        continued: Some(previous == Some(current_result) && current_result != "draw"),
    }))
}
