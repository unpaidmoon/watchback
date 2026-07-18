use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, Role},
    performance_stability::{evaluate_stability, CellDecision, StabilityConfig},
    training::{TrainingAudit, TrainingEnvironment, TrainingRow},
    training_report::{build_training_outputs, recompute_config_hash, validate_report_boundary},
};
use serde_json::{json, Value};

fn row(index: usize, role: &str, mode: ModeFamily) -> TrainingRow {
    TrainingRow {
        account_group: format!("account-id-secret-{}", index % 2),
        source_match_group: format!("match-id-secret-{index}"),
        match_group: format!("550e8400-e29b-41d4-a716-{index:012}"),
        season: 20 + (index % 3) as i64,
        begin_ts: index as i64,
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
        hero_group: Some(format!("synthetic_hero_{}", index % 2)),
        outcome_only: mode == ModeFamily::StadiumQuick,
        allow_performance: mode != ModeFamily::StadiumQuick,
        allow_performance_by_hero: mode != ModeFamily::StadiumQuick,
        result: (index % 2) as f64,
        expected_win_rate: 0.5,
        data_quality: 0.9,
        base_delta: Some(if index % 2 == 0 { 10.0 } else { -10.0 }),
        final_hit: Some(2.0 + index as f64),
        kill: Some(5.0 + index as f64),
        assist: Some(4.0),
        death: Some(2.0),
        damage: Some(4_000.0 + index as f64),
        healing: Some(2_000.0 + index as f64),
        mitigation: Some(1_000.0),
        healing_taken: Some(1_500.0),
        available_team_healing: Some(8_000.0),
        damage_taken: Some(3_000.0),
        objective_time_sec: Some(30.0),
        duration_sec: 600.0,
        hero_duration_sec: Some(600.0),
        healing_taken_semantic: "external_only",
        usable_for_resource_debt: true,
    }
}

fn synthetic_rows() -> Vec<TrainingRow> {
    let roles = ["tank", "damage", "support"];
    let modes = [
        ModeFamily::RoleCompetitive,
        ModeFamily::RoleQuick,
        ModeFamily::OpenCompetitive,
        ModeFamily::OpenQuick,
        ModeFamily::StadiumCompetitive,
        ModeFamily::StadiumQuick,
    ];
    roles
        .into_iter()
        .flat_map(|role| modes.into_iter().map(move |mode| (role, mode)))
        .enumerate()
        .map(|(index, (role, mode))| row(index, role, mode))
        .collect()
}

fn mark_cell_eligible(
    cell: &mut owstats_desktop_lib::career_mmr::performance_stability::CellEvaluation,
    report_index: usize,
) {
    let report = &mut cell.reports[report_index];
    report.eligible = true;
    cell.selected = Some(report.strategy.clone());
    cell.decision = CellDecision::Eligible;
    cell.rejected_reasons.clear();
    cell.nested_validation.selected_strategy = report.strategy.clone();
    cell.nested_validation.selected_variant = report.forward.variant;
    cell.nested_validation.selected_final_multiplier = report.forward.final_multiplier;
    cell.nested_validation.selected_lambda_core = report.forward.lambda_core;
    cell.nested_validation.selected_lambda_cohort = report.forward.lambda_cohort;
    cell.nested_validation.improved_folds = 5;
}

#[test]
fn produces_complete_replayable_and_privacy_safe_outputs() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 0x5eed,
            ..Default::default()
        },
    );

    let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 0x5eed).unwrap();

    assert_eq!(artifact.schema_version, 2);
    assert!(!artifact.algorithm.is_empty());
    assert!(!artifact.feature_version.is_empty());
    assert_eq!(artifact.seed, 0x5eed);
    assert_eq!(artifact.roles.len(), 3);
    assert_eq!(artifact.cells.len(), 15);
    assert!(!artifact.normalization.baselines.is_empty());
    assert_eq!(artifact.normalization.mad_scale, 0.6745);
    assert_eq!(artifact.normalization.mad_floor_absolute, 1.0e-6);
    assert_eq!(artifact.normalization.mad_floor_relative, 1.0e-6);
    assert_eq!(artifact.normalization.healing_min_regression_samples, 100);
    assert_eq!(artifact.normalization.healing_huber_iterations, 8);
    assert_eq!(artifact.normalization.healing_ridge_lambda, 0.1);
    assert_eq!(artifact.caps.competitive_mmr, 3.0);
    assert_eq!(artifact.caps.quick_mmr, 1.0);
    assert_eq!(artifact.caps.competitive_match_delta_mmr, 30.0);
    assert_eq!(artifact.caps.quick_match_delta_mmr, 10.0);
    assert_eq!(artifact.config_hash.len(), 64);
    assert_eq!(report.dataset.account_datasets, 2);
    assert_eq!(report.cells.len(), 15);
    assert_eq!(report.stadium_quick.decision, CellDecision::OutcomeOnly);
    assert!(report
        .cells
        .iter()
        .all(|cell| !cell.rejected_reasons.is_empty() || cell.eligible));
    assert!(report
        .cells
        .iter()
        .all(|cell| cell.metrics.strategy_reports.len() == 5));
    assert!(report
        .cells
        .iter()
        .all(|cell| cell.metrics.nested_validation.mode == cell.mode));

    for role in [Role::Tank, Role::Damage, Role::Support] {
        let role_comp = report
            .cells
            .iter()
            .find(|cell| cell.role == role && cell.mode == ModeFamily::RoleCompetitive)
            .unwrap();
        let role_quick = report
            .cells
            .iter()
            .find(|cell| cell.role == role && cell.mode == ModeFamily::RoleQuick)
            .unwrap();
        assert_eq!(
            role_comp.shared_parameter_version,
            role_quick.shared_parameter_version
        );
        assert_eq!(role_comp.role_model_ref, role_quick.role_model_ref);
        assert_ne!(role_comp.mode, role_quick.mode);
    }

    let value = serde_json::to_value((&artifact, &report)).unwrap();
    validate_report_boundary(&value).unwrap();
    let serialized = serde_json::to_string(&value).unwrap();
    for forbidden in [
        "BattleTag#1234",
        "550e8400-e29b-41d4-a716",
        "match-id-secret",
        "account-id-secret",
        "C:\\\\private\\\\owstats.sqlite3",
        "raw_json",
    ] {
        assert!(!serialized.contains(forbidden), "泄露了合成隐私标识");
    }

    let decoded: owstats_desktop_lib::career_mmr::training_report::PerformanceModelArtifact =
        serde_json::from_str(&serde_json::to_string(&artifact).unwrap()).unwrap();
    assert_eq!(decoded.schema_version, artifact.schema_version);
    assert_eq!(decoded.config_hash, artifact.config_hash);
    assert_eq!(decoded.cells, artifact.cells);
    assert_eq!(decoded.algorithm, artifact.algorithm);
    assert_eq!(decoded.feature_version, artifact.feature_version);
    assert_eq!(decoded.seed, artifact.seed);
    assert_eq!(decoded.roles, artifact.roles);
    assert_eq!(
        decoded.normalization.baselines.len(),
        artifact.normalization.baselines.len()
    );
    for (index, (decoded, original)) in decoded
        .normalization
        .baselines
        .iter()
        .zip(&artifact.normalization.baselines)
        .enumerate()
    {
        assert_eq!(decoded, original, "baseline index {index}");
    }
    assert_eq!(
        decoded.normalization.healing_expectations,
        artifact.normalization.healing_expectations
    );
    assert_eq!(decoded.normalization, artifact.normalization);
    assert_eq!(decoded.caps, artifact.caps);
    assert_eq!(decoded, artifact);
    assert_eq!(
        recompute_config_hash(&artifact).unwrap(),
        artifact.config_hash
    );
}

#[test]
fn config_hash_ignores_account_and_match_identifiers() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 7,
            ..Default::default()
        },
    );
    let first = build_training_outputs(&rows, &audit, &evaluation, 7)
        .unwrap()
        .0
        .config_hash;
    let mut renamed = rows;
    for (index, row) in renamed.iter_mut().enumerate() {
        row.account_group = format!("other-account-{index}");
        row.source_match_group = format!("other-source-match-{index}");
        row.match_group = format!("other-cross-account-match-{index}");
    }
    let second = build_training_outputs(&renamed, &audit, &evaluation, 7)
        .unwrap()
        .0
        .config_hash;
    assert_eq!(first, second);
}

#[test]
fn config_hash_covers_the_fitted_model_weights() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let mut evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 11,
            ..Default::default()
        },
    );
    let shadow_hash = build_training_outputs(&rows, &audit, &evaluation, 11)
        .unwrap()
        .0
        .config_hash;
    let cell = &mut evaluation.cells[0];
    mark_cell_eligible(cell, 0);

    let fitted_hash = build_training_outputs(&rows, &audit, &evaluation, 11)
        .unwrap()
        .0
        .config_hash;

    assert_ne!(shadow_hash, fitted_hash);
}

#[test]
fn config_hash_covers_normalization_and_every_validation_gate() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 13,
            ..Default::default()
        },
    );
    let original = build_training_outputs(&rows, &audit, &evaluation, 13)
        .unwrap()
        .0;

    let mut normalization_changed = original.clone();
    normalization_changed.normalization.baselines[0].median_per_ten += 1.0;
    assert_ne!(
        original.config_hash,
        recompute_config_hash(&normalization_changed).unwrap()
    );

    let mut gate_changed = original.clone();
    gate_changed.training_config.validation.max_ece_worsening += 0.001;
    assert_ne!(
        original.config_hash,
        recompute_config_hash(&gate_changed).unwrap()
    );
}

#[test]
fn conflicting_cell_candidates_share_one_order_independent_role_model() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let mut evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 17,
            ..Default::default()
        },
    );
    for (mode, report_index) in [
        (ModeFamily::RoleCompetitive, 0usize),
        (ModeFamily::RoleQuick, 4usize),
    ] {
        let cell = evaluation
            .cells
            .iter_mut()
            .find(|cell| cell.role == Role::Tank && cell.mode == mode)
            .unwrap();
        mark_cell_eligible(cell, report_index);
    }
    let first = build_training_outputs(&rows, &audit, &evaluation, 17)
        .unwrap()
        .0;
    evaluation.cells.reverse();
    let reordered = build_training_outputs(&rows, &audit, &evaluation, 17)
        .unwrap()
        .0;

    let pair = first
        .cells
        .iter()
        .filter(|cell| {
            cell.role == Role::Tank
                && matches!(
                    cell.mode,
                    ModeFamily::RoleCompetitive | ModeFamily::RoleQuick
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(pair.len(), 2);
    assert_eq!(pair[0].role_model_ref, pair[1].role_model_ref);
    assert!(pair[0].performance_enabled);
    assert!(!pair[1].performance_enabled);
    let losing_report = first
        .cells
        .iter()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleQuick)
        .unwrap();
    let losing_validation = build_training_outputs(&rows, &audit, &evaluation, 17)
        .unwrap()
        .1
        .cells
        .into_iter()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleQuick)
        .unwrap();
    assert_eq!(
        losing_report.performance_enabled,
        losing_validation.eligible
    );
    assert_eq!(losing_validation.decision, CellDecision::Shadow);
    assert!(losing_validation
        .rejected_reasons
        .contains(&"shared_role_model_not_validated_for_cell".into()));
    assert_eq!(first.roles[&Role::Tank], reordered.roles[&Role::Tank]);
    assert_eq!(first.cells, reordered.cells);
    assert_eq!(first.config_hash, reordered.config_hash);
}

#[test]
fn matching_shared_spec_can_enable_both_paired_modes() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let mut evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 19,
            ..Default::default()
        },
    );
    for mode in [ModeFamily::RoleCompetitive, ModeFamily::RoleQuick] {
        let cell = evaluation
            .cells
            .iter_mut()
            .find(|cell| cell.role == Role::Tank && cell.mode == mode)
            .unwrap();
        mark_cell_eligible(cell, 0);
    }
    let (_, report) = build_training_outputs(&rows, &audit, &evaluation, 19).unwrap();
    let pair = report
        .cells
        .iter()
        .filter(|cell| {
            cell.role == Role::Tank
                && matches!(
                    cell.mode,
                    ModeFamily::RoleCompetitive | ModeFamily::RoleQuick
                )
        })
        .collect::<Vec<_>>();
    assert!(pair.iter().all(|cell| cell.eligible));
}

#[test]
fn artifact_uses_the_actual_evaluation_snapshot_and_rejects_seed_mismatch() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 23,
            max_loo_worsened_fraction: 0.10,
            max_abs_average_adjustment: 0.05,
        },
    );
    assert_eq!(evaluation.config.seed, 23);
    assert_eq!(evaluation.config.max_loo_worsened_fraction, 0.10);
    assert_eq!(evaluation.config.max_abs_average_adjustment, 0.05);

    let artifact = build_training_outputs(&rows, &audit, &evaluation, 23)
        .unwrap()
        .0;
    assert_eq!(artifact.seed, 23);
    assert_eq!(
        artifact
            .training_config
            .validation
            .maximum_loo_worsened_fraction,
        0.10
    );
    assert_eq!(
        artifact
            .training_config
            .validation
            .maximum_average_adjustment_mmr,
        0.05
    );
    assert!(build_training_outputs(&rows, &audit, &evaluation, 24).is_err());
}

#[test]
fn recursively_rejects_sensitive_keys_without_echoing_values() {
    for key in [
        "token",
        "customer_token",
        "AUTHORIZATION",
        "session-cookie",
        "apiCredential",
        "battle.tag",
        "account_id",
        "PLAYER-ID",
        "bnetId",
        "match_id",
        "raw_json",
        "database.path",
    ] {
        let secret = format!("never-echo-{key}");
        let value = json!({ "outer": [{ key: secret.clone() }] });
        let error = validate_report_boundary(&value).unwrap_err().to_string();
        assert!(!error.contains(&secret));
    }

    let allowed: Value = json!({
        "algorithm": "career-role-mmr",
        "nested": [{ "trainingRows": 10, "mode": "role_quick" }]
    });
    validate_report_boundary(&allowed).unwrap();
}

#[test]
fn rejected_reasons_always_override_an_inconsistent_eligible_flag() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let mut evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 0,
            seed: 9,
            ..Default::default()
        },
    );
    let cell = &mut evaluation.cells[0];
    cell.selected = Some(cell.reports[0].strategy.clone());
    cell.decision = CellDecision::Eligible;
    cell.rejected_reasons = vec!["synthetic_rejection".into()];

    let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 9).unwrap();

    assert!(!artifact.cells[0].performance_enabled);
    assert!(!report.cells[0].eligible);
    assert_eq!(report.cells[0].decision, CellDecision::Shadow);
    assert_eq!(report.cells[0].rejected_reasons, ["synthetic_rejection"]);
}
