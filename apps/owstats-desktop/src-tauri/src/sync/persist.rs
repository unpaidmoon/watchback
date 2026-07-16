use super::*;

pub(super) fn write_season_payload(
    output_dir: &Path,
    season: i64,
    payload: &Value,
) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;
    let path = output_dir.join(format!("local_s{season}_full_season.json"));
    let sanitized = crate::redaction::sanitize_persisted_json(payload);
    let text = serde_json::to_string_pretty(&sanitized)?;
    std::fs::write(&path, text)
        .with_context(|| format!("failed to write season payload {}", path.display()))?;
    Ok(path)
}

#[allow(clippy::too_many_arguments)] // Sync facade passes the independently fetched payload parts explicitly.
pub fn sync_import_parts(
    db: &OwDatabase,
    _project_root: &Path,
    season: i64,
    cached_player: &Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    skipped_duplicates: usize,
) -> Result<SyncSummary> {
    db.initialize()?;
    let total = entries.len();
    let safe_detail_errors = sanitize_detail_errors(detail_errors);
    let payload = json!({
        "input_bnet": cached_player.get("input_bnet").cloned().unwrap_or(Value::Null),
        "resolved_bnet": cached_player.get("resolved_bnet").cloned().unwrap_or(Value::Null),
        "profile_card": cached_player.get("profile_card").cloned().unwrap_or(Value::Null),
        "fetch_mode": "rust_sync",
        "season": season,
        "matches": {
            "total": entries.len(),
            "entries": entries,
            "details": details,
            "detail_errors": safe_detail_errors,
        }
    });
    let import_summary = import_payload_value_embedded(db, &payload)?;
    let errors = payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let latest_errors = errors
        .iter()
        .rev()
        .take(5)
        .filter_map(|item| string_field(item, "message"))
        .collect::<Vec<_>>();
    Ok(SyncSummary {
        season,
        phase: "completed".to_string(),
        processed: total,
        total,
        current_match_id: None,
        imported_matches: import_summary.matches_seen,
        imported_details: import_summary.details_seen,
        skipped_duplicates,
        failed_details: errors.len(),
        latest_errors,
        import_summary,
    })
}

#[allow(clippy::too_many_arguments)] // Incremental sync passes independently fetched payload parts and attempt scope.
#[allow(dead_code)] // Wired by the incremental fetch task; kept internal so legacy dump/import cannot select it.
pub(super) fn sync_import_incremental_parts(
    db: &OwDatabase,
    season: i64,
    cached_player: &Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    attempted_detail_ids: HashSet<String>,
) -> Result<SyncSummary> {
    db.initialize()?;
    let total = entries.len();
    let safe_detail_errors = sanitize_detail_errors(detail_errors);
    let payload = json!({
        "input_bnet": cached_player.get("input_bnet").cloned().unwrap_or(Value::Null),
        "resolved_bnet": cached_player.get("resolved_bnet").cloned().unwrap_or(Value::Null),
        "profile_card": cached_player.get("profile_card").cloned().unwrap_or(Value::Null),
        "fetch_mode": "rust_sync",
        "season": season,
        "matches": {
            "total": entries.len(),
            "entries": entries,
            "details": details,
            "detail_errors": safe_detail_errors,
        }
    });
    let import_summary = crate::importer::import_incremental_payload_value_embedded(
        db,
        &payload,
        attempted_detail_ids,
    )?;
    let errors = payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let latest_errors = errors
        .iter()
        .rev()
        .take(5)
        .filter_map(|item| string_field(item, "message"))
        .collect::<Vec<_>>();
    Ok(SyncSummary {
        season,
        phase: "completed".to_string(),
        processed: total,
        total,
        current_match_id: None,
        imported_matches: import_summary.matches_seen,
        imported_details: import_summary.details_seen,
        skipped_duplicates: 0,
        failed_details: errors.len(),
        latest_errors,
        import_summary,
    })
}
