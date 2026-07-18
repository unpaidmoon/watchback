use super::{
    input::{ModeFamily, Role, TrainingCohort},
    performance::{build_feature_matrix, HealingTakenStrategy},
    performance_forward::ModelVariant,
    performance_forward::NestedCellReport,
    performance_model::{fit_model, FitConfig},
    performance_stability::{CellDecision, CellEvaluation, StabilityEvaluation, StabilityReport},
    training::{TrainingAudit, TrainingEnvironment, TrainingRow},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: u32 = 1;
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
    pub selected_strategy: Option<String>,
    pub performance_enabled: bool,
    pub model: Option<HierarchicalModelArtifact>,
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
    pub relative_to_base_delta: f64,
    pub competitive_mmr: f64,
    pub support_competitive_mmr: f64,
    pub quick_mmr: f64,
    pub stadium_quick_mmr: f64,
}

impl Default for PerformanceCaps {
    fn default() -> Self {
        Self {
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
    let normalization = build_normalization(rows);
    let cells = build_cell_artifacts(rows, evaluation)?;
    let roles = build_role_artifacts(&cells);
    let caps = PerformanceCaps::default();
    let config_hash = configuration_hash(rows, seed, &caps, &roles, &cells)?;
    let artifact = PerformanceModelArtifact {
        schema_version: SCHEMA_VERSION,
        algorithm: ALGORITHM.into(),
        feature_version: FEATURE_VERSION.into(),
        seed,
        roles,
        cells,
        normalization,
        caps,
        config_hash: config_hash.clone(),
    };
    // The immutable artifact is returned in exactly the representation that is
    // persisted. Besides rejecting non-finite floats through deserialization,
    // this avoids adjacent-float differences after a JSON save/load cycle.
    let artifact = serde_json::from_str(&serde_json::to_string(&artifact)?)?;
    let report = TrainingReport {
        schema_version: SCHEMA_VERSION,
        algorithm: ALGORITHM.into(),
        feature_version: FEATURE_VERSION.into(),
        seed,
        dataset: dataset_report(rows, audit),
        cells: build_cell_reports(rows, evaluation)?,
        stadium_quick: OutcomeOnlyReport {
            mode: ModeFamily::StadiumQuick,
            rows: rows
                .iter()
                .filter(|row| row.mode_family == ModeFamily::StadiumQuick)
                .count(),
            decision: CellDecision::OutcomeOnly,
            performance_adjustment_mmr: 0,
        },
        config_hash,
    };
    validate_report_boundary(&serde_json::to_value(&artifact)?)?;
    validate_report_boundary(&serde_json::to_value(&report)?)?;
    Ok((artifact, report))
}

fn build_cell_artifacts(
    rows: &[TrainingRow],
    evaluation: &StabilityEvaluation,
) -> Result<Vec<CellArtifact>> {
    require_complete_cells(evaluation)?;
    evaluation
        .cells
        .iter()
        .map(|cell| {
            let eligible = cell_is_eligible(cell);
            let selected_report = cell.selected.as_deref().and_then(|strategy| {
                cell.reports
                    .iter()
                    .find(|report| report.strategy == strategy && report.eligible)
            });
            let model = selected_report.and_then(|report| refit_artifact(rows, report));
            if eligible && model.is_none() {
                bail!("已通过验收的职责模式无法生成模型产物");
            }
            Ok(CellArtifact {
                role: cell.role,
                mode: cell.mode,
                training_cohort: cell.training_cohort,
                shared_parameter_version: cell.shared_parameter_version.clone(),
                selected_strategy: cell.selected.clone(),
                performance_enabled: eligible,
                model,
            })
        })
        .collect()
}

fn build_role_artifacts(cells: &[CellArtifact]) -> BTreeMap<Role, HierarchicalModelArtifact> {
    [Role::Tank, Role::Damage, Role::Support]
        .into_iter()
        .map(|role| {
            let selected = cells
                .iter()
                .filter(|cell| cell.role == role && cell.performance_enabled)
                .find_map(|cell| cell.model.clone());
            let artifact = selected.unwrap_or_default();
            (role, artifact)
        })
        .collect()
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
        calibration_p95_abs_effect: Some(report.forward.calibration_scale.p95_abs_effect),
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

fn build_cell_reports(
    rows: &[TrainingRow],
    evaluation: &StabilityEvaluation,
) -> Result<Vec<CellReport>> {
    require_complete_cells(evaluation)?;
    Ok(evaluation
        .cells
        .iter()
        .map(|cell| cell_report(rows, cell))
        .collect())
}

fn cell_report(rows: &[TrainingRow], cell: &CellEvaluation) -> CellReport {
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
    let eligible = cell_is_eligible(cell);
    let mut rejected_reasons = cell.rejected_reasons.clone();
    if !eligible && rejected_reasons.is_empty() {
        rejected_reasons.push("no_eligible_strategy".into());
    }
    CellReport {
        role: cell.role,
        mode: cell.mode,
        training_cohort: cell.training_cohort,
        shared_parameter_version: cell.shared_parameter_version.clone(),
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

#[derive(Serialize)]
struct HashInput<'a> {
    algorithm: &'a str,
    feature_version: &'a str,
    seed: u64,
    mode_weights: BTreeMap<ModeFamily, f64>,
    caps: &'a PerformanceCaps,
    roles: &'a BTreeMap<Role, HierarchicalModelArtifact>,
    cells: &'a [CellArtifact],
    sample_buckets: Vec<SampleBucket>,
}

#[derive(Serialize)]
struct SampleBucket {
    role: String,
    mode: ModeFamily,
    season: i64,
    rows: usize,
}

fn configuration_hash(
    rows: &[TrainingRow],
    seed: u64,
    caps: &PerformanceCaps,
    roles: &BTreeMap<Role, HierarchicalModelArtifact>,
    cells: &[CellArtifact],
) -> Result<String> {
    let mode_weights = BTreeMap::from([
        (ModeFamily::RoleCompetitive, 1.0),
        (ModeFamily::RoleQuick, 0.3),
        (ModeFamily::OpenCompetitive, 1.0),
        (ModeFamily::OpenQuick, 0.3),
        (ModeFamily::StadiumCompetitive, 0.5),
        (ModeFamily::StadiumQuick, 0.1),
    ]);
    let mut grouped = BTreeMap::new();
    for row in rows {
        *grouped
            .entry((row.role.clone(), row.mode_family, row.season))
            .or_insert(0) += 1;
    }
    let sample_buckets = grouped
        .into_iter()
        .map(|((role, mode, season), rows)| SampleBucket {
            role,
            mode,
            season,
            rows,
        })
        .collect();
    let bytes = serde_json::to_vec(&HashInput {
        algorithm: ALGORITHM,
        feature_version: FEATURE_VERSION,
        seed,
        mode_weights,
        caps,
        roles,
        cells,
        sample_buckets,
    })?;
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
