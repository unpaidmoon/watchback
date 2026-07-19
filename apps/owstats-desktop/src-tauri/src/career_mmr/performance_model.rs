use super::{
    input::{Role, TrainingCohort},
    performance::{HealingTakenStrategy, MatrixRow},
    performance_forward::{role_vector_with_multiplier, ModelVariant},
};
use std::collections::{BTreeMap, HashMap};

pub const MODEL_FIT_STEPS: usize = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct FitConfig {
    pub lambda_core: f64,
    pub lambda_cohort: f64,
    pub learning_rate: f64,
    pub steps: usize,
    pub min_cohort_offset_rows: usize,
    pub final_multiplier: f64,
}

impl Default for FitConfig {
    fn default() -> Self {
        Self {
            lambda_core: 0.02,
            lambda_cohort: 0.05,
            learning_rate: 0.02,
            steps: MODEL_FIT_STEPS,
            min_cohort_offset_rows: 150,
            final_multiplier: 1.25,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HierarchicalModel {
    pub role: Role,
    pub strategy: HealingTakenStrategy,
    pub variant: ModelVariant,
    pub feature_names: Vec<String>,
    pub core_weights: Vec<f64>,
    pub cohort_offsets: BTreeMap<TrainingCohort, Vec<f64>>,
    pub cohort_samples: BTreeMap<TrainingCohort, usize>,
    pub final_multiplier: f64,
}

impl HierarchicalModel {
    pub fn feature_index(&self, name: &str) -> Option<usize> {
        self.feature_names
            .iter()
            .position(|candidate| candidate == name)
    }

    pub fn core_weight(&self, name: &str) -> Option<f64> {
        self.feature_index(name)
            .map(|index| self.core_weights[index])
    }

    pub fn cohort_offset(&self, cohort: TrainingCohort) -> Option<&[f64]> {
        self.cohort_offsets.get(&cohort).map(Vec::as_slice)
    }

    pub fn effective_weights(&self, cohort: TrainingCohort) -> Vec<f64> {
        match self.cohort_offsets.get(&cohort) {
            Some(offset) => self
                .core_weights
                .iter()
                .zip(offset)
                .map(|(core, delta)| core + delta)
                .collect(),
            None => self.core_weights.clone(),
        }
    }
}

fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}

fn logit(probability: f64) -> f64 {
    let value = probability.clamp(1.0e-6, 1.0 - 1.0e-6);
    (value / (1.0 - value)).ln()
}

#[derive(Clone, Copy)]
struct ConstraintIndices {
    final_hit: Option<usize>,
    nonfinal: Option<usize>,
    death: Option<usize>,
    debt: Option<usize>,
}

fn constraint_indices(names: &[String]) -> ConstraintIndices {
    let index = |name: &str| names.iter().position(|candidate| candidate == name);
    ConstraintIndices {
        final_hit: index("final"),
        nonfinal: index("nonfinal"),
        death: index("death"),
        debt: index("debt"),
    }
}

fn project(weights: &mut [f64], indices: ConstraintIndices, multiplier: f64) {
    const EPSILON: f64 = 1.0e-9;
    if let Some(index) = indices.nonfinal {
        weights[index] = weights[index].max(EPSILON);
    }
    if let Some(index) = indices.final_hit {
        weights[index] = weights[index].max(EPSILON);
    }
    if let (Some(final_index), Some(nonfinal_index)) = (indices.final_hit, indices.nonfinal) {
        weights[final_index] = weights[final_index].max(multiplier * weights[nonfinal_index]);
    }
    if let Some(index) = indices.death {
        weights[index] = weights[index].min(-EPSILON);
    }
    if let Some(index) = indices.debt {
        weights[index] = weights[index].min(0.0);
    }
}

struct FitSample {
    values: Vec<f64>,
    offset_index: Option<usize>,
    logit_offset: f64,
    result: f64,
    sample_weight: f64,
}

fn season_weights(rows: &[&MatrixRow]) -> HashMap<i64, f64> {
    let mut counts = HashMap::<i64, usize>::new();
    for row in rows {
        *counts.entry(row.season).or_default() += 1;
    }
    let seasons = counts.len().max(1) as f64;
    let total = rows.len() as f64;
    counts
        .into_iter()
        .map(|(season, count)| (season, total / (seasons * count as f64)))
        .collect()
}

pub fn fit_model(
    rows: &[&MatrixRow],
    role: Role,
    variant: ModelVariant,
    strategy: HealingTakenStrategy,
    config: &FitConfig,
) -> Option<HierarchicalModel> {
    let rows = rows
        .iter()
        .copied()
        .filter(|row| row.mode_family.training_cohort() != TrainingCohort::OutcomeOnly)
        .collect::<Vec<_>>();
    let first = *rows.first()?;
    let prototype =
        role_vector_with_multiplier(first, role, variant, strategy, config.final_multiplier);
    let feature_names = prototype
        .names
        .iter()
        .map(|name| (*name).into())
        .collect::<Vec<_>>();
    let width = feature_names.len();
    if width == 0
        || rows.iter().any(|row| {
            role_vector_with_multiplier(row, role, variant, strategy, config.final_multiplier)
                .values
                .len()
                != width
        })
    {
        return None;
    }

    let mut cohort_samples = BTreeMap::<TrainingCohort, usize>::new();
    for row in &rows {
        *cohort_samples
            .entry(row.mode_family.training_cohort())
            .or_default() += 1;
    }
    let active = cohort_samples
        .iter()
        .filter_map(|(&cohort, &count)| {
            (cohort != TrainingCohort::OutcomeOnly && count >= config.min_cohort_offset_rows)
                .then_some(cohort)
        })
        .collect::<Vec<_>>();
    let active_indices = active
        .iter()
        .enumerate()
        .map(|(index, cohort)| (*cohort, index))
        .collect::<HashMap<_, _>>();
    let constraints = constraint_indices(&feature_names);
    let mut core = vec![0.0; width];
    project(&mut core, constraints, config.final_multiplier);
    let mut offsets = vec![vec![0.0; width]; active.len()];
    let balancing = season_weights(&rows);
    let samples = rows
        .iter()
        .map(|row| {
            let cohort = row.mode_family.training_cohort();
            FitSample {
                values: role_vector_with_multiplier(
                    row,
                    role,
                    variant,
                    strategy,
                    config.final_multiplier,
                )
                .values,
                offset_index: active_indices.get(&cohort).copied(),
                logit_offset: logit(row.expected_win_rate),
                result: row.result.clamp(0.0, 1.0),
                sample_weight: row.data_quality.clamp(0.0, 1.0)
                    * balancing.get(&row.season).copied().unwrap_or(1.0),
            }
        })
        .collect::<Vec<_>>();
    let normalizer = samples
        .iter()
        .map(|sample| sample.sample_weight)
        .sum::<f64>()
        .max(f64::EPSILON);

    let mut core_gradient = vec![0.0; width];
    let mut offset_gradients = vec![vec![0.0; width]; active.len()];
    let mut effective = vec![0.0; width];
    for _ in 0..config.steps {
        core_gradient.fill(0.0);
        for gradient in &mut offset_gradients {
            gradient.fill(0.0);
        }
        for sample in &samples {
            let delta = sample.offset_index.map(|index| offsets[index].as_slice());
            let linear = sample.logit_offset
                + sample
                    .values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        (core[index] + delta.map(|values| values[index]).unwrap_or(0.0)) * value
                    })
                    .sum::<f64>();
            let error = sigmoid(linear) - sample.result;
            for (index, value) in sample.values.iter().enumerate() {
                let gradient = sample.sample_weight * error * value / normalizer;
                core_gradient[index] += gradient;
                if let Some(offset_index) = sample.offset_index {
                    offset_gradients[offset_index][index] += gradient;
                }
            }
        }
        for (weight, gradient) in core.iter_mut().zip(core_gradient.iter().copied()) {
            *weight -= config.learning_rate * (gradient + 2.0 * config.lambda_core * *weight);
        }
        project(&mut core, constraints, config.final_multiplier);
        for (offset_index, delta) in offsets.iter_mut().enumerate() {
            let gradient = &offset_gradients[offset_index];
            for (index, value) in delta.iter_mut().enumerate() {
                *value -=
                    config.learning_rate * (gradient[index] + 2.0 * config.lambda_cohort * *value);
            }
            for index in 0..width {
                effective[index] = core[index] + delta[index];
            }
            project(&mut effective, constraints, config.final_multiplier);
            for (index, value) in delta.iter_mut().enumerate() {
                *value = effective[index] - core[index];
            }
        }
        if core
            .iter()
            .chain(offsets.iter().flatten())
            .any(|value| !value.is_finite())
        {
            return None;
        }
    }

    let cohort_offsets = active.into_iter().zip(offsets).collect();
    Some(HierarchicalModel {
        role,
        strategy,
        variant,
        feature_names,
        core_weights: core,
        cohort_offsets,
        cohort_samples,
        final_multiplier: config.final_multiplier,
    })
}

pub fn predict_probability(model: &HierarchicalModel, row: &MatrixRow) -> f64 {
    let vector = role_vector_with_multiplier(
        row,
        model.role,
        model.variant,
        model.strategy,
        model.final_multiplier,
    );
    let weights = model.effective_weights(row.mode_family.training_cohort());
    sigmoid(
        logit(row.expected_win_rate)
            + weights
                .iter()
                .zip(vector.values)
                .map(|(weight, value)| weight * value)
                .sum::<f64>(),
    )
}
