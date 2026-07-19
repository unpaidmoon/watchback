use anyhow::Result;
use owstats_desktop_lib::career_mmr::{
    input::{ModeFamily, Role},
    performance::{build_feature_matrix, HealingTakenStrategy},
    performance_stability::{evaluate_stability, StabilityConfig},
    shadow::{aggregate_shadow_summaries, matrix_row_from_artifact, simulate_shadow_replay},
    training::{TrainingAudit, TrainingEnvironment, TrainingRow},
    training_report::{
        build_training_outputs, AggregateSampleBucket, BaselineArtifact, CellArtifact,
        FitSearchArtifact, HierarchicalModelArtifact, NormalizationArtifact, PerformanceCaps,
        PerformanceModelArtifact, RegularizationCandidate, TrainingConfigArtifact,
        ValidationGateArtifact,
    },
};
use owstats_desktop_lib::{
    account_store::{
        write_manifest_atomic, write_registry_atomic, AccountId, AccountManifest, AccountRegistry,
        AccountRegistryEntry,
    },
    app_paths::SharedAppPaths,
    career_mmr::{
        training::build_multi_account_projection,
        training_sources::{discover_training_sources, open_query_only},
    },
    OwDatabase,
};
use rusqlite::params;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::fs;
use tempfile::TempDir;

fn row(
    account: &str,
    group: &str,
    role: &str,
    mode: ModeFamily,
    at: i64,
    duration: f64,
) -> TrainingRow {
    TrainingRow {
        account_group: account.into(),
        source_match_group: group.into(),
        match_group: group.into(),
        season: 22,
        begin_ts: at,
        role: role.into(),
        mode_family: mode,
        environment: if matches!(
            mode,
            ModeFamily::StadiumCompetitive | ModeFamily::StadiumQuick
        ) {
            TrainingEnvironment::Stadium
        } else {
            TrainingEnvironment::Ordinary
        },
        hero_group: Some(format!("{role}-hero")),
        outcome_only: mode == ModeFamily::StadiumQuick,
        allow_performance: mode != ModeFamily::StadiumQuick,
        allow_performance_by_hero: true,
        result: 1.0,
        expected_win_rate: 0.5,
        data_quality: 1.0,
        base_delta: Some(25.0),
        final_hit: Some(6.0),
        kill: Some(8.0),
        assist: Some(4.0),
        death: Some(2.0),
        damage: Some(1000.0),
        healing: Some(500.0),
        mitigation: Some(300.0),
        healing_taken: None,
        available_team_healing: None,
        damage_taken: Some(800.0),
        objective_time_sec: Some(20.0),
        duration_sec: 600.0,
        hero_duration_sec: Some(duration),
        healing_taken_semantic: "none",
        usable_for_resource_debt: false,
    }
}

fn artifact() -> PerformanceModelArtifact {
    let role_model = |_role: Role| HierarchicalModelArtifact {
        model_spec_id: None,
        selected_strategy: Some("none".into()),
        selected_variant: Some("core".into()),
        feature_weights: BTreeMap::from([
            ("final".into(), 1.0),
            ("nonfinal".into(), 0.0),
            ("assist".into(), 0.0),
            ("death".into(), 0.0),
        ]),
        training_cohort_offsets: BTreeMap::new(),
        training_cohort_samples: BTreeMap::new(),
        final_hit_multiplier: Some(1.0),
        lambda_core: Some(0.1),
        lambda_cohort: Some(0.1),
        calibration_p95_abs_probability_effect: Some(1.0),
    };
    let mut roles = BTreeMap::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        roles.insert(role, role_model(role));
    }
    let mut cells = Vec::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        for mode in [
            ModeFamily::RoleCompetitive,
            ModeFamily::RoleQuick,
            ModeFamily::OpenCompetitive,
            ModeFamily::OpenQuick,
            ModeFamily::StadiumCompetitive,
            ModeFamily::StadiumQuick,
        ] {
            cells.push(CellArtifact {
                role,
                mode,
                training_cohort: mode.training_cohort(),
                shared_parameter_version: "v".into(),
                role_model_ref: "r".into(),
                model_spec_id: None,
                performance_enabled: mode != ModeFamily::StadiumQuick,
                calibration_p95_abs_logit_effect: 1.0,
            });
        }
    }
    let mut baselines = Vec::new();
    for role in ["tank", "damage", "support"] {
        for feature in [
            "final_hit",
            "non_final_eliminations",
            "assist",
            "death",
            "damage",
            "healing",
            "mitigation",
            "damage_taken",
            "objective_time",
        ] {
            baselines.push(BaselineArtifact {
                role: role.into(),
                feature: feature.into(),
                environment: "ordinary".into(),
                hero: None,
                mode: None,
                season: None,
                sample_count: 100,
                median_per_ten: if feature == "final_hit" { 5.0 } else { 0.0 },
                mad_per_ten: 0.6745,
            });
        }
    }
    PerformanceModelArtifact {
        schema_version: 2,
        algorithm: "test".into(),
        feature_version: "test".into(),
        seed: 1,
        roles,
        cells,
        normalization: NormalizationArtifact {
            hierarchy: vec![],
            minimum_hero_samples: 30,
            z_clip: 3.0,
            mad_scale: 0.6745,
            mad_floor_absolute: 1e-6,
            mad_floor_relative: 1e-6,
            healing_min_regression_samples: 100,
            healing_huber_iterations: 8,
            healing_ridge_lambda: 0.1,
            baselines,
            healing_expectations: vec![],
        },
        caps: PerformanceCaps::default(),
        training_config: TrainingConfigArtifact {
            seed: 1,
            mode_weights: BTreeMap::new(),
            fit: FitSearchArtifact {
                variants: vec![],
                final_hit_multipliers: vec![],
                regularization_candidates: vec![RegularizationCandidate {
                    lambda_core: 0.1,
                    lambda_cohort: 0.1,
                }],
                learning_rate: 0.1,
                steps: 1,
                minimum_cohort_offset_rows: 1,
                promoted_candidates: None,
            },
            validation: ValidationGateArtifact {
                stage: "test".into(),
                bootstrap_iterations: 1,
                minimum_improved_folds: 1,
                minimum_role_core_rows: 1,
                minimum_cohort_rows: 1,
                minimum_season_loo_groups: 1,
                minimum_hero_loo_groups: 1,
                minimum_loo_row_coverage: 0.0,
                maximum_loo_worsened_fraction: 1.0,
                minimum_bootstrap_completion: 0.0,
                minimum_critical_direction_stability: 0.0,
                maximum_bootstrap_brier_ci_high: 0.0,
                max_ece_worsening: 0.0,
                maximum_single_feature_share: 1.0,
                maximum_average_adjustment_mmr: 1.0,
                maximum_cap_hit_rate: 1.0,
                require_account_coverage: false,
                require_critical_ci_excludes_zero: false,
                require_nonzero_reference_coefficients: false,
            },
        },
        sample_buckets: Vec::<AggregateSampleBucket>::new(),
        config_hash: "test".into(),
    }
}

#[test]
fn shadow_replay_is_stable_isolated_and_aggregates_role_shares() {
    let rows_a = vec![
        row(
            "account-a",
            "later",
            "damage",
            ModeFamily::RoleCompetitive,
            200,
            600.0,
        ),
        row(
            "account-a",
            "shared",
            "support",
            ModeFamily::RoleCompetitive,
            100,
            300.0,
        ),
        row(
            "account-a",
            "shared",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            300.0,
        ),
    ];
    let mut reversed = rows_a.clone();
    reversed.reverse();
    let first = simulate_shadow_replay(&rows_a, &artifact()).unwrap();
    let second = simulate_shadow_replay(&reversed, &artifact()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.account_group, "account-a");
    assert_eq!(first.replayed_matches.len(), 3);
    let shared_index = first
        .replayed_matches
        .iter()
        .find(|m| m.role == Role::Support)
        .unwrap()
        .match_index;
    let shared: f64 = first
        .replayed_matches
        .iter()
        .filter(|m| m.match_index == shared_index)
        .map(|m| m.role_share)
        .sum();
    assert_eq!(shared, 1.0);
    assert!(
        first
            .roles
            .iter()
            .find(|r| r.role == Role::Damage)
            .unwrap()
            .shadow_mmr
            > first
                .roles
                .iter()
                .find(|r| r.role == Role::Damage)
                .unwrap()
                .baseline_mmr
    );

    let other = simulate_shadow_replay(
        &[row(
            "account-b",
            "only",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            600.0,
        )],
        &artifact(),
    )
    .unwrap();
    assert_eq!(
        other
            .roles
            .iter()
            .find(|r| r.role == Role::Damage)
            .unwrap()
            .matches,
        1
    );
    assert_eq!(
        first
            .roles
            .iter()
            .find(|r| r.role == Role::Damage)
            .unwrap()
            .matches,
        2
    );
}

#[test]
fn only_eligible_cells_adjust_and_stadium_quick_is_always_outcome_only() {
    let mut model = artifact();
    model
        .cells
        .iter_mut()
        .find(|c| c.role == Role::Damage && c.mode == ModeFamily::RoleQuick)
        .unwrap()
        .performance_enabled = false;
    let rows = vec![
        row(
            "a",
            "eligible",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            600.0,
        ),
        row("a", "shadow", "damage", ModeFamily::RoleQuick, 200, 600.0),
        row(
            "a",
            "outcome",
            "damage",
            ModeFamily::StadiumQuick,
            300,
            600.0,
        ),
    ];
    let result = simulate_shadow_replay(&rows, &model).unwrap();
    let eligible = result
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    let shadow = result
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::RoleQuick)
        .unwrap();
    let outcome = result
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::StadiumQuick)
        .unwrap();
    assert!(eligible.performance_adjustment > 0.0);
    assert_eq!(shadow.performance_adjustment, 0.0);
    assert_eq!(outcome.performance_adjustment, 0.0);
}

#[test]
fn shadow_uses_exact_actual_cell_logit_p95_for_shared_role_model() {
    let mut model = artifact();
    model
        .cells
        .iter_mut()
        .find(|c| c.role == Role::Damage && c.mode == ModeFamily::RoleCompetitive)
        .unwrap()
        .calibration_p95_abs_logit_effect = 1.0;
    model
        .cells
        .iter_mut()
        .find(|c| c.role == Role::Damage && c.mode == ModeFamily::RoleQuick)
        .unwrap()
        .calibration_p95_abs_logit_effect = 2.0;
    let replay = simulate_shadow_replay(
        &[
            row(
                "a",
                "competitive",
                "damage",
                ModeFamily::RoleCompetitive,
                100,
                600.0,
            ),
            row("a", "quick", "damage", ModeFamily::RoleQuick, 200, 600.0),
        ],
        &model,
    )
    .unwrap();
    let competitive = replay
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::RoleCompetitive)
        .unwrap();
    let quick = replay
        .replayed_matches
        .iter()
        .find(|m| m.mode == ModeFamily::RoleQuick)
        .unwrap();
    assert_eq!(competitive.performance_adjustment, 1.0);
    assert_eq!(quick.performance_adjustment, 0.5);
}

#[test]
fn zero_effect_matches_baseline_exactly_and_caps_are_reported() {
    let mut model = artifact();
    for role in model.roles.values_mut() {
        role.feature_weights.values_mut().for_each(|v| *v = 0.0);
    }
    let baseline = simulate_shadow_replay(
        &[row(
            "a",
            "m",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            600.0,
        )],
        &model,
    )
    .unwrap();
    let damage = baseline
        .roles
        .iter()
        .find(|r| r.role == Role::Damage)
        .unwrap();
    assert_eq!(damage.shadow_mmr, damage.baseline_mmr);
    assert_eq!(damage.total_performance_adjustment, 0.0);

    let capped = simulate_shadow_replay(
        &[row("a", "q", "damage", ModeFamily::RoleQuick, 100, 600.0)],
        &artifact(),
    )
    .unwrap();
    assert_eq!(
        capped.cap_hits[&owstats_desktop_lib::career_mmr::shadow::CellKey {
            role: Role::Damage,
            mode: ModeFamily::RoleQuick,
        }]
            .upper,
        1
    );
}

#[test]
fn privacy_safe_shadow_aggregate_preserves_exact_role_and_overall_mmr_means() {
    let mut model = artifact();
    model
        .cells
        .iter_mut()
        .filter(|cell| cell.role == Role::Support)
        .for_each(|cell| cell.performance_enabled = false);
    let damage = simulate_shadow_replay(
        &[row(
            "private-account-a",
            "private-match-a",
            "damage",
            ModeFamily::RoleCompetitive,
            100,
            600.0,
        )],
        &model,
    )
    .unwrap();
    let support = simulate_shadow_replay(
        &[
            row(
                "private-account-b",
                "private-match-b",
                "support",
                ModeFamily::RoleCompetitive,
                100,
                600.0,
            ),
            row(
                "private-account-b",
                "private-match-c",
                "support",
                ModeFamily::StadiumQuick,
                200,
                600.0,
            ),
        ],
        &model,
    )
    .unwrap();
    let damage_state = damage
        .roles
        .iter()
        .find(|state| state.role == Role::Damage)
        .unwrap();
    let support_state = support
        .roles
        .iter()
        .find(|state| state.role == Role::Support)
        .unwrap();
    assert_ne!(damage_state.shadow_mmr, damage_state.baseline_mmr);
    assert_eq!(support_state.shadow_mmr, support_state.baseline_mmr);
    assert_eq!(
        support
            .replayed_matches
            .iter()
            .find(|item| item.mode == ModeFamily::StadiumQuick)
            .unwrap()
            .performance_adjustment,
        0.0
    );

    let aggregate = aggregate_shadow_summaries(&[damage.clone(), support.clone()], 2).unwrap();
    assert_eq!(aggregate.account_count, 2);
    assert_eq!(aggregate.role_state_count, 2);
    assert_eq!(aggregate.match_count, 3);
    let damage_aggregate = aggregate
        .roles
        .iter()
        .find(|item| item.role == Role::Damage)
        .unwrap();
    assert_eq!(damage_aggregate.account_count, 1);
    assert_eq!(damage_aggregate.match_count, 1);
    assert_eq!(
        damage_aggregate.mean_baseline_mmr,
        Some(damage_state.baseline_mmr)
    );
    assert_eq!(
        damage_aggregate.mean_shadow_mmr,
        Some(damage_state.shadow_mmr)
    );
    assert_eq!(
        damage_aggregate.mean_difference,
        Some(damage_state.shadow_mmr - damage_state.baseline_mmr)
    );
    let support_aggregate = aggregate
        .roles
        .iter()
        .find(|item| item.role == Role::Support)
        .unwrap();
    assert_eq!(support_aggregate.account_count, 1);
    assert_eq!(support_aggregate.match_count, 2);
    assert_eq!(support_aggregate.mean_difference, Some(0.0));
    assert_eq!(
        aggregate.mean_baseline_mmr,
        Some((damage_state.baseline_mmr + support_state.baseline_mmr) / 2.0)
    );
    assert_eq!(
        aggregate.mean_shadow_mmr,
        Some((damage_state.shadow_mmr + support_state.shadow_mmr) / 2.0)
    );
    assert_eq!(
        aggregate.mean_difference,
        Some(
            ((damage_state.shadow_mmr - damage_state.baseline_mmr)
                + (support_state.shadow_mmr - support_state.baseline_mmr))
                / 2.0
        )
    );
    let json = serde_json::to_string(&aggregate)
        .unwrap()
        .to_ascii_lowercase();
    for forbidden in [
        "private-account",
        "private-match",
        "account_group",
        "match_group",
        "database_path",
        "account_index",
    ] {
        assert!(!json.contains(forbidden), "leaked {forbidden}: {json}");
    }

    for case in ["role-difference", "overall-difference", "weighted-baseline"] {
        let mut invalid = aggregate.clone();
        match case {
            "role-difference" => {
                invalid
                    .roles
                    .iter_mut()
                    .find(|role| role.role == Role::Damage)
                    .unwrap()
                    .mean_difference = Some(99.0)
            }
            "overall-difference" => invalid.mean_difference = Some(99.0),
            "weighted-baseline" => invalid.mean_baseline_mmr = Some(99.0),
            _ => unreachable!(),
        }
        assert!(invalid.validate(2).is_err(), "accepted {case}");
    }
}

#[test]
fn shadow_replay_has_no_database_handle_and_cannot_change_source_aggregates() {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE career_mmr_events(role TEXT, final_delta REAL); INSERT INTO career_mmr_events VALUES ('damage', 25.0);").unwrap();
    connection.pragma_update(None, "query_only", true).unwrap();
    let before_changes = connection.total_changes();
    let before: (i64, f64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(final_delta),0) FROM career_mmr_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let projected = vec![row(
        "dataset",
        "memory",
        "damage",
        ModeFamily::RoleCompetitive,
        100,
        600.0,
    )];
    let _ = simulate_shadow_replay(&projected, &artifact()).unwrap();
    let after: (i64, f64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(final_delta),0) FROM career_mmr_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(connection.total_changes(), before_changes);
    assert_eq!(after, before);
}

#[test]
fn artifact_feature_replay_preserves_missing_option_semantics() {
    let mut model = artifact();
    let mut missing = row(
        "a",
        "missing",
        "damage",
        ModeFamily::RoleCompetitive,
        100,
        600.0,
    );
    missing.damage = None;
    let matrix = matrix_row_from_artifact(
        &missing,
        owstats_desktop_lib::career_mmr::performance::HealingTakenStrategy::None,
        &model.normalization,
    )
    .unwrap();
    assert_eq!(matrix.per_ten.damage, None);
    assert_eq!(matrix.standardized.damage, None);

    model
        .normalization
        .baselines
        .retain(|baseline| baseline.feature != "damage" && baseline.feature != "healing_taken");
    model.normalization.healing_expectations.push(
        owstats_desktop_lib::career_mmr::training_report::HealingExpectationArtifact {
            role: "damage".into(),
            environment: "ordinary".into(),
            hero: None,
            mode: None,
            season: None,
            sample_count: 100,
            fallback_median_per_ten: 500.0,
            coefficients: None,
        },
    );
    let mut complete = row(
        "a",
        "two-hero",
        "damage",
        ModeFamily::RoleCompetitive,
        100,
        300.0,
    );
    complete.healing_taken = Some(400.0);
    complete.usable_for_resource_debt = true;
    let missing_baseline = matrix_row_from_artifact(
        &complete,
        HealingTakenStrategy::ResourceConversionDebt,
        &model.normalization,
    )
    .unwrap();
    assert_eq!(missing_baseline.standardized.damage, None);
    assert_eq!(missing_baseline.damage_baseline_median_per_ten, None);
    assert_eq!(missing_baseline.healing_taken_derived, None);

    let mut second_hero = complete.clone();
    second_hero.hero_group = Some("damage-hero-2".into());
    second_hero.healing_taken = None;
    second_hero.usable_for_resource_debt = false;
    let external_unavailable = matrix_row_from_artifact(
        &second_hero,
        HealingTakenStrategy::DirectNegative,
        &model.normalization,
    )
    .unwrap();
    assert_eq!(external_unavailable.healing_taken_derived, None);
}

#[test]
fn stored_artifact_features_match_training_matrix_for_all_five_strategies() {
    let mut rows = Vec::new();
    for index in 0..35 {
        let mut candidate = row(
            "dataset",
            &format!("m-{index}"),
            "damage",
            ModeFamily::RoleCompetitive,
            100 + index,
            600.0,
        );
        candidate.hero_group = Some(if index % 2 == 0 { "hero-a" } else { "hero-b" }.into());
        candidate.final_hit = Some(2.0 + index as f64 * 0.1);
        candidate.kill = candidate.final_hit.map(|value| value + 3.0);
        candidate.damage = (index != 0).then_some(1_000.0 + index as f64 * 20.0);
        candidate.objective_time_sec = (index != 1).then_some(10.0 + index as f64);
        candidate.healing_taken = (index != 2).then_some(500.0 + index as f64 * 10.0);
        candidate.available_team_healing = (index != 3).then_some(4_000.0);
        candidate.usable_for_resource_debt = candidate.healing_taken.is_some();
        rows.push(candidate);
    }
    let evaluation = evaluate_stability(
        &[],
        StabilityConfig {
            seed: 91,
            ..Default::default()
        },
    );
    let normalization = build_training_outputs(&rows, &TrainingAudit::default(), &evaluation, 91)
        .unwrap()
        .0
        .normalization;
    for strategy in [
        HealingTakenStrategy::None,
        HealingTakenStrategy::DirectNegative,
        HealingTakenStrategy::HeroNormalized,
        HealingTakenStrategy::ExcessHealing,
        HealingTakenStrategy::ResourceConversionDebt,
    ] {
        let training = build_feature_matrix(&rows, &[], strategy);
        let expected = &training.training_by_role["damage"];
        for (source, expected) in rows.iter().zip(expected) {
            let actual = matrix_row_from_artifact(source, strategy, &normalization).unwrap();
            assert_eq!(
                actual, *expected,
                "strategy {strategy:?}, group {}",
                source.match_group
            );
        }
    }
    let missing_damage =
        matrix_row_from_artifact(&rows[0], HealingTakenStrategy::None, &normalization).unwrap();
    assert_eq!(missing_damage.per_ten.damage, None);
    let missing_conversion = matrix_row_from_artifact(
        &rows[1],
        HealingTakenStrategy::ResourceConversionDebt,
        &normalization,
    )
    .unwrap();
    assert_eq!(missing_conversion.healing_taken_derived, None);
    let external_unavailable = matrix_row_from_artifact(
        &rows[2],
        HealingTakenStrategy::DirectNegative,
        &normalization,
    )
    .unwrap();
    assert_eq!(external_unavailable.healing_taken_derived, None);
}

fn shadow_account_id(index: u8) -> Result<AccountId> {
    AccountId::parse(&format!("10000000-0000-4000-8000-{index:012}"))
}
fn shadow_source_fixture() -> Result<(TempDir, SharedAppPaths)> {
    let dir = tempfile::tempdir()?;
    let paths = SharedAppPaths::from_app_data_dir(dir.path());
    let mut entries = Vec::new();
    for index in [1u8, 2u8] {
        let account_id = shadow_account_id(index)?;
        let account = paths.account(&account_id);
        fs::create_dir_all(account.root())?;
        write_manifest_atomic(
            &account.manifest(),
            &AccountManifest {
                version: 1,
                account_id: account_id.clone(),
                role_id: index as i64,
                battle_tag: format!("合成影子#{index:04}"),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        )?;
        OwDatabase::new(account.database()).initialize()?;
        let conn = OwDatabase::new(account.database()).connect()?;
        conn.execute("INSERT INTO hero_dict(guid,name,role,updated_at) VALUES('shadow-hero','合成英雄','damage','now') ON CONFLICT(guid) DO NOTHING",[])?;
        let match_id = format!("shadow-source-{index}");
        conn.execute("INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,24,'preset','SportPreset','normal','win','shadow-map','shadow-hero',?2,1,0,6,4,2,1000,0,0,0,600,'{\"roleType\":\"damage\",\"heroPlayTimeList\":[{\"heroGuid\":\"shadow-hero\",\"durationSec\":600}]}','now')",params![match_id,1000+index as i64])?;
        conn.execute("INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,kill,assist,death,hero_damage,raw_json,updated_at) VALUES(?1,24,'teammate',?2,'合成当前玩家','shadow-hero',1,6,4,2,1000,'{\"finalHit\":4}','now')",params![match_id,format!("shadow-current-{index}")])?;
        entries.push(AccountRegistryEntry {
            account_id,
            battle_tag: format!("合成影子#{index:04}"),
            created_at: "2026-01-01T00:00:00Z".into(),
            last_used_at: "2026-01-01T00:00:00Z".into(),
            credential_state: "ready".into(),
        });
    }
    write_registry_atomic(
        &paths.registry(),
        &AccountRegistry {
            version: 1,
            active_account_id: None,
            accounts: entries,
        },
    )?;
    Ok((dir, paths))
}

#[test]
fn full_multi_account_projection_and_shadow_release_every_query_only_source_unchanged() -> Result<()>
{
    let (_dir, paths) = shadow_source_fixture()?;
    let sources = discover_training_sources(&paths)?;
    let guards = sources
        .iter()
        .map(open_query_only)
        .collect::<Result<Vec<_>>>()?;
    let before=guards.iter().map(|conn|->Result<_>{Ok((conn.total_changes(),conn.query_row("SELECT COUNT(*),COALESCE(SUM(final_delta),0.0),COALESCE(SUM(performance_adjustment),0.0) FROM career_mmr_events",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,f64>(1)?,r.get::<_,f64>(2)?)))?))}).collect::<Result<Vec<_>>>()?;
    let projection = build_multi_account_projection(&sources)?;
    assert_eq!(projection.audit.source_accounts, 2);
    for account in ["dataset-001", "dataset-002"] {
        let rows = projection
            .rows
            .iter()
            .filter(|row| row.account_group == account)
            .cloned()
            .collect::<Vec<_>>();
        let summary = simulate_shadow_replay(&rows, &artifact())?;
        assert_eq!(summary.account_group, account);
    }
    let after=guards.iter().map(|conn|->Result<_>{Ok((conn.total_changes(),conn.query_row("SELECT COUNT(*),COALESCE(SUM(final_delta),0.0),COALESCE(SUM(performance_adjustment),0.0) FROM career_mmr_events",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,f64>(1)?,r.get::<_,f64>(2)?)))?))}).collect::<Result<Vec<_>>>()?;
    assert_eq!(after, before);
    drop(guards);
    drop(projection);
    for index in [1u8, 2u8] {
        let database = paths.account(&shadow_account_id(index)?).database();
        let moved = database.with_extension("shadow-move");
        fs::rename(&database, &moved)?;
        fs::rename(&moved, &database)?;
    }
    Ok(())
}
