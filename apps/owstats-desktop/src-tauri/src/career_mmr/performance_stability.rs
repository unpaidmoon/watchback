use super::{
    input::{ModeFamily, TrainingCohort},
    performance::{build_feature_matrix, eligible_performance_rows, HealingTakenStrategy},
    performance_forward::{
        evaluate_cell_strategy, evaluate_cell_strategy_fixed, evaluate_nested_cell,
        evaluate_nested_cell_fixed, evaluate_nested_cell_shadow,
        fit_hierarchical_model_regularized, metrics, select_candidate_detailed, CalibrationScale,
        CandidateSelection, MetricReport, ModelVariant, NestedCellReport, Role, StrategyReport,
    },
    performance_model::{predict_probability, FitConfig, HierarchicalModel, MODEL_FIT_STEPS},
    training::TrainingRow,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;

const MAX_LOO_WORSENED_FRACTION: f64 = 0.25;
const MAX_ABS_AVERAGE_ADJUSTMENT: f64 = 0.10;

pub const VALIDATION_MODES: [ModeFamily; 5] = [
    ModeFamily::RoleCompetitive,
    ModeFamily::RoleQuick,
    ModeFamily::OpenCompetitive,
    ModeFamily::OpenQuick,
    ModeFamily::StadiumCompetitive,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellDecision {
    Eligible,
    Shadow,
    OutcomeOnly,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AccountLooReport {
    pub candidate_groups: usize,
    pub evaluated_groups: usize,
    pub coverage_limited_groups: usize,
    pub mean_brier_delta: Option<f64>,
    pub worst_brier_delta: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingStage {
    Screen100,
    Candidate1000,
    Final10000,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotedCandidate {
    pub role: Role,
    pub strategy: String,
    pub variant: ModelVariant,
    pub final_multiplier: f64,
    pub lambda_core: f64,
    pub lambda_cohort: f64,
}

impl PromotedCandidate {
    fn selection(&self) -> CandidateSelection {
        CandidateSelection {
            variant: self.variant,
            multiplier: self.final_multiplier,
            lambda_core: self.lambda_core,
            lambda_cohort: self.lambda_cohort,
            degraded: false,
        }
    }
}
impl TrainingStage {
    pub fn bootstrap_iterations(self) -> usize {
        match self {
            Self::Screen100 => 100,
            Self::Candidate1000 => 1_000,
            Self::Final10000 => 10_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgressEvent {
    pub stage: TrainingStage,
    pub completed: usize,
    pub total: usize,
    pub elapsed_ms: u128,
    pub eta_ms: Option<u128>,
}

fn aggregate_progress_event(
    stage: TrainingStage,
    completed: usize,
    total: usize,
    elapsed_ms: u128,
) -> ProgressEvent {
    let completed = completed.min(total);
    let eta_ms = if completed == 0 {
        None
    } else if completed == total {
        Some(0)
    } else {
        Some((elapsed_ms.saturating_mul((total - completed) as u128) / completed as u128).max(1))
    };
    ProgressEvent {
        stage,
        completed,
        total,
        elapsed_ms,
        eta_ms,
    }
}

pub fn aggregate_progress_schedule(stage: TrainingStage, batch_size: usize) -> Vec<ProgressEvent> {
    aggregate_progress_schedule_with_outcome(stage, batch_size, 15, true)
}

pub fn aggregate_progress_schedule_with_outcome(
    stage: TrainingStage,
    batch_size: usize,
    nested_cells_completed: usize,
    success: bool,
) -> Vec<ProgressEvent> {
    let bootstrap_total = stage
        .bootstrap_iterations()
        .saturating_mul(3)
        .saturating_mul(5);
    let total = bootstrap_total.saturating_add(15);
    let batch_size = batch_size.max(1);
    let mut events = vec![aggregate_progress_event(stage, 0, total, 0)];
    let mut completed = 0usize;
    while completed < bootstrap_total {
        completed = completed.saturating_add(batch_size).min(bootstrap_total);
        events.push(aggregate_progress_event(
            stage,
            completed,
            total,
            completed as u128 * 2,
        ));
    }
    if success {
        for cell in 1..=nested_cells_completed.min(15) {
            let completed = bootstrap_total + cell;
            events.push(aggregate_progress_event(
                stage,
                completed,
                total,
                completed as u128 * 2,
            ));
        }
    }
    events
}

#[derive(Debug, Clone, PartialEq)]
pub struct StabilityConfig {
    pub bootstrap_iterations: usize,
    pub seed: u64,
    pub max_loo_worsened_fraction: f64,
    pub max_abs_average_adjustment: f64,
    pub promoted_candidates: Option<Vec<PromotedCandidate>>,
}
impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            bootstrap_iterations: 1000,
            seed: 0x5eed,
            max_loo_worsened_fraction: MAX_LOO_WORSENED_FRACTION,
            max_abs_average_adjustment: MAX_ABS_AVERAGE_ADJUSTMENT,
            promoted_candidates: None,
        }
    }
}
impl StabilityConfig {
    pub fn for_stage(stage: TrainingStage) -> Self {
        Self {
            bootstrap_iterations: stage.bootstrap_iterations(),
            ..Self::default()
        }
    }

    pub fn for_stage_with_promoted(
        stage: TrainingStage,
        promoted_candidates: Vec<PromotedCandidate>,
    ) -> Self {
        Self {
            promoted_candidates: Some(promoted_candidates),
            ..Self::for_stage(stage)
        }
    }

    pub fn snapshot(&self) -> StabilityConfigSnapshot {
        let defaults = FitConfig::default();
        StabilityConfigSnapshot {
            seed: self.seed,
            bootstrap_iterations: self.bootstrap_iterations,
            stage: stage_for_iterations(self.bootstrap_iterations),
            max_loo_worsened_fraction: self.max_loo_worsened_fraction,
            max_abs_average_adjustment: self.max_abs_average_adjustment,
            fit_variants: vec!["core".into(), "role_full".into()],
            final_hit_multipliers: vec![1.0, 1.25, 1.5, 1.75, 2.0],
            regularization_candidates: vec![[0.02, 0.05], [0.05, 0.10]],
            fit_learning_rate: defaults.learning_rate,
            fit_steps: MODEL_FIT_STEPS,
            minimum_cohort_offset_rows: defaults.min_cohort_offset_rows,
            minimum_improved_folds: 4,
            minimum_role_core_rows: 500,
            minimum_cohort_rows: 150,
            minimum_season_loo_groups: 2,
            minimum_hero_loo_groups: 5,
            minimum_loo_row_coverage: 0.80,
            minimum_bootstrap_completion: 0.90,
            minimum_critical_direction_stability: 0.90,
            maximum_bootstrap_brier_ci_high: 0.0,
            max_ece_worsening: 0.005,
            maximum_single_feature_share: 0.35,
            maximum_cap_hit_rate: 0.05,
            require_account_coverage: true,
            require_critical_ci_excludes_zero: true,
            require_nonzero_reference_coefficients: true,
            promoted_candidates: self.promoted_candidates.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StabilityConfigSnapshot {
    pub seed: u64,
    pub bootstrap_iterations: usize,
    pub stage: TrainingStage,
    pub max_loo_worsened_fraction: f64,
    pub max_abs_average_adjustment: f64,
    pub fit_variants: Vec<String>,
    pub final_hit_multipliers: Vec<f64>,
    pub regularization_candidates: Vec<[f64; 2]>,
    pub fit_learning_rate: f64,
    pub fit_steps: usize,
    pub minimum_cohort_offset_rows: usize,
    pub minimum_improved_folds: usize,
    pub minimum_role_core_rows: usize,
    pub minimum_cohort_rows: usize,
    pub minimum_season_loo_groups: usize,
    pub minimum_hero_loo_groups: usize,
    pub minimum_loo_row_coverage: f64,
    pub minimum_bootstrap_completion: f64,
    pub minimum_critical_direction_stability: f64,
    pub maximum_bootstrap_brier_ci_high: f64,
    pub max_ece_worsening: f64,
    pub maximum_single_feature_share: f64,
    pub maximum_cap_hit_rate: f64,
    pub require_account_coverage: bool,
    pub require_critical_ci_excludes_zero: bool,
    pub require_nonzero_reference_coefficients: bool,
    pub promoted_candidates: Option<Vec<PromotedCandidate>>,
}

fn stage_for_iterations(iterations: usize) -> TrainingStage {
    match iterations {
        0..=100 => TrainingStage::Screen100,
        101..=1_000 => TrainingStage::Candidate1000,
        _ => TrainingStage::Final10000,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoefficientStability {
    pub weight: String,
    pub scope: String,
    pub direction_stability: f64,
    pub ci95_low: f64,
    pub ci95_high: f64,
    pub critical_constraint: bool,
    pub reference_nonzero: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BootstrapWorkEstimate {
    pub per_cell_model_fits: usize,
    pub per_cell_baseline_rebuilds: usize,
    pub per_cell_matrix_rows_rebuilt: usize,
    pub per_cell_model_fit_steps: usize,
    pub per_cell_weighted_gradient_cells: usize,
    pub per_cell_projected_updates: usize,
    pub total_model_fits: usize,
    pub total_baseline_rebuilds: usize,
    pub total_matrix_rows_rebuilt: usize,
    pub total_model_fit_steps: usize,
    pub total_weighted_gradient_cells: usize,
    pub total_projected_updates: usize,
}
pub fn bootstrap_work_estimate(
    rows: usize,
    features: usize,
    iterations: usize,
    role_count: usize,
    strategy_count: usize,
) -> BootstrapWorkEstimate {
    let steps = super::performance_model::MODEL_FIT_STEPS;
    let cells = role_count.saturating_mul(strategy_count);
    let per_cell_matrix_rows_rebuilt = rows.saturating_mul(iterations);
    let per_cell_model_fit_steps = steps.saturating_mul(iterations);
    let per_cell_weighted_gradient_cells = rows
        .saturating_mul(features)
        .saturating_mul(steps)
        .saturating_mul(iterations);
    let per_cell_projected_updates = features.saturating_mul(steps).saturating_mul(iterations);
    BootstrapWorkEstimate {
        per_cell_model_fits: iterations,
        per_cell_baseline_rebuilds: iterations,
        per_cell_matrix_rows_rebuilt,
        per_cell_model_fit_steps,
        per_cell_weighted_gradient_cells,
        per_cell_projected_updates,
        total_model_fits: iterations.saturating_mul(cells),
        total_baseline_rebuilds: iterations.saturating_mul(cells),
        total_matrix_rows_rebuilt: per_cell_matrix_rows_rebuilt.saturating_mul(cells),
        total_model_fit_steps: per_cell_model_fit_steps.saturating_mul(cells),
        total_weighted_gradient_cells: per_cell_weighted_gradient_cells.saturating_mul(cells),
        total_projected_updates: per_cell_projected_updates.saturating_mul(cells),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BootstrapReport {
    pub requested_iterations: usize,
    pub completed_iterations: usize,
    pub skipped_iterations: usize,
    pub fixed_variant: Option<ModelVariant>,
    pub fixed_final_multiplier: Option<f64>,
    pub fixed_lambda_core: Option<f64>,
    pub fixed_lambda_cohort: Option<f64>,
    pub hierarchical_offset_iterations: usize,
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
                scope: "core".into(),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StabilityReport {
    pub role: Role,
    pub mode: ModeFamily,
    pub training_cohort: TrainingCohort,
    pub shared_parameter_version: String,
    pub strategy: String,
    pub forward: StrategyReport,
    pub leave_one_season_out: LooReport,
    pub leave_one_hero_out: LooReport,
    pub leave_one_account_out: AccountLooReport,
    pub bootstrap: BootstrapReport,
    pub eligible: bool,
    pub decision: CellDecision,
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
            lambda_core: 0.02,
            lambda_cohort: 0.05,
            folds: vec![],
            improved_folds: 5,
            metrics: Some(MetricReport {
                auc: 0.5,
                brier: 0.2,
                ece: 0.1,
            }),
            baseline_metrics: Some(MetricReport {
                auc: 0.5,
                brier: 0.25,
                ece: 0.1,
            }),
            full_refit_weights: BTreeMap::new(),
            calibration_scale: CalibrationScale::fit([]),
            average_adjustment: 0.0,
            p95_single_feature_share: 0.0,
            upper_cap_rate: 0.0,
            lower_cap_rate: 0.0,
            missing_values: 0,
            rejected_reasons: vec![],
        };
        Self {
            role,
            mode: ModeFamily::RoleCompetitive,
            training_cohort: TrainingCohort::RoleQueue,
            shared_parameter_version: "role_queue-v1".into(),
            strategy: strategy.into(),
            forward,
            leave_one_season_out: LooReport::default(),
            leave_one_hero_out: LooReport::default(),
            leave_one_account_out: AccountLooReport::default(),
            bootstrap: BootstrapReport::default(),
            eligible: false,
            decision: CellDecision::Shadow,
            rejected_reasons: vec![],
            performance_adjustment: 0.0,
            max_loo_worsened_fraction: MAX_LOO_WORSENED_FRACTION,
            max_abs_average_adjustment: MAX_ABS_AVERAGE_ADJUSTMENT,
        }
    }
    pub fn apply_gates(&mut self) {
        let mut reasons = self.forward.rejected_reasons.clone();
        if self.forward.improved_folds < 4 {
            push_once(&mut reasons, "improved_folds_below_4");
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
            .any(|c| c.critical_constraint && c.ci95_low <= 0.0 && c.ci95_high >= 0.0)
        {
            push_once(&mut reasons, "critical_coefficient_ci_crosses_zero");
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
        if self
            .bootstrap
            .brier_delta_ci95_high
            .is_none_or(|v| v >= 0.0)
        {
            push_once(&mut reasons, "bootstrap_brier_ci_not_below_zero");
        }
        if self
            .forward
            .metrics
            .as_ref()
            .zip(self.forward.baseline_metrics.as_ref())
            .is_some_and(|(candidate, baseline)| candidate.ece > baseline.ece + 0.005)
        {
            push_once(&mut reasons, "ece_worse_than_0_005");
        }
        if self.forward.p95_single_feature_share > 0.35 {
            push_once(
                &mut reasons,
                "p95_single_feature_contribution_above_35_percent",
            );
        }
        if self.forward.average_adjustment.abs() > self.max_abs_average_adjustment {
            push_once(&mut reasons, "average_adjustment_above_0_10_mmr");
        }
        if self.forward.upper_cap_rate > 0.05 || self.forward.lower_cap_rate > 0.05 {
            push_once(&mut reasons, "cap_hit_rate_above_5_percent");
        }
        if self.strategy == "resourceconversiondebt"
            && self
                .bootstrap
                .coefficients
                .iter()
                .find(|coefficient| coefficient.weight == "debt")
                .is_none_or(|coefficient| coefficient.ci95_high >= 0.0)
        {
            push_once(&mut reasons, "resource_debt_ci_upper_not_below_zero");
        }
        self.rejected_reasons = reasons;
        self.eligible = self.rejected_reasons.is_empty();
        self.decision = if self.eligible {
            CellDecision::Eligible
        } else {
            CellDecision::Shadow
        };
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
pub struct CellEvaluation {
    pub role: Role,
    pub mode: ModeFamily,
    pub training_cohort: TrainingCohort,
    pub shared_parameter_version: String,
    pub selected: Option<String>,
    pub decision: CellDecision,
    pub rejected_reasons: Vec<String>,
    pub nested_validation: NestedCellReport,
    pub reports: Vec<StabilityReport>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StabilityEvaluation {
    pub config: StabilityConfigSnapshot,
    pub reports: Vec<StabilityReport>,
    pub roles: Vec<RoleSelection>,
    pub cells: Vec<CellEvaluation>,
    pub worker_errors: Vec<String>,
}

pub fn evaluate_stability(source: &[TrainingRow], config: StabilityConfig) -> StabilityEvaluation {
    evaluate_stability_with_progress(source, config, |_| {})
}

pub fn evaluate_stability_with_progress(
    source: &[TrainingRow],
    config: StabilityConfig,
    mut on_progress: impl FnMut(ProgressEvent),
) -> StabilityEvaluation {
    let config_snapshot = config.snapshot();
    let stage = config_snapshot.stage;
    let candidate_count = config.promoted_candidates.as_ref().map_or(15, Vec::len);
    let bootstrap_total = config.bootstrap_iterations.saturating_mul(candidate_count);
    let total = bootstrap_total.saturating_add(15);
    let started = std::time::Instant::now();
    on_progress(aggregate_progress_event(stage, 0, total, 0));
    let source = eligible_performance_rows(source);
    let (mut reports, mut worker_errors) = std::thread::scope(|scope| {
        let source = &source;
        let (progress_tx, progress_rx) = mpsc::channel::<usize>();
        let handles = [Role::Tank, Role::Damage, Role::Support].map(|role| {
            let config = &config;
            let progress_tx = progress_tx.clone();
            scope.spawn(move || {
                let role_rows_all = source
                    .iter()
                    .filter(|r| r.role == role_name(role))
                    .cloned()
                    .collect::<Vec<_>>();
                let role_core_rows = role_rows_all.len();
                let mut reports = Vec::new();
                let candidates = config
                    .promoted_candidates
                    .as_ref()
                    .map(|all| {
                        all.iter()
                            .filter(|candidate| candidate.role == role)
                            .filter_map(|candidate| {
                                let strategy = parse_strategy_checked(&candidate.strategy)?;
                                Some((strategy, Some(candidate.selection())))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| {
                        [
                            HealingTakenStrategy::None,
                            HealingTakenStrategy::DirectNegative,
                            HealingTakenStrategy::HeroNormalized,
                            HealingTakenStrategy::ExcessHealing,
                            HealingTakenStrategy::ResourceConversionDebt,
                        ]
                        .into_iter()
                        .map(|strategy| (strategy, None))
                        .collect()
                    });
                for (strategy, fixed) in candidates {
                    let forwards = VALIDATION_MODES
                        .into_iter()
                        .map(|mode| {
                            (
                                mode,
                                fixed.map_or_else(
                                    || evaluate_cell_strategy(&role_rows_all, role, mode, strategy),
                                    |selected| {
                                        evaluate_cell_strategy_fixed(
                                            &role_rows_all,
                                            role,
                                            mode,
                                            strategy,
                                            selected,
                                        )
                                    },
                                ),
                            )
                        })
                        .collect::<Vec<_>>();
                    let selected = &forwards[0].1;
                    debug_assert!(forwards.iter().all(|(_, forward)| {
                        forward.variant == selected.variant
                            && forward.final_multiplier == selected.final_multiplier
                            && forward.lambda_core == selected.lambda_core
                            && forward.lambda_cohort == selected.lambda_cohort
                    }));
                    let mut bootstraps = bootstrap_all_modes(
                        &role_rows_all,
                        role,
                        strategy,
                        selected.variant,
                        selected.final_multiplier,
                        selected.lambda_core,
                        selected.lambda_cohort,
                        config,
                        Some(&progress_tx),
                    );
                    for (mode, forward) in forwards {
                        let cohort = mode.training_cohort();
                        let season = loo_season(&role_rows_all, role, mode, strategy, fixed);
                        let hero = loo_hero(&role_rows_all, role, mode, strategy, fixed);
                        let account = loo_account(&role_rows_all, role, mode, strategy, fixed);
                        let bootstrap = bootstraps.remove(&mode).unwrap_or_default();
                        let mut report = StabilityReport {
                            role,
                            mode,
                            training_cohort: cohort,
                            shared_parameter_version: shared_parameter_version(cohort),
                            strategy: forward.strategy.clone(),
                            forward,
                            leave_one_season_out: season,
                            leave_one_hero_out: hero,
                            leave_one_account_out: account,
                            bootstrap,
                            eligible: false,
                            decision: CellDecision::Shadow,
                            rejected_reasons: vec![],
                            performance_adjustment: 0.0,
                            max_loo_worsened_fraction: config.max_loo_worsened_fraction,
                            max_abs_average_adjustment: config.max_abs_average_adjustment,
                        };
                        report.apply_gates();
                        if role_core_rows < 500 {
                            push_once(&mut report.rejected_reasons, "role_core_rows_below_500");
                            report.eligible = false;
                            report.decision = CellDecision::Shadow;
                        }
                        if report.leave_one_account_out.coverage_limited_groups > 0 {
                            push_once(&mut report.rejected_reasons, "account_coverage_limited");
                            report.eligible = false;
                            report.decision = CellDecision::Shadow;
                        }
                        reports.push(report);
                    }
                }
                reports
            })
        });
        drop(progress_tx);
        let mut raw_completed = 0usize;
        let mut emitted_completed = 0usize;
        while let Ok(batch) = progress_rx.recv() {
            raw_completed = raw_completed.saturating_add(batch);
            let completed = raw_completed.min(bootstrap_total);
            if completed > emitted_completed {
                emitted_completed = completed;
                let elapsed_ms = started.elapsed().as_millis();
                on_progress(aggregate_progress_event(
                    stage, completed, total, elapsed_ms,
                ));
            }
        }
        merge_worker_results(
            handles
                .into_iter()
                .zip([Role::Tank, Role::Damage, Role::Support])
                .map(|(handle, role)| (role, handle.join().ok()))
                .collect(),
        )
    });
    reports.sort_by_key(|report| (report.role, report.mode, strategy_order(&report.strategy)));
    let nested_started = std::time::Instant::now();
    let mut cells = Vec::new();
    if worker_errors.is_empty() {
        let (completed_cells, failed) = run_nested_sequence(
            |role, mode| {
                Ok(evaluate_nested_cell_report(
                    &source,
                    &reports,
                    role,
                    mode,
                    config.promoted_candidates.as_deref(),
                ))
            },
            |completed| {
                on_progress(nested_progress_event(
                    stage,
                    bootstrap_total,
                    completed,
                    total,
                    started.elapsed().as_millis(),
                    nested_started.elapsed().as_millis(),
                ));
            },
        );
        cells = completed_cells;
        if failed {
            worker_errors.push("nested_cell_worker_failed".into());
        }
    }
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
                // Selection is intentionally cell-local; this legacy role-level
                // summary must never substitute one actual mode's result for another.
                selected: None,
                reports: items,
            }
        })
        .collect();
    let evaluation = StabilityEvaluation {
        config: config_snapshot,
        reports,
        roles,
        cells,
        worker_errors,
    };
    evaluation
}

fn run_nested_sequence<T>(
    mut evaluator: impl FnMut(Role, ModeFamily) -> Result<T, ()>,
    mut on_complete: impl FnMut(usize),
) -> (Vec<T>, bool) {
    let mut completed = Vec::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        for mode in VALIDATION_MODES {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| evaluator(role, mode)));
            match result {
                Ok(Ok(value)) => {
                    completed.push(value);
                    on_complete(completed.len());
                }
                Ok(Err(())) | Err(_) => return (completed, true),
            }
        }
    }
    (completed, false)
}

fn nested_progress_event(
    stage: TrainingStage,
    bootstrap_total: usize,
    completed_cells: usize,
    total: usize,
    elapsed_ms: u128,
    nested_elapsed_ms: u128,
) -> ProgressEvent {
    let remaining_cells = 15usize.saturating_sub(completed_cells);
    let eta_ms = if remaining_cells == 0 {
        Some(0)
    } else {
        Some(
            nested_elapsed_ms
                .max(1)
                .saturating_mul(remaining_cells as u128)
                / completed_cells.max(1) as u128,
        )
        .map(|eta| eta.max(1))
    };
    ProgressEvent {
        stage,
        completed: bootstrap_total + completed_cells,
        total,
        elapsed_ms,
        eta_ms,
    }
}

fn evaluate_nested_cell_report(
    source: &[TrainingRow],
    reports: &[StabilityReport],
    role: Role,
    mode: ModeFamily,
    promoted: Option<&[PromotedCandidate]>,
) -> CellEvaluation {
    let items = reports
        .iter()
        .filter(|report| report.role == role && report.mode == mode)
        .cloned()
        .collect::<Vec<_>>();
    if promoted.is_some() && items.is_empty() {
        return CellEvaluation {
            role,
            mode,
            training_cohort: mode.training_cohort(),
            shared_parameter_version: shared_parameter_version(mode.training_cohort()),
            selected: None,
            decision: CellDecision::Shadow,
            rejected_reasons: vec!["no_promoted_candidate".into()],
            nested_validation: NestedCellReport {
                role,
                mode,
                folds: vec![],
                improved_folds: 0,
                metrics: None,
                baseline_metrics: None,
                selection_degraded: true,
                selected_strategy: "none".into(),
                selected_variant: ModelVariant::Core,
                selected_final_multiplier: 1.25,
                selected_lambda_core: 0.02,
                selected_lambda_cohort: 0.05,
            },
            reports: vec![],
        };
    }
    let role_core_rows = source
        .iter()
        .filter(|row| row.role == role_name(role))
        .count();
    let fixed = promoted.and_then(|_| {
        items
            .iter()
            .min_by(|left, right| {
                left.forward
                    .metrics
                    .as_ref()
                    .map(|metrics| metrics.brier)
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(
                        &right
                            .forward
                            .metrics
                            .as_ref()
                            .map(|metrics| metrics.brier)
                            .unwrap_or(f64::INFINITY),
                    )
                    .then(left.strategy.cmp(&right.strategy))
            })
            .and_then(|report| {
                Some((
                    parse_strategy_checked(&report.strategy)?,
                    CandidateSelection {
                        variant: report.forward.variant,
                        multiplier: report.forward.final_multiplier,
                        lambda_core: report.forward.lambda_core,
                        lambda_cohort: report.forward.lambda_cohort,
                        degraded: false,
                    },
                ))
            })
    });
    let nested_validation = if let Some((strategy, selected)) = fixed {
        evaluate_nested_cell_fixed(source, role, mode, strategy, selected)
    } else if role_core_rows < 500 {
        evaluate_nested_cell_shadow(source, role, mode)
    } else {
        evaluate_nested_cell(source, role, mode)
    };
    let mut selected = items
        .iter()
        .find(|report| {
            report.eligible
                && report.strategy == nested_validation.selected_strategy
                && report.forward.variant == nested_validation.selected_variant
                && report.forward.final_multiplier == nested_validation.selected_final_multiplier
                && report.forward.lambda_core == nested_validation.selected_lambda_core
                && report.forward.lambda_cohort == nested_validation.selected_lambda_cohort
        })
        .map(|report| report.strategy.clone());
    let nested_passes = nested_validation.improved_folds >= 4
        && nested_validation
            .metrics
            .as_ref()
            .zip(nested_validation.baseline_metrics.as_ref())
            .is_none_or(|(candidate, baseline)| candidate.ece <= baseline.ece + 0.005);
    if !nested_passes {
        selected = None;
    }
    let decision = if selected.is_some() {
        CellDecision::Eligible
    } else {
        CellDecision::Shadow
    };
    let mut rejected_reasons = if let Some(selected_strategy) = selected.as_deref() {
        items
            .iter()
            .find(|report| report.strategy == selected_strategy && report.eligible)
            .map(|report| report.rejected_reasons.clone())
            .unwrap_or_default()
    } else {
        items
            .iter()
            .flat_map(|report| report.rejected_reasons.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    };
    if rejected_reasons.is_empty() && selected.is_none() {
        rejected_reasons.push("no_eligible_strategy".into());
    }
    if nested_validation.improved_folds < 4 {
        push_once(&mut rejected_reasons, "improved_folds_below_4");
    }
    if nested_validation
        .metrics
        .as_ref()
        .zip(nested_validation.baseline_metrics.as_ref())
        .is_some_and(|(candidate, baseline)| candidate.ece > baseline.ece + 0.005)
    {
        push_once(&mut rejected_reasons, "ece_worse_than_0_005");
    }
    CellEvaluation {
        role,
        mode,
        training_cohort: mode.training_cohort(),
        shared_parameter_version: shared_parameter_version(mode.training_cohort()),
        selected,
        decision,
        rejected_reasons,
        nested_validation,
        reports: items,
    }
}

fn shared_parameter_version(cohort: TrainingCohort) -> String {
    match cohort {
        TrainingCohort::RoleQueue => "role_queue-v1",
        TrainingCohort::OpenQueue => "open_queue-v1",
        TrainingCohort::StadiumCompetitive => "stadium_competitive-v1",
        TrainingCohort::OutcomeOnly => "outcome_only-v1",
    }
    .into()
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

fn loo_season(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    strategy: HealingTakenStrategy,
    fixed: Option<CandidateSelection>,
) -> LooReport {
    let groups = source
        .iter()
        .filter(|r| r.mode_family == mode)
        .map(|r| r.season)
        .collect::<BTreeSet<_>>();
    let evals = groups
        .into_iter()
        .map(|held| {
            let (train, valid) = season_loo_split(source, held, mode);
            let candidate = valid.len();
            let (delta, evaluated) = validate_split(&train, &valid, role, mode, strategy, fixed);
            (delta, candidate, evaluated)
        })
        .collect::<Vec<_>>();
    loo_from_evals(evals)
}

fn season_loo_split(
    source: &[TrainingRow],
    held: i64,
    mode: ModeFamily,
) -> (Vec<TrainingRow>, Vec<TrainingRow>) {
    (
        source
            .iter()
            .filter(|row| row.season != held)
            .cloned()
            .collect(),
        source
            .iter()
            .filter(|row| row.season == held && row.mode_family == mode)
            .cloned()
            .collect(),
    )
}

fn loo_hero(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    strategy: HealingTakenStrategy,
    fixed: Option<CandidateSelection>,
) -> LooReport {
    let groups = source
        .iter()
        .filter(|r| r.mode_family == mode)
        .map(|r| r.hero_group.clone())
        .collect::<BTreeSet<_>>();
    let evals = groups
        .into_iter()
        .map(|held| {
            let (train, valid) = hero_loo_split(source, &held, mode);
            let candidate = valid.len();
            let (delta, evaluated) = validate_split(&train, &valid, role, mode, strategy, fixed);
            (delta, candidate, evaluated)
        })
        .collect::<Vec<_>>();
    loo_from_evals(evals)
}

fn hero_loo_split(
    source: &[TrainingRow],
    held: &Option<String>,
    mode: ModeFamily,
) -> (Vec<TrainingRow>, Vec<TrainingRow>) {
    // The target cell controls validation only. Because the role core and cohort
    // offsets train on every actual mode, every match containing the held hero in
    // any mode must leave the training pool as one indivisible group.
    let held_matches = source
        .iter()
        .filter(|row| &row.hero_group == held)
        .map(|row| row.match_group.as_str())
        .collect::<BTreeSet<_>>();
    let train = source
        .iter()
        .filter(|row| !held_matches.contains(row.match_group.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let valid = source
        .iter()
        .filter(|row| {
            held_matches.contains(row.match_group.as_str())
                && &row.hero_group == held
                && row.mode_family == mode
        })
        .cloned()
        .collect::<Vec<_>>();
    (train, valid)
}

fn loo_account(
    source: &[TrainingRow],
    role: Role,
    mode: ModeFamily,
    strategy: HealingTakenStrategy,
    fixed: Option<CandidateSelection>,
) -> AccountLooReport {
    let groups = source
        .iter()
        .filter(|row| row.mode_family == mode)
        .map(|row| row.account_group.clone())
        .collect::<BTreeSet<_>>();
    let candidate_groups = groups.len();
    let mut deltas = Vec::new();
    let mut coverage_limited_groups = 0;
    for held in groups {
        let (train, valid) = account_loo_split(source, &held, mode);
        let (delta, evaluated) = validate_split(&train, &valid, role, mode, strategy, fixed);
        if delta.is_none() || evaluated < valid.len() {
            coverage_limited_groups += 1;
        }
        if let Some(delta) = delta {
            deltas.push(delta);
        }
    }
    AccountLooReport {
        candidate_groups,
        evaluated_groups: deltas.len(),
        coverage_limited_groups,
        mean_brier_delta: mean(&deltas),
        worst_brier_delta: deltas.iter().copied().max_by(f64::total_cmp),
    }
}

fn account_loo_split(
    source: &[TrainingRow],
    held: &str,
    mode: ModeFamily,
) -> (Vec<TrainingRow>, Vec<TrainingRow>) {
    let held_matches = source
        .iter()
        .filter(|row| row.account_group == held)
        .map(|row| row.match_group.as_str())
        .collect::<BTreeSet<_>>();
    (
        source
            .iter()
            .filter(|row| !held_matches.contains(row.match_group.as_str()))
            .cloned()
            .collect(),
        source
            .iter()
            .filter(|row| row.account_group == held && row.mode_family == mode)
            .cloned()
            .collect(),
    )
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
    mode: ModeFamily,
    strategy: HealingTakenStrategy,
    fixed: Option<CandidateSelection>,
) -> (Option<f64>, usize) {
    if valid.is_empty() {
        return (None, 0);
    }
    let matrix = build_feature_matrix(train, valid, strategy);
    let Some(tr) = matrix.training_by_role.get(role_name(role)) else {
        return (None, 0);
    };
    let Some(all_va) = matrix.validation_by_role.get(role_name(role)) else {
        return (None, 0);
    };
    let va = all_va
        .iter()
        .filter(|row| row.mode_family == mode)
        .collect::<Vec<_>>();
    if tr.is_empty() || va.is_empty() {
        return (None, va.len());
    }
    let selected = fixed.unwrap_or_else(|| select_candidate_detailed(train, role, strategy));
    let Some(model) = fit_split_model(tr, role, strategy, selected) else {
        return (None, va.len());
    };
    let actual = va.iter().map(|r| r.result).collect::<Vec<_>>();
    let predicted = va
        .iter()
        .map(|r| predict_probability(&model, r))
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

fn fit_split_model(
    training: &[super::performance::MatrixRow],
    role: Role,
    strategy: HealingTakenStrategy,
    selected: CandidateSelection,
) -> Option<HierarchicalModel> {
    let refs = training.iter().collect::<Vec<_>>();
    fit_hierarchical_model_regularized(
        &refs,
        role,
        selected.variant,
        strategy,
        selected.multiplier,
        selected.lambda_core,
        selected.lambda_cohort,
    )
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

fn bootstrap_all_modes(
    source: &[TrainingRow],
    role: Role,
    strategy: HealingTakenStrategy,
    variant: ModelVariant,
    multiplier: f64,
    lambda_core: f64,
    lambda_cohort: f64,
    config: &StabilityConfig,
    progress: Option<&mpsc::Sender<usize>>,
) -> BTreeMap<ModeFamily, BootstrapReport> {
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
    let Some(full_model) = fit_hierarchical_model_regularized(
        &full_refs,
        role,
        variant,
        strategy,
        multiplier,
        lambda_core,
        lambda_cohort,
    ) else {
        return VALIDATION_MODES
            .into_iter()
            .map(|mode| {
                (
                    mode,
                    BootstrapReport {
                        requested_iterations: config.bootstrap_iterations,
                        skipped_iterations: config.bootstrap_iterations,
                        fixed_variant: Some(variant),
                        fixed_final_multiplier: Some(multiplier),
                        fixed_lambda_core: Some(lambda_core),
                        fixed_lambda_cohort: Some(lambda_cohort),
                        ..Default::default()
                    },
                )
            })
            .collect();
    };
    let names = full_model.feature_names.clone();
    let mut samples = vec![Vec::new(); names.len()];
    let mut deltas = VALIDATION_MODES
        .into_iter()
        .map(|mode| (mode, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let mut aucs = deltas.clone();
    let mut eces = deltas.clone();
    let mut skipped = 0;
    let mut hierarchical_offset_iterations = 0;
    for iteration in 0..config.bootstrap_iterations {
        let progress_batch =
            if (iteration + 1) % 10 == 0 || iteration + 1 == config.bootstrap_iterations {
                let previous = iteration / 10 * 10;
                iteration + 1 - previous
            } else {
                0
            };
        let _progress_after_iteration = BootstrapProgressGuard {
            sender: progress,
            completed: progress_batch,
        };
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
        let all_va = matrix
            .validation_by_role
            .get(role_name(role))
            .cloned()
            .unwrap_or_default();
        if tr.is_empty() || all_va.is_empty() {
            skipped += 1;
            continue;
        }
        let refs = tr.iter().collect::<Vec<_>>();
        // Each resample rebuilds fold-local baselines and refits the exact Task 5
        // hierarchical logit model with fixed selected hyperparameters.
        let Some(model) = fit_resample_model(
            &refs,
            role,
            variant,
            strategy,
            multiplier,
            lambda_core,
            lambda_cohort,
        ) else {
            skipped += 1;
            continue;
        };
        if model.feature_names != names {
            skipped += 1;
            continue;
        }
        if !model.cohort_offsets.is_empty() {
            hierarchical_offset_iterations += 1;
        }
        for mode in VALIDATION_MODES {
            let va = all_va
                .iter()
                .filter(|row| row.mode_family == mode)
                .collect::<Vec<_>>();
            if va.is_empty() {
                continue;
            }
            let actual = va.iter().map(|r| r.result).collect::<Vec<_>>();
            let predicted = va
                .iter()
                .map(|r| predict_probability(&model, r))
                .collect::<Vec<_>>();
            let Some(m) = metrics(&actual, &predicted) else {
                continue;
            };
            let base = va
                .iter()
                .map(|r| (r.result - r.expected_win_rate).powi(2))
                .sum::<f64>()
                / va.len() as f64;
            deltas.get_mut(&mode).unwrap().push(m.brier - base);
            aucs.get_mut(&mode).unwrap().push(m.auc);
            eces.get_mut(&mode).unwrap().push(m.ece);
        }
        for (i, v) in model.core_weights.into_iter().enumerate() {
            samples[i].push(v)
        }
    }
    let coefficients: Vec<CoefficientStability> = names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let mut s = samples[i].clone();
            let reference = full_model.core_weights.get(i).copied().unwrap_or(0.0);
            let expected_positive = matches!(name.as_str(), "final" | "nonfinal")
                || (!matches!(name.as_str(), "death" | "debt") && reference > 0.0);
            let stable = s
                .iter()
                .filter(|&&v| if expected_positive { v > 0.0 } else { v < 0.0 })
                .count();
            let critical_constraint =
                matches!(name.as_str(), "final" | "nonfinal" | "death" | "debt");
            CoefficientStability {
                weight: name,
                scope: "core".into(),
                direction_stability: stable as f64 / s.len().max(1) as f64,
                ci95_low: quantile(&mut s, 0.025).unwrap_or(0.0),
                ci95_high: quantile(&mut s, 0.975).unwrap_or(0.0),
                critical_constraint,
                reference_nonzero: reference.abs() > 1e-12,
            }
        })
        .collect();
    VALIDATION_MODES
        .into_iter()
        .map(|mode| {
            let mut mode_deltas = deltas.remove(&mode).unwrap_or_default();
            let mut low_samples = mode_deltas.clone();
            let mode_aucs = aucs.remove(&mode).unwrap_or_default();
            let mode_eces = eces.remove(&mode).unwrap_or_default();
            let completed_iterations = mode_deltas.len();
            (
                mode,
                BootstrapReport {
                    requested_iterations: config.bootstrap_iterations,
                    completed_iterations,
                    skipped_iterations: config
                        .bootstrap_iterations
                        .saturating_sub(completed_iterations)
                        .max(skipped),
                    fixed_variant: Some(variant),
                    fixed_final_multiplier: Some(multiplier),
                    fixed_lambda_core: Some(lambda_core),
                    fixed_lambda_cohort: Some(lambda_cohort),
                    hierarchical_offset_iterations,
                    coefficients: coefficients.clone(),
                    mean_brier_delta: mean(&mode_deltas),
                    brier_delta_ci95_low: quantile(&mut low_samples, 0.025),
                    brier_delta_ci95_high: quantile(&mut mode_deltas, 0.975),
                    mean_auc: mean(&mode_aucs),
                    mean_ece: mean(&mode_eces),
                },
            )
        })
        .collect()
}

struct BootstrapProgressGuard<'a> {
    sender: Option<&'a mpsc::Sender<usize>>,
    completed: usize,
}

impl Drop for BootstrapProgressGuard<'_> {
    fn drop(&mut self) {
        if self.completed > 0 {
            let _ = self.sender.map(|sender| sender.send(self.completed));
        }
    }
}

fn fit_resample_model(
    rows: &[&super::performance::MatrixRow],
    role: Role,
    variant: ModelVariant,
    strategy: HealingTakenStrategy,
    multiplier: f64,
    lambda_core: f64,
    lambda_cohort: f64,
) -> Option<HierarchicalModel> {
    fit_hierarchical_model_regularized(
        rows,
        role,
        variant,
        strategy,
        multiplier,
        lambda_core,
        lambda_cohort,
    )
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
fn parse_strategy_checked(s: &str) -> Option<HealingTakenStrategy> {
    match s {
        "none" => Some(HealingTakenStrategy::None),
        "directnegative" => Some(HealingTakenStrategy::DirectNegative),
        "heronormalized" => Some(HealingTakenStrategy::HeroNormalized),
        "excesshealing" => Some(HealingTakenStrategy::ExcessHealing),
        "resourceconversiondebt" => Some(HealingTakenStrategy::ResourceConversionDebt),
        _ => None,
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

#[cfg(test)]
mod hierarchical_bootstrap_tests {
    use super::*;
    use crate::career_mmr::{
        input::ModeFamily,
        training::{TrainingEnvironment, TrainingRow},
    };
    use std::cell::RefCell;

    fn row(index: usize, mode_family: ModeFamily) -> TrainingRow {
        let positive = index % 2 == 0;
        let opposite = mode_family == ModeFamily::StadiumCompetitive;
        let strong = if positive != opposite { 20.0 } else { 1.0 };
        TrainingRow {
            account_group: "synthetic-account".into(),
            source_match_group: format!("synthetic-source-{index}"),
            match_group: format!("synthetic-match-{index}"),
            season: 20 + (index % 3) as i64,
            begin_ts: index as i64,
            role: "damage".into(),
            mode_family,
            environment: if mode_family == ModeFamily::StadiumCompetitive {
                TrainingEnvironment::Stadium
            } else {
                TrainingEnvironment::Ordinary
            },
            hero_group: Some("synthetic-hero".into()),
            outcome_only: false,
            allow_performance: true,
            allow_performance_by_hero: true,
            result: f64::from(positive),
            expected_win_rate: 0.5,
            data_quality: 1.0,
            base_delta: Some(10.0),
            final_hit: Some(strong),
            kill: Some(strong + 2.0),
            assist: Some(2.0),
            death: Some(if strong > 1.0 { 1.0 } else { 5.0 }),
            damage: Some(strong * 100.0),
            healing: Some(0.0),
            mitigation: Some(0.0),
            healing_taken: Some(100.0),
            available_team_healing: Some(500.0),
            damage_taken: Some(200.0),
            objective_time_sec: Some(10.0),
            duration_sec: 600.0,
            hero_duration_sec: Some(600.0),
            healing_taken_semantic: "external",
            usable_for_resource_debt: true,
        }
    }

    #[test]
    fn bootstrap_refits_hierarchical_offsets_with_the_selected_multiplier() {
        let rows = (0..160)
            .map(|i| row(i, ModeFamily::RoleCompetitive))
            .chain((160..320).map(|i| row(i, ModeFamily::StadiumCompetitive)))
            .collect::<Vec<_>>();
        let report = bootstrap_all_modes(
            &rows,
            Role::Damage,
            HealingTakenStrategy::None,
            ModelVariant::RoleFull,
            2.0,
            0.02,
            0.05,
            &StabilityConfig {
                bootstrap_iterations: 3,
                seed: 17,
                ..Default::default()
            },
            None,
        )
        .remove(&ModeFamily::RoleCompetitive)
        .unwrap();
        assert_eq!(report.fixed_final_multiplier, Some(2.0));
        assert!(report.hierarchical_offset_iterations > 0);
        assert!(report
            .coefficients
            .iter()
            .all(|coefficient| coefficient.scope == "core"));
        assert!(report
            .coefficients
            .iter()
            .any(|coefficient| coefficient.weight == "damage_conversion"));
    }

    #[test]
    fn loo_split_model_keeps_cohort_offsets_in_validation_predictions() {
        let training = (0..160)
            .map(|i| row(i, ModeFamily::RoleCompetitive))
            .chain((160..320).map(|i| row(i, ModeFamily::StadiumCompetitive)))
            .collect::<Vec<_>>();
        let matrix = build_feature_matrix(&training, &[], HealingTakenStrategy::None);
        let rows = matrix.training_by_role.get("damage").unwrap();
        let model = fit_split_model(
            rows,
            Role::Damage,
            HealingTakenStrategy::None,
            CandidateSelection {
                variant: ModelVariant::Core,
                multiplier: 1.5,
                lambda_core: 0.02,
                lambda_cohort: 0.05,
                degraded: false,
            },
        )
        .unwrap();
        let role_row = rows
            .iter()
            .find(|row| row.mode_family == ModeFamily::RoleCompetitive)
            .unwrap()
            .clone();
        let mut stadium_row = role_row.clone();
        stadium_row.mode_family = ModeFamily::StadiumCompetitive;
        assert_ne!(
            predict_probability(&model, &role_row),
            predict_probability(&model, &stadium_row)
        );
    }

    #[test]
    fn hero_loo_excludes_held_hero_matches_from_every_actual_mode() {
        let mut source = (0..180)
            .map(|i| {
                let mut row = row(i, ModeFamily::RoleCompetitive);
                row.hero_group = Some("training-hero".into());
                row
            })
            .collect::<Vec<_>>();
        let mut held_comp = row(1_000, ModeFamily::RoleCompetitive);
        held_comp.hero_group = Some("held-hero".into());
        held_comp.match_group = "held-comp".into();
        let mut held_quick = row(1_001, ModeFamily::RoleQuick);
        held_quick.hero_group = Some("held-hero".into());
        held_quick.match_group = "held-quick".into();
        source.extend([held_comp, held_quick]);

        let held = Some("held-hero".into());
        let (train_before, valid_before) =
            hero_loo_split(&source, &held, ModeFamily::RoleCompetitive);
        let mut contaminated = source;
        let quick = contaminated
            .iter_mut()
            .find(|row| row.match_group == "held-quick")
            .unwrap();
        quick.final_hit = Some(1.0e15);
        quick.damage = Some(1.0e18);
        quick.healing_taken = Some(1.0e18);
        let (train_after, valid_after) =
            hero_loo_split(&contaminated, &held, ModeFamily::RoleCompetitive);

        assert_eq!(train_before, train_after);
        assert_eq!(valid_before, valid_after);
        assert!(train_after
            .iter()
            .all(|row| row.hero_group.as_deref() != Some("held-hero")));
        assert!(valid_after
            .iter()
            .all(|row| row.mode_family == ModeFamily::RoleCompetitive));
        assert_eq!(
            validate_split(
                &train_before,
                &valid_before,
                Role::Damage,
                ModeFamily::RoleCompetitive,
                HealingTakenStrategy::None,
                None,
            ),
            validate_split(
                &train_after,
                &valid_after,
                Role::Damage,
                ModeFamily::RoleCompetitive,
                HealingTakenStrategy::None,
                None,
            )
        );

        let held_season = contaminated
            .iter()
            .find(|row| row.match_group == "held-comp")
            .unwrap()
            .season;
        let (season_train, season_valid) =
            season_loo_split(&contaminated, held_season, ModeFamily::RoleCompetitive);
        assert!(season_train.iter().all(|row| row.season != held_season));
        assert!(season_valid
            .iter()
            .all(|row| row.mode_family == ModeFamily::RoleCompetitive));

        let (account_train, account_valid) = account_loo_split(
            &contaminated,
            "synthetic-account",
            ModeFamily::RoleCompetitive,
        );
        assert!(account_train.is_empty());
        assert!(account_valid
            .iter()
            .all(|row| row.mode_family == ModeFamily::RoleCompetitive));
    }

    #[test]
    fn account_loo_excludes_complete_matches_seen_by_the_held_account() {
        let mut held = row(2_000, ModeFamily::RoleCompetitive);
        held.account_group = "held-account".into();
        held.match_group = "shared-match".into();
        let mut duplicate = row(2_001, ModeFamily::RoleQuick);
        duplicate.account_group = "other-account".into();
        duplicate.match_group = "shared-match".into();
        duplicate.final_hit = Some(1.0e15);
        let mut ordinary = row(2_002, ModeFamily::RoleQuick);
        ordinary.account_group = "other-account".into();
        ordinary.match_group = "ordinary-match".into();
        let (train, valid) = account_loo_split(
            &[held, duplicate, ordinary],
            "held-account",
            ModeFamily::RoleCompetitive,
        );
        assert!(train.iter().all(|row| row.match_group != "shared-match"));
        assert_eq!(valid.len(), 1);
        assert_eq!(valid[0].account_group, "held-account");
        assert_eq!(valid[0].mode_family, ModeFamily::RoleCompetitive);
    }

    #[test]
    fn nested_progress_is_emitted_before_the_next_cell_starts_and_finishes_once() {
        let events = RefCell::new(Vec::<String>::new());
        let mut executions = 0usize;
        let (values, failed) = run_nested_sequence(
            |_, _| {
                executions += 1;
                if executions > 1 {
                    assert_eq!(
                        events.borrow().last().cloned(),
                        Some(format!("progress-{}", executions - 1))
                    );
                }
                events.borrow_mut().push(format!("execute-{executions}"));
                Ok(executions)
            },
            |completed| events.borrow_mut().push(format!("progress-{completed}")),
        );
        assert!(!failed);
        assert_eq!(values.len(), 15);
        assert_eq!(
            events
                .borrow()
                .iter()
                .filter(|event| *event == "progress-15")
                .count(),
            1
        );
    }

    #[test]
    fn nested_failure_stops_without_emitting_completion_for_the_failed_cell() {
        let progress = RefCell::new(Vec::new());
        let mut executions = 0usize;
        let (values, failed) = run_nested_sequence(
            |_, _| {
                executions += 1;
                if executions == 3 {
                    Err(())
                } else {
                    Ok(executions)
                }
            },
            |completed| progress.borrow_mut().push(completed),
        );
        assert!(failed);
        assert_eq!(values, vec![1, 2]);
        assert_eq!(*progress.borrow(), vec![1, 2]);
    }

    #[test]
    fn bootstrap_sample_fit_is_identical_to_the_formal_estimator() {
        let training = (0..160)
            .map(|i| row(i, ModeFamily::RoleCompetitive))
            .chain((160..320).map(|i| row(i, ModeFamily::StadiumCompetitive)))
            .collect::<Vec<_>>();
        let matrix = build_feature_matrix(&training, &[], HealingTakenStrategy::None);
        let rows = matrix.training_by_role.get("damage").unwrap();
        let refs = rows.iter().collect::<Vec<_>>();
        let formal = fit_hierarchical_model_regularized(
            &refs,
            Role::Damage,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None,
            2.0,
            0.02,
            0.05,
        )
        .unwrap();
        let bootstrap = fit_resample_model(
            &refs,
            Role::Damage,
            ModelVariant::RoleFull,
            HealingTakenStrategy::None,
            2.0,
            0.02,
            0.05,
        )
        .unwrap();
        assert_eq!(bootstrap, formal);
        assert_eq!(
            predict_probability(&bootstrap, &rows[0]),
            predict_probability(&formal, &rows[0])
        );
    }
}
