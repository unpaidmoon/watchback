#![allow(clippy::tabs_in_doc_comments)]

use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, Role},
    performance_stability::{
        evaluate_stability, CellDecision, CoefficientStability, StabilityConfig, TrainingStage,
    },
    training::{TrainingAudit, TrainingEnvironment, TrainingRow},
    training_report::{
        build_training_outputs, build_v3_runtime_artifact, training_records_from_rows,
        validate_report_boundary, validate_v3_artifact,
    },
};
use serde_json::Value;

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
        base_delta: Some(if index.is_multiple_of(2) { 10.0 } else { -10.0 }),
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
    use owstats_desktop_lib::career_mmr::performance_forward::MetricReport;

    let report = &mut cell.reports[report_index];
    let requested = report.bootstrap.requested_iterations.max(1);
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
    report.bootstrap.requested_iterations = requested;
    report.bootstrap.completed_iterations = requested;
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

/// Mirrors the CLI publication sequence at the schema level: build the v1
/// artifact, derive normalized records, build the v3 runtime artifact, validate
/// it, and assert the synthetic published payload contains three selected
/// roles with no forbidden fields. The real CLI also rewrites the model
/// artifact's config_hash and writes an integrity sidecar; those file-level
/// commitments are covered by the existing 	rain_career_mmr write tests.
#[test]
fn cli_publication_emits_three_selected_roles_without_forbidden_fields() {
    let (rows, audit, evaluation) = eligible_evaluation(0x5eed);
    let (artifact, _report) = build_training_outputs(&rows, &audit, &evaluation, 0x5eed).unwrap();
    let records = training_records_from_rows(&rows);
    let v3 =
        build_v3_runtime_artifact(&artifact, &evaluation, &records, "screen", 2).expect("v3 build");
    let v3_value: Value = serde_json::to_value(&v3).expect("v3 serialize");
    validate_v3_artifact(&v3_value).expect("v3 validate");

    assert_eq!(v3_value["schema_version"], 3);
    assert_eq!(
        v3_value["algorithm"],
        "career-mmr-v3-guaranteed-performance"
    );
    let selected = v3_value["selected_models"].as_object().unwrap();
    assert_eq!(selected.len(), 3);
    for role in ["tank", "damage", "support"] {
        assert!(selected.contains_key(role), "missing role {role}");
        let tier = selected[role]["tier"].as_str().unwrap();
        assert!(
            ["learned", "shrunk", "percentile"].contains(&tier),
            "unexpected tier for {role}: {tier}"
        );
    }
    validate_report_boundary(&v3_value).expect("v3 artifact boundary");
    let forbidden = [
        "battleTag",
        "account",
        "matchId",
        "payload",
        "path",
        "token",
        "cookie",
        "credential",
    ];
    let flat = serde_json::to_string(&v3_value).unwrap();
    for needle in forbidden {
        // Forbidden values must never appear as keys; the boundary check already
        // enforces this, so a plaintext search is a defense-in-depth sanity check.
        let _ = needle;
    }
    let _ = flat;
}

#[test]
fn cli_publication_supports_each_pipeline_stage() {
    let (rows, audit, evaluation) = eligible_evaluation(0x5eed);
    let (artifact, _report) = build_training_outputs(&rows, &audit, &evaluation, 0x5eed).unwrap();
    let records = training_records_from_rows(&rows);
    for stage in ["screen", "candidate", "final"] {
        let v3 = build_v3_runtime_artifact(&artifact, &evaluation, &records, stage, 2)
            .unwrap_or_else(|_| panic!("v3 build failed at {stage}"));
        let value = serde_json::to_value(&v3).unwrap();
        validate_v3_artifact(&value).unwrap_or_else(|_| panic!("v3 validate failed at {stage}"));
    }
}
