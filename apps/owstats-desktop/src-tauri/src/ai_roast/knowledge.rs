use std::collections::{BTreeMap, BTreeSet};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};

use super::task::{MatchupTaskProfile, UniversalTaskProfile};

const RAW_KNOWLEDGE: &str = include_str!("../../../runtime-config/ai-roast-knowledge.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoastKnowledge {
    pub version: i64,
    pub archetypes: BTreeMap<String, ArchetypeProfile>,
    pub heroes: BTreeMap<String, HeroKnowledge>,
    pub maps: BTreeMap<String, MapProfile>,
    pub universal_tasks: BTreeMap<String, UniversalTaskProfile>,
    pub matchup_tasks: BTreeMap<String, MatchupTaskProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArchetypeProfile {
    pub role: String,
    pub description: String,
    pub stat_expectations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HeroKnowledge {
    pub primary_archetype: Option<String>,
    #[serde(default)]
    pub secondary_archetypes: Vec<String>,
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub limitations: Vec<String>,
    #[serde(default)]
    pub review_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeroKnowledge {
    pub primary_archetype: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary_archetypes: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl From<&HeroKnowledge> for RuntimeHeroKnowledge {
    fn from(value: &HeroKnowledge) -> Self {
        Self {
            primary_archetype: value.primary_archetype.clone(),
            secondary_archetypes: value.secondary_archetypes.clone(),
            overrides: value.overrides.clone(),
            limitations: value.limitations.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MapProfile {
    pub traits: Vec<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeContext {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub archetypes: BTreeMap<String, ArchetypeProfile>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hero_overrides: BTreeMap<String, RuntimeHeroKnowledge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_profile: Option<MapProfile>,
}

impl RoastKnowledge {
    pub fn hero(&self, name: &str) -> Option<&HeroKnowledge> {
        self.heroes.get(name)
    }

    pub fn map(&self, name: &str) -> Option<&MapProfile> {
        self.maps.get(name)
    }

    pub fn archetypes_by_role(&self, role: &str) -> Vec<(&String, &ArchetypeProfile)> {
        self.archetypes
            .iter()
            .filter(|(_, value)| value.role == role)
            .collect()
    }
}

pub fn embedded_knowledge() -> Result<RoastKnowledge> {
    let knowledge: RoastKnowledge =
        serde_json::from_str(RAW_KNOWLEDGE).context("内置 AI 锐评知识库无效")?;
    ensure!(knowledge.version == 2, "内置 AI 锐评知识库版本无效");
    for (name, hero) in &knowledge.heroes {
        ensure!(
            hero.secondary_archetypes.len() <= 1,
            "英雄{name}的辅助分类过多"
        );
        for archetype in hero
            .primary_archetype
            .iter()
            .chain(hero.secondary_archetypes.iter())
        {
            ensure!(
                knowledge.archetypes.contains_key(archetype),
                "英雄{name}引用未知分类"
            );
        }
    }
    for role in ["tank", "damage", "support"] {
        ensure!(
            knowledge.archetypes_by_role(role).len() <= 3,
            "职责{role}的分类过多"
        );
    }
    for (key, task) in &knowledge.matchup_tasks {
        ensure!(!task.threat_heroes.is_empty(), "任务{key}缺少威胁英雄");
        ensure!(
            !task.primary_responders.is_empty(),
            "任务{key}缺少主要承担者"
        );
        ensure!(!task.evaluation_fields.is_empty(), "任务{key}缺少评价字段");
        for heroes in [
            &task.threat_heroes,
            &task.primary_responders,
            &task.secondary_responders,
        ] {
            ensure!(
                heroes.iter().collect::<BTreeSet<_>>().len() == heroes.len(),
                "任务{key}包含重复英雄"
            );
        }
    }
    Ok(knowledge)
}

pub fn knowledge_for_match(
    knowledge: &RoastKnowledge,
    heroes: impl Iterator<Item = String>,
    map_name: Option<&str>,
) -> KnowledgeContext {
    let mut context = KnowledgeContext::default();
    let mut used_archetypes = BTreeSet::new();
    for hero_name in heroes.collect::<BTreeSet<_>>() {
        let Some(hero) = knowledge.heroes.get(&hero_name) else {
            continue;
        };
        if let Some(value) = &hero.primary_archetype {
            used_archetypes.insert(value.clone());
        }
        used_archetypes.extend(hero.secondary_archetypes.iter().cloned());
        context
            .hero_overrides
            .insert(hero_name, RuntimeHeroKnowledge::from(hero));
    }
    for name in used_archetypes {
        if let Some(profile) = knowledge.archetypes.get(&name) {
            context.archetypes.insert(name, profile.clone());
        }
    }
    context.map_profile = map_name.and_then(|name| {
        knowledge
            .maps
            .get(name)
            .or_else(|| {
                name.split_once(" - ")
                    .and_then(|(parent, _)| knowledge.maps.get(parent))
            })
            .cloned()
    });
    context
}
