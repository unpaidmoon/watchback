use crate::career_mmr::input::{
    normalize_mode_family, normalize_role as normalize_input_role, stadium_final_snapshot,
    ExtractionContext, ModeFamily, TrainingCohort,
};
use crate::career_mmr::rating::{outcome_delta_for_expected, INITIAL_SIGMA};
use crate::career_mmr::training_sources::{
    open_query_only, validate_training_schema, TrainingSourceSpec,
};
use anyhow::{bail, Result};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingRow {
    pub account_group: String,
    pub source_match_group: String,
    pub match_group: String,
    pub season: i64,
    pub begin_ts: i64,
    pub role: String,
    pub mode_family: ModeFamily,
    pub environment: TrainingEnvironment,
    pub hero_group: Option<String>,
    pub outcome_only: bool,
    pub allow_performance: bool,
    pub allow_performance_by_hero: bool,
    pub result: f64,
    pub expected_win_rate: f64,
    pub data_quality: f64,
    pub base_delta: Option<f64>,
    pub final_hit: Option<f64>,
    pub kill: Option<f64>,
    pub assist: Option<f64>,
    pub death: Option<f64>,
    pub damage: Option<f64>,
    pub healing: Option<f64>,
    pub mitigation: Option<f64>,
    pub healing_taken: Option<f64>,
    pub available_team_healing: Option<f64>,
    pub damage_taken: Option<f64>,
    pub objective_time_sec: Option<f64>,
    pub duration_sec: f64,
    pub hero_duration_sec: Option<f64>,
    pub healing_taken_semantic: &'static str,
    pub usable_for_resource_debt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrainingEnvironment {
    Ordinary,
    Stadium,
}

impl TrainingEnvironment {
    fn for_mode(mode: ModeFamily) -> Self {
        match mode {
            ModeFamily::StadiumCompetitive | ModeFamily::StadiumQuick => Self::Stadium,
            _ => Self::Ordinary,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrainingProjection {
    pub rows: Vec<TrainingRow>,
    pub audit: TrainingAudit,
    row_audits: Vec<TrainingRowAudit>,
}

impl TrainingProjection {
    pub fn public_audit(&self) -> &TrainingAudit {
        &self.audit
    }

    pub fn merge(&mut self, other: Self) {
        self.audit.merge(other.audit);
        self.rows.extend(other.rows);
        self.row_audits.extend(other.row_audits);
        self.deduplicate_rows();
    }

    fn deduplicate_rows(&mut self) {
        let mut seen = HashSet::new();
        let mut pairs = std::mem::take(&mut self.rows)
            .into_iter()
            .zip(std::mem::take(&mut self.row_audits))
            .filter(|(row, _)| {
                seen.insert((
                    row.account_group.clone(),
                    row.source_match_group.clone(),
                    row.role.clone(),
                    row.hero_group.clone(),
                    row.begin_ts,
                ))
            })
            .collect::<Vec<_>>();
        pairs.sort_by(|(left, _), (right, _)| {
            left.begin_ts
                .cmp(&right.begin_ts)
                .then_with(|| left.account_group.cmp(&right.account_group))
                .then_with(|| left.source_match_group.cmp(&right.source_match_group))
                .then_with(|| left.role.cmp(&right.role))
                .then_with(|| left.hero_group.cmp(&right.hero_group))
        });
        (self.rows, self.row_audits) = pairs.into_iter().unzip();
        rebuild_row_audit(&self.rows, &self.row_audits, &mut self.audit);
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
    pub source_accounts: usize,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TrainingRowAudit {
    coverage: FeatureCoverage,
    minor_segment: bool,
    stadium_quick_outcome_only: bool,
    stadium_performance_eligible: bool,
    healing_taken_semantics: HealingTakenSemanticsAudit,
}

impl TrainingAudit {
    pub fn merge(&mut self, other: Self) {
        self.source_accounts += other.source_accounts;
        self.ordinary_candidates += other.ordinary_candidates;
        self.stadium_competitive_candidates += other.stadium_competitive_candidates;
        self.stadium_quick_outcome_only_rows += other.stadium_quick_outcome_only_rows;
        self.stadium_performance_eligible_rows += other.stadium_performance_eligible_rows;
        self.projected_rows += other.projected_rows;
        self.performance_eligible_rows += other.performance_eligible_rows;
        self.resource_debt_eligible_rows += other.resource_debt_eligible_rows;
        self.minor_segment_rows += other.minor_segment_rows;
        self.stadium_ambiguous_excluded += other.stadium_ambiguous_excluded;
        self.coverage.merge(other.coverage);
        self.healing_taken_semantics
            .merge(other.healing_taken_semantics);
        let mut roles = BTreeMap::new();
        for item in self.roles.drain(..).chain(other.roles) {
            *roles.entry(item.role).or_insert(0) += item.samples;
        }
        self.roles = roles
            .into_iter()
            .map(|(role, samples)| RoleTrainingAudit { role, samples })
            .collect();
    }
}

impl FeatureCoverage {
    fn merge(&mut self, other: Self) {
        self.final_hit += other.final_hit;
        self.kill += other.kill;
        self.assist += other.assist;
        self.death += other.death;
        self.damage += other.damage;
        self.healing += other.healing;
        self.mitigation += other.mitigation;
        self.healing_taken += other.healing_taken;
        self.damage_taken += other.damage_taken;
        self.objective_time += other.objective_time;
        self.duration += other.duration;
        self.exact_hero_duration += other.exact_hero_duration;
        self.final_hero += other.final_hero;
        self.hero_final_hit += other.hero_final_hit;
        self.hero_kill += other.hero_kill;
        self.hero_assist += other.hero_assist;
        self.hero_death += other.hero_death;
        self.hero_damage += other.hero_damage;
        self.hero_healing += other.hero_healing;
        self.hero_objective_time += other.hero_objective_time;
    }
}

impl HealingTakenSemanticsAudit {
    fn merge(&mut self, other: Self) {
        self.explicit_self_healing_rows += other.explicit_self_healing_rows;
        self.explicit_external_healing_rows += other.explicit_external_healing_rows;
        self.unknown_rows += other.unknown_rows;
    }
}

struct RawRow {
    match_id: String,
    season: i64,
    begin_ts: i64,
    map_guid: String,
    team_score: i64,
    opponent_score: i64,
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
pub fn build_training_projection(conn: &Connection) -> Result<TrainingProjection> {
    build_training_projection_for_account(conn, "dataset-001")
}

pub fn build_training_projection_for_account(
    conn: &Connection,
    account_group: &str,
) -> Result<TrainingProjection> {
    let mut active_events = HashMap::new();
    if table_exists(conn, "career_mmr_events")? && table_exists(conn, "mmr_model_versions")? {
        let mut event_stmt = conn.prepare("SELECT e.match_id,e.role,e.expected_win_rate,e.data_quality,e.base_delta FROM career_mmr_events e JOIN mmr_model_versions v ON v.version=e.model_version WHERE v.is_active=1")?;
        for item in event_stmt.query_map([], |q| {
            Ok((
                q.get::<_, String>(0)?,
                q.get::<_, String>(1)?,
                q.get::<_, f64>(2)?,
                q.get::<_, f64>(3)?,
                q.get::<_, f64>(4)?,
            ))
        })? {
            let (match_id, role, expected, quality, base_delta) = item?;
            if active_events
                .insert((match_id, role), (expected, quality, base_delta))
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
    let map_guid = if column_exists(conn, "matches", "map_guid")? {
        "COALESCE(m.map_guid,'')"
    } else {
        "''"
    };
    let team_score = if column_exists(conn, "matches", "team_score")? {
        "COALESCE(m.team_score,0)"
    } else {
        "0"
    };
    let opponent_score = if column_exists(conn, "matches", "opponent_score")? {
        "COALESCE(m.opponent_score,0)"
    } else {
        "0"
    };
    let ordinary_sql = format!(
        r#"SELECT m.match_id,m.season,m.begin_ts,{map_guid},{team_score},{opponent_score},
                  COALESCE(d.role,''),m.hero_guid,m.result,m.game_mode,
                  m.duration_sec,
                  m.raw_entry_json,COALESCE((SELECT p.raw_json FROM match_players p
                    WHERE p.match_id=m.match_id AND p.is_current_player=1
                    AND (SELECT COUNT(*) FROM match_players x WHERE x.match_id=m.match_id AND x.is_current_player=1)=1 LIMIT 1),'{{}}'),
                  COALESCE(md.raw_detail_json,'{{}}')
           FROM matches m
           LEFT JOIN hero_dict d ON d.guid=m.hero_guid
           LEFT JOIN match_details md ON md.match_id=m.match_id
           WHERE m.bucket<>'stadium'
           ORDER BY m.begin_ts,m.season,m.match_id"#,
    );
    let mut stmt = conn.prepare(&ordinary_sql)?;
    let mut raw: Vec<RawRow> = stmt
        .query_map([], |r| {
            Ok(RawRow {
                match_id: r.get(0)?,
                season: r.get(1)?,
                begin_ts: r.get(2)?,
                map_guid: r.get(3)?,
                team_score: r.get(4)?,
                opponent_score: r.get(5)?,
                dictionary_role: r.get(6)?,
                hero: r.get(7)?,
                result: r.get(8)?,
                game_mode: r.get(9)?,
                duration: r.get(10)?,
                entry: r.get(11)?,
                player: serde_json::from_str(&r.get::<_, String>(12)?).unwrap_or(Value::Null),
                detail: r.get(13)?,
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
        let current_player_key = if column_exists(conn, "match_players", "bnet_id")? {
            r#"(SELECT p.bnet_id FROM match_players p
                  WHERE p.match_id=m.match_id AND p.is_current_player=1
                  AND p.bnet_id=TRIM(p.bnet_id) AND TRIM(p.bnet_id)<>''
                  AND (SELECT COUNT(*) FROM match_players x WHERE x.match_id=m.match_id AND x.is_current_player=1)=1 LIMIT 1)"#
        } else {
            "NULL"
        };
        let stadium_sql = format!(
            r#"SELECT m.match_id,m.season,m.begin_ts,{map_guid},{team_score},{opponent_score},
                      COALESCE(d.role,''),m.hero_guid,m.result,m.game_mode,
                      m.duration_sec,m.raw_entry_json,'{{}}',COALESCE(md.raw_detail_json,'{{}}'),m.bucket,
                      {current_player_key}
               FROM matches m
               LEFT JOIN hero_dict d ON d.guid=m.hero_guid
               LEFT JOIN match_details md ON md.match_id=m.match_id
               WHERE m.bucket='stadium'
               ORDER BY m.begin_ts,m.season,m.match_id"#,
        );
        let mut stadium_stmt = conn.prepare(&stadium_sql)?;
        raw.extend(
            stadium_stmt
                .query_map([], |r| {
                    Ok(RawRow {
                        match_id: r.get(0)?,
                        season: r.get(1)?,
                        begin_ts: r.get(2)?,
                        map_guid: r.get(3)?,
                        team_score: r.get(4)?,
                        opponent_score: r.get(5)?,
                        dictionary_role: r.get(6)?,
                        hero: r.get(7)?,
                        result: r.get(8)?,
                        game_mode: r.get(9)?,
                        duration: r.get(10)?,
                        entry: r.get(11)?,
                        player: Value::Null,
                        detail: r.get(13)?,
                        bucket: r.get(14)?,
                        current_player_key: r.get(15)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        );
    }

    let mut audit = TrainingAudit {
        source_accounts: 1,
        ordinary_candidates,
        ..Default::default()
    };
    let extraction_context = (stadium_candidates > 0)
        .then(|| ExtractionContext::build(conn))
        .transpose()?;
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
    let mut row_audits = Vec::new();
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
        let match_group = stable_match_key(&r, mode_family);
        let source_match_group = stable_source_match_key(&r.match_id);
        if r.bucket == "stadium" {
            if mode_family == ModeFamily::StadiumCompetitive {
                audit.stadium_competitive_candidates += 1;
            }
            let rounds = detail
                .pointer("/payload/data/roundCountList")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let Some(current_player_key) = r.current_player_key.as_deref() else {
                audit.stadium_ambiguous_excluded += 1;
                continue;
            };
            let snapshot = stadium_final_snapshot(
                rounds,
                current_player_key,
                extraction_context.as_ref().unwrap(),
            );
            let fallback_role = entry
                .get("roleType")
                .and_then(Value::as_str)
                .and_then(|role| normalize_input_role(Some(role)));
            let (
                role,
                hero,
                statistics,
                available_team_healing,
                base_quality,
                has_performance_fields,
            ) = if let Some(snapshot) = snapshot {
                let base_quality = snapshot.data_quality();
                let has_performance_fields = snapshot.has_valid_performance_fields();
                (
                    snapshot.role,
                    Some(snapshot.hero_guid),
                    Some(snapshot.statistics),
                    snapshot.available_team_healing,
                    base_quality,
                    has_performance_fields,
                )
            } else if let Some(role) = fallback_role {
                (role, None, None, None, 0.35, false)
            } else {
                audit.stadium_ambiguous_excluded += 1;
                continue;
            };
            let training_cohort = mode_family.training_cohort();
            let stats = statistics.as_ref();
            let allow_performance = mode_family.trains_performance()
                && training_cohort == TrainingCohort::StadiumCompetitive
                && r.duration > 0
                && has_performance_fields;
            let outcome_only = training_cohort == TrainingCohort::OutcomeOnly || !allow_performance;
            let healing_taken = stats.and_then(|value| numeric(value, "healingTaken"));
            let allow_resource_debt = allow_performance && healing_taken.is_some();
            let mut row_audit = TrainingRowAudit::default();
            row_audit.coverage.final_hit =
                usize::from(stats.and_then(|value| numeric(value, "finalHit")).is_some());
            row_audit.coverage.kill =
                usize::from(stats.and_then(|value| numeric(value, "kill")).is_some());
            row_audit.coverage.assist =
                usize::from(stats.and_then(|value| numeric(value, "assist")).is_some());
            row_audit.coverage.death =
                usize::from(stats.and_then(|value| numeric(value, "death")).is_some());
            row_audit.coverage.damage = usize::from(
                stats
                    .and_then(|value| numeric(value, "heroDamage"))
                    .is_some(),
            );
            row_audit.coverage.healing =
                usize::from(stats.and_then(|value| numeric(value, "cure")).is_some());
            row_audit.coverage.mitigation = usize::from(
                stats
                    .and_then(|value| numeric(value, "resistDamage"))
                    .is_some(),
            );
            row_audit.coverage.healing_taken = usize::from(healing_taken.is_some());
            row_audit.coverage.damage_taken = usize::from(
                stats
                    .and_then(|value| numeric(value, "damageTaken"))
                    .is_some(),
            );
            row_audit.coverage.objective_time = usize::from(
                stats
                    .and_then(|value| numeric(value, "targetCompetingTime"))
                    .is_some(),
            );
            row_audit.coverage.duration = usize::from(r.duration > 0);
            row_audit.coverage.exact_hero_duration = usize::from(hero.is_some());
            row_audit.coverage.final_hero = usize::from(hero.is_some());
            row_audit.stadium_performance_eligible = allow_performance;
            row_audit.stadium_quick_outcome_only = training_cohort == TrainingCohort::OutcomeOnly;
            let healing_taken_semantic = if healing_taken.is_some() {
                "external"
            } else {
                "unknown"
            };
            audit_healing_semantics(
                healing_taken_semantic,
                &mut row_audit.healing_taken_semantics,
            );
            let role_name = role.as_str();
            let expected_win_rate = active_events
                .get(&(r.match_id.clone(), role_name.to_string()))
                .map(|(expected, _quality, _base_delta)| *expected)
                .unwrap_or(0.5);
            let data_quality = base_quality;
            let base_delta = active_events
                .get(&(r.match_id.clone(), role_name.to_string()))
                .map(|(_, _, base_delta)| *base_delta);
            rows.push(TrainingRow {
                account_group: account_group.to_owned(),
                source_match_group: source_match_group.clone(),
                match_group: match_group.clone(),
                season: r.season,
                begin_ts: r.begin_ts,
                role: role_name.into(),
                mode_family,
                environment: TrainingEnvironment::for_mode(mode_family),
                hero_group: hero.as_deref().map(stable_hero_key),
                outcome_only,
                allow_performance,
                allow_performance_by_hero: allow_performance,
                result,
                expected_win_rate,
                data_quality,
                base_delta,
                final_hit: performance_stat(allow_performance, stats, "finalHit"),
                kill: performance_stat(allow_performance, stats, "kill"),
                assist: performance_stat(allow_performance, stats, "assist"),
                death: performance_stat(allow_performance, stats, "death"),
                damage: performance_stat(allow_performance, stats, "heroDamage"),
                healing: performance_stat(allow_performance, stats, "cure"),
                mitigation: performance_stat(allow_performance, stats, "resistDamage"),
                healing_taken: allow_resource_debt.then_some(healing_taken).flatten(),
                available_team_healing: allow_performance
                    .then_some(available_team_healing)
                    .flatten(),
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
            row_audits.push(row_audit);
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
        let mut match_coverage = FeatureCoverage {
            final_hit: usize::from(has_numeric(&entry, &r.player, "finalHit")),
            kill: usize::from(has_numeric(&entry, &r.player, "kill")),
            assist: usize::from(has_numeric(&entry, &r.player, "assist")),
            death: usize::from(has_numeric(&entry, &r.player, "death")),
            damage: usize::from(has_numeric(&entry, &r.player, "heroDamage")),
            healing: usize::from(has_numeric(&entry, &r.player, "cure")),
            mitigation: usize::from(has_numeric(&entry, &r.player, "resistDamage")),
            healing_taken: usize::from(has_numeric(&entry, &r.player, "healingTaken")),
            damage_taken: usize::from(has_numeric(&entry, &r.player, "damageTaken")),
            objective_time: usize::from(has_numeric(&entry, &r.player, "targetCompetingTime")),
            duration: usize::from(r.duration > 0),
            final_hero: 1,
            ..Default::default()
        };
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
            let mut row_audit = TrainingRowAudit::default();
            row_audit.coverage.hero_final_hit = usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_FINAL_HIT))
                    .is_some(),
            );
            row_audit.coverage.hero_kill =
                usize::from(stats.and_then(|value| numeric(value, STAT_KILL)).is_some());
            row_audit.coverage.hero_assist = usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_ASSIST))
                    .is_some(),
            );
            row_audit.coverage.hero_death =
                usize::from(stats.and_then(|value| numeric(value, STAT_DEATH)).is_some());
            row_audit.coverage.hero_damage = usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_HERO_DAMAGE))
                    .is_some(),
            );
            row_audit.coverage.hero_healing = usize::from(stats.is_some_and(|value| {
                numeric(value, STAT_HEALING_PRIMARY)
                    .or_else(|| numeric(value, STAT_HEALING_FALLBACK))
                    .is_some()
            }));
            row_audit.coverage.hero_objective_time = usize::from(
                stats
                    .and_then(|value| numeric(value, STAT_OBJECTIVE_TIME))
                    .is_some(),
            );
            row_audit.coverage.exact_hero_duration = usize::from(
                segment.is_some() || (segment_count == 0 && legacy_hero_duration.is_some()),
            );
            row_audit
                .coverage
                .merge(std::mem::take(&mut match_coverage));
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
            row_audit.minor_segment = segment.is_some() && !exact_and_substantial;
            let healing_taken_semantic = if healing_taken.is_some() {
                "external"
            } else {
                "unknown"
            };
            audit_healing_semantics(
                healing_taken_semantic,
                &mut row_audit.healing_taken_semantics,
            );
            let (expected_win_rate, mut data_quality, event_base_delta) = active_events
                .get(&(r.match_id.clone(), role.to_string()))
                .copied()
                .map(|(expected, quality, delta)| (expected, quality, Some(delta)))
                .unwrap_or((0.5, if allow_performance { 1.0 } else { 0.55 }, None));
            if !allow_performance {
                data_quality = data_quality.min(0.55);
            }
            rows.push(TrainingRow {
                account_group: account_group.to_owned(),
                source_match_group: source_match_group.clone(),
                match_group: match_group.clone(),
                season: r.season,
                begin_ts: r.begin_ts,
                role: role.into(),
                mode_family,
                environment: TrainingEnvironment::for_mode(mode_family),
                hero_group: hero_duration_sec.map(|_| stable_hero_key(hero)),
                outcome_only: !allow_performance,
                allow_performance,
                allow_performance_by_hero: allow_performance,
                result,
                expected_win_rate,
                data_quality,
                base_delta: event_base_delta,
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
                available_team_healing: None,
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
            row_audits.push(row_audit);
        }
    }
    fill_missing_base_deltas(&mut rows);
    let mut projection = TrainingProjection {
        rows,
        audit,
        row_audits,
    };
    projection.deduplicate_rows();
    Ok(projection)
}

fn fill_missing_base_deltas(rows: &mut [TrainingRow]) {
    let mut durations: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let mut roles: HashMap<String, HashSet<String>> = HashMap::new();
    for row in rows.iter() {
        roles
            .entry(row.match_group.clone())
            .or_default()
            .insert(row.role.clone());
        if let Some(duration) = row
            .hero_duration_sec
            .filter(|value| value.is_finite() && *value > 0.0)
        {
            *durations
                .entry(row.match_group.clone())
                .or_default()
                .entry(row.role.clone())
                .or_default() += duration;
        }
    }
    for row in rows.iter_mut().filter(|row| row.base_delta.is_none()) {
        let by_role = durations.get(&row.match_group);
        let total = by_role
            .map(|items| items.values().sum::<f64>())
            .unwrap_or(0.0);
        let role_share = if total > 0.0 {
            by_role
                .and_then(|items| items.get(&row.role))
                .copied()
                .unwrap_or(0.0)
                / total
        } else {
            let role_count = roles.get(&row.match_group).map(HashSet::len).unwrap_or(1);
            1.0 / role_count.max(1) as f64
        };
        row.base_delta = Some(outcome_delta_for_expected(
            INITIAL_SIGMA,
            row.mode_family,
            role_share,
            row.data_quality,
            row.result,
            row.expected_win_rate,
        ));
    }
}

pub fn build_multi_account_projection(
    sources: &[TrainingSourceSpec],
) -> Result<TrainingProjection> {
    let result = (|| -> Result<TrainingProjection> {
        let mut projection = TrainingProjection::default();
        for source in sources {
            let conn = open_query_only(source)?;
            validate_training_schema(&conn)?;
            let source_projection =
                build_training_projection_for_account(&conn, &source.account_group)?;
            if conn.total_changes() != 0 {
                bail!("training source was modified");
            }
            projection.merge(source_projection);
        }
        Ok(projection)
    })();
    result.map_err(|_| anyhow::anyhow!("无法聚合多账号训练数据"))
}

fn rebuild_row_audit(
    rows: &[TrainingRow],
    row_audits: &[TrainingRowAudit],
    audit: &mut TrainingAudit,
) {
    debug_assert_eq!(rows.len(), row_audits.len());
    audit.projected_rows = rows.len();
    audit.performance_eligible_rows = 0;
    audit.resource_debt_eligible_rows = 0;
    audit.minor_segment_rows = 0;
    audit.stadium_quick_outcome_only_rows = 0;
    audit.stadium_performance_eligible_rows = 0;
    audit.coverage = FeatureCoverage::default();
    audit.healing_taken_semantics = HealingTakenSemanticsAudit::default();
    let mut roles = BTreeMap::new();
    for (row, row_audit) in rows.iter().zip(row_audits) {
        *roles.entry(row.role.clone()).or_insert(0) += 1;
        audit.performance_eligible_rows += usize::from(row.allow_performance);
        audit.resource_debt_eligible_rows += usize::from(row.usable_for_resource_debt);
        audit.minor_segment_rows += usize::from(row_audit.minor_segment);
        audit.stadium_quick_outcome_only_rows += usize::from(row_audit.stadium_quick_outcome_only);
        audit.stadium_performance_eligible_rows +=
            usize::from(row_audit.stadium_performance_eligible);
        audit.coverage.merge(row_audit.coverage.clone());
        audit
            .healing_taken_semantics
            .merge(row_audit.healing_taken_semantics.clone());
    }
    audit.roles = roles
        .into_iter()
        .map(|(role, samples)| RoleTrainingAudit { role, samples })
        .collect();
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )?)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name=?2)",
        [table, column],
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

fn stable_source_match_key(match_id: &str) -> String {
    format!("{:x}", Sha256::digest(match_id.as_bytes()))
}

// The cross-account cluster deliberately excludes the source-local match id and
// perspective-dependent result. Exact timestamp, normalized mode, map, duration,
// season, and unordered scores make accidental merges substantially less likely
// while allowing two participants' copies of the same match to converge.
fn stable_match_key(row: &RawRow, mode_family: ModeFamily) -> String {
    let (low_score, high_score) = if row.team_score <= row.opponent_score {
        (row.team_score, row.opponent_score)
    } else {
        (row.opponent_score, row.team_score)
    };
    let fingerprint = format!(
        "career-mmr-match-v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        row.season,
        row.begin_ts,
        mode_family.as_str(),
        row.map_guid.trim().to_ascii_lowercase(),
        row.duration,
        low_score,
        high_score,
    );
    format!("{:x}", Sha256::digest(fingerprint.as_bytes()))
}
