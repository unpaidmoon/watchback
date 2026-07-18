use super::{
    input::{Role, TrainingCohort},
    performance::{HealingTakenStrategy, MatrixRow},
    performance_forward::{role_vector_with_multiplier, ModelVariant},
};
use std::collections::{BTreeMap, HashMap};

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
            steps: 500,
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

fn project(weights: &mut [f64], names: &[String], multiplier: f64) {
    const EPSILON: f64 = 1.0e-9;
    let index = |name: &str| names.iter().position(|candidate| candidate == name);
    if let Some(index) = index("nonfinal") {
        weights[index] = weights[index].max(EPSILON);
    }
    if let Some(index) = index("final") {
        weights[index] = weights[index].max(EPSILON);
    }
    if let (Some(final_index), Some(nonfinal_index)) = (index("final"), index("nonfinal")) {
        weights[final_index] = weights[final_index].max(multiplier * weights[nonfinal_index]);
    }
    if let Some(index) = index("death") {
        weights[index] = weights[index].min(-EPSILON);
    }
    if let Some(index) = index("debt") {
        weights[index] = weights[index].min(0.0);
    }
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
    let mut core = vec![0.0; width];
    project(&mut core, &feature_names, config.final_multiplier);
    let mut offsets = active
        .iter()
        .map(|&cohort| (cohort, vec![0.0; width]))
        .collect::<BTreeMap<_, _>>();
    let balancing = season_weights(&rows);
    let normalizer = rows
        .iter()
        .map(|row| {
            row.data_quality.clamp(0.0, 1.0) * balancing.get(&row.season).copied().unwrap_or(1.0)
        })
        .sum::<f64>()
        .max(f64::EPSILON);

    for _ in 0..config.steps {
        let mut core_gradient = vec![0.0; width];
        let mut offset_gradients = active
            .iter()
            .map(|&cohort| (cohort, vec![0.0; width]))
            .collect::<BTreeMap<_, _>>();
        for row in &rows {
            let vector =
                role_vector_with_multiplier(row, role, variant, strategy, config.final_multiplier);
            let cohort = row.mode_family.training_cohort();
            let effective = match offsets.get(&cohort) {
                Some(delta) => core
                    .iter()
                    .zip(delta)
                    .map(|(a, b)| a + b)
                    .collect::<Vec<_>>(),
                None => core.clone(),
            };
            let linear = logit(row.expected_win_rate)
                + effective
                    .iter()
                    .zip(&vector.values)
                    .map(|(weight, value)| weight * value)
                    .sum::<f64>();
            let error = sigmoid(linear) - row.result.clamp(0.0, 1.0);
            let sample_weight = row.data_quality.clamp(0.0, 1.0)
                * balancing.get(&row.season).copied().unwrap_or(1.0);
            for (index, value) in vector.values.iter().enumerate() {
                let gradient = sample_weight * error * value / normalizer;
                core_gradient[index] += gradient;
                if let Some(cohort_gradient) = offset_gradients.get_mut(&cohort) {
                    cohort_gradient[index] += gradient;
                }
            }
        }
        for (weight, gradient) in core.iter_mut().zip(core_gradient) {
            *weight -= config.learning_rate * (gradient + 2.0 * config.lambda_core * *weight);
        }
        project(&mut core, &feature_names, config.final_multiplier);
        for cohort in &active {
            let delta = offsets.get_mut(cohort).expect("active cohort offset");
            let gradient = &offset_gradients[cohort];
            for (index, value) in delta.iter_mut().enumerate() {
                *value -=
                    config.learning_rate * (gradient[index] + 2.0 * config.lambda_cohort * *value);
            }
            let mut effective = core
                .iter()
                .zip(delta.iter())
                .map(|(a, b)| a + b)
                .collect::<Vec<_>>();
            project(&mut effective, &feature_names, config.final_multiplier);
            for (index, value) in delta.iter_mut().enumerate() {
                *value = effective[index] - core[index];
            }
        }
        if core
            .iter()
            .chain(offsets.values().flatten())
            .any(|value| !value.is_finite())
        {
            return None;
        }
    }

    Some(HierarchicalModel {
        role,
        strategy,
        variant,
        feature_names,
        core_weights: core,
        cohort_offsets: offsets,
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
