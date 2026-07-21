use owstats_desktop_lib::{
    ai_roast::{
        arena::{assert_arena_material_privacy, build_arena_roast_material, ArenaRoastMaterial},
        material::{FactSubject, RoastFactEvidence},
    },
    ArenaDetailSlice, ArenaMatchDetail, PlayerRow,
};
use std::collections::HashSet;

fn player(bnet_id: &str, end_worth: Option<i64>, worth: Option<i64>) -> PlayerRow {
    PlayerRow {
        side: "teammate".to_string(),
        name: format!("Synthetic#{bnet_id}"),
        bnet_id: bnet_id.to_string(),
        rank_label: None,
        rank_key: None,
        hero_guid: "synthetic-hero".to_string(),
        hero_name: "synthetic-hero-name".to_string(),
        kills: 0,
        assists: 0,
        deaths: 0,
        damage: 0,
        healing: 0,
        mitigation: 0,
        final_hits: 0,
        target_time_sec: 0,
        healing_taken: 0,
        damage_taken: 0,
        worth,
        end_worth,
        worth_max: false,
        mod_guids: Vec::new(),
        trait_guids: Vec::new(),
        has_source_max_flags: false,
        kill_max: false,
        hero_damage_max: false,
        cure_max: false,
        resist_damage_max: false,
        perks: Vec::new(),
        hero_stats: Vec::new(),
    }
}

fn slice(
    key: &str,
    result: Option<&str>,
    team_score: Option<i64>,
    opponent_score: Option<i64>,
) -> ArenaDetailSlice {
    ArenaDetailSlice {
        key: key.to_string(),
        label: key.to_string(),
        result: result.map(str::to_string),
        result_label: None,
        team_score,
        opponent_score,
        duration_sec: None,
        begin_ts: None,
        begin_label: None,
        map_guid: None,
        map_name: None,
        teammates: Vec::new(),
        enemies: Vec::new(),
    }
}

fn detail(rounds: &[(Option<&str>, Option<i64>, Option<i64>)]) -> ArenaMatchDetail {
    let final_scores = rounds
        .iter()
        .rev()
        .find_map(|(_, team_score, opponent_score)| Some(((*team_score)?, (*opponent_score)?)))
        .unwrap_or((0, 0));
    let summary_result = match final_scores.0.cmp(&final_scores.1) {
        std::cmp::Ordering::Greater => "win",
        std::cmp::Ordering::Less => "loss",
        std::cmp::Ordering::Equal => "draw",
    };
    ArenaMatchDetail {
        summary: ArenaDetailSlice {
            teammates: vec![
                player("current-synthetic", Some(4_500), Some(99_999)),
                player("teammate-a", Some(5_000), Some(1)),
                player("teammate-b", Some(4_000), Some(1)),
                player("teammate-c", Some(3_000), Some(1)),
                player("teammate-d", Some(2_000), Some(1)),
            ],
            ..slice(
                "summary",
                Some(summary_result),
                Some(final_scores.0),
                Some(final_scores.1),
            )
        },
        rounds: rounds
            .iter()
            .enumerate()
            .map(|(index, (result, team_score, opponent_score))| {
                slice(
                    &format!("round-{}", index + 1),
                    *result,
                    *team_score,
                    *opponent_score,
                )
            })
            .collect(),
    }
}

fn fact<'a>(material: &'a ArenaRoastMaterial, suffix: &str) -> &'a str {
    material
        .candidate_facts
        .iter()
        .find(|fact| fact.id.ends_with(suffix))
        .map(|fact| fact.text.as_str())
        .unwrap_or_else(|| panic!("missing fact {suffix}"))
}

#[test]
fn complete_ordinary_zero_four_loss_has_enough_grounded_material() {
    let mut detail = detail(&[
        (Some("loss"), Some(0), Some(1)),
        (Some("loss"), Some(0), Some(2)),
        (Some("loss"), Some(0), Some(3)),
        (Some("loss"), Some(0), Some(4)),
    ]);
    detail.summary.teammates[4].end_worth = None;

    let material = build_arena_roast_material(&detail, "current-synthetic");

    assert!(material.candidate_facts.len() >= 2);
    assert_eq!(
        fact(&material, "arena_final_round_score"),
        "本场最终以0:4告负"
    );
    assert_eq!(
        fact(&material, "arena_round_record"),
        "完整回合记录为0胜4负"
    );
}

#[test]
fn complete_alternating_sequence_has_enough_material_without_end_worth() {
    let mut detail = detail(&[
        (Some("win"), Some(1), Some(0)),
        (Some("loss"), Some(1), Some(1)),
        (Some("win"), Some(2), Some(1)),
        (Some("loss"), Some(2), Some(2)),
    ]);
    detail.summary.teammates[4].end_worth = None;

    let material = build_arena_roast_material(&detail, "current-synthetic");

    assert!(material.candidate_facts.len() >= 2);
    assert_eq!(
        fact(&material, "arena_final_round_score"),
        "本场最终战成2:2平"
    );
    assert_eq!(
        fact(&material, "arena_round_record"),
        "完整回合记录为2胜2负"
    );
    assert!(material
        .candidate_facts
        .iter()
        .all(|fact| !fact.id.ends_with("arena_final_team_funds_rank")));
}

#[test]
fn first_loses_three_then_wins_four_is_a_grounded_comeback() {
    let material = build_arena_roast_material(
        &detail(&[
            (Some("loss"), Some(0), Some(1)),
            (Some("loss"), Some(0), Some(2)),
            (Some("loss"), Some(0), Some(3)),
            (Some("win"), Some(1), Some(3)),
            (Some("win"), Some(2), Some(3)),
            (Some("win"), Some(3), Some(3)),
            (Some("win"), Some(4), Some(3)),
        ]),
        "current-synthetic",
    );

    assert_eq!(
        fact(&material, "arena_three_down_comeback"),
        "\u{524d}3\u{56de}\u{5408}\u{5168}\u{8d25}\u{ff0c}\u{968f}\u{540e}\u{8fde}\u{8d62}4\u{56de}\u{5408}\u{5b8c}\u{6210}\u{7ffb}\u{76d8}"
    );
}

#[test]
fn lead_then_tie_then_winning_decider_is_recorded_without_causality() {
    let material = build_arena_roast_material(
        &detail(&[
            (Some("win"), Some(1), Some(0)),
            (Some("win"), Some(2), Some(0)),
            (Some("loss"), Some(2), Some(1)),
            (Some("loss"), Some(2), Some(2)),
            (Some("win"), Some(3), Some(2)),
        ]),
        "current-synthetic",
    );

    assert_eq!(
        fact(&material, "arena_won_decider_after_tie"),
        "\u{672c}\u{573a}\u{4e00}\u{5ea6}\u{9886}\u{5148}\u{ff0c}\u{968f}\u{540e}\u{88ab}\u{8ffd}\u{5e73}\u{ff0c}\u{5e76}\u{8d62}\u{4e0b}\u{51b3}\u{80dc}\u{56de}\u{5408}"
    );
    assert!(material.candidate_facts.iter().all(|fact| {
        !fact.text.contains("\u{56e0}\u{4e3a}") && !fact.text.contains("\u{5bfc}\u{81f4}")
    }));
}

#[test]
fn ordinary_consecutive_round_wins_emit_a_stable_streak_fact() {
    let material = build_arena_roast_material(
        &detail(&[
            (Some("loss"), Some(0), Some(1)),
            (Some("win"), Some(1), Some(1)),
            (Some("win"), Some(2), Some(1)),
            (Some("loss"), Some(2), Some(2)),
        ]),
        "current-synthetic",
    );

    assert_eq!(
        fact(&material, "arena_longest_win_streak"),
        "\u{672c}\u{573a}\u{6700}\u{957f}\u{8fde}\u{7eed}\u{8d62}\u{4e0b}2\u{56de}\u{5408}"
    );
    let ids = material
        .candidate_facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), material.candidate_facts.len());
    assert_eq!(
        material
            .candidate_facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "match:arena_final_round_score",
            "match:arena_round_record",
            "match:arena_longest_win_streak",
            "current_player:arena_final_team_funds_rank",
        ]
    );
    assert!(material
        .candidate_facts
        .iter()
        .all(|fact| fact.strength.is_finite() && (0.35..=1.0).contains(&fact.strength)));
}

#[test]
fn draw_missing_and_inconsistent_rounds_break_sequences_and_are_ignored() {
    let material = build_arena_roast_material(
        &detail(&[
            (Some("win"), Some(1), Some(0)),
            (Some("draw"), Some(1), Some(1)),
            (Some("win"), Some(2), Some(1)),
            (None, Some(3), Some(1)),
            (Some("win"), None, None),
            (Some("loss"), Some(4), Some(1)),
        ]),
        "current-synthetic",
    );

    assert!(material.candidate_facts.iter().all(|fact| {
        !fact.id.ends_with("arena_longest_win_streak")
            && !fact.id.ends_with("arena_three_down_comeback")
            && !fact.id.ends_with("arena_won_decider_after_tie")
    }));
}

#[test]
fn reliable_draw_breaks_runs_without_invalidating_complete_round_evidence() {
    let material = build_arena_roast_material(
        &detail(&[
            (Some("win"), Some(1), Some(0)),
            (Some("win"), Some(2), Some(0)),
            (Some("draw"), Some(2), Some(0)),
            (Some("loss"), Some(2), Some(1)),
        ]),
        "current-synthetic",
    );

    assert_eq!(
        fact(&material, "arena_longest_win_streak"),
        "\u{672c}\u{573a}\u{6700}\u{957f}\u{8fde}\u{7eed}\u{8d62}\u{4e0b}2\u{56de}\u{5408}"
    );
}

#[test]
fn unreliable_gap_after_two_wins_suppresses_whole_match_longest_claim() {
    for unreliable_round in [(None, Some(2), Some(0)), (Some("loss"), Some(3), Some(0))] {
        let material = build_arena_roast_material(
            &detail(&[
                (Some("win"), Some(1), Some(0)),
                (Some("win"), Some(2), Some(0)),
                unreliable_round,
            ]),
            "current-synthetic",
        );

        assert!(material
            .candidate_facts
            .iter()
            .all(|fact| !fact.id.ends_with("arena_longest_win_streak")));
    }
}

#[test]
fn summary_contradictions_suppress_sequence_and_turning_point_claims() {
    let mut comeback = detail(&[
        (Some("loss"), Some(0), Some(1)),
        (Some("loss"), Some(0), Some(2)),
        (Some("loss"), Some(0), Some(3)),
        (Some("win"), Some(1), Some(3)),
        (Some("win"), Some(2), Some(3)),
        (Some("win"), Some(3), Some(3)),
        (Some("win"), Some(4), Some(3)),
    ]);
    comeback.summary.team_score = Some(3);
    comeback.summary.opponent_score = Some(4);
    comeback.summary.result = Some("loss".to_string());

    let mut decider = detail(&[
        (Some("win"), Some(1), Some(0)),
        (Some("win"), Some(2), Some(0)),
        (Some("loss"), Some(2), Some(1)),
        (Some("loss"), Some(2), Some(2)),
        (Some("win"), Some(3), Some(2)),
    ]);
    decider.summary.team_score = Some(4);
    decider.summary.opponent_score = Some(2);

    for material in [
        build_arena_roast_material(&comeback, "current-synthetic"),
        build_arena_roast_material(&decider, "current-synthetic"),
    ] {
        assert!(material.candidate_facts.iter().all(|fact| {
            !fact.id.ends_with("arena_longest_win_streak")
                && !fact.id.ends_with("arena_three_down_comeback")
                && !fact.id.ends_with("arena_won_decider_after_tie")
        }));
    }
}

#[test]
fn funds_rank_requires_complete_final_end_worth_and_identity() {
    let complete = build_arena_roast_material(
        &detail(&[(Some("win"), Some(1), Some(0))]),
        "current-synthetic",
    );
    let funds = complete
        .candidate_facts
        .iter()
        .find(|fact| fact.id.ends_with("arena_final_team_funds_rank"))
        .expect("complete final endWorth should produce a rank");
    assert_eq!(funds.subject, FactSubject::CurrentPlayer);
    assert_eq!(funds.evidence, RoastFactEvidence::Rank);
    assert_eq!(
        funds.text,
        "\u{6700}\u{7ec8}\u{8d44}\u{91d1}\u{4f4d}\u{5217}\u{961f}\u{5185}\u{7b2c}2\u{ff08}\u{5171}5\u{4eba}\u{ff09}"
    );

    let mut missing_end_worth = detail(&[(Some("win"), Some(1), Some(0))]);
    missing_end_worth.summary.teammates[4].end_worth = None;
    missing_end_worth.summary.teammates[4].worth = Some(999_999);
    let missing = build_arena_roast_material(&missing_end_worth, "current-synthetic");
    assert!(missing
        .candidate_facts
        .iter()
        .all(|fact| !fact.id.ends_with("arena_final_team_funds_rank")));

    let mut invalid_end_worth = detail(&[(Some("win"), Some(1), Some(0))]);
    invalid_end_worth.summary.teammates[4].end_worth = Some(-1);
    invalid_end_worth.summary.teammates[4].worth = Some(999_999);
    let invalid = build_arena_roast_material(&invalid_end_worth, "current-synthetic");
    assert!(invalid
        .candidate_facts
        .iter()
        .all(|fact| !fact.id.ends_with("arena_final_team_funds_rank")));

    let mut incomplete_identity = detail(&[(Some("win"), Some(1), Some(0))]);
    incomplete_identity.summary.teammates[4].bnet_id.clear();
    let incomplete = build_arena_roast_material(&incomplete_identity, "current-synthetic");
    assert!(incomplete
        .candidate_facts
        .iter()
        .all(|fact| !fact.id.ends_with("arena_final_team_funds_rank")));

    let absent_current = build_arena_roast_material(
        &detail(&[(Some("win"), Some(1), Some(0))]),
        "absent-synthetic",
    );
    assert!(absent_current
        .candidate_facts
        .iter()
        .all(|fact| !fact.id.ends_with("arena_final_team_funds_rank")));

    for teammate_count in [2, 4, 6] {
        let mut wrong_size = detail(&[(Some("win"), Some(1), Some(0))]);
        wrong_size.summary.teammates = (0..teammate_count)
            .map(|index| {
                let bnet_id = if index == 0 {
                    "current-synthetic".to_string()
                } else {
                    format!("teammate-{index}")
                };
                player(&bnet_id, Some(5_000 - index as i64), Some(999_999))
            })
            .collect();
        let material = build_arena_roast_material(&wrong_size, "current-synthetic");
        assert!(material
            .candidate_facts
            .iter()
            .all(|fact| !fact.id.ends_with("arena_final_team_funds_rank")));
    }

    let mut duplicate_current = detail(&[(Some("win"), Some(1), Some(0))]);
    duplicate_current.summary.teammates[4].bnet_id = "current-synthetic".to_string();
    let duplicate = build_arena_roast_material(&duplicate_current, "current-synthetic");
    assert!(duplicate
        .candidate_facts
        .iter()
        .all(|fact| !fact.id.ends_with("arena_final_team_funds_rank")));
}

#[test]
fn serialized_arena_material_is_anonymous_and_summary_scoped() {
    let material = build_arena_roast_material(
        &detail(&[
            (Some("win"), Some(1), Some(0)),
            (Some("win"), Some(2), Some(0)),
        ]),
        "current-synthetic",
    );
    let serialized = serde_json::to_string(&material).expect("arena material serializes");

    assert_arena_material_privacy(&material).expect("arena material remains private");
    assert!(material.candidate_facts.iter().all(|fact| matches!(
        fact.subject,
        FactSubject::CurrentPlayer | FactSubject::Match
    )));
    for forbidden in [
        "current-synthetic",
        "teammate-a",
        "Synthetic#",
        "matchId",
        "playerName",
        "battleTag",
        "bnetId",
        "accountId",
        "rawJson",
        "notes",
        "paths",
        "credentials",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
}
