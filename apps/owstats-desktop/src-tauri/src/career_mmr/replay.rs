use super::{
    input::{
        extract_ordinary_match_with_context, extract_stadium_match_with_context, ExtractionContext,
        MmrMatchInput, ModeFamily, Role,
    },
    performance_runtime::NormalizedPerformanceRow,
    performance_selection::SelectedRoleModel,
    rating::{
        apply_inactivity, confidence, expected_win, update_rating, update_rating_for_expected,
        RatingInput, RoleRatingState,
    },
    training_report::{
        validate_parsed_v3_artifact, CareerMmrV3Artifact, PerformanceModelArtifact,
        V3_ARTIFACT_ALGORITHM, V3_SCHEMA_VERSION,
    },
};
use crate::db::OwDatabase;
use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

pub const BASELINE_MODEL_VERSION: i64 = 2;
pub const PERFORMANCE_MODEL_VERSION: i64 = 3;
pub const PACKAGED_V3_ARTIFACT_JSON: &str =
    include_str!("../../../runtime-config/career-mmr-v3.json");
const ALGORITHM: &str = "role_bayesian_outcome_six_mode_v2";
const CONFIG_JSON: &str = r#"{"performanceWeight":0.0,"trainingCohorts":{"role_competitive":"role_queue","role_quick":"role_queue","open_competitive":"open_queue","open_quick":"open_queue","stadium_competitive":"stadium_competitive","stadium_quick":"outcome_only"},"modeWeights":{"role_competitive":1.0,"role_quick":0.3,"open_competitive":1.0,"open_quick":0.3,"stadium_competitive":0.5,"stadium_quick":0.1},"modeCaps":{"competitive":30.0,"quick":10.0}}"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrBuildSummary {
    pub model_version: i64,
    pub inputs_count: usize,
    pub events_count: usize,
    pub discovered_seasons: Vec<i64>,
    pub event_digest: String,
}

pub fn rebuild_career_mmr(db: &OwDatabase) -> Result<CareerMmrBuildSummary> {
    db.initialize()?;
    let mut conn = db.connect()?;
    let tx = conn.transaction()?;
    ensure_model_version(&tx)?;
    let context = ExtractionContext::build(&tx)?;
    let mut inputs = extract_all_inputs(&tx, &context)?;
    inputs.sort_by(|a, b| {
        (a.begin_ts, a.season, sha256_hex(&a.match_id), &a.match_id).cmp(&(
            b.begin_ts,
            b.season,
            sha256_hex(&b.match_id),
            &b.match_id,
        ))
    });
    let watermark = build_watermark(&inputs);
    replace_inputs(&tx, &inputs)?;
    let digest = replay_events(&tx, &inputs, &watermark)?;
    rebuild_summaries(&tx, &watermark, BASELINE_MODEL_VERSION)?;
    tx.execute(
        "UPDATE mmr_model_versions SET is_active = CASE WHEN version=?1 THEN 1 ELSE 0 END",
        [BASELINE_MODEL_VERSION],
    )?;
    tx.commit()?;
    Ok(CareerMmrBuildSummary {
        model_version: BASELINE_MODEL_VERSION,
        inputs_count: inputs.len(),
        events_count: inputs.len(),
        discovered_seasons: inputs
            .iter()
            .map(|i| i.season)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        event_digest: digest,
    })
}

pub fn rebuild_career_mmr_active(db: &OwDatabase) -> Result<CareerMmrBuildSummary> {
    let packaged: PerformanceModelArtifact =
        serde_json::from_str(PACKAGED_V3_ARTIFACT_JSON).context("无法读取打包的 v3 模型")?;
    let artifact = packaged.v3_runtime.context("打包模型缺少 v3 运行时产物")?;
    validate_parsed_v3_artifact(&artifact).context("打包的 v3 模型无效")?;
    let artifact_json = serde_json::to_string(&artifact).context("无法序列化打包的 v3 模型")?;
    rebuild_career_mmr_v3(db, &artifact_json)
}

/// Activate the v3 personal-performance model transactionally.
///
/// `artifact_json` is the serialized [`super::training_report::CareerMmrV3Artifact`]
/// produced by the training CLI. It is validated (schema, role coverage, business
/// signs and config hash) before any write transaction is opened, and the entire
/// activation is staged against model version [`PERFORMANCE_MODEL_VERSION`] inside
/// one transaction: v3 model definition and artifact are written, inputs are
/// rebuilt, v3 events and summaries are replayed with a bounded per-role
/// performance correction, three role summaries and event counts are verified,
/// and only then is the single active flag switched to v3. Any failure rolls the
/// transaction back and leaves v2 as the active, untouched model.
pub fn rebuild_career_mmr_v3(
    db: &OwDatabase,
    artifact_json: &str,
) -> Result<CareerMmrBuildSummary> {
    db.initialize()?;
    let artifact_value = serde_json::from_str::<Value>(artifact_json).context("v3 ?????? JSON")?;
    let artifact: CareerMmrV3Artifact =
        serde_json::from_value(artifact_value.clone()).context("v3 ????????")?;
    validate_parsed_v3_artifact(&artifact).context("v3 ??????")?;
    if artifact.algorithm != V3_ARTIFACT_ALGORITHM {
        anyhow::bail!("v3 ???????");
    }
    if artifact.schema_version != V3_SCHEMA_VERSION {
        anyhow::bail!("v3 ???????");
    }
    for role in [Role::Tank, Role::Damage, Role::Support] {
        if !artifact.selected_models.contains_key(&role) {
            anyhow::bail!("v3 ????????");
        }
    }

    let mut conn = db.connect()?;
    let tx = conn.transaction()?;
    ensure_v3_model_version(&tx, &artifact, artifact_json)?;
    let context = ExtractionContext::build(&tx)?;
    let mut inputs = extract_all_inputs(&tx, &context)?;
    inputs.sort_by(|a, b| {
        (a.begin_ts, a.season, sha256_hex(&a.match_id), &a.match_id).cmp(&(
            b.begin_ts,
            b.season,
            sha256_hex(&b.match_id),
            &b.match_id,
        ))
    });
    let watermark = build_watermark(&inputs);
    replace_inputs(&tx, &inputs)?;
    let digest = replay_events_v3(&tx, &inputs, &watermark, &artifact)?;
    rebuild_summaries(&tx, &watermark, PERFORMANCE_MODEL_VERSION)?;
    verify_v3_activation(&tx, &inputs)?;

    // Atomic cutover: v3 becomes the single active model. v2 rows are retained
    // unchanged for fast rollback.
    tx.execute(
        "UPDATE mmr_model_versions SET is_active = CASE WHEN version=?1 THEN 1 ELSE 0 END",
        [PERFORMANCE_MODEL_VERSION],
    )?;
    tx.commit()?;
    Ok(CareerMmrBuildSummary {
        model_version: PERFORMANCE_MODEL_VERSION,
        inputs_count: inputs.len(),
        events_count: inputs.len(),
        discovered_seasons: inputs
            .iter()
            .map(|i| i.season)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        event_digest: digest,
    })
}

fn ensure_v3_model_version(
    tx: &Transaction<'_>,
    artifact: &CareerMmrV3Artifact,
    artifact_json: &str,
) -> Result<()> {
    let algorithm = V3_ARTIFACT_ALGORITHM;
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO mmr_model_versions(version, algorithm, config_json, is_active, created_at) VALUES (?1, ?2, ?3, 0, ?4)
         ON CONFLICT(version) DO UPDATE SET algorithm=excluded.algorithm, config_json=excluded.config_json",
        params![PERFORMANCE_MODEL_VERSION, algorithm, artifact.config_hash, now],
    )?;
    tx.execute(
        "INSERT INTO career_mmr_model_artifacts(model_version, algorithm, config_hash, artifact_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(model_version) DO UPDATE SET algorithm=excluded.algorithm, config_hash=excluded.config_hash, artifact_json=excluded.artifact_json, created_at=excluded.created_at",
        params![PERFORMANCE_MODEL_VERSION, algorithm, artifact.config_hash, artifact_json, now],
    )?;
    Ok(())
}

fn verify_v3_activation(tx: &Transaction<'_>, inputs: &[MmrMatchInput]) -> Result<()> {
    let event_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM career_mmr_events WHERE model_version=?1",
        [PERFORMANCE_MODEL_VERSION],
        |row| row.get(0),
    )?;
    if event_count != inputs.len() as i64 {
        anyhow::bail!("v3 ???????????????? v2 ????");
    }
    let role_rows: i64 = tx.query_row(
        "SELECT COUNT(*) FROM career_role_mmr WHERE model_version=?1",
        [PERFORMANCE_MODEL_VERSION],
        |row| row.get(0),
    )?;
    if role_rows != 3 {
        anyhow::bail!("v3 ?????????????? v2 ????");
    }
    Ok(())
}

fn normalized_performance_row(input: &MmrMatchInput) -> NormalizedPerformanceRow {
    let duration_min = (input.duration_sec.max(1) as f64) / 60.0;
    let per10 = |key: &str| -> f64 {
        input
            .performance_features
            .get(key)
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            / duration_min
    };
    let final_per10 = per10("finalHit");
    let final_hit = final_per10.tanh();
    let kill_per10 = per10("kill");
    let non_final = (kill_per10 - final_per10).max(0.0).tanh();
    let assist = per10("assist").tanh();
    let death = -per10("death").tanh();
    NormalizedPerformanceRow {
        role: input.role,
        mode: input.mode_family,
        final_hit,
        non_final_eliminations: non_final,
        assist,
        death,
    }
}

fn v3_performance_correction(
    model: &SelectedRoleModel,
    row: &NormalizedPerformanceRow,
    mode: ModeFamily,
) -> f64 {
    let cap = model.cap_for(mode);
    let raw = model.evaluate(row);
    if !raw.is_finite() {
        return 0.0;
    }
    raw.clamp(-cap, cap)
}

fn replay_events_v3(
    tx: &Transaction<'_>,
    inputs: &[MmrMatchInput],
    computed_at: &str,
    artifact: &CareerMmrV3Artifact,
) -> Result<String> {
    tx.execute(
        "DELETE FROM career_mmr_events WHERE model_version=?1",
        [PERFORMANCE_MODEL_VERSION],
    )?;
    let mut states: HashMap<Role, RoleRatingState> = HashMap::new();
    let mut digest_material = String::new();
    for input in inputs {
        let model = artifact
            .selected_models
            .get(&input.role)
            .expect("validated artifact selects every role");
        let before = states
            .get(&input.role)
            .copied()
            .unwrap_or_else(|| RoleRatingState::new(input.begin_ts, input.season));
        let expected = expected_win(input.team_strength, input.opponent_strength);
        let rating_input = RatingInput {
            result: input.result,
            mode: input.mode_family,
            team_strength: input.team_strength,
            opponent_strength: input.opponent_strength,
            role_share: input.role_share,
            data_quality: input.data_quality,
            played_at_seconds: input.begin_ts,
            season: input.season,
        };
        // The per-event correction is bounded by the selected tier/mode cap before
        // being handed to the rating kernel, which then enforces the final mode
        // cap (+/-30 / +/-10) and sign-preservation on the combined delta.
        let correction =
            v3_performance_correction(model, &normalized_performance_row(input), input.mode_family);
        let update = update_rating_for_expected(before, rating_input, expected, correction)
            .context("MMR ???????????????")?;
        if update.new_state.last_played_seconds != Some(input.begin_ts) {
            anyhow::bail!("MMR ???????????????");
        }
        states.insert(input.role, update.new_state);
        let hash = input_hash(input);
        tx.execute(
            "INSERT INTO career_mmr_events(model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,performance_tier,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
            params![PERFORMANCE_MODEL_VERSION,input.match_id,input.role.as_str(),input.season,input.begin_ts,input.mode_family.as_str(),input.role_share,input.result,update.expected_win_rate,input.data_quality,update.performance_adjustment,model.tier().as_str(),before.mu,update.prepared_sigma_before,update.base_delta,update.delta,update.new_state.mu,update.new_state.sigma,update.confidence,hash,computed_at],
        )?;
        digest_material.push_str(&format!(
            "{}|{}|{}|{:.9}|{:.9}|{:.9};",
            hash,
            input.season,
            input.role.as_str(),
            update.new_state.mu,
            update.new_state.sigma,
            update.confidence
        ));
    }
    Ok(sha256_hex(&digest_material))
}

fn ensure_model_version(tx: &Transaction<'_>) -> Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO mmr_model_versions(version, algorithm, config_json, is_active, created_at) VALUES (?1, ?2, ?3, 0, '1970-01-01T00:00:00+00:00')",
        params![BASELINE_MODEL_VERSION, ALGORITHM, CONFIG_JSON],
    )?;
    let stored = tx.query_row(
        "SELECT algorithm, config_json FROM mmr_model_versions WHERE version=?1",
        [BASELINE_MODEL_VERSION],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    if stored != (ALGORITHM.to_string(), CONFIG_JSON.to_string()) {
        anyhow::bail!("MMR 模型版本定义冲突，已保留上一套完整结果");
    }
    Ok(())
}

fn extract_all_inputs(
    tx: &Transaction<'_>,
    context: &ExtractionContext,
) -> Result<Vec<MmrMatchInput>> {
    let matches = {
        let mut stmt = tx.prepare("SELECT match_id, bucket FROM matches ORDER BY match_id")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let mut inputs = Vec::new();
    for (id, bucket) in matches {
        let input = if bucket == "stadium" {
            extract_stadium_match_with_context(tx, &id, context)?
        } else {
            extract_ordinary_match_with_context(tx, &id, context)?
        };
        if let Some(input) = input {
            inputs.push(input);
        }
    }
    Ok(inputs)
}

fn replace_inputs(tx: &Transaction<'_>, inputs: &[MmrMatchInput]) -> Result<()> {
    tx.execute("DELETE FROM mmr_match_inputs", [])?;
    for input in inputs {
        let hash = input_hash(input);
        tx.execute(
            "INSERT INTO mmr_match_inputs(match_id, role, season, begin_ts, mode_family, result, duration_sec, role_share, role_source, data_quality, team_strength, opponent_strength, rank_coverage, performance_features_json, input_hash) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![input.match_id, input.role.as_str(), input.season, input.begin_ts, input.mode_family.as_str(), input.result, input.duration_sec, input.role_share, input.role_source, input.data_quality, input.team_strength, input.opponent_strength, input.rank_coverage, canonical_json(&input.performance_features), hash],
        )?;
    }
    Ok(())
}

fn replay_events(
    tx: &Transaction<'_>,
    inputs: &[MmrMatchInput],
    computed_at: &str,
) -> Result<String> {
    tx.execute(
        "DELETE FROM career_mmr_events WHERE model_version=?1",
        [BASELINE_MODEL_VERSION],
    )?;
    let mut states: HashMap<Role, RoleRatingState> = HashMap::new();
    let mut digest_material = String::new();
    for input in inputs {
        let before = states
            .get(&input.role)
            .copied()
            .unwrap_or_else(|| RoleRatingState::new(input.begin_ts, input.season));
        let update = update_rating(
            before,
            RatingInput {
                result: input.result,
                mode: input.mode_family,
                team_strength: input.team_strength,
                opponent_strength: input.opponent_strength,
                role_share: input.role_share,
                data_quality: input.data_quality,
                played_at_seconds: input.begin_ts,
                season: input.season,
            },
        )
        .context("MMR 状态无效，已保留上一套完整结果")?;
        if update.new_state.last_played_seconds != Some(input.begin_ts) {
            anyhow::bail!("MMR 输入无效，已保留上一套完整结果");
        }
        states.insert(input.role, update.new_state);
        let hash = input_hash(input);
        tx.execute(
            "INSERT INTO career_mmr_events(model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0.0,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            params![BASELINE_MODEL_VERSION,input.match_id,input.role.as_str(),input.season,input.begin_ts,input.mode_family.as_str(),input.role_share,input.result,update.expected_win_rate,input.data_quality,before.mu,update.prepared_sigma_before,update.base_delta,update.delta,update.new_state.mu,update.new_state.sigma,update.confidence,hash,computed_at],
        )?;
        digest_material.push_str(&format!(
            "{}|{}|{}|{:.9}|{:.9}|{:.9};",
            hash,
            input.season,
            input.role.as_str(),
            update.new_state.mu,
            update.new_state.sigma,
            update.confidence
        ));
    }
    Ok(sha256_hex(&digest_material))
}

fn rebuild_summaries(tx: &Transaction<'_>, now: &str, model_version: i64) -> Result<()> {
    tx.execute(
        "DELETE FROM career_role_mmr WHERE model_version=?1",
        [model_version],
    )?;
    tx.execute(
        "DELETE FROM career_role_mode_summary WHERE model_version=?1",
        [model_version],
    )?;
    tx.execute(
        "DELETE FROM career_role_season_summary WHERE model_version=?1",
        [model_version],
    )?;
    tx.execute(r#"INSERT INTO career_role_mmr(model_version,role,mmr,mu,sigma,confidence,effective_matches,effective_hours,peak_mmr,peak_at,last_played_at,updated_at)
        SELECT ?1,e.role,MIN(5000,MAX(0,CAST(ROUND(e.mu_after) AS INTEGER))),e.mu_after,e.sigma_after,e.confidence_after,
          (SELECT SUM(role_share*data_quality) FROM career_mmr_events x WHERE x.model_version=?1 AND x.role=e.role),
          (SELECT SUM(i.duration_sec*i.role_share)/3600.0 FROM mmr_match_inputs i WHERE i.role=e.role),
          NULL,NULL,
          e.begin_ts,?2 FROM career_mmr_events e WHERE e.model_version=?1 AND e.rowid=(SELECT MAX(x.rowid) FROM career_mmr_events x WHERE x.model_version=?1 AND x.role=e.role)"#, params![model_version, now])?;
    tx.execute(r#"INSERT INTO career_role_mode_summary(model_version,role,mode_family,matches_count,duration_sec,wins,losses,draws,cumulative_delta,updated_at) SELECT ?1,e.role,e.mode_family,COUNT(*),SUM(CAST(ROUND(i.duration_sec*i.role_share) AS INTEGER)),SUM(CASE WHEN e.result=1.0 THEN 1 ELSE 0 END),SUM(CASE WHEN e.result=0.0 THEN 1 ELSE 0 END),SUM(CASE WHEN e.result=0.5 THEN 1 ELSE 0 END),SUM(e.final_delta),?2 FROM career_mmr_events e JOIN mmr_match_inputs i ON i.match_id=e.match_id AND i.role=e.role WHERE e.model_version=?1 GROUP BY e.role,e.mode_family"#, params![model_version, now])?;
    tx.execute(r#"INSERT INTO career_role_season_summary SELECT ?1,season,role,COUNT(*),MIN(5000,MAX(0,CAST(ROUND((SELECT z.mu_before FROM career_mmr_events z WHERE z.model_version=?1 AND z.season=e.season AND z.role=e.role ORDER BY z.begin_ts,z.rowid LIMIT 1)) AS INTEGER))),MIN(5000,MAX(0,CAST(ROUND((SELECT z.mu_after FROM career_mmr_events z WHERE z.model_version=?1 AND z.season=e.season AND z.role=e.role ORDER BY z.begin_ts DESC,z.rowid DESC LIMIT 1)) AS INTEGER))),MIN(5000,MAX(0,MAX(CAST(ROUND(mu_after) AS INTEGER)))),SUM(final_delta),MIN(begin_ts),MAX(begin_ts),?2 FROM career_mmr_events e WHERE model_version=?1 GROUP BY season,role"#, params![model_version, now])?;
    let global_watermark: Option<(i64, i64)> = tx
        .query_row(
            "SELECT begin_ts,season FROM mmr_match_inputs ORDER BY begin_ts DESC,season DESC,input_hash DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((watermark_ts, watermark_season)) = global_watermark {
        for role in [Role::Tank, Role::Damage, Role::Support] {
            let terminal: Option<(f64, f64, f64, i64, i64, f64)> = tx
                .query_row(
                    "SELECT e.mu_after,e.sigma_after,(SELECT SUM(x.role_share*x.data_quality) FROM career_mmr_events x WHERE x.model_version=e.model_version AND x.role=e.role),e.begin_ts,e.season,e.data_quality FROM career_mmr_events e WHERE e.model_version=?1 AND e.role=?2 ORDER BY e.begin_ts DESC,e.rowid DESC LIMIT 1",
                    params![model_version, role.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .optional()?;
            if let Some((mu, sigma, effective_matches, last_ts, last_season, quality)) = terminal {
                let prepared = apply_inactivity(
                    RoleRatingState {
                        mu,
                        sigma,
                        effective_matches,
                        last_played_seconds: Some(last_ts),
                        last_season: Some(last_season),
                    },
                    watermark_ts,
                    watermark_season,
                )?;
                tx.execute(
                    "UPDATE career_role_mmr SET sigma=?1,confidence=?2 WHERE model_version=?3 AND role=?4",
                    params![prepared.sigma, confidence(prepared, quality), model_version, role.as_str()],
                )?;
            }
        }
    }
    for role in [Role::Tank, Role::Damage, Role::Support] {
        let events = {
            let mut stmt = tx.prepare("SELECT begin_ts, mu_after, confidence_after, role_share*data_quality FROM career_mmr_events WHERE model_version=?1 AND role=?2 ORDER BY begin_ts, rowid")?;
            let rows = stmt
                .query_map(params![model_version, role.as_str()], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        if let Some((peak, peak_at)) = rolling_peak(&events) {
            tx.execute("UPDATE career_role_mmr SET peak_mmr=MIN(5000,MAX(0,?1)), peak_at=?2 WHERE model_version=?3 AND role=?4", params![peak, peak_at, model_version, role.as_str()])?;
        }
        let seasons = events_by_season(tx, role, model_version)?;
        for (season, season_events) in seasons {
            // The schema requires a value even before the confidence/sample gate opens.
            // Use the season's starting rating rather than promoting an ineligible spike.
            tx.execute("UPDATE career_role_season_summary SET peak_mmr=start_mmr WHERE model_version=?1 AND season=?2 AND role=?3", params![model_version, season, role.as_str()])?;
            if let Some((peak, _)) = rolling_peak(&season_events) {
                tx.execute("UPDATE career_role_season_summary SET peak_mmr=MIN(5000,MAX(0,?1)) WHERE model_version=?2 AND season=?3 AND role=?4", params![peak, model_version, season, role.as_str()])?;
            }
        }
    }
    Ok(())
}

fn events_by_season(
    tx: &Transaction<'_>,
    role: Role,
    model_version: i64,
) -> Result<BTreeMap<i64, Vec<(i64, f64, f64, f64)>>> {
    let mut result: BTreeMap<i64, Vec<_>> = BTreeMap::new();
    let mut stmt = tx.prepare("SELECT season,begin_ts,mu_after,confidence_after,role_share*data_quality FROM career_mmr_events WHERE model_version=?1 AND role=?2 ORDER BY begin_ts,rowid")?;
    let rows = stmt.query_map(params![model_version, role.as_str()], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            (row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?),
        ))
    })?;
    for row in rows {
        let (season, event) = row?;
        result.entry(season).or_default().push(event);
    }
    Ok(result)
}

fn build_watermark(inputs: &[MmrMatchInput]) -> String {
    let seconds = inputs.iter().map(|input| input.begin_ts).max().unwrap_or(0);
    Utc.timestamp_opt(seconds.max(0), 0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap())
        .to_rfc3339()
}

fn rolling_peak(events: &[(i64, f64, f64, f64)]) -> Option<(i64, i64)> {
    const WINDOW: i64 = 7 * 86_400;
    let mut effective = 0.0;
    let mut best: Option<(f64, i64)> = None;
    let mut window = MedianWindow::default();
    let mut queue = VecDeque::new();
    for &(timestamp, mu, confidence, increment) in events {
        effective += increment;
        let quantized = (mu * 1_000_000.0).round() as i64;
        window.insert(quantized);
        queue.push_back((timestamp, quantized));
        let start = timestamp.saturating_sub(WINDOW);
        while queue.front().is_some_and(|(ts, _)| *ts < start) {
            let (_, expired) = queue.pop_front().unwrap();
            window.remove(expired);
        }
        if confidence < 0.60 || effective < 10.0 {
            continue;
        }
        let value = window.median() / 1_000_000.0;
        if best.is_none_or(|(current, _)| value > current) {
            best = Some((value, timestamp));
        }
    }
    best.map(|(value, timestamp)| (value.round() as i64, timestamp))
}

#[derive(Default)]
struct MedianWindow {
    lower: BTreeMap<i64, usize>,
    upper: BTreeMap<i64, usize>,
    lower_len: usize,
    upper_len: usize,
}
impl MedianWindow {
    fn insert(&mut self, value: i64) {
        if self
            .lower
            .keys()
            .next_back()
            .is_none_or(|max| value <= *max)
        {
            add(&mut self.lower, value);
            self.lower_len += 1;
        } else {
            add(&mut self.upper, value);
            self.upper_len += 1;
        }
        self.rebalance();
    }
    fn remove(&mut self, value: i64) {
        if remove(&mut self.lower, value) {
            self.lower_len -= 1;
        } else if remove(&mut self.upper, value) {
            self.upper_len -= 1;
        }
        self.rebalance();
    }
    fn rebalance(&mut self) {
        while self.lower_len > self.upper_len + 1 {
            let value = *self.lower.keys().next_back().unwrap();
            remove(&mut self.lower, value);
            self.lower_len -= 1;
            add(&mut self.upper, value);
            self.upper_len += 1;
        }
        while self.upper_len > self.lower_len {
            let value = *self.upper.keys().next().unwrap();
            remove(&mut self.upper, value);
            self.upper_len -= 1;
            add(&mut self.lower, value);
            self.lower_len += 1;
        }
    }
    fn median(&self) -> f64 {
        let low = *self.lower.keys().next_back().unwrap() as f64;
        if self.lower_len == self.upper_len {
            (low + *self.upper.keys().next().unwrap() as f64) / 2.0
        } else {
            low
        }
    }
}
fn add(map: &mut BTreeMap<i64, usize>, value: i64) {
    *map.entry(value).or_default() += 1;
}
fn remove(map: &mut BTreeMap<i64, usize>, value: i64) -> bool {
    let Some(count) = map.get_mut(&value) else {
        return false;
    };
    *count -= 1;
    if *count == 0 {
        map.remove(&value);
    }
    true
}

#[cfg(test)]
fn weighted_duration_seconds(duration: i64, role_share: f64) -> i64 {
    (duration as f64 * role_share).round() as i64
}

#[cfg(test)]
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn canonical_json(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".into())
}
fn input_hash(input: &MmrMatchInput) -> String {
    let material = format!(
        "{}|{}|{}|{}|{:.9}|{}|{:.9}|{}|{:.9}|{:?}|{:?}|{:.9}|{}",
        sha256_hex(&input.match_id),
        input.season,
        input.begin_ts,
        input.mode_family.as_str(),
        input.result,
        input.duration_sec,
        input.role_share,
        input.role.as_str(),
        input.data_quality,
        input.team_strength,
        input.opponent_strength,
        input.rank_coverage,
        canonical_json(&input.performance_features)
    );
    sha256_hex(&material)
}
fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{rolling_peak, weighted_duration_seconds};

    #[test]
    fn rolling_peak_suppresses_a_single_match_spike_and_includes_seven_day_boundary() {
        let day = 86_400;
        let events = (0..12)
            .map(|index| {
                let mu = if index == 10 { 3000.0 } else { 2500.0 };
                ((index as i64 + 1) * day, mu, 0.7, 1.0)
            })
            .collect::<Vec<_>>();
        assert_eq!(rolling_peak(&events), Some((2500, 10 * day)));
    }

    #[test]
    fn rolling_peak_uses_even_average_and_recognizes_sustained_high_rating() {
        let day = 86_400;
        let mut events = (0..10)
            .map(|index| ((index as i64 + 1) * day, 2500.0, 0.7, 1.0))
            .collect::<Vec<_>>();
        events.extend((10..18).map(|index| ((index as i64 + 1) * day, 2700.0, 0.7, 1.0)));
        assert_eq!(rolling_peak(&events), Some((2700, 15 * day)));
        assert_eq!(super::median(&mut [2400.0, 2600.0]), 2500.0);
    }

    #[test]
    fn mode_duration_is_split_by_role_share() {
        assert_eq!(weighted_duration_seconds(1_000, 0.4), 400);
        assert_eq!(weighted_duration_seconds(1_000, 0.6), 600);
    }

    #[test]
    fn season_peak_uses_the_same_spike_suppressing_window_rules() {
        let day = 86_400;
        let events = (0..20)
            .map(|index| {
                let mu = if index == 15 { 3200.0 } else { 2600.0 };
                ((index as i64 + 1) * day, mu, 0.8, 1.0)
            })
            .collect::<Vec<_>>();
        assert_eq!(rolling_peak(&events), Some((2600, 10 * day)));
    }

    #[test]
    fn rolling_peak_handles_a_large_sequence_with_a_stable_result() {
        let events = (0..20_000)
            .map(|index| (index * 60, 2500.0 + (index % 11) as f64, 0.9, 1.0))
            .collect::<Vec<_>>();
        let peak = rolling_peak(&events).unwrap();
        assert!((2500..=2510).contains(&peak.0));
    }
}
