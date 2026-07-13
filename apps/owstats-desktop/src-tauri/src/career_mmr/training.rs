use crate::career_mmr::input::{normalize_mode_family, ModeFamily};
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
    pub projected_rows: usize,
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
}

pub fn build_training_projection(conn: &Connection) -> Result<TrainingProjection> {
    let mut active_events = HashMap::new();
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
    let stadium_ambiguous_excluded = conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE bucket='stadium'",
        [],
        |row| row.get::<_, usize>(0),
    )?;
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
                    AND (SELECT COUNT(*) FROM match_players x WHERE x.match_id=m.match_id AND x.is_current_player=1)=1 LIMIT 1),'{}')
           FROM matches m
           LEFT JOIN hero_dict d ON d.guid=m.hero_guid
           WHERE m.bucket<>'stadium'
           ORDER BY m.begin_ts,m.season,m.match_id"#,
    )?;
    let raw: Vec<RawRow> = stmt
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
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut audit = TrainingAudit {
        ordinary_candidates,
        stadium_ambiguous_excluded,
        ..Default::default()
    };
    let mut role_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = Vec::new();
    for r in raw {
        let entry = serde_json::from_str::<Value>(&r.entry).unwrap_or(Value::Null);
        let Some(mode_family) = normalize_mode_family(
            &r.game_mode,
            entry.get("instanceType").and_then(Value::as_str),
        ) else {
            continue;
        };
        let role_value = entry
            .get("roleType")
            .and_then(Value::as_str)
            .unwrap_or(&r.dictionary_role);
        let Some(role) = normalize_role(role_value) else {
            continue;
        };
        let Some(result) = normalize_result(&r.result) else {
            continue;
        };
        if r.hero.is_empty() {
            continue;
        }
        let final_hit = numeric(&entry, "finalHit").or_else(|| numeric(&r.player, "finalHit"));
        let mixed_healing_taken =
            numeric(&entry, "healingTaken").or_else(|| numeric(&r.player, "healingTaken"));
        let damage_taken =
            numeric(&entry, "damageTaken").or_else(|| numeric(&r.player, "damageTaken"));
        let objective_time = numeric(&entry, "targetCompetingTime")
            .or_else(|| numeric(&r.player, "targetCompetingTime"));
        let hero_duration_sec = exact_hero_duration(&entry, &r.hero);
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
        audit.coverage.exact_hero_duration += usize::from(hero_duration_sec.is_some());
        audit.coverage.final_hero += 1;
        let healing_taken_semantic = healing_semantic(&entry, &r.player);
        audit_healing_semantics(healing_taken_semantic, &mut audit.healing_taken_semantics);
        let (expected_win_rate, mut data_quality) = active_events
            .get(&(r.match_id.clone(), role.to_string()))
            .copied()
            .unwrap_or((
                0.5,
                if hero_duration_sec.is_some() {
                    1.0
                } else {
                    0.55
                },
            ));
        let allow_performance = false;
        data_quality = data_quality.min(0.55);
        let external_healing = numeric(&entry, "healingReceivedFromOthers")
            .or_else(|| numeric(&r.player, "healingReceivedFromOthers"));
        let selected_healing = if healing_taken_semantic == "external" {
            external_healing
        } else {
            mixed_healing_taken
        };
        *role_counts.entry(role.to_string()).or_default() += 1;
        rows.push(TrainingRow {
            match_group: stable_match_key(&r.match_id),
            season: r.season,
            begin_ts: r.begin_ts,
            role: role.into(),
            mode_family,
            hero_group: hero_duration_sec.map(|_| stable_hero_key(&r.hero)),
            outcome_only: !allow_performance,
            allow_performance,
            allow_performance_by_hero: false,
            result,
            expected_win_rate,
            data_quality,
            final_hit: allow_performance.then_some(final_hit).flatten(),
            kill: allow_performance
                .then_some(numeric(&entry, "kill").or_else(|| numeric(&r.player, "kill")))
                .flatten(),
            assist: allow_performance
                .then_some(numeric(&entry, "assist").or_else(|| numeric(&r.player, "assist")))
                .flatten(),
            death: allow_performance
                .then_some(numeric(&entry, "death").or_else(|| numeric(&r.player, "death")))
                .flatten(),
            damage: allow_performance
                .then_some(
                    numeric(&entry, "heroDamage").or_else(|| numeric(&r.player, "heroDamage")),
                )
                .flatten(),
            healing: allow_performance
                .then_some(numeric(&entry, "cure").or_else(|| numeric(&r.player, "cure")))
                .flatten(),
            mitigation: allow_performance
                .then_some(
                    numeric(&entry, "resistDamage").or_else(|| numeric(&r.player, "resistDamage")),
                )
                .flatten(),
            healing_taken: allow_performance.then_some(selected_healing).flatten(),
            damage_taken: allow_performance.then_some(damage_taken).flatten(),
            objective_time_sec: allow_performance.then_some(objective_time).flatten(),
            duration_sec: r.duration as f64,
            hero_duration_sec,
            healing_taken_semantic,
            usable_for_resource_debt: allow_performance && healing_taken_semantic == "external",
        });
    }
    audit.projected_rows = rows.len();
    audit.roles = role_counts
        .into_iter()
        .map(|(role, samples)| RoleTrainingAudit { role, samples })
        .collect();
    Ok(TrainingProjection { rows, audit })
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

fn exact_hero_duration(entry: &Value, hero: &str) -> Option<f64> {
    let list = entry.get("heroPlayTimeList")?.as_array()?;
    let item = list
        .iter()
        .find(|v| v.get("heroGuid").and_then(Value::as_str) == Some(hero))?;
    item.get("durationSec")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.0)
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
fn healing_semantic(entry: &Value, player: &Value) -> &'static str {
    if numeric(entry, "healingReceivedFromOthers")
        .or_else(|| numeric(player, "healingReceivedFromOthers"))
        .is_some()
    {
        "external"
    } else if numeric(entry, "selfHealing")
        .or_else(|| numeric(player, "selfHealing"))
        .is_some()
    {
        "mixed_or_self"
    } else {
        "unknown"
    }
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
