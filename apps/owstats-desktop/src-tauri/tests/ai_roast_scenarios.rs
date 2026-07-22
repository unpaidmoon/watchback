use std::collections::BTreeMap;

use owstats_desktop_lib::ai_roast::knowledge::{
    KnowledgeContext, MapProfile, RuntimeHeroKnowledge,
};
use owstats_desktop_lib::ai_roast::report::{
    AiRoastReport, ClaimType, RoastInsight, RoastVerdict, TeamInsight, TeamInsightKind,
};
use owstats_desktop_lib::ai_roast::snapshot::{
    ArenaContext, ArenaRoundResult, DerivedMetrics, PlayerStats, RoastMatchContext, RoastMatchKind,
    RoastPlayer, RoastScoreboard, RoastSnapshot, INPUT_SCHEMA_VERSION,
};
use owstats_desktop_lib::ai_roast::task::TaskKnowledgeContext;
use owstats_desktop_lib::ai_roast::validator::{validate_report_output, RoastValidationError};

fn player(subject: &str, hero: &str, current: bool) -> RoastPlayer {
    RoastPlayer {
        subject: subject.into(),
        is_current_player: current,
        hero: hero.into(),
        role: Some(
            if matches!(hero, "天使" | "生命之梭" | "禅雅塔") {
                "support"
            } else {
                "damage"
            }
            .into(),
        ),
        stats: PlayerStats {
            kills: Some(8),
            assists: Some(7),
            deaths: Some(4),
            damage: Some(8_000),
            healing: Some(2_000),
            mitigation: Some(0),
            final_hits: Some(5),
            objective_time_seconds: Some(40),
            healing_received: Some(1_500),
            damage_taken: Some(5_000),
        },
        derived: DerivedMetrics::default(),
    }
}

fn snapshot(current_hero: &str) -> RoastSnapshot {
    let current_subject = format!("我方{current_hero}");
    RoastSnapshot {
        schema_version: INPUT_SCHEMA_VERSION,
        match_kind: RoastMatchKind::Standard,
        match_context: RoastMatchContext::default(),
        current_player: current_subject.clone(),
        scoreboard: RoastScoreboard {
            team: vec![player(&current_subject, current_hero, true)],
            opponent: vec![player("敌方卡西迪", "卡西迪", false)],
        },
        role_matchups: vec![],
        current_player_baseline: None,
        streak: None,
        game_knowledge: KnowledgeContext {
            archetypes: BTreeMap::new(),
            hero_overrides: BTreeMap::new(),
            map_profile: None,
        },
        task_knowledge: TaskKnowledgeContext::default(),
        arena: None,
    }
}

fn insight(subject: &str, title: &str, commentary: &str) -> RoastInsight {
    RoastInsight {
        title: title.into(),
        commentary: commentary.into(),
        claim_type: ClaimType::DirectFact,
        evidence_refs: vec![format!("{subject}.stats.assists")],
    }
}

fn report(subject: &str) -> AiRoastReport {
    AiRoastReport {
        verdict: RoastVerdict {
            title: "合成判词".into(),
            summary: "只验证匿名游戏理解边界。".into(),
        },
        decisive_factors: vec![
            insight(subject, "胜负手甲", "助攻提供了直接事实。"),
            insight(subject, "胜负手乙", "助攻仍是可核验事实。"),
        ],
        current_player: insight(subject, "你这局", "表现基于合成事实。"),
        team_and_matchups: vec![TeamInsight {
            kind: TeamInsightKind::RoleMatchup,
            insight: insight(subject, "职责对位", "这里只陈述合成事实。"),
        }],
        closing_roast: "事实可以尖锐，因果不能乱接。".into(),
    }
}

fn validate(report: &AiRoastReport, snapshot: &RoastSnapshot) -> Result<(), RoastValidationError> {
    validate_report_output(&serde_json::to_string(report).unwrap(), snapshot).map(|_| ())
}

fn add_hero_knowledge(snapshot: &mut RoastSnapshot, hero: &str, key: &str, value: &str) {
    snapshot.game_knowledge.hero_overrides.insert(
        hero.into(),
        RuntimeHeroKnowledge {
            primary_archetype: None,
            secondary_archetypes: vec![],
            overrides: BTreeMap::from([(key.into(), value.into())]),
            limitations: vec![],
        },
    );
}

#[test]
fn mercy_survival_environment_requires_game_context_inference() {
    let mut snapshot = snapshot("天使");
    snapshot
        .scoreboard
        .opponent
        .push(player("敌方艾什", "艾什", false));
    add_hero_knowledge(
        &mut snapshot,
        "天使",
        "survival",
        "面对中远程枪线时生存压力会提高。",
    );
    let mut valid = report("我方天使");
    valid.current_player = RoastInsight {
        title: "生存环境".into(),
        commentary: "结合枪线环境看待阵亡。".into(),
        claim_type: ClaimType::GameContextInference,
        evidence_refs: vec![
            "我方天使.stats.deaths".into(),
            "gameKnowledge.heroes.天使.overrides.survival".into(),
        ],
    };
    assert_eq!(validate(&valid, &snapshot), Ok(()));
    let mut invalid = valid;
    invalid.current_player.claim_type = ClaimType::StatisticalInterpretation;
    assert_eq!(
        validate(&invalid, &snapshot),
        Err(RoastValidationError::UnsupportedClaim)
    );
}

#[test]
fn anti_air_task_allows_bounded_result_but_rejects_absolute_duel_claim() {
    let mut snapshot = snapshot("士兵：76");
    snapshot.scoreboard.opponent[0] = player("敌方法老之鹰", "法老之鹰", false);
    add_hero_knowledge(
        &mut snapshot,
        "法老之鹰",
        "airThreat",
        "持续空中威胁需要枪位回应。",
    );
    let mut valid = report("我方士兵：76");
    valid.team_and_matchups[0].insight = RoastInsight {
        title: "对空任务".into(),
        commentary: "对空任务没有形成结果。".into(),
        claim_type: ClaimType::GameContextInference,
        evidence_refs: vec![
            "我方士兵：76.stats.finalHits".into(),
            "gameKnowledge.heroes.法老之鹰.overrides.airThreat".into(),
        ],
    };
    assert_eq!(validate(&valid, &snapshot), Ok(()));
    let mut invalid = valid;
    invalid.team_and_matchups[0].insight.commentary = "整局没打过法老之鹰。".into();
    assert_eq!(
        validate(&invalid, &snapshot),
        Err(RoastValidationError::UnsupportedClaim)
    );
}

#[test]
fn offensive_support_and_low_damage_support_roles_are_not_judged_by_one_rank() {
    let mut zen = snapshot("禅雅塔");
    zen.scoreboard.team[0].derived.team_healing_rank = Some(5);
    add_hero_knowledge(&mut zen, "禅雅塔", "healing", "治疗较低不能直接判定失职。");
    let mut valid = report("我方禅雅塔");
    valid.current_player = RoastInsight {
        title: "进攻支援".into(),
        commentary: "治疗排名靠后不能单独判定失职。".into(),
        claim_type: ClaimType::GameContextInference,
        evidence_refs: vec![
            "我方禅雅塔.derived.teamHealingRank".into(),
            "gameKnowledge.heroes.禅雅塔.overrides.healing".into(),
        ],
    };
    assert_eq!(validate(&valid, &zen), Ok(()));
    let mut invalid = report("我方禅雅塔");
    invalid.current_player.commentary = "治疗垫底，所以本职失职。".into();
    invalid.current_player.claim_type = ClaimType::StatisticalInterpretation;
    invalid.current_player.evidence_refs = vec!["我方禅雅塔.derived.teamHealingRank".into()];
    assert_eq!(
        validate(&invalid, &zen),
        Err(RoastValidationError::UnsupportedClaim)
    );

    for hero in ["天使", "生命之梭"] {
        let mut support = snapshot(hero);
        support.scoreboard.team[0].derived.team_damage_rank = Some(5);
        add_hero_knowledge(&mut support, hero, "damage", "低伤害符合该支援的职责结构。");
        let subject = format!("我方{hero}");
        let mut invalid = report(&subject);
        invalid.current_player.commentary = "伤害垫底，拖累了全队。".into();
        invalid.current_player.claim_type = ClaimType::StatisticalInterpretation;
        invalid.current_player.evidence_refs = vec![format!("{subject}.derived.teamDamageRank")];
        assert_eq!(
            validate(&invalid, &support),
            Err(RoastValidationError::UnsupportedClaim)
        );
    }
}

#[test]
fn highest_objective_time_must_be_weighed_without_becoming_a_blanket_excuse() {
    let mut snapshot = snapshot("士兵：76");
    let current = &mut snapshot.scoreboard.team[0];
    current.derived.team_objective_time_rank = Some(1);
    current.derived.team_damage_rank = Some(3);
    current.derived.team_kills_rank = Some(3);
    let mut valid = report("我方士兵：76");
    valid.current_player.commentary = "目标时间全队最高，也要结合稍低的战斗数据评价。".into();
    valid.current_player.claim_type = ClaimType::StatisticalInterpretation;
    valid.current_player.evidence_refs = vec![
        "我方士兵：76.derived.teamObjectiveTimeRank".into(),
        "我方士兵：76.derived.teamDamageRank".into(),
    ];
    assert_eq!(validate(&valid, &snapshot), Ok(()));
    let mut missing_objective = report("我方士兵：76");
    missing_objective.current_player.commentary = "战斗数据稍低。".into();
    missing_objective.current_player.claim_type = ClaimType::StatisticalInterpretation;
    missing_objective.current_player.evidence_refs =
        vec!["我方士兵：76.derived.teamDamageRank".into()];
    assert_eq!(
        validate(&missing_objective, &snapshot),
        Err(RoastValidationError::UnsupportedClaim)
    );

    let current = &mut snapshot.scoreboard.team[0];
    current.derived.team_damage_rank = Some(5);
    current.derived.team_kills_rank = Some(5);
    current.derived.team_final_hits_rank = Some(5);
    let mut excuse = report("我方士兵：76");
    excuse.current_player.commentary = "目标时间最高，所以战斗数据可以免责。".into();
    excuse.current_player.claim_type = ClaimType::StatisticalInterpretation;
    excuse.current_player.evidence_refs = vec!["我方士兵：76.derived.teamObjectiveTimeRank".into()];
    assert_eq!(
        validate(&excuse, &snapshot),
        Err(RoastValidationError::UnsupportedClaim)
    );
}

#[test]
fn short_map_supports_dive_context_but_does_not_decide_performance() {
    let mut snapshot = snapshot("猎空");
    snapshot.game_knowledge.map_profile = Some(MapProfile {
        traits: vec!["短图，高参与更能体现狗位节奏".into()],
        limitations: vec!["地图本身不能判定表现好坏".into()],
    });
    let mut valid = report("我方猎空");
    valid.current_player = RoastInsight {
        title: "短图参与".into(),
        commentary: "高参与得到短图上下文支持。".into(),
        claim_type: ClaimType::GameContextInference,
        evidence_refs: vec![
            "我方猎空.stats.assists".into(),
            "gameKnowledge.map.traits.0".into(),
        ],
    };
    assert_eq!(validate(&valid, &snapshot), Ok(()));
    let mut invalid = valid;
    invalid.current_player.commentary = "因为阿特利斯是短图，所以表现优秀。".into();
    assert_eq!(
        validate(&invalid, &snapshot),
        Err(RoastValidationError::UnsupportedClaim)
    );
}

#[test]
fn arena_comeback_and_final_worth_are_context_not_causation() {
    let mut snapshot = snapshot("士兵：76");
    snapshot.match_kind = RoastMatchKind::Arena;
    snapshot.arena = Some(ArenaContext {
        round_results: vec![
            ArenaRoundResult {
                round: 4,
                result: "win".into(),
                score_after: "1:3".into(),
            },
            ArenaRoundResult {
                round: 7,
                result: "win".into(),
                score_after: "4:3".into(),
            },
        ],
        final_worth: BTreeMap::from([("我方士兵：76".into(), 7_050), ("敌方卡西迪".into(), 6_400)]),
    });
    let mut valid = report("我方士兵：76");
    valid.decisive_factors[0] = RoastInsight {
        title: "决胜回合".into(),
        commentary: "回合序列呈现让三追四。".into(),
        claim_type: ClaimType::DirectFact,
        evidence_refs: vec![
            "arena.roundResults.4.result".into(),
            "arena.roundResults.7.result".into(),
        ],
    };
    valid.current_player = RoastInsight {
        title: "资金反差".into(),
        commentary: "最终资金第一与普通表现形成反差。".into(),
        claim_type: ClaimType::StatisticalInterpretation,
        evidence_refs: vec![
            "arena.finalWorth.我方士兵：76".into(),
            "我方士兵：76.stats.finalHits".into(),
        ],
    };
    assert_eq!(validate(&valid, &snapshot), Ok(()));
    let mut invalid = valid;
    invalid.current_player.commentary = "因为最有钱，所以完成了翻盘。".into();
    invalid
        .current_player
        .evidence_refs
        .push("arena.roundResults.7.result".into());
    assert_eq!(
        validate(&invalid, &snapshot),
        Err(RoastValidationError::UnsupportedClaim)
    );
}
