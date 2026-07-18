use owstats_desktop_lib::career_mmr::{
    input::ModeFamily,
    performance::{BaselineLevel, FeatureValues, MatrixRow},
    performance_forward::{
        build_forward_folds, evaluate_forward, max_feature_contribution_share, metrics,
        missing_values, role_vector, role_vector_with_multiplier, HealingTakenStrategy,
        ModelVariant, Role,
    },
    training::TrainingEnvironment,
};
use std::collections::BTreeMap;

fn row(group: &str, ts: i64, _role: Role, value: f64) -> MatrixRow {
    let mut features = FeatureValues::default();
    features.final_hit = Some(value);
    features.non_final_eliminations = Some(value / 2.0);
    features.assist = Some(value / 3.0);
    features.death = Some(value / 4.0);
    features.damage = Some(value * 2.0);
    features.healing = Some(value * 3.0);
    features.mitigation = Some(value * 4.0);
    features.damage_taken = Some(value * 5.0);
    features.objective_time = Some(value / 5.0);
    MatrixRow {
        match_group: group.into(),
        hero_group: "private-hero".into(),
        role: match _role {
            Role::Tank => "tank",
            Role::Damage => "damage",
            Role::Support => "support",
        }
        .into(),
        season: 23,
        begin_ts: ts,
        result: if value > 0.0 { 1.0 } else { 0.0 },
        expected_win_rate: 0.5,
        data_quality: 1.0,
        mode_family: ModeFamily::RoleCompetitive,
        per_ten: features.clone(),
        standardized: features,
        baselines: BTreeMap::<_, BaselineLevel>::new(),
        healing_taken_derived: Some(value.abs()),
        damage_baseline_median_per_ten: Some(100.0),
    }
}

fn training(role: &str, i: i64) -> owstats_desktop_lib::career_mmr::training::TrainingRow {
    use owstats_desktop_lib::career_mmr::training::TrainingRow;
    TrainingRow {
        account_group: "dataset-001".into(),
        source_match_group: format!("source-{role}-{i}"),
        match_group: format!("private-{role}-{i}"),
        season: 23,
        begin_ts: i,
        role: role.into(),
        mode_family: ModeFamily::RoleCompetitive,
        environment: TrainingEnvironment::Ordinary,
        hero_group: Some(format!("hero-{role}")),
        outcome_only: false,
        allow_performance: true,
        allow_performance_by_hero: true,
        result: if i % 2 == 0 { 1.0 } else { 0.0 },
        expected_win_rate: 0.5,
        data_quality: 1.0,
        base_delta: Some(10.0),
        final_hit: Some((i % 5 + 1) as f64),
        kill: Some((i % 7 + 3) as f64),
        assist: Some((i % 4 + 1) as f64),
        death: Some((i % 3 + 1) as f64),
        damage: Some((i % 9 + 1) as f64 * 100.0),
        healing: Some((i % 8 + 1) as f64 * 100.0),
        mitigation: Some((i % 6 + 1) as f64 * 100.0),
        healing_taken: Some((i % 10 + 1) as f64 * 100.0),
        available_team_healing: Some((i % 8 + 2) as f64 * 100.0),
        damage_taken: Some((i % 11 + 1) as f64 * 100.0),
        objective_time_sec: Some((i % 6 + 1) as f64),
        duration_sec: 600.0,
        hero_duration_sec: Some(600.0),
        healing_taken_semantic: "external",
        usable_for_resource_debt: true,
    }
}

#[test]
fn overlapping_time_components_never_leak_across_forward_folds() {
    let rows = vec![
        row("same", 10, Role::Tank, 1.0),
        row("bridge", 10, Role::Tank, 2.0),
        row("same", 30, Role::Tank, 3.0),
        row("later", 40, Role::Tank, 4.0),
        row("fifty", 50, Role::Tank, 5.0),
        row("sixty", 60, Role::Tank, 6.0),
        row("seventy", 70, Role::Tank, 7.0),
    ];
    let folds = build_forward_folds(&rows, 5);
    for fold in &folds {
        let max_train = fold.training.iter().map(|&i| rows[i].begin_ts).max();
        let min_valid = fold.validation.iter().map(|&i| rows[i].begin_ts).min();
        if let (Some(a), Some(b)) = (max_train, min_valid) {
            assert!(a < b);
        }
        for group in ["same", "bridge"] {
            let train = fold.training.iter().any(|&i| rows[i].match_group == group);
            let valid = fold
                .validation
                .iter()
                .any(|&i| rows[i].match_group == group);
            assert!(!(train && valid));
        }
    }
}

#[test]
fn role_vectors_are_distinct_and_never_double_negate_death_or_debt() {
    let source = row("m", 1, Role::Tank, 2.0);
    let tank = role_vector(
        &source,
        Role::Tank,
        ModelVariant::RoleFull,
        HealingTakenStrategy::DirectNegative,
    );
    let damage = role_vector(
        &source,
        Role::Damage,
        ModelVariant::RoleFull,
        HealingTakenStrategy::ResourceConversionDebt,
    );
    let support = role_vector(
        &source,
        Role::Support,
        ModelVariant::RoleFull,
        HealingTakenStrategy::ResourceConversionDebt,
    );
    assert_ne!(tank.values, damage.values);
    assert_ne!(damage.values, support.values);
    assert!(tank.values[tank.death_index.unwrap()] >= 0.0);
    assert!(tank.values[tank.debt_index.unwrap()] >= 0.0);
    assert!(support.conservative_scale < 1.0);
}

#[test]
fn direct_exposure_is_fixed_scaled_and_clamped_without_training_statistics() {
    let mut source = row("m", 1, Role::Tank, 2.0);
    source.healing_taken_derived = Some(50_000.0);
    let vector = role_vector(
        &source,
        Role::Tank,
        ModelVariant::Core,
        HealingTakenStrategy::DirectNegative,
    );
    assert_eq!(vector.values[vector.debt_index.unwrap()], 3.0);
}

#[test]
fn role_vector_consumes_fold_local_resource_debt_without_recomputing_it() {
    let mut low = row("low", 1, Role::Tank, 0.0);
    low.healing_taken_derived = Some(2.0);
    let mut high = low.clone();
    high.healing_taken_derived = Some(0.0);
    high.standardized.final_hit = Some(3.0);
    high.standardized.non_final_eliminations = Some(3.0);
    high.standardized.assist = Some(3.0);
    high.standardized.damage = Some(3.0);
    high.standardized.mitigation = Some(3.0);
    let low_v = role_vector(
        &low,
        Role::Tank,
        ModelVariant::RoleFull,
        HealingTakenStrategy::ResourceConversionDebt,
    );
    let high_v = role_vector(
        &high,
        Role::Tank,
        ModelVariant::RoleFull,
        HealingTakenStrategy::ResourceConversionDebt,
    );
    assert_eq!(low_v.values[low_v.debt_index.unwrap()], 2.0);
    assert_eq!(high_v.values[high_v.debt_index.unwrap()], 0.0);
}

#[test]
fn damage_conversion_uses_selected_final_multiplier_and_penalizes_empty_volume() {
    let mut source = row("damage-conversion", 1, Role::Damage, 1.0);
    source.per_ten.final_hit = Some(10.0);
    source.per_ten.non_final_eliminations = Some(5.0);
    source.per_ten.damage = Some(1000.0);
    source.damage_baseline_median_per_ten = Some(2000.0);
    let base = role_vector_with_multiplier(
        &source,
        Role::Damage,
        ModelVariant::RoleFull,
        HealingTakenStrategy::None,
        1.0,
    );
    let boosted = role_vector_with_multiplier(
        &source,
        Role::Damage,
        ModelVariant::RoleFull,
        HealingTakenStrategy::None,
        2.0,
    );
    let slot = base
        .names
        .iter()
        .position(|name| *name == "damage_conversion")
        .unwrap();
    assert!(boosted.values[slot] > base.values[slot]);
    let mut raw_damage_only = source.clone();
    raw_damage_only.per_ten.damage = Some(10_000.0);
    let raw = role_vector_with_multiplier(
        &raw_damage_only,
        Role::Damage,
        ModelVariant::RoleFull,
        HealingTakenStrategy::None,
        1.0,
    );
    assert!(raw.values[slot] < base.values[slot]);
}

#[test]
fn contribution_share_uses_the_selected_non_unit_final_multiplier() {
    let mut source = row("contribution", 1, Role::Damage, 1.0);
    source.per_ten.final_hit = Some(10.0);
    source.per_ten.non_final_eliminations = Some(1.0);
    source.per_ten.damage = Some(100.0);
    source.damage_baseline_median_per_ten = Some(100.0);
    let weights = vec![
        0.1;
        role_vector_with_multiplier(
            &source,
            Role::Damage,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None,
            2.0,
        )
        .values
        .len()
    ];
    let unit = max_feature_contribution_share(
        &source,
        Role::Damage,
        ModelVariant::RoleFull,
        HealingTakenStrategy::None,
        1.0,
        &weights,
    );
    let selected = max_feature_contribution_share(
        &source,
        Role::Damage,
        ModelVariant::RoleFull,
        HealingTakenStrategy::None,
        2.0,
        &weights,
    );
    assert_ne!(unit, selected);
}

#[test]
fn missing_value_count_matches_each_roles_actual_vector_inputs_and_strategy() {
    let mut complete = row("missing-contract", 1, Role::Tank, 2.0);
    complete.standardized.frontline_conversion = Some(1.0);
    complete.standardized.survival = Some(1.0);
    complete.standardized.abnormal_damage_taken = Some(1.0);
    complete.damage_baseline_median_per_ten = Some(100.0);
    assert_eq!(
        missing_values(
            &complete,
            Role::Tank,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None,
        ),
        0
    );
    assert_eq!(
        missing_values(
            &complete,
            Role::Damage,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None,
        ),
        0
    );
    assert_eq!(
        missing_values(
            &complete,
            Role::Support,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None,
        ),
        0
    );

    let mut tank = complete.clone();
    tank.standardized.damage = None;
    assert_eq!(
        missing_values(
            &tank,
            Role::Tank,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None
        ),
        0
    );
    tank.standardized.frontline_conversion = None;
    tank.standardized.abnormal_damage_taken = None;
    assert_eq!(
        missing_values(
            &tank,
            Role::Tank,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None
        ),
        2
    );

    let mut damage = complete.clone();
    damage.per_ten.damage = None;
    assert_eq!(
        missing_values(
            &damage,
            Role::Damage,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None
        ),
        1
    );
    damage.per_ten.damage = complete.per_ten.damage;
    damage.damage_baseline_median_per_ten = None;
    assert_eq!(
        missing_values(
            &damage,
            Role::Damage,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None
        ),
        1
    );

    let mut support = complete.clone();
    support.standardized.mitigation = None;
    assert_eq!(
        missing_values(
            &support,
            Role::Support,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None
        ),
        0
    );
    support.standardized.survival = None;
    assert_eq!(
        missing_values(
            &support,
            Role::Support,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None
        ),
        1
    );
    support.standardized.abnormal_damage_taken = None;
    assert_eq!(
        missing_values(
            &support,
            Role::Support,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None
        ),
        2
    );

    for strategy in [
        HealingTakenStrategy::None,
        HealingTakenStrategy::DirectNegative,
        HealingTakenStrategy::HeroNormalized,
        HealingTakenStrategy::ExcessHealing,
        HealingTakenStrategy::ResourceConversionDebt,
    ] {
        let mut row = complete.clone();
        row.healing_taken_derived = None;
        assert_eq!(
            missing_values(&row, Role::Tank, ModelVariant::Core, strategy),
            usize::from(strategy != HealingTakenStrategy::None)
        );
    }
}

#[test]
fn metrics_are_none_for_empty_and_auc_uses_ties() {
    assert_eq!(metrics(&[], &[]), None);
    let report = metrics(&[1.0, 0.0, 1.0], &[0.8, 0.8, 0.2]).unwrap();
    assert!((report.auc - 0.25).abs() < 1e-12);
    assert!(report.brier > 0.0);
    assert!(report.ece >= 0.0);
}

#[test]
fn internal_fold_debug_serialization_does_not_expose_private_keys() {
    let rows = vec![row("private-match", 1, Role::Damage, 1.0)];
    let folds = build_forward_folds(&rows, 5);
    let json = serde_json::to_string(&folds).unwrap();
    assert!(!json.contains("private-match"));
    assert!(!json.contains("private-hero"));
}

#[test]
fn all_role_strategy_combinations_are_preliminary_and_inactive() {
    let rows = ["tank", "damage", "support"]
        .into_iter()
        .flat_map(|role| (0..40).map(move |i| training(role, i)))
        .collect::<Vec<_>>();
    let report = evaluate_forward(&rows);
    assert_eq!(report.len(), 15);
    assert!(report.iter().all(|item| item.preliminary));
    assert!(report.iter().all(|item| item.performance_adjustment == 0.0));
    assert!(report
        .iter()
        .all(|item| item.folds.iter().all(|f| f.validation_rows <= 8)));
    assert!(report
        .iter()
        .flat_map(|item| &item.folds)
        .all(|fold| fold.inner_validation_rows > 0 || fold.selection_degraded));
    assert!(report
        .iter()
        .flat_map(|item| &item.folds)
        .any(|fold| fold.inner_validation_rows > 0));
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("private-"));
    assert!(!json.contains("hero-"));
    let strategies = report
        .iter()
        .map(|item| item.strategy.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(strategies.len(), 5);
}

#[test]
fn outcome_only_extremes_cannot_change_any_forward_result() {
    let base = (0..80).map(|i| training("damage", i)).collect::<Vec<_>>();
    let expected = evaluate_forward(&base);
    let mut contaminated = base;
    contaminated.extend((0..120).map(|i| {
        let mut row = training("damage", 1_000 + i);
        row.mode_family = ModeFamily::StadiumQuick;
        row.environment = TrainingEnvironment::Stadium;
        row.outcome_only = true;
        row.final_hit = Some(1.0e12);
        row.kill = Some(1.0e12);
        row.damage = Some(1.0e15);
        row.healing_taken = Some(1.0e15);
        row.result = if i % 2 == 0 { 0.0 } else { 1.0 };
        row
    }));
    contaminated.extend((0..20).map(|i| {
        let mut row = training("damage", 2_000 + i);
        row.outcome_only = true;
        row.allow_performance = true;
        row.final_hit = Some(1.0e12);
        row.damage = Some(1.0e15);
        row
    }));
    assert_eq!(evaluate_forward(&contaminated), expected);
}

#[test]
fn auc_ignores_draw_rows() {
    let report = metrics(&[1.0, 0.0, 0.5], &[0.9, 0.1, 1.0]).unwrap();
    assert_eq!(report.auc, 1.0);
}

#[test]
fn metrics_reject_non_finite_input() {
    assert_eq!(metrics(&[1.0], &[f64::NAN]), None);
}

#[test]
fn stability_defaults_are_deterministic_private_and_cover_every_strategy() {
    use owstats_desktop_lib::career_mmr::performance_stability::{
        evaluate_stability, StabilityConfig,
    };
    use owstats_desktop_lib::career_mmr::training::TrainingRow;
    let make = |role: &str, i: i64| TrainingRow {
        account_group: "dataset-001".into(),
        source_match_group: format!("source-{role}-{i}"),
        match_group: format!("private-match-{role}-{}", i / 2),
        season: 21 + i % 3,
        begin_ts: i,
        role: role.into(),
        mode_family: ModeFamily::RoleCompetitive,
        environment: TrainingEnvironment::Ordinary,
        hero_group: Some(format!("private-hero-{}", i % 4)),
        outcome_only: false,
        allow_performance: true,
        allow_performance_by_hero: true,
        result: if i % 2 == 0 { 1.0 } else { 0.0 },
        expected_win_rate: 0.5,
        data_quality: 1.0,
        base_delta: Some(10.0),
        final_hit: Some((i % 5 + 1) as f64),
        kill: Some((i % 7 + 3) as f64),
        assist: Some((i % 4 + 1) as f64),
        death: Some((i % 3 + 1) as f64),
        damage: Some((i % 9 + 1) as f64 * 100.0),
        healing: Some((i % 8 + 1) as f64 * 100.0),
        mitigation: Some((i % 6 + 1) as f64 * 100.0),
        healing_taken: Some((i % 10 + 1) as f64 * 100.0),
        available_team_healing: Some((i % 8 + 2) as f64 * 100.0),
        damage_taken: Some((i % 11 + 1) as f64 * 100.0),
        objective_time_sec: Some((i % 6 + 1) as f64),
        duration_sec: 600.0,
        hero_duration_sec: Some(600.0),
        healing_taken_semantic: "external",
        usable_for_resource_debt: true,
    };
    let rows = ["tank", "damage", "support"]
        .into_iter()
        .flat_map(|role| (0..30).map(move |i| make(role, i)))
        .collect::<Vec<_>>();
    assert_eq!(StabilityConfig::default().bootstrap_iterations, 1000);
    let config = StabilityConfig {
        bootstrap_iterations: 100,
        seed: 7,
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let first = evaluate_stability(&rows, config.clone());
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    let second = evaluate_stability(&rows, config);
    assert_eq!(first, second);
    assert_eq!(first.reports.len(), 15);
    assert!(first
        .reports
        .iter()
        .all(|r| r.bootstrap.requested_iterations == 100));
    assert!(first.roles.iter().all(|r| r.selected.is_none()
        || r.reports
            .iter()
            .any(|x| x.eligible && Some(x.strategy.clone()) == r.selected)));
    let json = serde_json::to_string(&first).unwrap();
    assert!(!json.contains("private-match"));
    assert!(!json.contains("private-hero"));
}

#[test]
fn bootstrap_cluster_clones_remain_distinct_and_empty_loo_groups_are_skipped() {
    use owstats_desktop_lib::career_mmr::performance_stability::{
        clone_cluster_for_bootstrap, LooReport,
    };
    use owstats_desktop_lib::career_mmr::training::TrainingRow;
    let a = TrainingRow {
        account_group: "dataset-001".into(),
        source_match_group: "source-a".into(),
        match_group: "secret".into(),
        season: 23,
        begin_ts: 1,
        role: "tank".into(),
        mode_family: ModeFamily::RoleCompetitive,
        environment: TrainingEnvironment::Ordinary,
        hero_group: Some("hero".into()),
        outcome_only: false,
        allow_performance: true,
        allow_performance_by_hero: true,
        result: 1.0,
        expected_win_rate: 0.5,
        data_quality: 1.0,
        base_delta: Some(10.0),
        final_hit: Some(1.0),
        kill: Some(2.0),
        assist: Some(1.0),
        death: Some(1.0),
        damage: Some(1.0),
        healing: Some(1.0),
        mitigation: Some(1.0),
        healing_taken: Some(1.0),
        available_team_healing: Some(2.0),
        damage_taken: Some(1.0),
        objective_time_sec: Some(1.0),
        duration_sec: 600.0,
        hero_duration_sec: Some(600.0),
        healing_taken_semantic: "external",
        usable_for_resource_debt: true,
    };
    let mut b = a.clone();
    b.begin_ts = 2;
    let first = clone_cluster_for_bootstrap(&[a, b], 0);
    let second = clone_cluster_for_bootstrap(&first, 1);
    assert_eq!(first[0].match_group, first[1].match_group);
    assert_ne!(first[0].match_group, second[0].match_group);
    let report = LooReport::from_deltas(vec![None, Some(-0.1), Some(0.2)]);
    assert_eq!(report.validation_groups, 2);
    assert_eq!(report.skipped_groups, 1);
    assert_eq!(report.worsened_groups, 1);
}

#[test]
fn eligibility_never_falls_back_and_direction_gate_rejects() {
    use owstats_desktop_lib::career_mmr::performance_stability::{
        select_eligible, BootstrapReport, StabilityReport,
    };
    let mut report = StabilityReport::testing(Role::Tank, "none");
    report.bootstrap = BootstrapReport::testing_direction(0.5);
    report.apply_gates();
    assert!(!report.eligible);
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "critical_direction_stability_below_90_percent"));
    assert_eq!(select_eligible(Role::Tank, &[report]), None);
}

#[test]
fn stability_gates_require_enough_loo_groups_and_every_coefficient_ci() {
    use owstats_desktop_lib::career_mmr::performance_stability::{
        BootstrapReport, CoefficientStability, LooReport, StabilityReport,
    };
    let mut report = StabilityReport::testing(Role::Tank, "none");
    report.leave_one_season_out = LooReport::from_deltas(vec![Some(-0.1)]);
    report.leave_one_hero_out = LooReport::from_deltas(vec![Some(-0.1); 4]);
    report.bootstrap = BootstrapReport {
        brier_delta_ci95_high: Some(-0.01),
        coefficients: vec![CoefficientStability {
            weight: "damage".into(),
            direction_stability: 0.95,
            ci95_low: -0.1,
            ci95_high: 0.2,
            critical_constraint: false,
            reference_nonzero: true,
        }],
        ..Default::default()
    };
    report.apply_gates();
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "season_loo_valid_groups_below_2"));
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "hero_loo_valid_groups_below_5"));
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "coefficient_ci95_crosses_zero"));

    report.bootstrap.coefficients[0].ci95_low = 0.01;
    report.bootstrap.coefficients[0].reference_nonzero = false;
    report.apply_gates();
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "zero_reference_coefficient_unstable"));
}

#[test]
fn bootstrap_work_estimate_and_hundred_iteration_smoke_are_bounded() {
    use owstats_desktop_lib::career_mmr::performance_stability::{
        bootstrap_work_estimate, StabilityConfig,
    };
    let estimate = bootstrap_work_estimate(60, 9, 100, StabilityConfig::default().optimizer_passes);
    assert_eq!(estimate.baseline_rebuilds, 100);
    assert!(estimate.normal_equation_cells <= 60 * 9 * 9 * 100);
    assert!(estimate.coordinate_updates <= 9 * 20 * 100);
}

#[test]
fn bootstrap_completion_and_loo_coverage_are_mandatory() {
    use owstats_desktop_lib::career_mmr::performance_stability::{
        BootstrapReport, LooReport, StabilityReport,
    };
    let mut report = StabilityReport::testing(Role::Damage, "none");
    report.leave_one_season_out = LooReport::from_deltas(vec![Some(-0.1); 2]);
    report.leave_one_hero_out = LooReport::from_deltas(vec![Some(-0.1); 5]);
    report.bootstrap = BootstrapReport {
        requested_iterations: 1000,
        completed_iterations: 899,
        brier_delta_ci95_high: Some(-0.01),
        coefficients: vec![],
        ..Default::default()
    };
    report.apply_gates();
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "bootstrap_completion_below_90_percent"));
    report.bootstrap.requested_iterations = 0;
    report.apply_gates();
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "bootstrap_iterations_zero"));
    report.leave_one_hero_out.candidate_rows = 100;
    report.leave_one_hero_out.evaluated_rows = 50;
    report.apply_gates();
    assert!(report
        .rejected_reasons
        .iter()
        .any(|r| r == "hero_loo_row_coverage_below_80_percent"));
}

#[test]
fn worker_failure_is_merged_without_panicking_or_selecting_role() {
    use owstats_desktop_lib::career_mmr::performance_stability::merge_worker_results;
    let (reports, errors) =
        merge_worker_results(vec![(Role::Tank, None), (Role::Damage, Some(vec![]))]);
    assert!(reports.is_empty());
    assert_eq!(errors, vec!["tank_role_worker_failed"]);
}
