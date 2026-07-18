use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, Role},
    performance::{build_feature_matrix, HealingTakenStrategy},
    performance_forward::{MetricReport, ModelVariant, NestedFoldReport},
    performance_model::{fit_model, FitConfig},
    performance_stability::{
        evaluate_stability, CellDecision, CoefficientStability, PromotedCandidate, StabilityConfig,
        TrainingStage,
    },
    training::{TrainingAudit, TrainingEnvironment, TrainingRow},
    training_report::{
        build_training_outputs, recompute_config_hash, recompute_promotion_checksum,
        validate_promotion_report, validate_report_boundary,
    },
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
    let requested_iterations = report.bootstrap.requested_iterations.max(1);
    report.eligible = true;
    report.decision = CellDecision::Eligible;
    report.rejected_reasons.clear();
    report.role = cell.role;
    report.mode = cell.mode;
    report.training_cohort = cell.training_cohort;
    report.shared_parameter_version = cell.shared_parameter_version.clone();
    report.forward.role = cell.role;
    report.forward.strategy = report.strategy.clone();
    report.forward.rejected_reasons.clear();
    report.forward.metrics = Some(MetricReport {
        auc: 0.6,
        brier: 0.20,
        ece: 0.02,
    });
    report.forward.baseline_metrics = Some(MetricReport {
        auc: 0.5,
        brier: 0.25,
        ece: 0.02,
    });
    report.forward.average_adjustment = 0.01;
    report.forward.p95_single_feature_share = 0.20;
    report.forward.upper_cap_rate = 0.01;
    report.forward.lower_cap_rate = 0.01;
    report.leave_one_season_out.validation_groups = 2;
    report.leave_one_season_out.candidate_rows = 2;
    report.leave_one_season_out.evaluated_rows = 2;
    report.leave_one_season_out.mean_brier_delta = Some(-0.01);
    report.leave_one_season_out.worst_brier_delta = Some(-0.005);
    report.leave_one_hero_out.validation_groups = 5;
    report.leave_one_hero_out.candidate_rows = 5;
    report.leave_one_hero_out.evaluated_rows = 5;
    report.leave_one_hero_out.mean_brier_delta = Some(-0.01);
    report.leave_one_hero_out.worst_brier_delta = Some(-0.005);
    report.leave_one_account_out.candidate_groups = 1;
    report.leave_one_account_out.evaluated_groups = 1;
    report.leave_one_account_out.mean_brier_delta = Some(-0.01);
    report.leave_one_account_out.worst_brier_delta = Some(-0.005);
    report.bootstrap.requested_iterations = requested_iterations;
    report.bootstrap.completed_iterations = requested_iterations;
    report.bootstrap.fixed_variant = Some(report.forward.variant);
    report.bootstrap.fixed_final_multiplier = Some(report.forward.final_multiplier);
    report.bootstrap.fixed_lambda_core = Some(report.forward.lambda_core);
    report.bootstrap.fixed_lambda_cohort = Some(report.forward.lambda_cohort);
    report.bootstrap.mean_brier_delta = Some(-0.01);
    report.bootstrap.brier_delta_ci95_low = Some(-0.02);
    report.bootstrap.brier_delta_ci95_high = Some(-0.001);
    report.bootstrap.mean_auc = Some(0.6);
    report.bootstrap.mean_ece = Some(0.02);
    report.bootstrap.coefficients = [
        ("final", 0.10, 0.20),
        ("nonfinal", 0.10, 0.20),
        ("death", -0.20, -0.10),
    ]
    .into_iter()
    .map(|(weight, low, high)| CoefficientStability {
        weight: weight.into(),
        scope: "core".into(),
        direction_stability: 1.0,
        ci95_low: low,
        ci95_high: high,
        critical_constraint: true,
        reference_nonzero: true,
    })
    .collect();
    cell.selected = Some(report.strategy.clone());
    cell.decision = CellDecision::Eligible;
    cell.rejected_reasons.clear();
    cell.nested_validation.selected_strategy = report.strategy.clone();
    cell.nested_validation.selected_variant = report.forward.variant;
    cell.nested_validation.selected_final_multiplier = report.forward.final_multiplier;
    cell.nested_validation.selected_lambda_core = report.forward.lambda_core;
    cell.nested_validation.selected_lambda_cohort = report.forward.lambda_cohort;
    cell.nested_validation.improved_folds = 5;
    cell.nested_validation.role = cell.role;
    cell.nested_validation.mode = cell.mode;
    cell.nested_validation.metrics = report.forward.metrics.clone();
    cell.nested_validation.baseline_metrics = report.forward.baseline_metrics.clone();
    cell.nested_validation.selection_degraded = false;
    cell.nested_validation.folds = (0..5)
        .map(|fold| NestedFoldReport {
            fold,
            training_rows: 10,
            validation_rows: 2,
            strategy: report.strategy.clone(),
            variant: report.forward.variant,
            final_multiplier: report.forward.final_multiplier,
            lambda_core: report.forward.lambda_core,
            lambda_cohort: report.forward.lambda_cohort,
            inner_validation_rows: 4,
            selection_degraded: false,
            metrics: report.forward.metrics.clone(),
            baseline_brier: Some(0.25),
            brier_delta: Some(-0.05),
        })
        .collect();
}

fn eligible_evaluation(
    seed: u64,
) -> (
    Vec<TrainingRow>,
    TrainingAudit,
    owstats_desktop_lib::career_mmr::performance_stability::StabilityEvaluation,
) {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let mut config = StabilityConfig::for_stage(TrainingStage::Screen100);
    config.seed = seed;
    let mut evaluation = evaluate_stability(&[], config);
    let cell = evaluation
        .cells
        .iter_mut()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    mark_cell_eligible(cell, 0);
    (rows, audit, evaluation)
}

fn promoted_candidate_from_cell(
    evaluation: &owstats_desktop_lib::career_mmr::performance_stability::StabilityEvaluation,
) -> PromotedCandidate {
    let cell = evaluation
        .cells
        .iter()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    let report = cell.reports.iter().find(|report| report.eligible).unwrap();
    PromotedCandidate {
        role: Role::Tank,
        strategy: report.strategy.clone(),
        variant: report.forward.variant,
        final_multiplier: report.forward.final_multiplier,
        lambda_core: report.forward.lambda_core,
        lambda_cohort: report.forward.lambda_cohort,
    }
}

fn restrict_to_promoted(
    mut evaluation: owstats_desktop_lib::career_mmr::performance_stability::StabilityEvaluation,
    candidate: PromotedCandidate,
    stage: TrainingStage,
) -> owstats_desktop_lib::career_mmr::performance_stability::StabilityEvaluation {
    evaluation.config =
        StabilityConfig::for_stage_with_promoted(stage, vec![candidate.clone()]).snapshot();
    let bootstrap_iterations = stage.bootstrap_iterations();
    for cell in &mut evaluation.cells {
        if cell.role == candidate.role {
            cell.reports.retain(|report| {
                report.strategy == candidate.strategy
                    && report.forward.variant == candidate.variant
                    && report.forward.final_multiplier == candidate.final_multiplier
                    && report.forward.lambda_core == candidate.lambda_core
                    && report.forward.lambda_cohort == candidate.lambda_cohort
            });
            for report in &mut cell.reports {
                report.bootstrap.requested_iterations = bootstrap_iterations;
                report.bootstrap.completed_iterations = bootstrap_iterations;
                report.bootstrap.skipped_iterations = 0;
            }
        } else {
            cell.reports.clear();
        }
    }
    evaluation
}

#[test]
fn screen_candidate_final_chain_keeps_the_exact_promoted_model_eligible() {
    let (rows, audit, mut screen_evaluation) = eligible_evaluation(0x5eed);
    let role_quick = screen_evaluation
        .cells
        .iter_mut()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleQuick)
        .unwrap();
    mark_cell_eligible(role_quick, 0);
    let (_, screen_report) =
        build_training_outputs(&rows, &audit, &screen_evaluation, 0x5eed).unwrap();
    assert!(
        screen_report
            .cells
            .iter()
            .filter(|cell| cell.role == Role::Tank && cell.eligible)
            .count()
            == 2
    );

    let dir = tempfile::tempdir().unwrap();
    let read_promotion =
        |name: &str,
         report: &owstats_desktop_lib::career_mmr::training_report::TrainingReport,
         stage: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, serde_json::to_vec(report).unwrap()).unwrap();
            let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(
                recompute_promotion_checksum(&value).unwrap(),
                value["promotion_checksum"].as_str().unwrap()
            );
            validate_promotion_report(&value, stage, 0x5eed).unwrap()
        };
    let screen_candidates = read_promotion("screen.json", &screen_report, "screen_100");
    let candidate = promoted_candidate_from_cell(&screen_evaluation);
    assert_eq!(screen_candidates, vec![candidate.clone()]);
    let candidate_evaluation = restrict_to_promoted(
        screen_evaluation.clone(),
        screen_candidates[0].clone(),
        TrainingStage::Candidate1000,
    );
    let (_, candidate_report) =
        build_training_outputs(&rows, &audit, &candidate_evaluation, 0x5eed).unwrap();
    let candidate_cell = candidate_report
        .cells
        .iter()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    assert!(candidate_cell.eligible, "{candidate_cell:#?}");
    assert!(candidate_report.cells.iter().any(|cell| {
        cell.role == Role::Tank && cell.mode == ModeFamily::RoleQuick && cell.eligible
    }));

    let candidate_candidates =
        read_promotion("candidate.json", &candidate_report, "candidate_1000");
    assert_eq!(candidate_candidates, vec![candidate.clone()]);
    let filtered = evaluate_stability(
        &[],
        StabilityConfig::for_stage_with_promoted(
            TrainingStage::Candidate1000,
            candidate_candidates.clone(),
        ),
    );
    assert_eq!(filtered.config.bootstrap_iterations, 1_000);
    assert!(filtered.cells.iter().all(|cell| {
        if cell.role == Role::Tank {
            cell.reports.len() == 1
        } else {
            cell.reports.is_empty()
        }
    }));

    let final_evaluation = restrict_to_promoted(
        candidate_evaluation,
        candidate_candidates[0].clone(),
        TrainingStage::Final10000,
    );
    let (artifact, final_report) =
        build_training_outputs(&rows, &audit, &final_evaluation, 0x5eed).unwrap();
    assert!(
        final_report
            .cells
            .iter()
            .filter(|cell| cell.role == Role::Tank && cell.eligible)
            .count()
            == 2
    );
    assert!(artifact.roles[&Role::Tank].model_spec_id.is_some());
    assert_eq!(final_report.training_config.validation.stage, "final_10000");
    assert_eq!(
        final_report.training_config.validation.bootstrap_iterations,
        10_000
    );
    let final_value = serde_json::to_value(&final_report).unwrap();
    assert_eq!(
        recompute_promotion_checksum(&final_value).unwrap(),
        final_report.promotion_checksum
    );
    assert!(!rows.is_empty());
    assert_eq!(
        ModelVariant::Core,
        artifact.roles[&Role::Tank]
            .selected_variant
            .as_deref()
            .map(|variant| if variant == "core" {
                ModelVariant::Core
            } else {
                ModelVariant::RoleFull
            })
            .unwrap()
    );
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
            bootstrap_iterations: 10,
            seed: 0x5eed,
            ..Default::default()
        },
    );

    let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 0x5eed).unwrap();

    assert_eq!(artifact.schema_version, 5);
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
fn promotion_checksum_covers_shadow_nested_and_dataset_content_but_is_not_authentication() {
    let (rows, audit, evaluation) = eligible_evaluation(91);
    let (_, report) = build_training_outputs(&rows, &audit, &evaluation, 91).unwrap();
    let original = serde_json::to_value(&report).unwrap();
    assert_eq!(report.integrity_kind, "checksum_not_authentication");
    for pointer in [
        "/dataset/projected_rows",
        "/cells/1/metrics/nested_validation/improved_folds",
    ] {
        let mut changed = original.clone();
        let value = changed.pointer_mut(pointer).unwrap();
        *value = serde_json::json!(value.as_u64().unwrap_or(0) + 1);
        assert_ne!(
            recompute_promotion_checksum(&changed).unwrap(),
            report.promotion_checksum,
            "{pointer}"
        );
    }
}

#[test]
fn each_actual_cell_stores_its_own_training_logit_effect_p95() {
    let (mut rows, audit, mut evaluation) = eligible_evaluation(73);
    let quick = evaluation
        .cells
        .iter_mut()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleQuick)
        .unwrap();
    mark_cell_eligible(quick, 0);
    rows.retain(|row| row.role != "tank");
    for index in 0..40 {
        let mut competitive = row(1_000 + index, "tank", ModeFamily::RoleCompetitive);
        competitive.hero_group = Some("same_tank".into());
        competitive.final_hit = Some(2.0 + index as f64 * 0.2);
        competitive.kill = competitive.final_hit.map(|v| v + 2.0);
        rows.push(competitive);
        let mut quick = row(2_000 + index, "tank", ModeFamily::RoleQuick);
        quick.hero_group = Some("same_tank".into());
        quick.final_hit = Some(if index < 38 {
            5.0
        } else {
            100.0 + index as f64
        });
        quick.kill = quick.final_hit.map(|v| v + 2.0);
        rows.push(quick);
    }
    let artifact = build_training_outputs(&rows, &audit, &evaluation, 73)
        .unwrap()
        .0;
    let competitive = artifact
        .cells
        .iter()
        .find(|c| c.role == Role::Tank && c.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    let quick = artifact
        .cells
        .iter()
        .find(|c| c.role == Role::Tank && c.mode == ModeFamily::RoleQuick)
        .unwrap();
    assert!(competitive.calibration_p95_abs_logit_effect.is_finite());
    assert!(quick.calibration_p95_abs_logit_effect.is_finite());
    assert_ne!(
        competitive.calibration_p95_abs_logit_effect,
        quick.calibration_p95_abs_logit_effect
    );
    let decoded: owstats_desktop_lib::career_mmr::training_report::PerformanceModelArtifact =
        serde_json::from_str(&serde_json::to_string(&artifact).unwrap()).unwrap();
    assert_eq!(decoded.cells, artifact.cells);
    let original_hash = artifact.config_hash.clone();
    let mut changed = artifact;
    changed
        .cells
        .iter_mut()
        .find(|c| c.role == Role::Tank && c.mode == ModeFamily::RoleQuick)
        .unwrap()
        .calibration_p95_abs_logit_effect += 0.5;
    assert_ne!(recompute_config_hash(&changed).unwrap(), original_hash);
}

#[test]
fn artifact_boundary_rejects_nonfinite_healing_expectation_values() {
    let (rows, audit, evaluation) = eligible_evaluation(79);
    let artifact = build_training_outputs(&rows, &audit, &evaluation, 79)
        .unwrap()
        .0;
    assert!(!artifact.normalization.healing_expectations.is_empty());
    let mut invalid_fallback = artifact.clone();
    invalid_fallback.normalization.healing_expectations[0].fallback_median_per_ten = f64::NAN;
    assert!(recompute_config_hash(&invalid_fallback).is_err());
    let mut invalid_coefficients = artifact;
    invalid_coefficients.normalization.healing_expectations[0].coefficients =
        Some([0.0, f64::INFINITY, 0.0, 0.0, 0.0]);
    assert!(recompute_config_hash(&invalid_coefficients).is_err());
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
            bootstrap_iterations: 10,
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
            bootstrap_iterations: 10,
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
            bootstrap_iterations: 10,
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
            bootstrap_iterations: 10,
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
            bootstrap_iterations: 10,
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
            bootstrap_iterations: 10,
            seed: 23,
            max_loo_worsened_fraction: 0.10,
            max_abs_average_adjustment: 0.05,
            promoted_candidates: None,
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
            bootstrap_iterations: 10,
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

#[test]
fn artifact_boundary_shadows_every_incomplete_or_mismatched_validation_identity() {
    for case in [
        "cell_cohort",
        "report_mode",
        "forward_role",
        "nested_mode",
        "nested_strategy",
        "missing_nested_metrics",
        "nan_forward_metric",
        "missing_bootstrap_ci",
        "non_candidate_multiplier",
    ] {
        let (rows, audit, mut evaluation) = eligible_evaluation(31);
        let cell = evaluation
            .cells
            .iter_mut()
            .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
            .unwrap();
        match case {
            "cell_cohort" => {
                cell.training_cohort =
                    owstats_desktop_lib::career_mmr::input::TrainingCohort::OpenQueue
            }
            "report_mode" => cell.reports[0].mode = ModeFamily::OpenCompetitive,
            "forward_role" => cell.reports[0].forward.role = Role::Damage,
            "nested_mode" => cell.nested_validation.mode = ModeFamily::RoleQuick,
            "nested_strategy" => cell.nested_validation.selected_strategy = "directnegative".into(),
            "missing_nested_metrics" => cell.nested_validation.metrics = None,
            "nan_forward_metric" => {
                cell.reports[0].forward.metrics.as_mut().unwrap().brier = f64::NAN
            }
            "missing_bootstrap_ci" => cell.reports[0].bootstrap.brier_delta_ci95_high = None,
            "non_candidate_multiplier" => {
                cell.reports[0].forward.final_multiplier = 1.0000000000000002;
                cell.nested_validation.selected_final_multiplier = 1.0000000000000002;
                for fold in &mut cell.nested_validation.folds {
                    fold.final_multiplier = 1.0000000000000002;
                }
            }
            _ => unreachable!(),
        }
        let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 31).unwrap();
        let deployed = artifact
            .cells
            .iter()
            .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
            .unwrap();
        let reported = report
            .cells
            .iter()
            .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
            .unwrap();
        assert!(!deployed.performance_enabled, "case {case}");
        assert_eq!(reported.decision, CellDecision::Shadow, "case {case}");
        assert!(
            reported
                .rejected_reasons
                .contains(&"invalid_validation_evidence".to_string()),
            "case {case}: {:?}",
            reported.rejected_reasons
        );
    }
}

#[test]
fn non_default_fit_snapshot_is_used_exactly_by_authoritative_refit() {
    let (rows, audit, mut evaluation) = eligible_evaluation(37);
    evaluation.config.fit_learning_rate = 0.007;
    evaluation.config.fit_steps = 7;
    evaluation.config.minimum_cohort_offset_rows = 1;
    let artifact = build_training_outputs(&rows, &audit, &evaluation, 37)
        .unwrap()
        .0;
    let role_artifact = &artifact.roles[&Role::Tank];
    let matrix = build_feature_matrix(&rows, &[], HealingTakenStrategy::None);
    let role_rows = matrix.training_by_role["tank"].iter().collect::<Vec<_>>();
    let expected = fit_model(
        &role_rows,
        Role::Tank,
        cell_variant(&evaluation),
        HealingTakenStrategy::None,
        &FitConfig {
            lambda_core: role_artifact.lambda_core.unwrap(),
            lambda_cohort: role_artifact.lambda_cohort.unwrap(),
            learning_rate: 0.007,
            steps: 7,
            min_cohort_offset_rows: 1,
            final_multiplier: role_artifact.final_hit_multiplier.unwrap(),
        },
    )
    .unwrap();
    for (name, weight) in expected.feature_names.iter().zip(&expected.core_weights) {
        assert_eq!(role_artifact.feature_weights[name], *weight);
    }
    for (cohort, offsets) in &expected.cohort_offsets {
        for (name, offset) in expected.feature_names.iter().zip(offsets) {
            assert_eq!(role_artifact.training_cohort_offsets[cohort][name], *offset);
        }
    }
    assert_eq!(artifact.training_config.fit.learning_rate, 0.007);
    assert_eq!(artifact.training_config.fit.steps, 7);
    assert_eq!(artifact.training_config.fit.minimum_cohort_offset_rows, 1);
}

fn cell_variant(
    evaluation: &owstats_desktop_lib::career_mmr::performance_stability::StabilityEvaluation,
) -> owstats_desktop_lib::career_mmr::performance_forward::ModelVariant {
    evaluation
        .cells
        .iter()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap()
        .reports[0]
        .forward
        .variant
}

#[test]
fn model_spec_id_is_canonical_and_rejects_adjacent_or_ambiguous_candidates() {
    let (rows, audit, evaluation) = eligible_evaluation(41);
    let first = build_training_outputs(&rows, &audit, &evaluation, 41)
        .unwrap()
        .0;
    let decoded: owstats_desktop_lib::career_mmr::training_report::PerformanceModelArtifact =
        serde_json::from_str(&serde_json::to_string(&first).unwrap()).unwrap();
    assert_eq!(
        first.roles[&Role::Tank].model_spec_id,
        decoded.roles[&Role::Tank].model_spec_id
    );
    assert!(first.roles[&Role::Tank].model_spec_id.is_some());

    let mut adjacent = evaluation.clone();
    let cell = adjacent
        .cells
        .iter_mut()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    cell.reports[0].forward.lambda_core = f64::from_bits(0.02f64.to_bits() + 1);
    cell.nested_validation.selected_lambda_core = cell.reports[0].forward.lambda_core;
    for fold in &mut cell.nested_validation.folds {
        fold.lambda_core = cell.reports[0].forward.lambda_core;
    }
    let artifact = build_training_outputs(&rows, &audit, &adjacent, 41)
        .unwrap()
        .0;
    assert!(
        !artifact
            .cells
            .iter()
            .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
            .unwrap()
            .performance_enabled
    );

    let mut ambiguous = evaluation;
    ambiguous.config.final_hit_multipliers.push(1.0);
    assert!(build_training_outputs(&rows, &audit, &ambiguous, 41).is_err());
}

#[test]
fn canonical_degraded_fallback_from_real_evaluation_can_be_eligible() {
    let rows = synthetic_rows();
    let audit = TrainingAudit {
        source_accounts: 2,
        projected_rows: rows.len(),
        ..Default::default()
    };
    let mut evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            bootstrap_iterations: 10,
            seed: 43,
            ..Default::default()
        },
    );
    let cell = evaluation
        .cells
        .iter_mut()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    assert!(cell.nested_validation.selection_degraded);
    assert_eq!(cell.nested_validation.selected_strategy, "none");
    assert_eq!(
        cell.nested_validation.selected_variant,
        owstats_desktop_lib::career_mmr::performance_forward::ModelVariant::Core
    );
    assert_eq!(cell.nested_validation.selected_final_multiplier, 1.25);
    mark_cell_eligible(cell, 0);
    cell.nested_validation.selection_degraded = true;
    for fold in &mut cell.nested_validation.folds {
        fold.selection_degraded = true;
    }

    let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 43).unwrap();
    assert!(artifact.cells.iter().any(|cell| cell.role == Role::Tank
        && cell.mode == ModeFamily::RoleCompetitive
        && cell.performance_enabled));
    assert!(report.cells.iter().any(|cell| cell.role == Role::Tank
        && cell.mode == ModeFamily::RoleCompetitive
        && cell.decision == CellDecision::Eligible));
}

#[test]
fn degraded_complex_spec_is_shadowed() {
    let (rows, audit, mut evaluation) = eligible_evaluation(47);
    let cell = evaluation
        .cells
        .iter_mut()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    cell.nested_validation.selection_degraded = true;
    cell.nested_validation.selected_variant =
        owstats_desktop_lib::career_mmr::performance_forward::ModelVariant::RoleFull;
    cell.reports[0].forward.variant =
        owstats_desktop_lib::career_mmr::performance_forward::ModelVariant::RoleFull;
    cell.reports[0].bootstrap.fixed_variant =
        Some(owstats_desktop_lib::career_mmr::performance_forward::ModelVariant::RoleFull);
    for fold in &mut cell.nested_validation.folds {
        fold.selection_degraded = true;
        fold.variant = owstats_desktop_lib::career_mmr::performance_forward::ModelVariant::RoleFull;
    }
    let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 47).unwrap();
    assert_shadow(&artifact, &report, Role::Tank, ModeFamily::RoleCompetitive);
}

#[test]
fn incomplete_duplicate_or_wrong_scope_critical_coefficients_are_shadowed() {
    for case in [
        "missing",
        "duplicate",
        "duplicate_noncritical",
        "wrong_scope",
        "cross_zero",
    ] {
        let (rows, audit, mut evaluation) = eligible_evaluation(53);
        let report = &mut evaluation
            .cells
            .iter_mut()
            .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
            .unwrap()
            .reports[0];
        match case {
            "missing" => report
                .bootstrap
                .coefficients
                .retain(|coefficient| coefficient.weight != "final"),
            "duplicate" => report
                .bootstrap
                .coefficients
                .push(report.bootstrap.coefficients[0].clone()),
            "duplicate_noncritical" => {
                for scope in ["core", "cohort"] {
                    report.bootstrap.coefficients.push(CoefficientStability {
                        weight: "assist".into(),
                        scope: scope.into(),
                        direction_stability: 0.8,
                        ci95_low: -0.1,
                        ci95_high: 0.2,
                        critical_constraint: false,
                        reference_nonzero: false,
                    });
                }
            }
            "wrong_scope" => {
                report
                    .bootstrap
                    .coefficients
                    .iter_mut()
                    .find(|coefficient| coefficient.weight == "final")
                    .unwrap()
                    .scope = "cohort".into()
            }
            "cross_zero" => {
                let coefficient = report
                    .bootstrap
                    .coefficients
                    .iter_mut()
                    .find(|coefficient| coefficient.weight == "death")
                    .unwrap();
                coefficient.ci95_low = -0.1;
                coefficient.ci95_high = 0.1;
            }
            _ => unreachable!(),
        }
        let (artifact, training_report) =
            build_training_outputs(&rows, &audit, &evaluation, 53).unwrap();
        assert_shadow(
            &artifact,
            &training_report,
            Role::Tank,
            ModeFamily::RoleCompetitive,
        );
    }
}

#[test]
fn resource_debt_requires_one_negative_core_debt_coefficient() {
    for case in ["missing", "wrong_scope", "nonnegative_ci"] {
        let rows = synthetic_rows();
        let audit = TrainingAudit {
            source_accounts: 2,
            projected_rows: rows.len(),
            ..Default::default()
        };
        let mut evaluation = evaluate_stability(
            &[],
            StabilityConfig {
                bootstrap_iterations: 10,
                seed: 59,
                ..Default::default()
            },
        );
        let cell = evaluation
            .cells
            .iter_mut()
            .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
            .unwrap();
        mark_cell_eligible(cell, 4);
        if case != "missing" {
            cell.reports[4]
                .bootstrap
                .coefficients
                .push(CoefficientStability {
                    weight: "debt".into(),
                    scope: if case == "wrong_scope" {
                        "cohort".into()
                    } else {
                        "core".into()
                    },
                    direction_stability: 1.0,
                    ci95_low: if case == "nonnegative_ci" { -0.1 } else { -0.2 },
                    ci95_high: if case == "nonnegative_ci" { 0.1 } else { -0.1 },
                    critical_constraint: true,
                    reference_nonzero: true,
                });
        }
        let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 59).unwrap();
        assert_shadow(&artifact, &report, Role::Tank, ModeFamily::RoleCompetitive);
    }
}

#[test]
fn account_coverage_limited_group_is_shadowed() {
    let (rows, audit, mut evaluation) = eligible_evaluation(61);
    evaluation
        .cells
        .iter_mut()
        .find(|cell| cell.role == Role::Tank && cell.mode == ModeFamily::RoleCompetitive)
        .unwrap()
        .reports[0]
        .leave_one_account_out
        .coverage_limited_groups = 1;
    let (artifact, report) = build_training_outputs(&rows, &audit, &evaluation, 61).unwrap();
    assert_shadow(&artifact, &report, Role::Tank, ModeFamily::RoleCompetitive);
}

fn assert_shadow(
    artifact: &owstats_desktop_lib::career_mmr::training_report::PerformanceModelArtifact,
    report: &owstats_desktop_lib::career_mmr::training_report::TrainingReport,
    role: Role,
    mode: ModeFamily,
) {
    assert!(
        !artifact
            .cells
            .iter()
            .find(|cell| cell.role == role && cell.mode == mode)
            .unwrap()
            .performance_enabled
    );
    let cell = report
        .cells
        .iter()
        .find(|cell| cell.role == role && cell.mode == mode)
        .unwrap();
    assert_eq!(cell.decision, CellDecision::Shadow);
    assert!(cell
        .rejected_reasons
        .contains(&"invalid_validation_evidence".to_string()));
}
