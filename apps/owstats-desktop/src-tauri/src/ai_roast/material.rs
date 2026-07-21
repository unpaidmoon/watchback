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
const MIN_MEANINGFUL_RELATIVE_CONTRAST: f64 = 0.01;
const INSUFFICIENT_DETAIL_ERROR: &str = "\u{5f53}\u{524d}\u{5bf9}\u{5c40}\u{7f3a}\u{5c11}\u{8db3}\u{591f}\u{7684}\u{53ef}\u{6838}\u{9a8c}\u{9510}\u{8bc4}\u{7ec6}\u{8282}";
const MIN_FACT_STRENGTH: f64 = 0.35;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastMaterial {
    pub material_version: i64,
    pub evaluation: RoastEvaluation,
    pub selected_facts: Vec<RoastFact>,
    pub relationship: Option<RoastRelationship>,
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
    pub subject: FactSubject,
    pub text: String,
    pub strength: f64,
    pub evidence: RoastFactEvidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FactSubject {
    CurrentPlayer,
    RelationshipSubject,
    Team,
    Match,
}

impl FactSubject {
    pub fn id_prefix(self) -> &'static str {
        match self {
            Self::CurrentPlayer => "current_player:",
            Self::RelationshipSubject => "relationship_subject:",
            Self::Team => "team:",
            Self::Match => "match:",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoastFactEvidence {
    Rank,
    SameRole,
    Contribution,
    StatComparison,
    Result,
    Counterpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoastRelationshipKind {
    WeakLink,
    Carry,
    Scapegoat,
    SelfRoast,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnonymousPlayerRole {
    Tank,
    Damage,
    Support,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnonymousOrdinal {
    First,
    Second,
    Third,
    Fourth,
    Fifth,
}

impl AnonymousOrdinal {
    fn from_slot(slot: usize) -> Option<Self> {
        match slot {
            1 => Some(Self::First),
            2 => Some(Self::Second),
            3 => Some(Self::Third),
            4 => Some(Self::Fourth),
            5 => Some(Self::Fifth),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoastRelationshipSubject {
    CurrentPlayer,
    Teammate {
        role: AnonymousPlayerRole,
        ordinal: AnonymousOrdinal,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastRelationship {
    pub kind: RoastRelationshipKind,
    pub subject: RoastRelationshipSubject,
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
    subject: RoastRelationshipSubject,
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
    metric: String,
    category: MetricCategory,
    score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricCategory {
    CompositeRank,
    RolePrimary,
    TeamShare,
    Deaths,
    Other,
}

#[derive(Debug, Clone)]
struct RelationshipSelection {
    relationship: RoastRelationship,
    candidates: Vec<FactCandidate>,
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
    if players.len() < 2 {
        return Err(anyhow!(
            "\u{5f53}\u{524d}\u{5bf9}\u{5c40}\u{7f3a}\u{5c11}\u{8db3}\u{591f}\u{7684}\u{53ef}\u{6838}\u{9a8c}\u{9510}\u{8bc4}\u{7ec6}\u{8282}"
        ));
    }
    let relationship_selection = select_relationship(conn, &context, &evaluation, &players)?;
    let mut selected_candidates = vec![result_fact_candidate(&context.result)?];
    if let Some(current) = players.iter().find(|player| player.current) {
        selected_candidates.extend(fact_candidates(
            conn,
            &context,
            &evaluation,
            &players,
            current,
            FactSubject::CurrentPlayer,
            None,
        )?);
    }
    if let Some(selection) = &relationship_selection {
        selected_candidates.extend(selection.candidates.clone());
    }
    let selected_facts = select_facts(selected_candidates);
    if selected_facts.len() < 3 {
        return Err(anyhow!(INSUFFICIENT_DETAIL_ERROR));
    }
    let material = RoastMaterial {
        material_version: MATERIAL_VERSION,
        evaluation,
        selected_facts,
        relationship: relationship_selection.map(|selection| selection.relationship),
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
        confidence: if valid_probability(value.confidence) {
            value.confidence
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
    let mut invalid = false;
    let mut current_count = 0;
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
            invalid = true;
            continue;
        };
        if [kills, assists, deaths, damage, healing, mitigation]
            .iter()
            .any(|value| *value < 0)
        {
            invalid = true;
            continue;
        }
        let slot = role_slots.entry(role).or_insert(0usize);
        *slot += 1;
        if current {
            current_count += 1;
        }
        let subject = if current {
            RoastRelationshipSubject::CurrentPlayer
        } else {
            RoastRelationshipSubject::Teammate {
                role: role.anonymous_role(),
                ordinal: AnonymousOrdinal::from_slot(*slot)
                    .ok_or_else(|| anyhow!(INSUFFICIENT_DETAIL_ERROR))?,
            }
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
    if invalid || players.len() != 5 || current_count != 1 {
        return Err(anyhow!(INSUFFICIENT_DETAIL_ERROR));
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

    fn anonymous_role(self) -> AnonymousPlayerRole {
        match self {
            Self::Tank => AnonymousPlayerRole::Tank,
            Self::Damage => AnonymousPlayerRole::Damage,
            Self::Support => AnonymousPlayerRole::Support,
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
) -> Result<Option<RelationshipSelection>> {
    if players.len() < 2 {
        return Ok(None);
    }
    let current = players.iter().find(|player| player.current);
    let weakest = unique_extreme(players, false);
    let strongest = unique_extreme(players, true);
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
        return Ok(None);
    };
    let Some(subject) = subject else {
        return Ok(None);
    };
    let candidates = fact_candidates(
        conn,
        context,
        evaluation,
        players,
        subject,
        FactSubject::RelationshipSubject,
        Some(&kind),
    )?;
    let facts = select_facts(candidates.clone());
    if facts.len() < 2
        || !facts.iter().any(|fact| {
            matches!(
                fact.evidence,
                RoastFactEvidence::Rank | RoastFactEvidence::SameRole
            )
        })
    {
        return Ok(None);
    }
    let counter_facts = if kind == RoastRelationshipKind::Scapegoat {
        counter_facts(players, subject)
    } else {
        Vec::new()
    };
    let confidence =
        (facts.iter().map(|fact| fact.strength).sum::<f64>() / facts.len() as f64).clamp(0.0, 1.0);
    Ok(Some(RelationshipSelection {
        relationship: RoastRelationship {
            kind,
            subject: subject.subject,
            facts,
            counter_facts,
            confidence,
        },
        candidates,
    }))
}

fn ranked(players: &[PlayerMetrics], descending: bool) -> Vec<&PlayerMetrics> {
    let mut ranked = players.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        let order = left
            .composite
            .partial_cmp(&right.composite)
            .unwrap_or(Ordering::Equal);
        if descending {
            order.reverse()
        } else {
            order
        }
    });
    ranked
}

fn unique_extreme(players: &[PlayerMetrics], descending: bool) -> Option<&PlayerMetrics> {
    let ranked = ranked(players, descending);
    let extreme = ranked.first().copied()?;
    let next = ranked.get(1).copied()?;
    meaningful_relative_contrast(extreme.composite, next.composite).then_some(extreme)
}

fn competition_rank(players: &[PlayerMetrics], subject: &PlayerMetrics) -> (usize, usize) {
    let rank = players
        .iter()
        .filter(|player| player.composite > subject.composite)
        .count()
        + 1;
    let tied = players
        .iter()
        .filter(|player| player.composite == subject.composite)
        .count();
    (rank, tied)
}

fn maximum_relative_contrast(subject_value: f64, peer_values: impl Iterator<Item = f64>) -> f64 {
    peer_values
        .map(|peer_value| relative_contrast(subject_value, peer_value))
        .fold(0.0, f64::max)
}

fn fact_candidates(
    conn: &Connection,
    context: &MatchContext,
    evaluation: &RoastEvaluation,
    players: &[PlayerMetrics],
    subject: &PlayerMetrics,
    fact_subject: FactSubject,
    kind: Option<&RoastRelationshipKind>,
) -> Result<Vec<FactCandidate>> {
    let mut facts = Vec::new();
    let composite_contrast = maximum_relative_contrast(
        subject.composite,
        players
            .iter()
            .filter(|player| player.rowid != subject.rowid)
            .map(|player| player.composite),
    );
    if players.len() >= 2 && composite_contrast >= MIN_MEANINGFUL_RELATIVE_CONTRAST {
        let (team_rank, _) = competition_rank(players, subject);
        let rank_text = match team_rank {
            1 => "综合表现位列全队第1".to_string(),
            rank if rank == players.len() => format!("综合表现位列全队第{rank}"),
            rank => format!("综合表现位列全队第{rank}"),
        };
        push_fact(
            &mut facts,
            fact_subject,
            "team_rank_composite",
            rank_text,
            RoastFactEvidence::Rank,
            "composite_rank",
            0.98,
            composite_contrast,
            evaluation,
        );
    }

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
        let role_contrast = relative_contrast(role_metric, peer_average);
        if role_contrast >= MIN_MEANINGFUL_RELATIVE_CONTRAST {
            let metric_name = primary_role_metric_name(subject.role);
            let direction = if delta >= 0.0 { "高" } else { "低" };
            push_fact(
                &mut facts,
                fact_subject,
                "same_role_primary_delta",
                format!(
                    "{metric_name}比同位置队友平均{direction}{}",
                    format_metric(delta.abs())
                ),
                RoastFactEvidence::SameRole,
                "role_primary",
                0.90,
                role_contrast,
                evaluation,
            );
        }
    }

    let damage_total = players
        .iter()
        .map(|player| player.damage as f64)
        .sum::<f64>();
    let healing_total = players
        .iter()
        .map(|player| player.healing as f64)
        .sum::<f64>();
    let contribution = match subject.role {
        Role::Support if healing_total > 0.0 => {
            ("team_share_healing", "治疗", subject.healing, healing_total)
        }
        _ => ("team_share_damage", "伤害", subject.damage, damage_total),
    };
    if players.len() >= 2 && contribution.3 > 0.0 {
        let share = contribution.2 as f64 / contribution.3 as f64;
        let contribution_contrast = relative_contrast(share, 1.0 / players.len() as f64);
        if contribution_contrast >= MIN_MEANINGFUL_RELATIVE_CONTRAST {
            push_fact(
                &mut facts,
                fact_subject,
                contribution.0,
                format!("贡献全队{}的{:.0}%", contribution.1, share * 100.0),
                RoastFactEvidence::Contribution,
                "team_share",
                0.78,
                contribution_contrast,
                evaluation,
            );
        }
    }

    let death_contrast = maximum_relative_contrast(
        subject.deaths as f64,
        players
            .iter()
            .filter(|player| player.rowid != subject.rowid)
            .map(|player| player.deaths as f64),
    );
    if players.len() >= 2 && death_contrast >= MIN_MEANINGFUL_RELATIVE_CONTRAST {
        let death_rank = players
            .iter()
            .filter(|player| player.deaths > subject.deaths)
            .count()
            + 1;
        push_fact(
            &mut facts,
            fact_subject,
            "team_rank_deaths",
            format!("阵亡{}次，按阵亡数排全队第{death_rank}", subject.deaths),
            RoastFactEvidence::Rank,
            "deaths",
            0.82,
            death_contrast,
            evaluation,
        );
    }

    if subject.current {
        if let Some(average) = same_role_season_average(conn, context.season, subject.role)? {
            add_season_average_facts(&mut facts, subject, fact_subject, average, evaluation);
        }
    }
    if subject.role == Role::Support
        && subject.healing < subject.damage
        && meaningful_relative_contrast(subject.healing as f64, subject.damage as f64)
    {
        let ratio = if subject.damage == 0 {
            0.0
        } else {
            subject.healing as f64 / subject.damage as f64
        };
        push_fact(
            &mut facts,
            fact_subject,
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
    if kind == Some(&RoastRelationshipKind::Carry) {
        for fact in &mut facts {
            if matches!(
                fact.category,
                MetricCategory::CompositeRank | MetricCategory::RolePrimary
            ) {
                fact.score += 0.08;
            }
        }
    }
    Ok(facts)
}

fn push_fact(
    facts: &mut Vec<FactCandidate>,
    subject: FactSubject,
    id: &str,
    text: String,
    evidence: RoastFactEvidence,
    metric: &'static str,
    evidence_strength: f64,
    contrast: f64,
    evaluation: &RoastEvaluation,
) {
    if matches!(
        evidence,
        RoastFactEvidence::Rank
            | RoastFactEvidence::SameRole
            | RoastFactEvidence::Contribution
            | RoastFactEvidence::StatComparison
    ) && contrast < MIN_MEANINGFUL_RELATIVE_CONTRAST
    {
        return;
    }
    let label_relevance = if evaluation
        .evidence_tags
        .iter()
        .any(|tag| fact_matches_evidence(metric_category(metric), tag))
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
            id: format!("{}{id}", subject.id_prefix()),
            subject,
            text,
            strength,
            evidence,
        },
        metric: format!("{}{metric}", subject.id_prefix()),
        category: metric_category(metric),
        score: strength,
    });
}

fn metric_category(metric: &str) -> MetricCategory {
    match metric {
        "composite_rank" => MetricCategory::CompositeRank,
        "role_primary" | "season_role" => MetricCategory::RolePrimary,
        "team_share" => MetricCategory::TeamShare,
        "deaths" => MetricCategory::Deaths,
        _ => MetricCategory::Other,
    }
}

fn fact_matches_evidence(metric: MetricCategory, tag: &str) -> bool {
    matches!(
        (metric, tag),
        (MetricCategory::RolePrimary, "伤害最高")
            | (MetricCategory::RolePrimary, "治疗最高")
            | (MetricCategory::TeamShare, "伤害最高")
            | (MetricCategory::TeamShare, "治疗最高")
            | (MetricCategory::Deaths, "KDA最高")
    )
}

fn primary_role_metric(role: Role, player: &PlayerMetrics) -> f64 {
    match role {
        Role::Tank => player.damage as f64 + player.mitigation as f64,
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

fn relative_contrast(value: f64, baseline: f64) -> f64 {
    if value == 0.0 && baseline == 0.0 {
        0.0
    } else {
        ((value - baseline).abs() / value.abs().max(baseline.abs()).max(1.0)).min(1.0)
    }
}

fn meaningful_relative_contrast(value: f64, baseline: f64) -> bool {
    value != baseline && relative_contrast(value, baseline) >= MIN_MEANINGFUL_RELATIVE_CONTRAST
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
    fact_subject: FactSubject,
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
        fact_subject,
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
        fact_subject,
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
            fact_subject,
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
    let damage_total = players
        .iter()
        .map(|player| player.damage as f64)
        .sum::<f64>();
    if damage_total <= 0.0 || subject.damage <= 0 {
        return Vec::new();
    }
    vec![RoastFact {
        id: format!(
            "{}counter_team_damage_share",
            FactSubject::RelationshipSubject.id_prefix()
        ),
        subject: FactSubject::RelationshipSubject,
        text: format!(
            "仍贡献全队伤害的{:.0}%",
            subject.damage as f64 / damage_total * 100.0
        ),
        strength: 0.62,
        evidence: RoastFactEvidence::Counterpoint,
    }]
}

fn result_fact_candidate(result: &str) -> Result<FactCandidate> {
    let text = match result {
        "win" => "本场结果为胜利",
        "loss" => "本场结果为失利",
        "draw" => "本场结果为平局",
        _ => return Err(anyhow!("当前对局结果无法形成可核验锐评事实")),
    };
    Ok(FactCandidate {
        fact: RoastFact {
            id: format!("{}match_result", FactSubject::Match.id_prefix()),
            subject: FactSubject::Match,
            text: text.to_string(),
            strength: 0.88,
            evidence: RoastFactEvidence::Result,
        },
        metric: format!("{}match_result", FactSubject::Match.id_prefix()),
        category: MetricCategory::Other,
        score: 0.88,
    })
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

fn valid_probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn valid_fact_strength(value: f64) -> bool {
    valid_probability(value) && value >= MIN_FACT_STRENGTH
}

pub fn assert_material_privacy(material: &RoastMaterial) -> Result<()> {
    if material.material_version != MATERIAL_VERSION {
        return Err(anyhow!(
            "\u{9510}\u{8bc4}\u{6750}\u{6599}\u{7248}\u{672c}\u{65e0}\u{6548}"
        ));
    }
    if !valid_probability(material.evaluation.confidence) {
        return Err(anyhow!(
            "\u{9510}\u{8bc4}\u{8bc4}\u{4ef7}\u{7f6e}\u{4fe1}\u{5ea6}\u{65e0}\u{6548}"
        ));
    }
    if material.selected_facts.len() < 3 || material.selected_facts.len() > 4 {
        return Err(anyhow!(
            "\u{9510}\u{8bc4}\u{4e8b}\u{5b9e}\u{6570}\u{91cf}\u{65e0}\u{6548}"
        ));
    }
    if material
        .selected_facts
        .iter()
        .any(|fact| !valid_fact_strength(fact.strength))
    {
        return Err(anyhow!(
            "\u{9510}\u{8bc4}\u{4e8b}\u{5b9e}\u{5f3a}\u{5ea6}\u{65e0}\u{6548}"
        ));
    }
    let selected_ids = material
        .selected_facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();
    if selected_ids.len() != material.selected_facts.len() {
        return Err(anyhow!(
            "\u{9510}\u{8bc4}\u{4e8b}\u{5b9e}\u{672a}\u{53bb}\u{91cd}"
        ));
    }
    if material
        .selected_facts
        .iter()
        .any(|fact| !fact.id.starts_with(fact.subject.id_prefix()))
    {
        return Err(anyhow!(
            "\u{9510}\u{8bc4}\u{4e8b}\u{5b9e}\u{4e3b}\u{4f53}\u{6807}\u{8bc6}\u{65e0}\u{6548}"
        ));
    }
    if let Some(relationship) = &material.relationship {
        if !valid_probability(relationship.confidence) {
            return Err(anyhow!(
                "\u{9510}\u{8bc4}\u{5173}\u{7cfb}\u{7f6e}\u{4fe1}\u{5ea6}\u{65e0}\u{6548}"
            ));
        }
        if relationship
            .facts
            .iter()
            .chain(relationship.counter_facts.iter())
            .any(|fact| {
                fact.subject != FactSubject::RelationshipSubject
                    || !fact.id.starts_with(fact.subject.id_prefix())
                    || !valid_fact_strength(fact.strength)
            })
        {
            return Err(anyhow!("\u{9510}\u{8bc4}\u{5173}\u{7cfb}\u{4e8b}\u{5b9e}\u{4e3b}\u{4f53}\u{6216}\u{5f3a}\u{5ea6}\u{65e0}\u{6548}"));
        }
        if relationship.facts.len() < 2
            || relationship.facts.len() > 4
            || !relationship.facts.iter().any(|fact| {
                matches!(
                    fact.evidence,
                    RoastFactEvidence::Rank | RoastFactEvidence::SameRole
                )
            })
        {
            return Err(anyhow!("\u{9510}\u{8bc4}\u{5173}\u{7cfb}\u{7f3a}\u{5c11}\u{8db3}\u{591f}\u{7684}\u{6392}\u{540d}\u{6216}\u{540c}\u{4f4d}\u{7f6e}\u{8bc1}\u{636e}"));
        }
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
            let compact = lower
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>();
            let normalized = compact.replace(['_', '-'], "");
            if lower.contains("battle.net")
                || lower.contains("battletag")
                || normalized.contains("authorization")
                || lower.contains("bearer ")
                || normalized.contains("cookie=")
                || normalized.contains("cookie:")
                || normalized.contains("credential=")
                || normalized.contains("credential:")
                || normalized.contains("token=")
                || normalized.contains("token:")
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
