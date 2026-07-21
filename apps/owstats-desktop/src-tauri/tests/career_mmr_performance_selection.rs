//! Task 3 contract tests: tier selection, runtime invariants, stadium caps.
use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, PerformanceTier, Role, TrainingCohort},
    performance_runtime::{
        invariant_checked_correction, NormalizedPerformanceRow, SHRUNK_MIN_DIRECTION_STABILITY,
    },
    performance_selection::{
        select_role_model, LearnedArtifactSummary, PerformanceRecord, SelectedRoleModel,
        TierOneEvidence,
    },
};
use std::collections::BTreeMap;

fn learned_eligible(role: Role) -> LearnedArtifactSummary {
    let mut core_weights = BTreeMap::new();
    core_weights.insert("final".into(), 0.05);
    core_weights.insert("assist".into(), 0.02);
    core_weights.insert("death".into(), -0.03);
    LearnedArtifactSummary {
        role,
        eligible: true,
        core_weights,
        cohort_offset: BTreeMap::new(),
        base_offset: 0.0,
    }
}

fn records_for(role: Role, cohort: TrainingCohort) -> Vec<PerformanceRecord> {
    let features = ["final", "assist", "death"];
    features
        .iter()
        .flat_map(|feature| {
            let sign = match *feature {
                "death" => -1.0,
                _ => 1.0,
            };
            (0..30).map(move |i| PerformanceRecord {
                role,
                cohort,
                feature: (*feature).into(),
                value: sign * (i as f64) * 0.01,
                outcome: 1.0,
            })
        })
        .collect()
}

#[test]
fn empty_role_data_returns_chinese_error() {
    let result = select_role_model(Role::Tank, &[], &TierOneEvidence::failed(Role::Tank), None);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("??"));
    assert!(err.contains("??????"));
}

#[test]
fn tier_one_passing_evidence_selects_learned_tier() {
    let learned = learned_eligible(Role::Tank);
    let evidence = TierOneEvidence::eligible(Role::Tank);
    let records = records_for(Role::Tank, TrainingCohort::RoleQueue);
    let selected = select_role_model(Role::Tank, &records, &evidence, Some(&learned)).unwrap();
    assert_eq!(selected.tier(), PerformanceTier::Learned);
    assert_eq!(selected.role, Role::Tank);
}

#[test]
fn failed_tier_one_with_directions_selects_shrunk_tier() {
    let mut evidence = TierOneEvidence::failed(Role::Damage);
    evidence.direction_stability = SHRUNK_MIN_DIRECTION_STABILITY;
    evidence.brier_delta_ci95_high = Some(0.005); // shrunk allows brier degradation up to 0.01
    evidence.cap_hit_rate = 0.02;
    evidence.factor_directions = BTreeMap::from([("final".into(), 1.0), ("death".into(), -1.0)]);
    let records = records_for(Role::Damage, TrainingCohort::RoleQueue);
    let selected = select_role_model(Role::Damage, &records, &evidence, None).unwrap();
    assert_eq!(selected.tier(), PerformanceTier::Shrunk);
}

#[test]
fn unstable_directions_fall_back_to_percentile() {
    let mut evidence = TierOneEvidence::failed(Role::Support);
    evidence.direction_stability = 0.20; // below the shrunk threshold
    evidence.factor_directions = BTreeMap::from([("final".into(), 1.0)]);
    let records = records_for(Role::Support, TrainingCohort::RoleQueue);
    let selected = select_role_model(Role::Support, &records, &evidence, None).unwrap();
    assert_eq!(selected.tier(), PerformanceTier::Percentile);
}

#[test]
fn learned_rejects_sign_violation_and_falls_through_to_shrunk() {
    let mut learned = learned_eligible(Role::Tank);
    learned.core_weights.insert("death".into(), 0.05); // positive death violates business invariant
    let mut evidence = TierOneEvidence::eligible(Role::Tank);
    evidence.factor_directions = BTreeMap::from([("death".into(), -1.0)]);
    evidence.direction_stability = SHRUNK_MIN_DIRECTION_STABILITY;
    let records = records_for(Role::Tank, TrainingCohort::RoleQueue);
    let selected = select_role_model(Role::Tank, &records, &evidence, Some(&learned)).unwrap();
    assert_ne!(selected.tier(), PerformanceTier::Learned);
}

fn model_with(tier: PerformanceTier) -> SelectedRoleModel {
    let mut weights = BTreeMap::new();
    weights.insert("final".into(), 0.5);
    weights.insert("death".into(), -0.5);
    SelectedRoleModel {
        role: Role::Damage,
        tier,
        weights,
        base_offset: 0.0,
        shrink: 1.0,
        training_cohort: TrainingCohort::RoleQueue,
        config_hash: "test".into(),
    }
}

#[test]
fn runtime_invariant_finite_for_all_tiers_and_modes() {
    for tier in [
        PerformanceTier::Learned,
        PerformanceTier::Shrunk,
        PerformanceTier::Percentile,
    ] {
        let model = model_with(tier);
        for mode in [
            ModeFamily::RoleCompetitive,
            ModeFamily::RoleQuick,
            ModeFamily::OpenCompetitive,
            ModeFamily::OpenQuick,
            ModeFamily::StadiumCompetitive,
            ModeFamily::StadiumQuick,
        ] {
            let row = NormalizedPerformanceRow {
                role: Role::Damage,
                mode,
                final_hit: f64::INFINITY,
                non_final_eliminations: -f64::INFINITY,
                assist: f64::NAN,
                death: 1e308,
            };
            let base = 10.0;
            let correction = invariant_checked_correction(&model, &row, base);
            assert!(
                correction.is_finite(),
                "tier {:?} mode {:?} produced non-finite",
                tier,
                mode
            );
            assert!(
                correction.abs() <= model.cap_for(mode) + 1e-9,
                "tier {:?} mode {:?} correction {} exceeds cap {}",
                tier,
                mode,
                correction,
                model.cap_for(mode)
            );
        }
    }
}

#[test]
fn runtime_invariant_preserves_outcome_direction() {
    let model = model_with(PerformanceTier::Learned);
    for base in [10.0, 5.0, 1.0, -1.0, -5.0, -10.0] {
        let row = NormalizedPerformanceRow {
            role: Role::Damage,
            mode: ModeFamily::RoleCompetitive,
            final_hit: 0.0,
            non_final_eliminations: 0.0,
            assist: 0.0,
            death: 0.0,
        };
        let correction = invariant_checked_correction(&model, &row, base);
        assert!(
            (base + correction).signum() == base.signum() || correction == 0.0,
            "base {} + correction {} flipped direction",
            base,
            correction
        );
        if base == 0.0 {
            continue;
        }
        assert!(
            !(base > 0.0 && base + correction < 0.0) && !(base < 0.0 && base + correction > 0.0),
            "correction reversed outcome direction for base {}",
            base
        );
    }
}

#[test]
fn stadium_quick_caps_match_design() {
    assert_eq!(
        ModeFamily::StadiumQuick.runtime_correction_cap(PerformanceTier::Learned),
        0.10
    );
    assert_eq!(
        ModeFamily::StadiumQuick.runtime_correction_cap(PerformanceTier::Shrunk),
        0.10
    );
    assert_eq!(
        ModeFamily::StadiumQuick.runtime_correction_cap(PerformanceTier::Percentile),
        0.05
    );
}

#[test]
fn stadium_quick_correction_respects_05_percentile_cap() {
    let model = model_with(PerformanceTier::Percentile);
    let row = NormalizedPerformanceRow {
        role: Role::Damage,
        mode: ModeFamily::StadiumQuick,
        final_hit: 10.0,
        non_final_eliminations: 0.0,
        assist: 0.0,
        death: -10.0,
    };
    let correction = invariant_checked_correction(&model, &row, 10.0);
    assert!(
        correction.abs() <= 0.05 + 1e-12,
        "stadium quick percentile cap violated: {}",
        correction
    );
}

#[test]
fn stadium_quick_correction_respects_10_shrunk_cap() {
    let model = model_with(PerformanceTier::Shrunk);
    let row = NormalizedPerformanceRow {
        role: Role::Damage,
        mode: ModeFamily::StadiumQuick,
        final_hit: 10.0,
        non_final_eliminations: 0.0,
        assist: 0.0,
        death: -10.0,
    };
    let correction = invariant_checked_correction(&model, &row, 10.0);
    assert!(
        correction.abs() <= 0.10 + 1e-12,
        "stadium quick shrunk cap violated: {}",
        correction
    );
}

#[test]
fn selected_model_is_serializable_without_private_keys() {
    let learned = learned_eligible(Role::Tank);
    let evidence = TierOneEvidence::eligible(Role::Tank);
    let records = records_for(Role::Tank, TrainingCohort::RoleQueue);
    let selected = select_role_model(Role::Tank, &records, &evidence, Some(&learned)).unwrap();
    let json = serde_json::to_string(&selected).unwrap();
    for forbidden in [
        "account",
        "match",
        "player",
        "guid",
        "token",
        "cookie",
        "credential",
    ] {
        assert!(
            !json.to_lowercase().contains(forbidden),
            "serialization leaked {}",
            forbidden
        );
    }
    let decoded: SelectedRoleModel = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, selected);
}
