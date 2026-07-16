use super::*;
use rusqlite::params;

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
