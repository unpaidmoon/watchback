use super::*;

pub(super) fn build_full_season_payload(
    season: i64,
    cached_player: &Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    list_pages: Vec<Value>,
) -> Value {
    let safe_detail_errors = sanitize_detail_errors(detail_errors);
    json!({
        "input_bnet": strip_sensitive_fields(cached_player.get("input_bnet").cloned().unwrap_or(Value::Null)),
        "resolved_bnet": strip_sensitive_fields(cached_player.get("resolved_bnet").cloned().unwrap_or(Value::Null)),
        "profile_card": strip_sensitive_fields(cached_player.get("profile_card").cloned().unwrap_or(Value::Null)),
        "fetch_mode": "rust_sync",
        "season": season,
        "matches": {
            "total": entries.len(),
            "entries": strip_sensitive_fields(Value::Array(entries)),
            "details": strip_sensitive_fields(Value::Array(details)),
            "detail_errors": strip_sensitive_fields(Value::Array(safe_detail_errors)),
            "list_pages": strip_sensitive_fields(Value::Array(list_pages)),
        }
    })
}

pub(super) fn strip_sensitive_fields(value: Value) -> Value {
    crate::redaction::sanitize_persisted_json(&value)
}

pub(super) fn sanitize_detail_errors(detail_errors: Vec<Value>) -> Vec<Value> {
    detail_errors
        .into_iter()
        .map(|error| {
            json!({
                "match_id": string_field(&error, "match_id"),
                "error_type": string_field(&error, "error_type").unwrap_or_else(|| "DetailFetchError".to_string()),
                "message": safe_error_message(
                    &string_field(&error, "message")
                        .or_else(|| string_field(&error, "error"))
                        .unwrap_or_else(|| "detail fetch failed".to_string())
                ),
            })
        })
        .collect()
}

pub(super) fn extract_match_entries(payload: &Value) -> Vec<Value> {
    for container in [
        Some(payload),
        payload.get("data"),
        payload.get("result"),
        payload.pointer("/data/result"),
        payload.pointer("/result/data"),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(entries) = extract_match_entries_from_container(container) {
            return entries;
        }
    }
    Vec::new()
}

pub(super) fn extract_match_entries_from_container(container: &Value) -> Option<Vec<Value>> {
    if let Some(items) = container.as_array() {
        return match_objects(items);
    }
    for key in [
        "matchList",
        "recentMatchList",
        "list",
        "records",
        "rows",
        "items",
    ] {
        if let Some(items) = container.get(key).and_then(Value::as_array) {
            if let Some(entries) = match_objects(items) {
                return Some(entries);
            }
        }
    }
    None
}

pub(super) fn match_objects(items: &[Value]) -> Option<Vec<Value>> {
    let entries = items
        .iter()
        .filter(|item| item.is_object() && item.get("matchId").is_some())
        .cloned()
        .collect::<Vec<_>>();
    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

pub(super) fn dedupe_entries(entries: Vec<Value>, season: i64) -> Vec<Value> {
    let mut seen = HashSet::<String>::new();
    let mut deduped = Vec::new();
    for mut entry in entries {
        let match_id = string_field(&entry, "matchId").unwrap_or_default();
        if !match_id.is_empty() && !seen.insert(match_id) {
            continue;
        }
        entry["_dashenSeason"] = Value::Number(season.into());
        deduped.push(entry);
    }
    deduped.sort_by_key(|item| item.get("beginTs").and_then(Value::as_i64).unwrap_or(0));
    deduped.reverse();
    deduped
}

pub(super) fn safe_error_message(message: &str) -> String {
    crate::redaction::redact_sensitive_text(message)
        .chars()
        .take(240)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_errors_do_not_include_request_urls_or_tokens() {
        let message = safe_status_error("match list", StatusCode::TOO_MANY_REQUESTS);

        assert_eq!(message, "match list request returned HTTP 429");
        assert!(!message.contains("http"));
        assert!(!message.contains("token"));
    }

    #[test]
    fn safe_error_message_redacts_token_cookie_fragments() {
        let message = safe_error_message(
            "failed token=token-secret customerToken=customer-secret cookie=session-secret visible",
        );

        assert!(!message.contains("token-secret"));
        assert!(!message.contains("customer-secret"));
        assert!(!message.contains("session-secret"));
        assert!(message.contains("visible"));
    }

    #[test]
    fn extracts_match_entries_from_result_match_list_wrapper() {
        let payload = json!({
            "code": 0,
            "result": {
                "matchList": [
                    {"matchId": "wrapped-result-match", "beginTs": 1700000000000_i64}
                ]
            }
        });

        let entries = extract_match_entries(&payload);

        assert_eq!(entries.len(), 1);
        assert_eq!(
            string_field(&entries[0], "matchId").as_deref(),
            Some("wrapped-result-match")
        );
    }

    #[test]
    fn normal_fetch_modes_include_precise_upstream_game_modes() {
        for mode in ["SportPreset", "SportOpen", "LeisurePreset", "Leisure6v6"] {
            assert!(
                NORMAL_MODES.contains(&mode),
                "missing precise normal gameMode {mode}"
            );
        }
    }
}
