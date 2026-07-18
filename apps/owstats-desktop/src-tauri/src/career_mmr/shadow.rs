//! Pure in-memory shadow replay for validated performance artifacts.

use super::{
    input::{ModeFamily, Role},
    performance::{
        predict_healing_expectation, BaselineLevel, FeatureName, FeatureValues,
        HealingTakenStrategy, MatrixRow,
    },
    performance_forward::{role_vector_with_multiplier, ModelVariant},
    rating::{
        bounded_performance_adjustment, update_rating_for_expected, RatingInput, RoleRatingState,
        INITIAL_MU,
    },
    training::{TrainingEnvironment, TrainingRow},
    training_report::{
        BaselineArtifact, HealingExpectationArtifact, HierarchicalModelArtifact,
        PerformanceModelArtifact,
    },
};
use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellKey {
    pub role: Role,
    pub mode: ModeFamily,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapHitSummary {
    pub upper: usize,
    pub lower: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShadowRoleSummary {
    pub role: Role,
    pub matches: usize,
    pub baseline_mmr: f64,
    pub shadow_mmr: f64,
    pub total_performance_adjustment: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShadowReplayMatch {
    /// Stable, replay-local ordinal; never a source match identifier.
    pub match_index: usize,
    pub role: Role,
    pub mode: ModeFamily,
    pub role_share: f64,
    pub base_delta: f64,
    pub performance_adjustment: f64,
    pub final_delta: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShadowAccountSummary {
    pub account_group: String,
    pub roles: Vec<ShadowRoleSummary>,
    pub cap_hits: BTreeMap<CellKey, CapHitSummary>,
    pub replayed_matches: Vec<ShadowReplayMatch>,
}

struct ReplayGroup<'a> {
    rows: Vec<&'a TrainingRow>,
    role: Role,
    mode: ModeFamily,
    begin_ts: i64,
    season: i64,
    match_group: String,
    role_share: f64,
    result: f64,
    expected: f64,
    quality: f64,
}

pub fn simulate_shadow_replay(
    rows: &[TrainingRow],
    artifact: &PerformanceModelArtifact,
) -> Result<ShadowAccountSummary> {
    let accounts = rows
        .iter()
        .map(|r| r.account_group.as_str())
        .collect::<BTreeSet<_>>();
    if accounts.len() != 1 {
        bail!("影子重放必须且只能接收一个匿名数据集");
    }
    let account_group = accounts.into_iter().next().unwrap_or_default().to_owned();
    let mut match_denominators = BTreeMap::<&str, f64>::new();
    let mut duration_sums = BTreeMap::<&str, f64>::new();
    for row in rows {
        let duration = row
            .hero_duration_sec
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(0.0);
        *duration_sums.entry(&row.match_group).or_default() += duration;
        let denominator = match_denominators.entry(&row.match_group).or_default();
        *denominator = denominator.max(row.duration_sec.max(0.0));
    }
    for (group, sum) in duration_sums {
        match_denominators
            .entry(group)
            .and_modify(|v| *v = v.max(sum));
    }

    let mut grouped = BTreeMap::<(String, Role), Vec<&TrainingRow>>::new();
    for row in rows {
        if let Some(role) = parse_role(&row.role) {
            grouped
                .entry((row.match_group.clone(), role))
                .or_default()
                .push(row);
        }
    }
    let mut replay = Vec::new();
    for ((match_group, role), mut fragments) in grouped {
        fragments.sort_by(|a, b| {
            a.hero_group
                .cmp(&b.hero_group)
                .then(a.source_match_group.cmp(&b.source_match_group))
        });
        let first = fragments[0];
        if fragments.iter().any(|row| {
            row.mode_family != first.mode_family
                || row.begin_ts != first.begin_ts
                || row.season != first.season
        }) {
            continue;
        }
        let valid = fragments
            .iter()
            .filter_map(|row| {
                row.hero_duration_sec
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .map(|d| (*row, d))
            })
            .collect::<Vec<_>>();
        let role_duration = valid.iter().map(|(_, d)| *d).sum::<f64>();
        let denominator = match_denominators
            .get(match_group.as_str())
            .copied()
            .unwrap_or(0.0);
        if role_duration <= 0.0 || denominator <= 0.0 {
            continue;
        }
        let weighted = |f: fn(&TrainingRow) -> f64| {
            valid.iter().map(|(r, d)| f(r) * d).sum::<f64>() / role_duration
        };
        let result = weighted(|r| r.result);
        let expected = weighted(|r| r.expected_win_rate);
        let quality = weighted(|r| r.data_quality).clamp(0.0, 1.0);
        replay.push(ReplayGroup {
            rows: valid.into_iter().map(|(r, _)| r).collect(),
            role,
            mode: first.mode_family,
            begin_ts: first.begin_ts,
            season: first.season,
            match_group,
            role_share: (role_duration / denominator).clamp(0.0, 1.0),
            result,
            expected,
            quality,
        });
    }
    replay.sort_by(|a, b| {
        a.begin_ts
            .cmp(&b.begin_ts)
            .then(a.match_group.cmp(&b.match_group))
            .then(a.role.cmp(&b.role))
    });
    let initial_ts = replay.first().map_or(1, |g| g.begin_ts.max(1));
    let initial_season = replay.first().map_or(0, |g| g.season);
    let mut baseline = BTreeMap::new();
    let mut shadow = BTreeMap::new();
    for role in [Role::Tank, Role::Damage, Role::Support] {
        baseline.insert(role, RoleRatingState::new(initial_ts, initial_season));
        shadow.insert(role, RoleRatingState::new(initial_ts, initial_season));
    }
    let mut counts = BTreeMap::<Role, usize>::new();
    let mut adjustments = BTreeMap::<Role, f64>::new();
    let mut cap_hits = BTreeMap::<CellKey, CapHitSummary>::new();
    let mut replayed_matches = Vec::new();
    let mut match_ordinals = BTreeMap::<String, usize>::new();
    for group in replay {
        let input = RatingInput {
            result: group.result,
            mode: group.mode,
            team_strength: None,
            opponent_strength: None,
            role_share: group.role_share,
            data_quality: group.quality,
            played_at_seconds: group.begin_ts,
            season: group.season,
        };
        let baseline_update =
            update_rating_for_expected(baseline[&group.role], input, group.expected, 0.0)?;
        let requested = requested_adjustment(&group, artifact);
        let bounded = bounded_performance_adjustment(
            baseline_update.base_delta,
            requested,
            group.role,
            group.mode,
        );
        let key = CellKey {
            role: group.role,
            mode: group.mode,
        };
        if requested.is_finite() && requested != bounded {
            let hits = cap_hits.entry(key).or_default();
            if requested > bounded {
                hits.upper += 1
            } else {
                hits.lower += 1
            }
        }
        let shadow_update =
            update_rating_for_expected(shadow[&group.role], input, group.expected, bounded)?;
        baseline.insert(group.role, baseline_update.new_state);
        shadow.insert(group.role, shadow_update.new_state);
        *counts.entry(group.role).or_default() += 1;
        *adjustments.entry(group.role).or_default() += shadow_update.performance_adjustment;
        let next = match_ordinals.len();
        let match_index = *match_ordinals.entry(group.match_group).or_insert(next);
        replayed_matches.push(ShadowReplayMatch {
            match_index,
            role: group.role,
            mode: group.mode,
            role_share: group.role_share,
            base_delta: shadow_update.base_delta,
            performance_adjustment: shadow_update.performance_adjustment,
            final_delta: shadow_update.delta,
        });
    }
    let roles = [Role::Tank, Role::Damage, Role::Support]
        .into_iter()
        .map(|role| ShadowRoleSummary {
            role,
            matches: counts.get(&role).copied().unwrap_or(0),
            baseline_mmr: baseline.get(&role).map_or(INITIAL_MU, |s| s.mu),
            shadow_mmr: shadow.get(&role).map_or(INITIAL_MU, |s| s.mu),
            total_performance_adjustment: adjustments.get(&role).copied().unwrap_or(0.0),
        })
        .collect();
    Ok(ShadowAccountSummary {
        account_group,
        roles,
        cap_hits,
        replayed_matches,
    })
}

fn requested_adjustment(group: &ReplayGroup<'_>, artifact: &PerformanceModelArtifact) -> f64 {
    let enabled = artifact
        .cells
        .iter()
        .any(|c| c.role == group.role && c.mode == group.mode && c.performance_enabled);
    if !enabled || group.mode == ModeFamily::StadiumQuick {
        return 0.0;
    }
    let Some(model) = artifact.roles.get(&group.role) else {
        return 0.0;
    };
    let p95 = artifact
        .cells
        .iter()
        .find(|cell| cell.role == group.role && cell.mode == group.mode)
        .map(|cell| cell.calibration_p95_abs_logit_effect)
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0);
    let mut weighted = 0.0;
    let mut duration = 0.0;
    for row in &group.rows {
        let d = row.hero_duration_sec.unwrap_or(0.0);
        if let Some(effect) = artifact_logit_effect(row, group.role, model, &artifact.normalization)
        {
            weighted += effect * d;
            duration += d;
        }
    }
    if duration == 0.0 {
        return 0.0;
    }
    (weighted / duration / p95).clamp(-1.0, 1.0)
}

pub(crate) fn artifact_logit_effect(
    row: &TrainingRow,
    role: Role,
    model: &HierarchicalModelArtifact,
    normalization: &super::training_report::NormalizationArtifact,
) -> Option<f64> {
    let strategy = parse_strategy(model.selected_strategy.as_deref()?)?;
    let variant = parse_variant(model.selected_variant.as_deref()?)?;
    let matrix = matrix_row_from_artifact(row, strategy, normalization)?;
    let final_mult = model.final_hit_multiplier.unwrap_or(1.0);
    let vector = role_vector_with_multiplier(&matrix, role, variant, strategy, final_mult);
    let offsets = model
        .training_cohort_offsets
        .get(&row.mode_family.training_cohort());
    let effect = vector
        .names
        .into_iter()
        .zip(vector.values)
        .map(|(name, value)| {
            (model.feature_weights.get(name).copied().unwrap_or(0.0)
                + offsets.and_then(|o| o.get(name)).copied().unwrap_or(0.0))
                * value
        })
        .sum::<f64>();
    effect.is_finite().then_some(effect)
}

pub fn matrix_row_from_artifact(
    row: &TrainingRow,
    strategy: HealingTakenStrategy,
    normalization: &super::training_report::NormalizationArtifact,
) -> Option<MatrixRow> {
    if !row.allow_performance || row.outcome_only || !row.mode_family.trains_performance() {
        return None;
    }
    let hero = row.hero_group.as_deref()?;
    let duration = row
        .hero_duration_sec
        .filter(|value| value.is_finite() && *value > 0.0)?;
    let scale = 600.0 / duration;
    let scaled = |value: Option<f64>| {
        value
            .filter(|v| v.is_finite())
            .map(|v| v * scale)
            .filter(|v| v.is_finite())
    };
    let raw = BTreeMap::from([
        (FeatureName::FinalHit, scaled(row.final_hit)),
        (
            FeatureName::NonFinalEliminations,
            row.kill
                .zip(row.final_hit)
                .filter(|(k, f)| k.is_finite() && f.is_finite())
                .map(|(k, f)| (k - f).max(0.0) * scale)
                .filter(|v| v.is_finite()),
        ),
        (FeatureName::Assist, scaled(row.assist)),
        (FeatureName::Death, scaled(row.death)),
        (FeatureName::Damage, scaled(row.damage)),
        (FeatureName::Healing, scaled(row.healing)),
        (FeatureName::HealingTaken, scaled(row.healing_taken)),
        (FeatureName::Mitigation, scaled(row.mitigation)),
        (FeatureName::DamageTaken, scaled(row.damage_taken)),
        (FeatureName::ObjectiveTime, scaled(row.objective_time_sec)),
        (
            FeatureName::AvailableTeamHealing,
            scaled(row.available_team_healing),
        ),
    ]);
    let mut standardized = BTreeMap::new();
    let mut levels = BTreeMap::new();
    for (&feature, &value) in &raw {
        let (baseline, level) = choose_feature_baseline(row, feature, normalization);
        let z = value.zip(baseline).map(|(value, baseline)| {
            (normalization.mad_scale * (value - baseline.median_per_ten) / baseline.mad_per_ten)
                .clamp(-normalization.z_clip, normalization.z_clip)
        });
        standardized.insert(feature, z);
        levels.insert(feature, level);
    }
    let safe = row.usable_for_resource_debt;
    let healing_raw = raw[&FeatureName::HealingTaken];
    let healing_z = standardized[&FeatureName::HealingTaken];
    let expected_healing = choose_expectation_for_level(
        row,
        levels[&FeatureName::HealingTaken],
        &normalization.healing_expectations,
    )
    .map(|expectation| {
        predict_healing_expectation(
            expectation.coefficients,
            expectation.fallback_median_per_ten,
            [
                (row.duration_sec / 600.0).max(0.0),
                standardized[&FeatureName::DamageTaken].unwrap_or(0.0),
                standardized[&FeatureName::Death].unwrap_or(0.0),
                standardized[&FeatureName::AvailableTeamHealing].unwrap_or(0.0),
            ],
        )
    });
    let excess = healing_raw
        .zip(expected_healing)
        .map(|(actual, expected)| (actual - expected).max(0.0));
    let conversion = [
        FeatureName::FinalHit,
        FeatureName::NonFinalEliminations,
        FeatureName::Assist,
        FeatureName::ObjectiveTime,
    ]
    .into_iter()
    .map(|feature| standardized[&feature].map(|v| v.max(0.0)))
    .collect::<Option<Vec<_>>>()
    .map(|values| values.into_iter().sum::<f64>() / 12.0);
    let healing_taken_derived = match strategy {
        HealingTakenStrategy::None => None,
        HealingTakenStrategy::DirectNegative if safe => healing_raw,
        HealingTakenStrategy::HeroNormalized if safe => healing_z,
        HealingTakenStrategy::ExcessHealing if safe => excess,
        HealingTakenStrategy::ResourceConversionDebt if safe => excess
            .zip(conversion)
            .map(|(excess, conversion)| excess * (1.0 - conversion.clamp(0.0, 1.0)).max(0.0)),
        _ => None,
    };
    let make_values = |source: &BTreeMap<FeatureName, Option<f64>>, derived: bool| {
        let death = source[&FeatureName::Death];
        let damage_taken = source[&FeatureName::DamageTaken];
        let outputs = [
            FeatureName::FinalHit,
            FeatureName::NonFinalEliminations,
            FeatureName::Assist,
            FeatureName::ObjectiveTime,
        ];
        let frontline_conversion = derived
            .then(|| {
                Some(
                    damage_taken?.max(0.0)
                        * (0.5 + 0.5 * (-death? / 3.0).clamp(0.0, 1.0))
                        * (outputs
                            .into_iter()
                            .map(|f| source[&f].map(|v| v.max(0.0)))
                            .collect::<Option<Vec<_>>>()?
                            .into_iter()
                            .sum::<f64>()
                            / 4.0),
                )
            })
            .flatten()
            .filter(|_| row.role == "tank");
        let survival = derived
            .then(|| death.map(|v| (-v).clamp(-3.0, 3.0)))
            .flatten()
            .filter(|_| row.role == "support");
        let abnormal_damage_taken = derived
            .then(|| {
                damage_taken
                    .zip(death)
                    .map(|(d, x)| d.max(0.0) * x.max(0.0))
            })
            .flatten();
        FeatureValues {
            final_hit: source[&FeatureName::FinalHit],
            non_final_eliminations: source[&FeatureName::NonFinalEliminations],
            assist: source[&FeatureName::Assist],
            death,
            damage: source[&FeatureName::Damage],
            healing: source[&FeatureName::Healing],
            mitigation: source[&FeatureName::Mitigation],
            damage_taken,
            objective_time: source[&FeatureName::ObjectiveTime],
            frontline_conversion,
            survival,
            abnormal_damage_taken,
        }
    };
    let damage_baseline_median_per_ten =
        choose_feature_baseline(row, FeatureName::Damage, normalization)
            .0
            .map(|b| b.median_per_ten);
    Some(MatrixRow {
        match_group: row.match_group.clone(),
        hero_group: hero.into(),
        role: row.role.clone(),
        season: row.season,
        begin_ts: row.begin_ts,
        result: row.result,
        expected_win_rate: row.expected_win_rate,
        data_quality: row.data_quality,
        mode_family: row.mode_family,
        per_ten: make_values(&raw, false),
        standardized: make_values(&standardized, true),
        baselines: levels,
        healing_taken_derived,
        damage_baseline_median_per_ten,
    })
}

fn parse_strategy(value: &str) -> Option<HealingTakenStrategy> {
    match value {
        "none" => Some(HealingTakenStrategy::None),
        "directnegative" => Some(HealingTakenStrategy::DirectNegative),
        "heronormalized" => Some(HealingTakenStrategy::HeroNormalized),
        "excesshealing" => Some(HealingTakenStrategy::ExcessHealing),
        "resourceconversiondebt" => Some(HealingTakenStrategy::ResourceConversionDebt),
        _ => None,
    }
}
fn parse_variant(value: &str) -> Option<ModelVariant> {
    match value {
        "core" => Some(ModelVariant::Core),
        "role_full" => Some(ModelVariant::RoleFull),
        _ => None,
    }
}
fn feature_name(feature: FeatureName) -> &'static str {
    match feature {
        FeatureName::FinalHit => "final_hit",
        FeatureName::NonFinalEliminations => "non_final_eliminations",
        FeatureName::Assist => "assist",
        FeatureName::Death => "death",
        FeatureName::Damage => "damage",
        FeatureName::Healing => "healing",
        FeatureName::HealingTaken => "healing_taken",
        FeatureName::Mitigation => "mitigation",
        FeatureName::DamageTaken => "damage_taken",
        FeatureName::ObjectiveTime => "objective_time",
        FeatureName::AvailableTeamHealing => "available_team_healing",
    }
}
fn choose_feature_baseline<'a>(
    row: &TrainingRow,
    feature: FeatureName,
    normalization: &'a super::training_report::NormalizationArtifact,
) -> (Option<&'a BaselineArtifact>, BaselineLevel) {
    let hero = row.hero_group.as_deref();
    let candidates = [
        (
            hero,
            Some(row.mode_family),
            Some(row.season),
            true,
            BaselineLevel::HeroModeSeason,
        ),
        (
            hero,
            Some(row.mode_family),
            None,
            true,
            BaselineLevel::HeroMode,
        ),
        (hero, None, None, true, BaselineLevel::HeroEnvironment),
        (
            None,
            Some(row.mode_family),
            None,
            false,
            BaselineLevel::RoleMode,
        ),
        (None, None, None, false, BaselineLevel::RoleEnvironment),
    ];
    for (h, m, s, needs, level) in candidates {
        if let Some(b) = normalization.baselines.iter().find(|b| {
            b.role == row.role
                && b.feature == feature_name(feature)
                && b.environment == environment(row.environment)
                && b.hero.as_deref() == h
                && b.mode == m
                && b.season == s
                && (!needs || b.sample_count >= normalization.minimum_hero_samples)
                && b.median_per_ten.is_finite()
                && b.mad_per_ten.is_finite()
                && b.mad_per_ten > 0.0
        }) {
            return (Some(b), level);
        }
    }
    (None, BaselineLevel::Unavailable)
}

fn choose_expectation_for_level<'a>(
    row: &TrainingRow,
    level: BaselineLevel,
    all: &'a [HealingExpectationArtifact],
) -> Option<&'a HealingExpectationArtifact> {
    let hero = row.hero_group.as_deref();
    let (hero, mode, season) = match level {
        BaselineLevel::HeroModeSeason => (hero, Some(row.mode_family), Some(row.season)),
        BaselineLevel::HeroMode => (hero, Some(row.mode_family), None),
        BaselineLevel::HeroEnvironment => (hero, None, None),
        BaselineLevel::RoleMode => (None, Some(row.mode_family), None),
        BaselineLevel::RoleEnvironment => (None, None, None),
        BaselineLevel::Unavailable => return None,
    };
    all.iter().find(|expectation| {
        expectation.role == row.role
            && expectation.environment == environment(row.environment)
            && expectation.hero.as_deref() == hero
            && expectation.mode == mode
            && expectation.season == season
    })
}
fn environment(value: TrainingEnvironment) -> &'static str {
    match value {
        TrainingEnvironment::Ordinary => "ordinary",
        TrainingEnvironment::Stadium => "stadium",
    }
}
fn parse_role(value: &str) -> Option<Role> {
    match value {
        "tank" => Some(Role::Tank),
        "damage" => Some(Role::Damage),
        "support" => Some(Role::Support),
        _ => None,
    }
}
