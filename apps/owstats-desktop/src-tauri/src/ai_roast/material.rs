use crate::match_evaluation::{evaluation_for_match_conn, PHASE5_SCORING_RULE_VERSION};
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

pub const MATERIAL_VERSION: i64 = 1;

const SAFE_PRIMARY_TAGS: &[&str] = &[
    "\u{5e26}\u{98de}\u{5c40}",
    "\u{8eba}\u{8d62}\u{5c40}",
    "\u{5c3d}\u{529b}\u{5c40}",
    "\u{80cc}\u{9505}\u{5c40}",
    "\u{987a}\u{98ce}\u{5c40}",
    "\u{60dc}\u{8d25}\u{5c40}",
    "\u{80dc}\u{5229}\u{5c40}",
    "\u{5931}\u{8d25}\u{5c40}",
];
const SAFE_EVIDENCE_TAGS: &[&str] = &[
    "KDA\u{6700}\u{9ad8}",
    "\u{51fb}\u{6740}\u{6700}\u{591a}",
    "\u{4f24}\u{5bb3}\u{6700}\u{9ad8}",
    "\u{6cbb}\u{7597}\u{6700}\u{9ad8}",
    "\u{683c}\u{6321}\u{6700}\u{591a}",
    "\u{7ffb}\u{76d8}\u{5c40}",
    "\u{4f2f}\u{4ef2}\u{5c40}",
];

const OUTPUT_HEAVY_SUPPORTS: &[&str] = &[
    "207165582859042848",
    "207165582859043234",
    "207165582859043612",
];
const HEALING_HEAVY_SUPPORTS: &[&str] = &[
    "207165582859042820",
    "207165582859043131",
    "207165582859043473",
];
const HYBRID_SUPPORTS: &[&str] = &[
    "207165582859042937",
    "207165582859043221",
    "207165582859043361",
    "207165582859043377",
    "207165582859043685",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastMaterial {
    pub material_version: i64,
    pub evaluation: RoastEvaluation,
    pub relationship: RoastRelationship,
    pub streak: RoastStreakContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastEvaluation {
    pub rule_version: i64,
    pub primary_tag: String,
    pub evidence_tags: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastFact {
    pub id: String,
    pub text: String,
    pub strength: f64,
    pub evidence: RoastFactEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoastFactEvidence {
    Rank,
    SameRole,
    Contribution,
    StatComparison,
    Result,
    Availability,
    Counterpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoastRelationshipKind {
    WeakLink,
    Carry,
    Scapegoat,
    SelfRoast,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastRelationship {
    pub kind: RoastRelationshipKind,
    pub subject: String,
    pub facts: Vec<RoastFact>,
    pub counter_facts: Vec<RoastFact>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoastStreakContext {
    pub before_current: usize,
    pub previous_event: Option<String>,
    pub current_event: String,
}

#[derive(Debug, Clone)]
struct MatchContext {
    season: i64,
    result: String,
    begin_ts: i64,
}

#[derive(Debug, Clone)]
struct PlayerMetrics {
    rowid: i64,
    hero_guid: String,
    role: Role,
    current: bool,
    kills: i64,
    assists: i64,
    deaths: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
    subject: String,
    composite: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Role {
    Tank,
    Damage,
    Support,
}

#[derive(Debug, Clone)]
struct FactCandidate {
    fact: RoastFact,
    metric: &'static str,
    score: f64,
}

#[derive(Debug, Clone, Copy)]
struct SeasonAverage {
    kills: f64,
    deaths: f64,
    damage: f64,
    healing: f64,
}

pub fn build_roast_material(conn: &Connection, match_id: &str) -> Result<RoastMaterial> {
    let context = match_context(conn, match_id)?;
    let evaluation = evaluation_for_match_conn(conn, match_id)?
        .map(|value| safe_evaluation(value, &context.result))
        .unwrap_or_else(|| neutral_evaluation(&context.result));
    let players = teammate_metrics(conn, match_id)?;
    let relationship = select_relationship(conn, &context, &evaluation, &players)?;
    let material = RoastMaterial {
        material_version: MATERIAL_VERSION,
        evaluation,
        relationship,
        streak: streak_context(conn, match_id, &context)?,
    };
    assert_material_privacy(&material)?;
    Ok(material)
}

fn safe_evaluation(
    value: crate::match_evaluation::MatchEvaluation,
    result: &str,
) -> RoastEvaluation {
    if !SAFE_PRIMARY_TAGS.contains(&value.primary_tag.as_str()) {
        return neutral_evaluation(result);
    }
    RoastEvaluation {
        rule_version: value.rule_version,
        primary_tag: value.primary_tag,
        evidence_tags: value
            .evidence_tags
            .into_iter()
            .filter(|tag| SAFE_EVIDENCE_TAGS.contains(&tag.as_str()))
            .collect(),
        confidence: if value.confidence.is_finite() {
            value.confidence.clamp(0.0, 1.0)
        } else {
            0.5
        },
    }
}

fn match_context(conn: &Connection, match_id: &str) -> Result<MatchContext> {
    conn.query_row(
        "SELECT season, result, begin_ts FROM matches WHERE match_id=? AND match_kind='normal'",
        params![match_id],
        |row| {
            Ok(MatchContext {
                season: row.get(0)?,
                result: row.get(1)?,
                begin_ts: row.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| anyhow!("未找到普通对局材料"))
}

fn neutral_evaluation(result: &str) -> RoastEvaluation {
    RoastEvaluation {
        rule_version: PHASE5_SCORING_RULE_VERSION,
        primary_tag: match result {
            "win" => "胜利局",
            "loss" => "失败局",
            "draw" => "平局",
            _ => "结果未知",
        }
        .to_string(),
        evidence_tags: Vec::new(),
        confidence: 0.5,
    }
}

fn teammate_metrics(conn: &Connection, match_id: &str) -> Result<Vec<PlayerMetrics>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT rowid, hero_guid, role_type, is_current_player, kill, assist, death,
               hero_damage, cure, resist_damage
        FROM match_players
        WHERE match_id=? AND side='teammate'
        ORDER BY rowid ASC
        "#,
    )?;
    let rows = stmt.query_map(params![match_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)? == 1,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, i64>(9)?,
        ))
    })?;
    let mut role_slots = HashMap::new();
    let mut players = Vec::new();
    for row in rows {
        let (
            rowid,
            hero_guid,
            raw_role,
            current,
            kills,
            assists,
            deaths,
            damage,
            healing,
            mitigation,
        ) = row?;
        let Some(role) = Role::from_raw(raw_role.as_deref()) else {
            continue;
        };
        if [kills, assists, deaths, damage, healing, mitigation]
            .iter()
            .any(|value| *value < 0)
        {
            continue;
        }
        let slot = role_slots.entry(role).or_insert(0usize);
        *slot += 1;
        let subject = if current {
            format!("当前玩家（{}位{}号）", role.label(), slot)
        } else {
            format!("我方{}位{}号", role.label(), slot)
        };
        let mut player = PlayerMetrics {
            rowid,
            hero_guid,
            role,
            current,
            kills,
            assists,
            deaths,
            damage,
            healing,
            mitigation,
            subject,
            composite: 0.0,
        };
        player.composite = player.composite_score();
        players.push(player);
    }
    Ok(players)
}

impl Role {
    fn from_raw(value: Option<&str>) -> Option<Self> {
        match value {
            Some("tank") => Some(Self::Tank),
            Some("dps") | Some("damage") => Some(Self::Damage),
            Some("healer") | Some("support") => Some(Self::Support),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Tank => "坦克",
            Self::Damage => "输出",
            Self::Support => "支援",
        }
    }
}

impl PlayerMetrics {
    fn composite_score(&self) -> f64 {
        let death_penalty = self.deaths as f64 * 650.0;
        match self.role {
            Role::Tank => {
                self.damage as f64 * 0.30
                    + self.mitigation as f64 * 0.45
                    + self.kills as f64 * 300.0
                    + self.assists as f64 * 120.0
                    - death_penalty
            }
            Role::Damage => {
                self.damage as f64 * 0.70 + self.kills as f64 * 500.0 + self.assists as f64 * 100.0
                    - death_penalty
            }
            Role::Support if OUTPUT_HEAVY_SUPPORTS.contains(&self.hero_guid.as_str()) => {
                self.damage as f64 * 0.35
                    + self.healing as f64 * 0.35
                    + self.kills as f64 * 220.0
                    + self.assists as f64 * 180.0
                    - death_penalty
            }
            Role::Support if HEALING_HEAVY_SUPPORTS.contains(&self.hero_guid.as_str()) => {
                self.damage as f64 * 0.05 + self.healing as f64 * 0.70 + self.assists as f64 * 190.0
                    - death_penalty
            }
            Role::Support if HYBRID_SUPPORTS.contains(&self.hero_guid.as_str()) => {
                self.damage as f64 * 0.25
                    + self.healing as f64 * 0.45
                    + self.kills as f64 * 160.0
                    + self.assists as f64 * 180.0
                    - death_penalty
            }
            Role::Support => {
                self.damage as f64 * 0.15
                    + self.healing as f64 * 0.55
                    + self.kills as f64 * 150.0
                    + self.assists as f64 * 180.0
                    - death_penalty
            }
        }
    }
}

fn select_relationship(
    conn: &Connection,
    context: &MatchContext,
    evaluation: &RoastEvaluation,
    players: &[PlayerMetrics],
) -> Result<RoastRelationship> {
    if players.len() < 2 {
        return Ok(neutral_relationship(&context.result));
    }
    let current = players.iter().find(|player| player.current);
    let weakest = ranked(players, false).into_iter().next();
    let strongest = ranked(players, true).into_iter().next();
    let (kind, subject) = if current
        .is_some_and(|player| weakest.is_some_and(|weakest| weakest.rowid == player.rowid))
    {
        (RoastRelationshipKind::SelfRoast, current)
    } else if evaluation.primary_tag == "躺赢局" {
        (
            RoastRelationshipKind::Carry,
            strongest.filter(|player| !player.current),
        )
    } else if context.result == "loss" {
        (
            RoastRelationshipKind::Scapegoat,
            weakest.filter(|player| !player.current),
        )
    } else if context.result == "win" {
        (
            RoastRelationshipKind::WeakLink,
            weakest.filter(|player| !player.current),
        )
    } else {
        return Ok(neutral_relationship(&context.result));
    };
    let Some(subject) = subject else {
        return Ok(neutral_relationship(&context.result));
    };
    let facts = select_facts(fact_candidates(
        conn, context, evaluation, players, subject, &kind,
    )?);
    if facts.len() < 3
        || !facts.iter().any(|fact| {
            matches!(
                fact.evidence,
                RoastFactEvidence::Rank | RoastFactEvidence::SameRole
            )
        })
    {
        return Ok(neutral_relationship(&context.result));
    }
    let counter_facts = if kind == RoastRelationshipKind::Scapegoat {
        counter_facts(players, subject)
    } else {
        Vec::new()
    };
    let confidence =
        (facts.iter().map(|fact| fact.strength).sum::<f64>() / facts.len() as f64).clamp(0.0, 1.0);
    Ok(RoastRelationship {
        kind,
        subject: subject.subject.clone(),
        facts,
        counter_facts,
        confidence,
    })
}

fn ranked(players: &[PlayerMetrics], descending: bool) -> Vec<&PlayerMetrics> {
    let mut ranked = players.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        let order = left
            .composite
            .partial_cmp(&right.composite)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.rowid.cmp(&right.rowid));
        if descending {
            order.reverse()
        } else {
            order
        }
    });
    ranked
}

fn fact_candidates(
    conn: &Connection,
    context: &MatchContext,
    evaluation: &RoastEvaluation,
    players: &[PlayerMetrics],
    subject: &PlayerMetrics,
    kind: &RoastRelationshipKind,
) -> Result<Vec<FactCandidate>> {
    let mut facts = Vec::new();
    let team_rank = ranked(players, true)
        .iter()
        .position(|player| player.rowid == subject.rowid)
        .unwrap_or(players.len() - 1)
        + 1;
    let rank_text = match team_rank {
        1 => "综合表现位列全队第1".to_string(),
        rank if rank == players.len() => format!("综合表现位列全队第{rank}"),
        rank => format!("综合表现位列全队第{rank}"),
    };
    push_fact(
        &mut facts,
        "team_rank_composite",
        rank_text,
        RoastFactEvidence::Rank,
        "composite_rank",
        0.98,
        rank_contrast(team_rank, players.len()),
        evaluation,
    );

    let role_peers = players
        .iter()
        .filter(|player| player.role == subject.role)
        .collect::<Vec<_>>();
    if role_peers.len() >= 2 {
        let role_metric = primary_role_metric(subject.role, subject);
        let peer_average = role_peers
            .iter()
            .filter(|player| player.rowid != subject.rowid)
            .map(|player| primary_role_metric(subject.role, player))
            .sum::<f64>()
            / (role_peers.len() - 1) as f64;
        let delta = role_metric - peer_average;
        let metric_name = primary_role_metric_name(subject.role);
        let direction = if delta >= 0.0 { "高" } else { "低" };
        push_fact(
            &mut facts,
            "same_role_primary_delta",
            format!(
                "{metric_name}比同位置队友平均{direction}{}",
                format_metric(delta.abs())
            ),
            RoastFactEvidence::SameRole,
            "role_primary",
            0.90,
            relative_contrast(role_metric, peer_average),
            evaluation,
        );
    }

    let damage_total = players.iter().map(|player| player.damage).sum::<i64>();
    let healing_total = players.iter().map(|player| player.healing).sum::<i64>();
    let contribution = match subject.role {
        Role::Support if healing_total > 0 => {
            ("team_share_healing", "治疗", subject.healing, healing_total)
        }
        _ => ("team_share_damage", "伤害", subject.damage, damage_total),
    };
    if contribution.3 > 0 {
        let share = contribution.2 as f64 / contribution.3 as f64;
        push_fact(
            &mut facts,
            contribution.0,
            format!("贡献全队{}的{:.0}%", contribution.1, share * 100.0),
            RoastFactEvidence::Contribution,
            "team_share",
            0.78,
            (share * players.len() as f64 - 1.0).abs().min(1.0),
            evaluation,
        );
    }

    let death_rank = players
        .iter()
        .filter(|player| player.deaths > subject.deaths)
        .count()
        + 1;
    push_fact(
        &mut facts,
        "team_rank_deaths",
        format!("阵亡{}次，按阵亡数排全队第{death_rank}", subject.deaths),
        RoastFactEvidence::Rank,
        "deaths",
        0.82,
        rank_contrast(death_rank, players.len()),
        evaluation,
    );

    if subject.current {
        if let Some(average) = same_role_season_average(conn, context.season, subject.role)? {
            add_season_average_facts(&mut facts, subject, average, evaluation);
        }
    }
    if subject.role == Role::Support && subject.healing < subject.damage {
        let ratio = if subject.damage == 0 {
            0.0
        } else {
            subject.healing as f64 / subject.damage as f64
        };
        push_fact(
            &mut facts,
            "support_healing_damage_balance",
            format!(
                "治疗量{}，相当于伤害量{}的{:.0}%",
                subject.healing,
                subject.damage,
                ratio * 100.0
            ),
            RoastFactEvidence::StatComparison,
            "support_balance",
            0.94,
            (1.0 - ratio).clamp(0.0, 1.0),
            evaluation,
        );
        if let Some(fact) = facts.last_mut() {
            fact.score += 0.15;
        }
    }
    if *kind == RoastRelationshipKind::Carry {
        for fact in &mut facts {
            if fact.metric == "composite_rank" || fact.metric == "role_primary" {
                fact.score += 0.08;
            }
        }
    }
    Ok(facts)
}

fn push_fact(
    facts: &mut Vec<FactCandidate>,
    id: &str,
    text: String,
    evidence: RoastFactEvidence,
    metric: &'static str,
    evidence_strength: f64,
    contrast: f64,
    evaluation: &RoastEvaluation,
) {
    let label_relevance = if evaluation
        .evidence_tags
        .iter()
        .any(|tag| fact_matches_evidence(metric, tag))
    {
        1.0
    } else {
        0.5
    };
    let comprehensibility = if text.len() <= 36 { 1.0 } else { 0.75 };
    let strength = (evidence_strength * 0.55
        + contrast.clamp(0.0, 1.0) * 0.25
        + label_relevance * 0.12
        + comprehensibility * 0.08)
        .clamp(0.0, 1.0);
    facts.push(FactCandidate {
        fact: RoastFact {
            id: id.to_string(),
            text,
            strength,
            evidence,
        },
        metric,
        score: strength,
    });
}

fn fact_matches_evidence(metric: &str, tag: &str) -> bool {
    matches!(
        (metric, tag),
        ("role_primary", "伤害最高")
            | ("role_primary", "治疗最高")
            | ("team_share", "伤害最高")
            | ("team_share", "治疗最高")
            | ("deaths", "KDA最高")
    )
}

fn primary_role_metric(role: Role, player: &PlayerMetrics) -> f64 {
    match role {
        Role::Tank => (player.damage + player.mitigation) as f64,
        Role::Damage => player.damage as f64,
        Role::Support => player.healing as f64,
    }
}

fn primary_role_metric_name(role: Role) -> &'static str {
    match role {
        Role::Tank => "伤害与承伤合计",
        Role::Damage => "伤害",
        Role::Support => "治疗",
    }
}

fn format_metric(value: f64) -> String {
    if value >= 1_000.0 {
        format!("{:.1}千", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}

fn rank_contrast(rank: usize, total: usize) -> f64 {
    if total <= 1 {
        0.0
    } else {
        ((rank as f64 - 1.0) / (total as f64 - 1.0) - 0.5).abs() * 2.0
    }
}

fn relative_contrast(value: f64, baseline: f64) -> f64 {
    if value == 0.0 && baseline == 0.0 {
        0.0
    } else {
        ((value - baseline).abs() / value.abs().max(baseline.abs()).max(1.0)).min(1.0)
    }
}

fn same_role_season_average(
    conn: &Connection,
    season: i64,
    role: Role,
) -> Result<Option<SeasonAverage>> {
    let raw_roles: &[&str] = match role {
        Role::Tank => &["tank"],
        Role::Damage => &["dps", "damage"],
        Role::Support => &["healer", "support"],
    };
    let placeholders = std::iter::repeat_n("?", raw_roles.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT AVG(mp.kill), AVG(mp.death), AVG(mp.hero_damage), AVG(mp.cure) FROM match_players mp JOIN matches m ON m.match_id=mp.match_id WHERE mp.season=? AND mp.is_current_player=1 AND mp.role_type IN ({placeholders}) AND m.match_kind='normal'"
    );
    let mut values = Vec::<Box<dyn rusqlite::ToSql>>::new();
    values.push(Box::new(season));
    values.extend(
        raw_roles
            .iter()
            .map(|role| Box::new((*role).to_string()) as Box<dyn rusqlite::ToSql>),
    );
    let refs = values
        .iter()
        .map(|value| value.as_ref())
        .collect::<Vec<_>>();
    conn.query_row(&sql, refs.as_slice(), |row| {
        let kills = row.get::<_, Option<f64>>(0)?;
        let deaths = row.get::<_, Option<f64>>(1)?;
        let damage = row.get::<_, Option<f64>>(2)?;
        let healing = row.get::<_, Option<f64>>(3)?;
        Ok(match (kills, deaths, damage, healing) {
            (Some(kills), Some(deaths), Some(damage), Some(healing)) => Some(SeasonAverage {
                kills,
                deaths,
                damage,
                healing,
            }),
            _ => None,
        })
    })
    .map_err(Into::into)
}

fn add_season_average_facts(
    facts: &mut Vec<FactCandidate>,
    subject: &PlayerMetrics,
    average: SeasonAverage,
    evaluation: &RoastEvaluation,
) {
    let (id, name, value, baseline) = match subject.role {
        Role::Tank => (
            "same_role_damage",
            "伤害",
            subject.damage as f64,
            average.damage,
        ),
        Role::Damage => (
            "same_role_damage",
            "伤害",
            subject.damage as f64,
            average.damage,
        ),
        Role::Support => (
            "same_role_healing",
            "治疗",
            subject.healing as f64,
            average.healing,
        ),
    };
    let direction = if value >= baseline {
        "高于"
    } else {
        "低于"
    };
    push_fact(
        facts,
        id,
        format!(
            "本场{name}{}，{direction}赛季同角色均值{}",
            format_metric(value),
            format_metric(baseline)
        ),
        RoastFactEvidence::SameRole,
        "season_role",
        0.96,
        relative_contrast(value, baseline),
        evaluation,
    );
    if let Some(fact) = facts.last_mut() {
        fact.score += 0.18;
    }
    push_fact(
        facts,
        "same_role_deaths",
        format!(
            "本场阵亡{}次，赛季同角色均值{:.1}次",
            subject.deaths, average.deaths
        ),
        RoastFactEvidence::SameRole,
        "deaths",
        0.84,
        relative_contrast(subject.deaths as f64, average.deaths),
        evaluation,
    );
    if subject.role != Role::Support {
        push_fact(
            facts,
            "same_role_kills",
            format!(
                "本场击杀{}次，赛季同角色均值{:.1}次",
                subject.kills, average.kills
            ),
            RoastFactEvidence::SameRole,
            "kills",
            0.82,
            relative_contrast(subject.kills as f64, average.kills),
            evaluation,
        );
    }
}

fn select_facts(mut candidates: Vec<FactCandidate>) -> Vec<RoastFact> {
    candidates.retain(|candidate| candidate.fact.strength >= 0.35);
    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.fact.id.cmp(&right.fact.id))
    });
    let mut metrics = HashSet::new();
    let mut selected = Vec::new();
    for candidate in candidates {
        if !metrics.insert(candidate.metric) {
            continue;
        }
        selected.push(candidate.fact);
        if selected.len() == 4 {
            break;
        }
    }
    if selected.len() == 4 && selected[3].strength < 0.55 {
        selected.pop();
    }
    selected
}

fn counter_facts(players: &[PlayerMetrics], subject: &PlayerMetrics) -> Vec<RoastFact> {
    let damage_total = players.iter().map(|player| player.damage).sum::<i64>();
    if damage_total <= 0 || subject.damage <= 0 {
        return Vec::new();
    }
    vec![RoastFact {
        id: "counter_team_damage_share".to_string(),
        text: format!(
            "仍贡献全队伤害的{:.0}%",
            subject.damage as f64 / damage_total as f64 * 100.0
        ),
        strength: 0.62,
        evidence: RoastFactEvidence::Counterpoint,
    }]
}

fn neutral_relationship(result: &str) -> RoastRelationship {
    let result_text = match result {
        "win" => "本场结果为胜利",
        "loss" => "本场结果为失利",
        "draw" => "本场结果为平局",
        _ => "本场结果暂不明确",
    };
    RoastRelationship {
        kind: RoastRelationshipKind::Neutral,
        subject: "本场整体".to_string(),
        facts: vec![
            RoastFact {
                id: "result_neutral".to_string(),
                text: result_text.to_string(),
                strength: 0.60,
                evidence: RoastFactEvidence::Result,
            },
            RoastFact {
                id: "evidence_neutral".to_string(),
                text: "现有数据不足以稳定归因到单个位置".to_string(),
                strength: 0.58,
                evidence: RoastFactEvidence::Availability,
            },
            RoastFact {
                id: "relationship_neutral".to_string(),
                text: "本次仅保留结果层面的中性材料".to_string(),
                strength: 0.56,
                evidence: RoastFactEvidence::Availability,
            },
        ],
        counter_facts: Vec::new(),
        confidence: 0.55,
    }
}

fn streak_context(
    conn: &Connection,
    match_id: &str,
    context: &MatchContext,
) -> Result<RoastStreakContext> {
    let mut stmt = conn.prepare(
        r#"
        SELECT match_id, result, begin_ts
        FROM matches
        WHERE season=? AND match_kind='normal'
        ORDER BY begin_ts ASC, match_id ASC
        "#,
    )?;
    let rows = stmt
        .query_map(params![context.season], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let current_index = rows
        .iter()
        .position(|(row_match_id, _, begin_ts)| {
            row_match_id == match_id && *begin_ts == context.begin_ts
        })
        .ok_or_else(|| anyhow!("未找到当前对局的连续结果位置"))?;
    let previous_result = current_index
        .checked_sub(1)
        .and_then(|index| rows.get(index))
        .map(|(_, result, _)| result.as_str());
    let before_current = previous_result
        .filter(|result| *result != "draw")
        .map(|result| {
            rows[..current_index]
                .iter()
                .rev()
                .take_while(|(_, row_result, _)| row_result == result)
                .count()
        })
        .unwrap_or(0);
    let previous_event = previous_result.and_then(|result| match result {
        "win" if before_current > 0 => Some(format!("此前连续获胜{before_current}场")),
        "loss" if before_current > 0 => Some(format!("此前连续失利{before_current}场")),
        "draw" => Some("此前一场为平局".to_string()),
        _ => None,
    });
    let current_event = match context.result.as_str() {
        "win" if previous_result == Some("win") => {
            format!("将连胜延长到{}场", before_current + 1)
        }
        "loss" if previous_result == Some("loss") => {
            format!("将连败延长到{}场", before_current + 1)
        }
        "win" if previous_result == Some("loss") => {
            format!("结束{before_current}场连败并获胜")
        }
        "loss" if previous_result == Some("win") => {
            format!("结束{before_current}场连胜并失利")
        }
        "draw" => "平局中断连续结果".to_string(),
        "win" => "本场获胜".to_string(),
        "loss" => "本场失利".to_string(),
        _ => "本场结果未知".to_string(),
    };
    Ok(RoastStreakContext {
        before_current,
        previous_event,
        current_event,
    })
}

pub fn assert_material_privacy(material: &RoastMaterial) -> Result<()> {
    if material.material_version != MATERIAL_VERSION {
        return Err(anyhow!("锐评材料版本无效"));
    }
    if material.relationship.facts.len() < 3 || material.relationship.facts.len() > 4 {
        return Err(anyhow!("锐评关系事实数量无效"));
    }
    if material.relationship.kind != RoastRelationshipKind::Neutral
        && !material.relationship.facts.iter().any(|fact| {
            matches!(
                fact.evidence,
                RoastFactEvidence::Rank | RoastFactEvidence::SameRole
            )
        })
    {
        return Err(anyhow!("锐评关系缺少排名或同位置比较"));
    }
    if material.relationship.kind != RoastRelationshipKind::Neutral
        && !(material.relationship.subject.starts_with("我方")
            || material.relationship.subject.starts_with("当前玩家（"))
    {
        return Err(anyhow!("锐评主体不是匿名位置描述"));
    }
    let value = serde_json::to_value(material)?;
    scan_value(&value)
}

fn scan_value(value: &Value) -> Result<()> {
    match value {
        Value::Object(values) => {
            for (key, value) in values {
                let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
                if [
                    "matchid",
                    "playername",
                    "battletag",
                    "bnetid",
                    "accountid",
                    "rawjson",
                    "raw",
                    "note",
                    "notes",
                    "path",
                    "credential",
                    "authorization",
                    "cookie",
                    "token",
                    "customertoken",
                    "lineup",
                ]
                .contains(&normalized.as_str())
                {
                    return Err(anyhow!("锐评材料包含禁止字段"));
                }
                scan_value(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                scan_value(value)?;
            }
        }
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            if lower.contains("battle.net")
                || lower.contains("battletag")
                || lower.contains("authorization:")
                || lower.contains("bearer ")
                || lower.contains("cookie=")
                || lower.contains("customertoken")
                || lower.contains(":\\")
                || lower.contains("/@fs/")
            {
                return Err(anyhow!("锐评材料包含禁止内容"));
            }
        }
        _ => {}
    }
    Ok(())
}
