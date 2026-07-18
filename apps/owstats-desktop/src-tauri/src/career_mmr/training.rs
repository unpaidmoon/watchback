use crate::career_mmr::input::{
    normalize_mode_family, normalize_role as normalize_input_role, stadium_final_snapshot,
    ExtractionContext, ModeFamily, TrainingCohort,
};
use anyhow::{bail, Result};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingRow {
    pub match_group: String,
    pub season: i64,
    pub begin_ts: i64,
    pub role: String,
    pub mode_family: ModeFamily,
    pub hero_group: Option<String>,
    pub outcome_only: bool,
    pub allow_performance: bool,
    pub allow_performance_by_hero: bool,
    pub result: f64,
    pub expected_win_rate: f64,
    pub data_quality: f64,
    pub final_hit: Option<f64>,
    pub kill: Option<f64>,
    pub assist: Option<f64>,
    pub death: Option<f64>,
    pub damage: Option<f64>,
    pub healing: Option<f64>,
    pub mitigation: Option<f64>,
    pub healing_taken: Option<f64>,
    pub damage_taken: Option<f64>,
    pub objective_time_sec: Option<f64>,
    pub duration_sec: f64,
    pub hero_duration_sec: Option<f64>,
    pub healing_taken_semantic: &'static str,
    pub usable_for_resource_debt: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingProjection {
    pub rows: Vec<TrainingRow>,
    pub audit: TrainingAudit,
}

impl TrainingProjection {
    pub fn public_audit(&self) -> &TrainingAudit {
        &self.audit
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoleTrainingAudit {
    pub role: String,
    pub samples: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FeatureCoverage {
    pub final_hit: usize,
    pub kill: usize,
    pub assist: usize,
    pub death: usize,
    pub damage: usize,
    pub healing: usize,
    pub mitigation: usize,
    pub healing_taken: usize,
    pub damage_taken: usize,
    pub objective_time: usize,
    pub duration: usize,
    pub exact_hero_duration: usize,
    pub final_hero: usize,
    pub hero_final_hit: usize,
    pub hero_kill: usize,
    pub hero_assist: usize,
    pub hero_death: usize,
    pub hero_damage: usize,
    pub hero_healing: usize,
    pub hero_objective_time: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HealingTakenSemanticsAudit {
    pub explicit_self_healing_rows: usize,
    pub explicit_external_healing_rows: usize,
    pub unknown_rows: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TrainingAudit {
    pub ordinary_candidates: usize,
    pub stadium_competitive_candidates: usize,
    pub stadium_quick_outcome_only_rows: usize,
    pub stadium_performance_eligible_rows: usize,
    pub projected_rows: usize,
    pub performance_eligible_rows: usize,
    pub resource_debt_eligible_rows: usize,
    pub minor_segment_rows: usize,
    pub stadium_ambiguous_excluded: usize,
    pub roles: Vec<RoleTrainingAudit>,
    pub coverage: FeatureCoverage,
    pub healing_taken_semantics: HealingTakenSemanticsAudit,
}

struct RawRow {
    match_id: String,
    season: i64,
    begin_ts: i64,
    dictionary_role: String,
    hero: String,
    result: String,
    game_mode: String,
    player: Value,
    duration: i64,
    entry: String,
    detail: String,
    bucket: String,
    current_player_key: Option<String>,
}

#[derive(Debug)]
struct HeroSegment {
    hero: String,
    duration_sec: f64,
    use_rate: Option<f64>,
    stats: Value,
}

const STAT_KILL: &str = "603482350067646495";
const STAT_FINAL_HIT: &str = "603482350067646507";
const STAT_DEATH: &str = "603482350067646506";
const STAT_ASSIST: &str = "603482350067648392";
const STAT_HERO_DAMAGE: &str = "603482350067647671";
const STAT_HEALING_PRIMARY: &str = "603482350067646913";
const STAT_HEALING_FALLBACK: &str = "603482350067647479";
const STAT_OBJECTIVE_TIME: &str = "603482350067648534";
const STADIUM_PERFORMANCE_FIELDS: &[&str] = &[
    "kill",
    "assist",
    "death",
    "heroDamage",
    "cure",
    "resistDamage",
    "finalHit",
    "targetCompetingTime",
    "healingTaken",
    "damageTaken",
];

pub fn build_training_projection(conn: &Connection) -> Result<TrainingProjection> {
    let mut active_events = HashMap::new();
    if table_exists(conn, "career_mmr_events")? && table_exists(conn, "mmr_model_versions")? {
        let mut event_stmt = conn.prepare("SELECT e.match_id,e.role,e.expected_win_rate,e.data_quality FROM career_mmr_events e JOIN mmr_model_versions v ON v.version=e.model_version WHERE v.is_active=1")?;
        for item in event_stmt.query_map([], |q| {
            Ok((
                q.get::<_, String>(0)?,
                q.get::<_, String>(1)?,
                q.get::<_, f64>(2)?,
                q.get::<_, f64>(3)?,
            ))
        })? {
            let (match_id, role, expected, quality) = item?;
            if active_events
                .insert((match_id, role), (expected, quality))
                .is_some()
            {
                bail!("存在重复的 active MMR 基准事件");
            }
        }
    }
    let ordinary_candidates = conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE bucket<>'stadium'",
        [],
        |row| row.get::<_, usize>(0),
    )?;
    let mut stmt = conn.prepare(
        r#"SELECT m.match_id,m.season,m.begin_ts,COALESCE(d.role,''),m.hero_guid,m.result,m.game_mode,
                  m.duration_sec,
                  m.raw_entry_json,COALESCE((SELECT p.raw_json FROM match_players p
                    WHERE p.match_id=m.match_id AND p.is_current_player=1
                    AND (SELECT COUNT(*) FROM match_players x WHERE x.match_id=m.match_id AND x.is_current_player=1)=1 LIMIT 1),'{}'),
                  COALESCE(md.raw_detail_json,'{}')
           FROM matches m
           LEFT JOIN hero_dict d ON d.guid=m.hero_guid
           LEFT JOIN match_details md ON md.match_id=m.match_id
           WHERE m.bucket<>'stadium'
           ORDER BY m.begin_ts,m.season,m.match_id"#,
    )?;
    let mut raw: Vec<RawRow> = stmt
        .query_map([], |r| {
            Ok(RawRow {
                match_id: r.get(0)?,
                season: r.get(1)?,
                begin_ts: r.get(2)?,
                dictionary_role: r.get(3)?,
                hero: r.get(4)?,
                result: r.get(5)?,
                game_mode: r.get(6)?,
                duration: r.get(7)?,
                entry: r.get(8)?,
                player: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or(Value::Null),
                detail: r.get(10)?,
                bucket: "ordinary".into(),
                current_player_key: None,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let stadium_candidates = conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE bucket='stadium'",
        [],
        |row| row.get::<_, usize>(0),
    )?;
    if stadium_candidates > 0 {
        let mut stadium_stmt = conn.prepare(
            r#"SELECT m.match_id,m.season,m.begin_ts,COALESCE(d.role,''),m.hero_guid,m.result,m.game_mode,
                      m.duration_sec,m.raw_entry_json,'{}',COALESCE(md.raw_detail_json,'{}'),m.bucket,
                      (SELECT p.bnet_id FROM match_players p
                        WHERE p.match_id=m.match_id AND p.is_current_player=1
                        AND (SELECT COUNT(*) FROM match_players x WHERE x.match_id=m.match_id AND x.is_current_player=1)=1 LIMIT 1)
               FROM matches m
               LEFT JOIN hero_dict d ON d.guid=m.hero_guid
               LEFT JOIN match_details md ON md.match_id=m.match_id
               WHERE m.bucket='stadium'
               ORDER BY m.begin_ts,m.season,m.match_id"#,
        )?;
        raw.extend(
            stadium_stmt
                .query_map([], |r| {
                    Ok(RawRow {
                        match_id: r.get(0)?,
                        season: r.get(1)?,
                        begin_ts: r.get(2)?,
                        dictionary_role: r.get(3)?,
                        hero: r.get(4)?,
                        result: r.get(5)?,
                        game_mode: r.get(6)?,
                        duration: r.get(7)?,
                        entry: r.get(8)?,
                        player: Value::Null,
                        detail: r.get(10)?,
                        bucket: r.get(11)?,
                        current_player_key: r.get(12)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        );
    }

    let mut audit = TrainingAudit {
        ordinary_candidates,
        ..Default::default()
    };
    let extraction_context = (stadium_candidates > 0)
        .then(|| ExtractionContext::build(conn))
        .transpose()?;
    let mut role_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut hero_roles: HashMap<String, String> = {
        let mut stmt = conn.prepare("SELECT guid,role FROM hero_dict WHERE role IS NOT NULL")?;
        let roles = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        roles
    };
    let mut inferred_role_counts: HashMap<(String, String), usize> = HashMap::new();
    for item in &raw {
        if item.bucket == "stadium" {
            continue;
        }
        let entry = serde_json::from_str::<Value>(&item.entry).unwrap_or(Value::Null);
        if let Some(role) = entry
            .get("roleType")
            .and_then(Value::as_str)
            .and_then(normalize_role)
        {
            *inferred_role_counts
                .entry((item.hero.clone(), role.to_string()))
                .or_default() += 1;
        }
    }
    let mut inferred_by_hero: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    for ((hero, role), count) in inferred_role_counts {
        inferred_by_hero
            .entry(hero)
            .or_default()
            .push((role, count));
    }
    for (hero, roles) in inferred_by_hero {
        if hero_roles.contains_key(&hero) {
            continue;
        }
        if let Some((role, _)) = roles
            .into_iter()
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
        {
            hero_roles.insert(hero, role);
        }
    }
    let mut rows = Vec::new();
    for r in raw {
        let entry = serde_json::from_str::<Value>(&r.entry).unwrap_or(Value::Null);
        let detail = serde_json::from_str::<Value>(&r.detail).unwrap_or(Value::Null);
        let Some(mode_family) = normalize_mode_family(
            &r.game_mode,
            entry.get("instanceType").and_then(Value::as_str),
        ) else {
            audit.stadium_ambiguous_excluded += usize::from(r.bucket == "stadium");
            continue;
        };
        let Some(result) = normalize_result(&r.result) else {
            audit.stadium_ambiguous_excluded += usize::from(r.bucket == "stadium");
            continue;
        };
        if r.bucket == "stadium" {
            if mode_family == ModeFamily::StadiumCompetitive {
                audit.stadium_competitive_candidates += 1;
            }
            let rounds = detail
                .pointer("/payload/data/roundCountList")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let snapshot = r.current_player_key.as_deref().and_then(|key| {
                stadium_final_snapshot(rounds, key, extraction_context.as_ref().unwrap())
            });
            let fallback_role = entry
                .get("roleType")
                .and_then(Value::as_str)
                .and_then(|role| normalize_input_role(Some(role)));
            let (role, hero, statistics, base_quality) = if let Some(snapshot) = snapshot {
                (
                    snapshot.role,
                    Some(snapshot.hero_guid),
                    Some(snapshot.statistics),
                    0.9,
                )
            } else if let Some(role) = fallback_role {
                (role, None, None, 0.35)
            } else {
                audit.stadium_ambiguous_excluded += 1;
                continue;
            };
            let training_cohort = mode_family.training_cohort();
            let stats = statistics.as_ref();
            let has_performance_fields = stats.is_some_and(|value| {
                STADIUM_PERFORMANCE_FIELDS
                    .iter()
                    .any(|key| numeric(value, key).is_some())
            });
            let allow_performance = mode_family.trains_performance()
                && training_cohort == TrainingCohort::StadiumCompetitive
                && r.duration > 0
                && has_performance_fields;
            let outcome_only = training_cohort == TrainingCohort::OutcomeOnly || !allow_performance;
            let healing_taken = stats.and_then(|value| numeric(value, "healingTaken"));
            let allow_resource_debt = allow_performance && healing_taken.is_some();
            audit.coverage.final_hit +=
                usize::from(stats.and_then(|value| numeric(value, "finalHit")).is_some());
            audit.coverage.kill +=
                usize::from(stats.and_then(|value| numeric(value, "kill")).is_some());
            audit.coverage.assist +=
                usize::from(stats.and_then(|value| numeric(value, "assist")).is_some());
            audit.coverage.death +=
                usize::from(stats.and_then(|value| numeric(value, "death")).is_some());
            audit.coverage.damage += usize::from(
                stats
                    .and_then(|value| numeric(value, "heroDamage"))
                    .is_some(),
            );
            audit.coverage.healing +=
                usize::from(stats.and_then(|value| numeric(value, "cure")).is_some());
            audit.coverage.mitigation += usize::from(
                stats
                    .and_then(|value| numeric(value, "resistDamage"))
                    .is_some(),
            );
            audit.coverage.healing_taken += usize::from(healing_taken.is_some());
            audit.coverage.damage_taken += usize::from(
                stats
                    .and_then(|value| numeric(value, "damageTaken"))
                    .is_some(),
            );
            audit.coverage.objective_time += usize::from(
                stats
                    .and_then(|value| numeric(value, "targetCompetingTime"))
                    .is_some(),
            );
            audit.coverage.duration += usize::from(r.duration > 0);
            audit.coverage.exact_hero_duration += usize::from(hero.is_some());
            audit.coverage.final_hero += usize::from(hero.is_some());
            audit.performance_eligible_rows += usize::from(allow_performance);
            audit.stadium_performance_eligible_rows += usize::from(allow_performance);
            audit.resource_debt_eligible_rows += usize::from(allow_resource_debt);
            audit.stadium_quick_outcome_only_rows +=
                usize::from(training_cohort == TrainingCohort::OutcomeOnly);
            let healing_taken_semantic = if healing_taken.is_some() {
                "external"
            } else {
                "unknown"
            };
            audit_healing_semantics(healing_taken_semantic, &mut audit.healing_taken_semantics);
            let role_name = role.as_str();
            let (expected_win_rate, mut data_quality) = active_events
                .get(&(r.match_id.clone(), role_name.to_string()))
                .copied()
                .unwrap_or((0.5, base_quality));
            if !allow_performance {
                data_quality = data_quality.min(0.55);
            }
            *role_counts.entry(role_name.to_string()).or_default() += 1;
            rows.push(TrainingRow {
                match_group: stable_match_key(&r.match_id),
                season: r.season,
                begin_ts: r.begin_ts,
                role: role_name.into(),
                mode_family,
                hero_group: hero.as_deref().map(stable_hero_key),
                outcome_only,
                allow_performance,
                allow_performance_by_hero: allow_performance,
                result,
                expected_win_rate,
                data_quality,
                final_hit: performance_stat(allow_performance, stats, "finalHit"),
                kill: performance_stat(allow_performance, stats, "kill"),
                assist: performance_stat(allow_performance, stats, "assist"),
                death: performance_stat(allow_performance, stats, "death"),
                damage: performance_stat(allow_performance, stats, "heroDamage"),
                healing: performance_stat(allow_performance, stats, "cure"),
                mitigation: performance_stat(allow_performance, stats, "resistDamage"),
                healing_taken: allow_resource_debt.then_some(healing_taken).flatten(),
                damage_taken: performance_stat(allow_performance, stats, "damageTaken"),
                objective_time_sec: performance_stat(
                    allow_performance,
                    stats,
                    "targetCompetingTime",
                ),
                duration_sec: r.duration as f64,
                hero_duration_sec: hero.as_ref().map(|_| r.duration as f64),
                healing_taken_semantic,
                usable_for_resource_debt: allow_resource_debt,
            });
            continue;
        }
        if r.hero.is_empty() {
            continue;
        }
        let healing_taken =
            numeric(&entry, "healingTaken").or_else(|| numeric(&r.player, "healingTaken"));
        let damage_taken =
            numeric(&entry, "damageTaken").or_else(|| numeric(&r.player, "damageTaken"));
        let objective_time = numeric(&entry, "targetCompetingTime")
            .or_else(|| numeric(&r.player, "targetCompetingTime"));
        let segments = hero_segments(&detail);
        let legacy_hero_duration = legacy_hero_duration(&entry, &r.hero);
        audit.coverage.final_hit += usize::from(has_numeric(&entry, &r.player, "finalHit"));
        audit.coverage.kill += usize::from(has_numeric(&entry, &r.player, "kill"));
        audit.coverage.assist += usize::from(has_numeric(&entry, &r.player, "assist"));
        audit.coverage.death += usize::from(has_numeric(&entry, &r.player, "death"));
        audit.coverage.damage += usize::from(has_numeric(&entry, &r.player, "heroDamage"));
        audit.coverage.healing += usize::from(has_numeric(&entry, &r.player, "cure"));
        audit.coverage.mitigation += usize::from(has_numeric(&entry, &r.player, "resistDamage"));
        audit.coverage.healing_taken += usize::from(has_numeric(&entry, &r.player, "healingTaken"));
        audit.coverage.damage_taken += usize::from(has_numeric(&entry, &r.player, "damageTaken"));
        audit.coverage.objective_time +=
            usize::from(has_numeric(&entry, &r.player, "targetCompetingTime"));
        audit.coverage.duration += usize::from(r.duration > 0);
        audit.coverage.exact_hero_duration +=
            segments.len() + usize::from(segments.is_empty() && legacy_hero_duration.is_some());
        audit.coverage.final_hero += 1;
        let segment_count = segments.len();
        let segment_rows: Vec<Option<HeroSegment>> = if segment_count == 0 {
            vec![None]
        } else {
            segments.into_iter().map(Some).collect()
        };
        for segment in segment_rows {
            let hero = segment
                .as_ref()
                .map(|item| item.hero.as_str())
                .unwrap_or(&r.hero);
            let role_value = if segment.is_some() {
                hero_roles.get(hero).map(String::as_str)
            } else {
                entry
                    .get("roleType")
                    .and_then(Value::as_str)
                    .or(Some(r.dictionary_role.as_str()))
            };
            let Some(role_value) = role_value else {
                continue;
            };
            let Some(role) = normalize_role(role_value) else {
                continue;
            };
            let hero_duration_sec = segment
                .as_ref()
                .map(|item| item.duration_sec)
                .or(legacy_hero_duration);
            let stats = segment.as_ref().map(|item| &item.stats);
            audit.coverage.hero_final_hit += usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_FINAL_HIT))
                    .is_some(),
            );
            audit.coverage.hero_kill +=
                usize::from(stats.and_then(|value| numeric(value, STAT_KILL)).is_some());
            audit.coverage.hero_assist += usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_ASSIST))
                    .is_some(),
            );
            audit.coverage.hero_death +=
                usize::from(stats.and_then(|value| numeric(value, STAT_DEATH)).is_some());
            audit.coverage.hero_damage += usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_HERO_DAMAGE))
                    .is_some(),
            );
            audit.coverage.hero_healing += usize::from(stats.is_some_and(|value| {
                numeric(value, STAT_HEALING_PRIMARY)
                    .or_else(|| numeric(value, STAT_HEALING_FALLBACK))
                    .is_some()
            }));
            audit.coverage.hero_objective_time += usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_OBJECTIVE_TIME))
                    .is_some(),
            );
            let exact_and_substantial = segment.as_ref().is_some_and(|item| {
                item.duration_sec >= 60.0
                    && item
                        .use_rate
                        .unwrap_or(item.duration_sec / r.duration.max(1) as f64)
                        >= 0.1
            });
            let training_cohort = mode_family.training_cohort();
            let allow_performance = exact_and_substantial
                && mode_family.trains_performance()
                && training_cohort != TrainingCohort::OutcomeOnly;
            let allow_resource_debt =
                allow_performance && segment_count == 1 && healing_taken.is_some();
            audit.performance_eligible_rows += usize::from(allow_performance);
            audit.resource_debt_eligible_rows += usize::from(allow_resource_debt);
            audit.minor_segment_rows += usize::from(segment.is_some() && !exact_and_substantial);
            let healing_taken_semantic = if healing_taken.is_some() {
                "external"
            } else {
                "unknown"
            };
            audit_healing_semantics(healing_taken_semantic, &mut audit.healing_taken_semantics);
            let (expected_win_rate, mut data_quality) = active_events
                .get(&(r.match_id.clone(), role.to_string()))
                .copied()
                .unwrap_or((0.5, if allow_performance { 1.0 } else { 0.55 }));
            if !allow_performance {
                data_quality = data_quality.min(0.55);
            }
            *role_counts.entry(role.to_string()).or_default() += 1;
            rows.push(TrainingRow {
                match_group: stable_match_key(&r.match_id),
                season: r.season,
                begin_ts: r.begin_ts,
                role: role.into(),
                mode_family,
                hero_group: hero_duration_sec.map(|_| stable_hero_key(hero)),
                outcome_only: !allow_performance,
                allow_performance,
                allow_performance_by_hero: allow_performance,
                result,
                expected_win_rate,
                data_quality,
                final_hit: performance_stat(allow_performance, stats, STAT_FINAL_HIT),
                kill: performance_stat(allow_performance, stats, STAT_KILL),
                assist: performance_stat(allow_performance, stats, STAT_ASSIST),
                death: performance_stat(allow_performance, stats, STAT_DEATH),
                damage: performance_stat(allow_performance, stats, STAT_HERO_DAMAGE),
                healing: allow_performance
                    .then(|| {
                        stats.and_then(|value| {
                            numeric(value, STAT_HEALING_PRIMARY)
                                .or_else(|| numeric(value, STAT_HEALING_FALLBACK))
                        })
                    })
                    .flatten(),
                mitigation: None,
                healing_taken: allow_resource_debt.then_some(healing_taken).flatten(),
                damage_taken: (allow_performance && segment_count == 1)
                    .then_some(damage_taken)
                    .flatten(),
                objective_time_sec: performance_stat(allow_performance, stats, STAT_OBJECTIVE_TIME)
                    .or_else(|| {
                        (allow_performance && segment_count == 1)
                            .then_some(objective_time)
                            .flatten()
                    }),
                duration_sec: r.duration as f64,
                hero_duration_sec,
                healing_taken_semantic,
                usable_for_resource_debt: allow_resource_debt,
            });
        }
    }
    audit.projected_rows = rows.len();
    audit.roles = role_counts
        .into_iter()
        .map(|(role, samples)| RoleTrainingAudit { role, samples })
        .collect();
    Ok(TrainingProjection { rows, audit })
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )?)
}

fn normalize_role(role: &str) -> Option<&'static str> {
    match role.trim().to_ascii_lowercase().as_str() {
        "tank" | "0" => Some("tank"),
        "damage" | "dps" | "1" => Some("damage"),
        "support" | "healer" | "2" => Some("support"),
        _ => None,
    }
}

fn normalize_result(result: &str) -> Option<f64> {
    match result.trim().to_ascii_lowercase().as_str() {
        "win" => Some(1.0),
        "loss" => Some(0.0),
        "draw" => Some(0.5),
        _ => None,
    }
}

fn hero_segments(detail: &Value) -> Vec<HeroSegment> {
    detail
        .pointer("/payload/data/heroList")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let hero = item.get("heroId").and_then(value_key)?;
            let duration_sec = numeric(item, "userTimeSec")?;
            (duration_sec > 0.0).then(|| HeroSegment {
                hero,
                duration_sec,
                use_rate: numeric(item, "useTimeRate"),
                stats: item.get("statMap").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn legacy_hero_duration(entry: &Value, hero: &str) -> Option<f64> {
    entry
        .get("heroPlayTimeList")?
        .as_array()?
        .iter()
        .find(|item| item.get("heroGuid").and_then(Value::as_str) == Some(hero))
        .and_then(|item| numeric(item, "durationSec"))
        .filter(|duration| *duration > 0.0)
}

fn value_key(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|item| item.to_string()))
}

fn performance_stat(enabled: bool, stats: Option<&Value>, key: &str) -> Option<f64> {
    enabled
        .then(|| stats.and_then(|value| numeric(value, key)))
        .flatten()
}

fn numeric(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn has_numeric(entry: &Value, player: &Value, key: &str) -> bool {
    numeric(entry, key)
        .or_else(|| numeric(player, key))
        .is_some()
}
fn audit_healing_semantics(kind: &str, audit: &mut HealingTakenSemanticsAudit) {
    if kind == "mixed_or_self" {
        audit.explicit_self_healing_rows += 1;
    } else if kind == "external" {
        audit.explicit_external_healing_rows += 1;
    } else {
        audit.unknown_rows += 1;
    }
}
fn stable_hero_key(hero: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(hero.trim().to_ascii_lowercase().as_bytes())
    )
}

fn stable_match_key(match_id: &str) -> String {
    format!("{:x}", Sha256::digest(match_id.as_bytes()))
}
