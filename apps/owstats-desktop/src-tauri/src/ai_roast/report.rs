use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::snapshot::RoastSnapshot;

pub const REPORT_SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiRoastNarrative {
    pub analysis: NarrativeAnalysis,
    pub title: String,
    pub commentary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NarrativeAnalysis {
    pub match_thesis: RoastClaim,
    pub current_player: RoastClaim,
    pub key_actors: Vec<KeyActorClaim>,
    pub supporting_factors: Vec<RoastClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoastClaim {
    pub judgment: String,
    pub claim_type: ClaimType,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyActorClaim {
    pub subject: String,
    pub impact: ActorImpact,
    #[serde(flatten)]
    pub claim: RoastClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorImpact {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StoredRoastPayload {
    Narrative(AiRoastNarrative),
    Legacy(AiRoastReport),
}

impl StoredRoastPayload {
    pub fn evidence_refs(&self) -> Vec<String> {
        match self {
            Self::Narrative(report) => std::iter::once(&report.analysis.match_thesis)
                .chain(std::iter::once(&report.analysis.current_player))
                .chain(report.analysis.key_actors.iter().map(|item| &item.claim))
                .chain(report.analysis.supporting_factors.iter())
                .flat_map(|claim| claim.evidence_refs.iter().cloned())
                .collect(),
            Self::Legacy(report) => report
                .decisive_factors
                .iter()
                .chain(std::iter::once(&report.current_player))
                .chain(report.team_and_matchups.iter().map(|item| &item.insight))
                .flat_map(|item| item.evidence_refs.iter().cloned())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiRoastReport {
    pub verdict: RoastVerdict,
    pub decisive_factors: Vec<RoastInsight>,
    pub current_player: RoastInsight,
    pub team_and_matchups: Vec<TeamInsight>,
    pub closing_roast: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoastVerdict {
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoastInsight {
    pub title: String,
    pub commentary: String,
    pub claim_type: ClaimType,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeamInsight {
    #[serde(rename = "type")]
    pub kind: TeamInsightKind,
    #[serde(flatten)]
    pub insight: RoastInsight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    DirectFact,
    StatisticalInterpretation,
    GameContextInference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamInsightKind {
    PositivePlayer,
    NegativePlayer,
    RoleMatchup,
    ArenaTrend,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvidenceValue {
    Integer(i64),
    Decimal(f64),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSemantic {
    RawStat,
    Rank,
    Share,
    Delta,
    RoundResult,
    FinalWorth,
    GameKnowledge,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceEntry {
    pub value: EvidenceValue,
    pub semantic: EvidenceSemantic,
}

pub type EvidenceCatalog = BTreeMap<String, EvidenceEntry>;

pub fn evidence_catalog(snapshot: &RoastSnapshot) -> EvidenceCatalog {
    let mut catalog = EvidenceCatalog::new();
    add_optional_text(
        &mut catalog,
        "match.result",
        snapshot.match_context.result.as_ref(),
    );
    add_optional_text(
        &mut catalog,
        "match.mode",
        snapshot.match_context.mode.as_ref(),
    );
    add_optional_text(
        &mut catalog,
        "match.map",
        snapshot.match_context.map.as_ref(),
    );
    add_optional_integer(
        &mut catalog,
        "match.teamScore",
        snapshot.match_context.team_score,
    );
    add_optional_integer(
        &mut catalog,
        "match.opponentScore",
        snapshot.match_context.opponent_score,
    );
    add_optional_integer(
        &mut catalog,
        "match.durationSeconds",
        snapshot.match_context.duration_seconds,
    );
    for player in snapshot
        .scoreboard
        .team
        .iter()
        .chain(&snapshot.scoreboard.opponent)
    {
        add_object(
            &mut catalog,
            &format!("{}.stats", player.subject),
            &player.stats,
            EvidenceSemantic::RawStat,
        );
        let value = serde_json::to_value(&player.derived).expect("derived metrics serialize");
        if let Some(object) = value.as_object() {
            for (name, value) in object {
                let semantic = if name.ends_with("Rank") {
                    EvidenceSemantic::Rank
                } else {
                    EvidenceSemantic::Share
                };
                add_value(
                    &mut catalog,
                    format!("{}.derived.{name}", player.subject),
                    value,
                    semantic,
                );
            }
        }
    }
    for matchup in &snapshot.role_matchups {
        add_object(
            &mut catalog,
            &format!("roleMatchups.{}.teamTotals", matchup.role),
            &matchup.team_totals,
            EvidenceSemantic::RawStat,
        );
        add_object(
            &mut catalog,
            &format!("roleMatchups.{}.opponentTotals", matchup.role),
            &matchup.opponent_totals,
            EvidenceSemantic::RawStat,
        );
        add_object(
            &mut catalog,
            &format!("roleMatchups.{}.deltas", matchup.role),
            &matchup.deltas,
            EvidenceSemantic::Delta,
        );
    }
    add_object(
        &mut catalog,
        "taskKnowledge",
        &snapshot.task_knowledge,
        EvidenceSemantic::GameKnowledge,
    );
    if let Some(arena) = &snapshot.arena {
        for round in &arena.round_results {
            catalog.insert(
                format!("arena.roundResults.{}.result", round.round),
                EvidenceEntry {
                    value: EvidenceValue::Text(round.result.clone()),
                    semantic: EvidenceSemantic::RoundResult,
                },
            );
            catalog.insert(
                format!("arena.roundResults.{}.scoreAfter", round.round),
                EvidenceEntry {
                    value: EvidenceValue::Text(round.score_after.clone()),
                    semantic: EvidenceSemantic::RoundResult,
                },
            );
        }
        for (subject, worth) in &arena.final_worth {
            catalog.insert(
                format!("arena.finalWorth.{subject}"),
                EvidenceEntry {
                    value: EvidenceValue::Integer(*worth),
                    semantic: EvidenceSemantic::FinalWorth,
                },
            );
        }
    }
    for (hero, knowledge) in &snapshot.game_knowledge.hero_overrides {
        if let Some(archetype) = &knowledge.primary_archetype {
            catalog.insert(
                format!("gameKnowledge.heroes.{hero}.primaryArchetype"),
                EvidenceEntry {
                    value: EvidenceValue::Text(archetype.clone()),
                    semantic: EvidenceSemantic::GameKnowledge,
                },
            );
        }
        for (field, text) in &knowledge.overrides {
            catalog.insert(
                format!("gameKnowledge.heroes.{hero}.overrides.{field}"),
                EvidenceEntry {
                    value: EvidenceValue::Text(text.clone()),
                    semantic: EvidenceSemantic::GameKnowledge,
                },
            );
        }
        for (index, text) in knowledge.limitations.iter().enumerate() {
            catalog.insert(
                format!("gameKnowledge.heroes.{hero}.limitations.{index}"),
                EvidenceEntry {
                    value: EvidenceValue::Text(text.clone()),
                    semantic: EvidenceSemantic::GameKnowledge,
                },
            );
        }
    }
    for (name, archetype) in &snapshot.game_knowledge.archetypes {
        catalog.insert(
            format!("gameKnowledge.archetypes.{name}.description"),
            EvidenceEntry {
                value: EvidenceValue::Text(archetype.description.clone()),
                semantic: EvidenceSemantic::GameKnowledge,
            },
        );
        for (field, text) in &archetype.stat_expectations {
            catalog.insert(
                format!("gameKnowledge.archetypes.{name}.statExpectations.{field}"),
                EvidenceEntry {
                    value: EvidenceValue::Text(text.clone()),
                    semantic: EvidenceSemantic::GameKnowledge,
                },
            );
        }
    }
    if let Some(map) = &snapshot.game_knowledge.map_profile {
        for (index, text) in map.traits.iter().enumerate() {
            catalog.insert(
                format!("gameKnowledge.map.traits.{index}"),
                EvidenceEntry {
                    value: EvidenceValue::Text(text.clone()),
                    semantic: EvidenceSemantic::GameKnowledge,
                },
            );
        }
    }
    catalog
}

fn add_optional_text(catalog: &mut EvidenceCatalog, path: &str, value: Option<&String>) {
    if let Some(value) = value {
        catalog.insert(
            path.to_string(),
            EvidenceEntry {
                value: EvidenceValue::Text(value.clone()),
                semantic: EvidenceSemantic::RawStat,
            },
        );
    }
}
fn add_optional_integer(catalog: &mut EvidenceCatalog, path: &str, value: Option<i64>) {
    if let Some(value) = value {
        catalog.insert(
            path.to_string(),
            EvidenceEntry {
                value: EvidenceValue::Integer(value),
                semantic: EvidenceSemantic::RawStat,
            },
        );
    }
}

fn add_object<T: Serialize>(
    catalog: &mut EvidenceCatalog,
    prefix: &str,
    value: &T,
    semantic: EvidenceSemantic,
) {
    let value = serde_json::to_value(value).expect("evidence serializes");
    if let Some(object) = value.as_object() {
        for (name, value) in object {
            add_value(catalog, format!("{prefix}.{name}"), value, semantic);
        }
    }
}

fn add_value(
    catalog: &mut EvidenceCatalog,
    path: String,
    value: &Value,
    semantic: EvidenceSemantic,
) {
    match value {
        Value::Object(object) => {
            for (name, child) in object {
                add_value(catalog, format!("{path}.{name}"), child, semantic);
            }
            return;
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                add_value(catalog, format!("{path}.{index}"), child, semantic);
            }
            return;
        }
        _ => {}
    }
    let value = match value {
        Value::Number(number) if number.is_i64() => number.as_i64().map(EvidenceValue::Integer),
        Value::Number(number) => number.as_f64().map(EvidenceValue::Decimal),
        Value::String(text) => Some(EvidenceValue::Text(text.clone())),
        _ => None,
    };
    if let Some(value) = value {
        catalog.insert(path, EvidenceEntry { value, semantic });
    }
}
