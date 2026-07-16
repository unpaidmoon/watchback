use super::*;
use std::collections::HashMap;

const INVALID_DETAIL_MESSAGE: &str = "详情响应无有效数据";

fn generic_detail_error(match_id: &str) -> Value {
    json!({
        "match_id": match_id,
        "error_type": "DetailFetchError",
        "message": INVALID_DETAIL_MESSAGE,
    })
}

fn normalize_incremental_detail_outcomes(
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    attempted_detail_ids: &HashSet<String>,
) -> (Vec<Value>, Vec<Value>) {
    let mut valid_details = Vec::new();
    let mut success_ids = HashSet::new();
    let mut invalid_ids = HashSet::new();
    for detail in details {
        let match_id = string_field(&detail, "match_id");
        if crate::importer::is_valid_match_detail(&detail) {
            if let Some(match_id) = match_id {
                success_ids.insert(match_id);
            }
            valid_details.push(detail);
        } else if let Some(match_id) = match_id.filter(|id| attempted_detail_ids.contains(id)) {
            invalid_ids.insert(match_id);
        }
    }

    let mut errors_by_id = HashMap::new();
    for error in sanitize_detail_errors(detail_errors) {
        let Some(match_id) =
            string_field(&error, "match_id").filter(|id| attempted_detail_ids.contains(id))
        else {
            continue;
        };
        errors_by_id.entry(match_id).or_insert(error);
    }

    let normalized_errors = attempted_detail_ids
        .iter()
        .filter(|match_id| !match_id.trim().is_empty() && !success_ids.contains(*match_id))
        .map(|match_id| {
            if invalid_ids.contains(match_id) {
                generic_detail_error(match_id)
            } else {
                errors_by_id
                    .remove(match_id)
                    .unwrap_or_else(|| generic_detail_error(match_id))
            }
        })
        .collect();
    (valid_details, normalized_errors)
}

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
    let (details, safe_detail_errors) =
        normalize_incremental_detail_outcomes(details, detail_errors, &attempted_detail_ids);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(match_id: &str) -> Value {
        json!({
            "matchId": match_id,
            "gameMode": "SportPreset",
            "matchRet": 1,
            "beginTs": 1
        })
    }

    fn valid_detail(match_id: &str) -> Value {
        json!({
            "match_id": match_id,
            "match_kind": "normal",
            "payload": {"code": 0, "data": {}}
        })
    }

    fn seed_old_error(db: &OwDatabase, match_id: &str) {
        import_payload_value_embedded(
            db,
            &json!({
                "season": 23,
                "matches": {
                    "entries": [entry(match_id)],
                    "details": [],
                    "detail_errors": [{
                        "match_id": match_id,
                        "error_type": "DetailFetchError",
                        "message": "旧错误"
                    }]
                }
            }),
        )
        .unwrap();
    }

    fn persisted_error_state(db: &OwDatabase) -> (i64, String, String, i64) {
        let conn = db.connect().unwrap();
        let (message, raw_json) = conn
            .query_row(
                "SELECT message, raw_json FROM sync_errors WHERE season=23",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let details_count = conn
            .query_row("SELECT COUNT(*) FROM match_details", [], |row| row.get(0))
            .unwrap();
        let errors_count = conn
            .query_row(
                "SELECT errors_count FROM seasons WHERE season=23",
                [],
                |row| row.get(0),
            )
            .unwrap();
        (details_count, message, raw_json, errors_count)
    }

    #[test]
    fn incremental_persist_turns_invalid_detail_into_safe_current_error() {
        let temp = tempfile::tempdir().unwrap();
        let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
        seed_old_error(&db, "invalid");

        let summary = sync_import_incremental_parts(
            &db,
            23,
            &Value::Null,
            vec![],
            vec![json!({
                "match_id": "invalid",
                "match_kind": "normal",
                "payload": {
                    "code": 0,
                    "data": [],
                    "customerToken": "private-token"
                }
            })],
            vec![],
            HashSet::from(["invalid".to_string()]),
        )
        .unwrap();

        let (details_count, message, raw_json, errors_count) = persisted_error_state(&db);
        assert_eq!(details_count, 0);
        assert_eq!(message, "详情响应无有效数据");
        assert!(!raw_json.contains("private-token"));
        assert_eq!(summary.failed_details, 1);
        assert_eq!(summary.latest_errors, vec!["详情响应无有效数据"]);
        assert_eq!(errors_count, 1);
    }

    #[test]
    fn incremental_persist_turns_missing_attempt_result_into_failure() {
        let temp = tempfile::tempdir().unwrap();
        let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
        seed_old_error(&db, "missing");

        let summary = sync_import_incremental_parts(
            &db,
            23,
            &Value::Null,
            vec![],
            vec![],
            vec![],
            HashSet::from(["missing".to_string()]),
        )
        .unwrap();

        let (_, message, _, errors_count) = persisted_error_state(&db);
        assert_eq!(message, "详情响应无有效数据");
        assert_eq!(summary.failed_details, 1);
        assert_eq!(errors_count, 1);
    }

    #[test]
    fn incremental_persist_valid_detail_wins_and_clears_old_error() {
        let temp = tempfile::tempdir().unwrap();
        let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
        seed_old_error(&db, "recovered");

        let summary = sync_import_incremental_parts(
            &db,
            23,
            &Value::Null,
            vec![],
            vec![valid_detail("recovered")],
            vec![json!({
                "match_id": "recovered",
                "error_type": "DetailFetchError",
                "message": "stale failure"
            })],
            HashSet::from(["recovered".to_string()]),
        )
        .unwrap();

        let conn = db.connect().unwrap();
        let details_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM match_details", [], |row| row.get(0))
            .unwrap();
        let errors_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_errors", [], |row| row.get(0))
            .unwrap();
        let season_errors_count: i64 = conn
            .query_row(
                "SELECT errors_count FROM seasons WHERE season=23",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(details_count, 1);
        assert_eq!(errors_count, 0);
        assert_eq!(season_errors_count, 0);
        assert_eq!(summary.failed_details, 0);
        assert!(summary.latest_errors.is_empty());
    }
}
