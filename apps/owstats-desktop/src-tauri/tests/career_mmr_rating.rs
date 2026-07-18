use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, Role},
    rating::{
        apply_inactivity, bounded_performance_adjustment, expected_win, outcome_delta_for_expected,
        update_rating, update_rating_for_expected, RatingError, RatingInput, RatingUpdate,
        RoleRatingState, INITIAL_MU, INITIAL_SIGMA,
    },
};

const DAY_SECONDS: i64 = 86_400;

fn state() -> RoleRatingState {
    RoleRatingState::new(1_700_000_000, 22)
}

#[test]
fn performance_adjustment_uses_exact_role_mode_and_relative_caps() {
    assert_eq!(
        bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::RoleCompetitive),
        2.0
    );
    assert_eq!(
        bounded_performance_adjustment(-20.0, 99.0, Role::Damage, ModeFamily::RoleCompetitive),
        2.0
    );
    assert_eq!(
        bounded_performance_adjustment(20.0, 99.0, Role::Support, ModeFamily::RoleCompetitive),
        2.0
    );
    assert_eq!(
        bounded_performance_adjustment(40.0, 99.0, Role::Damage, ModeFamily::OpenCompetitive),
        3.0
    );
    assert_eq!(
        bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::RoleQuick),
        1.0
    );
    assert_eq!(
        bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::OpenQuick),
        1.0
    );
    assert_eq!(
        bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::StadiumQuick),
        0.0
    );
    assert!(
        20.0 + bounded_performance_adjustment(
            20.0,
            -99.0,
            Role::Damage,
            ModeFamily::RoleCompetitive
        ) > 0.0
    );
    assert!(
        -20.0
            + bounded_performance_adjustment(
                -20.0,
                99.0,
                Role::Damage,
                ModeFamily::RoleCompetitive
            )
            < 0.0
    );
}

#[test]
fn performance_limiter_fails_safe_for_non_finite_inputs() {
    for requested in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            bounded_performance_adjustment(
                20.0,
                requested,
                Role::Tank,
                ModeFamily::RoleCompetitive
            ),
            0.0
        );
    }
    assert_eq!(
        bounded_performance_adjustment(f64::NAN, 1.0, Role::Tank, ModeFamily::RoleCompetitive),
        0.0
    );
}

#[test]
fn expected_update_applies_performance_before_the_existing_match_cap() {
    let mut decisive = input(1.0, ModeFamily::RoleCompetitive);
    decisive.team_strength = None;
    decisive.opponent_strength = None;
    let baseline = update_rating_for_expected(state(), decisive, 0.5, 0.0).unwrap();
    let adjusted = update_rating_for_expected(state(), decisive, 0.5, 2.0).unwrap();
    assert_eq!(baseline.delta, 25.0);
    assert_eq!(adjusted.base_delta, 25.0);
    assert_eq!(adjusted.performance_adjustment, 2.0);
    assert_eq!(adjusted.delta, 27.0);

    let capped = update_rating_for_expected(state(), decisive, 0.0, 3.0).unwrap();
    assert_eq!(capped.delta, 30.0);
    assert_eq!(capped.performance_adjustment, 0.0);

    let mut draw = decisive;
    draw.result = 0.5;
    let neutral = update_rating_for_expected(state(), draw, 0.5, 2.0).unwrap();
    assert_eq!(neutral.base_delta, 0.0);
    assert_eq!(neutral.performance_adjustment, 0.0);
    assert_eq!(neutral.delta, 0.0);
}

#[test]
fn expected_update_rejects_invalid_expected_or_adjustment_without_state_change() {
    for (expected, adjustment) in [
        (f64::NAN, 0.0),
        (0.5, f64::INFINITY),
        (-0.1, 0.0),
        (1.1, 0.0),
    ] {
        let result = update_rating_for_expected(
            state(),
            input(1.0, ModeFamily::RoleCompetitive),
            expected,
            adjustment,
        )
        .unwrap();
        assert_eq!(result.new_state, state());
        assert_eq!(result.delta, 0.0);
    }
}
fn input(result: f64, mode: ModeFamily) -> RatingInput {
    RatingInput {
        result,
        mode,
        team_strength: Some(2500.0),
        opponent_strength: Some(2500.0),
        role_share: 1.0,
        data_quality: 1.0,
        played_at_seconds: 1_700_000_600,
        season: 22,
    }
}

fn update(state: RoleRatingState, input: RatingInput) -> RatingUpdate {
    update_rating(state, input).unwrap()
}

#[test]
fn stronger_opponents_produce_more_gain_for_a_win() {
    let even = update(state(), input(1.0, ModeFamily::RoleCompetitive));
    let mut upset_input = input(1.0, ModeFamily::RoleCompetitive);
    upset_input.team_strength = Some(2300.0);
    upset_input.opponent_strength = Some(2800.0);
    assert!(update(state(), upset_input).delta > even.delta);
}

#[test]
fn opponent_strength_orders_win_gains_and_loss_penalties() {
    let mut favored_win = input(1.0, ModeFamily::RoleCompetitive);
    favored_win.team_strength = Some(2700.0);
    favored_win.opponent_strength = Some(2300.0);
    let even_win = input(1.0, ModeFamily::RoleCompetitive);
    let mut upset_win = input(1.0, ModeFamily::RoleCompetitive);
    upset_win.team_strength = Some(2300.0);
    upset_win.opponent_strength = Some(2700.0);

    let favored_gain = update(state(), favored_win).delta;
    let even_gain = update(state(), even_win).delta;
    let upset_gain = update(state(), upset_win).delta;
    assert!(upset_gain > even_gain);
    assert!(even_gain > favored_gain);

    let mut weak_opponent_loss = input(0.0, ModeFamily::RoleCompetitive);
    weak_opponent_loss.team_strength = Some(2700.0);
    weak_opponent_loss.opponent_strength = Some(2300.0);
    let even_loss = input(0.0, ModeFamily::RoleCompetitive);
    let mut strong_opponent_loss = input(0.0, ModeFamily::RoleCompetitive);
    strong_opponent_loss.team_strength = Some(2300.0);
    strong_opponent_loss.opponent_strength = Some(2700.0);

    let weak_penalty = update(state(), weak_opponent_loss).delta.abs();
    let even_penalty = update(state(), even_loss).delta.abs();
    let strong_penalty = update(state(), strong_opponent_loss).delta.abs();
    assert!(weak_penalty > even_penalty);
    assert!(even_penalty > strong_penalty);
}

#[test]
fn result_direction_never_reverses_and_draw_is_finite() {
    assert!(update(state(), input(1.0, ModeFamily::RoleCompetitive)).delta >= 0.0);
    assert!(update(state(), input(0.0, ModeFamily::RoleCompetitive)).delta <= 0.0);
    assert!(update(state(), input(0.5, ModeFamily::RoleCompetitive))
        .delta
        .is_finite());
}

#[test]
fn per_role_caps_apply_after_role_share() {
    let uncertain = state();
    let mut competitive_input = input(1.0, ModeFamily::RoleCompetitive);
    competitive_input.opponent_strength = Some(3000.0);
    let competitive = update(uncertain, competitive_input);
    let quick = update(uncertain, input(1.0, ModeFamily::RoleQuick));
    assert_eq!(competitive.delta, 30.0);
    assert_eq!(quick.delta, 7.5);
    let mut partial = competitive_input;
    partial.role_share = 0.4;
    assert!(update(uncertain, partial).delta.abs() < competitive.delta.abs());
}

#[test]
fn every_mode_caps_both_directions_and_applies_share_before_clamping() {
    for (mode, cap) in [
        (ModeFamily::RoleCompetitive, 30.0),
        (ModeFamily::RoleQuick, 10.0),
        (ModeFamily::OpenCompetitive, 30.0),
        (ModeFamily::OpenQuick, 10.0),
        (ModeFamily::StadiumCompetitive, 30.0),
        (ModeFamily::StadiumQuick, 10.0),
    ] {
        let mut decisive_win = input(1.0, mode);
        decisive_win.team_strength = Some(1500.0);
        decisive_win.opponent_strength = Some(3500.0);
        let mut decisive_loss = input(0.0, mode);
        decisive_loss.team_strength = Some(3500.0);
        decisive_loss.opponent_strength = Some(1500.0);

        let capped_win = update(state(), decisive_win).delta;
        let capped_loss = update(state(), decisive_loss).delta;
        assert!(
            capped_win > 0.0 && capped_win <= cap,
            "positive cap for {mode:?}"
        );
        assert!(
            capped_loss < 0.0 && capped_loss >= -cap,
            "negative cap for {mode:?}"
        );

        let mut partial_win = decisive_win;
        partial_win.role_share = 0.2;
        let mut partial_loss = decisive_loss;
        partial_loss.role_share = 0.2;
        assert!(update(state(), partial_win).delta.abs() < capped_win.abs());
        assert!(update(state(), partial_loss).delta.abs() < capped_loss.abs());
    }
}

#[test]
fn every_mode_has_the_exact_outcome_weight() {
    for (mode, expected_delta) in [
        (ModeFamily::RoleCompetitive, 25.0),
        (ModeFamily::RoleQuick, 7.5),
        (ModeFamily::OpenCompetitive, 25.0),
        (ModeFamily::OpenQuick, 7.5),
        (ModeFamily::StadiumCompetitive, 12.5),
        (ModeFamily::StadiumQuick, 2.5),
    ] {
        assert_eq!(
            outcome_delta_for_expected(INITIAL_SIGMA, mode, 1.0, 1.0, 1.0, 0.5),
            expected_delta,
            "weight for {mode:?}"
        );
    }
}

#[test]
fn every_mode_caps_extreme_positive_and_negative_outcomes() {
    for (mode, cap) in [
        (ModeFamily::RoleCompetitive, 30.0),
        (ModeFamily::RoleQuick, 10.0),
        (ModeFamily::OpenCompetitive, 30.0),
        (ModeFamily::OpenQuick, 10.0),
        (ModeFamily::StadiumCompetitive, 30.0),
        (ModeFamily::StadiumQuick, 10.0),
    ] {
        assert_eq!(
            outcome_delta_for_expected(INITIAL_SIGMA, mode, 1.0, 1.0, 10.0, 0.0),
            cap
        );
        assert_eq!(
            outcome_delta_for_expected(INITIAL_SIGMA, mode, 1.0, 1.0, 0.0, 10.0),
            -cap
        );
    }
}

#[test]
fn expected_win_uses_fifty_percent_fallback_and_stays_finite() {
    assert_eq!(expected_win(None, Some(2500.0)), 0.5);
    assert_eq!(expected_win(Some(2500.0), None), 0.5);
    assert_eq!(expected_win(Some(f64::NAN), Some(2500.0)), 0.5);
    assert!((0.0..=1.0).contains(&expected_win(Some(-1.0e300), Some(1.0e300))));
}

#[test]
fn informative_matches_reduce_sigma_and_raise_effective_matches() {
    let first = update(state(), input(1.0, ModeFamily::RoleCompetitive));
    let second = update(first.new_state, input(0.0, ModeFamily::RoleCompetitive));
    assert!(first.new_state.sigma < INITIAL_SIGMA);
    assert!(second.new_state.sigma < first.new_state.sigma);
    assert!(second.new_state.effective_matches > first.new_state.effective_matches);
}

#[test]
fn inactivity_and_season_transition_expand_sigma_without_changing_mu() {
    let established = RoleRatingState {
        mu: 2650.0,
        sigma: 300.0,
        effective_matches: 20.0,
        last_played_seconds: Some(1_700_000_000),
        last_season: Some(22),
    };
    let inactive = apply_inactivity(established, 1_700_000_000 + 91 * DAY_SECONDS, 22).unwrap();
    assert_eq!(inactive.mu, established.mu);
    assert!(inactive.sigma > established.sigma);
    let next_season = apply_inactivity(established, 1_700_000_000 + DAY_SECONDS, 23).unwrap();
    assert_eq!(next_season.mu, established.mu);
    assert!(next_season.sigma > established.sigma);
    assert_eq!(
        apply_inactivity(established, 1_700_000_000 + 90 * DAY_SECONDS, 22)
            .unwrap()
            .sigma,
        established.sigma
    );
}

#[test]
fn update_reports_sigma_after_pre_match_season_preparation() {
    let established = RoleRatingState {
        mu: 2700.0,
        sigma: 300.0,
        effective_matches: 20.0,
        last_played_seconds: Some(1_700_000_000),
        last_season: Some(22),
    };
    let mut next = input(1.0, ModeFamily::RoleCompetitive);
    next.played_at_seconds = 1_700_000_000 + DAY_SECONDS;
    next.season = 23;
    let update = update_rating(established, next).unwrap();
    assert!(update.prepared_sigma_before > established.sigma);
    assert!(update.prepared_sigma_before > update.new_state.sigma);
}

#[test]
fn confidence_is_bounded_and_performance_is_disabled() {
    for candidate in [
        state(),
        RoleRatingState {
            mu: INITIAL_MU,
            sigma: 100.0,
            effective_matches: 1.0e9,
            last_played_seconds: None,
            last_season: None,
        },
    ] {
        let result = update(candidate, input(1.0, ModeFamily::OpenCompetitive));
        assert!((0.0..=1.0).contains(&result.confidence));
        assert_eq!(result.performance_adjustment, 0.0);
    }
}

#[test]
fn invalid_result_share_or_quality_is_a_complete_no_op() {
    let original = state();
    for (result, share, quality) in [
        (-0.1, 1.0, 1.0),
        (1.1, 1.0, 1.0),
        (f64::NAN, 1.0, 1.0),
        (1.0, -0.1, 1.0),
        (1.0, 1.1, 1.0),
        (1.0, f64::NAN, 1.0),
        (1.0, 1.0, -0.1),
        (1.0, 1.0, 50.0),
        (1.0, 1.0, f64::NAN),
    ] {
        let mut invalid = input(result, ModeFamily::StadiumCompetitive);
        invalid.role_share = share;
        invalid.data_quality = quality;
        let result = update(original, invalid);
        assert_eq!(result.new_state, original);
        assert_eq!(result.delta, 0.0);
        assert_eq!(result.base_delta, 0.0);
        assert_eq!(result.performance_adjustment, 0.0);
    }
}

#[test]
fn invalid_or_older_timestamps_are_no_ops_and_cannot_create_false_inactivity() {
    let original = state();
    for timestamp in [-1, 0, 1_699_999_999] {
        let mut invalid = input(1.0, ModeFamily::RoleCompetitive);
        invalid.played_at_seconds = timestamp;
        let result = update(original, invalid);
        assert_eq!(result.new_state, original);
        assert_eq!(result.delta, 0.0);
    }

    let mut older = input(1.0, ModeFamily::RoleCompetitive);
    older.played_at_seconds = 1_699_999_999;
    let ignored = update(original, older);
    let mut modern = input(1.0, ModeFamily::RoleCompetitive);
    modern.played_at_seconds = 1_700_000_600;
    let modern_result = update(ignored.new_state, modern);
    assert_eq!(
        modern_result.new_state.last_played_seconds,
        Some(1_700_000_600)
    );
    assert!(modern_result.new_state.sigma < original.sigma);
}

#[test]
fn non_finite_strengths_only_fall_back_expected_win() {
    let original = state();
    let mut valid = input(1.0, ModeFamily::RoleCompetitive);
    valid.team_strength = Some(f64::INFINITY);
    valid.opponent_strength = Some(f64::NAN);
    let result = update(original, valid);
    assert_eq!(result.expected_win_rate, 0.5);
    assert_ne!(result.new_state, original);
}

#[test]
fn invalid_rating_states_return_errors_without_producing_updates() {
    let valid_input = input(1.0, ModeFamily::RoleCompetitive);
    let invalid_states = [
        RoleRatingState {
            mu: f64::NAN,
            ..state()
        },
        RoleRatingState {
            mu: f64::INFINITY,
            ..state()
        },
        RoleRatingState {
            sigma: f64::NAN,
            ..state()
        },
        RoleRatingState {
            sigma: 0.0,
            ..state()
        },
        RoleRatingState {
            sigma: -1.0,
            ..state()
        },
        RoleRatingState {
            sigma: 1.0,
            ..state()
        },
        RoleRatingState {
            sigma: 99.9,
            ..state()
        },
        RoleRatingState {
            sigma: INITIAL_SIGMA + 0.1,
            ..state()
        },
        RoleRatingState {
            effective_matches: -1.0,
            ..state()
        },
        RoleRatingState {
            effective_matches: f64::INFINITY,
            ..state()
        },
        RoleRatingState {
            last_played_seconds: Some(0),
            ..state()
        },
        RoleRatingState {
            last_played_seconds: Some(-1),
            ..state()
        },
    ];
    for invalid in invalid_states {
        assert_eq!(
            update_rating(invalid, valid_input),
            Err(RatingError::InvalidState)
        );
    }
}

#[test]
fn public_inactivity_rejects_non_positive_or_rewound_time() {
    let original = state();
    for invalid_time in [-1, 0, 1_699_999_999] {
        assert_eq!(
            apply_inactivity(original, invalid_time, 22),
            Err(RatingError::InvalidTime)
        );
    }
}
