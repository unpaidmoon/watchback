use owstats_desktop_lib::career_mmr::{
    evaluation::{evaluate_outcome_baselines, ECE_BINS, MIN_REPORT_SAMPLES},
    input::ModeFamily,
    training::TrainingRow,
};

fn row(ts: i64, role: &str, mode: ModeFamily, result: f64, expected: f64) -> TrainingRow {
    TrainingRow {
        account_group: "dataset-001".into(),
        source_match_group: format!("source-{ts}"),
        match_group: format!("group-{ts}"),
        season: 22,
        begin_ts: ts,
        role: role.into(),
        mode_family: mode,
        hero_group: None,
        outcome_only: true,
        allow_performance: false,
        allow_performance_by_hero: false,
        result,
        expected_win_rate: expected,
        data_quality: 0.55,
        final_hit: None,
        kill: None,
        assist: None,
        death: None,
        damage: None,
        healing: None,
        mitigation: None,
        healing_taken: None,
        damage_taken: None,
        objective_time_sec: None,
        duration_sec: 600.0,
        hero_duration_sec: None,
        healing_taken_semantic: "unknown",
        usable_for_resource_debt: false,
    }
}

#[test]
fn keeps_every_match_group_wholly_inside_one_time_fold() {
    let mut rows = Vec::new();
    for group in 0..12 {
        let mut first = row(group * 10, "tank", ModeFamily::RoleCompetitive, 0.0, 0.4);
        first.match_group = format!("g-{group}");
        let mut second = row(
            group * 10 + 9,
            "support",
            ModeFamily::RoleCompetitive,
            1.0,
            0.6,
        );
        second.match_group = first.match_group.clone();
        rows.extend([first, second]);
    }
    let report = evaluate_outcome_baselines(&rows);
    assert_eq!(report.folds.len(), 5);
    assert!(report.folds.iter().all(|f| f.validation_samples % 2 == 0));
    assert_eq!(
        report
            .folds
            .iter()
            .map(|f| f.validation_match_groups)
            .sum::<usize>(),
        10
    );
}

#[test]
fn tied_timestamps_are_never_split_across_a_fold_boundary() {
    let mut rows = (0..18)
        .map(|i| {
            row(
                i / 2,
                "tank",
                ModeFamily::RoleCompetitive,
                (i % 2) as f64,
                0.5,
            )
        })
        .collect::<Vec<_>>();
    for (index, item) in rows.iter_mut().enumerate() {
        item.match_group = format!("tied-{index}");
    }
    let report = evaluate_outcome_baselines(&rows);
    assert_eq!(report.folds.len(), 5);
    assert!(report
        .folds
        .iter()
        .all(|f| f.train_end_ts < f.validation_start_ts));
}

#[test]
fn a_match_spanning_time_is_grouped_with_every_overlapping_time_component() {
    let mut rows = Vec::new();
    let mut spanning_start = row(2, "tank", ModeFamily::RoleCompetitive, 0.0, 0.5);
    spanning_start.match_group = "spanning".into();
    let mut spanning_end = row(8, "support", ModeFamily::RoleCompetitive, 1.0, 0.5);
    spanning_end.match_group = "spanning".into();
    rows.extend([spanning_start, spanning_end]);
    for ts in [0, 1, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14] {
        rows.push(row(
            ts,
            "damage",
            ModeFamily::RoleQuick,
            (ts % 2) as f64,
            0.5,
        ));
    }
    let report = evaluate_outcome_baselines(&rows);
    assert_eq!(report.folds.len(), 5);
    assert!(report
        .folds
        .iter()
        .all(|f| f.train_end_ts < f.validation_start_ts));
    assert_eq!(report.embargoed_rows, 0);
}

#[test]
fn creates_five_strictly_forward_validation_folds() {
    let rows = (0..60)
        .map(|i| row(i, "tank", ModeFamily::RoleCompetitive, (i % 2) as f64, 0.6))
        .collect::<Vec<_>>();
    let report = evaluate_outcome_baselines(&rows);
    assert_eq!(report.folds.len(), 5);
    for fold in &report.folds {
        assert!(fold.train_end_ts < fold.validation_start_ts);
        assert!(fold.train_samples > 0);
        assert!(fold.validation_samples > 0);
        assert!(fold.expected.brier.is_finite());
        assert!(fold.fixed_half.brier.is_finite());
    }
    assert!(report
        .folds
        .windows(2)
        .all(|w| w[0].validation_end_ts < w[1].validation_start_ts));
}

#[test]
fn reports_brier_auc_and_ten_bin_ece_by_role_and_mode() {
    let mut rows = Vec::new();
    for i in 0..120 {
        rows.push(row(
            i,
            if i % 2 == 0 { "tank" } else { "support" },
            if i % 3 == 0 {
                ModeFamily::RoleQuick
            } else {
                ModeFamily::RoleCompetitive
            },
            (i % 2) as f64,
            if i % 2 == 0 { 0.2 } else { 0.8 },
        ));
    }
    let report = evaluate_outcome_baselines(&rows);
    assert_eq!(report.ece_bins, ECE_BINS);
    assert!(report.ece_definition.contains("[0.0,0.1)"));
    assert!(report.ece_definition.contains("[0.9,1.0]"));
    assert!(report
        .groups
        .iter()
        .any(|g| g.dimension == "role" && g.key == "tank"));
    assert!(report
        .groups
        .iter()
        .any(|g| g.dimension == "mode" && g.key == "role_quick"));
    let overall = report
        .groups
        .iter()
        .find(|g| g.dimension == "overall")
        .unwrap();
    assert!(overall.expected.brier < overall.fixed_half.brier);
    assert_eq!(overall.expected.auc, Some(1.0));
    assert!((overall.expected.ece - 0.2).abs() < 1e-12);
    assert!(overall.fixed_half.ece < 1e-12);
}

#[test]
fn explicitly_degrades_small_or_single_class_groups() {
    let rows = (0..20)
        .map(|i| row(i, "damage", ModeFamily::OpenCompetitive, 1.0, 0.7))
        .collect::<Vec<_>>();
    let report = evaluate_outcome_baselines(&rows);
    let group = report
        .groups
        .iter()
        .find(|g| g.dimension == "role")
        .unwrap();
    assert_eq!(group.samples, 16); // first chronological sixth is training-only
    assert!(group.samples < MIN_REPORT_SAMPLES);
    assert_eq!(group.status, "insufficient_sample");
    assert_eq!(group.expected.auc, None);
}

#[test]
fn keeps_performance_rows_but_deduplicates_same_match_role_segments() {
    let mut rows = (0..60)
        .map(|i| row(i, "tank", ModeFamily::RoleCompetitive, (i % 2) as f64, 0.6))
        .collect::<Vec<_>>();
    rows[59].allow_performance = true;
    rows[59].outcome_only = false;
    let mut duplicate_segment = rows[59].clone();
    duplicate_segment.hero_group = Some("different-hero".into());
    rows.push(duplicate_segment);
    rows[58].expected_win_rate = 2.0;
    let report = evaluate_outcome_baselines(&rows);
    assert_eq!(report.excluded_rows, 1);
    assert_eq!(
        report
            .folds
            .iter()
            .map(|fold| fold.validation_samples)
            .sum::<usize>(),
        49
    );
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("hero_group"));
    assert!(!json.contains("match_id"));
    assert!(!json.contains("guid"));
    assert!(!json.contains("hash"));
    assert!(!json.contains("raw_json"));
}

#[test]
fn auc_uses_rank_sum_instead_of_pairwise_comparison() {
    let source = include_str!("../src/career_mmr/evaluation.rs");
    assert!(
        source.contains("fn rank_sum_auc(values:"),
        "AUC must use the O(n log n) rank-sum implementation"
    );
}
