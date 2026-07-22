use owstats_desktop_lib::ai_roast::knowledge::{
    ArchetypeProfile, KnowledgeContext, RuntimeHeroKnowledge,
};
use owstats_desktop_lib::ai_roast::report::{
    evidence_catalog, ActorImpact, AiRoastNarrative, AiRoastReport, ClaimType, EvidenceSemantic,
    EvidenceValue, KeyActorClaim, NarrativeAnalysis, RoastClaim, RoastInsight, RoastVerdict,
    StoredRoastPayload, TeamInsight, TeamInsightKind,
};
use owstats_desktop_lib::ai_roast::snapshot::{
    ArenaContext, ArenaRoundResult, DerivedMetrics, PlayerStats, RoastMatchContext, RoastMatchKind,
    RoastPlayer, RoastScoreboard, RoastSnapshot, RoleMatchup, INPUT_SCHEMA_VERSION,
};
use owstats_desktop_lib::ai_roast::task::{TaskAssignment, TaskKnowledgeContext};
use owstats_desktop_lib::ai_roast::validator::{
    validate_narrative_output, validate_report_output, RoastValidationError,
};
use std::collections::BTreeMap;

fn insight(title: &str) -> RoastInsight {
    RoastInsight {
        title: title.to_string(),
        commentary: "合成分析。".to_string(),
        claim_type: ClaimType::DirectFact,
        evidence_refs: vec!["我方士兵：76.stats.assists".to_string()],
    }
}

fn synthetic_snapshot() -> RoastSnapshot {
    let soldier = RoastPlayer {
        subject: "我方士兵：76".to_string(),
        is_current_player: true,
        hero: "士兵：76".to_string(),
        role: Some("damage".to_string()),
        stats: PlayerStats {
            kills: Some(12),
            assists: Some(6),
            deaths: Some(4),
            damage: Some(10_000),
            healing: Some(0),
            mitigation: Some(0),
            final_hits: Some(7),
            objective_time_seconds: Some(30),
            healing_received: Some(2_000),
            damage_taken: Some(6_000),
        },
        derived: DerivedMetrics {
            team_damage_rank: Some(1),
            match_damage_rank: Some(2),
            team_damage_share: Some(0.4),
            ..Default::default()
        },
    };
    let enemy = RoastPlayer {
        subject: "敌方艾什".to_string(),
        is_current_player: false,
        hero: "艾什".to_string(),
        role: Some("damage".to_string()),
        stats: PlayerStats {
            damage: Some(12_000),
            final_hits: Some(9),
            ..Default::default()
        },
        derived: DerivedMetrics::default(),
    };
    let mut hero_overrides = BTreeMap::new();
    hero_overrides.insert(
        "禅雅塔".to_string(),
        RuntimeHeroKnowledge {
            primary_archetype: Some("进攻型".to_string()),
            secondary_archetypes: Vec::new(),
            overrides: BTreeMap::from([(
                "healing".to_string(),
                "治疗较低不能直接判定失职".to_string(),
            )]),
            limitations: Vec::new(),
        },
    );
    let mut archetypes = BTreeMap::new();
    archetypes.insert(
        "进攻型".to_string(),
        ArchetypeProfile {
            role: "support".to_string(),
            description: "进攻支援".to_string(),
            stat_expectations: BTreeMap::new(),
        },
    );
    RoastSnapshot {
        schema_version: INPUT_SCHEMA_VERSION,
        match_kind: RoastMatchKind::Arena,
        match_context: RoastMatchContext {
            result: Some("win".to_string()),
            team_score: Some(4),
            opponent_score: Some(3),
            ..Default::default()
        },
        current_player: "我方士兵：76".to_string(),
        scoreboard: RoastScoreboard {
            team: vec![soldier],
            opponent: vec![enemy],
        },
        role_matchups: vec![RoleMatchup {
            role: "damage".to_string(),
            team_subjects: vec!["我方士兵：76".to_string()],
            opponent_subjects: vec!["敌方艾什".to_string()],
            team_totals: PlayerStats {
                final_hits: Some(7),
                ..Default::default()
            },
            opponent_totals: PlayerStats {
                final_hits: Some(9),
                ..Default::default()
            },
            deltas: PlayerStats {
                final_hits: Some(-2),
                ..Default::default()
            },
        }],
        current_player_baseline: None,
        streak: None,
        game_knowledge: KnowledgeContext {
            archetypes,
            hero_overrides,
            map_profile: None,
        },
        task_knowledge: TaskKnowledgeContext::default(),
        arena: Some(ArenaContext {
            round_results: vec![ArenaRoundResult {
                round: 4,
                result: "win".to_string(),
                score_after: "2:2".to_string(),
            }],
            final_worth: BTreeMap::from([("我方士兵：76".to_string(), 7050)]),
        }),
    }
}

fn synthetic_report() -> AiRoastReport {
    AiRoastReport {
        verdict: RoastVerdict {
            title: "治疗在线，火力断电".to_string(),
            summary: "合成总评。".to_string(),
        },
        decisive_factors: vec![insight("胜负手一"), insight("胜负手二")],
        current_player: insight("你这局"),
        team_and_matchups: vec![TeamInsight {
            kind: TeamInsightKind::RoleMatchup,
            insight: insight("职责对位"),
        }],
        closing_roast: "合成收尾。".to_string(),
    }
}

#[test]
fn report_requires_five_section_contract() {
    let report = synthetic_report();
    assert!(!report.verdict.title.is_empty());
    assert!((2..=3).contains(&report.decisive_factors.len()));
    assert!((1..=3).contains(&report.team_and_matchups.len()));
    assert!(!report.closing_roast.is_empty());

    let value = serde_json::to_value(report).unwrap();
    assert!(value.get("decisiveFactors").is_some());
    assert!(value.get("currentPlayer").is_some());
    assert!(value.get("teamAndMatchups").is_some());
    assert_eq!(value["teamAndMatchups"][0]["type"], "role_matchup");
}

#[test]
fn evidence_catalog_uses_stable_public_paths_and_semantics() {
    let catalog = evidence_catalog(&synthetic_snapshot());
    assert_eq!(
        catalog["我方士兵：76.stats.damage"].value,
        EvidenceValue::Integer(10_000)
    );
    assert_eq!(catalog["match.teamScore"].value, EvidenceValue::Integer(4));
    assert_eq!(
        catalog["match.opponentScore"].value,
        EvidenceValue::Integer(3)
    );
    assert_eq!(
        catalog["我方士兵：76.derived.teamDamageRank"].semantic,
        EvidenceSemantic::Rank
    );
    assert_eq!(
        catalog["roleMatchups.damage.deltas.finalHits"].value,
        EvidenceValue::Integer(-2)
    );
    assert_eq!(
        catalog["arena.roundResults.4.result"].semantic,
        EvidenceSemantic::RoundResult
    );
    assert_eq!(
        catalog["arena.finalWorth.我方士兵：76"].semantic,
        EvidenceSemantic::FinalWorth
    );
    assert_eq!(
        catalog["gameKnowledge.heroes.禅雅塔.overrides.healing"].semantic,
        EvidenceSemantic::GameKnowledge
    );
    let paths = catalog.keys().cloned().collect::<Vec<_>>().join(" ");
    for forbidden in ["bnet", "guid", "rawJson", "path", "SyntheticPlayer"] {
        assert!(!paths.contains(forbidden));
    }
}

fn valid_report_json() -> String {
    serde_json::to_string(&synthetic_report()).unwrap()
}

#[test]
fn report_rejects_invalid_section_counts_and_unknown_or_duplicate_evidence() {
    let snapshot = synthetic_snapshot();
    let mut report = synthetic_report();
    report.decisive_factors.truncate(1);
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::InvalidSectionCount)
    );
    let mut report = synthetic_report();
    report.current_player.evidence_refs = vec!["missing.path".to_string()];
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::UnknownEvidence)
    );
    let mut report = synthetic_report();
    report.current_player.evidence_refs = vec![
        "我方士兵：76.stats.damage".to_string(),
        "我方士兵：76.stats.damage".to_string(),
    ];
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::DuplicateEvidence)
    );
}

#[test]
fn report_rejects_empty_and_overlong_visible_text() {
    let snapshot = synthetic_snapshot();
    let mut report = synthetic_report();
    report.verdict.title.clear();
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::EmptyField)
    );
    let mut report = synthetic_report();
    report.current_player.commentary = "长".repeat(161);
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::TextTooLong)
    );
}

#[test]
fn report_rejects_unsupported_numbers_and_comparisons() {
    let snapshot = synthetic_snapshot();
    let mut report = synthetic_report();
    report.current_player.commentary = "打出了99999点伤害。".to_string();
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::UnsupportedNumber)
    );
    let mut report = synthetic_report();
    report.current_player.commentary = "伤害全场第一。".to_string();
    report.current_player.evidence_refs = vec!["我方士兵：76.derived.matchDamageRank".to_string()];
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::UnsupportedClaim)
    );
}

#[test]
fn report_accepts_grounded_numbers_percentages_and_game_context() {
    let snapshot = synthetic_snapshot();
    let mut report = synthetic_report();
    report.current_player.commentary = "伤害10000，占队伍40%，但只是全场第二。".to_string();
    report.current_player.claim_type = ClaimType::StatisticalInterpretation;
    report.current_player.evidence_refs = vec![
        "我方士兵：76.stats.damage".to_string(),
        "我方士兵：76.derived.teamDamageShare".to_string(),
        "我方士兵：76.derived.matchDamageRank".to_string(),
    ];
    report.team_and_matchups[0].insight.commentary = "结合进攻型支援知识解释治疗。".to_string();
    report.team_and_matchups[0].insight.claim_type = ClaimType::GameContextInference;
    report.team_and_matchups[0].insight.evidence_refs = vec![
        "我方士兵：76.stats.damage".to_string(),
        "gameKnowledge.heroes.禅雅塔.overrides.healing".to_string(),
    ];
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot).unwrap(),
        report
    );
    assert_eq!(
        validate_report_output(&valid_report_json(), &snapshot).unwrap(),
        synthetic_report()
    );
}

#[test]
fn report_accepts_normally_rounded_whole_percentages() {
    let mut snapshot = synthetic_snapshot();
    snapshot.scoreboard.team[0].derived.team_damage_share = Some(0.396);
    let mut report = synthetic_report();
    report.current_player.commentary = "伤害占队伍40%。".to_string();
    report.current_player.claim_type = ClaimType::StatisticalInterpretation;
    report.current_player.evidence_refs = vec!["我方士兵：76.derived.teamDamageShare".to_string()];
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot).unwrap(),
        report
    );
}

#[test]
fn chinese_grammar_is_not_a_stat_but_chinese_kills_are_checked() {
    let snapshot = synthetic_snapshot();
    let mut report = synthetic_report();
    report.current_player.commentary = "一人扛线不是额外战绩数字，十二杀才是。".to_string();
    report.current_player.evidence_refs = vec!["我方士兵：76.stats.kills".to_string()];
    assert!(validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot).is_ok());

    report.current_player.commentary = "一名队友可以是自然语法，十三杀仍需证据。".to_string();
    assert_eq!(
        validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::UnsupportedNumber)
    );
}

#[test]
fn compact_thousand_and_ten_thousand_units_match_referenced_values() {
    let mut snapshot = synthetic_snapshot();
    snapshot.scoreboard.team[0].stats.damage = Some(12_345);
    let mut report = synthetic_report();
    report.current_player.commentary = "伤害约1.2万。".to_string();
    report.current_player.evidence_refs = vec!["我方士兵：76.stats.damage".to_string()];
    assert!(validate_report_output(&serde_json::to_string(&report).unwrap(), &snapshot).is_ok());
}

fn narrative_claim(judgment: &str) -> RoastClaim {
    RoastClaim {
        judgment: judgment.to_string(),
        claim_type: ClaimType::StatisticalInterpretation,
        evidence_refs: vec!["我方士兵：76.stats.damage".to_string()],
    }
}

fn synthetic_narrative() -> AiRoastNarrative {
    AiRoastNarrative {
        analysis: NarrativeAnalysis {
            match_thesis: RoastClaim {
                judgment: "我方输出没有把伤害转成同等击杀收益".to_string(),
                claim_type: ClaimType::StatisticalInterpretation,
                evidence_refs: vec![
                    "我方士兵：76.stats.damage".to_string(),
                    "敌方艾什.stats.damage".to_string(),
                ],
            },
            current_player: RoastClaim {
                judgment: "当前玩家有输出但转化不足".to_string(),
                claim_type: ClaimType::StatisticalInterpretation,
                evidence_refs: vec![
                    "我方士兵：76.stats.damage".to_string(),
                    "我方士兵：76.stats.finalHits".to_string(),
                ],
            },
            key_actors: Vec::new(),
            supporting_factors: Vec::new(),
        },
        title: "伤害账单很豪华，结账没带钱包".to_string(),
        commentary: "这个士兵：76到底在忙什么？输出账单看着挺热闹，真正该收掉的人却没留下多少结果，这把说他交了张白卷不算冤。".to_string(),
    }
}

#[test]
fn narrative_report_has_hidden_plan_title_and_commentary() {
    let report = AiRoastNarrative {
        analysis: NarrativeAnalysis {
            match_thesis: narrative_claim("我方没有处理关键空中威胁"),
            current_player: narrative_claim("当前玩家有贡献但转化不足"),
            key_actors: vec![KeyActorClaim {
                subject: "我方士兵：76".to_string(),
                impact: ActorImpact::Negative,
                claim: narrative_claim("主要对空任务没有形成结果"),
            }],
            supporting_factors: Vec::new(),
        },
        title: "法老之鹰在天上续了年卡".to_string(),
        commentary: "我方并非没有输出，但最该处理的威胁一直活得很舒服。".to_string(),
    };
    let value = serde_json::to_value(&report).unwrap();
    assert!(value["analysis"]["matchThesis"].is_object());
    assert_eq!(value["title"], "法老之鹰在天上续了年卡");
    assert!(value.get("decisiveFactors").is_none());
}

#[test]
fn stored_payload_reads_legacy_five_section_json() {
    let json = serde_json::to_string(&synthetic_report()).unwrap();
    let payload: StoredRoastPayload = serde_json::from_str(&json).unwrap();
    assert!(matches!(payload, StoredRoastPayload::Legacy(_)));
}

#[test]
fn task_assignments_are_available_as_game_knowledge_evidence() {
    let mut snapshot = synthetic_snapshot();
    snapshot.task_knowledge.assignments.insert(
        "anti_air".to_string(),
        TaskAssignment {
            label: "对空与空中威胁处理".to_string(),
            threat_subjects: vec!["敌方法老之鹰".to_string()],
            primary_responsible_subjects: vec!["我方士兵：76".to_string()],
            secondary_responsible_subjects: Vec::new(),
            evaluation_fields: vec!["finalHits".to_string()],
            limitations: vec!["不能断言具体攻击目标。".to_string()],
        },
    );
    let catalog = evidence_catalog(&snapshot);
    let entry = catalog
        .get("taskKnowledge.assignments.anti_air.primaryResponsibleSubjects.0")
        .unwrap();
    assert_eq!(entry.semantic, EvidenceSemantic::GameKnowledge);
    assert_eq!(entry.value, EvidenceValue::Text("我方士兵：76".to_string()));
}

#[test]
fn narrative_requires_one_thesis_and_at_most_two_supporting_factors() {
    let snapshot = synthetic_snapshot();
    let mut report = synthetic_narrative();
    report.analysis.supporting_factors = vec![
        narrative_claim("辅助因素一"),
        narrative_claim("辅助因素二"),
        narrative_claim("辅助因素三"),
    ];
    assert_eq!(
        validate_narrative_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::InvalidSectionCount)
    );
}

#[test]
fn narrative_allows_game_slang_but_rejects_identity_attacks() {
    let snapshot = synthetic_snapshot();
    let mut report = synthetic_narrative();
    assert!(validate_narrative_output(&serde_json::to_string(&report).unwrap(), &snapshot).is_ok());
    report.commentary = "这个玩家就是蠢货。".to_string();
    assert_eq!(
        validate_narrative_output(&serde_json::to_string(&report).unwrap(), &snapshot),
        Err(RoastValidationError::Unfriendly)
    );
}
