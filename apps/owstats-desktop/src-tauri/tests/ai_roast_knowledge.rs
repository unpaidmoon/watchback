use std::collections::BTreeSet;

use owstats_desktop_lib::ai_roast::knowledge::{embedded_knowledge, knowledge_for_match};
use serde_json::Value;

fn lookup_hero_names() -> BTreeSet<String> {
    let lookup: Value =
        serde_json::from_str(include_str!("../../runtime-config/ow-lookups.json")).unwrap();
    lookup["heroes"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(|hero| hero["name"].as_str().map(ToOwned::to_owned))
        .collect()
}

#[test]
fn knowledge_covers_every_reviewed_hero_with_bounded_archetypes() {
    let knowledge = embedded_knowledge().unwrap();
    assert_eq!(knowledge.version, 2);
    assert_eq!(
        knowledge.heroes.keys().cloned().collect::<BTreeSet<_>>(),
        lookup_hero_names()
    );
    assert!(knowledge
        .heroes
        .values()
        .all(|hero| hero.secondary_archetypes.len() <= 1));
    assert!(knowledge.archetypes_by_role("tank").len() <= 3);
    assert!(knowledge.archetypes_by_role("damage").len() <= 3);
    assert!(knowledge.archetypes_by_role("support").len() <= 3);
}

#[test]
fn knowledge_contains_bounded_task_ownership_rules() {
    let knowledge = embedded_knowledge().unwrap();
    assert!(knowledge.universal_tasks.contains_key("damage_conversion"));
    let anti_air = knowledge.matchup_tasks.get("anti_air").unwrap();
    assert!(anti_air.threat_heroes.contains(&"法老之鹰".to_string()));
    assert_eq!(anti_air.primary_responders.first().unwrap(), "士兵：76");
    assert!(anti_air.secondary_responders.contains(&"D.Va".to_string()));
}

#[test]
fn knowledge_contains_high_confidence_hero_and_map_rules() {
    let knowledge = embedded_knowledge().unwrap();
    assert!(knowledge
        .hero("禅雅塔")
        .unwrap()
        .overrides
        .contains_key("healing"));
    assert!(knowledge
        .hero("天使")
        .unwrap()
        .overrides
        .contains_key("damage"));
    assert!(knowledge
        .hero("生命之梭")
        .unwrap()
        .limitations
        .iter()
        .any(|value| value.contains("生命之握")));
    assert!(knowledge
        .map("阿特利斯")
        .unwrap()
        .traits
        .contains(&"交火距离偏短".to_string()));
    assert!(knowledge
        .map("伊利奥斯")
        .unwrap()
        .traits
        .contains(&"拉扯快".to_string()));
}

#[test]
fn match_context_only_contains_knowledge_for_present_heroes_and_map() {
    let knowledge = embedded_knowledge().unwrap();
    let context = knowledge_for_match(
        &knowledge,
        [
            "禅雅塔".to_string(),
            "天使".to_string(),
            "不存在英雄".to_string(),
        ]
        .into_iter(),
        Some("伊利奥斯"),
    );
    assert_eq!(context.hero_overrides.len(), 2);
    assert!(context.hero_overrides.contains_key("禅雅塔"));
    assert!(context.hero_overrides.contains_key("天使"));
    assert!(context
        .map_profile
        .as_ref()
        .unwrap()
        .traits
        .contains(&"拉扯快".to_string()));
    let json = serde_json::to_string(&context).unwrap();
    assert!(!json.contains("reviewSources"));
    assert!(!json.contains("生命之梭"));
}
