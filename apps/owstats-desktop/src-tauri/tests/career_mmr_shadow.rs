use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, Role},
    shadow::simulate_shadow_replay,
    training::{TrainingEnvironment, TrainingRow},
    training_report::{
        AggregateSampleBucket, BaselineArtifact, CellArtifact, FitSearchArtifact,
        HierarchicalModelArtifact, NormalizationArtifact, PerformanceCaps,
        PerformanceModelArtifact, RegularizationCandidate, TrainingConfigArtifact,
        ValidationGateArtifact,
    },
};
use rusqlite::Connection;
use std::collections::BTreeMap;

fn row(
    account: &str,
    group: &str,
    role: &str,
    mode: ModeFamily,
    at: i64,
    duration: f64,
) -> TrainingRow {
    TrainingRow {
        account_group: account.into(),
        source_match_group: group.into(),
        match_group: group.into(),
        season: 22,
        begin_ts: at,
        role: role.into(),
        mode_family: mode,
        environment: if matches!(
            mode,
            ModeFamily::StadiumCompetitive | ModeFamily::StadiumQuick
        ) {
            TrainingEnvironment::Stadium
        } else {
            TrainingEnvironment::Ordinary
        },
        hero_group: Some(format!("{role}-hero")),
        outcome_only: mode == ModeFamily::StadiumQuick,
        allow_performance: mode != ModeFamily::StadiumQuick,
        allow_performance_by_hero: true,
        result: 1.0,
        expected_win_rate: 0.5,
        data_quality: 1.0,
        base_delta: Some(25.0),
        final_hit: Some(6.0),
        kill: Some(8.0),
        assist: Some(4.0),
        death: Some(2.0),
        damage: Some(1000.0),
        healing: Some(500.0),
        mitigation: Some(300.0),
        healing_taken: None,
        available_team_healing: None,
        damage_taken: Some(800.0),
        objective_time_sec: Some(20.0),
        duration_sec: 600.0,
        hero_duration_sec: Some(duration),
        healing_taken_semantic: "none",
        usable_for_resource_debt: false,
    }
}

fn artifact() -> PerformanceModelArtifact {
    let role_model = |_role: Role| HierarchicalModelArtifact {
        model_spec_id: None,
        selected_strategy: Some("none".into()),
        selected_variant: Some("core".into()),
        feature_weights: BTreeMap::from([
            ("final".into(), 1.0),
            ("nonfinal".into(), 0.0),
            ("assist".into(), 0.0),
            ("death".into(), 0.0),
        ]),
        training_cohort_offsets: BTreeMap::new(),
        training_cohort_samples: BTreeMap::new(),
        final_hit_multiplier: Some(1.0),
        lambda_core: Some(0.1),
        lambda_cohort: Some(0.1),
        calibration_p95_abs_effect: Some(1.0),
    };
    let mut roles = BTreeMap::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        roles.insert(role, role_model(role));
    }
    let mut cells = Vec::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        for mode in [
            ModeFamily::RoleCompetitive,
            ModeFamily::RoleQuick,
            ModeFamily::OpenCompetitive,
            ModeFamily::OpenQuick,
            ModeFamily::StadiumCompetitive,
            ModeFamily::StadiumQuick,
        ] {
            cells.push(CellArtifact {
                role,
                mode,
                training_cohort: mode.training_cohort(),
                shared_parameter_version: "v".into(),
                role_model_ref: "r".into(),
                model_spec_id: None,
                performance_enabled: mode != ModeFamily::StadiumQuick,
            });
        }
    }
    let mut baselines = Vec::new();
    for role in ["tank", "damage", "support"] {
        for feature in [
            "final_hit",
            "non_final_eliminations",
            "assist",
            "death",
            "damage",
            "healing",
            "mitigation",
            "damage_taken",
            "objective_time",
        ] {
            baselines.push(BaselineArtifact {
                role: role.into(),
                feature: feature.into(),
                environment: "ordinary".into(),
                hero: None,
                mode: None,
                season: None,
                sample_count: 100,
                median_per_ten: if feature == "final_hit" { 5.0 } else { 0.0 },
                mad_per_ten: 0.6745,
            });
        }
    }
    PerformanceModelArtifact {
        schema_version: 2,
        algorithm: "test".into(),
        feature_version: "test".into(),
        seed: 1,
        roles,
        cells,
        normalization: NormalizationArtifact {
            hierarchy: vec![],
            minimum_hero_samples: 30,
            z_clip: 3.0,
            mad_scale: 0.6745,
            mad_floor_absolute: 1e-6,
            mad_floor_relative: 1e-6,
            healing_min_regression_samples: 100,
            healing_huber_iterations: 8,
            healing_ridge_lambda: 0.1,
            baselines,
            healing_expectations: vec![],
        },
        caps: PerformanceCaps::default(),
        training_config: TrainingConfigArtifact {
            seed: 1,
            mode_weights: BTreeMap::new(),
            fit: FitSearchArtifact {
                variants: vec![],
                final_hit_multipliers: vec![],
                regularization_candidates: vec![RegularizationCandidate {
                    lambda_core: 0.1,
                    lambda_cohort: 0.1,
                }],
                learning_rate: 0.1,
                steps: 1,
                minimum_cohort_offset_rows: 1,
            },
            validation: ValidationGateArtifact {
                stage: "test".into(),
                bootstrap_iterations: 1,
                minimum_improved_folds: 1,
                minimum_role_core_rows: 1,
                minimum_cohort_rows: 1,
                minimum_season_loo_groups: 1,
                minimum_hero_loo_groups: 1,
                minimum_loo_row_coverage: 0.0,
                maximum_loo_worsened_fraction: 1.0,
                minimum_bootstrap_completion: 0.0,
                minimum_critical_direction_stability: 0.0,
                maximum_bootstrap_brier_ci_high: 0.0,
                max_ece_worsening: 0.0,
                maximum_single_feature_share: 1.0,
                maximum_average_adjustment_mmr: 1.0,
                maximum_cap_hit_rate: 1.0,
                require_account_coverage: false,
                require_critical_ci_excludes_zero: false,
                require_nonzero_reference_coefficients: false,
            },
        },
        sample_buckets: Vec::<AggregateSampleBucket>::new(),
        config_hash: "test".into(),
    }
}

#[test]
fn shadow_replay_is_stable_isolated_and_aggregates_role_shares() {
    let rows_a = vec![
        row(
            "account-a",
            "later",
            "damage",
            ModeFamily::RoleCompetitive,
            200,
            600.0,
        ),
        row(
            "account-a",
            "shared",
            "support",
            ModeFamily::RoleCompetitive,
            100,
            300.0,
        ),
        row(
            "account-a",
            "shared",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            300.0,
        ),
    ];
    let mut reversed = rows_a.clone();
    reversed.reverse();
    let first = simulate_shadow_replay(&rows_a, &artifact()).unwrap();
    let second = simulate_shadow_replay(&reversed, &artifact()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.account_group, "account-a");
    assert_eq!(first.replayed_matches.len(), 3);
    let shared_index = first
        .replayed_matches
        .iter()
        .find(|m| m.role == Role::Support)
        .unwrap()
        .match_index;
    let shared: f64 = first
        .replayed_matches
        .iter()
        .filter(|m| m.match_index == shared_index)
        .map(|m| m.role_share)
        .sum();
    assert_eq!(shared, 1.0);
    assert!(
        first
            .roles
            .iter()
            .find(|r| r.role == Role::Damage)
            .unwrap()
            .shadow_mmr
            > first
                .roles
                .iter()
                .find(|r| r.role == Role::Damage)
                .unwrap()
                .baseline_mmr
    );

    let other = simulate_shadow_replay(
        &[row(
            "account-b",
            "only",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            600.0,
        )],
        &artifact(),
    )
    .unwrap();
    assert_eq!(
        other
            .roles
            .iter()
            .find(|r| r.role == Role::Damage)
            .unwrap()
            .matches,
        1
    );
    assert_eq!(
        first
            .roles
            .iter()
            .find(|r| r.role == Role::Damage)
            .unwrap()
            .matches,
        2
    );
}

#[test]
fn only_eligible_cells_adjust_and_stadium_quick_is_always_outcome_only() {
    let mut model = artifact();
    model
        .cells
        .iter_mut()
        .find(|c| c.role == Role::Damage && c.mode == ModeFamily::RoleQuick)
        .unwrap()
        .performance_enabled = false;
    let rows = vec![
        row(
            "a",
            "eligible",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            600.0,
        ),
        row("a", "shadow", "damage", ModeFamily::RoleQuick, 200, 600.0),
        row(
            "a",
            "outcome",
            "damage",
            ModeFamily::StadiumQuick,
            300,
            600.0,
        ),
    ];
    let result = simulate_shadow_replay(&rows, &model).unwrap();
    let eligible = result
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    let shadow = result
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::RoleQuick)
        .unwrap();
    let outcome = result
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::StadiumQuick)
        .unwrap();
    assert!(eligible.performance_adjustment > 0.0);
    assert_eq!(shadow.performance_adjustment, 0.0);
    assert_eq!(outcome.performance_adjustment, 0.0);
}

#[test]
fn zero_effect_matches_baseline_exactly_and_caps_are_reported() {
    let mut model = artifact();
    for role in model.roles.values_mut() {
        role.feature_weights.values_mut().for_each(|v| *v = 0.0);
    }
    let baseline = simulate_shadow_replay(
        &[row(
            "a",
            "m",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            600.0,
        )],
        &model,
    )
    .unwrap();
    let damage = baseline
        .roles
        .iter()
        .find(|r| r.role == Role::Damage)
        .unwrap();
    assert_eq!(damage.shadow_mmr, damage.baseline_mmr);
    assert_eq!(damage.total_performance_adjustment, 0.0);

    let capped = simulate_shadow_replay(
        &[row("a", "q", "damage", ModeFamily::RoleQuick, 100, 600.0)],
        &artifact(),
    )
    .unwrap();
    assert_eq!(
        capped.cap_hits[&owstats_desktop_lib::career_mmr::shadow::CellKey {
            role: Role::Damage,
            mode: ModeFamily::RoleQuick,
        }]
            .upper,
        1
    );
}

#[test]
fn shadow_replay_has_no_database_handle_and_cannot_change_source_aggregates() {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE career_mmr_events(role TEXT, final_delta REAL); INSERT INTO career_mmr_events VALUES ('damage', 25.0);").unwrap();
    connection.pragma_update(None, "query_only", true).unwrap();
    let before_changes = connection.total_changes();
    let before: (i64, f64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(final_delta),0) FROM career_mmr_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let projected = vec![row(
        "dataset",
        "memory",
        "damage",
        ModeFamily::RoleCompetitive,
        100,
        600.0,
    )];
    let _ = simulate_shadow_replay(&projected, &artifact()).unwrap();
    let after: (i64, f64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(final_delta),0) FROM career_mmr_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(connection.total_changes(), before_changes);
    assert_eq!(after, before);
}
