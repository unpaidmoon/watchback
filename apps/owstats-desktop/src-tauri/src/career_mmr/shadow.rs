//! Pure in-memory shadow replay for validated performance artifacts.

use super::{
    input::{ModeFamily, Role},
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
    let p95 = model
        .calibration_p95_abs_effect
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0);
    let mut weighted = 0.0;
    let mut duration = 0.0;
    for row in &group.rows {
        let d = row.hero_duration_sec.unwrap_or(0.0);
        if let Some(effect) = artifact_logit_effect(row, group.role, model, artifact) {
            weighted += effect * d;
            duration += d;
        }
    }
    if duration == 0.0 {
        return 0.0;
    }
    (weighted / duration / p95).clamp(-1.0, 1.0)
}

fn artifact_logit_effect(
    row: &TrainingRow,
    role: Role,
    model: &HierarchicalModelArtifact,
    artifact: &PerformanceModelArtifact,
) -> Option<f64> {
    let strategy = model.selected_strategy.as_deref()?;
    let variant = model.selected_variant.as_deref()?;
    let mut z = BTreeMap::<&str, f64>::new();
    let mut per_ten = BTreeMap::<&str, f64>::new();
    for (name, value) in raw_features(row) {
        if let Some(raw) = value.filter(|v| v.is_finite()) {
            let d = row.hero_duration_sec?;
            if d <= 0.0 {
                return None;
            }
            let value = raw * 600.0 / d;
            per_ten.insert(name, value);
            if let Some(b) = choose_baseline(
                row,
                name,
                &artifact.normalization.baselines,
                artifact.normalization.minimum_hero_samples,
            ) {
                z.insert(
                    name,
                    (artifact.normalization.mad_scale * (value - b.median_per_ten)
                        / b.mad_per_ten.max(artifact.normalization.mad_floor_absolute))
                    .clamp(
                        -artifact.normalization.z_clip,
                        artifact.normalization.z_clip,
                    ),
                );
            }
        }
    }
    let final_z = *z.get("final_hit").unwrap_or(&0.0);
    let nonfinal_z = *z.get("non_final_eliminations").unwrap_or(&0.0);
    let assist_z = *z.get("assist").unwrap_or(&0.0);
    let death_z = *z.get("death").unwrap_or(&0.0);
    let damage_taken = *z.get("damage_taken").unwrap_or(&0.0);
    let objective = *z.get("objective_time").unwrap_or(&0.0);
    let final_mult = model.final_hit_multiplier.unwrap_or(1.0);
    let damage_conversion = {
        let damage = *per_ten.get("damage").unwrap_or(&0.0);
        let median = choose_baseline(
            row,
            "damage",
            &artifact.normalization.baselines,
            artifact.normalization.minimum_hero_samples,
        )
        .map_or(0.0, |b| b.median_per_ten);
        (per_ten.get("final_hit").copied().unwrap_or(0.0) * final_mult
            + per_ten
                .get("non_final_eliminations")
                .copied()
                .unwrap_or(0.0))
            / damage.max(0.25 * median).max(f64::EPSILON)
    };
    let frontline = damage_taken.max(0.0)
        * (0.5 + 0.5 * (-death_z / 3.0).clamp(0.0, 1.0))
        * ([
            final_z.max(0.0),
            nonfinal_z.max(0.0),
            assist_z.max(0.0),
            objective.max(0.0),
        ]
        .iter()
        .sum::<f64>()
            / 4.0);
    let abnormal = damage_taken.max(0.0) * death_z.max(0.0);
    let survival = (-death_z).clamp(-3.0, 3.0);
    let mut values = BTreeMap::from([
        ("final", final_z),
        ("nonfinal", nonfinal_z),
        ("assist", assist_z),
        ("death", death_z),
    ]);
    if variant == "role_full" {
        match role {
            Role::Tank => {
                values.insert("frontline_conversion", frontline);
                values.insert("mitigation", *z.get("mitigation").unwrap_or(&0.0));
                values.insert("abnormal_damage_taken", abnormal);
            }
            Role::Damage => {
                values.insert("damage_conversion", damage_conversion);
            }
            Role::Support => {
                values.insert("healing", *z.get("healing").unwrap_or(&0.0));
                values.insert("damage", *z.get("damage").unwrap_or(&0.0));
                values.insert("survival", survival);
                values.insert("abnormal_damage_taken", abnormal);
            }
        }
    }
    if strategy != "none" {
        values.insert(
            "debt",
            healing_debt(
                row,
                strategy,
                &z,
                &per_ten,
                &artifact.normalization.healing_expectations,
                artifact,
            ),
        );
    }
    let offsets = model
        .training_cohort_offsets
        .get(&row.mode_family.training_cohort());
    let effect = values
        .into_iter()
        .map(|(name, value)| {
            (model.feature_weights.get(name).copied().unwrap_or(0.0)
                + offsets.and_then(|o| o.get(name)).copied().unwrap_or(0.0))
                * value
        })
        .sum::<f64>();
    effect.is_finite().then_some(effect)
}

fn raw_features(row: &TrainingRow) -> [(&'static str, Option<f64>); 11] {
    [
        ("final_hit", row.final_hit),
        (
            "non_final_eliminations",
            row.kill.zip(row.final_hit).map(|(k, f)| (k - f).max(0.0)),
        ),
        ("assist", row.assist),
        ("death", row.death),
        ("damage", row.damage),
        ("healing", row.healing),
        ("healing_taken", row.healing_taken),
        ("mitigation", row.mitigation),
        ("damage_taken", row.damage_taken),
        ("objective_time", row.objective_time_sec),
        ("available_team_healing", row.available_team_healing),
    ]
}

fn choose_baseline<'a>(
    row: &TrainingRow,
    feature: &str,
    all: &'a [BaselineArtifact],
    min: usize,
) -> Option<&'a BaselineArtifact> {
    let env = environment(row.environment);
    let hero = row.hero_group.as_deref();
    let candidates = [
        (hero, Some(row.mode_family), Some(row.season), true),
        (hero, Some(row.mode_family), None, true),
        (hero, None, None, true),
        (None, Some(row.mode_family), None, false),
        (None, None, None, false),
    ];
    candidates.into_iter().find_map(|(h, m, s, needs)| {
        all.iter().find(|b| {
            b.role == row.role
                && b.feature == feature
                && b.environment == env
                && b.hero.as_deref() == h
                && b.mode == m
                && b.season == s
                && (!needs || b.sample_count >= min)
                && b.median_per_ten.is_finite()
                && b.mad_per_ten.is_finite()
                && b.mad_per_ten > 0.0
        })
    })
}

fn healing_debt(
    row: &TrainingRow,
    strategy: &str,
    z: &BTreeMap<&str, f64>,
    raw: &BTreeMap<&str, f64>,
    expectations: &[HealingExpectationArtifact],
    artifact: &PerformanceModelArtifact,
) -> f64 {
    if !row.usable_for_resource_debt {
        return 0.0;
    }
    let actual = raw.get("healing_taken").copied().unwrap_or(0.0);
    if strategy == "directnegative" {
        return (actual / 10_000.0).clamp(0.0, 3.0);
    }
    if strategy == "heronormalized" {
        return z.get("healing_taken").copied().unwrap_or(0.0).max(0.0);
    }
    let expectation = choose_expectation(
        row,
        expectations,
        artifact.normalization.minimum_hero_samples,
    );
    let expected = expectation.map_or(0.0, |e| {
        e.coefficients.map_or(e.fallback_median_per_ten, |beta| {
            let x = [
                (row.duration_sec / 600.0).max(0.0),
                z.get("damage_taken").copied().unwrap_or(0.0),
                z.get("death").copied().unwrap_or(0.0),
                z.get("available_team_healing").copied().unwrap_or(0.0),
            ];
            beta[0] + beta[1] * x[0] + beta[2] * x[1] + beta[3] * x[2] + beta[4] * x[3]
        })
    });
    let excess = (actual - expected).max(0.0);
    if strategy == "excesshealing" {
        return excess;
    }
    let conversion = [
        "final_hit",
        "non_final_eliminations",
        "assist",
        "objective_time",
    ]
    .iter()
    .map(|n| z.get(n).copied().unwrap_or(0.0).max(0.0))
    .sum::<f64>()
        / 12.0;
    excess * (1.0 - conversion.clamp(0.0, 1.0)).max(0.0)
}
fn choose_expectation<'a>(
    row: &TrainingRow,
    all: &'a [HealingExpectationArtifact],
    min: usize,
) -> Option<&'a HealingExpectationArtifact> {
    let env = environment(row.environment);
    let hero = row.hero_group.as_deref();
    [
        (hero, Some(row.mode_family), Some(row.season), true),
        (hero, Some(row.mode_family), None, true),
        (hero, None, None, true),
        (None, Some(row.mode_family), None, false),
        (None, None, None, false),
    ]
    .into_iter()
    .find_map(|(h, m, s, needs)| {
        all.iter().find(|e| {
            e.role == row.role
                && e.environment == env
                && e.hero.as_deref() == h
                && e.mode == m
                && e.season == s
                && (!needs || e.sample_count >= min)
        })
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
