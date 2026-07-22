use owstats_desktop_lib::ai_roast::knowledge::embedded_knowledge;
use owstats_desktop_lib::ai_roast::snapshot::{DerivedMetrics, PlayerStats, RoastPlayer};
use owstats_desktop_lib::ai_roast::task::task_knowledge_for_players;

fn player(subject: &str, hero: &str, role: &str) -> RoastPlayer {
    RoastPlayer {
        subject: subject.to_string(),
        is_current_player: false,
        hero: hero.to_string(),
        role: Some(role.to_string()),
        stats: PlayerStats::default(),
        derived: DerivedMetrics::default(),
    }
}

#[test]
fn anti_air_assigns_primary_responder_before_secondary_helpers() {
    let knowledge = embedded_knowledge().unwrap();
    let team = vec![
        player("我方士兵：76", "士兵：76", "damage"),
        player("我方D.Va", "D.Va", "tank"),
        player("我方安娜", "安娜", "support"),
    ];
    let opponent = vec![player("敌方法老之鹰", "法老之鹰", "damage")];
    let context = task_knowledge_for_players(&knowledge, &team, &opponent);
    let anti_air = context.assignments.get("anti_air").unwrap();
    assert_eq!(anti_air.threat_subjects, vec!["敌方法老之鹰"]);
    assert_eq!(anti_air.primary_responsible_subjects, vec!["我方士兵：76"]);
    assert_eq!(
        anti_air.secondary_responsible_subjects,
        vec!["我方D.Va", "我方安娜"]
    );
}

#[test]
fn matchup_task_is_omitted_without_a_primary_responder() {
    let knowledge = embedded_knowledge().unwrap();
    let team = vec![player("我方安娜", "安娜", "support")];
    let opponent = vec![player("敌方法老之鹰", "法老之鹰", "damage")];
    let context = task_knowledge_for_players(&knowledge, &team, &opponent);
    assert!(!context.assignments.contains_key("anti_air"));
}
