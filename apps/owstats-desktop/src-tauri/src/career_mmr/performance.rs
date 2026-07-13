use crate::career_mmr::{input::ModeFamily, training::TrainingRow};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

const MIN_HERO_BASELINE_SAMPLES: usize = 30;
const MAD_SCALE: f64 = 0.6745;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealingTakenStrategy {
    None,
    DirectNegative,
    HeroNormalized,
    ExcessHealing,
    ResourceConversionDebt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeatureName {
    FinalHit,
    NonFinalEliminations,
    Assist,
    Death,
    Damage,
    Healing,
    HealingTaken,
    Mitigation,
    DamageTaken,
    ObjectiveTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineLevel {
    HeroModeSeason,
    HeroMode,
    Hero,
    RoleMode,
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeatureValues {
    pub final_hit: Option<f64>,
    pub non_final_eliminations: Option<f64>,
    pub assist: Option<f64>,
    pub death: Option<f64>,
    pub damage: Option<f64>,
    pub healing: Option<f64>,
    pub mitigation: Option<f64>,
    pub damage_taken: Option<f64>,
    pub objective_time: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixRow {
    pub match_group: String,
    pub hero_group: String,
    pub role: String,
    pub season: i64,
    pub begin_ts: i64,
    pub result: f64,
    pub expected_win_rate: f64,
    pub mode_family: ModeFamily,
    pub per_ten: FeatureValues,
    pub standardized: FeatureValues,
    pub baselines: BTreeMap<FeatureName, BaselineLevel>,
    pub healing_taken_derived: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoleMatrixAudit {
    pub role: String,
    pub training_rows: usize,
    pub validation_rows: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FeatureMatrixAudit {
    pub training_rows: usize,
    pub validation_rows: usize,
    pub excluded_rows: usize,
    pub overlap_excluded_rows: usize,
    pub missing_feature_values: usize,
    pub unavailable_baselines: usize,
    pub unsafe_healing_taken_excluded: usize,
    pub roles: Vec<RoleMatrixAudit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureMatrixResult {
    pub training_by_role: BTreeMap<String, Vec<MatrixRow>>,
    pub validation_by_role: BTreeMap<String, Vec<MatrixRow>>,
    pub audit: FeatureMatrixAudit,
}

#[derive(Clone, Copy)]
struct Baseline {
    median: f64,
    mad: f64,
}

pub fn build_feature_matrix(
    training: &[TrainingRow],
    validation: &[TrainingRow],
    strategy: HealingTakenStrategy,
) -> FeatureMatrixResult {
    let training_groups: HashSet<&str> = training
        .iter()
        .map(|row| row.match_group.as_str())
        .collect();
    let eligible_training: Vec<_> = training.iter().filter_map(prepared).collect();
    let baselines = fit_baselines(&eligible_training);
    let mut audit = FeatureMatrixAudit::default();
    let training_by_role = transform(&eligible_training, &baselines, strategy, true, &mut audit);
    audit.overlap_excluded_rows = validation
        .iter()
        .filter(|row| training_groups.contains(row.match_group.as_str()))
        .count();
    let eligible_validation: Vec<_> = validation
        .iter()
        .filter(|row| !training_groups.contains(row.match_group.as_str()))
        .filter_map(prepared)
        .collect();
    let validation_by_role = transform(
        &eligible_validation,
        &baselines,
        strategy,
        false,
        &mut audit,
    );
    audit.excluded_rows =
        training.len() + validation.len() - eligible_training.len() - eligible_validation.len();
    let mut roles: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (role, rows) in &training_by_role {
        roles.entry(role.clone()).or_default().0 = rows.len();
    }
    for (role, rows) in &validation_by_role {
        roles.entry(role.clone()).or_default().1 = rows.len();
    }
    audit.roles = roles
        .into_iter()
        .map(|(role, (training_rows, validation_rows))| RoleMatrixAudit {
            role,
            training_rows,
            validation_rows,
        })
        .collect();
    FeatureMatrixResult {
        training_by_role,
        validation_by_role,
        audit,
    }
}

struct Prepared<'a> {
    source: &'a TrainingRow,
    hero: &'a str,
    values: BTreeMap<FeatureName, Option<f64>>,
}

fn prepared(row: &TrainingRow) -> Option<Prepared<'_>> {
    if !row.allow_performance {
        return None;
    }
    let hero = row.hero_group.as_deref()?;
    let duration = row
        .hero_duration_sec
        .filter(|v| v.is_finite() && *v > 0.0)?;
    let rate = 600.0 / duration;
    let scaled = |value: Option<f64>| {
        value
            .filter(|v| v.is_finite())
            .map(|v| v * rate)
            .filter(|v| v.is_finite())
    };
    let non_final = match (row.kill, row.final_hit) {
        (Some(kill), Some(final_hit)) if kill.is_finite() && final_hit.is_finite() => {
            let value = (kill - final_hit).max(0.0) * rate;
            value.is_finite().then_some(value)
        }
        _ => None,
    };
    let values = BTreeMap::from([
        (FeatureName::FinalHit, scaled(row.final_hit)),
        (FeatureName::NonFinalEliminations, non_final),
        (FeatureName::Assist, scaled(row.assist)),
        (FeatureName::Death, scaled(row.death)),
        (FeatureName::Damage, scaled(row.damage)),
        (FeatureName::Healing, scaled(row.healing)),
        (FeatureName::HealingTaken, scaled(row.healing_taken)),
        (FeatureName::Mitigation, scaled(row.mitigation)),
        (FeatureName::DamageTaken, scaled(row.damage_taken)),
        (FeatureName::ObjectiveTime, scaled(row.objective_time_sec)),
    ]);
    Some(Prepared {
        source: row,
        hero,
        values,
    })
}

type BaselineKey = (
    String,
    FeatureName,
    Option<String>,
    Option<ModeFamily>,
    Option<i64>,
);

fn fit_baselines(rows: &[Prepared<'_>]) -> HashMap<BaselineKey, Vec<f64>> {
    let mut result: HashMap<BaselineKey, Vec<f64>> = HashMap::new();
    for row in rows {
        for (&feature, &value) in &row.values {
            let Some(value) = value else { continue };
            if feature == FeatureName::HealingTaken && !row.source.usable_for_resource_debt {
                continue;
            }
            let role = row.source.role.clone();
            for key in [
                (
                    role.clone(),
                    feature,
                    Some(row.hero.into()),
                    Some(row.source.mode_family),
                    Some(row.source.season),
                ),
                (
                    role.clone(),
                    feature,
                    Some(row.hero.into()),
                    Some(row.source.mode_family),
                    None,
                ),
                (role.clone(), feature, Some(row.hero.into()), None, None),
                (
                    role.clone(),
                    feature,
                    None,
                    Some(row.source.mode_family),
                    None,
                ),
            ] {
                result.entry(key).or_default().push(value);
            }
        }
    }
    result
}

fn choose_baseline(
    rows: &HashMap<BaselineKey, Vec<f64>>,
    row: &Prepared<'_>,
    feature: FeatureName,
) -> (Option<Baseline>, BaselineLevel) {
    let role = row.source.role.clone();
    let candidates = [
        (
            (
                role.clone(),
                feature,
                Some(row.hero.into()),
                Some(row.source.mode_family),
                Some(row.source.season),
            ),
            BaselineLevel::HeroModeSeason,
            true,
        ),
        (
            (
                role.clone(),
                feature,
                Some(row.hero.into()),
                Some(row.source.mode_family),
                None,
            ),
            BaselineLevel::HeroMode,
            true,
        ),
        (
            (role.clone(), feature, Some(row.hero.into()), None, None),
            BaselineLevel::Hero,
            true,
        ),
        (
            (role, feature, None, Some(row.source.mode_family), None),
            BaselineLevel::RoleMode,
            false,
        ),
    ];
    for (key, level, requires_minimum) in candidates {
        let Some(values) = rows.get(&key) else {
            continue;
        };
        if requires_minimum && values.len() < MIN_HERO_BASELINE_SAMPLES {
            continue;
        }
        if let Some(baseline) = robust_baseline(values) {
            return (Some(baseline), level);
        }
    }
    (None, BaselineLevel::Unavailable)
}

fn robust_baseline(values: &[f64]) -> Option<Baseline> {
    if values.is_empty() {
        return None;
    }
    let center = median(values.to_vec());
    let mad = median(values.iter().map(|value| (value - center).abs()).collect());
    (mad > f64::EPSILON).then_some(Baseline {
        median: center,
        mad,
    })
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn transform(
    rows: &[Prepared<'_>],
    baselines: &HashMap<BaselineKey, Vec<f64>>,
    strategy: HealingTakenStrategy,
    training: bool,
    audit: &mut FeatureMatrixAudit,
) -> BTreeMap<String, Vec<MatrixRow>> {
    let mut grouped = BTreeMap::new();
    for row in rows {
        if training {
            audit.training_rows += 1;
        } else {
            audit.validation_rows += 1;
        }
        let mut standardized = BTreeMap::new();
        let mut levels = BTreeMap::new();
        for (&feature, &value) in &row.values {
            if value.is_none() {
                audit.missing_feature_values += 1;
            }
            let (baseline, level) = choose_baseline(baselines, row, feature);
            let z = value.zip(baseline).map(|(value, baseline)| {
                (MAD_SCALE * (value - baseline.median) / baseline.mad).clamp(-3.0, 3.0)
            });
            if value.is_some() && z.is_none() {
                audit.unavailable_baselines += 1;
            }
            standardized.insert(feature, z);
            levels.insert(feature, level);
        }
        let safe = row.source.usable_for_resource_debt;
        let healing_raw = row.values[&FeatureName::HealingTaken];
        let healing_z = standardized[&FeatureName::HealingTaken];
        let healing_taken_derived = match strategy {
            HealingTakenStrategy::None => None,
            HealingTakenStrategy::DirectNegative if safe => healing_raw,
            HealingTakenStrategy::HeroNormalized if safe => healing_z,
            HealingTakenStrategy::ExcessHealing if safe => healing_z.map(|value| value.max(0.0)),
            // The final conversion formula belongs to model fitting; this slot carries its leak-free input.
            HealingTakenStrategy::ResourceConversionDebt if safe => healing_z,
            _ => {
                if healing_raw.is_some() {
                    audit.unsafe_healing_taken_excluded += 1;
                }
                None
            }
        };
        let feature_values = |source: &BTreeMap<FeatureName, Option<f64>>| FeatureValues {
            final_hit: source[&FeatureName::FinalHit],
            non_final_eliminations: source[&FeatureName::NonFinalEliminations],
            assist: source[&FeatureName::Assist],
            death: source[&FeatureName::Death],
            damage: source[&FeatureName::Damage],
            healing: source[&FeatureName::Healing],
            mitigation: source[&FeatureName::Mitigation],
            damage_taken: source[&FeatureName::DamageTaken],
            objective_time: source[&FeatureName::ObjectiveTime],
        };
        grouped
            .entry(row.source.role.clone())
            .or_insert_with(Vec::new)
            .push(MatrixRow {
                match_group: row.source.match_group.clone(),
                hero_group: row.hero.into(),
                role: row.source.role.clone(),
                season: row.source.season,
                begin_ts: row.source.begin_ts,
                result: row.source.result,
                expected_win_rate: row.source.expected_win_rate,
                mode_family: row.source.mode_family,
                per_ten: feature_values(&row.values),
                standardized: feature_values(&standardized),
                baselines: levels,
                healing_taken_derived,
            });
    }
    grouped
}
