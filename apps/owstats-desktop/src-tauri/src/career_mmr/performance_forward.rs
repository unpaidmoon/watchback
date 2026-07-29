use super::{
    input::ModeFamily,
    performance::{
        build_feature_matrix, eligible_performance_rows, HealingTakenStrategy as MatrixStrategy,
        MatrixRow,
    },
    performance_model::{fit_model, predict_probability, FitConfig, HierarchicalModel},
    training::TrainingRow,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub use super::performance::HealingTakenStrategy;
pub use super::rating::bounded_performance_adjustment;

const MULTIPLIERS: [f64; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];
const DIRECT_EXPOSURE_SCALE: f64 = 10_000.0;
const MAX_DIRECT_EXPOSURE: f64 = 3.0;

pub use super::input::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelVariant {
    Core,
    RoleFull,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForwardFold {
    pub training: Vec<usize>,
    pub validation: Vec<usize>,
}

/// Groups rows by each match's complete [min_ts,max_ts] span, then merges spans
/// that overlap or touch. A component is therefore indivisible by group or time.
pub fn build_forward_folds(rows: &[MatrixRow], fold_count: usize) -> Vec<ForwardFold> {
    if rows.is_empty() || fold_count == 0 {
        return Vec::new();
    }
    let mut groups: HashMap<&str, (i64, i64, Vec<usize>)> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        let entry =
            groups
                .entry(&row.match_group)
                .or_insert((row.begin_ts, row.begin_ts, Vec::new()));
        entry.0 = entry.0.min(row.begin_ts);
        entry.1 = entry.1.max(row.begin_ts);
        entry.2.push(index);
    }
    let mut spans: Vec<_> = groups.into_values().collect();
    spans.sort_by_key(|(start, end, _)| (*start, *end));
    let mut components: Vec<(i64, i64, Vec<usize>)> = Vec::new();
    for (start, end, indices) in spans {
        if let Some(last) = components.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                last.2.extend(indices);
                continue;
            }
        }
        components.push((start, end, indices));
    }
    if components.len() < 2 {
        return Vec::new();
    }
    let validation_components = components.len() - 1;
    let actual = fold_count.min(validation_components);
    (0..actual)
        .filter_map(|fold| {
            let start = 1 + fold * validation_components / actual;
            let end = 1 + (fold + 1) * validation_components / actual;
            let training: Vec<_> = components[..start]
                .iter()
                .flat_map(|c| c.2.iter().copied())
                .collect();
            let validation: Vec<_> = components[start..end]
                .iter()
                .flat_map(|c| c.2.iter().copied())
                .collect();
            (!validation.is_empty()).then_some(ForwardFold {
                training,
                validation,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoleVector {
    pub names: Vec<&'static str>,
    pub values: Vec<f64>,
    pub final_index: Option<usize>,
    pub nonfinal_index: Option<usize>,
    pub death_index: Option<usize>,
    pub debt_index: Option<usize>,
    pub conservative_scale: f64,
}

fn value(v: Option<f64>) -> f64 {
    v.filter(|x| x.is_finite()).unwrap_or(0.0)
}

pub fn role_vector(
    row: &MatrixRow,
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
) -> RoleVector {
    role_vector_with_multiplier(row, role, variant, strategy, 1.0)
}

fn damage_conversion(row: &MatrixRow, final_multiplier: f64) -> Option<f64> {
    let final_hit = row.per_ten.final_hit.filter(|value| value.is_finite())?;
    let non_final = row
        .per_ten
        .non_final_eliminations
        .filter(|value| value.is_finite())?;
    let damage = row.per_ten.damage.filter(|value| value.is_finite())?;
    let median = row
        .damage_baseline_median_per_ten
        .filter(|value| value.is_finite() && *value >= 0.0)?;
    let denominator = damage.max(0.25 * median).max(f64::EPSILON);
    let result = (final_hit * final_multiplier + non_final) / denominator;
    result.is_finite().then_some(result)
}

pub fn role_vector_with_multiplier(
    row: &MatrixRow,
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    final_multiplier: f64,
) -> RoleVector {
    let f = &row.standardized;
    let mut pairs = match role {
        Role::Tank => vec![
            ("final", value(f.final_hit)),
            ("nonfinal", value(f.non_final_eliminations)),
            ("assist", value(f.assist)),
            ("death", value(f.death)),
        ],
        Role::Damage => vec![
            ("final", value(f.final_hit)),
            ("nonfinal", value(f.non_final_eliminations)),
            ("assist", value(f.assist)),
            ("death", value(f.death)),
        ],
        Role::Support => vec![
            ("assist", value(f.assist)),
            ("final", value(f.final_hit)),
            ("nonfinal", value(f.non_final_eliminations)),
            ("death", value(f.death)),
        ],
    };
    if variant == ModelVariant::RoleFull {
        match role {
            Role::Tank => pairs.extend([
                ("frontline_conversion", value(f.frontline_conversion)),
                ("mitigation", value(f.mitigation)),
                ("abnormal_damage_taken", value(f.abnormal_damage_taken)),
            ]),
            Role::Damage => {
                let conversion = damage_conversion(row, final_multiplier);
                pairs.push(("damage_conversion", value(conversion)));
            }
            Role::Support => pairs.extend([
                ("healing", value(f.healing)),
                ("damage", value(f.damage)),
                ("survival", value(f.survival)),
                ("abnormal_damage_taken", value(f.abnormal_damage_taken)),
            ]),
        }
    }
    let mut debt_index = None;
    if strategy != MatrixStrategy::None {
        let exposure = value(row.healing_taken_derived);
        let debt = if strategy == MatrixStrategy::ResourceConversionDebt {
            exposure.max(0.0)
        } else if strategy == MatrixStrategy::DirectNegative {
            (exposure / DIRECT_EXPOSURE_SCALE).clamp(0.0, MAX_DIRECT_EXPOSURE)
        } else {
            exposure.max(0.0)
        };
        debt_index = Some(pairs.len());
        pairs.push(("debt", debt));
    }
    let names = pairs.iter().map(|p| p.0).collect::<Vec<_>>();
    let values = pairs.iter().map(|p| p.1).collect::<Vec<_>>();
    RoleVector {
        final_index: names.iter().position(|n| *n == "final"),
        nonfinal_index: names.iter().position(|n| *n == "nonfinal"),
        death_index: names.iter().position(|n| *n == "death"),
        debt_index,
        names,
        values,
        conservative_scale: if role == Role::Support { 0.67 } else { 1.0 },
    }
}

pub fn max_feature_contribution_share(
    row: &MatrixRow,
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    final_multiplier: f64,
    weights: &[f64],
) -> f64 {
    let vector = role_vector_with_multiplier(row, role, variant, strategy, final_multiplier);
    let parts = weights
        .iter()
        .zip(vector.values)
        .map(|(weight, value)| (weight * value).abs())
        .collect::<Vec<_>>();
    let total = parts.iter().sum::<f64>();
    parts.into_iter().fold(0.0, f64::max) / total.max(f64::EPSILON)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricReport {
    pub auc: f64,
    pub brier: f64,
    pub ece: f64,
}

pub fn metrics(actual: &[f64], predicted: &[f64]) -> Option<MetricReport> {
    if actual.is_empty()
        || actual.len() != predicted.len()
        || actual.iter().chain(predicted).any(|v| !v.is_finite())
    {
        return None;
    }
    let brier = actual
        .iter()
        .zip(predicted)
        .map(|(a, p)| (a - p).powi(2))
        .sum::<f64>()
        / actual.len() as f64;
    let mut order: Vec<usize> = (0..actual.len()).filter(|&i| actual[i] != 0.5).collect();
    order.sort_by(|&a, &b| predicted[a].total_cmp(&predicted[b]));
    let positives = order.iter().filter(|&&i| actual[i] > 0.5).count();
    let negatives = order.len() - positives;
    let auc = if positives == 0 || negatives == 0 {
        0.5
    } else {
        let mut rank_sum = 0.0;
        let mut i = 0;
        while i < order.len() {
            let mut j = i + 1;
            while j < order.len() && predicted[order[j]] == predicted[order[i]] {
                j += 1
            }
            let rank = (i + j + 1) as f64 / 2.0;
            rank_sum += order[i..j].iter().filter(|&&k| actual[k] > 0.5).count() as f64 * rank;
            i = j;
        }
        (rank_sum - (positives * (positives + 1) / 2) as f64) / (positives * negatives) as f64
    };
    let mut bins = [(0usize, 0.0, 0.0); 10];
    for (&a, &p) in actual.iter().zip(predicted) {
        let k = ((p.clamp(0.0, 1.0) * 10.0) as usize).min(9);
        bins[k].0 += 1;
        bins[k].1 += a;
        bins[k].2 += p;
    }
    let ece = bins
        .iter()
        .filter(|b| b.0 > 0)
        .map(|b| b.0 as f64 / actual.len() as f64 * ((b.1 - b.2) / b.0 as f64).abs())
        .sum();
    Some(MetricReport { auc, brier, ece })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoldReport {
    pub fold: usize,
    pub training_rows: usize,
    pub validation_rows: usize,
    pub metrics: Option<MetricReport>,
    pub baseline_brier: Option<f64>,
    pub brier_delta: Option<f64>,
    pub rejected_35_percent: bool,
    pub weights: BTreeMap<String, f64>,
    pub variant: ModelVariant,
    pub final_multiplier: f64,
    pub lambda_core: f64,
    pub lambda_cohort: f64,
    pub inner_validation_rows: usize,
    pub selection_degraded: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyReport {
    pub role: Role,
    pub strategy: String,
    pub preliminary: bool,
    pub performance_adjustment: f64,
    pub variant: ModelVariant,
    pub final_multiplier: f64,
    pub lambda_core: f64,
    pub lambda_cohort: f64,
    pub folds: Vec<FoldReport>,
    pub improved_folds: usize,
    pub metrics: Option<MetricReport>,
    pub baseline_metrics: Option<MetricReport>,
    pub full_refit_weights: BTreeMap<String, f64>,
    pub calibration_scale: CalibrationScale,
    pub average_adjustment: f64,
    pub p95_single_feature_share: f64,
    pub upper_cap_rate: f64,
    pub lower_cap_rate: f64,
    pub missing_values: usize,
    pub rejected_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CalibrationScale {
    pub p95_abs_effect: f64,
}

impl CalibrationScale {
    pub fn fit(effects: impl IntoIterator<Item = f64>) -> Self {
        let mut absolute = effects
            .into_iter()
            .filter(|effect| effect.is_finite())
            .map(f64::abs)
            .collect::<Vec<_>>();
        absolute.sort_by(f64::total_cmp);
        let p95_abs_effect = absolute
            .get(((absolute.len() as f64 * 0.95).ceil() as usize).saturating_sub(1))
            .copied()
            .unwrap_or(1.0)
            .max(f64::EPSILON);
        Self { p95_abs_effect }
    }

    pub fn apply(self, effect: f64) -> f64 {
        effect / self.p95_abs_effect
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundedFeatureAttribution {
    pub final_adjustment: f64,
    pub gross_absolute_exposure: f64,
    pub allocated_absolute_mmr: Vec<f64>,
    pub shares: Vec<f64>,
    pub max_share: f64,
}

fn formal_diagnostic_adjustment(
    base_delta: f64,
    requested: f64,
    role: Role,
    mode: ModeFamily,
) -> (f64, bool, bool) {
    let bounded = bounded_performance_adjustment(base_delta, requested, role, mode);
    (
        bounded,
        requested > 0.0 && bounded < requested,
        requested < 0.0 && bounded > requested,
    )
}

pub fn bounded_single_feature_share(
    base_delta: f64,
    normalized_contributions: &[f64],
    role: Role,
    mode: ModeFamily,
) -> f64 {
    bounded_feature_attribution(base_delta, normalized_contributions, role, mode).max_share
}

pub fn bounded_feature_attribution(
    base_delta: f64,
    normalized_contributions: &[f64],
    role: Role,
    mode: ModeFamily,
) -> BoundedFeatureAttribution {
    let requested_total = normalized_contributions.iter().sum::<f64>();
    let final_adjustment = bounded_performance_adjustment(base_delta, requested_total, role, mode);
    let gross_absolute_exposure = normalized_contributions
        .iter()
        .map(|v| v.abs())
        .sum::<f64>();
    let shares = if gross_absolute_exposure <= f64::EPSILON {
        vec![0.0; normalized_contributions.len()]
    } else {
        normalized_contributions
            .iter()
            .map(|value| value.abs() / gross_absolute_exposure)
            .collect::<Vec<_>>()
    };
    let allocated_absolute_mmr = shares
        .iter()
        .map(|share| share * final_adjustment.abs())
        .collect::<Vec<_>>();
    let max_share = shares.iter().copied().fold(0.0, f64::max);
    BoundedFeatureAttribution {
        final_adjustment,
        gross_absolute_exposure,
        allocated_absolute_mmr,
        shares,
        max_share,
    }
}

pub(crate) fn fit_hierarchical_model_regularized(
    rows: &[&MatrixRow],
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    multiplier: f64,
    lambda_core: f64,
    lambda_cohort: f64,
) -> Option<HierarchicalModel> {
    fit_model(
        rows,
        role,
        variant,
        strategy,
        &FitConfig {
            final_multiplier: multiplier,
            lambda_core,
            lambda_cohort,
            ..FitConfig::default()
        },
    )
}

fn model_contributions(
    model: &HierarchicalModel,
    row: &MatrixRow,
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    multiplier: f64,
) -> Vec<f64> {
    let effective = model.effective_weights(row.mode_family.training_cohort());
    let vector = role_vector_with_multiplier(row, role, variant, strategy, multiplier);
    effective
        .iter()
        .zip(vector.values)
        .map(|(weight, value)| weight * value)
        .collect()
}

fn fit_calibration_scale(
    model: &HierarchicalModel,
    rows: &[MatrixRow],
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    multiplier: f64,
) -> CalibrationScale {
    CalibrationScale::fit(rows.iter().map(|row| {
        model_contributions(model, row, role, variant, strategy, multiplier)
            .iter()
            .sum()
    }))
}

#[derive(Clone, Copy)]
pub(crate) struct CandidateSelection {
    pub(crate) variant: ModelVariant,
    pub(crate) multiplier: f64,
    pub(crate) lambda_core: f64,
    pub(crate) lambda_cohort: f64,
    pub(crate) degraded: bool,
}

impl CandidateSelection {
    fn core_degraded() -> Self {
        Self {
            variant: ModelVariant::Core,
            multiplier: 1.25,
            lambda_core: 0.02,
            lambda_cohort: 0.05,
            degraded: true,
        }
    }
}
fn select_matrix_candidate(
    rows: &[MatrixRow],
    role: Role,
    strategy: MatrixStrategy,
) -> CandidateSelection {
    let inner = build_forward_folds(rows, 3);
    if inner.len() < 2 {
        return CandidateSelection::core_degraded();
    }
    let mut selected = CandidateSelection {
        degraded: false,
        ..CandidateSelection::core_degraded()
    };
    let mut best = f64::INFINITY;
    for variant in [ModelVariant::Core, ModelVariant::RoleFull] {
        for multiplier in MULTIPLIERS {
            for (lambda_core, lambda_cohort) in [(0.02, 0.05), (0.05, 0.10)] {
                let mut loss = 0.0;
                let mut count = 0usize;
                let mut valid = true;
                for fold in &inner {
                    let train = fold.training.iter().map(|&i| &rows[i]).collect::<Vec<_>>();
                    let Some(model) = fit_hierarchical_model_regularized(
                        &train,
                        role,
                        variant,
                        strategy,
                        multiplier,
                        lambda_core,
                        lambda_cohort,
                    ) else {
                        valid = false;
                        break;
                    };
                    for &index in &fold.validation {
                        let error = rows[index].result - predict_probability(&model, &rows[index]);
                        if !error.is_finite() {
                            valid = false;
                            break;
                        }
                        loss += error.powi(2);
                        count += 1;
                    }
                }
                if valid && count > 0 && loss / (count as f64) < best {
                    best = loss / count as f64;
                    selected = CandidateSelection {
                        variant,
                        multiplier,
                        lambda_core,
                        lambda_cohort,
                        degraded: false,
                    };
                }
            }
        }
    }
    selected
}

pub fn evaluate_strategy(
    rows: &[MatrixRow],
    role: Role,
    strategy: MatrixStrategy,
) -> StrategyReport {
    let role_name = match role {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    };
    let rows = rows
        .iter()
        .filter(|row| {
            row.role == role_name
                && row.mode_family.training_cohort() != super::input::TrainingCohort::OutcomeOnly
        })
        .cloned()
        .collect::<Vec<_>>();
    let folds = build_forward_folds(&rows, 5);
    let mut reports = Vec::new();
    let mut all_a = Vec::new();
    let mut all_p = Vec::new();
    let mut all_base = Vec::new();
    let mut improved = 0;
    for (number, fold) in folds.iter().enumerate() {
        let train = fold.training.iter().map(|&i| &rows[i]).collect::<Vec<_>>();
        let train_rows = train.iter().map(|row| (*row).clone()).collect::<Vec<_>>();
        let fold_selected = select_matrix_candidate(&train_rows, role, strategy);
        let Some(model) = fit_hierarchical_model_regularized(
            &train,
            role,
            fold_selected.variant,
            strategy,
            fold_selected.multiplier,
            fold_selected.lambda_core,
            fold_selected.lambda_cohort,
        ) else {
            continue;
        };
        let a = fold
            .validation
            .iter()
            .map(|&i| rows[i].result)
            .collect::<Vec<_>>();
        let p = fold
            .validation
            .iter()
            .map(|&i| predict_probability(&model, &rows[i]))
            .collect::<Vec<_>>();
        let base = a
            .iter()
            .zip(fold.validation.iter())
            .map(|(a, &i)| (a - rows[i].expected_win_rate).powi(2))
            .sum::<f64>()
            / a.len().max(1) as f64;
        let m = metrics(&a, &p);
        let delta = m.as_ref().map(|x| x.brier - base);
        if delta.unwrap_or(0.0) < 0.0 {
            improved += 1
        }
        let mut rejected = fold
            .validation
            .iter()
            .map(|&i| {
                let effective = model.effective_weights(rows[i].mode_family.training_cohort());
                max_feature_contribution_share(
                    &rows[i],
                    role,
                    fold_selected.variant,
                    strategy,
                    fold_selected.multiplier,
                    &effective,
                ) > 0.35
            })
            .collect::<Vec<_>>();
        rejected.sort();
        let rejected = rejected
            .get(
                ((fold.validation.len() as f64 * 0.95).floor() as usize)
                    .min(fold.validation.len().saturating_sub(1)),
            )
            .copied()
            .unwrap_or(false);
        all_a.extend(&a);
        all_p.extend(&p);
        all_base.extend(fold.validation.iter().map(|&i| rows[i].expected_win_rate));
        let weights = role_vector_with_multiplier(
            &rows[fold.validation[0]],
            role,
            fold_selected.variant,
            strategy,
            fold_selected.multiplier,
        )
        .names
        .into_iter()
        .zip(model.core_weights.iter().copied())
        .map(|(n, v)| (n.into(), v))
        .collect();
        reports.push(FoldReport {
            fold: number + 1,
            training_rows: fold.training.len(),
            validation_rows: a.len(),
            metrics: m,
            baseline_brier: Some(base),
            brier_delta: delta,
            rejected_35_percent: rejected,
            weights,
            variant: fold_selected.variant,
            final_multiplier: fold_selected.multiplier,
            lambda_core: fold_selected.lambda_core,
            lambda_cohort: fold_selected.lambda_cohort,
            inner_validation_rows: build_forward_folds(&train_rows, 3)
                .iter()
                .map(|fold| fold.validation.len())
                .sum(),
            selection_degraded: fold_selected.degraded,
        });
    }
    let full = rows.iter().collect::<Vec<_>>();
    let full_selected = select_matrix_candidate(&rows, role, strategy);
    let selected = full_selected;
    let model = fit_hierarchical_model_regularized(
        &full,
        role,
        selected.variant,
        strategy,
        selected.multiplier,
        selected.lambda_core,
        selected.lambda_cohort,
    );
    let proto = rows
        .first()
        .map(|r| role_vector(r, role, selected.variant, strategy));
    let full_refit_weights: BTreeMap<String, f64> = proto
        .map(|p| {
            p.names
                .into_iter()
                .zip(
                    model
                        .as_ref()
                        .map(|model| model.core_weights.as_slice())
                        .unwrap_or_default()
                        .iter()
                        .copied(),
                )
                .map(|(n, v)| (n.into(), v))
                .collect()
        })
        .unwrap_or_default();
    let adjustments = rows
        .iter()
        .filter_map(|r| {
            model
                .as_ref()
                .map(|model| predict_probability(model, r) - r.expected_win_rate)
        })
        .collect::<Vec<_>>();
    let calibration_scale = model
        .as_ref()
        .map(|model| {
            fit_calibration_scale(
                model,
                &rows,
                role,
                selected.variant,
                strategy,
                selected.multiplier,
            )
        })
        .unwrap_or_else(|| CalibrationScale::fit([]));
    let mut rejected_reasons = Vec::new();
    if improved < 4 {
        rejected_reasons.push("improved_folds_below_4".into());
    }
    if reports.iter().any(|fold| fold.rejected_35_percent) {
        rejected_reasons.push("p95_single_feature_contribution_above_35_percent".into());
    }
    let final_ok = full_refit_weights
        .get("final")
        .zip(full_refit_weights.get("nonfinal"))
        .map(|(f, n)| *f > 0.0 && f > n)
        .unwrap_or(false);
    if !final_ok {
        rejected_reasons.push("final_weight_not_strictly_above_nonfinal".into());
    }
    let p95_rejected = reports.iter().any(|fold| fold.rejected_35_percent);
    StrategyReport {
        role,
        strategy: format!("{strategy:?}").to_lowercase(),
        preliminary: true,
        performance_adjustment: 0.0,
        variant: selected.variant,
        final_multiplier: selected.multiplier,
        lambda_core: selected.lambda_core,
        lambda_cohort: selected.lambda_cohort,
        folds: reports,
        improved_folds: improved,
        metrics: metrics(&all_a, &all_p),
        baseline_metrics: metrics(&all_a, &all_base),
        full_refit_weights,
        calibration_scale,
        average_adjustment: if adjustments.is_empty() {
            0.0
        } else {
            adjustments.iter().sum::<f64>() / adjustments.len() as f64
        },
        p95_single_feature_share: f64::from(p95_rejected),
        upper_cap_rate: 0.0,
        lower_cap_rate: 0.0,
        missing_values: rows
            .iter()
            .map(|r| missing_values(r, role, selected.variant, strategy))
            .sum(),
        rejected_reasons,
    }
}

pub fn missing_values(
    row: &MatrixRow,
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
) -> usize {
    let f = &row.standardized;
    let mut values = match role {
        Role::Tank => vec![f.final_hit, f.non_final_eliminations, f.assist, f.death],
        Role::Damage => vec![f.final_hit, f.non_final_eliminations, f.assist, f.death],
        Role::Support => vec![f.assist, f.final_hit, f.non_final_eliminations, f.death],
    };
    if variant == ModelVariant::RoleFull {
        match role {
            Role::Tank => values.extend([
                f.frontline_conversion,
                f.mitigation,
                f.abnormal_damage_taken,
            ]),
            Role::Damage => values.push(damage_conversion(row, 1.0)),
            Role::Support => {
                values.extend([f.healing, f.damage, f.survival, f.abnormal_damage_taken])
            }
        }
    }
    values.iter().filter(|v| v.is_none()).count()
        + usize::from(strategy != MatrixStrategy::None && row.healing_taken_derived.is_none())
}

pub fn evaluate_forward(source: &[TrainingRow]) -> Vec<StrategyReport> {
    let source = eligible_performance_rows(source);
    let source = &source;
    let strategies = [
        MatrixStrategy::None,
        MatrixStrategy::DirectNegative,
        MatrixStrategy::HeroNormalized,
        MatrixStrategy::ExcessHealing,
        MatrixStrategy::ResourceConversionDebt,
    ];
    [Role::Tank, Role::Damage, Role::Support]
        .into_iter()
        .flat_map(|role| {
            strategies.into_iter().map(move |strategy| {
                let role_name = match role {
                    Role::Tank => "tank",
                    Role::Damage => "damage",
                    Role::Support => "support",
                };
                let role_rows = source
                    .iter()
                    .filter(|r| r.role == role_name)
                    .cloned()
                    .collect::<Vec<_>>();
                evaluate_raw_strategy(&role_rows, role, strategy, None, None)
            })
        })
        .collect()
}

fn raw_folds(rows: &[TrainingRow], requested: usize) -> Vec<ForwardFold> {
    let mut groups: HashMap<&str, (i64, i64, Vec<usize>)> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        let e = groups
            .entry(&row.match_group)
            .or_insert((row.begin_ts, row.begin_ts, Vec::new()));
        e.0 = e.0.min(row.begin_ts);
        e.1 = e.1.max(row.begin_ts);
        e.2.push(index);
    }
    let mut spans = groups.into_values().collect::<Vec<_>>();
    spans.sort_by_key(|s| (s.0, s.1));
    let mut components: Vec<(i64, i64, Vec<usize>)> = Vec::new();
    for (start, end, indices) in spans {
        if let Some(last) = components.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                last.2.extend(indices);
                continue;
            }
        }
        components.push((start, end, indices));
    }
    if components.len() < 2 {
        return Vec::new();
    }
    let available = components.len() - 1;
    let count = requested.min(available);
    (0..count)
        .map(|fold| {
            let start = 1 + fold * available / count;
            let end = 1 + (fold + 1) * available / count;
            ForwardFold {
                training: components[..start]
                    .iter()
                    .flat_map(|c| c.2.iter().copied())
                    .collect(),
                validation: components[start..end]
                    .iter()
                    .flat_map(|c| c.2.iter().copied())
                    .collect(),
            }
        })
        .collect()
}

pub fn evaluate_cell_strategy(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    strategy: MatrixStrategy,
) -> StrategyReport {
    let role_name = match role {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    };
    let rows = eligible_performance_rows(source)
        .into_iter()
        .filter(|row| row.role == role_name)
        .collect::<Vec<_>>();
    evaluate_raw_strategy(&rows, role, strategy, Some(mode), None)
}

pub(crate) fn evaluate_cell_strategy_fixed(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    strategy: MatrixStrategy,
    selected: CandidateSelection,
) -> StrategyReport {
    let role_name = match role {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    };
    let rows = eligible_performance_rows(source)
        .into_iter()
        .filter(|row| row.role == role_name)
        .collect::<Vec<_>>();
    evaluate_raw_strategy(&rows, role, strategy, Some(mode), Some(selected))
}

pub fn fit_formal_validation_adjustments(
    training: &[TrainingRow],
    validation: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    strategy: MatrixStrategy,
) -> Vec<f64> {
    let role_name = match role {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    };
    let selected = select_candidate_detailed(training, role, strategy);
    let matrix = build_feature_matrix(training, validation, strategy);
    let train = matrix
        .training_by_role
        .get(role_name)
        .cloned()
        .unwrap_or_default();
    let valid = matrix
        .validation_by_role
        .get(role_name)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|row| row.mode_family == mode)
        .collect::<Vec<_>>();
    let refs = train.iter().collect::<Vec<_>>();
    let Some(model) = fit_hierarchical_model_regularized(
        &refs,
        role,
        selected.variant,
        strategy,
        selected.multiplier,
        selected.lambda_core,
        selected.lambda_cohort,
    ) else {
        return Vec::new();
    };
    let calibration = fit_calibration_scale(
        &model,
        &train,
        role,
        selected.variant,
        strategy,
        selected.multiplier,
    );
    let raw = validation
        .iter()
        .map(|row| (row.match_group.as_str(), row))
        .collect::<HashMap<_, _>>();
    let effects = valid
        .iter()
        .filter_map(|row| {
            let base_delta = raw.get(row.match_group.as_str())?.base_delta?;
            let effect = model_contributions(
                &model,
                row,
                role,
                selected.variant,
                strategy,
                selected.multiplier,
            )
            .iter()
            .sum::<f64>();
            Some((base_delta, calibration.apply(effect), row.mode_family))
        })
        .collect::<Vec<_>>();
    effects
        .into_iter()
        .map(|(base_delta, effect, mode)| {
            bounded_performance_adjustment(base_delta, effect, role, mode)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestedFoldReport {
    pub fold: usize,
    pub training_rows: usize,
    pub validation_rows: usize,
    pub strategy: String,
    pub variant: ModelVariant,
    pub final_multiplier: f64,
    pub lambda_core: f64,
    pub lambda_cohort: f64,
    pub inner_validation_rows: usize,
    pub selection_degraded: bool,
    pub metrics: Option<MetricReport>,
    pub baseline_brier: Option<f64>,
    pub brier_delta: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestedCellReport {
    pub role: Role,
    pub mode: ModeFamily,
    pub folds: Vec<NestedFoldReport>,
    pub improved_folds: usize,
    pub metrics: Option<MetricReport>,
    pub baseline_metrics: Option<MetricReport>,
    pub selection_degraded: bool,
    pub selected_strategy: String,
    pub selected_variant: ModelVariant,
    pub selected_final_multiplier: f64,
    pub selected_lambda_core: f64,
    pub selected_lambda_cohort: f64,
}

#[derive(Clone, Copy)]
struct CompleteCandidate {
    strategy: MatrixStrategy,
    hyperparameters: CandidateSelection,
    inner_validation_rows: usize,
}

fn select_complete_candidate(
    rows: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    force_degraded: bool,
) -> CompleteCandidate {
    let inner = raw_folds(rows, 3);
    if inner.len() < 2 || force_degraded {
        return CompleteCandidate {
            strategy: MatrixStrategy::None,
            hyperparameters: CandidateSelection::core_degraded(),
            inner_validation_rows: inner.iter().map(|fold| fold.validation.len()).sum(),
        };
    }
    let mut best = f64::INFINITY;
    let mut selected = CompleteCandidate {
        strategy: MatrixStrategy::None,
        hyperparameters: CandidateSelection::core_degraded(),
        inner_validation_rows: inner.iter().map(|fold| fold.validation.len()).sum(),
    };
    for strategy in [
        MatrixStrategy::None,
        MatrixStrategy::DirectNegative,
        MatrixStrategy::HeroNormalized,
        MatrixStrategy::ExcessHealing,
        MatrixStrategy::ResourceConversionDebt,
    ] {
        for variant in [ModelVariant::Core, ModelVariant::RoleFull] {
            for multiplier in MULTIPLIERS {
                for (lambda_core, lambda_cohort) in [(0.02, 0.05), (0.05, 0.10)] {
                    let mut loss = 0.0;
                    let mut count = 0usize;
                    let mut valid_candidate = true;
                    for fold in &inner {
                        let train = fold
                            .training
                            .iter()
                            .map(|&index| rows[index].clone())
                            .collect::<Vec<_>>();
                        let valid = fold
                            .validation
                            .iter()
                            .map(|&index| rows[index].clone())
                            .filter(|row| row.mode_family == mode)
                            .collect::<Vec<_>>();
                        if valid.is_empty() {
                            valid_candidate = false;
                            break;
                        }
                        let matrix = build_feature_matrix(&train, &valid, strategy);
                        let role_name = match role {
                            Role::Tank => "tank",
                            Role::Damage => "damage",
                            Role::Support => "support",
                        };
                        let training = matrix
                            .training_by_role
                            .get(role_name)
                            .cloned()
                            .unwrap_or_default();
                        let validation = matrix
                            .validation_by_role
                            .get(role_name)
                            .cloned()
                            .unwrap_or_default();
                        let refs = training.iter().collect::<Vec<_>>();
                        let Some(model) = fit_hierarchical_model_regularized(
                            &refs,
                            role,
                            variant,
                            strategy,
                            multiplier,
                            lambda_core,
                            lambda_cohort,
                        ) else {
                            valid_candidate = false;
                            break;
                        };
                        for row in &validation {
                            let error = row.result - predict_probability(&model, row);
                            if !error.is_finite() {
                                valid_candidate = false;
                                break;
                            }
                            loss += error.powi(2);
                            count += 1;
                        }
                    }
                    if valid_candidate && count > 0 && loss / (count as f64) < best {
                        best = loss / count as f64;
                        selected = CompleteCandidate {
                            strategy,
                            hyperparameters: CandidateSelection {
                                variant,
                                multiplier,
                                lambda_core,
                                lambda_cohort,
                                degraded: false,
                            },
                            inner_validation_rows: inner
                                .iter()
                                .map(|fold| fold.validation.len())
                                .sum(),
                        };
                    }
                }
            }
        }
    }
    selected
}

pub fn evaluate_nested_cell(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
) -> NestedCellReport {
    evaluate_nested_cell_impl(source, role, mode, false, None)
}

pub(crate) fn evaluate_nested_cell_shadow(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
) -> NestedCellReport {
    evaluate_nested_cell_impl(source, role, mode, true, None)
}

pub(crate) fn evaluate_nested_cell_fixed(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    strategy: MatrixStrategy,
    hyperparameters: CandidateSelection,
) -> NestedCellReport {
    evaluate_nested_cell_impl(
        source,
        role,
        mode,
        false,
        Some(CompleteCandidate {
            strategy,
            hyperparameters,
            inner_validation_rows: 0,
        }),
    )
}

fn evaluate_nested_cell_impl(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    force_degraded: bool,
    fixed: Option<CompleteCandidate>,
) -> NestedCellReport {
    let role_name = match role {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    };
    let rows = eligible_performance_rows(source)
        .into_iter()
        .filter(|row| row.role == role_name)
        .collect::<Vec<_>>();
    let outer = raw_folds(&rows, 5);
    let mut folds = Vec::new();
    let mut actual = Vec::new();
    let mut predicted = Vec::new();
    let mut baseline = Vec::new();
    for (number, fold) in outer.iter().enumerate() {
        let train = fold
            .training
            .iter()
            .map(|&index| rows[index].clone())
            .collect::<Vec<_>>();
        let valid = fold
            .validation
            .iter()
            .map(|&index| rows[index].clone())
            .filter(|row| row.mode_family == mode)
            .collect::<Vec<_>>();
        if valid.is_empty() {
            continue;
        }
        let mut selected =
            fixed.unwrap_or_else(|| select_complete_candidate(&train, role, mode, force_degraded));
        selected.inner_validation_rows = raw_folds(&train, 3)
            .iter()
            .map(|fold| fold.validation.len())
            .sum();
        let matrix = build_feature_matrix(&train, &valid, selected.strategy);
        let training = matrix
            .training_by_role
            .get(role_name)
            .cloned()
            .unwrap_or_default();
        let validation = matrix
            .validation_by_role
            .get(role_name)
            .cloned()
            .unwrap_or_default();
        let refs = training.iter().collect::<Vec<_>>();
        let Some(model) = fit_hierarchical_model_regularized(
            &refs,
            role,
            selected.hyperparameters.variant,
            selected.strategy,
            selected.hyperparameters.multiplier,
            selected.hyperparameters.lambda_core,
            selected.hyperparameters.lambda_cohort,
        ) else {
            continue;
        };
        let fold_actual = validation.iter().map(|row| row.result).collect::<Vec<_>>();
        let fold_predicted = validation
            .iter()
            .map(|row| predict_probability(&model, row))
            .collect::<Vec<_>>();
        let fold_baseline = validation
            .iter()
            .map(|row| row.expected_win_rate)
            .collect::<Vec<_>>();
        let candidate_metrics = metrics(&fold_actual, &fold_predicted);
        let baseline_metrics = metrics(&fold_actual, &fold_baseline);
        let baseline_brier = baseline_metrics.as_ref().map(|report| report.brier);
        let brier_delta = candidate_metrics
            .as_ref()
            .zip(baseline_brier)
            .map(|(candidate, baseline)| candidate.brier - baseline);
        actual.extend(&fold_actual);
        predicted.extend(&fold_predicted);
        baseline.extend(&fold_baseline);
        folds.push(NestedFoldReport {
            fold: number + 1,
            training_rows: training.len(),
            validation_rows: validation.len(),
            strategy: format!("{:?}", selected.strategy).to_lowercase(),
            variant: selected.hyperparameters.variant,
            final_multiplier: selected.hyperparameters.multiplier,
            lambda_core: selected.hyperparameters.lambda_core,
            lambda_cohort: selected.hyperparameters.lambda_cohort,
            inner_validation_rows: selected.inner_validation_rows,
            selection_degraded: selected.hyperparameters.degraded,
            metrics: candidate_metrics,
            baseline_brier,
            brier_delta,
        });
    }
    let final_selection =
        fixed.unwrap_or_else(|| select_complete_candidate(&rows, role, mode, force_degraded));
    NestedCellReport {
        role,
        mode,
        improved_folds: folds
            .iter()
            .filter(|fold| fold.brier_delta.is_some_and(|delta| delta < 0.0))
            .count(),
        selection_degraded: folds.iter().any(|fold| fold.selection_degraded)
            || final_selection.hyperparameters.degraded,
        selected_strategy: format!("{:?}", final_selection.strategy).to_lowercase(),
        selected_variant: final_selection.hyperparameters.variant,
        selected_final_multiplier: final_selection.hyperparameters.multiplier,
        selected_lambda_core: final_selection.hyperparameters.lambda_core,
        selected_lambda_cohort: final_selection.hyperparameters.lambda_cohort,
        metrics: metrics(&actual, &predicted),
        baseline_metrics: metrics(&actual, &baseline),
        folds,
    }
}

fn evaluate_raw_strategy(
    source: &[TrainingRow],
    role: Role,
    strategy: MatrixStrategy,
    validation_mode: Option<ModeFamily>,
    fixed: Option<CandidateSelection>,
) -> StrategyReport {
    let role_name = match role {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    };
    let folds = raw_folds(source, 5);
    let mut fold_reports = Vec::new();
    let mut all_a = Vec::new();
    let mut all_p = Vec::new();
    let mut all_base = Vec::new();
    let mut missing = 0;
    let mut diagnostic_effects: Vec<(f64, f64, ModeFamily, Vec<f64>)> = Vec::new();
    for (number, fold) in folds.iter().enumerate() {
        let train = fold
            .training
            .iter()
            .map(|&i| source[i].clone())
            .collect::<Vec<_>>();
        let validation = fold
            .validation
            .iter()
            .map(|&i| source[i].clone())
            .filter(|row| validation_mode.is_none_or(|mode| row.mode_family == mode))
            .collect::<Vec<_>>();
        if validation.is_empty() {
            continue;
        }
        let matrix = build_feature_matrix(&train, &validation, strategy);
        let train_rows = matrix
            .training_by_role
            .get(role_name)
            .cloned()
            .unwrap_or_default();
        let valid_rows = matrix
            .validation_by_role
            .get(role_name)
            .cloned()
            .unwrap_or_default();
        if train_rows.is_empty() || valid_rows.is_empty() {
            continue;
        }
        let selected = fixed.unwrap_or_else(|| select_candidate_detailed(&train, role, strategy));
        let refs = train_rows.iter().collect::<Vec<_>>();
        let Some(model) = fit_hierarchical_model_regularized(
            &refs,
            role,
            selected.variant,
            strategy,
            selected.multiplier,
            selected.lambda_core,
            selected.lambda_cohort,
        ) else {
            continue;
        };
        let calibration = fit_calibration_scale(
            &model,
            &train_rows,
            role,
            selected.variant,
            strategy,
            selected.multiplier,
        );
        let a = valid_rows.iter().map(|r| r.result).collect::<Vec<_>>();
        let p = valid_rows
            .iter()
            .map(|r| predict_probability(&model, r))
            .collect::<Vec<_>>();
        let raw_validation = validation
            .iter()
            .map(|row| (row.match_group.as_str(), row))
            .collect::<HashMap<_, _>>();
        diagnostic_effects.extend(valid_rows.iter().filter_map(|row| {
            let raw = raw_validation.get(row.match_group.as_str())?;
            let base_delta = raw.base_delta?;
            let contributions = model_contributions(
                &model,
                row,
                role,
                selected.variant,
                strategy,
                selected.multiplier,
            )
            .into_iter()
            .map(|contribution| calibration.apply(contribution))
            .collect::<Vec<_>>();
            let effect = contributions.iter().sum::<f64>();
            Some((base_delta, effect, row.mode_family, contributions))
        }));
        let base = valid_rows
            .iter()
            .map(|r| (r.result - r.expected_win_rate).powi(2))
            .sum::<f64>()
            / valid_rows.len() as f64;
        let metric = metrics(&a, &p);
        let delta = metric.as_ref().map(|m| m.brier - base);
        let names = role_vector(&valid_rows[0], role, selected.variant, strategy).names;
        let weights = names
            .into_iter()
            .zip(model.core_weights.iter().copied())
            .map(|(n, v)| (n.into(), v))
            .collect();
        missing += valid_rows
            .iter()
            .map(|r| missing_values(r, role, selected.variant, strategy))
            .sum::<usize>();
        all_a.extend(&a);
        all_p.extend(&p);
        all_base.extend(valid_rows.iter().map(|row| row.expected_win_rate));
        fold_reports.push(FoldReport {
            fold: number + 1,
            training_rows: train_rows.len(),
            validation_rows: valid_rows.len(),
            metrics: metric,
            baseline_brier: Some(base),
            brier_delta: delta,
            rejected_35_percent: false,
            weights,
            variant: selected.variant,
            final_multiplier: selected.multiplier,
            lambda_core: selected.lambda_core,
            lambda_cohort: selected.lambda_cohort,
            inner_validation_rows: raw_folds(&train, 3)
                .iter()
                .map(|f| f.validation.len())
                .sum(),
            selection_degraded: selected.degraded,
        });
    }
    let full_matrix = build_feature_matrix(source, &[], strategy);
    let full_rows = full_matrix
        .training_by_role
        .get(role_name)
        .cloned()
        .unwrap_or_default();
    let selected = fixed.unwrap_or_else(|| select_candidate_detailed(source, role, strategy));
    let refs = full_rows.iter().collect::<Vec<_>>();
    let model = fit_hierarchical_model_regularized(
        &refs,
        role,
        selected.variant,
        strategy,
        selected.multiplier,
        selected.lambda_core,
        selected.lambda_cohort,
    );
    let calibration_scale = model
        .as_ref()
        .map(|model| {
            fit_calibration_scale(
                model,
                &full_rows,
                role,
                selected.variant,
                strategy,
                selected.multiplier,
            )
        })
        .unwrap_or_else(|| CalibrationScale::fit([]));
    let names = full_rows
        .first()
        .map(|r| role_vector(r, role, selected.variant, strategy).names)
        .unwrap_or_default();
    let full_refit_weights: BTreeMap<String, f64> = names
        .into_iter()
        .zip(
            model
                .as_ref()
                .map(|model| model.core_weights.as_slice())
                .unwrap_or_default()
                .iter()
                .copied(),
        )
        .map(|(n, v)| (n.into(), v))
        .collect();
    let improved = fold_reports
        .iter()
        .filter(|f| f.brier_delta.unwrap_or(0.0) < 0.0)
        .count();
    let mut rejected = Vec::new();
    if improved < 4 {
        rejected.push("improved_folds_below_4".into())
    }
    if selected.degraded {
        rejected.push("inner_forward_selection_insufficient_data_core_1_25".into());
    }
    if !full_refit_weights
        .get("final")
        .zip(full_refit_weights.get("nonfinal"))
        .map(|(f, n)| *f > 0.0 && f > n)
        .unwrap_or(false)
    {
        rejected.push("final_weight_not_strictly_above_nonfinal".into())
    }
    let adjustment_samples = diagnostic_effects
        .iter()
        .map(|(base, effect, mode, _)| formal_diagnostic_adjustment(*base, *effect, role, *mode))
        .collect::<Vec<_>>();
    let mut bounded_feature_shares = diagnostic_effects
        .iter()
        .map(|(base, _, mode, contributions)| {
            bounded_single_feature_share(*base, contributions, role, *mode)
        })
        .collect::<Vec<_>>();
    bounded_feature_shares.sort_by(f64::total_cmp);
    let p95_single_feature_share = bounded_feature_shares
        .get(((bounded_feature_shares.len() as f64 * 0.95).ceil() as usize).saturating_sub(1))
        .copied()
        .unwrap_or(0.0);
    if p95_single_feature_share > 0.35 {
        rejected.push("p95_single_feature_contribution_above_35_percent".into());
    }
    StrategyReport {
        role,
        strategy: format!("{strategy:?}").to_lowercase(),
        preliminary: true,
        performance_adjustment: 0.0,
        variant: selected.variant,
        final_multiplier: selected.multiplier,
        lambda_core: selected.lambda_core,
        lambda_cohort: selected.lambda_cohort,
        folds: fold_reports,
        improved_folds: improved,
        metrics: metrics(&all_a, &all_p),
        baseline_metrics: metrics(&all_a, &all_base),
        full_refit_weights,
        calibration_scale,
        average_adjustment: if adjustment_samples.is_empty() {
            0.0
        } else {
            adjustment_samples
                .iter()
                .map(|sample| sample.0)
                .sum::<f64>()
                / adjustment_samples.len() as f64
        },
        p95_single_feature_share,
        upper_cap_rate: adjustment_samples.iter().filter(|sample| sample.1).count() as f64
            / adjustment_samples.len().max(1) as f64,
        lower_cap_rate: adjustment_samples.iter().filter(|sample| sample.2).count() as f64
            / adjustment_samples.len().max(1) as f64,
        missing_values: missing,
        rejected_reasons: rejected,
    }
}

pub(crate) fn select_candidate_detailed(
    rows: &[TrainingRow],
    role: Role,
    strategy: MatrixStrategy,
) -> CandidateSelection {
    let rows = eligible_performance_rows(rows);
    let inner = raw_folds(&rows, 3);
    if inner.len() < 2 {
        return CandidateSelection::core_degraded();
    }
    let mut selected = CandidateSelection {
        degraded: false,
        ..CandidateSelection::core_degraded()
    };
    let mut best = f64::INFINITY;
    for variant in [ModelVariant::Core, ModelVariant::RoleFull] {
        for multiplier in MULTIPLIERS {
            for (lambda_core, lambda_cohort) in [(0.02, 0.05), (0.05, 0.10)] {
                let mut loss = 0.0;
                let mut count = 0usize;
                let mut valid = true;
                for fold in &inner {
                    let raw_train = fold
                        .training
                        .iter()
                        .map(|&i| rows[i].clone())
                        .collect::<Vec<_>>();
                    let raw_validation = fold
                        .validation
                        .iter()
                        .map(|&i| rows[i].clone())
                        .collect::<Vec<_>>();
                    let matrix = build_feature_matrix(&raw_train, &raw_validation, strategy);
                    let role_name = match role {
                        Role::Tank => "tank",
                        Role::Damage => "damage",
                        Role::Support => "support",
                    };
                    let train_rows = matrix
                        .training_by_role
                        .get(role_name)
                        .cloned()
                        .unwrap_or_default();
                    let validation_rows = matrix
                        .validation_by_role
                        .get(role_name)
                        .cloned()
                        .unwrap_or_default();
                    if train_rows.is_empty() || validation_rows.is_empty() {
                        valid = false;
                        break;
                    }
                    let train = train_rows.iter().collect::<Vec<_>>();
                    let Some(model) = fit_hierarchical_model_regularized(
                        &train,
                        role,
                        variant,
                        strategy,
                        multiplier,
                        lambda_core,
                        lambda_cohort,
                    ) else {
                        valid = false;
                        break;
                    };
                    let proto = train_rows
                        .first()
                        .map(|r| role_vector(r, role, variant, strategy));
                    let identifiable = model.core_weights.len()
                        == proto.as_ref().map(|p| p.values.len()).unwrap_or(usize::MAX)
                        && proto
                            .and_then(|p| {
                                p.final_index.zip(p.nonfinal_index).map(|(f, n)| {
                                    model.core_weights[f] > 0.0
                                        && model.core_weights[f] > model.core_weights[n]
                                })
                            })
                            .unwrap_or(false);
                    if !identifiable {
                        valid = false;
                        break;
                    }
                    for row in &validation_rows {
                        let p = predict_probability(&model, row);
                        let e = (row.result - p).powi(2);
                        if !e.is_finite() {
                            valid = false;
                            break;
                        }
                        loss += e;
                        count += 1
                    }
                }
                if !valid || count == 0 {
                    continue;
                }
                let score = loss / count as f64;
                if score < best {
                    best = score;
                    selected = CandidateSelection {
                        variant,
                        multiplier,
                        lambda_core,
                        lambda_cohort,
                        degraded: false,
                    }
                }
            }
        }
    }
    selected
}
