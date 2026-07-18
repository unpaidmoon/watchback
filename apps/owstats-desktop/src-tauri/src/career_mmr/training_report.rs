use super::{
    input::{ModeFamily, Role, TrainingCohort},
    performance::{build_feature_matrix, HealingTakenStrategy},
    performance_forward::{CalibrationScale, ModelVariant, NestedCellReport},
    performance_model::{fit_model, predict_probability, FitConfig},
    performance_stability::{
        CellDecision, CellEvaluation, StabilityConfigSnapshot, StabilityEvaluation,
        StabilityReport, TrainingStage,
    },
    training::{TrainingAudit, TrainingEnvironment, TrainingRow},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: u32 = 2;
const ALGORITHM: &str = "career-role-mmr-hierarchical-logit-v1";
const FEATURE_VERSION: &str = "role-features-and-resource-debt-v1";
const MAD_SCALE: f64 = 0.6745;
const MAD_FLOOR_ABSOLUTE: f64 = 1.0e-6;
const MAD_FLOOR_RELATIVE: f64 = 1.0e-6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceModelArtifact {
    pub schema_version: u32,
    pub algorithm: String,
    pub feature_version: String,
    pub seed: u64,
    pub roles: BTreeMap<Role, HierarchicalModelArtifact>,
    pub cells: Vec<CellArtifact>,
    pub normalization: NormalizationArtifact,
    pub caps: PerformanceCaps,
    pub training_config: TrainingConfigArtifact,
    pub sample_buckets: Vec<AggregateSampleBucket>,
    pub config_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HierarchicalModelArtifact {
    pub selected_strategy: Option<String>,
    pub selected_variant: Option<String>,
    pub feature_weights: BTreeMap<String, f64>,
    pub training_cohort_offsets: BTreeMap<TrainingCohort, BTreeMap<String, f64>>,
    pub training_cohort_samples: BTreeMap<TrainingCohort, usize>,
    pub final_hit_multiplier: Option<f64>,
    pub lambda_core: Option<f64>,
    pub lambda_cohort: Option<f64>,
    pub calibration_p95_abs_effect: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellArtifact {
    pub role: Role,
    pub mode: ModeFamily,
    pub training_cohort: TrainingCohort,
    pub shared_parameter_version: String,
    pub role_model_ref: String,
    pub performance_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingConfigArtifact {
    pub seed: u64,
    pub mode_weights: BTreeMap<ModeFamily, f64>,
    pub fit: FitSearchArtifact,
    pub validation: ValidationGateArtifact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitSearchArtifact {
    pub variants: Vec<String>,
    pub final_hit_multipliers: Vec<f64>,
    pub regularization_candidates: Vec<RegularizationCandidate>,
    pub learning_rate: f64,
    pub steps: usize,
    pub minimum_cohort_offset_rows: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegularizationCandidate {
    pub lambda_core: f64,
    pub lambda_cohort: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationGateArtifact {
    pub stage: String,
    pub bootstrap_iterations: usize,
    pub minimum_improved_folds: usize,
    pub minimum_role_core_rows: usize,
    pub minimum_cohort_rows: usize,
    pub minimum_season_loo_groups: usize,
    pub minimum_hero_loo_groups: usize,
    pub minimum_loo_row_coverage: f64,
    pub maximum_loo_worsened_fraction: f64,
    pub minimum_bootstrap_completion: f64,
    pub minimum_critical_direction_stability: f64,
    pub maximum_bootstrap_brier_ci_high: f64,
    pub max_ece_worsening: f64,
    pub maximum_single_feature_share: f64,
    pub maximum_average_adjustment_mmr: f64,
    pub maximum_cap_hit_rate: f64,
    pub require_account_coverage: bool,
    pub require_critical_ci_excludes_zero: bool,
    pub require_nonzero_reference_coefficients: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateSampleBucket {
    pub role: String,
    pub mode: ModeFamily,
    pub season: i64,
    pub rows: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NormalizationArtifact {
    pub hierarchy: Vec<String>,
    pub minimum_hero_samples: usize,
    pub z_clip: f64,
    pub mad_scale: f64,
    pub mad_floor_absolute: f64,
    pub mad_floor_relative: f64,
    pub healing_min_regression_samples: usize,
    pub healing_huber_iterations: usize,
    pub healing_ridge_lambda: f64,
    pub baselines: Vec<BaselineArtifact>,
    pub healing_expectations: Vec<HealingExpectationArtifact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineArtifact {
    pub role: String,
    pub feature: String,
    pub environment: String,
    pub hero: Option<String>,
    pub mode: Option<ModeFamily>,
    pub season: Option<i64>,
    pub sample_count: usize,
    pub median_per_ten: f64,
    pub mad_per_ten: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealingExpectationArtifact {
    pub role: String,
    pub environment: String,
    pub hero: Option<String>,
    pub mode: Option<ModeFamily>,
    pub season: Option<i64>,
    pub sample_count: usize,
    pub fallback_median_per_ten: f64,
    pub coefficients: Option<[f64; 5]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceCaps {
    pub competitive_match_delta_mmr: f64,
    pub quick_match_delta_mmr: f64,
    pub relative_to_base_delta: f64,
    pub competitive_mmr: f64,
    pub support_competitive_mmr: f64,
    pub quick_mmr: f64,
    pub stadium_quick_mmr: f64,
}

impl Default for PerformanceCaps {
    fn default() -> Self {
        Self {
            competitive_match_delta_mmr: 30.0,
            quick_match_delta_mmr: 10.0,
            relative_to_base_delta: 0.10,
            competitive_mmr: 3.0,
            support_competitive_mmr: 2.0,
            quick_mmr: 1.0,
            stadium_quick_mmr: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrainingReport {
    pub schema_version: u32,
    pub algorithm: String,
    pub feature_version: String,
    pub seed: u64,
    pub dataset: DatasetReport,
    pub cells: Vec<CellReport>,
    pub stadium_quick: OutcomeOnlyReport,
    pub training_config: TrainingConfigArtifact,
    pub config_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatasetReport {
    pub account_datasets: usize,
    pub projected_rows: usize,
    pub performance_rows: usize,
    pub seasons: usize,
    pub mode_rows: BTreeMap<ModeFamily, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CellReport {
    pub role: Role,
    pub mode: ModeFamily,
    pub training_cohort: TrainingCohort,
    pub shared_parameter_version: String,
    pub role_model_ref: String,
    pub training_rows: usize,
    pub validation_rows: usize,
    pub seasons: usize,
    pub heroes: usize,
    pub decision: CellDecision,
    pub eligible: bool,
    pub rejected_reasons: Vec<String>,
    pub metrics: CellMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CellMetrics {
    pub candidate_auc: Option<f64>,
    pub candidate_brier: Option<f64>,
    pub candidate_ece: Option<f64>,
    pub baseline_auc: Option<f64>,
    pub baseline_brier: Option<f64>,
    pub baseline_ece: Option<f64>,
    pub improved_folds: usize,
    pub nested_validation: NestedCellReport,
    pub strategy_reports: Vec<StabilityReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutcomeOnlyReport {
    pub mode: ModeFamily,
    pub rows: usize,
    pub decision: CellDecision,
    pub performance_adjustment_mmr: i32,
}

pub fn build_training_outputs(
    rows: &[TrainingRow],
    audit: &TrainingAudit,
    evaluation: &StabilityEvaluation,
    seed: u64,
) -> Result<(PerformanceModelArtifact, TrainingReport)> {
    require_complete_cells(evaluation)?;
    if seed != evaluation.config.seed {
        bail!("模型产物种子与训练评估配置不一致");
    }
    let normalization = build_normalization(rows);
    let roles = build_role_artifacts(rows, evaluation)?;
    let cells = build_cell_artifacts(evaluation, &roles);
    let report_cells = build_cell_reports(rows, evaluation, &roles)?;
    let caps = PerformanceCaps::default();
    let training_config = training_config(evaluation);
    let sample_buckets = aggregate_sample_buckets(rows);
    let mut artifact = PerformanceModelArtifact {
        schema_version: SCHEMA_VERSION,
        algorithm: ALGORITHM.into(),
        feature_version: FEATURE_VERSION.into(),
        seed: evaluation.config.seed,
        roles,
        cells,
        normalization,
        caps,
        training_config: training_config.clone(),
        sample_buckets,
        config_hash: String::new(),
    };
    // The immutable artifact is returned in exactly the representation that is
    // persisted. Besides rejecting non-finite floats through deserialization,
    // this avoids adjacent-float differences after a JSON save/load cycle.
    artifact = serde_json::from_str(&serde_json::to_string(&artifact)?)?;
    artifact.config_hash = recompute_config_hash(&artifact)?;
    artifact = serde_json::from_str(&serde_json::to_string(&artifact)?)?;
    let config_hash = artifact.config_hash.clone();
    let report = TrainingReport {
        schema_version: SCHEMA_VERSION,
        algorithm: ALGORITHM.into(),
        feature_version: FEATURE_VERSION.into(),
        seed: evaluation.config.seed,
        dataset: dataset_report(rows, audit),
        cells: report_cells,
        stadium_quick: OutcomeOnlyReport {
            mode: ModeFamily::StadiumQuick,
            rows: rows
                .iter()
                .filter(|row| row.mode_family == ModeFamily::StadiumQuick)
                .count(),
            decision: CellDecision::OutcomeOnly,
            performance_adjustment_mmr: 0,
        },
        training_config,
        config_hash,
    };
    validate_report_boundary(&serde_json::to_value(&artifact)?)?;
    validate_report_boundary(&serde_json::to_value(&report)?)?;
    Ok((artifact, report))
}

fn build_cell_artifacts(
    evaluation: &StabilityEvaluation,
    roles: &BTreeMap<Role, HierarchicalModelArtifact>,
) -> Vec<CellArtifact> {
    let mut cells = evaluation
        .cells
        .iter()
        .map(|cell| {
            let role_model = &roles[&cell.role];
            CellArtifact {
                role: cell.role,
                mode: cell.mode,
                training_cohort: cell.mode.training_cohort(),
                shared_parameter_version: shared_parameter_version(cell.mode.training_cohort()),
                role_model_ref: role_model_ref(cell.role, role_model),
                performance_enabled: cell_validates_role_model(
                    cell,
                    role_model,
                    &evaluation.config,
                ),
            }
        })
        .collect::<Vec<_>>();
    cells.sort_by_key(|cell| (cell.role, cell.mode));
    cells
}

fn build_role_artifacts(
    rows: &[TrainingRow],
    evaluation: &StabilityEvaluation,
) -> Result<BTreeMap<Role, HierarchicalModelArtifact>> {
    let mut roles = BTreeMap::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        let reports = evaluation
            .cells
            .iter()
            .filter(|cell| cell.role == role)
            .filter_map(|cell| {
                let selected = cell.selected.as_deref()?;
                let report = cell
                    .reports
                    .iter()
                    .find(|report| report.strategy == selected && report.eligible)?;
                cell_validates_report(cell, report, &evaluation.config).then_some(report)
            })
            .collect::<Vec<_>>();
        let artifact = if let Some(promoted) = promote_role_candidate(&reports) {
            refit_artifact(rows, promoted)
                .ok_or_else(|| anyhow::anyhow!("已通过验收的职责无法生成权威模型产物"))?
        } else {
            HierarchicalModelArtifact::default()
        };
        roles.insert(role, artifact);
    }
    Ok(roles)
}

fn promote_role_candidate<'a>(reports: &[&'a StabilityReport]) -> Option<&'a StabilityReport> {
    let mut candidates = reports
        .iter()
        .map(|report| {
            let support = reports
                .iter()
                .filter(|candidate| same_model_spec(report, candidate))
                .count();
            (*report, support)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|(left, left_support), (right, right_support)| {
        right_support
            .cmp(left_support)
            .then_with(|| {
                variant_rank(left.forward.variant).cmp(&variant_rank(right.forward.variant))
            })
            .then_with(|| strategy_rank(&left.strategy).cmp(&strategy_rank(&right.strategy)))
            .then_with(|| {
                left.forward
                    .final_multiplier
                    .total_cmp(&right.forward.final_multiplier)
            })
            .then_with(|| {
                left.forward
                    .lambda_core
                    .total_cmp(&right.forward.lambda_core)
            })
            .then_with(|| {
                left.forward
                    .lambda_cohort
                    .total_cmp(&right.forward.lambda_cohort)
            })
    });
    candidates.first().map(|(report, _)| *report)
}

fn same_model_spec(left: &StabilityReport, right: &StabilityReport) -> bool {
    left.strategy == right.strategy
        && left.forward.variant == right.forward.variant
        && left.forward.final_multiplier.to_bits() == right.forward.final_multiplier.to_bits()
        && left.forward.lambda_core.to_bits() == right.forward.lambda_core.to_bits()
        && left.forward.lambda_cohort.to_bits() == right.forward.lambda_cohort.to_bits()
}

fn cell_validates_report(
    cell: &CellEvaluation,
    report: &StabilityReport,
    config: &StabilityConfigSnapshot,
) -> bool {
    cell_is_eligible(cell)
        && cell.selected.as_deref() == Some(report.strategy.as_str())
        && report.eligible
        && cell.nested_validation.selected_strategy == report.strategy
        && cell.nested_validation.selected_variant == report.forward.variant
        && cell.nested_validation.selected_final_multiplier.to_bits()
            == report.forward.final_multiplier.to_bits()
        && cell.nested_validation.selected_lambda_core.to_bits()
            == report.forward.lambda_core.to_bits()
        && cell.nested_validation.selected_lambda_cohort.to_bits()
            == report.forward.lambda_cohort.to_bits()
        && cell.nested_validation.improved_folds >= config.minimum_improved_folds
        && cell
            .nested_validation
            .metrics
            .as_ref()
            .zip(cell.nested_validation.baseline_metrics.as_ref())
            .is_none_or(|(candidate, baseline)| {
                candidate.ece <= baseline.ece + config.max_ece_worsening
            })
}

fn cell_validates_role_model(
    cell: &CellEvaluation,
    model: &HierarchicalModelArtifact,
    config: &StabilityConfigSnapshot,
) -> bool {
    let Some(strategy) = model.selected_strategy.as_deref() else {
        return false;
    };
    cell.reports.iter().any(|report| {
        report.strategy == strategy
            && artifact_matches_report(model, report)
            && cell_validates_report(cell, report, config)
    })
}

fn artifact_matches_report(model: &HierarchicalModelArtifact, report: &StabilityReport) -> bool {
    model.selected_strategy.as_deref() == Some(report.strategy.as_str())
        && model.selected_variant.as_deref() == Some(variant_name(report.forward.variant))
        && model.final_hit_multiplier.map(f64::to_bits)
            == Some(report.forward.final_multiplier.to_bits())
        && model.lambda_core.map(f64::to_bits) == Some(report.forward.lambda_core.to_bits())
        && model.lambda_cohort.map(f64::to_bits) == Some(report.forward.lambda_cohort.to_bits())
}

fn variant_rank(variant: ModelVariant) -> usize {
    match variant {
        ModelVariant::Core => 0,
        ModelVariant::RoleFull => 1,
    }
}

fn strategy_rank(strategy: &str) -> usize {
    match strategy {
        "none" => 0,
        "directnegative" => 1,
        "heronormalized" => 2,
        "excesshealing" => 3,
        "resourceconversiondebt" => 4,
        _ => usize::MAX,
    }
}

fn refit_artifact(
    rows: &[TrainingRow],
    report: &super::performance_stability::StabilityReport,
) -> Option<HierarchicalModelArtifact> {
    let strategy = parse_strategy(&report.strategy)?;
    let matrix = build_feature_matrix(rows, &[], strategy);
    let role_rows = matrix
        .training_by_role
        .get(role_name(report.role))?
        .iter()
        .collect::<Vec<_>>();
    let config = FitConfig {
        lambda_core: report.forward.lambda_core,
        lambda_cohort: report.forward.lambda_cohort,
        final_multiplier: report.forward.final_multiplier,
        ..FitConfig::default()
    };
    let model = fit_model(
        &role_rows,
        report.role,
        report.forward.variant,
        strategy,
        &config,
    )?;
    let calibration = CalibrationScale::fit(
        role_rows
            .iter()
            .map(|row| predict_probability(&model, row) - row.expected_win_rate),
    );
    let feature_weights = model
        .feature_names
        .iter()
        .cloned()
        .zip(model.core_weights.iter().copied())
        .collect();
    let training_cohort_offsets = model
        .cohort_offsets
        .iter()
        .map(|(&cohort, offsets)| {
            (
                cohort,
                model
                    .feature_names
                    .iter()
                    .cloned()
                    .zip(offsets.iter().copied())
                    .collect(),
            )
        })
        .collect();
    Some(HierarchicalModelArtifact {
        selected_strategy: Some(report.strategy.clone()),
        selected_variant: Some(variant_name(report.forward.variant).into()),
        feature_weights,
        training_cohort_offsets,
        training_cohort_samples: model.cohort_samples,
        final_hit_multiplier: Some(model.final_multiplier),
        lambda_core: Some(config.lambda_core),
        lambda_cohort: Some(config.lambda_cohort),
        calibration_p95_abs_effect: Some(calibration.p95_abs_effect),
    })
}

fn parse_strategy(value: &str) -> Option<HealingTakenStrategy> {
    match value {
        "none" => Some(HealingTakenStrategy::None),
        "directnegative" => Some(HealingTakenStrategy::DirectNegative),
        "heronormalized" => Some(HealingTakenStrategy::HeroNormalized),
        "excesshealing" => Some(HealingTakenStrategy::ExcessHealing),
        "resourceconversiondebt" => Some(HealingTakenStrategy::ResourceConversionDebt),
        _ => None,
    }
}

fn variant_name(value: ModelVariant) -> &'static str {
    match value {
        ModelVariant::Core => "core",
        ModelVariant::RoleFull => "role_full",
    }
}

fn shared_parameter_version(cohort: TrainingCohort) -> String {
    match cohort {
        TrainingCohort::RoleQueue => "role_queue-v2",
        TrainingCohort::OpenQueue => "open_queue-v2",
        TrainingCohort::StadiumCompetitive => "stadium_competitive-v2",
        TrainingCohort::OutcomeOnly => "outcome_only-v2",
    }
    .into()
}

fn role_model_ref(role: Role, model: &HierarchicalModelArtifact) -> String {
    let bytes = serde_json::to_vec(model).expect("权威职责模型只能包含可序列化聚合值");
    let role = role_name(role);
    let digest = format!("{:x}", Sha256::digest(bytes));
    format!("{role}-role-model-v2-{}", &digest[..16])
}

fn build_cell_reports(
    rows: &[TrainingRow],
    evaluation: &StabilityEvaluation,
    roles: &BTreeMap<Role, HierarchicalModelArtifact>,
) -> Result<Vec<CellReport>> {
    require_complete_cells(evaluation)?;
    let mut reports = evaluation
        .cells
        .iter()
        .map(|cell| cell_report(rows, cell, &roles[&cell.role], &evaluation.config))
        .collect::<Vec<_>>();
    reports.sort_by_key(|cell| (cell.role, cell.mode));
    Ok(reports)
}

fn cell_report(
    rows: &[TrainingRow],
    cell: &CellEvaluation,
    role_model: &HierarchicalModelArtifact,
    config: &StabilityConfigSnapshot,
) -> CellReport {
    let role = role_name(cell.role);
    let cohort_rows = rows.iter().filter(|row| {
        row.role == role
            && row.allow_performance
            && !row.outcome_only
            && row.mode_family.training_cohort() == cell.training_cohort
    });
    let training_rows = cohort_rows.count();
    let actual = rows
        .iter()
        .filter(|row| {
            row.role == role
                && row.mode_family == cell.mode
                && row.allow_performance
                && !row.outcome_only
        })
        .collect::<Vec<_>>();
    let seasons = actual
        .iter()
        .map(|row| row.season)
        .collect::<BTreeSet<_>>()
        .len();
    let heroes = actual
        .iter()
        .filter_map(|row| row.hero_group.as_deref())
        .collect::<BTreeSet<_>>()
        .len();
    let candidate = cell.nested_validation.metrics.as_ref();
    let baseline = cell.nested_validation.baseline_metrics.as_ref();
    let eligible = cell_validates_role_model(cell, role_model, config);
    let mut rejected_reasons = cell.rejected_reasons.clone();
    if cell_is_eligible(cell) && !eligible {
        rejected_reasons.push("shared_role_model_not_validated_for_cell".into());
    }
    if !eligible && rejected_reasons.is_empty() {
        rejected_reasons.push("no_eligible_strategy".into());
    }
    CellReport {
        role: cell.role,
        mode: cell.mode,
        training_cohort: cell.mode.training_cohort(),
        shared_parameter_version: shared_parameter_version(cell.mode.training_cohort()),
        role_model_ref: role_model_ref(cell.role, role_model),
        training_rows,
        validation_rows: actual.len(),
        seasons,
        heroes,
        decision: if eligible {
            CellDecision::Eligible
        } else {
            CellDecision::Shadow
        },
        eligible,
        rejected_reasons,
        metrics: CellMetrics {
            candidate_auc: candidate.map(|metrics| metrics.auc),
            candidate_brier: candidate.map(|metrics| metrics.brier),
            candidate_ece: candidate.map(|metrics| metrics.ece),
            baseline_auc: baseline.map(|metrics| metrics.auc),
            baseline_brier: baseline.map(|metrics| metrics.brier),
            baseline_ece: baseline.map(|metrics| metrics.ece),
            improved_folds: cell.nested_validation.improved_folds,
            nested_validation: cell.nested_validation.clone(),
            strategy_reports: cell.reports.clone(),
        },
    }
}

fn cell_is_eligible(cell: &CellEvaluation) -> bool {
    cell.decision == CellDecision::Eligible
        && cell.selected.is_some()
        && cell.rejected_reasons.is_empty()
}

fn require_complete_cells(evaluation: &StabilityEvaluation) -> Result<()> {
    if !evaluation.worker_errors.is_empty() {
        bail!("训练验收尚有未完成工作单元");
    }
    let expected = [Role::Tank, Role::Damage, Role::Support]
        .into_iter()
        .flat_map(|role| {
            super::performance_stability::VALIDATION_MODES
                .into_iter()
                .map(move |mode| (role, mode))
        })
        .collect::<BTreeSet<_>>();
    let actual = evaluation
        .cells
        .iter()
        .map(|cell| (cell.role, cell.mode))
        .collect::<BTreeSet<_>>();
    if expected != actual || evaluation.cells.len() != 15 {
        bail!("训练报告缺少职责模式验收格");
    }
    Ok(())
}

fn dataset_report(rows: &[TrainingRow], audit: &TrainingAudit) -> DatasetReport {
    let mut mode_rows = BTreeMap::new();
    for row in rows {
        *mode_rows.entry(row.mode_family).or_insert(0) += 1;
    }
    DatasetReport {
        account_datasets: audit.source_accounts,
        projected_rows: rows.len(),
        performance_rows: rows
            .iter()
            .filter(|row| row.allow_performance && !row.outcome_only)
            .count(),
        seasons: rows
            .iter()
            .map(|row| row.season)
            .collect::<BTreeSet<_>>()
            .len(),
        mode_rows,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BaselineKey {
    role: String,
    feature: String,
    environment: String,
    hero: Option<String>,
    mode: Option<ModeFamily>,
    season: Option<i64>,
}

fn build_normalization(rows: &[TrainingRow]) -> NormalizationArtifact {
    let mut grouped = BTreeMap::<BaselineKey, Vec<f64>>::new();
    for row in rows.iter().filter(|row| {
        row.allow_performance
            && !row.outcome_only
            && row.hero_group.is_some()
            && row
                .hero_duration_sec
                .is_some_and(|value| value.is_finite() && value > 0.0)
    }) {
        let duration = row.hero_duration_sec.unwrap_or_default();
        for (feature, value) in feature_values(row) {
            if feature == "healing_taken" && !row.usable_for_resource_debt {
                continue;
            }
            let Some(value) = value.filter(|value| value.is_finite()) else {
                continue;
            };
            let per_ten = value * 600.0 / duration;
            let hero = row.hero_group.clone();
            let environment = environment_name(row.environment).to_owned();
            for (hero, mode, season) in [
                (hero.clone(), Some(row.mode_family), Some(row.season)),
                (hero.clone(), Some(row.mode_family), None),
                (hero, None, None),
                (None, Some(row.mode_family), None),
                (None, None, None),
            ] {
                grouped
                    .entry(BaselineKey {
                        role: row.role.clone(),
                        feature: feature.into(),
                        environment: environment.clone(),
                        hero,
                        mode,
                        season,
                    })
                    .or_default()
                    .push(per_ten);
            }
        }
    }
    let baselines = grouped
        .into_iter()
        .filter_map(|(key, values)| {
            robust_baseline(&values).map(|(median, mad)| (key, values.len(), median, mad))
        })
        .map(
            |(key, sample_count, median_per_ten, mad_per_ten)| BaselineArtifact {
                role: key.role,
                feature: key.feature,
                environment: key.environment,
                hero: key.hero,
                mode: key.mode,
                season: key.season,
                sample_count,
                median_per_ten,
                mad_per_ten,
            },
        )
        .collect::<Vec<_>>();
    let healing_expectations = build_healing_expectations(rows, &baselines);
    NormalizationArtifact {
        hierarchy: vec![
            "hero_mode_season".into(),
            "hero_mode".into(),
            "hero_environment".into(),
            "role_mode".into(),
            "role_environment".into(),
            "unavailable".into(),
        ],
        minimum_hero_samples: 30,
        z_clip: 3.0,
        mad_scale: MAD_SCALE,
        mad_floor_absolute: MAD_FLOOR_ABSOLUTE,
        mad_floor_relative: MAD_FLOOR_RELATIVE,
        healing_min_regression_samples: 100,
        healing_huber_iterations: 8,
        healing_ridge_lambda: 0.1,
        baselines,
        healing_expectations,
    }
}

fn feature_values(row: &TrainingRow) -> [(&'static str, Option<f64>); 11] {
    [
        ("final_hit", row.final_hit),
        (
            "non_final_eliminations",
            row.kill
                .zip(row.final_hit)
                .map(|(kill, final_hit)| (kill - final_hit).max(0.0)),
        ),
        ("assist", row.assist),
        ("death", row.death),
        ("damage", row.damage),
        ("healing", row.healing),
        ("healing_taken", row.healing_taken),
        ("mitigation", row.mitigation),
        ("damage_taken", row.damage_taken),
        ("objective_time", row.objective_time_sec),
        ("available_team_healing", row.available_team_healing),
    ]
}

fn robust_baseline(values: &[f64]) -> Option<(f64, f64)> {
    if values.is_empty() {
        return None;
    }
    let median = median(values.to_vec());
    let observed_mad = median_value(values.iter().map(|value| (value - median).abs()).collect());
    let mad = observed_mad.max((median.abs() * MAD_FLOOR_RELATIVE).max(MAD_FLOOR_ABSOLUTE));
    Some((median, mad))
}

fn median(values: Vec<f64>) -> f64 {
    median_value(values)
}

fn median_value(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn build_healing_expectations(
    rows: &[TrainingRow],
    baselines: &[BaselineArtifact],
) -> Vec<HealingExpectationArtifact> {
    let mut groups = BTreeMap::<BaselineKey, Vec<([f64; 4], f64)>>::new();
    for row in rows.iter().filter(|row| row.usable_for_resource_debt) {
        let Some(duration) = row
            .hero_duration_sec
            .filter(|value| value.is_finite() && *value > 0.0)
        else {
            continue;
        };
        let Some(actual) = row.healing_taken.filter(|value| value.is_finite()) else {
            continue;
        };
        if row.hero_group.is_none() {
            continue;
        }
        let environment = environment_name(row.environment).to_owned();
        let Some(healing_baseline) = choose_artifact_baseline(baselines, row, "healing_taken")
        else {
            continue;
        };
        let hero = healing_baseline.hero.clone();
        let mode = healing_baseline.mode;
        let season = healing_baseline.season;
        let z = |feature: &str, raw: Option<f64>| {
            let baseline = choose_artifact_baseline(baselines, row, feature);
            raw.zip(baseline)
                .map(|(value, baseline)| {
                    (MAD_SCALE * (value * 600.0 / duration - baseline.median_per_ten)
                        / baseline.mad_per_ten)
                        .clamp(-3.0, 3.0)
                })
                .unwrap_or(0.0)
        };
        let inputs = [
            (row.duration_sec / 600.0).max(0.0),
            z("damage_taken", row.damage_taken),
            z("death", row.death),
            z("available_team_healing", row.available_team_healing),
        ];
        groups
            .entry(BaselineKey {
                role: row.role.clone(),
                feature: "healing_expectation".into(),
                environment,
                hero,
                mode,
                season,
            })
            .or_default()
            .push((inputs, actual * 600.0 / duration));
    }
    groups
        .into_iter()
        .map(|(key, samples)| {
            let fallback = median(samples.iter().map(|(_, target)| *target).collect());
            let coefficients = fit_huber(&samples, fallback);
            HealingExpectationArtifact {
                role: key.role,
                environment: key.environment,
                hero: key.hero,
                mode: key.mode,
                season: key.season,
                sample_count: samples.len(),
                fallback_median_per_ten: fallback,
                coefficients,
            }
        })
        .collect()
}

fn choose_artifact_baseline<'a>(
    baselines: &'a [BaselineArtifact],
    row: &TrainingRow,
    feature: &str,
) -> Option<&'a BaselineArtifact> {
    let environment = environment_name(row.environment);
    let hero = row.hero_group.as_ref();
    [
        (hero, Some(row.mode_family), Some(row.season), true),
        (hero, Some(row.mode_family), None, true),
        (hero, None, None, true),
        (None, Some(row.mode_family), None, false),
        (None, None, None, false),
    ]
    .into_iter()
    .find_map(|(hero, mode, season, requires_minimum)| {
        baselines.iter().find(|baseline| {
            baseline.role == row.role
                && baseline.feature == feature
                && baseline.environment == environment
                && baseline.hero.as_ref() == hero
                && baseline.mode == mode
                && baseline.season == season
                && (!requires_minimum || baseline.sample_count >= 30)
        })
    })
}

fn fit_huber(samples: &[([f64; 4], f64)], fallback: f64) -> Option<[f64; 5]> {
    if samples.len() < 100 {
        return None;
    }
    let mut beta = [fallback, 0.0, 0.0, 0.0, 0.0];
    for _ in 0..8 {
        let residuals = samples
            .iter()
            .map(|(x, y)| y - linear(beta, *x))
            .collect::<Vec<_>>();
        let scale = (median(residuals.iter().map(|r| r.abs()).collect()) / MAD_SCALE)
            .max(MAD_FLOOR_ABSOLUTE);
        let mut matrix = [[0.0; 5]; 5];
        let mut rhs = [0.0; 5];
        for ((x, y), residual) in samples.iter().zip(residuals) {
            let design = [1.0, x[0], x[1], x[2], x[3]];
            let scaled = residual.abs() / scale;
            let weight = if scaled <= 1.345 { 1.0 } else { 1.345 / scaled };
            for i in 0..5 {
                rhs[i] += weight * design[i] * y;
                for j in 0..5 {
                    matrix[i][j] += weight * design[i] * design[j];
                }
            }
        }
        for (index, row) in matrix.iter_mut().enumerate().skip(1) {
            row[index] += 0.1;
        }
        beta = solve(matrix, rhs)?;
    }
    Some(beta)
}

fn linear(beta: [f64; 5], x: [f64; 4]) -> f64 {
    beta[0] + beta[1] * x[0] + beta[2] * x[1] + beta[3] * x[2] + beta[4] * x[3]
}

fn solve(mut matrix: [[f64; 5]; 5], mut rhs: [f64; 5]) -> Option<[f64; 5]> {
    for column in 0..5 {
        let pivot = (column..5)
            .max_by(|&a, &b| matrix[a][column].abs().total_cmp(&matrix[b][column].abs()))?;
        if matrix[pivot][column].abs() <= 1.0e-12 {
            return None;
        }
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let divisor = matrix[column][column];
        for value in &mut matrix[column][column..] {
            *value /= divisor;
        }
        rhs[column] /= divisor;
        for row in 0..5 {
            if row == column {
                continue;
            }
            let factor = matrix[row][column];
            for index in column..5 {
                matrix[row][index] -= factor * matrix[column][index];
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    Some(rhs)
}

fn aggregate_sample_buckets(rows: &[TrainingRow]) -> Vec<AggregateSampleBucket> {
    let mut grouped = BTreeMap::new();
    for row in rows {
        *grouped
            .entry((row.role.clone(), row.mode_family, row.season))
            .or_insert(0) += 1;
    }
    grouped
        .into_iter()
        .map(|((role, mode, season), rows)| AggregateSampleBucket {
            role,
            mode,
            season,
            rows,
        })
        .collect()
}

fn training_config(evaluation: &StabilityEvaluation) -> TrainingConfigArtifact {
    let snapshot = &evaluation.config;
    let stage = match snapshot.stage {
        TrainingStage::Screen100 => "screen_100",
        TrainingStage::Candidate1000 => "candidate_1000",
        TrainingStage::Final10000 => "final_10000",
    };
    TrainingConfigArtifact {
        seed: snapshot.seed,
        mode_weights: BTreeMap::from([
            (ModeFamily::RoleCompetitive, 1.0),
            (ModeFamily::RoleQuick, 0.3),
            (ModeFamily::OpenCompetitive, 1.0),
            (ModeFamily::OpenQuick, 0.3),
            (ModeFamily::StadiumCompetitive, 0.5),
            (ModeFamily::StadiumQuick, 0.1),
        ]),
        fit: FitSearchArtifact {
            variants: snapshot.fit_variants.clone(),
            final_hit_multipliers: snapshot.final_hit_multipliers.clone(),
            regularization_candidates: snapshot
                .regularization_candidates
                .iter()
                .map(|candidate| RegularizationCandidate {
                    lambda_core: candidate[0],
                    lambda_cohort: candidate[1],
                })
                .collect(),
            learning_rate: snapshot.fit_learning_rate,
            steps: snapshot.fit_steps,
            minimum_cohort_offset_rows: snapshot.minimum_cohort_offset_rows,
        },
        validation: ValidationGateArtifact {
            stage: stage.into(),
            bootstrap_iterations: snapshot.bootstrap_iterations,
            minimum_improved_folds: snapshot.minimum_improved_folds,
            minimum_role_core_rows: snapshot.minimum_role_core_rows,
            minimum_cohort_rows: snapshot.minimum_cohort_rows,
            minimum_season_loo_groups: snapshot.minimum_season_loo_groups,
            minimum_hero_loo_groups: snapshot.minimum_hero_loo_groups,
            minimum_loo_row_coverage: snapshot.minimum_loo_row_coverage,
            maximum_loo_worsened_fraction: snapshot.max_loo_worsened_fraction,
            minimum_bootstrap_completion: snapshot.minimum_bootstrap_completion,
            minimum_critical_direction_stability: snapshot.minimum_critical_direction_stability,
            maximum_bootstrap_brier_ci_high: snapshot.maximum_bootstrap_brier_ci_high,
            max_ece_worsening: snapshot.max_ece_worsening,
            maximum_single_feature_share: snapshot.maximum_single_feature_share,
            maximum_average_adjustment_mmr: snapshot.max_abs_average_adjustment,
            maximum_cap_hit_rate: snapshot.maximum_cap_hit_rate,
            require_account_coverage: snapshot.require_account_coverage,
            require_critical_ci_excludes_zero: snapshot.require_critical_ci_excludes_zero,
            require_nonzero_reference_coefficients: snapshot.require_nonzero_reference_coefficients,
        },
    }
}

pub fn recompute_config_hash(artifact: &PerformanceModelArtifact) -> Result<String> {
    let mut value = serde_json::to_value(artifact)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("模型产物结构无效"))?;
    object.remove("config_hash");
    let bytes = serde_json::to_vec(&value)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn validate_report_boundary(value: &Value) -> Result<()> {
    fn visit(value: &Value) -> bool {
        match value {
            Value::Object(object) => object.iter().any(|(key, value)| {
                let normalized = key
                    .chars()
                    .filter(|ch| ch.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                const FORBIDDEN: [&str; 12] = [
                    "token",
                    "customertoken",
                    "authorization",
                    "cookie",
                    "credential",
                    "battletag",
                    "accountid",
                    "playerid",
                    "bnetid",
                    "matchid",
                    "rawjson",
                    "databasepath",
                ];
                FORBIDDEN.iter().any(|needle| normalized.contains(needle)) || visit(value)
            }),
            Value::Array(items) => items.iter().any(visit),
            _ => false,
        }
    }
    if visit(value) {
        bail!("训练产物包含禁止字段");
    }
    Ok(())
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    }
}

fn environment_name(environment: TrainingEnvironment) -> &'static str {
    match environment {
        TrainingEnvironment::Ordinary => "ordinary",
        TrainingEnvironment::Stadium => "stadium",
    }
}
