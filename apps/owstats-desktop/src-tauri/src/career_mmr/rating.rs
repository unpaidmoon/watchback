//! Pure baseline role-rating math.
//!
//! This is a Bayesian-inspired uncertainty model, not an implementation of
//! TrueSkill or any other official Overwatch rating system. All timestamps are
//! Unix seconds.

use super::input::ModeFamily;
use std::{error::Error, fmt};

pub const INITIAL_MU: f64 = 2500.0;
pub const INITIAL_SIGMA: f64 = 833.0;
const MIN_SIGMA: f64 = 100.0;
const INACTIVITY_THRESHOLD_SECONDS: i64 = 90 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoleRatingState {
    pub mu: f64,
    pub sigma: f64,
    pub effective_matches: f64,
    pub last_played_seconds: Option<i64>,
    pub last_season: Option<i64>,
}

impl RoleRatingState {
    pub fn new(played_at_seconds: i64, season: i64) -> Self {
        Self {
            mu: INITIAL_MU,
            sigma: INITIAL_SIGMA,
            effective_matches: 0.0,
            last_played_seconds: (played_at_seconds >= 0).then_some(played_at_seconds),
            last_season: Some(season),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RatingInput {
    pub result: f64,
    pub mode: ModeFamily,
    pub team_strength: Option<f64>,
    pub opponent_strength: Option<f64>,
    pub role_share: f64,
    pub data_quality: f64,
    pub played_at_seconds: i64,
    pub season: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatingUpdate {
    /// Sigma after deterministic season/inactivity preparation and before this match.
    pub prepared_sigma_before: f64,
    pub delta: f64,
    pub new_state: RoleRatingState,
    pub expected_win_rate: f64,
    pub confidence: f64,
    pub base_delta: f64,
    pub performance_adjustment: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingError {
    InvalidState,
    InvalidTime,
}

impl fmt::Display for RatingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState => formatter.write_str("invalid role rating state"),
            Self::InvalidTime => formatter.write_str("invalid rating timestamp"),
        }
    }
}

impl Error for RatingError {}

pub fn expected_win(team: Option<f64>, opponent: Option<f64>) -> f64 {
    let (team, opponent) = match (team, opponent) {
        (Some(team), Some(opponent)) if team.is_finite() && opponent.is_finite() => {
            (team, opponent)
        }
        _ => return 0.5,
    };
    let exponent = ((opponent - team) / 400.0).clamp(-16.0, 16.0);
    (1.0 / (1.0 + 10_f64.powf(exponent))).clamp(0.0, 1.0)
}

pub fn outcome_delta_for_expected(
    sigma: f64,
    mode: ModeFamily,
    role_share: f64,
    data_quality: f64,
    result: f64,
    expected: f64,
) -> f64 {
    let uncertainty_step = 50.0 * (sigma / INITIAL_SIGMA).clamp(0.2, 1.5);
    finite_or(
        uncertainty_step * mode_weight(mode) * role_share * data_quality * (result - expected),
        0.0,
    )
    .clamp(-mode_cap(mode), mode_cap(mode))
}

pub fn apply_inactivity(
    state: RoleRatingState,
    now_seconds: i64,
    current_season: i64,
) -> Result<RoleRatingState, RatingError> {
    validate_state(&state)?;
    if now_seconds <= 0
        || state
            .last_played_seconds
            .is_some_and(|last| now_seconds < last)
    {
        return Err(RatingError::InvalidTime);
    }
    Ok(apply_inactivity_valid(state, now_seconds, current_season))
}

fn apply_inactivity_valid(
    mut state: RoleRatingState,
    now_seconds: i64,
    current_season: i64,
) -> RoleRatingState {
    let season_changed = state
        .last_season
        .is_some_and(|season| season != current_season);
    let inactive = state.last_played_seconds.is_some_and(|last| {
        now_seconds >= 0 && now_seconds.saturating_sub(last) > INACTIVITY_THRESHOLD_SECONDS
    });
    if season_changed || inactive {
        let season_factor = if season_changed { 1.12 } else { 1.0 };
        let inactivity_factor = if inactive { 1.10 } else { 1.0 };
        state.sigma = (state.sigma * season_factor * inactivity_factor).min(INITIAL_SIGMA);
    }
    state
}

pub fn update_rating(
    state: RoleRatingState,
    input: RatingInput,
) -> Result<RatingUpdate, RatingError> {
    validate_state(&state)?;
    let expected = expected_win(input.team_strength, input.opponent_strength);
    if !validate_update_input(&state, &input) {
        return Ok(no_op_update(state, expected));
    }
    let played_at = input.played_at_seconds;
    let state = apply_inactivity_valid(state, played_at, input.season);
    let role_share = input.role_share;
    let data_quality = input.data_quality;
    let base_delta = outcome_delta_for_expected(
        state.sigma,
        input.mode,
        role_share,
        data_quality,
        input.result,
        expected,
    );
    let delta = base_delta;
    let information = role_share * data_quality;
    let sigma_reduction = (1.0 - 0.025 * information).clamp(0.90, 1.0);
    let new_sigma = (state.sigma * sigma_reduction).clamp(MIN_SIGMA, INITIAL_SIGMA);
    let new_state = RoleRatingState {
        mu: finite_or(state.mu + delta, state.mu),
        sigma: new_sigma,
        effective_matches: finite_or(
            state.effective_matches + information,
            state.effective_matches,
        ),
        last_played_seconds: Some(played_at),
        last_season: Some(input.season),
    };
    Ok(RatingUpdate {
        prepared_sigma_before: state.sigma,
        delta,
        new_state,
        expected_win_rate: expected,
        confidence: confidence(new_state, data_quality),
        base_delta,
        performance_adjustment: 0.0,
    })
}

fn validate_state(state: &RoleRatingState) -> Result<(), RatingError> {
    let valid = state.mu.is_finite()
        && state.sigma.is_finite()
        && state.sigma >= MIN_SIGMA
        && state.sigma <= INITIAL_SIGMA
        && state.effective_matches.is_finite()
        && state.effective_matches >= 0.0
        && state.last_played_seconds.is_none_or(|value| value > 0);
    if valid {
        Ok(())
    } else {
        Err(RatingError::InvalidState)
    }
}

fn validate_update_input(state: &RoleRatingState, input: &RatingInput) -> bool {
    input.result.is_finite()
        && (0.0..=1.0).contains(&input.result)
        && input.role_share.is_finite()
        && input.role_share > 0.0
        && input.role_share <= 1.0
        && input.data_quality.is_finite()
        && (0.0..=1.0).contains(&input.data_quality)
        && input.played_at_seconds > 0
        && state
            .last_played_seconds
            .is_none_or(|last| input.played_at_seconds >= last)
}

fn no_op_update(state: RoleRatingState, expected_win_rate: f64) -> RatingUpdate {
    RatingUpdate {
        prepared_sigma_before: state.sigma,
        delta: 0.0,
        new_state: state,
        expected_win_rate,
        confidence: confidence(state, 0.0),
        base_delta: 0.0,
        performance_adjustment: 0.0,
    }
}

pub fn confidence(state: RoleRatingState, data_quality: f64) -> f64 {
    let uncertainty = 1.0 - (state.sigma / INITIAL_SIGMA).clamp(0.0, 1.0);
    let samples = 1.0 - (-state.effective_matches / 20.0).exp();
    (0.45 * uncertainty + 0.40 * samples + 0.15 * unit_interval(data_quality, 0.0)).clamp(0.0, 1.0)
}

fn mode_weight(mode: ModeFamily) -> f64 {
    match mode {
        ModeFamily::QuickPlay => 0.5,
        ModeFamily::RoleCompetitive
        | ModeFamily::OpenCompetitive
        | ModeFamily::StadiumCompetitive => 1.0,
    }
}

fn mode_cap(mode: ModeFamily) -> f64 {
    match mode {
        ModeFamily::QuickPlay => 10.0,
        ModeFamily::RoleCompetitive
        | ModeFamily::OpenCompetitive
        | ModeFamily::StadiumCompetitive => 30.0,
    }
}

fn unit_interval(value: f64, fallback: f64) -> f64 {
    finite_or(value, fallback).clamp(0.0, 1.0)
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}
