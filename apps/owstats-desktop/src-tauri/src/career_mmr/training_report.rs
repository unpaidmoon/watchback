use super::{
    input::{ModeFamily, Role, TrainingCohort},
    performance::{build_feature_matrix, HealingTakenStrategy},
    performance_forward::{CalibrationScale, ModelVariant, NestedCellReport},
    performance_model::{fit_model, predict_probability, FitConfig},
    performance_stability::{
        CellDecision, CellEvaluation, PromotedCandidate, StabilityConfig, StabilityConfigSnapshot,
        StabilityEvaluation, StabilityReport, TrainingStage,
    },
    training::{TrainingAudit, TrainingEnvironment, TrainingRow},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: u32 = 5;
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
    pub model_spec_id: Option<ModelSpecId>,
    pub selected_strategy: Option<String>,
    pub selected_variant: Option<String>,
    pub feature_weights: BTreeMap<String, f64>,
    pub training_cohort_offsets: BTreeMap<TrainingCohort, BTreeMap<String, f64>>,
    pub training_cohort_samples: BTreeMap<TrainingCohort, usize>,
    pub final_hit_multiplier: Option<f64>,
    pub lambda_core: Option<f64>,
    pub lambda_cohort: Option<f64>,
    pub calibration_p95_abs_probability_effect: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModelSpecId {
    pub strategy_index: usize,
    pub variant_index: usize,
    pub multiplier_index: usize,
    pub regularization_index: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellArtifact {
    pub role: Role,
    pub mode: ModeFamily,
    pub training_cohort: TrainingCohort,
    pub shared_parameter_version: String,
    pub role_model_ref: String,
    pub model_spec_id: Option<ModelSpecId>,
    pub performance_enabled: bool,
    pub calibration_p95_abs_logit_effect: f64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promoted_candidates: Option<Vec<PromotedCandidate>>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub promotable_candidates: Vec<PromotedCandidate>,
    pub integrity_kind: String,
    pub promotion_checksum: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<super::shadow::ShadowAggregate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetReport {
    pub account_datasets: usize,
    pub projected_rows: usize,
    pub performance_rows: usize,
    pub seasons: usize,
    pub mode_rows: BTreeMap<ModeFamily, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellReport {
    pub role: Role,
    pub mode: ModeFamily,
    pub training_cohort: TrainingCohort,
    pub shared_parameter_version: String,
    pub role_model_ref: String,
    pub model_spec_id: Option<ModelSpecId>,
    pub training_rows: usize,
    pub validation_rows: usize,
    pub seasons: usize,
    pub heroes: usize,
    pub decision: CellDecision,
    pub eligible: bool,
    pub rejected_reasons: Vec<String>,
    pub metrics: CellMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    validate_config_snapshot(&evaluation.config)?;
    require_complete_cells(evaluation)?;
    if seed != evaluation.config.seed {
        bail!("模型产物种子与训练评估配置不一致");
    }
    let normalization = build_normalization(rows);
    let roles = build_role_artifacts(rows, evaluation)?;
    let cells = build_cell_artifacts(rows, evaluation, &roles, &normalization);
    let report_cells = build_cell_reports(rows, evaluation, &roles)?;
    let promotable_candidates = build_promotable_candidates(&report_cells, &roles)?;
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
    let mut report = TrainingReport {
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
        promotable_candidates,
        integrity_kind: "checksum_not_authentication".into(),
        promotion_checksum: String::new(),
        shadow: None,
    };
    report.promotion_checksum = recompute_promotion_checksum(&serde_json::to_value(&report)?)?;
    validate_report_boundary(&serde_json::to_value(&artifact)?)?;
    validate_report_boundary(&serde_json::to_value(&report)?)?;
    Ok((artifact, report))
}

pub fn recompute_promotion_checksum(value: &Value) -> Result<String> {
    let mut canonical = value.clone();
    canonical
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("训练报告无效"))?
        .remove("promotion_checksum");
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonicalize_json(&canonical))?)
    ))
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        _ => value.clone(),
    }
}

/// Validates the complete promotion boundary used by the staged training CLI.
/// The caller is responsible for pinning and reading the source file safely.
pub fn validate_promotion_report(
    value: &Value,
    expected_stage: &str,
    expected_seed: u64,
) -> Result<Vec<PromotedCandidate>> {
    let report: TrainingReport = serde_json::from_value(value.clone())?;
    report.validate_for_promotion(value, expected_stage, expected_seed)?;
    let invalid = || anyhow::anyhow!("上一阶段报告无效");
    let config_hash = value
        .get("config_hash")
        .and_then(Value::as_str)
        .ok_or_else(&invalid)?;
    let promotion_checksum = value
        .get("promotion_checksum")
        .and_then(Value::as_str)
        .ok_or_else(&invalid)?;
    if value.get("schema_version").and_then(Value::as_u64) != Some(SCHEMA_VERSION.into())
        || value.get("seed").and_then(Value::as_u64) != Some(expected_seed)
        || value
            .pointer("/training_config/validation/stage")
            .and_then(Value::as_str)
            != Some(expected_stage)
        || !is_sha256(config_hash)
        || !is_sha256(promotion_checksum)
        || recompute_promotion_checksum(value)? != promotion_checksum
        || value.pointer("/training_config/fit/variants")
            != Some(&serde_json::json!(["core", "role_full"]))
        || value.pointer("/training_config/fit/final_hit_multipliers")
            != Some(&serde_json::json!([1.0, 1.25, 1.5, 1.75, 2.0]))
        || value.pointer("/training_config/fit/regularization_candidates")
            != Some(&serde_json::json!([
                {"lambda_core": 0.02, "lambda_cohort": 0.05},
                {"lambda_core": 0.05, "lambda_cohort": 0.10}
            ]))
    {
        bail!("上一阶段报告无效");
    }
    let cells = value
        .get("cells")
        .and_then(Value::as_array)
        .ok_or_else(&invalid)?;
    let mut by_role = BTreeMap::<Role, PromotedCandidate>::new();
    for cell in cells
        .iter()
        .filter(|cell| cell.get("eligible").and_then(Value::as_bool) == Some(true))
    {
        let role = match cell.get("role").and_then(Value::as_str) {
            Some("tank") => Role::Tank,
            Some("damage") => Role::Damage,
            Some("support") => Role::Support,
            _ => bail!("上一阶段报告无效"),
        };
        let reports = cell
            .pointer("/metrics/strategy_reports")
            .and_then(Value::as_array)
            .ok_or_else(&invalid)?;
        let spec = cell.get("model_spec_id").ok_or_else(&invalid)?;
        let strategy_index = promotion_index(spec, "strategy_index", 5)?;
        let variant_index = promotion_index(spec, "variant_index", 2)?;
        let multiplier_index = promotion_index(spec, "multiplier_index", 5)?;
        let regularization_index = promotion_index(spec, "regularization_index", 2)?;
        let strategy = [
            "none",
            "directnegative",
            "heronormalized",
            "excesshealing",
            "resourceconversiondebt",
        ][strategy_index];
        let variant = [ModelVariant::Core, ModelVariant::RoleFull][variant_index];
        let final_multiplier = [1.0, 1.25, 1.5, 1.75, 2.0][multiplier_index];
        let (lambda_core, lambda_cohort) = [(0.02, 0.05), (0.05, 0.10)][regularization_index];
        let report = reports
            .iter()
            .find(|report| report.get("strategy").and_then(Value::as_str) == Some(strategy))
            .filter(|report| report.get("eligible").and_then(Value::as_bool) == Some(true))
            .ok_or_else(&invalid)?;
        if report.pointer("/forward/variant").and_then(Value::as_str)
            != Some(match variant {
                ModelVariant::Core => "core",
                ModelVariant::RoleFull => "role_full",
            })
            || promotion_number(report, "/forward/final_multiplier")? != final_multiplier
            || promotion_number(report, "/forward/lambda_core")? != lambda_core
            || promotion_number(report, "/forward/lambda_cohort")? != lambda_cohort
        {
            bail!("上一阶段报告无效");
        }
        let candidate = PromotedCandidate {
            role,
            strategy: strategy.into(),
            variant,
            final_multiplier,
            lambda_core,
            lambda_cohort,
        };
        if by_role
            .get(&role)
            .is_some_and(|existing| existing != &candidate)
        {
            bail!("上一阶段报告无效");
        }
        by_role.insert(role, candidate);
    }
    let candidates = by_role.into_values().collect::<Vec<_>>();
    let declared: Vec<PromotedCandidate> = serde_json::from_value(
        value
            .get("promotable_candidates")
            .cloned()
            .ok_or_else(&invalid)?,
    )?;
    if declared != candidates {
        bail!("上一阶段报告无效");
    }
    Ok(candidates)
}

impl TrainingReport {
    pub fn validate_for_promotion(
        &self,
        raw: &Value,
        expected_stage: &str,
        expected_seed: u64,
    ) -> Result<()> {
        validate_report_boundary(raw)?;
        let stage = match expected_stage {
            "screen_100" => TrainingStage::Screen100,
            "candidate_1000" => TrainingStage::Candidate1000,
            "final_10000" => TrainingStage::Final10000,
            _ => bail!("上一阶段报告无效"),
        };
        let promoted = self.training_config.fit.promoted_candidates.clone();
        match stage {
            TrainingStage::Screen100 if promoted.is_some() => {
                bail!("上一阶段报告无效")
            }
            TrainingStage::Candidate1000 | TrainingStage::Final10000 => {
                let candidates = promoted
                    .as_ref()
                    .filter(|items| !items.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("上一阶段报告无效"))?;
                let mut roles = BTreeSet::new();
                if candidates.iter().any(|candidate| {
                    !roles.insert(candidate.role)
                        || model_spec_id_from_values(
                            &candidate.strategy,
                            candidate.variant,
                            candidate.final_multiplier,
                            candidate.lambda_core,
                            candidate.lambda_cohort,
                            &StabilityConfig::for_stage(stage).snapshot(),
                        )
                        .is_none()
                }) {
                    bail!("上一阶段报告无效");
                }
            }
            _ => {}
        }
        let mut expected_config = promoted.map_or_else(
            || StabilityConfig::for_stage(stage),
            |candidates| StabilityConfig::for_stage_with_promoted(stage, candidates),
        );
        expected_config.seed = expected_seed;
        let expected_training_config = training_config(&StabilityEvaluation {
            config: expected_config.snapshot(),
            reports: Vec::new(),
            roles: Vec::new(),
            cells: Vec::new(),
            worker_errors: Vec::new(),
        });
        if self.schema_version != SCHEMA_VERSION
            || self.algorithm != ALGORITHM
            || self.feature_version != FEATURE_VERSION
            || self.seed != expected_seed
            || self.training_config.seed != expected_seed
            || self.training_config.validation.stage != expected_stage
            || self.training_config != expected_training_config
            || !is_sha256(&self.config_hash)
            || self.integrity_kind != "checksum_not_authentication"
            || !is_sha256(&self.promotion_checksum)
            || recompute_promotion_checksum(raw)? != self.promotion_checksum
            || self.dataset.performance_rows > self.dataset.projected_rows
            || self.stadium_quick.mode != ModeFamily::StadiumQuick
            || self.stadium_quick.decision != CellDecision::OutcomeOnly
            || self.stadium_quick.performance_adjustment_mmr != 0
        {
            bail!("上一阶段报告无效");
        }
        if self
            .shadow
            .as_ref()
            .is_some_and(|shadow| shadow.validate(self.dataset.account_datasets).is_err())
        {
            bail!("上一阶段报告无效");
        }
        let expected = [Role::Tank, Role::Damage, Role::Support]
            .into_iter()
            .flat_map(|role| {
                super::performance_stability::VALIDATION_MODES
                    .into_iter()
                    .map(move |mode| (role, mode))
            })
            .collect::<BTreeSet<_>>();
        let actual = self
            .cells
            .iter()
            .map(|cell| (cell.role, cell.mode))
            .collect::<BTreeSet<_>>();
        if self.cells.len() != 15 || actual != expected {
            bail!("上一阶段报告无效");
        }
        for cell in &self.cells {
            let eligible = cell.decision == CellDecision::Eligible
                && cell.eligible
                && cell.rejected_reasons.is_empty()
                && cell.model_spec_id.is_some();
            let shadow = cell.decision == CellDecision::Shadow
                && !cell.eligible
                && !cell.rejected_reasons.is_empty();
            if (!eligible && !shadow)
                || cell.role_model_ref.is_empty()
                || cell.training_cohort != cell.mode.training_cohort()
                || cell.metrics.nested_validation.role != cell.role
                || cell.metrics.nested_validation.mode != cell.mode
            {
                bail!("上一阶段报告无效");
            }
        }
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn promotion_index(value: &Value, field: &str, length: usize) -> Result<usize> {
    let index = value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| anyhow::anyhow!("上一阶段报告无效"))?;
    if index >= length {
        bail!("上一阶段报告无效");
    }
    Ok(index)
}

fn promotion_number(value: &Value, pointer: &str) -> Result<f64> {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| anyhow::anyhow!("上一阶段报告无效"))
}

fn build_promotable_candidates(
    cells: &[CellReport],
    roles: &BTreeMap<Role, HierarchicalModelArtifact>,
) -> Result<Vec<PromotedCandidate>> {
    let mut candidates = Vec::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        if !cells.iter().any(|cell| cell.role == role && cell.eligible) {
            continue;
        }
        let model = &roles[&role];
        let candidate = PromotedCandidate {
            role,
            strategy: model
                .selected_strategy
                .clone()
                .ok_or_else(|| anyhow::anyhow!("可晋级职责缺少策略"))?,
            variant: match model.selected_variant.as_deref() {
                Some("core") => ModelVariant::Core,
                Some("role_full") => ModelVariant::RoleFull,
                _ => bail!("可晋级职责缺少模型变体"),
            },
            final_multiplier: model
                .final_hit_multiplier
                .ok_or_else(|| anyhow::anyhow!("可晋级职责缺少最后一击倍率"))?,
            lambda_core: model
                .lambda_core
                .ok_or_else(|| anyhow::anyhow!("可晋级职责缺少核心正则"))?,
            lambda_cohort: model
                .lambda_cohort
                .ok_or_else(|| anyhow::anyhow!("可晋级职责缺少队列正则"))?,
        };
        candidates.push(candidate);
    }
    candidates.sort_by(promoted_candidate_order);
    Ok(candidates)
}

fn promoted_candidate_order(
    left: &PromotedCandidate,
    right: &PromotedCandidate,
) -> std::cmp::Ordering {
    left.role
        .cmp(&right.role)
        .then(left.strategy.cmp(&right.strategy))
        .then(variant_rank(left.variant).cmp(&variant_rank(right.variant)))
        .then(left.final_multiplier.total_cmp(&right.final_multiplier))
        .then(left.lambda_core.total_cmp(&right.lambda_core))
        .then(left.lambda_cohort.total_cmp(&right.lambda_cohort))
}

fn build_cell_artifacts(
    rows: &[TrainingRow],
    evaluation: &StabilityEvaluation,
    roles: &BTreeMap<Role, HierarchicalModelArtifact>,
    normalization: &NormalizationArtifact,
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
                model_spec_id: role_model.model_spec_id.clone(),
                performance_enabled: cell_validates_role_model(
                    cell,
                    role_model,
                    &evaluation.config,
                ),
                calibration_p95_abs_logit_effect: cell_logit_p95(
                    rows,
                    cell.role,
                    cell.mode,
                    role_model,
                    normalization,
                ),
            }
        })
        .collect::<Vec<_>>();
    cells.sort_by_key(|cell| (cell.role, cell.mode));
    cells
}

fn cell_logit_p95(
    rows: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    model: &HierarchicalModelArtifact,
    normalization: &NormalizationArtifact,
) -> f64 {
    let mut effects = rows
        .iter()
        .filter(|row| row.role == role_name(role) && row.mode_family == mode)
        .filter_map(|row| super::shadow::artifact_logit_effect(row, role, model, normalization))
        .filter(|value| value.is_finite())
        .map(f64::abs)
        .collect::<Vec<_>>();
    effects.sort_by(f64::total_cmp);
    effects
        .get(((effects.len() as f64 * 0.95).ceil() as usize).saturating_sub(1))
        .copied()
        .unwrap_or(1.0)
        .max(f64::EPSILON)
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
        let artifact = if let Some(promoted) = promote_role_candidate(&reports, &evaluation.config)
        {
            refit_artifact(rows, promoted, &evaluation.config)
                .ok_or_else(|| anyhow::anyhow!("已通过验收的职责无法生成权威模型产物"))?
        } else {
            HierarchicalModelArtifact::default()
        };
        roles.insert(role, artifact);
    }
    Ok(roles)
}

fn promote_role_candidate<'a>(
    reports: &[&'a StabilityReport],
    config: &StabilityConfigSnapshot,
) -> Option<&'a StabilityReport> {
    if let Some(promoted) = promoted_spec_for_role(config, reports.first()?.role) {
        if reports
            .iter()
            .any(|report| model_spec_id(report, config) != Some(promoted.clone()))
        {
            return None;
        }
        return reports
            .iter()
            .copied()
            .min_by_key(|report| mode_rank(report.mode));
    }
    let mut candidates = reports
        .iter()
        .map(|report| {
            let support = reports
                .iter()
                .filter(|candidate| {
                    model_spec_id(report, config) == model_spec_id(candidate, config)
                })
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

fn mode_rank(mode: ModeFamily) -> usize {
    match mode {
        ModeFamily::RoleCompetitive => 0,
        ModeFamily::RoleQuick => 1,
        ModeFamily::OpenCompetitive => 2,
        ModeFamily::OpenQuick => 3,
        ModeFamily::StadiumCompetitive => 4,
        ModeFamily::StadiumQuick => 5,
    }
}

fn cell_validates_report(
    cell: &CellEvaluation,
    report: &StabilityReport,
    config: &StabilityConfigSnapshot,
) -> bool {
    let Some(spec_id) = model_spec_id(report, config) else {
        return false;
    };
    cell_is_eligible(cell)
        && cell.training_cohort == cell.mode.training_cohort()
        && cell.selected.as_deref() == Some(report.strategy.as_str())
        && report.eligible
        && report.decision == CellDecision::Eligible
        && report.rejected_reasons.is_empty()
        && report.role == cell.role
        && report.mode == cell.mode
        && report.training_cohort == cell.training_cohort
        && report.shared_parameter_version == cell.shared_parameter_version
        && report.forward.role == cell.role
        && report.forward.strategy == report.strategy
        && report.forward.rejected_reasons.is_empty()
        && finite_forward_evidence(report, config)
        && complete_strategy_identities(cell, config)
        && cell.nested_validation.role == cell.role
        && cell.nested_validation.mode == cell.mode
        && nested_degraded_state_is_valid(cell, config)
        && cell.nested_validation.selected_strategy == report.strategy
        && cell.nested_validation.selected_variant == report.forward.variant
        && model_spec_id_from_values(
            &cell.nested_validation.selected_strategy,
            cell.nested_validation.selected_variant,
            cell.nested_validation.selected_final_multiplier,
            cell.nested_validation.selected_lambda_core,
            cell.nested_validation.selected_lambda_cohort,
            config,
        ) == Some(spec_id.clone())
        && cell.nested_validation.improved_folds >= config.minimum_improved_folds
        && finite_nested_evidence(cell, config)
}

fn cell_validates_role_model(
    cell: &CellEvaluation,
    model: &HierarchicalModelArtifact,
    config: &StabilityConfigSnapshot,
) -> bool {
    let Some(strategy) = model.selected_strategy.as_deref() else {
        return false;
    };
    let promoted_matches = config.promoted_candidates.as_ref().is_none_or(|_| {
        promoted_spec_for_role(config, cell.role).as_ref() == model.model_spec_id.as_ref()
    });
    promoted_matches
        && cell.reports.iter().any(|report| {
            report.strategy == strategy
                && model.model_spec_id.as_ref() == model_spec_id(report, config).as_ref()
                && artifact_matches_report(model, report)
                && cell_validates_report(cell, report, config)
        })
}

fn artifact_matches_report(model: &HierarchicalModelArtifact, report: &StabilityReport) -> bool {
    model.model_spec_id.is_some()
        && model.selected_strategy.as_deref() == Some(report.strategy.as_str())
        && model.selected_variant.as_deref() == Some(variant_name(report.forward.variant))
        && model.final_hit_multiplier == Some(report.forward.final_multiplier)
        && model.lambda_core == Some(report.forward.lambda_core)
        && model.lambda_cohort == Some(report.forward.lambda_cohort)
}

fn metric_is_finite(metric: &super::performance_forward::MetricReport) -> bool {
    metric.auc.is_finite() && metric.brier.is_finite() && metric.ece.is_finite()
}

fn option_finite(value: Option<f64>) -> bool {
    value.is_some_and(f64::is_finite)
}

fn complete_strategy_identities(cell: &CellEvaluation, config: &StabilityConfigSnapshot) -> bool {
    const STRATEGIES: [&str; 5] = [
        "none",
        "directnegative",
        "heronormalized",
        "excesshealing",
        "resourceconversiondebt",
    ];
    if config.promoted_candidates.is_some() {
        let Some(promoted) = promoted_spec_for_role(config, cell.role) else {
            return cell.reports.is_empty();
        };
        return cell.reports.len() == 1
            && model_spec_id(&cell.reports[0], config) == Some(promoted)
            && cell.reports[0].role == cell.role
            && cell.reports[0].mode == cell.mode
            && cell.reports[0].training_cohort == cell.training_cohort
            && cell.reports[0].shared_parameter_version == cell.shared_parameter_version
            && cell.reports[0].forward.role == cell.role
            && cell.reports[0].forward.strategy == cell.reports[0].strategy;
    }
    cell.reports.len() == STRATEGIES.len()
        && STRATEGIES.iter().all(|strategy| {
            cell.reports
                .iter()
                .filter(|report| report.strategy == *strategy)
                .count()
                == 1
        })
        && cell.reports.iter().all(|report| {
            report.role == cell.role
                && report.mode == cell.mode
                && report.training_cohort == cell.training_cohort
                && report.shared_parameter_version == cell.shared_parameter_version
                && report.forward.role == cell.role
                && report.forward.strategy == report.strategy
        })
}

fn promoted_spec_for_role(config: &StabilityConfigSnapshot, role: Role) -> Option<ModelSpecId> {
    let candidates = config.promoted_candidates.as_ref()?;
    let mut candidates = candidates.iter().filter(|candidate| candidate.role == role);
    let candidate = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    model_spec_id_from_values(
        &candidate.strategy,
        candidate.variant,
        candidate.final_multiplier,
        candidate.lambda_core,
        candidate.lambda_cohort,
        config,
    )
}

fn finite_forward_evidence(report: &StabilityReport, config: &StabilityConfigSnapshot) -> bool {
    let Some(candidate) = report.forward.metrics.as_ref() else {
        return false;
    };
    let Some(baseline) = report.forward.baseline_metrics.as_ref() else {
        return false;
    };
    metric_is_finite(candidate)
        && metric_is_finite(baseline)
        && candidate.ece <= baseline.ece + config.max_ece_worsening
        && report.forward.average_adjustment.is_finite()
        && report.forward.average_adjustment.abs() <= config.max_abs_average_adjustment
        && report.forward.p95_single_feature_share.is_finite()
        && report.forward.p95_single_feature_share <= config.maximum_single_feature_share
        && report.forward.upper_cap_rate.is_finite()
        && report.forward.lower_cap_rate.is_finite()
        && report.forward.upper_cap_rate <= config.maximum_cap_hit_rate
        && report.forward.lower_cap_rate <= config.maximum_cap_hit_rate
        && loo_is_complete(
            &report.leave_one_season_out,
            config.minimum_season_loo_groups,
            config,
        )
        && loo_is_complete(
            &report.leave_one_hero_out,
            config.minimum_hero_loo_groups,
            config,
        )
        && (!config.require_account_coverage
            || (report.leave_one_account_out.candidate_groups > 0
                && report.leave_one_account_out.evaluated_groups > 0
                && report.leave_one_account_out.coverage_limited_groups == 0
                && option_finite(report.leave_one_account_out.mean_brier_delta)
                && option_finite(report.leave_one_account_out.worst_brier_delta)))
        && report.bootstrap.requested_iterations > 0
        && report.bootstrap.requested_iterations == config.bootstrap_iterations
        && report.bootstrap.completed_iterations as f64
            / report.bootstrap.requested_iterations as f64
            >= config.minimum_bootstrap_completion
        && option_finite(report.bootstrap.mean_brier_delta)
        && option_finite(report.bootstrap.brier_delta_ci95_low)
        && report.bootstrap.brier_delta_ci95_high.is_some_and(|value| {
            value.is_finite() && value <= config.maximum_bootstrap_brier_ci_high
        })
        && option_finite(report.bootstrap.mean_auc)
        && option_finite(report.bootstrap.mean_ece)
        && bootstrap_model_spec_id(report, config) == model_spec_id(report, config)
        && coefficients_are_complete(report, config)
}

fn loo_is_complete(
    loo: &super::performance_stability::LooReport,
    minimum_groups: usize,
    config: &StabilityConfigSnapshot,
) -> bool {
    loo.validation_groups >= minimum_groups
        && loo.candidate_rows > 0
        && loo.evaluated_rows as f64 / loo.candidate_rows as f64 >= config.minimum_loo_row_coverage
        && option_finite(loo.mean_brier_delta)
        && option_finite(loo.worst_brier_delta)
        && loo.worsened_groups as f64 / loo.validation_groups as f64
            <= config.max_loo_worsened_fraction
}

fn finite_nested_evidence(cell: &CellEvaluation, config: &StabilityConfigSnapshot) -> bool {
    let Some(candidate) = cell.nested_validation.metrics.as_ref() else {
        return false;
    };
    let Some(baseline) = cell.nested_validation.baseline_metrics.as_ref() else {
        return false;
    };
    metric_is_finite(candidate)
        && metric_is_finite(baseline)
        && candidate.ece <= baseline.ece + config.max_ece_worsening
        && !cell.nested_validation.folds.is_empty()
        && cell.nested_validation.folds.iter().all(|fold| {
            model_spec_id_from_values(
                &fold.strategy,
                fold.variant,
                fold.final_multiplier,
                fold.lambda_core,
                fold.lambda_cohort,
                config,
            )
            .is_some()
                && (!fold.selection_degraded
                    || is_fallback_spec(
                        &fold.strategy,
                        fold.variant,
                        fold.final_multiplier,
                        fold.lambda_core,
                        fold.lambda_cohort,
                        config,
                    ))
                && fold.metrics.as_ref().is_some_and(metric_is_finite)
                && option_finite(fold.baseline_brier)
                && option_finite(fold.brier_delta)
        })
}

fn nested_degraded_state_is_valid(cell: &CellEvaluation, config: &StabilityConfigSnapshot) -> bool {
    let any_fold_degraded = cell
        .nested_validation
        .folds
        .iter()
        .any(|fold| fold.selection_degraded);
    (!any_fold_degraded || cell.nested_validation.selection_degraded)
        && (!cell.nested_validation.selection_degraded
            || is_fallback_spec(
                &cell.nested_validation.selected_strategy,
                cell.nested_validation.selected_variant,
                cell.nested_validation.selected_final_multiplier,
                cell.nested_validation.selected_lambda_core,
                cell.nested_validation.selected_lambda_cohort,
                config,
            ))
}

fn is_fallback_spec(
    strategy: &str,
    variant: ModelVariant,
    multiplier: f64,
    lambda_core: f64,
    lambda_cohort: f64,
    config: &StabilityConfigSnapshot,
) -> bool {
    let Some(fallback_regularization) = config.regularization_candidates.first() else {
        return false;
    };
    strategy == "none"
        && variant == ModelVariant::Core
        && multiplier == 1.25
        && lambda_core == fallback_regularization[0]
        && lambda_cohort == fallback_regularization[1]
}

fn coefficients_are_complete(report: &StabilityReport, config: &StabilityConfigSnapshot) -> bool {
    let coefficients = &report.bootstrap.coefficients;
    let unique_weights = coefficients
        .iter()
        .map(|coefficient| &coefficient.weight)
        .collect::<BTreeSet<_>>();
    if unique_weights.len() != coefficients.len()
        || coefficients.iter().any(|coefficient| {
            !coefficient.direction_stability.is_finite()
                || !coefficient.ci95_low.is_finite()
                || !coefficient.ci95_high.is_finite()
        })
    {
        return false;
    }
    for (weight, positive) in [("final", true), ("nonfinal", true), ("death", false)] {
        if !one_valid_critical_coefficient(coefficients, weight, positive, config) {
            return false;
        }
    }
    report.strategy != "resourceconversiondebt"
        || one_valid_critical_coefficient(coefficients, "debt", false, config)
}

fn one_valid_critical_coefficient(
    coefficients: &[super::performance_stability::CoefficientStability],
    weight: &str,
    positive: bool,
    config: &StabilityConfigSnapshot,
) -> bool {
    let matching = coefficients
        .iter()
        .filter(|coefficient| coefficient.weight == weight)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return false;
    }
    let coefficient = matching[0];
    coefficient.scope == "core"
        && coefficient.critical_constraint
        && coefficient.direction_stability >= config.minimum_critical_direction_stability
        && coefficient.reference_nonzero
        && if positive {
            coefficient.ci95_low > 0.0
        } else {
            coefficient.ci95_high < 0.0
        }
}

fn bootstrap_model_spec_id(
    report: &StabilityReport,
    config: &StabilityConfigSnapshot,
) -> Option<ModelSpecId> {
    model_spec_id_from_values(
        &report.strategy,
        report.bootstrap.fixed_variant?,
        report.bootstrap.fixed_final_multiplier?,
        report.bootstrap.fixed_lambda_core?,
        report.bootstrap.fixed_lambda_cohort?,
        config,
    )
}

fn model_spec_id(
    report: &StabilityReport,
    config: &StabilityConfigSnapshot,
) -> Option<ModelSpecId> {
    model_spec_id_from_values(
        &report.strategy,
        report.forward.variant,
        report.forward.final_multiplier,
        report.forward.lambda_core,
        report.forward.lambda_cohort,
        config,
    )
}

fn unique_index<T: PartialEq>(values: &[T], target: &T) -> Option<usize> {
    let mut matches = values
        .iter()
        .enumerate()
        .filter(|(_, value)| *value == target);
    let index = matches.next()?.0;
    matches.next().is_none().then_some(index)
}

fn model_spec_id_from_values(
    strategy: &str,
    variant: ModelVariant,
    multiplier: f64,
    lambda_core: f64,
    lambda_cohort: f64,
    config: &StabilityConfigSnapshot,
) -> Option<ModelSpecId> {
    if !multiplier.is_finite() || !lambda_core.is_finite() || !lambda_cohort.is_finite() {
        return None;
    }
    let strategies = [
        "none",
        "directnegative",
        "heronormalized",
        "excesshealing",
        "resourceconversiondebt",
    ];
    let variant_name = variant_name(variant).to_string();
    Some(ModelSpecId {
        strategy_index: unique_index(&strategies, &strategy)?,
        variant_index: unique_index(&config.fit_variants, &variant_name)?,
        multiplier_index: unique_index(&config.final_hit_multipliers, &multiplier)?,
        regularization_index: unique_index(
            &config.regularization_candidates,
            &[lambda_core, lambda_cohort],
        )?,
    })
}

fn validate_config_snapshot(config: &StabilityConfigSnapshot) -> Result<()> {
    let valid_variants = ["core", "role_full"];
    let unique = |values: &[String]| values.iter().collect::<BTreeSet<_>>().len() == values.len();
    let invalid_promoted = config
        .promoted_candidates
        .as_ref()
        .is_some_and(|candidates| {
            candidates.is_empty()
                || candidates.iter().any(|candidate| {
                    strategy_rank(&candidate.strategy) == usize::MAX
                        || !config
                            .fit_variants
                            .contains(&variant_name(candidate.variant).to_owned())
                        || !config
                            .final_hit_multipliers
                            .contains(&candidate.final_multiplier)
                        || !config
                            .regularization_candidates
                            .contains(&[candidate.lambda_core, candidate.lambda_cohort])
                })
                || candidates.iter().enumerate().any(|(index, candidate)| {
                    candidates[..index]
                        .iter()
                        .any(|previous| previous == candidate)
                })
        });
    if invalid_promoted
        || config.fit_variants.is_empty()
        || !unique(&config.fit_variants)
        || config
            .fit_variants
            .iter()
            .any(|value| !valid_variants.contains(&value.as_str()))
        || config.final_hit_multipliers.is_empty()
        || config
            .final_hit_multipliers
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        || config
            .final_hit_multipliers
            .iter()
            .enumerate()
            .any(|(i, value)| config.final_hit_multipliers[..i].contains(value))
        || config.regularization_candidates.is_empty()
        || config
            .regularization_candidates
            .iter()
            .any(|pair| pair.iter().any(|value| !value.is_finite() || *value < 0.0))
        || config
            .regularization_candidates
            .iter()
            .enumerate()
            .any(|(i, pair)| config.regularization_candidates[..i].contains(pair))
        || !config.fit_learning_rate.is_finite()
        || config.fit_learning_rate <= 0.0
        || config.fit_steps == 0
        || config.minimum_cohort_offset_rows == 0
        || config.minimum_improved_folds == 0
        || config.minimum_role_core_rows == 0
        || config.minimum_cohort_rows == 0
        || config.minimum_season_loo_groups == 0
        || config.minimum_hero_loo_groups == 0
        || !unit_interval(config.minimum_loo_row_coverage)
        || !unit_interval(config.max_loo_worsened_fraction)
        || !unit_interval(config.minimum_bootstrap_completion)
        || !unit_interval(config.minimum_critical_direction_stability)
        || !config.maximum_bootstrap_brier_ci_high.is_finite()
        || !config.max_ece_worsening.is_finite()
        || config.max_ece_worsening < 0.0
        || !unit_interval(config.maximum_single_feature_share)
        || !config.max_abs_average_adjustment.is_finite()
        || config.max_abs_average_adjustment < 0.0
        || !unit_interval(config.maximum_cap_hit_rate)
    {
        bail!("训练配置快照无效，拒绝生成模型产物");
    }
    Ok(())
}

fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
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
    snapshot: &StabilityConfigSnapshot,
) -> Option<HierarchicalModelArtifact> {
    let spec_id = model_spec_id(report, snapshot)?;
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
        learning_rate: snapshot.fit_learning_rate,
        steps: snapshot.fit_steps,
        min_cohort_offset_rows: snapshot.minimum_cohort_offset_rows,
        final_multiplier: report.forward.final_multiplier,
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
        model_spec_id: Some(spec_id),
        selected_strategy: Some(report.strategy.clone()),
        selected_variant: Some(variant_name(report.forward.variant).into()),
        feature_weights,
        training_cohort_offsets,
        training_cohort_samples: model.cohort_samples,
        final_hit_multiplier: Some(model.final_multiplier),
        lambda_core: Some(config.lambda_core),
        lambda_cohort: Some(config.lambda_cohort),
        calibration_p95_abs_probability_effect: Some(calibration.p95_abs_effect),
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
        rejected_reasons.push("invalid_validation_evidence".into());
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
        model_spec_id: role_model.model_spec_id.clone(),
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
            promoted_candidates: snapshot.promoted_candidates.clone(),
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
    validate_artifact_numeric_boundary(artifact)?;
    let mut value = serde_json::to_value(artifact)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("模型产物结构无效"))?;
    object.remove("config_hash");
    let bytes = serde_json::to_vec(&value)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_artifact_numeric_boundary(artifact: &PerformanceModelArtifact) -> Result<()> {
    if artifact
        .normalization
        .healing_expectations
        .iter()
        .any(|expectation| {
            !expectation.fallback_median_per_ten.is_finite()
                || expectation
                    .coefficients
                    .is_some_and(|values| values.into_iter().any(|value| !value.is_finite()))
        })
    {
        bail!("模型产物包含无效治疗期望参数");
    }
    Ok(())
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
