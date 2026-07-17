use super::{ClaimedSeasonJob, RequestShape, SeasonSyncStore};
use crate::dashen_client::DashenClient;
use crate::season_fetch::FullSeasonRequest;
use anyhow::{anyhow, Result};
use chrono::Utc;
use futures_util::{stream, StreamExt};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackgroundRunSummary {
    pub season: i64,
    pub pages_scanned: usize,
    pub new_list_matches: usize,
    pub details_written: usize,
    pub failed_details: usize,
    pub snapshot_rebuilt: bool,
}

pub async fn run_background_job_once(
    store: &SeasonSyncStore,
    request: &FullSeasonRequest,
    season_output_dir: &Path,
) -> Result<BackgroundRunSummary> {
    let Some(job) = store.claim_next(request.season)? else {
        return Ok(BackgroundRunSummary {
            season: request.season,
            ..Default::default()
        });
    };
    match run_claimed_job(store, request, season_output_dir, &job).await {
        Ok(summary) => Ok(summary),
        Err(error) => {
            mark_job_retryable(store, &job.job_id, "background")?;
            Err(anyhow!(crate::redaction::safe_error_message(error)))
        }
    }
}

async fn run_claimed_job(
    store: &SeasonSyncStore,
    request: &FullSeasonRequest,
    season_output_dir: &Path,
    job: &ClaimedSeasonJob,
) -> Result<BackgroundRunSummary> {
    let client = DashenClient::new(
        request.credentials.clone(),
        request.customer_base_url.clone(),
    )?;
    let mut summary = BackgroundRunSummary {
        season: job.season,
        ..Default::default()
    };

    for mode in &job.modes {
        let request_season = match mode.request_shape {
            RequestShape::ImplicitCurrentSeason => None,
            RequestShape::ExplicitSeason => Some(job.season),
        };
        for page in mode.next_page..=mode.max_page {
            let payload = if mode.match_kind == "fight" {
                client
                    .fight_query_match_list(
                        &request.customer_token,
                        &mode.game_mode,
                        page,
                        request_season,
                    )
                    .await?
            } else {
                client
                    .query_match_list(
                        &request.customer_token,
                        &mode.game_mode,
                        page,
                        request_season,
                    )
                    .await?
            };
            let committed =
                super::quick_list::commit_background_page(store, job, mode, page, payload)?;
            summary.pages_scanned += 1;
            summary.new_list_matches += committed.new_matches;
            if committed.boundary_reached {
                break;
            }
        }
    }

    set_job_phase(store, &job.job_id, job.season, "fetchDetails")?;
    let candidates = pending_detail_entries(store, &job.job_id)?;
    let attempted = candidates
        .iter()
        .map(|(match_id, _, _)| match_id.clone())
        .collect::<HashSet<_>>();
    let results = stream::iter(candidates.into_iter().map(|(match_id, match_kind, entry)| {
        let client = &client;
        let token = &request.customer_token;
        async move {
            let result = if match_kind == "fight" {
                client.fight_query_match_info(token, &match_id).await
            } else {
                client.query_match_info(token, &match_id).await
            };
            match result {
                Ok(payload) => Ok(json!({
                    "match_id": match_id,
                    "match_kind": match_kind,
                    "source_match": entry,
                    "payload": payload,
                })),
                Err(_) => Err(json!({
                    "match_id": match_id,
                    "error_type": "DetailFetchError",
                    "message": "详情请求失败",
                })),
            }
        }
    }))
    .buffer_unordered(2)
    .collect::<Vec<_>>()
    .await;
    let mut details = Vec::new();
    let mut failures = Vec::new();
    for result in results {
        match result {
            Ok(detail) => details.push(detail),
            Err(error) => failures.push(error),
        }
    }
    if !attempted.is_empty() {
        super::persist::sync_import_incremental_parts(
            store.database(),
            job.season,
            &json!({}),
            vec![],
            details.clone(),
            failures.clone(),
            attempted,
        )?;
    }
    summary.details_written = details.len();
    summary.failed_details = failures.len();
    update_detail_job_rows(store, &job.job_id, &details, &failures)?;

    set_job_phase(store, &job.job_id, job.season, "rebuildSnapshot")?;
    let target_revision = current_season_revision(store, job.season)?;
    let base = json!({"season": job.season, "matches": {}});
    let payload =
        super::snapshot::rebuild_complete_season_payload(store.database(), job.season, &base)?;
    if current_season_revision(store, job.season)? != target_revision {
        mark_job_queued(store, &job.job_id, target_revision)?;
        return Ok(summary);
    }
    super::persist::write_season_payload(season_output_dir, job.season, &payload)?;
    if failures.is_empty() {
        complete_job(store, &job.job_id, job.season, target_revision)?;
    } else {
        finish_snapshot_with_retryable_details(store, &job.job_id, job.season, target_revision)?;
    }
    summary.snapshot_rebuilt = true;
    Ok(summary)
}

fn pending_detail_entries(
    store: &SeasonSyncStore,
    job_id: &str,
) -> Result<Vec<(String, String, Value)>> {
    let conn = store.database().connect()?;
    let mut statement = conn.prepare(
        r#"
        SELECT m.match_id, m.match_kind, m.raw_entry_json
        FROM season_sync_job_matches j
        JOIN matches m ON m.match_id=j.match_id
        LEFT JOIN match_details d ON d.match_id=m.match_id
        WHERE j.job_id=?1
          AND j.detail_status IN ('queued','retryable')
          AND d.match_id IS NULL
        ORDER BY m.begin_ts DESC
        "#,
    )?;
    let rows = statement.query_map([job_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut result = Vec::new();
    for row in rows {
        let (match_id, match_kind, raw) = row?;
        result.push((match_id, match_kind, serde_json::from_str(&raw)?));
    }
    Ok(result)
}

fn update_detail_job_rows(
    store: &SeasonSyncStore,
    job_id: &str,
    details: &[Value],
    failures: &[Value],
) -> Result<()> {
    let mut conn = store.database().connect()?;
    let tx = conn.transaction()?;
    let now = Utc::now().to_rfc3339();
    for detail in details {
        if let Some(match_id) = detail.get("match_id").and_then(Value::as_str) {
            tx.execute(
                "UPDATE season_sync_job_matches SET detail_status='completed', error_class=NULL, updated_at=?3 WHERE job_id=?1 AND match_id=?2",
                params![job_id, match_id, now],
            )?;
        }
    }
    for failure in failures {
        if let Some(match_id) = failure.get("match_id").and_then(Value::as_str) {
            tx.execute(
                "UPDATE season_sync_job_matches SET detail_status='retryable', retry_count=retry_count+1, error_class='network', updated_at=?3 WHERE job_id=?1 AND match_id=?2",
                params![job_id, match_id, now],
            )?;
        }
    }
    let pending: i64 = tx.query_row(
        "SELECT COUNT(*) FROM season_sync_job_matches WHERE job_id=?1 AND detail_status IN ('queued','retryable')",
        [job_id],
        |row| row.get(0),
    )?;
    let season: i64 = tx.query_row(
        "SELECT season FROM season_sync_jobs WHERE job_id=?1",
        [job_id],
        |row| row.get(0),
    )?;
    tx.execute(
        "UPDATE season_sync_state SET pending_detail_count=?2, updated_at=?3 WHERE season=?1",
        params![season, pending, now],
    )?;
    tx.commit()?;
    Ok(())
}

fn current_season_revision(store: &SeasonSyncStore, season: i64) -> Result<i64> {
    Ok(store.database().connect()?.query_row(
        "SELECT season_revision FROM season_sync_state WHERE season=?1",
        [season],
        |row| row.get(0),
    )?)
}

fn set_job_phase(store: &SeasonSyncStore, job_id: &str, season: i64, phase: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let conn = store.database().connect()?;
    conn.execute(
        "UPDATE season_sync_jobs SET phase=?2, updated_at=?3 WHERE job_id=?1",
        params![job_id, phase, now],
    )?;
    conn.execute(
        "UPDATE season_sync_state SET background_status=?2, updated_at=?3 WHERE season=?1",
        params![season, phase, now],
    )?;
    Ok(())
}

fn complete_job(
    store: &SeasonSyncStore,
    job_id: &str,
    season: i64,
    snapshot_revision: i64,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let mut conn = store.database().connect()?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE season_sync_jobs SET status='completed', phase='completed', completed_at=?2, updated_at=?2 WHERE job_id=?1",
        params![job_id, now],
    )?;
    tx.execute(
        r#"
        UPDATE season_sync_state
        SET snapshot_revision=?2, background_status='completed',
            pending_detail_count=0, updated_at=?3
        WHERE season=?1
        "#,
        params![season, snapshot_revision, now],
    )?;
    tx.commit()?;
    Ok(())
}

fn finish_snapshot_with_retryable_details(
    store: &SeasonSyncStore,
    job_id: &str,
    season: i64,
    snapshot_revision: i64,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let mut conn = store.database().connect()?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE season_sync_jobs SET status='retryable', phase='fetchDetails', retry_count=retry_count+1, error_class='network', updated_at=?2 WHERE job_id=?1",
        params![job_id, now],
    )?;
    tx.execute(
        r#"
        UPDATE season_sync_state
        SET snapshot_revision=?2, background_status='failed',
            pending_detail_count=(
                SELECT COUNT(*) FROM season_sync_job_matches
                WHERE job_id=?3 AND detail_status IN ('queued','retryable')
            ), updated_at=?4
        WHERE season=?1
        "#,
        params![season, snapshot_revision, job_id, now],
    )?;
    tx.commit()?;
    Ok(())
}

fn mark_job_retryable(store: &SeasonSyncStore, job_id: &str, error_class: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let conn = store.database().connect()?;
    let season = conn
        .query_row(
            "SELECT season FROM season_sync_jobs WHERE job_id=?1",
            [job_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    conn.execute(
        "UPDATE season_sync_jobs SET status='retryable', retry_count=retry_count+1, error_class=?2, updated_at=?3 WHERE job_id=?1",
        params![job_id, error_class, now],
    )?;
    if let Some(season) = season {
        conn.execute(
            "UPDATE season_sync_state SET background_status='failed', updated_at=?2 WHERE season=?1",
            params![season, now],
        )?;
    }
    Ok(())
}

fn mark_job_queued(store: &SeasonSyncStore, job_id: &str, revision: i64) -> Result<()> {
    store.database().connect()?.execute(
        "UPDATE season_sync_jobs SET status='queued', target_revision=?2, phase='rebuildSnapshot', updated_at=?3 WHERE job_id=?1",
        params![job_id, revision, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}
