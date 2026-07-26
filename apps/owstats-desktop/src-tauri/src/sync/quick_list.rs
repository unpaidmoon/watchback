use crate::db::OwDatabase;
use crate::models::{QuickListCommitSummary, QuickModeResult};
use crate::season_fetch::{QuickListModeProbe, QuickListProbe};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BackgroundPageCommit {
    pub new_matches: usize,
    pub boundary_reached: bool,
}

pub fn commit_quick_list_probe(
    db: &OwDatabase,
    probe: QuickListProbe,
) -> Result<QuickListCommitSummary> {
    db.initialize()?;
    let mut conn = db.connect()?;
    let tx = conn.transaction()?;
    let now = Utc::now().to_rfc3339();
    super::state::ensure_season_state(&tx, probe.season, &now)?;

    let mut results = Vec::with_capacity(probe.modes.len());
    let mut new_match_ids = Vec::new();
    let mut mode_work = Vec::new();
    let mut partial_failure = false;

    for mode in probe.modes {
        let result = commit_mode_page_one(
            &tx,
            probe.season,
            &now,
            mode,
            &mut new_match_ids,
            &mut mode_work,
        )?;
        partial_failure |= matches!(result.outcome.as_str(), "failed" | "unsafe");
        results.push(result);
    }

    let data_changed = !new_match_ids.is_empty();
    if data_changed {
        tx.execute(
            r#"
            UPDATE season_sync_state
            SET season_revision=season_revision+1, updated_at=?2
            WHERE season=?1
            "#,
            params![probe.season, now],
        )?;
    }
    let target_revision: i64 = tx.query_row(
        "SELECT season_revision FROM season_sync_state WHERE season=?1",
        [probe.season],
        |row| row.get(0),
    )?;
    let needs_background = !mode_work.is_empty() || !new_match_ids.is_empty();
    let background_status = if needs_background { "queued" } else { "idle" };
    if needs_background {
        enqueue_background_job(
            &tx,
            probe.season,
            target_revision,
            &mode_work,
            &new_match_ids,
            &now,
        )?;
    }
    tx.execute(
        r#"
        UPDATE season_sync_state
        SET last_checked_at=?2,
            background_status=?3,
            pending_detail_count=?4,
            updated_at=?2
        WHERE season=?1
        "#,
        params![
            probe.season,
            now,
            background_status,
            new_match_ids.len() as i64
        ],
    )?;
    tx.commit()?;

    Ok(QuickListCommitSummary {
        season: probe.season,
        new_list_matches: new_match_ids.len(),
        data_changed,
        partial_failure,
        pending_details: new_match_ids.len(),
        background_status: background_status.to_string(),
        checked_at: now,
        modes: results,
    })
}

fn commit_mode_page_one(
    tx: &Transaction<'_>,
    season: i64,
    now: &str,
    mode: QuickListModeProbe,
    new_match_ids: &mut Vec<String>,
    mode_work: &mut Vec<super::ModeWork>,
) -> Result<QuickModeResult> {
    let Some(payload) = mode.payload else {
        mode_work.push(super::ModeWork {
            match_kind: mode.match_kind.clone(),
            game_mode: mode.game_mode.clone(),
            request_shape: mode.request_shape,
            next_page: 1,
            max_page: crate::season_fetch::QUICK_UPDATE_MAX_PAGES_PER_MODE,
        });
        return Ok(QuickModeResult {
            match_kind: mode.match_kind,
            game_mode: mode.game_mode,
            outcome: "failed".to_string(),
            new_matches: 0,
            error_class: mode.error_class.or_else(|| Some("network".to_string())),
        });
    };
    let known_ids = known_mode_match_ids(tx, season, &mode.match_kind, &mode.game_mode)?;
    let watermark = tx
        .query_row(
            r#"
            SELECT latest_begin_ts FROM season_sync_cursors
            WHERE season=?1 AND match_kind=?2 AND game_mode=?3
            "#,
            params![season, mode.match_kind, mode.game_mode],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten()
        .or(canonical_mode_watermark(
            tx,
            season,
            &mode.match_kind,
            &mode.game_mode,
        )?);
    let decision = super::classify_quick_page(&payload, &known_ids, watermark);
    if matches!(decision, super::QuickPageDecision::Unsafe) {
        mode_work.push(super::ModeWork {
            match_kind: mode.match_kind.clone(),
            game_mode: mode.game_mode.clone(),
            request_shape: mode.request_shape,
            next_page: 1,
            max_page: crate::season_fetch::QUICK_UPDATE_MAX_PAGES_PER_MODE,
        });
        return Ok(QuickModeResult {
            match_kind: mode.match_kind,
            game_mode: mode.game_mode,
            outcome: "unsafe".to_string(),
            new_matches: 0,
            error_class: Some("invalidPayload".to_string()),
        });
    }

    let mut entries = crate::season_fetch::extract_match_entries(&payload);
    for entry in &mut entries {
        entry["gameMode"] = Value::String(mode.game_mode.clone());
        entry["match_kind"] = Value::String(mode.match_kind.clone());
        entry["_dashenSeason"] = Value::Number(season.into());
    }
    let mut mode_new = 0;
    let mut page_latest = None;
    for entry in &entries {
        let Some(match_id) = crate::importer::string_field(entry, "matchId") else {
            continue;
        };
        let begin_ts =
            crate::importer::normalize_timestamp(crate::importer::int_field(entry, "beginTs"));
        page_latest = Some(page_latest.map_or(begin_ts, |latest: i64| latest.max(begin_ts)));
        if !known_ids.contains(&match_id) {
            new_match_ids.push(match_id.clone());
            mode_new += 1;
        }
        upsert_list_entry(tx, season, &mode.match_kind, entry, now)?;
    }

    let (outcome, stop_reason, empty_confirmed) = match decision {
        super::QuickPageDecision::Empty => ("emptyBoundary", "empty", 1),
        super::QuickPageDecision::WatermarkReached => ("unchangedBoundary", "watermarkReached", 0),
        super::QuickPageDecision::Continue => {
            mode_work.push(super::ModeWork {
                match_kind: mode.match_kind.clone(),
                game_mode: mode.game_mode.clone(),
                request_shape: mode.request_shape,
                next_page: 2,
                max_page: crate::season_fetch::QUICK_UPDATE_MAX_PAGES_PER_MODE,
            });
            ("newEntries", "continue", 0)
        }
        super::QuickPageDecision::Unsafe => unreachable!(),
    };
    tx.execute(
        r#"
        INSERT INTO season_sync_cursors (
            season, match_kind, game_mode, request_shape, latest_begin_ts,
            empty_confirmed, last_checked_at, last_stop_reason
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(season, match_kind, game_mode) DO UPDATE SET
            request_shape=excluded.request_shape,
            latest_begin_ts=CASE
                WHEN excluded.latest_begin_ts IS NULL THEN season_sync_cursors.latest_begin_ts
                WHEN season_sync_cursors.latest_begin_ts IS NULL THEN excluded.latest_begin_ts
                ELSE MAX(season_sync_cursors.latest_begin_ts, excluded.latest_begin_ts)
            END,
            empty_confirmed=excluded.empty_confirmed,
            last_checked_at=excluded.last_checked_at,
            last_stop_reason=excluded.last_stop_reason
        "#,
        params![
            season,
            mode.match_kind,
            mode.game_mode,
            mode.request_shape.as_str(),
            page_latest,
            empty_confirmed,
            now,
            stop_reason
        ],
    )?;
    Ok(QuickModeResult {
        match_kind: mode.match_kind,
        game_mode: mode.game_mode,
        outcome: outcome.to_string(),
        new_matches: mode_new,
        error_class: None,
    })
}

fn known_mode_match_ids(
    tx: &Transaction<'_>,
    season: i64,
    match_kind: &str,
    game_mode: &str,
) -> Result<HashSet<String>> {
    let (modes, kind) = mode_storage_names(match_kind, game_mode);
    let mut statement = tx.prepare(
        "SELECT match_id FROM matches WHERE season=?1 AND match_kind=?2 AND game_mode IN (?3,?4,?5)",
    )?;
    let rows = statement.query_map(
        params![
            season,
            kind,
            modes.first().copied().unwrap_or(""),
            modes.get(1).copied().unwrap_or(""),
            modes.get(2).copied().unwrap_or("")
        ],
        |row| row.get::<_, String>(0),
    )?;
    Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
}

fn canonical_mode_watermark(
    tx: &Transaction<'_>,
    season: i64,
    match_kind: &str,
    game_mode: &str,
) -> Result<Option<i64>> {
    let (modes, kind) = mode_storage_names(match_kind, game_mode);
    Ok(tx.query_row(
        "SELECT MAX(begin_ts) FROM matches WHERE season=?1 AND match_kind=?2 AND game_mode IN (?3,?4,?5)",
        params![
            season,
            kind,
            modes.first().copied().unwrap_or(""),
            modes.get(1).copied().unwrap_or(""),
            modes.get(2).copied().unwrap_or("")
        ],
        |row| row.get(0),
    )?)
}

fn mode_storage_names<'a>(match_kind: &'a str, game_mode: &'a str) -> (Vec<&'a str>, &'a str) {
    match (match_kind, game_mode) {
        ("normal", "sport") => (vec!["sport", "SportPreset", "SportOpen"], "normal"),
        ("normal", "leisure") => (vec!["leisure", "LeisurePreset", "Leisure6v6"], "normal"),
        _ => (vec![game_mode], match_kind),
    }
}

fn upsert_list_entry(
    tx: &Transaction<'_>,
    season: i64,
    match_kind: &str,
    entry: &Value,
    now: &str,
) -> Result<()> {
    let match_id = crate::importer::string_field(entry, "matchId").unwrap_or_default();
    let mode = crate::importer::normalized_game_mode(entry);
    let map_guid = crate::importer::string_field(entry, "mapGuid").unwrap_or_default();
    let hero_guid = crate::importer::string_field(entry, "heroGuid").unwrap_or_default();
    tx.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result,
            map_guid, map_name, hero_guid, hero_name, begin_ts,
            team_score, opponent_score, kills, assists, deaths, damage,
            healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6,
            ?7, (SELECT name FROM map_dict WHERE guid=?7),
            ?8, (SELECT name FROM hero_dict WHERE guid=?8), ?9,
            ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, 0, ?19, ?20
        )
        ON CONFLICT(match_id) DO UPDATE SET
            season=excluded.season,
            bucket=excluded.bucket,
            game_mode=excluded.game_mode,
            match_kind=excluded.match_kind,
            result=excluded.result,
            map_guid=excluded.map_guid,
            map_name=COALESCE(excluded.map_name, matches.map_name),
            hero_guid=excluded.hero_guid,
            hero_name=COALESCE(excluded.hero_name, matches.hero_name),
            begin_ts=excluded.begin_ts,
            team_score=excluded.team_score,
            opponent_score=excluded.opponent_score,
            kills=excluded.kills,
            assists=excluded.assists,
            deaths=excluded.deaths,
            damage=excluded.damage,
            healing=excluded.healing,
            mitigation=excluded.mitigation,
            total_worth=excluded.total_worth,
            duration_sec=CASE WHEN matches.duration_sec>0 THEN matches.duration_sec ELSE 0 END,
            raw_entry_json=excluded.raw_entry_json,
            updated_at=excluded.updated_at
        "#,
        params![
            match_id,
            season,
            crate::importer::bucket_for(&mode, match_kind),
            mode,
            match_kind,
            crate::importer::result_label(crate::importer::int_field(entry, "matchRet")),
            map_guid,
            hero_guid,
            crate::importer::normalize_timestamp(crate::importer::int_field(entry, "beginTs")),
            crate::importer::int_field(entry, "teamScore"),
            crate::importer::int_field(entry, "opponentScore"),
            crate::importer::int_field(entry, "kill"),
            crate::importer::int_field(entry, "assist"),
            crate::importer::int_field(entry, "death"),
            crate::importer::int_field(entry, "heroDamage"),
            crate::importer::int_field(entry, "cure"),
            crate::importer::int_field(entry, "resistDamage"),
            crate::importer::int_field(entry, "totalWorth"),
            serde_json::to_string(entry)?,
            now
        ],
    )?;
    Ok(())
}

fn enqueue_background_job(
    tx: &Transaction<'_>,
    season: i64,
    target_revision: i64,
    modes: &[super::ModeWork],
    match_ids: &[String],
    now: &str,
) -> Result<()> {
    let existing = tx
        .query_row(
            "SELECT job_id FROM season_sync_jobs WHERE season=?1 AND status IN ('queued','running','retryable') LIMIT 1",
            [season],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let job_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
    tx.execute(
        r#"
        INSERT INTO season_sync_jobs (
            job_id, season, target_revision, phase, status,
            retry_count, created_at, updated_at
        ) VALUES (?1, ?2, ?3, 'continuePages', 'queued', 0, ?4, ?4)
        ON CONFLICT(job_id) DO UPDATE SET
            target_revision=MAX(target_revision, excluded.target_revision),
            updated_at=excluded.updated_at
        "#,
        params![job_id, season, target_revision, now],
    )?;
    for mode in modes {
        tx.execute(
            r#"
            INSERT INTO season_sync_job_modes (
                job_id, match_kind, game_mode, request_shape,
                next_page, max_page, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued')
            ON CONFLICT(job_id, match_kind, game_mode) DO UPDATE SET
                next_page=MIN(next_page, excluded.next_page),
                max_page=MAX(max_page, excluded.max_page),
                request_shape=excluded.request_shape,
                status='queued'
            "#,
            params![
                job_id,
                mode.match_kind,
                mode.game_mode,
                mode.request_shape.as_str(),
                mode.next_page,
                mode.max_page
            ],
        )?;
    }
    for match_id in match_ids {
        tx.execute(
            r#"
            INSERT INTO season_sync_job_matches (job_id, match_id, detail_status, updated_at)
            VALUES (?1, ?2, 'queued', ?3)
            ON CONFLICT(job_id, match_id) DO NOTHING
            "#,
            params![job_id, match_id, now],
        )?;
    }
    Ok(())
}

pub(super) fn commit_background_page(
    store: &super::SeasonSyncStore,
    job: &super::ClaimedSeasonJob,
    mode: &super::ModeWork,
    page: i64,
    payload: Value,
) -> Result<BackgroundPageCommit> {
    let mut conn = store.database().connect()?;
    let tx = conn.transaction()?;
    let now = Utc::now().to_rfc3339();
    let known_ids = known_mode_match_ids(&tx, job.season, &mode.match_kind, &mode.game_mode)?;
    let watermark = tx
        .query_row(
            "SELECT latest_begin_ts FROM season_sync_cursors WHERE season=?1 AND match_kind=?2 AND game_mode=?3",
            params![job.season, mode.match_kind, mode.game_mode],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten()
        .or(canonical_mode_watermark(
            &tx,
            job.season,
            &mode.match_kind,
            &mode.game_mode,
        )?);
    let decision = super::classify_quick_page(&payload, &known_ids, watermark);
    if decision == super::QuickPageDecision::Unsafe {
        tx.execute(
            "UPDATE season_sync_job_modes SET status='retryable', stop_reason='unsafeResponse' WHERE job_id=?1 AND match_kind=?2 AND game_mode=?3",
            params![job.job_id, mode.match_kind, mode.game_mode],
        )?;
        tx.commit()?;
        return Err(anyhow::anyhow!("invalid background list payload"));
    }
    let mut entries = crate::season_fetch::extract_match_entries(&payload);
    for entry in &mut entries {
        entry["gameMode"] = Value::String(mode.game_mode.clone());
        entry["match_kind"] = Value::String(mode.match_kind.clone());
        entry["_dashenSeason"] = Value::Number(job.season.into());
    }
    let mut new_ids = Vec::new();
    let mut latest = None;
    for entry in &entries {
        let Some(match_id) = crate::importer::string_field(entry, "matchId") else {
            continue;
        };
        let begin_ts =
            crate::importer::normalize_timestamp(crate::importer::int_field(entry, "beginTs"));
        latest = Some(latest.map_or(begin_ts, |current: i64| current.max(begin_ts)));
        if !known_ids.contains(&match_id) {
            new_ids.push(match_id.clone());
        }
        upsert_list_entry(&tx, job.season, &mode.match_kind, entry, &now)?;
    }
    if !new_ids.is_empty() {
        tx.execute(
            "UPDATE season_sync_state SET season_revision=season_revision+1, updated_at=?2 WHERE season=?1",
            params![job.season, now],
        )?;
        for match_id in &new_ids {
            tx.execute(
                r#"
                INSERT INTO season_sync_job_matches (job_id, match_id, detail_status, updated_at)
                VALUES (?1, ?2, 'queued', ?3)
                ON CONFLICT(job_id, match_id) DO NOTHING
                "#,
                params![job.job_id, match_id, now],
            )?;
        }
    }
    let boundary_reached = matches!(
        decision,
        super::QuickPageDecision::Empty | super::QuickPageDecision::WatermarkReached
    ) || page >= mode.max_page;
    let stop_reason = match decision {
        super::QuickPageDecision::Empty => "empty",
        super::QuickPageDecision::WatermarkReached => "watermarkReached",
        super::QuickPageDecision::Continue if page >= mode.max_page => "pageLimit",
        super::QuickPageDecision::Continue => "continue",
        super::QuickPageDecision::Unsafe => unreachable!(),
    };
    tx.execute(
        r#"
        UPDATE season_sync_cursors
        SET latest_begin_ts=CASE
                WHEN ?4 IS NULL THEN latest_begin_ts
                WHEN latest_begin_ts IS NULL THEN ?4
                ELSE MAX(latest_begin_ts, ?4)
            END,
            empty_confirmed=CASE WHEN ?5='empty' THEN 1 ELSE 0 END,
            last_checked_at=?6,
            last_stop_reason=?5
        WHERE season=?1 AND match_kind=?2 AND game_mode=?3
        "#,
        params![
            job.season,
            mode.match_kind,
            mode.game_mode,
            latest,
            stop_reason,
            now
        ],
    )?;
    tx.execute(
        r#"
        UPDATE season_sync_job_modes
        SET next_page=?4,
            status=?5,
            stop_reason=?6
        WHERE job_id=?1 AND match_kind=?2 AND game_mode=?3
        "#,
        params![
            job.job_id,
            mode.match_kind,
            mode.game_mode,
            page + 1,
            if boundary_reached {
                "completed"
            } else {
                "running"
            },
            stop_reason
        ],
    )?;
    let revision: i64 = tx.query_row(
        "SELECT season_revision FROM season_sync_state WHERE season=?1",
        [job.season],
        |row| row.get(0),
    )?;
    tx.execute(
        "UPDATE season_sync_jobs SET target_revision=?2, updated_at=?3 WHERE job_id=?1",
        params![job.job_id, revision, now],
    )?;
    tx.execute(
        r#"
        UPDATE season_sync_state
        SET pending_detail_count=(
                SELECT COUNT(*) FROM season_sync_job_matches
                WHERE job_id=?2 AND detail_status IN ('queued','retryable')
            ),
            updated_at=?3
        WHERE season=?1
        "#,
        params![job.season, job.job_id, now],
    )?;
    tx.commit()?;
    Ok(BackgroundPageCommit {
        new_matches: new_ids.len(),
        boundary_reached,
    })
}
