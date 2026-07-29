//! Guaranteed per-role personal-performance model selection.
//!
//! Selects between three tiers per role, in priority order:
//!   1. [`PerformanceTier::Learned`]: reuse the fully trained artifact when the
//!      balanced tier-one gates pass.
//!   2. [`PerformanceTier::Shrunk`]: keep only directionally stable features
//!      with valid business signs, then binary-search a single shrink factor.
//!   3. [`PerformanceTier::Percentile`]: robust within-role percentile baseline
//!      using only the four core features.
//!
//! Every non-empty dataset must land on exactly one tier, satisfying the launch
//! mandate. Empty role datasets return a Chinese error instead of fabricating
//! an artifact. Runtime invariants (finite output, cap, sign preservation)
//! are enforced by [`super::performance_runtime::invariant_checked_correction`].
use super::input::{ModeFamily, PerformanceTier, Role, TrainingCohort};
use super::performance_runtime::{
    invariant_checked_correction, signed_feature_weight, NormalizedPerformanceRow,
    SHRUNK_MAX_BRIER_DEGRADATION, SHRUNK_MAX_CAP_HIT_RATE, SHRUNK_MAX_MEAN_ABS_ADJUSTMENT,
    SHRUNK_MIN_DIRECTION_STABILITY,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Aggregated tier-one evidence used to decide whether a role can use the
/// learned model. This consumed the existing StabilityReport+bootstrap fields;
/// see design section 4.1 for the balanced gate threshold values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierOneEvidence {
    pub role: Role,
    pub eligible: bool,
    pub direction_stability: f64,
    pub brier_delta_ci95_high: Option<f64>,
    pub improved_folds: usize,
    pub mean_abs_adjustment: f64,
    pub cap_hit_rate: f64,
    pub factor_directions: BTreeMap<String, f64>,
}

impl TierOneEvidence {
    pub fn eligible(role: Role) -> Self {
        Self {
            role,
            eligible: true,
            direction_stability: 0.90,
            brier_delta_ci95_high: Some(-0.01),
            improved_folds: 4,
            mean_abs_adjustment: 0.05,
            cap_hit_rate: 0.02,
            factor_directions: BTreeMap::new(),
        }
    }
    pub fn failed(role: Role) -> Self {
        Self {
            role,
            eligible: false,
            direction_stability: 0.50,
            brier_delta_ci95_high: Some(0.05),
            improved_folds: 2,
            mean_abs_adjustment: 0.20,
            cap_hit_rate: 0.15,
            factor_directions: BTreeMap::new(),
        }
    }
}

/// A scaled, normalized row record used to fit the shrunk and percentile tiers
/// when no artifact is available. Task 5 supplies real normalized rows from the
/// upstream pipeline; the selection tests construct synthetic rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceRecord {
    pub role: Role,
    pub cohort: TrainingCohort,
    pub feature: String,
    pub value: f64,
    pub outcome: f64,
}

/// Final model artifact selected for a role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectedRoleModel {
    pub role: Role,
    pub tier: PerformanceTier,
    pub weights: BTreeMap<String, f64>,
    pub base_offset: f64,
    pub shrink: f64,
    pub training_cohort: TrainingCohort,
    pub config_hash: String,
}

impl SelectedRoleModel {
    pub fn tier(&self) -> PerformanceTier {
        self.tier
    }

    pub fn cap_for(&self, mode: ModeFamily) -> f64 {
        mode.runtime_correction_cap(self.tier)
    }

    pub fn evaluate(&self, row: &NormalizedPerformanceRow) -> f64 {
        let mut acc = self.base_offset;
        let feature_pairs = [
            ("final", row.final_hit),
            ("final_hit", row.final_hit),
            ("nonfinal", row.non_final_eliminations),
            ("assist", row.assist),
            ("death", row.death),
        ];
        for (name, value) in feature_pairs {
            if let Some(w) = self.weights.get(name) {
                acc += w * value;
            }
        }
        acc
    }

    pub fn evaluate_sign_safe(&self, row: &NormalizedPerformanceRow, base_delta: f64) -> f64 {
        invariant_checked_correction(self, row, base_delta)
    }
}

/// Evidence used to certify a learned artifact. Constructed from the existing
/// training report / stability evaluation by Task 4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnedArtifactSummary {
    pub role: Role,
    pub eligible: bool,
    pub core_weights: BTreeMap<String, f64>,
    pub cohort_offset: BTreeMap<String, f64>,
    pub base_offset: f64,
}

/// Result of selecting the final role model.
pub type SelectionResult = Result<SelectedRoleModel, String>;

/// Entry point: pick the final per-role model.
/// - `role`: the role to select for.
/// - `records`: normalized training records for this role (may be empty).
/// - `tier_one`: evidence from the evaluated artifact / stability report.
/// - `learned`: optional learned artifact summary; required to emit Learned.
pub fn select_role_model(
    role: Role,
    records: &[PerformanceRecord],
    tier_one: &TierOneEvidence,
    learned: Option<&LearnedArtifactSummary>,
) -> SelectionResult {
    if records.is_empty() && learned.is_none() {
        return Err(format!("{} ???????????????????", role_name(role)));
    }
    if let Some(artifact) = learned {
        if tier_one.eligible && learned_is_stable(artifact, tier_one) {
            return Ok(SelectedRoleModel {
                role,
                tier: PerformanceTier::Learned,
                weights: artifact.core_weights.clone(),
                base_offset: artifact.base_offset,
                shrink: 1.0,
                training_cohort: TrainingCohort::RoleQueue,
                config_hash: config_hash(tier_one, "learned"),
            });
        }
    }
    if records.is_empty() {
        return Err(format!("{} ???????????????????", role_name(role)));
    }
    if let Some(shrunk) = try_build_shrunk(role, records, tier_one) {
        if shrunk.role == role {
            return Ok(shrunk);
        }
    }
    Ok(build_percentile(role, records))
}

fn learned_is_stable(artifact: &LearnedArtifactSummary, tier_one: &TierOneEvidence) -> bool {
    if !artifact.eligible {
        return false;
    }
    if tier_one.direction_stability < SHRUNK_MIN_DIRECTION_STABILITY {
        return false;
    }
    if let Some(ci) = tier_one.brier_delta_ci95_high {
        if ci >= 0.02 {
            return false;
        }
    }
    if tier_one.improved_folds < 3 {
        return false;
    }
    // Validate business signs; death must be non-positive, final/assist positive.
    for (name, value) in &artifact.core_weights {
        if let Some(sign) = signed_feature_weight(artifact.role, name) {
            if (*value > 0.0 && sign < 0.0) || (*value < 0.0 && sign > 0.0) {
                return false;
            }
        }
    }
    true
}

fn try_build_shrunk(
    role: Role,
    records: &[PerformanceRecord],
    tier_one: &TierOneEvidence,
) -> Option<SelectedRoleModel> {
    let mut candidate: BTreeMap<String, f64> = BTreeMap::new();
    let mut direction_ok = false;
    if !tier_one.factor_directions.is_empty()
        && tier_one.direction_stability >= SHRUNK_MIN_DIRECTION_STABILITY
    {
        direction_ok = true;
        for (name, sign) in &tier_one.factor_directions {
            let approx_weight = shrink_mean_weight(records, name);
            let signed =
                if (*sign > 0.0 && approx_weight >= 0.0) || (*sign < 0.0 && approx_weight <= 0.0) {
                    approx_weight
                } else {
                    0.0
                };
            candidate.insert(name.clone(), signed);
        }
    }
    if !direction_ok {
        return None;
    }
    // Binary-search shrink factor so mean absolute correction <= 0.05.
    let shrink = binary_search_shrink(&candidate);
    let mut weights = BTreeMap::new();
    for (name, value) in &candidate {
        weights.insert(name.clone(), value * shrink);
    }
    let mean_abs = records
        .iter()
        .map(|r| correction_abs(&weights, r))
        .sum::<f64>()
        / records.len() as f64;
    if mean_abs > SHRUNK_MAX_MEAN_ABS_ADJUSTMENT {
        return None;
    }
    let cap_hit_rate = shrunk_cap_hit_rate(&weights, records);
    if cap_hit_rate > SHRUNK_MAX_CAP_HIT_RATE {
        return None;
    }
    if tier_one
        .brier_delta_ci95_high
        .is_some_and(|v| v > SHRUNK_MAX_BRIER_DEGRADATION)
    {
        return None;
    }
    Some(SelectedRoleModel {
        role,
        tier: PerformanceTier::Shrunk,
        weights,
        base_offset: 0.0,
        shrink,
        training_cohort: TrainingCohort::RoleQueue,
        config_hash: config_hash(tier_one, "shrunk"),
    })
}

#[allow(clippy::manual_clamp)]
fn build_percentile(role: Role, records: &[PerformanceRecord]) -> SelectedRoleModel {
    let mut bucket: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for record in records {
        bucket
            .entry(record.feature.clone())
            .or_default()
            .push(record.value);
    }
    let baseline = baseline_percentiles(&bucket);
    let mut weights: BTreeMap<String, f64> = baseline
        .iter()
        .map(|(name, stats)| {
            let sign = signed_feature_weight(role, name).unwrap_or(1.0);
            let direction = stats.median.signum();
            let weight = (direction * sign.abs()).max(0.0).min(1.0) * 0.3 * sign.signum().max(0.0);
            (
                name.clone(),
                if sign < 0.0 {
                    -weight.abs()
                } else {
                    weight.abs()
                },
            )
        })
        .collect();
    recenter_zero(&mut weights, records);
    let cohort = records
        .first()
        .map(|r| r.cohort)
        .unwrap_or(TrainingCohort::RoleQueue);
    SelectedRoleModel {
        role,
        tier: PerformanceTier::Percentile,
        weights,
        base_offset: 0.0,
        shrink: 0.3,
        training_cohort: cohort,
        config_hash: format!("percentile:{}", role_name(role)),
    }
}

#[allow(clippy::manual_clamp)]
fn recenter_zero(weights: &mut BTreeMap<String, f64>, records: &[PerformanceRecord]) {
    if records.is_empty() {
        return;
    }
    let mean: f64 = records
        .iter()
        .map(|r| weights.get(&r.feature).copied().unwrap_or(0.0) * r.value)
        .sum::<f64>()
        / records.len() as f64;
    if !mean.is_finite() || mean.abs() < 1e-12 {
        return;
    }
    let dampen = (1.0 - 1.0 / (records.len() as f64 + 1.0)).max(0.0).min(1.0);
    for value in weights.values_mut() {
        *value *= dampen;
    }
}

#[derive(Debug, Clone)]
struct PercentileStats {
    median: f64,
}
fn baseline_percentiles(bucket: &BTreeMap<String, Vec<f64>>) -> BTreeMap<String, PercentileStats> {
    bucket
        .iter()
        .map(|(name, values)| {
            let median = if values.is_empty() {
                0.0
            } else {
                let mut sorted = values.clone();
                sorted.sort_by(|a, b| a.total_cmp(b));
                let mid = sorted.len() / 2;
                if sorted.len() % 2 == 1 {
                    sorted[mid]
                } else {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                }
            };
            (name.clone(), PercentileStats { median })
        })
        .collect()
}

fn binary_search_shrink(candidate: &BTreeMap<String, f64>) -> f64 {
    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..32 {
        let mid = (lo + hi) / 2.0;
        let scaled: BTreeMap<String, f64> = candidate
            .iter()
            .map(|(name, value)| (name.clone(), value * mid))
            .collect();
        // Estimate mean absolute correction on synthetic zero-centered rows.
        let max_abs: f64 = scaled.values().map(|v| v.abs()).sum::<f64>();
        if max_abs <= SHRUNK_MAX_MEAN_ABS_ADJUSTMENT {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn shrink_mean_weight(records: &[PerformanceRecord], feature: &str) -> f64 {
    let matching: Vec<&PerformanceRecord> =
        records.iter().filter(|r| r.feature == feature).collect();
    if matching.is_empty() {
        return 0.0;
    }
    let mean = matching.iter().map(|r| r.value).sum::<f64>() / matching.len() as f64;
    mean.tanh() * 0.05
}

fn correction_abs(weights: &BTreeMap<String, f64>, record: &PerformanceRecord) -> f64 {
    weights
        .get(&record.feature)
        .map(|w| (w * record.value).abs())
        .unwrap_or(0.0)
}

fn shrunk_cap_hit_rate(weights: &BTreeMap<String, f64>, records: &[PerformanceRecord]) -> f64 {
    if records.is_empty() {
        return 0.0;
    }
    let _role = records.first().map(|r| r.role).unwrap_or(Role::Damage);
    let mode = records
        .first()
        .map(|r| r.cohort)
        .map(|c| match c {
            TrainingCohort::RoleQueue | TrainingCohort::OpenQueue => ModeFamily::RoleCompetitive,
            _ => ModeFamily::StadiumCompetitive,
        })
        .unwrap_or(ModeFamily::RoleCompetitive);
    let cap = mode.runtime_correction_cap(PerformanceTier::Shrunk);
    let hits = records
        .iter()
        .filter(|r| correction_abs(weights, r).min(cap) + 1e-9 >= cap)
        .count();
    hits as f64 / records.len() as f64
}

fn config_hash(_tier_one: &TierOneEvidence, prefix: &str) -> String {
    format!("{prefix}:{}", _tier_one.role.as_str())
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Tank => "??",
        Role::Damage => "??",
        Role::Support => "??",
    }
}
