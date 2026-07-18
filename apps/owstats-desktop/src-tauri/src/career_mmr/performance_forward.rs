use super::{
    performance::{build_feature_matrix, HealingTakenStrategy as MatrixStrategy, MatrixRow},
    training::TrainingRow,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

pub use super::performance::HealingTakenStrategy;

const MULTIPLIERS: [f64; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];
const DIRECT_EXPOSURE_SCALE: f64 = 10_000.0;
const MAX_DIRECT_EXPOSURE: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Tank,
    Damage,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
                pairs.extend([
                    ("objective", value(f.objective_time)),
                    ("damage_conversion", value(conversion)),
                ]);
            }
            Role::Support => pairs.extend([
                ("healing", value(f.healing)),
                ("damage", value(f.damage)),
                ("objective", value(f.objective_time)),
                ("survival", value(f.survival)),
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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
    pub inner_validation_rows: usize,
    pub selection_degraded: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StrategyReport {
    pub role: Role,
    pub strategy: String,
    pub preliminary: bool,
    pub performance_adjustment: f64,
    pub variant: ModelVariant,
    pub final_multiplier: f64,
    pub folds: Vec<FoldReport>,
    pub improved_folds: usize,
    pub metrics: Option<MetricReport>,
    pub full_refit_weights: BTreeMap<String, f64>,
    pub average_adjustment: f64,
    pub missing_values: usize,
    pub rejected_reasons: Vec<String>,
}

pub(crate) fn fit(
    rows: &[&MatrixRow],
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    multiplier: f64,
) -> Vec<f64> {
    fit_with_steps(rows, role, variant, strategy, multiplier, 300)
}

pub(crate) fn fit_with_steps(
    rows: &[&MatrixRow],
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    multiplier: f64,
    gradient_steps: usize,
) -> Vec<f64> {
    if rows.is_empty() {
        return Vec::new();
    }
    let prototype = role_vector_with_multiplier(rows[0], role, variant, strategy, multiplier);
    let p = prototype.values.len();
    let mut w = vec![0.0; p];
    for _ in 0..gradient_steps {
        let mut g = vec![0.0; p];
        for row in rows {
            let x = role_vector_with_multiplier(row, role, variant, strategy, multiplier);
            let y = row.result - row.expected_win_rate;
            let e = w.iter().zip(&x.values).map(|(a, b)| a * b).sum::<f64>() - y;
            if !e.is_finite() {
                return Vec::new();
            }
            for j in 0..p {
                g[j] += 2.0 * e * x.values[j];
            }
        }
        for j in 0..p {
            g[j] = g[j] / rows.len() as f64 + 0.02 * w[j];
            if !g[j].is_finite() {
                return Vec::new();
            }
            w[j] -= 0.02 * g[j];
            if !w[j].is_finite() {
                return Vec::new();
            }
        }
        if let Some(j) = prototype.final_index {
            w[j] = w[j].max(0.0)
        }
        if let Some(j) = prototype.nonfinal_index {
            w[j] = w[j].max(0.0)
        }
        if let Some(j) = prototype.death_index {
            w[j] = w[j].min(0.0)
        }
        if let Some(j) = prototype.debt_index {
            w[j] = w[j].min(0.0)
        }
    }
    if let (Some(f), Some(n)) = (prototype.final_index, prototype.nonfinal_index) {
        if w[n] > 0.0 {
            w[f] = w[f].max(w[n] * multiplier)
        }
    }
    w
}
pub(crate) fn predict(
    row: &MatrixRow,
    role: Role,
    variant: ModelVariant,
    strategy: MatrixStrategy,
    multiplier: f64,
    w: &[f64],
) -> f64 {
    let x = role_vector_with_multiplier(row, role, variant, strategy, multiplier);
    (row.expected_win_rate
        + w.iter().zip(x.values).map(|(a, b)| a * b).sum::<f64>() * x.conservative_scale)
        .clamp(0.0, 1.0)
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
        .filter(|row| row.role == role_name)
        .cloned()
        .collect::<Vec<_>>();
    let folds = build_forward_folds(&rows, 5);
    let mut reports = Vec::new();
    let mut all_a = Vec::new();
    let mut all_p = Vec::new();
    let mut improved = 0;
    let mut selected = (ModelVariant::Core, 1.0);
    let mut best = f64::INFINITY;
    for variant in [ModelVariant::Core, ModelVariant::RoleFull] {
        for multiplier in MULTIPLIERS {
            let mut score = 0.0;
            let mut n = 0;
            let train = rows.iter().collect::<Vec<_>>();
            let w = fit(&train, role, variant, strategy, multiplier);
            for row in &rows {
                let p = predict(row, role, variant, strategy, multiplier, &w);
                score += (row.result - p).powi(2);
                n += 1;
            }
            let proto = rows
                .first()
                .map(|r| role_vector(r, role, variant, strategy));
            let identifiable = proto
                .and_then(|p| {
                    p.final_index
                        .zip(p.nonfinal_index)
                        .map(|(f, n)| w[f] > 0.0 && w[f] > w[n])
                })
                .unwrap_or(false);
            if !identifiable {
                continue;
            }
            if n > 0 && score / (n as f64) < best {
                best = score / n as f64;
                selected = (variant, multiplier)
            }
        }
    }
    for (number, fold) in folds.iter().enumerate() {
        let train = fold.training.iter().map(|&i| &rows[i]).collect::<Vec<_>>();
        let w = fit(&train, role, selected.0, strategy, selected.1);
        let a = fold
            .validation
            .iter()
            .map(|&i| rows[i].result)
            .collect::<Vec<_>>();
        let p = fold
            .validation
            .iter()
            .map(|&i| predict(&rows[i], role, selected.0, strategy, selected.1, &w))
            .collect::<Vec<_>>();
        let base = a
            .iter()
            .zip(fold.validation.iter())
            .map(|(a, &i)| (a - rows[i].expected_win_rate).powi(2))
            .sum::<f64>()
            / a.len().max(1) as f64;
        let m = metrics(&a, &p);
        let delta = m.as_ref().map(|x| base - x.brier);
        if delta.unwrap_or(0.0) > 0.0 {
            improved += 1
        }
        let rejected = fold
            .validation
            .iter()
            .map(|&i| {
                let x = role_vector(&rows[i], role, selected.0, strategy);
                let total = w
                    .iter()
                    .zip(&x.values)
                    .map(|(a, b)| (a * b).abs())
                    .sum::<f64>();
                w.iter()
                    .zip(x.values)
                    .map(|(a, b)| (a * b).abs())
                    .fold(0.0, f64::max)
                    > 0.35 * total.max(f64::EPSILON)
            })
            .collect::<Vec<_>>()
            .tap_mut(|v| v.sort())
            .get(
                ((fold.validation.len() as f64 * 0.95).floor() as usize)
                    .min(fold.validation.len().saturating_sub(1)),
            )
            .copied()
            .unwrap_or(false);
        all_a.extend(&a);
        all_p.extend(&p);
        let weights = role_vector(&rows[fold.validation[0]], role, selected.0, strategy)
            .names
            .into_iter()
            .zip(w.iter().copied())
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
            variant: selected.0,
            final_multiplier: selected.1,
            inner_validation_rows: 0,
            selection_degraded: true,
        });
    }
    let full = rows.iter().collect::<Vec<_>>();
    let w = fit(&full, role, selected.0, strategy, selected.1);
    let proto = rows
        .first()
        .map(|r| role_vector(r, role, selected.0, strategy));
    let full_refit_weights: BTreeMap<String, f64> = proto
        .map(|p| {
            p.names
                .into_iter()
                .zip(w.iter().copied())
                .map(|(n, v)| (n.into(), v))
                .collect()
        })
        .unwrap_or_default();
    let adjustments = rows
        .iter()
        .map(|r| predict(r, role, selected.0, strategy, selected.1, &w) - r.expected_win_rate)
        .collect::<Vec<_>>();
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
    StrategyReport {
        role,
        strategy: format!("{strategy:?}").to_lowercase(),
        preliminary: true,
        performance_adjustment: 0.0,
        variant: selected.0,
        final_multiplier: selected.1,
        folds: reports,
        improved_folds: improved,
        metrics: metrics(&all_a, &all_p),
        full_refit_weights,
        average_adjustment: if adjustments.is_empty() {
            0.0
        } else {
            adjustments.iter().sum::<f64>() / adjustments.len() as f64
        },
        missing_values: rows
            .iter()
            .map(|r| missing_values(r, role, selected.0, strategy))
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
            Role::Damage => values.extend([f.objective_time, damage_conversion(row, 1.0)]),
            Role::Support => values.extend([f.healing, f.damage, f.objective_time, f.survival]),
        }
    }
    values.iter().filter(|v| v.is_none()).count()
        + usize::from(strategy != MatrixStrategy::None && row.healing_taken_derived.is_none())
}

pub fn evaluate_forward(source: &[TrainingRow]) -> Vec<StrategyReport> {
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
                evaluate_raw_strategy(&role_rows, role, strategy)
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

fn evaluate_raw_strategy(
    source: &[TrainingRow],
    role: Role,
    strategy: MatrixStrategy,
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
    let mut missing = 0;
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
            .collect::<Vec<_>>();
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
        let selected = select_candidate(&train, role, strategy);
        let refs = train_rows.iter().collect::<Vec<_>>();
        let w = fit(&refs, role, selected.0, strategy, selected.1);
        let a = valid_rows.iter().map(|r| r.result).collect::<Vec<_>>();
        let p = valid_rows
            .iter()
            .map(|r| predict(r, role, selected.0, strategy, selected.1, &w))
            .collect::<Vec<_>>();
        let base = valid_rows
            .iter()
            .map(|r| (r.result - r.expected_win_rate).powi(2))
            .sum::<f64>()
            / valid_rows.len() as f64;
        let metric = metrics(&a, &p);
        let delta = metric.as_ref().map(|m| base - m.brier);
        let mut ratios = valid_rows
            .iter()
            .map(|r| {
                let x = role_vector(r, role, selected.0, strategy);
                let parts = w
                    .iter()
                    .zip(x.values)
                    .map(|(a, b)| (a * b).abs())
                    .collect::<Vec<_>>();
                let total = parts.iter().sum::<f64>();
                parts.into_iter().fold(0.0, f64::max) / total.max(f64::EPSILON)
            })
            .collect::<Vec<_>>();
        ratios.sort_by(f64::total_cmp);
        let p95 = ratios
            .get(((ratios.len() as f64 * 0.95).ceil() as usize).saturating_sub(1))
            .copied()
            .unwrap_or(0.0);
        let names = role_vector(&valid_rows[0], role, selected.0, strategy).names;
        let weights = names
            .into_iter()
            .zip(w)
            .map(|(n, v)| (n.into(), v))
            .collect();
        missing += valid_rows
            .iter()
            .map(|r| missing_values(r, role, selected.0, strategy))
            .sum::<usize>();
        all_a.extend(&a);
        all_p.extend(&p);
        fold_reports.push(FoldReport {
            fold: number + 1,
            training_rows: train_rows.len(),
            validation_rows: valid_rows.len(),
            metrics: metric,
            baseline_brier: Some(base),
            brier_delta: delta,
            rejected_35_percent: p95 > 0.35,
            weights,
            variant: selected.0,
            final_multiplier: selected.1,
            inner_validation_rows: raw_folds(&train, 3)
                .iter()
                .map(|f| f.validation.len())
                .sum(),
            selection_degraded: selected.2,
        });
    }
    let full_matrix = build_feature_matrix(source, &[], strategy);
    let full_rows = full_matrix
        .training_by_role
        .get(role_name)
        .cloned()
        .unwrap_or_default();
    let selected = select_candidate(source, role, strategy);
    let refs = full_rows.iter().collect::<Vec<_>>();
    let w = fit(&refs, role, selected.0, strategy, selected.1);
    let names = full_rows
        .first()
        .map(|r| role_vector(r, role, selected.0, strategy).names)
        .unwrap_or_default();
    let full_refit_weights: BTreeMap<String, f64> = names
        .into_iter()
        .zip(w.iter().copied())
        .map(|(n, v)| (n.into(), v))
        .collect();
    let improved = fold_reports
        .iter()
        .filter(|f| f.brier_delta.unwrap_or(0.0) > 0.0)
        .count();
    let mut rejected = Vec::new();
    if improved < 4 {
        rejected.push("improved_folds_below_4".into())
    }
    if selected.2 {
        rejected.push("inner_forward_selection_insufficient_data_core_1_25".into());
    }
    if fold_reports.iter().any(|f| f.rejected_35_percent) {
        rejected.push("p95_single_feature_contribution_above_35_percent".into())
    }
    if !full_refit_weights
        .get("final")
        .zip(full_refit_weights.get("nonfinal"))
        .map(|(f, n)| *f > 0.0 && f > n)
        .unwrap_or(false)
    {
        rejected.push("final_weight_not_strictly_above_nonfinal".into())
    }
    let adjustments = full_rows
        .iter()
        .map(|r| predict(r, role, selected.0, strategy, selected.1, &w) - r.expected_win_rate)
        .collect::<Vec<_>>();
    StrategyReport {
        role,
        strategy: format!("{strategy:?}").to_lowercase(),
        preliminary: true,
        performance_adjustment: 0.0,
        variant: selected.0,
        final_multiplier: selected.1,
        folds: fold_reports,
        improved_folds: improved,
        metrics: metrics(&all_a, &all_p),
        full_refit_weights,
        average_adjustment: if adjustments.is_empty() {
            0.0
        } else {
            adjustments.iter().sum::<f64>() / adjustments.len() as f64
        },
        missing_values: missing,
        rejected_reasons: rejected,
    }
}

pub(crate) fn select_candidate(
    rows: &[TrainingRow],
    role: Role,
    strategy: MatrixStrategy,
) -> (ModelVariant, f64, bool) {
    let inner = raw_folds(rows, 3);
    if inner.len() < 2 {
        return (ModelVariant::Core, 1.25, true);
    }
    let mut selected = (ModelVariant::Core, 1.25, false);
    let mut best = f64::INFINITY;
    for variant in [ModelVariant::Core, ModelVariant::RoleFull] {
        for multiplier in MULTIPLIERS {
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
                let w = fit(&train, role, variant, strategy, multiplier);
                let proto = train_rows
                    .first()
                    .map(|r| role_vector(r, role, variant, strategy));
                let identifiable = w.len()
                    == proto.as_ref().map(|p| p.values.len()).unwrap_or(usize::MAX)
                    && proto
                        .and_then(|p| {
                            p.final_index
                                .zip(p.nonfinal_index)
                                .map(|(f, n)| w[f] > 0.0 && w[f] > w[n])
                        })
                        .unwrap_or(false);
                if !identifiable {
                    valid = false;
                    break;
                }
                for row in &validation_rows {
                    let p = predict(row, role, variant, strategy, multiplier, &w);
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
                selected = (variant, multiplier, false)
            }
        }
    }
    selected
}

trait TapMut: Sized {
    fn tap_mut(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}
impl<T> TapMut for T {}
