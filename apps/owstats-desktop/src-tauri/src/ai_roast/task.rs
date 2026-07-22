use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{knowledge::RoastKnowledge, snapshot::RoastPlayer};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchupTaskProfile {
    pub label: String,
    pub threat_heroes: Vec<String>,
    pub primary_responders: Vec<String>,
    pub secondary_responders: Vec<String>,
    pub evaluation_fields: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UniversalTaskProfile {
    pub label: String,
    pub explanation: String,
    pub evaluation_fields: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskKnowledgeContext {
    pub universal: BTreeMap<String, UniversalTaskProfile>,
    pub assignments: BTreeMap<String, TaskAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskAssignment {
    pub label: String,
    pub threat_subjects: Vec<String>,
    pub primary_responsible_subjects: Vec<String>,
    pub secondary_responsible_subjects: Vec<String>,
    pub evaluation_fields: Vec<String>,
    pub limitations: Vec<String>,
}

pub fn task_knowledge_for_players(
    knowledge: &RoastKnowledge,
    team: &[RoastPlayer],
    opponent: &[RoastPlayer],
) -> TaskKnowledgeContext {
    let assignments = knowledge
        .matchup_tasks
        .iter()
        .filter_map(|(key, profile)| {
            let threat_subjects = opponent
                .iter()
                .filter(|row| profile.threat_heroes.contains(&row.hero))
                .map(|row| row.subject.clone())
                .collect::<Vec<_>>();
            let primary_responsible_subjects = team
                .iter()
                .filter(|row| profile.primary_responders.contains(&row.hero))
                .map(|row| row.subject.clone())
                .collect::<Vec<_>>();
            if threat_subjects.is_empty() || primary_responsible_subjects.is_empty() {
                return None;
            }
            let secondary_responsible_subjects = team
                .iter()
                .filter(|row| profile.secondary_responders.contains(&row.hero))
                .map(|row| row.subject.clone())
                .collect::<Vec<_>>();
            Some((
                key.clone(),
                TaskAssignment {
                    label: profile.label.clone(),
                    threat_subjects,
                    primary_responsible_subjects,
                    secondary_responsible_subjects,
                    evaluation_fields: profile.evaluation_fields.clone(),
                    limitations: profile.limitations.clone(),
                },
            ))
        })
        .collect();
    TaskKnowledgeContext {
        universal: knowledge.universal_tasks.clone(),
        assignments,
    }
}
