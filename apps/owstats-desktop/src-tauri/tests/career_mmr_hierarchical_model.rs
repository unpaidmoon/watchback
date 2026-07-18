use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, Role, TrainingCohort},
    performance::{BaselineLevel, FeatureValues, HealingTakenStrategy, MatrixRow},
    performance_forward::ModelVariant,
    performance_model::{fit_model, predict_probability, FitConfig},
};
use std::collections::BTreeMap;

fn row(index: usize, mode_family: ModeFamily, positive_pattern: bool) -> MatrixRow {
    let positive = index % 2 == 0;
    let sign = if positive == positive_pattern {
        1.0
    } else {
        -1.0
    };
    let mut features = FeatureValues::default();
    features.final_hit = Some(2.0 * sign);
    features.non_final_eliminations = Some(sign);
    features.death = Some(-sign);
    MatrixRow {
        match_group: format!("synthetic-{mode_family:?}-{index}"),
        hero_group: "synthetic-hero".into(),
        role: "damage".into(),
        season: 20 + (index % 3) as i64,
        begin_ts: index as i64,
        result: if positive { 1.0 } else { 0.0 },
        expected_win_rate: if index % 4 < 2 { 0.35 } else { 0.65 },
        data_quality: if index % 5 == 0 { 0.5 } else { 1.0 },
        mode_family,
        per_ten: features.clone(),
        standardized: features,
        baselines: BTreeMap::<_, BaselineLevel>::new(),
        healing_taken_derived: None,
        damage_baseline_median_per_ten: Some(100.0),
    }
}

#[test]
fn constrained_hierarchy_shares_all_queue_offsets_and_never_trains_outcome_only() {
    let mut rows = Vec::new();
    rows.extend((0..100).map(|i| row(i, ModeFamily::RoleCompetitive, true)));
    rows.extend((100..200).map(|i| row(i, ModeFamily::RoleQuick, true)));
    rows.extend((200..280).map(|i| row(i, ModeFamily::OpenCompetitive, true)));
    rows.extend((280..360).map(|i| row(i, ModeFamily::OpenQuick, true)));
    rows.extend((360..560).map(|i| row(i, ModeFamily::StadiumCompetitive, false)));
    rows.extend((560..760).map(|i| row(i, ModeFamily::StadiumQuick, false)));
    let model = fit_model(
        &rows.iter().collect::<Vec<_>>(),
        Role::Damage,
        ModelVariant::Core,
        HealingTakenStrategy::None,
        &FitConfig {
            final_multiplier: 1.5,
            ..FitConfig::default()
        },
    )
    .expect("synthetic fit");

    assert!(model.core_weight("final").unwrap() > model.core_weight("nonfinal").unwrap());
    assert!(model.core_weight("nonfinal").unwrap() > 0.0);
    assert!(model.core_weight("death").unwrap() < 0.0);
    assert!(model.cohort_offset(TrainingCohort::OpenQueue).is_some());
    assert!(model.cohort_offset(TrainingCohort::RoleQueue).is_some());
    assert!(model
        .cohort_offset(TrainingCohort::StadiumCompetitive)
        .is_some());
    assert_ne!(
        model.cohort_offset(TrainingCohort::RoleQueue),
        model.cohort_offset(TrainingCohort::StadiumCompetitive)
    );
    assert!(!model
        .cohort_samples
        .contains_key(&TrainingCohort::OutcomeOnly));

    for cohort in [
        TrainingCohort::RoleQueue,
        TrainingCohort::OpenQueue,
        TrainingCohort::StadiumCompetitive,
    ] {
        let weights = model.effective_weights(cohort);
        let final_weight = weights[model.feature_index("final").unwrap()];
        let nonfinal_weight = weights[model.feature_index("nonfinal").unwrap()];
        let death_weight = weights[model.feature_index("death").unwrap()];
        assert!(final_weight > 0.0);
        assert!(nonfinal_weight > 0.0);
        assert!(final_weight >= 1.5 * nonfinal_weight - 1e-12);
        assert!(death_weight < 0.0);
    }

    let role_comp = predict_probability(&model, &row(700, ModeFamily::RoleCompetitive, true));
    let role_quick = predict_probability(&model, &row(700, ModeFamily::RoleQuick, true));
    assert!((role_comp - role_quick).abs() < 1e-12);
    let open_comp = predict_probability(&model, &row(702, ModeFamily::OpenCompetitive, true));
    let open_quick = predict_probability(&model, &row(702, ModeFamily::OpenQuick, true));
    assert!((open_comp - open_quick).abs() < 1e-12);

    let mut offset_only = row(702, ModeFamily::RoleCompetitive, true);
    offset_only.expected_win_rate = 0.8;
    offset_only.standardized = FeatureValues::default();
    assert!((predict_probability(&model, &offset_only) - 0.8).abs() < 1e-12);
}

#[test]
fn sparse_cohort_offset_remains_forced_to_zero() {
    let rows = (0..149)
        .map(|i| row(i, ModeFamily::OpenCompetitive, true))
        .chain((149..349).map(|i| row(i, ModeFamily::RoleCompetitive, true)))
        .collect::<Vec<_>>();
    let model = fit_model(
        &rows.iter().collect::<Vec<_>>(),
        Role::Damage,
        ModelVariant::Core,
        HealingTakenStrategy::None,
        &FitConfig::default(),
    )
    .unwrap();
    assert!(model.cohort_offset(TrainingCohort::OpenQueue).is_none());
}

#[test]
fn logit_offset_model_improves_on_informative_synthetic_features() {
    let rows = (0..300)
        .map(|i| row(i, ModeFamily::RoleCompetitive, true))
        .collect::<Vec<_>>();
    let model = fit_model(
        &rows.iter().collect::<Vec<_>>(),
        Role::Damage,
        ModelVariant::Core,
        HealingTakenStrategy::None,
        &FitConfig::default(),
    )
    .unwrap();
    let baseline = rows
        .iter()
        .map(|row| (row.result - row.expected_win_rate).powi(2))
        .sum::<f64>()
        / rows.len() as f64;
    let candidate = rows
        .iter()
        .map(|row| (row.result - predict_probability(&model, row)).powi(2))
        .sum::<f64>()
        / rows.len() as f64;
    assert!(candidate < baseline);
}

#[test]
fn resource_debt_is_non_positive_in_core_and_every_effective_cohort() {
    let mut rows = (0..320)
        .map(|i| {
            let mut row = row(i, ModeFamily::RoleCompetitive, true);
            row.healing_taken_derived = Some((i % 7) as f64 / 2.0);
            row
        })
        .collect::<Vec<_>>();
    for (index, row) in rows.iter_mut().enumerate().skip(160) {
        row.mode_family = ModeFamily::StadiumCompetitive;
        row.healing_taken_derived = Some((index % 5) as f64);
    }
    let model = fit_model(
        &rows.iter().collect::<Vec<_>>(),
        Role::Damage,
        ModelVariant::Core,
        HealingTakenStrategy::ResourceConversionDebt,
        &FitConfig::default(),
    )
    .unwrap();
    let debt = model.feature_index("debt").unwrap();
    assert!(model.core_weights[debt] <= 0.0);
    for cohort in [
        TrainingCohort::RoleQueue,
        TrainingCohort::StadiumCompetitive,
    ] {
        assert!(model.effective_weights(cohort)[debt] <= 0.0);
    }
}
