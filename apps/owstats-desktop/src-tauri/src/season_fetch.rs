use crate::dashen_client::{DashenClient, DashenCredentials, DashenRateLimitConfig};
use anyhow::{anyhow, Result};
use futures_util::{stream, StreamExt};
use serde_json::{json, Value};
use std::collections::HashSet;

const NORMAL_MODES: &[&str] = &["sport", "leisure"];
const FIGHT_MODES: &[&str] = &["QuickFight", "LeisureFight", "SportFight"];
const DASHEN_CURRENT_SEASON: i64 = 23;

#[derive(Debug, Clone)]
pub struct FullSeasonRequest {
    pub credentials: DashenCredentials,
    pub customer_token: String,
    pub input_bnet: String,
    pub resolved_bnet: Option<String>,
    pub season: i64,
    pub customer_base_url: String,
    pub pages_per_batch: i64,
    pub detail_concurrency: usize,
    pub include_fight: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullSeasonFetch {
    pub payload: Value,
    pub entries: usize,
    pub details: usize,
    pub failed_details: usize,
}

pub async fn fetch_full_season_with_base_url(
    request: FullSeasonRequest,
) -> Result<FullSeasonFetch> {
    fetch_full_season_with_rate_limit_base_url(request, DashenRateLimitConfig::default()).await
}

pub async fn fetch_full_season_with_rate_limit_base_url(
    request: FullSeasonRequest,
    rate_limit: DashenRateLimitConfig,
) -> Result<FullSeasonFetch> {
    if request.season <= 0 {
        return Err(anyhow!("season must be greater than 0"));
    }
    if request.customer_token.trim().is_empty() {
        return Err(anyhow!("请先绑定目标玩家"));
    }

    let client = DashenClient::new_with_rate_limit(
        request.credentials.clone(),
        request.customer_base_url.clone(),
        rate_limit,
    )?;
    let season_candidates = request_season_candidates(request.season);
    let (profile_card, sport_count, leisure_count) = tokio::join!(
        async {
            client
                .query_card(&request.customer_token)
                .await
                .unwrap_or(Value::Null)
        },
        fetch_count_info_for_candidates(
            &client,
            &request.customer_token,
            "sport",
            &season_candidates,
        ),
        fetch_count_info_for_candidates(
            &client,
            &request.customer_token,
            "leisure",
            &season_candidates,
        )
    );

    let max_pages = request.pages_per_batch.max(1);
    let mut entries = Vec::<Value>::new();
    let mut list_pages = Vec::<Value>::new();
    for mode in NORMAL_MODES {
        let fetched = fetch_history_for_mode(
            &client,
            &request.customer_token,
            mode,
            &season_candidates,
            max_pages,
            false,
        )
        .await?;
        entries.extend(fetched.entries);
        list_pages.extend(fetched.list_pages);
    }
    if request.include_fight {
        for mode in FIGHT_MODES {
            let fetched = fetch_history_for_mode(
                &client,
                &request.customer_token,
                mode,
                &season_candidates,
                max_pages,
                true,
            )
            .await?;
            entries.extend(fetched.entries);
            list_pages.extend(fetched.list_pages);
        }
    }

    let entries = dedupe_entries(entries, request.season);
    let (details, detail_errors) = fetch_details(
        &client,
        &request.customer_token,
        &entries,
        request.detail_concurrency,
    )
    .await;
    let payload = build_payload(
        &request,
        profile_card,
        sport_count,
        leisure_count,
        entries,
        details,
        detail_errors,
        list_pages,
    );
    let entries_count = payload["matches"]["entries"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let details_count = payload["matches"]["details"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let failed_details = payload["matches"]["detail_errors"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);

    Ok(FullSeasonFetch {
        payload,
        entries: entries_count,
        details: details_count,
        failed_details,
    })
}

struct ModeFetchPages {
    entries: Vec<Value>,
    list_pages: Vec<Value>,
}

async fn fetch_history_for_mode(
    client: &DashenClient,
    customer_token: &str,
    mode: &str,
    season_candidates: &[Option<i64>],
    pages_per_batch: i64,
    fight: bool,
) -> Result<ModeFetchPages> {
    let mut entries = Vec::<Value>::new();
    let mut list_pages = Vec::<Value>::new();
    let batch_size = pages_per_batch.max(1);
    for request_season in season_candidates {
        let mut candidate_entries = Vec::<Value>::new();
        let mut candidate_pages = Vec::<Value>::new();
        let mut page_start = 1_i64;
        loop {
            let page_end = page_start + batch_size;
            let pages = (page_start..page_end).collect::<Vec<_>>();
            let results = stream::iter(pages.into_iter().map(|page| async move {
                let payload = if fight {
                    client
                        .fight_query_match_list(customer_token, mode, page, *request_season)
                        .await
                } else {
                    client
                        .query_match_list(customer_token, mode, page, *request_season)
                        .await
                }?;
                Ok::<_, anyhow::Error>((page, payload))
            }))
            .buffer_unordered(batch_size as usize)
            .collect::<Vec<_>>()
            .await;

            let mut batch_has_entries = false;
            for result in results {
                let (page, payload) = result?;
                candidate_pages.push(json!({
                    "kind": if fight { "fight" } else { "normal" },
                    "gameMode": mode,
                    "requestSeason": request_season,
                    "page": page,
                    "payload": payload.clone(),
                }));
                let mut page_entries = extract_match_entries(&payload);
                if page_entries.is_empty() {
                    continue;
                }
                batch_has_entries = true;
                for item in &mut page_entries {
                    item["gameMode"] = Value::String(mode.to_string());
                    if fight {
                        item["match_kind"] = Value::String("fight".to_string());
                    }
                }
                candidate_entries.extend(page_entries);
            }

            if !batch_has_entries {
                break;
            }
            page_start = page_end;
        }
        list_pages.extend(candidate_pages);
        if !candidate_entries.is_empty() {
            entries.extend(candidate_entries);
            break;
        }
    }
    Ok(ModeFetchPages {
        entries,
        list_pages,
    })
}

async fn fetch_count_info_for_candidates(
    client: &DashenClient,
    customer_token: &str,
    game_mode: &str,
    season_candidates: &[Option<i64>],
) -> Value {
    let mut last = Value::Null;
    for request_season in season_candidates {
        let payload = client
            .query_count_info(customer_token, game_mode, *request_season)
            .await
            .unwrap_or(Value::Null);
        if payload_has_meaningful_data(&payload) {
            return payload;
        }
        last = payload;
    }
    last
}

fn request_season_candidates(season: i64) -> Vec<Option<i64>> {
    if season == DASHEN_CURRENT_SEASON {
        vec![None, Some(season)]
    } else {
        vec![Some(season)]
    }
}

fn payload_has_meaningful_data(payload: &Value) -> bool {
    let Some(data) = payload.get("data") else {
        return false;
    };
    value_has_meaningful_data(data)
}

fn value_has_meaningful_data(value: &Value) -> bool {
    match value {
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => map.values().any(value_has_meaningful_data),
        Value::Number(number) => number.as_i64().unwrap_or(0) != 0,
        Value::String(text) => !text.trim().is_empty(),
        Value::Bool(value) => *value,
        Value::Null => false,
    }
}

async fn fetch_details(
    client: &DashenClient,
    customer_token: &str,
    entries: &[Value],
    detail_concurrency: usize,
) -> (Vec<Value>, Vec<Value>) {
    let concurrency = detail_concurrency.max(1);
    let results = stream::iter(entries.iter().cloned().map(|entry| async move {
        let match_id = string_field(&entry, "matchId").unwrap_or_default();
        if match_id.is_empty() {
            return None;
        }
        let fight = is_fight_match(&entry);
        let result = if fight {
            client
                .fight_query_match_info(customer_token, &match_id)
                .await
        } else {
            client.query_match_info(customer_token, &match_id).await
        };
        Some(match result {
            Ok(payload) => Ok(json!({
                "match_id": match_id,
                "match_kind": if fight { "fight" } else { "normal" },
                "source_match": &entry,
                "payload": payload,
            })),
            Err(error) => Err(json!({
                "match_id": match_id,
                "error_type": "DetailFetchError",
                "message": safe_error_message(&error.to_string()),
            })),
        })
    }))
    .buffer_unordered(concurrency)
    .collect::<Vec<_>>()
    .await;

    let mut details = Vec::<Value>::new();
    let mut detail_errors = Vec::<Value>::new();
    for result in results.into_iter().flatten() {
        match result {
            Ok(detail) => details.push(detail),
            Err(error) => detail_errors.push(error),
        }
    }
    (details, detail_errors)
}

fn build_payload(
    request: &FullSeasonRequest,
    profile_card: Value,
    sport_count: Value,
    leisure_count: Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    list_pages: Vec<Value>,
) -> Value {
    json!({
        "input_bnet": request.input_bnet,
        "resolved_bnet": request.resolved_bnet,
        "customer_token": request.customer_token,
        "profile_card": profile_card,
        "count_info": {
            "sport": sport_count,
            "leisure": leisure_count,
        },
        "fetch_mode": "rust_sync",
        "season": request.season,
        "matches": {
            "total": entries.len(),
            "entries": entries,
            "details": details,
            "detail_errors": detail_errors,
            "list_pages": list_pages,
        }
    })
}

fn extract_match_entries(payload: &Value) -> Vec<Value> {
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

fn extract_match_entries_from_container(container: &Value) -> Option<Vec<Value>> {
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

fn match_objects(items: &[Value]) -> Option<Vec<Value>> {
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

fn dedupe_entries(entries: Vec<Value>, season: i64) -> Vec<Value> {
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

fn is_fight_match(entry: &Value) -> bool {
    string_field(entry, "match_kind")
        .map(|kind| kind.eq_ignore_ascii_case("fight"))
        .unwrap_or(false)
        || string_field(entry, "gameMode")
            .map(|mode| mode.to_ascii_lowercase().contains("fight"))
            .unwrap_or(false)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn safe_error_message(message: &str) -> String {
    let mut parts = Vec::new();
    for part in message.split_whitespace() {
        let lower = part.to_ascii_lowercase();
        if lower.contains("token") || lower.contains("cookie") {
            parts.push("[REDACTED]".to_string());
        } else {
            parts.push(part.to_string());
        }
    }
    parts.join(" ").chars().take(240).collect()
}
