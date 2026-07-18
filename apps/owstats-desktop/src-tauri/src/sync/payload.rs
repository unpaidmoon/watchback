use super::*;

pub(super) fn build_full_season_payload(
    season: i64,
    cached_player: &Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    list_pages: Vec<Value>,
) -> Value {
    let detail_errors = normalize_detail_errors(detail_errors);
    json!({
        "input_bnet": cached_player.get("input_bnet").cloned().unwrap_or(Value::Null),
        "resolved_bnet": cached_player.get("resolved_bnet").cloned().unwrap_or(Value::Null),
        "profile_card": cached_player.get("profile_card").cloned().unwrap_or(Value::Null),
        "fetch_mode": "rust_sync",
        "season": season,
        "matches": {
            "total": entries.len(),
            "entries": entries,
            "details": details,
            "detail_errors": detail_errors,
            "list_pages": list_pages,
        }
    })
}

pub(super) fn normalize_detail_errors(detail_errors: Vec<Value>) -> Vec<Value> {
    detail_errors
        .into_iter()
        .map(|mut error| {
            let Some(object) = error.as_object_mut() else {
                return error;
            };
            object
                .entry("error_type".to_string())
                .or_insert_with(|| Value::String("DetailFetchError".to_string()));
            if !object.contains_key("message") {
                if let Some(message) = object.get("error").cloned() {
                    object.insert("message".to_string(), message);
                }
            }
            error
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_detail_errors_without_removing_existing_fields() {
        let errors = normalize_detail_errors(vec![json!({
            "match_id": "synthetic-match",
            "error": "token=synthetic-token",
            "authorization": "synthetic-authorization"
        })]);

        assert_eq!(errors[0]["error_type"], "DetailFetchError");
        assert_eq!(errors[0]["message"], "token=synthetic-token");
        assert_eq!(errors[0]["authorization"], "synthetic-authorization");
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
