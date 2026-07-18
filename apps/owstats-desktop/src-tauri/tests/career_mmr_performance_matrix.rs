use owstats_desktop_lib::career_mmr::{
    input::ModeFamily,
    performance::{build_feature_matrix, BaselineLevel, FeatureName, HealingTakenStrategy},
    training::TrainingRow,
};

fn row(hero: &str, role: &str, season: i64, value: f64) -> TrainingRow {
    TrainingRow {
        account_group: "dataset-001".into(),
        source_match_group: format!("source-{hero}-{season}-{value}"),
        match_group: format!("match-{hero}-{season}-{value}"),
        season,
        begin_ts: value as i64,
        role: role.into(),
        mode_family: ModeFamily::RoleCompetitive,
        hero_group: Some(hero.into()),
        outcome_only: false,
        allow_performance: true,
        allow_performance_by_hero: true,
        result: 1.0,
        expected_win_rate: 0.5,
        data_quality: 1.0,
        final_hit: Some(value),
        kill: Some(value + 2.0),
        assist: Some(value),
        death: Some(value),
        damage: Some(value * 100.0),
        healing: Some(value * 50.0),
        mitigation: None,
        healing_taken: Some(value * 25.0),
        damage_taken: None,
        objective_time_sec: None,
        duration_sec: 600.0,
        hero_duration_sec: Some(600.0),
        healing_taken_semantic: "external",
        usable_for_resource_debt: true,
    }
}

fn validation_row(hero: &str, role: &str, season: i64, value: f64) -> TrainingRow {
    let mut result = row(hero, role, season, value);
    result.match_group = format!("validation-{}", result.match_group);
    result
}

#[test]
fn rates_per_ten_minutes_and_derives_non_final_eliminations() {
    let mut training = (0..30)
        .map(|i| row("hero-a", "damage", 23, 90.0 + (i % 2) as f64 * 20.0))
        .collect::<Vec<_>>();
    training[0].hero_duration_sec = Some(300.0);
    training[0].final_hit = Some(5.0);
    training[0].kill = Some(9.0);
    let mut validation = training[0].clone();
    validation.match_group = "validation-rate".into();
    let result = build_feature_matrix(&training, &[validation], HealingTakenStrategy::None);
    let fitted = result
        .validation_by_role
        .get("damage")
        .unwrap()
        .first()
        .unwrap();
    assert_eq!(fitted.per_ten.final_hit, Some(10.0));
    assert_eq!(fitted.per_ten.non_final_eliminations, Some(8.0));
    assert_eq!(fitted.begin_ts, 90);
    assert_eq!(fitted.result, 1.0);
    assert_eq!(fitted.expected_win_rate, 0.5);
}

#[test]
fn carries_role_specific_optional_features_per_ten_and_standardized() {
    let mut training = (0..30)
        .map(|i| row("hero-a", "tank", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    for (i, item) in training.iter_mut().enumerate() {
        item.mitigation = Some(100.0 + i as f64 * 10.0);
        item.damage_taken = Some(200.0 + i as f64 * 20.0);
        item.objective_time_sec = Some(10.0 + i as f64);
    }
    let mut validation = training[4].clone();
    validation.match_group = "validation-role-features".into();
    let result = build_feature_matrix(&training, &[validation], HealingTakenStrategy::None);
    let fitted = &result.validation_by_role["tank"][0];
    assert_eq!(fitted.per_ten.mitigation, Some(140.0));
    assert_eq!(fitted.per_ten.damage_taken, Some(280.0));
    assert_eq!(fitted.per_ten.objective_time, Some(14.0));
    assert!(fitted.standardized.mitigation.is_some());
    assert!(fitted.standardized.damage_taken.is_some());
    assert!(fitted.standardized.objective_time.is_some());
}

#[test]
fn fits_only_on_training_and_clamps_validation_robust_z() {
    let training = (0..30)
        .map(|i| row("hero-a", "damage", 23, 90.0 + (i % 2) as f64 * 20.0))
        .collect::<Vec<_>>();
    let validation = vec![validation_row("hero-a", "damage", 23, 10_000.0)];
    let result = build_feature_matrix(&training, &validation, HealingTakenStrategy::None);
    let fitted = &result.validation_by_role["damage"][0];
    assert_eq!(fitted.standardized.final_hit, Some(3.0));
    assert_eq!(
        fitted.baselines[&FeatureName::FinalHit],
        BaselineLevel::HeroModeSeason
    );
    assert_eq!(result.audit.validation_rows, 1);
    assert_eq!(result.audit.roles[0].role, "damage");
}

#[test]
fn falls_back_in_documented_order_and_never_crosses_roles() {
    let mut training = Vec::new();
    for i in 0..30 {
        training.push(row("hero-a", "damage", 22, 10.0 + (i % 2) as f64 * 2.0));
        training.push(row("other", "tank", 23, 1000.0 + (i % 2) as f64 * 2.0));
    }
    let validation = vec![validation_row("hero-a", "damage", 23, 20.0)];
    let result = build_feature_matrix(&training, &validation, HealingTakenStrategy::None);
    let fitted = &result.validation_by_role["damage"][0];
    assert_eq!(
        fitted.baselines[&FeatureName::FinalHit],
        BaselineLevel::HeroMode
    );
    assert!(fitted.standardized.final_hit.unwrap() > 0.0);
}

#[test]
fn preserves_missing_features_instead_of_fabricating_zero() {
    let mut training = (0..30)
        .map(|i| row("hero-a", "support", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    training[0].assist = None;
    training[0].kill = None;
    let mut validation = training[0].clone();
    validation.match_group = "validation-missing".into();
    let result = build_feature_matrix(&training, &[validation], HealingTakenStrategy::None);
    let fitted = &result.validation_by_role["support"][0];
    assert_eq!(fitted.per_ten.assist, None);
    assert_eq!(fitted.per_ten.non_final_eliminations, None);
    assert_eq!(fitted.standardized.assist, None);
    assert!(result.audit.missing_feature_values >= 2);
}

#[test]
fn gates_every_healing_taken_strategy_but_exposes_all_five_strategies() {
    let training = (0..30)
        .map(|i| row("hero-a", "tank", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    let mut unsafe_row = validation_row("hero-a", "tank", 23, 12.0);
    unsafe_row.usable_for_resource_debt = false;
    let strategies = [
        HealingTakenStrategy::None,
        HealingTakenStrategy::DirectNegative,
        HealingTakenStrategy::HeroNormalized,
        HealingTakenStrategy::ExcessHealing,
        HealingTakenStrategy::ResourceConversionDebt,
    ];
    for strategy in strategies {
        let result = build_feature_matrix(&training, &[unsafe_row.clone()], strategy);
        let slot = result.validation_by_role["tank"][0].healing_taken_derived;
        assert_eq!(slot, None);
    }
}

#[test]
fn direct_negative_keeps_a_positive_exposure_for_a_negative_coefficient() {
    let training = (0..30)
        .map(|i| row("hero-a", "tank", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    let validation = validation_row("hero-a", "tank", 23, 12.0);
    let result = build_feature_matrix(
        &training,
        &[validation],
        HealingTakenStrategy::DirectNegative,
    );
    assert!(
        result.validation_by_role["tank"][0]
            .healing_taken_derived
            .unwrap()
            > 0.0
    );
}

#[test]
fn public_audit_serialization_contains_only_aggregates() {
    let training = (0..30)
        .map(|i| {
            row(
                "private-hero-hash",
                "damage",
                23,
                10.0 + (i % 2) as f64 * 2.0,
            )
        })
        .collect::<Vec<_>>();
    let result = build_feature_matrix(
        &training,
        &[validation_row("private-hero-hash", "damage", 23, 12.0)],
        HealingTakenStrategy::None,
    );
    let json = serde_json::to_string(&result.audit).unwrap();
    for forbidden in [
        "match_group",
        "hero_group",
        "guid",
        "raw_json",
        "private-hero-hash",
    ] {
        assert!(!json.contains(forbidden), "public audit leaked {forbidden}");
    }
}

#[test]
fn drops_non_finite_values_created_by_per_ten_scaling() {
    let training = (0..30)
        .map(|i| row("hero-a", "damage", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    let mut validation = validation_row("hero-a", "damage", 23, f64::MAX);
    validation.hero_duration_sec = Some(f64::MIN_POSITIVE);
    let result = build_feature_matrix(&training, &[validation], HealingTakenStrategy::None);
    let fitted = &result.validation_by_role["damage"][0];
    assert_eq!(fitted.per_ten.final_hit, None);
    assert_eq!(fitted.standardized.final_hit, None);
}

#[test]
fn excludes_match_groups_that_overlap_training_and_validation() {
    let training = (0..30)
        .map(|i| row("hero-a", "damage", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    let mut validation = validation_row("hero-a", "damage", 23, 12.0);
    validation.match_group = training[0].match_group.clone();
    let result = build_feature_matrix(&training, &[validation], HealingTakenStrategy::None);
    assert!(result.validation_by_role.is_empty());
    assert_eq!(result.audit.overlap_excluded_rows, 1);
}
