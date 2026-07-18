use super::{
    performance::{
        build_feature_matrix, eligible_performance_rows, HealingTakenStrategy, MatrixRow,
    },
    performance_forward::{
        evaluate_forward, fit, metrics, predict, role_vector, select_candidate, MetricReport,
        ModelVariant, Role, StrategyReport,
    },
    training::TrainingRow,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_LOO_WORSENED_FRACTION: f64 = 0.25;
const MAX_ABS_AVERAGE_ADJUSTMENT: f64 = 0.02;

#[derive(Debug, Clone, PartialEq)]
pub struct StabilityConfig {
    pub bootstrap_iterations: usize,
    pub seed: u64,
    pub max_loo_worsened_fraction: f64,
    pub max_abs_average_adjustment: f64,
    /// Cheap projected coordinate passes over a precomputed small Gram matrix.
    pub optimizer_passes: usize,
}
impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            bootstrap_iterations: 1000,
            seed: 0x5eed,
            max_loo_worsened_fraction: MAX_LOO_WORSENED_FRACTION,
            max_abs_average_adjustment: MAX_ABS_AVERAGE_ADJUSTMENT,
            optimizer_passes: 12,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LooReport {
    pub validation_groups: usize,
    pub skipped_groups: usize,
    pub mean_brier_delta: Option<f64>,
    pub worst_brier_delta: Option<f64>,
    pub worsened_groups: usize,
    pub systematic_worsening: bool,
    pub candidate_rows: usize,
    pub evaluated_rows: usize,
}
impl LooReport {
    pub fn from_deltas(deltas: Vec<Option<f64>>) -> Self {
        let candidate_rows = deltas.len();
        let evaluated_rows = deltas.iter().filter(|d| d.is_some()).count();
        let skipped_groups = deltas.iter().filter(|d| d.is_none()).count();
        let values = deltas
            .into_iter()
            .flatten()
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let validation_groups = values.len();
        let worsened_groups = values.iter().filter(|&&v| v > 0.0).count();
        Self {
            validation_groups,
            skipped_groups,
            mean_brier_delta: mean(&values),
            worst_brier_delta: values.iter().copied().max_by(f64::total_cmp),
            worsened_groups,
            systematic_worsening: validation_groups > 0 && worsened_groups * 2 > validation_groups,
            candidate_rows,
            evaluated_rows,
        }
    }
    fn worsened_fraction(&self) -> f64 {
        self.worsened_groups as f64 / self.validation_groups.max(1) as f64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoefficientStability {
    pub weight: String,
    pub direction_stability: f64,
    pub ci95_low: f64,
    pub ci95_high: f64,
    pub critical_constraint: bool,
    pub reference_nonzero: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BootstrapWorkEstimate {
    pub baseline_rebuilds: usize,
    pub normal_equation_cells: usize,
    pub coordinate_updates: usize,
}
pub fn bootstrap_work_estimate(
    rows: usize,
    features: usize,
    iterations: usize,
    passes: usize,
) -> BootstrapWorkEstimate {
    BootstrapWorkEstimate {
        baseline_rebuilds: iterations,
        normal_equation_cells: rows
            .saturating_mul(features)
            .saturating_mul(features)
            .saturating_mul(iterations),
        coordinate_updates: features.saturating_mul(passes).saturating_mul(iterations),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BootstrapReport {
    pub requested_iterations: usize,
    pub completed_iterations: usize,
    pub skipped_iterations: usize,
    pub fixed_variant: Option<ModelVariant>,
    pub fixed_final_multiplier: Option<f64>,
    pub coefficients: Vec<CoefficientStability>,
    pub mean_brier_delta: Option<f64>,
    pub brier_delta_ci95_low: Option<f64>,
    pub brier_delta_ci95_high: Option<f64>,
    pub mean_auc: Option<f64>,
    pub mean_ece: Option<f64>,
}
impl BootstrapReport {
    pub fn testing_direction(rate: f64) -> Self {
        Self {
            coefficients: vec![CoefficientStability {
                weight: "final".into(),
                direction_stability: rate,
                ci95_low: 0.0,
                ci95_high: 0.0,
                critical_constraint: true,
                reference_nonzero: true,
            }],
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StabilityReport {
    pub role: Role,
    pub strategy: String,
    pub forward: StrategyReport,
    pub leave_one_season_out: LooReport,
    pub leave_one_hero_out: LooReport,
    pub bootstrap: BootstrapReport,
    pub eligible: bool,
    pub rejected_reasons: Vec<String>,
    pub performance_adjustment: f64,
    #[serde(skip)]
    max_loo_worsened_fraction: f64,
    #[serde(skip)]
    max_abs_average_adjustment: f64,
}
impl StabilityReport {
    pub fn testing(role: Role, strategy: &str) -> Self {
        let forward = StrategyReport {
            role,
            strategy: strategy.into(),
            preliminary: true,
            performance_adjustment: 0.0,
            variant: ModelVariant::Core,
            final_multiplier: 1.0,
            folds: vec![],
            improved_folds: 5,
            metrics: Some(MetricReport {
                auc: 0.5,
                brier: 0.2,
                ece: 0.1,
            }),
            full_refit_weights: BTreeMap::new(),
            average_adjustment: 0.0,
            missing_values: 0,
            rejected_reasons: vec![],
        };
        Self {
            role,
            strategy: strategy.into(),
            forward,
            leave_one_season_out: LooReport::default(),
            leave_one_hero_out: LooReport::default(),
            bootstrap: BootstrapReport::default(),
            eligible: false,
            rejected_reasons: vec![],
            performance_adjustment: 0.0,
            max_loo_worsened_fraction: MAX_LOO_WORSENED_FRACTION,
            max_abs_average_adjustment: MAX_ABS_AVERAGE_ADJUSTMENT,
        }
    }
    pub fn apply_gates(&mut self) {
        let mut reasons = self.forward.rejected_reasons.clone();
        if self.forward.improved_folds < 4 {
            push_once(&mut reasons, "forward_improved_folds_below_4");
        }
        if self
            .bootstrap
            .coefficients
            .iter()
            .any(|c| c.reference_nonzero && c.direction_stability < 0.9)
        {
            push_once(
                &mut reasons,
                "coefficient_direction_stability_below_90_percent",
            );
        }
        if self
            .bootstrap
            .coefficients
            .iter()
            .any(|c| c.critical_constraint && c.direction_stability < 0.9)
        {
            push_once(
                &mut reasons,
                "critical_direction_stability_below_90_percent",
            );
        }
        if self
            .bootstrap
            .coefficients
            .iter()
            .any(|c| !c.reference_nonzero)
        {
            push_once(&mut reasons, "zero_reference_coefficient_unstable");
        }
        if self
            .bootstrap
            .coefficients
            .iter()
            .any(|c| c.ci95_low <= 0.0 && c.ci95_high >= 0.0)
        {
            push_once(&mut reasons, "coefficient_ci95_crosses_zero");
        }
        if self.leave_one_season_out.validation_groups < 2 {
            push_once(&mut reasons, "season_loo_valid_groups_below_2");
        }
        if self.leave_one_hero_out.validation_groups < 5 {
            push_once(&mut reasons, "hero_loo_valid_groups_below_5");
        }
        if self.leave_one_season_out.candidate_rows > 0
            && self.leave_one_season_out.evaluated_rows * 5
                < self.leave_one_season_out.candidate_rows * 4
        {
            push_once(&mut reasons, "season_loo_row_coverage_below_80_percent");
        }
        if self.leave_one_hero_out.candidate_rows > 0
            && self.leave_one_hero_out.evaluated_rows * 5
                < self.leave_one_hero_out.candidate_rows * 4
        {
            push_once(&mut reasons, "hero_loo_row_coverage_below_80_percent");
        }
        if self.bootstrap.requested_iterations == 0 {
            push_once(&mut reasons, "bootstrap_iterations_zero");
        } else if self.bootstrap.completed_iterations * 10 < self.bootstrap.requested_iterations * 9
        {
            push_once(&mut reasons, "bootstrap_completion_below_90_percent");
        }
        for (name, loo) in [
            ("season", &self.leave_one_season_out),
            ("hero", &self.leave_one_hero_out),
        ] {
            if loo.mean_brier_delta.is_some_and(|v| v > 0.0) {
                push_once(
                    &mut reasons,
                    &format!("{name}_loo_mean_brier_delta_above_zero"),
                );
            }
            if loo.worsened_fraction() > self.max_loo_worsened_fraction {
                push_once(
                    &mut reasons,
                    &format!("{name}_loo_worsened_fraction_above_25_percent"),
                );
            }
        }
        if self.bootstrap.brier_delta_ci95_high.is_none_or(|v| v > 0.0) {
            push_once(&mut reasons, "bootstrap_brier_delta_ci95_upper_above_zero");
        }
        if self.forward.average_adjustment.abs() > self.max_abs_average_adjustment {
            push_once(&mut reasons, "average_adjustment_abs_above_0_02");
        }
        if self.role == Role::Support {
            push_once(&mut reasons, "support_conservative_hold");
        }
        self.rejected_reasons = reasons;
        self.eligible = self.rejected_reasons.is_empty();
        self.performance_adjustment = 0.0;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoleSelection {
    pub role: Role,
    pub selected: Option<String>,
    pub reports: Vec<StabilityReport>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StabilityEvaluation {
    pub reports: Vec<StabilityReport>,
    pub roles: Vec<RoleSelection>,
    pub worker_errors: Vec<String>,
}

pub fn evaluate_stability(source: &[TrainingRow], config: StabilityConfig) -> StabilityEvaluation {
    let source = eligible_performance_rows(source);
    let forwards = evaluate_forward(&source);
    let (mut reports, worker_errors) = std::thread::scope(|scope| {
        let source = &source;
        let handles = [Role::Tank, Role::Damage, Role::Support].map(|role| {
            let role_forwards = forwards
                .iter()
                .filter(|f| f.role == role)
                .cloned()
                .collect::<Vec<_>>();
            let config = &config;
            scope.spawn(move || {
                let role_rows = source
                    .iter()
                    .filter(|r| r.role == role_name(role))
                    .cloned()
                    .collect::<Vec<_>>();
                role_forwards
                    .into_iter()
                    .map(|forward| {
                        let strategy = parse_strategy(&forward.strategy);
                        let season = loo_season(&role_rows, role, strategy);
                        let hero = loo_hero(&role_rows, role, strategy);
                        let bootstrap = bootstrap(
                            &role_rows,
                            role,
                            strategy,
                            forward.variant,
                            forward.final_multiplier,
                            config,
                        );
                        let mut report = StabilityReport {
                            role,
                            strategy: forward.strategy.clone(),
                            forward,
                            leave_one_season_out: season,
                            leave_one_hero_out: hero,
                            bootstrap,
                            eligible: false,
                            rejected_reasons: vec![],
                            performance_adjustment: 0.0,
                            max_loo_worsened_fraction: config.max_loo_worsened_fraction,
                            max_abs_average_adjustment: config.max_abs_average_adjustment,
                        };
                        report.apply_gates();
                        report
                    })
                    .collect::<Vec<_>>()
            })
        });
        merge_worker_results(
            handles
                .into_iter()
                .zip([Role::Tank, Role::Damage, Role::Support])
                .map(|(handle, role)| (role, handle.join().ok()))
                .collect(),
        )
    });
    reports.sort_by_key(|report| (report.role, strategy_order(&report.strategy)));
    let roles = [Role::Tank, Role::Damage, Role::Support]
        .into_iter()
        .map(|role| {
            let items = reports
                .iter()
                .filter(|r| r.role == role)
                .cloned()
                .collect::<Vec<_>>();
            RoleSelection {
                role,
                selected: select_eligible(role, &items),
                reports: items,
            }
        })
        .collect();
    StabilityEvaluation {
        reports,
        roles,
        worker_errors,
    }
}

pub fn merge_worker_results(
    results: Vec<(Role, Option<Vec<StabilityReport>>)>,
) -> (Vec<StabilityReport>, Vec<String>) {
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for (role, result) in results {
        match result {
            Some(mut value) => reports.append(&mut value),
            None => errors.push(format!("{}_role_worker_failed", role_name(role))),
        }
    }
    (reports, errors)
}

pub fn select_eligible(role: Role, reports: &[StabilityReport]) -> Option<String> {
    reports
        .iter()
        .filter(|r| r.role == role && r.eligible)
        .min_by(|a, b| {
            let af = a
                .forward
                .metrics
                .as_ref()
                .map(|m| m.brier)
                .unwrap_or(f64::INFINITY);
            let bf = b
                .forward
                .metrics
                .as_ref()
                .map(|m| m.brier)
                .unwrap_or(f64::INFINITY);
            af.total_cmp(&bf)
                .then_with(|| {
                    a.bootstrap
                        .mean_brier_delta
                        .unwrap_or(f64::INFINITY)
                        .total_cmp(&b.bootstrap.mean_brier_delta.unwrap_or(f64::INFINITY))
                })
                .then_with(|| bootstrap_stability(b).total_cmp(&bootstrap_stability(a)))
                .then_with(|| complexity(a).cmp(&complexity(b)))
                .then_with(|| a.strategy.cmp(&b.strategy))
        })
        .map(|r| r.strategy.clone())
}

fn complexity(r: &StabilityReport) -> usize {
    r.forward.full_refit_weights.len() + usize::from(r.forward.variant == ModelVariant::RoleFull)
}
fn bootstrap_stability(r: &StabilityReport) -> f64 {
    r.bootstrap
        .coefficients
        .iter()
        .map(|c| c.direction_stability)
        .fold(1.0, f64::min)
}

fn loo_season(source: &[TrainingRow], role: Role, strategy: HealingTakenStrategy) -> LooReport {
    let groups = source.iter().map(|r| r.season).collect::<BTreeSet<_>>();
    let evals = groups
        .into_iter()
        .map(|held| {
            let train = source
                .iter()
                .filter(|r| r.season != held)
                .cloned()
                .collect::<Vec<_>>();
            let valid = source
                .iter()
                .filter(|r| r.season == held)
                .cloned()
                .collect::<Vec<_>>();
            let candidate = valid.len();
            let (delta, evaluated) = validate_split(&train, &valid, role, strategy, None);
            (delta, candidate, evaluated)
        })
        .collect::<Vec<_>>();
    loo_from_evals(evals)
}

fn loo_hero(source: &[TrainingRow], role: Role, strategy: HealingTakenStrategy) -> LooReport {
    let groups = source
        .iter()
        .map(|r| r.hero_group.clone())
        .collect::<BTreeSet<_>>();
    let evals = groups
        .into_iter()
        .map(|held| {
            let held_matches = source
                .iter()
                .filter(|r| r.hero_group == held)
                .map(|r| r.match_group.as_str())
                .collect::<BTreeSet<_>>();
            let train = source
                .iter()
                .filter(|r| !held_matches.contains(r.match_group.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let valid = source
                .iter()
                .filter(|r| held_matches.contains(r.match_group.as_str()) && r.hero_group == held)
                .cloned()
                .collect::<Vec<_>>();
            let candidate = valid.len();
            let (delta, evaluated) = validate_split(&train, &valid, role, strategy, None);
            (delta, candidate, evaluated)
        })
        .collect::<Vec<_>>();
    loo_from_evals(evals)
}

fn loo_from_evals(evals: Vec<(Option<f64>, usize, usize)>) -> LooReport {
    let candidate_rows = evals.iter().map(|e| e.1).sum();
    let evaluated_rows = evals.iter().map(|e| e.2).sum();
    let mut report = LooReport::from_deltas(evals.into_iter().map(|e| e.0).collect());
    report.candidate_rows = candidate_rows;
    report.evaluated_rows = evaluated_rows;
    report
}

fn validate_split(
    train: &[TrainingRow],
    valid: &[TrainingRow],
    role: Role,
    strategy: HealingTakenStrategy,
    fixed: Option<(ModelVariant, f64)>,
) -> (Option<f64>, usize) {
    if valid.is_empty() {
        return (None, 0);
    }
    let matrix = build_feature_matrix(train, valid, strategy);
    let Some(tr) = matrix.training_by_role.get(role_name(role)) else {
        return (None, 0);
    };
    let Some(va) = matrix.validation_by_role.get(role_name(role)) else {
        return (None, 0);
    };
    if tr.is_empty() || va.is_empty() {
        return (None, va.len());
    }
    let selected = fixed.unwrap_or_else(|| {
        let x = select_candidate(train, role, strategy);
        (x.0, x.1)
    });
    let refs = tr.iter().collect::<Vec<_>>();
    let weights = fit(&refs, role, selected.0, strategy, selected.1);
    if weights.is_empty() {
        return (None, va.len());
    }
    let actual = va.iter().map(|r| r.result).collect::<Vec<_>>();
    let predicted = va
        .iter()
        .map(|r| predict(r, role, selected.0, strategy, selected.1, &weights))
        .collect::<Vec<_>>();
    let Some(model) = metrics(&actual, &predicted).map(|m| m.brier) else {
        return (None, va.len());
    };
    let base = va
        .iter()
        .map(|r| (r.result - r.expected_win_rate).powi(2))
        .sum::<f64>()
        / va.len() as f64;
    (Some(model - base), va.len())
}

pub fn clone_cluster_for_bootstrap(rows: &[TrainingRow], instance: usize) -> Vec<TrainingRow> {
    rows.iter()
        .cloned()
        .map(|mut r| {
            r.match_group = format!("bootstrap-{instance}");
            r
        })
        .collect()
}

fn fit_bootstrap(
    rows: &[&MatrixRow],
    role: Role,
    variant: ModelVariant,
    strategy: HealingTakenStrategy,
    multiplier: f64,
    passes: usize,
) -> Vec<f64> {
    let Some(first) = rows.first() else {
        return Vec::new();
    };
    let prototype = role_vector(first, role, variant, strategy);
    let p = prototype.values.len();
    let mut gram = vec![vec![0.0; p]; p];
    let mut target = vec![0.0; p];
    for row in rows {
        let vector = role_vector(row, role, variant, strategy);
        let y = row.result - row.expected_win_rate;
        for j in 0..p {
            target[j] += vector.values[j] * y;
            for k in j..p {
                gram[j][k] += vector.values[j] * vector.values[k];
            }
        }
    }
    let n = rows.len() as f64;
    for j in 0..p {
        target[j] /= n;
        for k in j..p {
            gram[j][k] /= n;
            gram[k][j] = gram[j][k];
        }
        gram[j][j] += 0.01;
    }
    let mut weights = vec![0.0; p];
    for _ in 0..passes.max(1) {
        for j in 0..p {
            let other = (0..p)
                .filter(|&k| k != j)
                .map(|k| gram[j][k] * weights[k])
                .sum::<f64>();
            weights[j] = (target[j] - other) / gram[j][j].max(f64::EPSILON);
            if Some(j) == prototype.final_index || Some(j) == prototype.nonfinal_index {
                weights[j] = weights[j].max(0.0);
            }
            if Some(j) == prototype.death_index || Some(j) == prototype.debt_index {
                weights[j] = weights[j].min(0.0);
            }
        }
        if let (Some(f), Some(n)) = (prototype.final_index, prototype.nonfinal_index) {
            if weights[n] > 0.0 {
                weights[f] = weights[f].max(weights[n] * multiplier);
            }
        }
    }
    weights
}

fn bootstrap(
    source: &[TrainingRow],
    role: Role,
    strategy: HealingTakenStrategy,
    variant: ModelVariant,
    multiplier: f64,
    config: &StabilityConfig,
) -> BootstrapReport {
    let mut clusters: BTreeMap<&str, Vec<TrainingRow>> = BTreeMap::new();
    for r in source {
        clusters.entry(&r.match_group).or_default().push(r.clone());
    }
    let clusters = clusters.into_values().collect::<Vec<_>>();
    let mut rng = Lcg(config.seed ^ role as u64 ^ strategy as u64);
    let full = build_feature_matrix(source, &[], strategy);
    let full_rows = full
        .training_by_role
        .get(role_name(role))
        .cloned()
        .unwrap_or_default();
    let full_refs = full_rows.iter().collect::<Vec<_>>();
    let full_w = fit(&full_refs, role, variant, strategy, multiplier);
    let names = full_rows
        .first()
        .map(|r| role_vector(r, role, variant, strategy).names)
        .unwrap_or_default();
    let mut samples = vec![Vec::new(); names.len()];
    let mut deltas = Vec::new();
    let mut aucs = Vec::new();
    let mut eces = Vec::new();
    let mut skipped = 0;
    for iteration in 0..config.bootstrap_iterations {
        if clusters.is_empty() {
            skipped += 1;
            continue;
        }
        let mut counts = vec![0usize; clusters.len()];
        let mut train = Vec::new();
        for draw in 0..clusters.len() {
            let i = rng.index(clusters.len());
            counts[i] += 1;
            train.extend(clone_cluster_for_bootstrap(
                &clusters[i],
                iteration * clusters.len() + draw,
            ));
        }
        let valid = clusters
            .iter()
            .enumerate()
            .filter(|(i, _)| counts[*i] == 0)
            .flat_map(|(_, c)| c.clone())
            .collect::<Vec<_>>();
        let matrix = build_feature_matrix(&train, &valid, strategy);
        let tr = matrix
            .training_by_role
            .get(role_name(role))
            .cloned()
            .unwrap_or_default();
        let va = matrix
            .validation_by_role
            .get(role_name(role))
            .cloned()
            .unwrap_or_default();
        if tr.is_empty() || va.is_empty() {
            skipped += 1;
            continue;
        }
        let refs = tr.iter().collect::<Vec<_>>();
        // Bootstrap fixes Task2A's full-data hyperparameters and uses fewer deterministic
        // optimizer steps; every resample still rebuilds its own robust baselines and refits.
        let w = fit_bootstrap(
            &refs,
            role,
            variant,
            strategy,
            multiplier,
            config.optimizer_passes,
        );
        if w.len() != names.len() {
            skipped += 1;
            continue;
        }
        let actual = va.iter().map(|r| r.result).collect::<Vec<_>>();
        let predicted = va
            .iter()
            .map(|r| predict(r, role, variant, strategy, multiplier, &w))
            .collect::<Vec<_>>();
        let Some(m) = metrics(&actual, &predicted) else {
            skipped += 1;
            continue;
        };
        let base = va
            .iter()
            .map(|r| (r.result - r.expected_win_rate).powi(2))
            .sum::<f64>()
            / va.len() as f64;
        deltas.push(m.brier - base);
        aucs.push(m.auc);
        eces.push(m.ece);
        for (i, v) in w.into_iter().enumerate() {
            samples[i].push(v)
        }
    }
    let coefficients = names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let mut s = samples[i].clone();
            let reference = full_w.get(i).copied().unwrap_or(0.0);
            let expected_positive = matches!(name, "final" | "nonfinal")
                || (!matches!(name, "death" | "debt") && reference > 0.0);
            let stable = s
                .iter()
                .filter(|&&v| if expected_positive { v > 0.0 } else { v < 0.0 })
                .count();
            CoefficientStability {
                weight: name.into(),
                direction_stability: stable as f64 / s.len().max(1) as f64,
                ci95_low: quantile(&mut s, 0.025).unwrap_or(0.0),
                ci95_high: quantile(&mut s, 0.975).unwrap_or(0.0),
                critical_constraint: matches!(name, "final" | "nonfinal" | "death" | "debt"),
                reference_nonzero: reference.abs() > 1e-12,
            }
        })
        .collect();
    let mut d = deltas.clone();
    BootstrapReport {
        requested_iterations: config.bootstrap_iterations,
        completed_iterations: deltas.len(),
        skipped_iterations: skipped,
        fixed_variant: Some(variant),
        fixed_final_multiplier: Some(multiplier),
        coefficients,
        mean_brier_delta: mean(&deltas),
        brier_delta_ci95_low: quantile(&mut d, 0.025),
        brier_delta_ci95_high: quantile(&mut deltas, 0.975),
        mean_auc: mean(&aucs),
        mean_ece: mean(&eces),
    }
}

struct Lcg(u64);
impl Lcg {
    fn index(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 32) as usize) % n
    }
}
fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}
fn quantile(v: &mut [f64], q: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[((v.len() - 1) as f64 * q).round() as usize])
}
fn role_name(r: Role) -> &'static str {
    match r {
        Role::Tank => "tank",
        Role::Damage => "damage",
        Role::Support => "support",
    }
}
fn parse_strategy(s: &str) -> HealingTakenStrategy {
    match s {
        "directnegative" => HealingTakenStrategy::DirectNegative,
        "heronormalized" => HealingTakenStrategy::HeroNormalized,
        "excesshealing" => HealingTakenStrategy::ExcessHealing,
        "resourceconversiondebt" => HealingTakenStrategy::ResourceConversionDebt,
        _ => HealingTakenStrategy::None,
    }
}
fn strategy_order(s: &str) -> usize {
    match parse_strategy(s) {
        HealingTakenStrategy::None => 0,
        HealingTakenStrategy::DirectNegative => 1,
        HealingTakenStrategy::HeroNormalized => 2,
        HealingTakenStrategy::ExcessHealing => 3,
        HealingTakenStrategy::ResourceConversionDebt => 4,
    }
}
fn push_once(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.into())
    }
}
