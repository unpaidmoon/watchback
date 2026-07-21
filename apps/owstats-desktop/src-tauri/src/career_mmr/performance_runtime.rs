//! Guaranteed personal-performance correction runtime.
//!
//! Evaluates a selected role model for one normalized performance row and
//! returns a bounded, finite, sign-preserving correction in MMR points.
//!
//! This module intentionally consumes a compact, deterministic
//! [`NormalizedPerformanceRow`] instead of coupling back to the heavyweight
//! performance matrix pipeline. Task 5 stitches the upstream normalization
//! into this contract; the selection and invariant logic here is independent.
use super::input::{ModeFamily, Role};
use super::performance_selection::SelectedRoleModel;
use serde::{Deserialize, Serialize};

/// Compact normalized performance row used by the v3 correction runtime.
///
/// Field names mirror upstream protocol fields (kept verbatim, not translated);
/// values are already standardized (zero-centered per role+mode+hero baseline).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPerformanceRow {
    pub role: Role,
    pub mode: ModeFamily,
    pub final_hit: f64,
    pub non_final_eliminations: f64,
    pub assist: f64,
    pub death: f64,
}

impl Default for NormalizedPerformanceRow {
    fn default() -> Self {
        Self {
            role: Role::Damage,
            mode: ModeFamily::RoleCompetitive,
            final_hit: 0.0,
            non_final_eliminations: 0.0,
            assist: 0.0,
            death: 0.0,
        }
    }
}

/// Signs preserved from the trained model (business invariants).
/// Death must contribute non-positively; final blow and assists non-negatively.
pub fn signed_feature_weight(role: Role, name: &str) -> Option<f64> {
    let sign = match (role, name) {
        (_, "death") => -1.0,
        (_, "final") | (_, "final_hit") | (_, "assist") => 1.0,
        (Role::Tank, "frontline_conversion") | (Role::Tank, "mitigation") => 1.0,
        (Role::Damage, "damage_conversion") => 1.0,
        (Role::Support, "healing") => 1.0,
        _ => 1.0,
    };
    Some(sign)
}

/// Raw (un-capped) correction produced by the selected model for the row.
pub fn raw_adjustment(model: &SelectedRoleModel, row: &NormalizedPerformanceRow) -> f64 {
    let cap = model.cap_for(row.mode);
    model.evaluate(row).clamp(-cap * 2.0, cap * 2.0)
}

/// Final bounded correction respecting the universal runtime invariants.
pub fn bounded_correction(
    model: &SelectedRoleModel,
    row: &NormalizedPerformanceRow,
    base_delta: f64,
) -> f64 {
    if !base_delta.is_finite() || base_delta == 0.0 {
        return 0.0;
    }
    let raw = raw_adjustment(model, row);
    if !raw.is_finite() {
        return 0.0;
    }
    let cap = model.cap_for(row.mode);
    let clamped = raw.clamp(-cap, cap);
    // Guarantee the result never reverses the outcome direction.
    if (base_delta > 0.0 && base_delta + clamped < 0.0)
        || (base_delta < 0.0 && base_delta + clamped > 0.0)
    {
        return base_delta.signum() * 0.0_f64.max(cap - base_delta.abs());
    }
    clamped
}

/// Apply every universal invariant on the final correction and return a clean
/// finite value. Used by the selection tests and by Task 5 replay.
pub fn invariant_checked_correction(
    model: &SelectedRoleModel,
    row: &NormalizedPerformanceRow,
    base_delta: f64,
) -> f64 {
    let correction = bounded_correction(model, row, base_delta);
    if !correction.is_finite() {
        return 0.0;
    }
    let cap = model.cap_for(row.mode);
    let correction = correction.clamp(-cap, cap);
    if base_delta == 0.0 {
        return 0.0;
    }
    if base_delta > 0.0 && base_delta + correction < 0.0 {
        return (-base_delta).max(-cap);
    }
    if base_delta < 0.0 && base_delta + correction > 0.0 {
        return (-base_delta).min(cap);
    }
    correction
}

/// Standard direction-stability threshold used by the shrunk tier.
pub const SHRUNK_MIN_DIRECTION_STABILITY: f64 = 0.60;
/// Maximum tolerated Brier degradation for the shrunk tier.
pub const SHRUNK_MAX_BRIER_DEGRADATION: f64 = 0.01;
/// Maximum tolerated cap-hit rate for the shrunk tier.
pub const SHRUNK_MAX_CAP_HIT_RATE: f64 = 0.05;
/// Maximum tolerated mean absolute correction for the shrunk tier.
pub const SHRUNK_MAX_MEAN_ABS_ADJUSTMENT: f64 = 0.05;

/// Smallest runtime correction that still counts as a cap hit for testing.
pub const CAP_HIT_EPSILON: f64 = 1e-9;

#[cfg(test)]
pub fn dummy_normalized_row(role: Role, mode: ModeFamily) -> NormalizedPerformanceRow {
    NormalizedPerformanceRow {
        role,
        mode,
        ..Default::default()
    }
}
