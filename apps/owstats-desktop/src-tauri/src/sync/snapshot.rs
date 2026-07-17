use super::*;
use chrono::Utc;
use rusqlite::params;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotOutcome {
    Unchanged,
    Replaced,
    StaleRevision,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotBenchmark {
    pub season: i64,
    pub entries: usize,
    pub details: usize,
    pub errors: usize,
    pub assemble_ms: u64,
    pub serialize_ms: u64,
    pub write_and_sync_ms: u64,
    pub total_ms: u64,
    pub output_bytes: u64,
}

pub fn benchmark_snapshot(
    db: &OwDatabase,
    season: i64,
    output: &Path,
) -> Result<SnapshotBenchmark> {
    db.initialize()?;
    let total_started = Instant::now();

    let assemble_started = Instant::now();
    let payload = rebuild_complete_season_payload(
        db,
        season,
        &json!({
            "season": season,
            "matches": {},
        }),
    )?;
    let assemble_ms = assemble_started.elapsed().as_millis() as u64;
    let entries = payload
        .pointer("/matches/entries")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let details = payload
        .pointer("/matches/details")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let errors = payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);

    let serialize_started = Instant::now();
    let bytes = serde_json::to_vec_pretty(&payload)?;
    let serialize_ms = serialize_started.elapsed().as_millis() as u64;

    let write_and_sync_started = Instant::now();
    let parent = output
        .parent()
        .ok_or_else(|| anyhow!("benchmark snapshot destination has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(output).map_err(|error| error.error)?;
    let write_and_sync_ms = write_and_sync_started.elapsed().as_millis() as u64;

    Ok(SnapshotBenchmark {
        season,
        entries,
        details,
        errors,
        assemble_ms,
        serialize_ms,
        write_and_sync_ms,
        total_ms: total_started.elapsed().as_millis() as u64,
        output_bytes: bytes.len() as u64,
    })
}

pub fn rebuild_snapshot_for_revision(
    db: &OwDatabase,
    season: i64,
    target_revision: i64,
    destination: &Path,
) -> Result<SnapshotOutcome> {
    rebuild_snapshot_for_revision_with_hook(db, season, target_revision, destination, || Ok(()))
}

#[doc(hidden)]
pub fn rebuild_snapshot_for_revision_with_hook<F>(
    db: &OwDatabase,
    season: i64,
    target_revision: i64,
    destination: &Path,
    before_revision_check: F,
) -> Result<SnapshotOutcome>
where
    F: FnOnce() -> Result<()>,
{
    db.initialize()?;
    let revisions = db.connect()?.query_row(
        "SELECT season_revision, snapshot_revision FROM season_sync_state WHERE season=?1",
        [season],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    if revisions.1 >= target_revision {
        return Ok(SnapshotOutcome::Unchanged);
    }
    if revisions.0 != target_revision {
        return Ok(SnapshotOutcome::StaleRevision);
    }

    let payload = rebuild_complete_season_payload(
        db,
        season,
        &json!({
            "season": season,
            "matches": {},
        }),
    )?;
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("season snapshot destination has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let sanitized = sanitize_snapshot_value(&payload);
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), &sanitized)?;
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;

    before_revision_check()?;

    // BEGIN IMMEDIATE prevents list/detail writers from changing the revision
    // between the final check, filesystem replacement, and revision commit.
    let mut conn = db.connect()?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current = tx.query_row(
        "SELECT season_revision, snapshot_revision FROM season_sync_state WHERE season=?1",
        [season],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    if current.1 >= target_revision {
        tx.rollback()?;
        return Ok(SnapshotOutcome::Unchanged);
    }
    if current.0 != target_revision {
        tx.rollback()?;
        return Ok(SnapshotOutcome::StaleRevision);
    }

    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    let updated = tx.execute(
        r#"
        UPDATE season_sync_state
        SET snapshot_revision=?2, updated_at=?3
        WHERE season=?1 AND season_revision=?2 AND snapshot_revision < ?2
        "#,
        params![season, target_revision, Utc::now().to_rfc3339()],
    )?;
    if updated != 1 {
        return Err(anyhow!("season snapshot revision changed before commit"));
    }
    tx.commit()?;
    Ok(SnapshotOutcome::Replaced)
}

pub fn rebuild_complete_season_payload(
    db: &OwDatabase,
    season: i64,
    current_fetch_payload: &Value,
) -> Result<Value> {
    db.initialize()?;
    let conn = db.connect()?;
    let transaction = conn.unchecked_transaction()?;

    let entries = parse_rows(
        &transaction,
        "SELECT raw_entry_json
         FROM matches
         WHERE season = ?1
         ORDER BY begin_ts DESC, match_id ASC",
        season,
        "match entry",
    )?;
    let details = parse_rows(
        &transaction,
        "SELECT d.raw_detail_json
         FROM match_details d
         JOIN matches m ON m.match_id = d.match_id
         WHERE m.season = ?1
         ORDER BY m.begin_ts DESC, m.match_id ASC",
        season,
        "match detail",
    )?;
    let detail_errors = parse_rows(
        &transaction,
        "SELECT raw_json
         FROM sync_errors
         WHERE season = ?1
         ORDER BY id ASC",
        season,
        "sync error",
    )?;
    transaction.rollback()?;

    let mut rebuilt = current_fetch_payload.clone();
    let matches = rebuilt
        .as_object_mut()
        .ok_or_else(|| anyhow!("current fetch payload must be a JSON object"))?
        .entry("matches")
        .or_insert_with(|| json!({}));
    let matches = matches
        .as_object_mut()
        .ok_or_else(|| anyhow!("current fetch payload matches field must be a JSON object"))?;
    matches.insert("total".to_string(), json!(entries.len()));
    matches.insert("entries".to_string(), Value::Array(entries));
    matches.insert("details".to_string(), Value::Array(details));
    matches.insert("detail_errors".to_string(), Value::Array(detail_errors));

    Ok(sanitize_snapshot_value(&rebuilt))
}

fn sanitize_snapshot_value(value: &Value) -> Value {
    redact_snapshot_strings(&crate::redaction::sanitize_persisted_json(value))
}

fn redact_snapshot_strings(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, child)| (key.clone(), redact_snapshot_strings(child)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact_snapshot_strings).collect()),
        Value::String(text) => Value::String(crate::redaction::redact_sensitive_text(text)),
        _ => value.clone(),
    }
}

fn parse_rows(
    conn: &rusqlite::Connection,
    sql: &str,
    season: i64,
    row_kind: &str,
) -> Result<Vec<Value>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement
        .query_map(params![season], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    rows.into_iter()
        .enumerate()
        .map(|(index, raw)| {
            serde_json::from_str(&raw).map_err(|_| {
                anyhow!(
                    "failed to parse {row_kind} JSON for season {season} at row {}",
                    index + 1
                )
            })
        })
        .collect()
}
