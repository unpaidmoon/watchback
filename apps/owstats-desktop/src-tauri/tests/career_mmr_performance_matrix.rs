use owstats_desktop_lib::career_mmr::{
    input::ModeFamily,
    performance::{build_feature_matrix, BaselineLevel, FeatureName, HealingTakenStrategy},
    training::{TrainingEnvironment, TrainingRow},
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
        environment: TrainingEnvironment::Ordinary,
        hero_group: Some(hero.into()),
        outcome_only: false,
        allow_performance: true,
        allow_performance_by_hero: true,
        result: 1.0,
        expected_win_rate: 0.5,
        data_quality: 1.0,
        base_delta: Some(10.0),
        final_hit: Some(value),
        kill: Some(value + 2.0),
        assist: Some(value),
        death: Some(value),
        damage: Some(value * 100.0),
        healing: Some(value * 50.0),
        mitigation: None,
        healing_taken: Some(value * 25.0),
        available_team_healing: Some(value * 40.0),
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

#[test]
fn exercises_every_environment_scoped_fallback_without_crossing_stadium() {
    let mut training = Vec::new();
    for i in 0..30 {
        let offset = (i % 2) as f64 * 2.0;
        let mut exact = row("exact", "damage", 23, 10.0 + offset);
        exact.mode_family = ModeFamily::RoleCompetitive;
        training.push(exact);

        let mut hero_mode = row("hero-mode", "damage", 22, 20.0 + offset);
        hero_mode.mode_family = ModeFamily::RoleCompetitive;
        training.push(hero_mode);

        let mut hero_environment = row("hero-environment", "damage", 22, 30.0 + offset);
        hero_environment.mode_family = ModeFamily::OpenCompetitive;
        training.push(hero_environment);

        let mut role_mode = row("role-source", "support", 22, 40.0 + offset);
        role_mode.mode_family = ModeFamily::RoleQuick;
        training.push(role_mode);

        let mut role_environment = row("role-env-source", "tank", 22, 50.0 + offset);
        role_environment.mode_family = ModeFamily::OpenQuick;
        training.push(role_environment);

        let mut stadium = row("stadium-only", "damage", 22, 5000.0 + offset);
        stadium.mode_family = ModeFamily::StadiumCompetitive;
        stadium.environment = TrainingEnvironment::Stadium;
        training.push(stadium);
    }

    let mut exact = validation_row("exact", "damage", 23, 12.0);
    exact.mode_family = ModeFamily::RoleCompetitive;
    let mut hero_mode = validation_row("hero-mode", "damage", 23, 22.0);
    hero_mode.mode_family = ModeFamily::RoleCompetitive;
    let mut hero_environment = validation_row("hero-environment", "damage", 23, 32.0);
    hero_environment.mode_family = ModeFamily::OpenQuick;
    let mut role_mode = validation_row("missing-support", "support", 23, 42.0);
    role_mode.mode_family = ModeFamily::RoleQuick;
    let mut role_environment = validation_row("missing-tank", "tank", 23, 52.0);
    role_environment.mode_family = ModeFamily::RoleCompetitive;
    let mut unavailable = validation_row("missing-stadium", "support", 23, 52.0);
    unavailable.mode_family = ModeFamily::StadiumCompetitive;
    unavailable.environment = TrainingEnvironment::Stadium;

    let result = build_feature_matrix(
        &training,
        &[
            exact,
            hero_mode,
            hero_environment,
            role_mode,
            role_environment,
            unavailable,
        ],
        HealingTakenStrategy::None,
    );
    let level = |role: &str, index: usize| {
        result.validation_by_role[role][index].baselines[&FeatureName::Damage]
    };
    assert_eq!(level("damage", 0), BaselineLevel::HeroModeSeason);
    assert_eq!(level("damage", 1), BaselineLevel::HeroMode);
    assert_eq!(level("damage", 2), BaselineLevel::HeroEnvironment);
    assert_eq!(level("support", 0), BaselineLevel::RoleMode);
    assert_eq!(level("tank", 0), BaselineLevel::RoleEnvironment);
    assert_eq!(level("support", 1), BaselineLevel::Unavailable);
}

#[test]
fn validation_extremes_never_change_fold_local_damage_center() {
    let training = (0..30)
        .map(|i| row("hero-a", "damage", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    let ordinary = validation_row("hero-a", "damage", 23, 12.0);
    let extreme = validation_row("hero-a", "damage", 23, 1.0e12);
    let first = build_feature_matrix(&training, &[ordinary.clone()], HealingTakenStrategy::None);
    let second = build_feature_matrix(&training, &[ordinary, extreme], HealingTakenStrategy::None);
    assert_eq!(
        first.validation_by_role["damage"][0].damage_baseline_median_per_ten,
        second.validation_by_role["damage"][0].damage_baseline_median_per_ten
    );
    assert_eq!(
        first.validation_by_role["damage"][0].standardized.damage,
        second.validation_by_role["damage"][0].standardized.damage
    );
}

#[test]
fn resource_debt_alone_distinguishes_conversion_for_equal_healing_exposure() {
    let training = (0..30)
        .map(|i| {
            let mut item = row("tank-a", "tank", 23, 10.0 + (i % 2) as f64 * 2.0);
            item.healing_taken = Some(1000.0 + i as f64 * 10.0);
            item
        })
        .collect::<Vec<_>>();
    let mut high = validation_row("tank-a", "tank", 23, 12.0);
    high.healing_taken = Some(2000.0);
    high.final_hit = Some(50.0);
    high.kill = Some(70.0);
    high.assist = Some(50.0);
    high.objective_time_sec = Some(60.0);
    let mut low = high.clone();
    low.match_group = "validation-low-conversion".into();
    low.final_hit = Some(1.0);
    low.kill = Some(2.0);
    low.assist = Some(1.0);
    low.objective_time_sec = Some(1.0);

    for strategy in [
        HealingTakenStrategy::None,
        HealingTakenStrategy::DirectNegative,
        HealingTakenStrategy::HeroNormalized,
        HealingTakenStrategy::ExcessHealing,
        HealingTakenStrategy::ResourceConversionDebt,
    ] {
        let result = build_feature_matrix(&training, &[high.clone(), low.clone()], strategy);
        let rows = &result.validation_by_role["tank"];
        if strategy == HealingTakenStrategy::None {
            assert_eq!(rows[0].healing_taken_derived, None);
            assert_eq!(rows[1].healing_taken_derived, None);
        } else if strategy == HealingTakenStrategy::ResourceConversionDebt {
            assert!(
                rows[1].healing_taken_derived.unwrap() > rows[0].healing_taken_derived.unwrap()
            );
        } else {
            assert_eq!(rows[0].healing_taken_derived, rows[1].healing_taken_derived);
        }
    }
}

#[test]
fn derives_role_features_from_standardized_conversion_not_raw_volume() {
    let training = (0..30)
        .map(|i| row("tank-a", "tank", 23, 10.0 + (i % 2) as f64 * 2.0))
        .collect::<Vec<_>>();
    let mut efficient = validation_row("tank-a", "tank", 23, 12.0);
    efficient.damage_taken = Some(1000.0);
    efficient.death = Some(1.0);
    efficient.final_hit = Some(20.0);
    efficient.kill = Some(30.0);
    let mut raw_only = efficient.clone();
    raw_only.match_group = "validation-raw-only".into();
    raw_only.damage = Some(1.0e9);
    raw_only.healing = Some(1.0e9);
    let result = build_feature_matrix(
        &training,
        &[efficient, raw_only],
        HealingTakenStrategy::None,
    );
    let rows = &result.validation_by_role["tank"];
    assert_eq!(
        rows[0].standardized.frontline_conversion,
        rows[1].standardized.frontline_conversion
    );
    assert_eq!(
        rows[0].standardized.abnormal_damage_taken,
        rows[1].standardized.abnormal_damage_taken
    );
}

#[test]
fn healing_expectation_uses_fold_local_irls_after_one_hundred_samples() {
    let training = (0..120)
        .map(|i| {
            let signal = 1000.0 + i as f64 * 20.0;
            let mut item = row("tank-irls", "tank", 23, 10.0 + (i % 3) as f64);
            item.damage_taken = Some(signal);
            item.death = Some(2.0 + (i % 5) as f64);
            item.available_team_healing = Some(5000.0 + i as f64 * 10.0);
            item.healing_taken = Some(0.6 * signal + 200.0);
            item
        })
        .collect::<Vec<_>>();
    let mut validation = validation_row("tank-irls", "tank", 23, 12.0);
    validation.damage_taken = Some(3400.0);
    validation.death = Some(6.0);
    validation.available_team_healing = Some(6200.0);
    validation.healing_taken = Some(2240.0);
    let result = build_feature_matrix(
        &training,
        &[validation],
        HealingTakenStrategy::ExcessHealing,
    );
    assert!(
        result.validation_by_role["tank"][0]
            .healing_taken_derived
            .unwrap()
            < 50.0
    );
}

#[test]
fn derived_role_features_are_none_when_any_required_input_is_missing() {
    let mut training = Vec::new();
    for i in 0..30 {
        for role in ["tank", "support"] {
            let mut item = row("hero-required", role, 23, 10.0 + (i % 2) as f64 * 2.0);
            item.damage_taken = Some(1000.0 + (i % 2) as f64 * 200.0);
            item.objective_time_sec = Some(20.0 + (i % 2) as f64 * 4.0);
            training.push(item);
        }
    }
    let mut tank_missing_output = validation_row("hero-required", "tank", 23, 12.0);
    tank_missing_output.damage_taken = Some(1200.0);
    tank_missing_output.objective_time_sec = None;
    let mut tank_missing_death = validation_row("hero-required", "tank", 23, 12.0);
    tank_missing_death.match_group = "validation-tank-missing-death".into();
    tank_missing_death.damage_taken = Some(1200.0);
    tank_missing_death.objective_time_sec = Some(24.0);
    tank_missing_death.death = None;
    let mut support_missing_death = validation_row("hero-required", "support", 23, 12.0);
    support_missing_death.death = None;
    let result = build_feature_matrix(
        &training,
        &[
            tank_missing_output,
            tank_missing_death,
            support_missing_death,
        ],
        HealingTakenStrategy::None,
    );
    let tank = &result.validation_by_role["tank"];
    assert_eq!(tank[0].standardized.frontline_conversion, None);
    assert_eq!(tank[1].standardized.frontline_conversion, None);
    assert_eq!(tank[1].standardized.abnormal_damage_taken, None);
    assert_eq!(
        result.validation_by_role["support"][0]
            .standardized
            .survival,
        None
    );
}
