use crate::db::OwDatabase;
use crate::importer::{import_payload_value_embedded, string_field};
use crate::models::{SyncProgress, SyncSummary};
use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DASHEN_CUSTOMER_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool/customer";
const CLIENT_TYPE: &str = "60";
const ORIGIN: &str = "https://act.ds.163.com";
const REFERER: &str = "https://act.ds.163.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36 app/df_client dfVersion/100111";
const NORMAL_MODES: &[&str] = &[
    "SportPreset",
    "SportOpen",
    "LeisurePreset",
    "Leisure6v6",
    "sport",
    "leisure",
];
const FIGHT_MODES: &[&str] = &["QuickFight", "LeisureFight", "SportFight"];
const MAX_PAGES_PER_MODE: i64 = 80;
const MAX_HTTP_ATTEMPTS: usize = 5;
const REMOTE_REQUEST_SPACING_MS: u64 = 350;
const HTTP_TIMEOUT_SECONDS: u64 = 20;

#[derive(Debug, Deserialize)]
pub(crate) struct Credentials {
    #[serde(alias = "roleId")]
    pub(crate) role_id: i64,
    pub(crate) token: String,
    #[serde(default, alias = "customerToken")]
    pub(crate) customer_token: Option<String>,
    pub(crate) dts: Option<i64>,
    pub(crate) server: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonJsonDump {
    pub path: PathBuf,
    pub entries: usize,
    pub details: usize,
    pub failed_details: usize,
    pub skipped_duplicates: usize,
}

struct SeasonFetchParts {
    cached_player: Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    list_pages: Vec<Value>,
    skipped_duplicates: usize,
}

struct ModeFetchPages {
    entries: Vec<Value>,
    list_pages: Vec<Value>,
}

pub async fn sync_season(db: &OwDatabase, project_root: &Path, season: i64) -> Result<SyncSummary> {
    sync_season_with_progress(db, project_root, season, |_| {}).await
}

pub async fn sync_season_with_progress(
    db: &OwDatabase,
    project_root: &Path,
    season: i64,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    let credential_path = default_project_credential_path(project_root);
    sync_season_with_credentials_path_base_url(
        db,
        project_root,
        &credential_path,
        season,
        DASHEN_CUSTOMER_BASE,
        progress,
    )
    .await
}

pub async fn sync_season_with_credentials_path_progress(
    db: &OwDatabase,
    project_root: &Path,
    credential_path: &Path,
    season: i64,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    sync_season_with_credentials_path_base_url(
        db,
        project_root,
        credential_path,
        season,
        DASHEN_CUSTOMER_BASE,
        progress,
    )
    .await
}

pub async fn sync_season_with_base_url(
    db: &OwDatabase,
    project_root: &Path,
    season: i64,
    customer_base_url: &str,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    let credential_path = default_project_credential_path(project_root);
    sync_season_with_credentials_path_base_url(
        db,
        project_root,
        &credential_path,
        season,
        customer_base_url,
        progress,
    )
    .await
}

pub async fn sync_season_with_credentials_path_base_url(
    db: &OwDatabase,
    project_root: &Path,
    credential_path: &Path,
    season: i64,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    db.initialize()?;
    let parts =
        fetch_season_parts_with_base_url(credential_path, season, customer_base_url, &mut progress)
            .await?;
    let total = parts.entries.len();
    let payload = build_full_season_payload(
        season,
        &parts.cached_player,
        parts.entries,
        parts.details,
        parts.detail_errors,
        parts.list_pages,
    );
    write_season_payload(&project_root.join("data").join("players"), season, &payload)?;
    if total == 0 {
        return Err(anyhow!(
            "未拉取到 S{season} 赛季对局；已保存本地 JSON，请确认该赛季有战绩，或稍后重新登录后再试。"
        ));
    }
    let entries = payload
        .pointer("/matches/entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let details = payload
        .pointer("/matches/details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_errors = payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    progress(SyncProgress {
        season,
        phase: "写入 SQLite".to_string(),
        processed: total,
        total,
        current_match_id: None,
        failed_details: detail_errors.len(),
        latest_errors: detail_errors
            .iter()
            .rev()
            .take(5)
            .filter_map(|item| string_field(item, "message"))
            .collect(),
    });
    let summary = sync_import_parts(
        db,
        project_root,
        season,
        &payload,
        entries,
        details,
        detail_errors,
        parts.skipped_duplicates,
    )?;
    progress(SyncProgress {
        season,
        phase: "完成".to_string(),
        processed: summary.processed,
        total: summary.total,
        current_match_id: None,
        failed_details: summary.failed_details,
        latest_errors: summary.latest_errors.clone(),
    });
    Ok(summary)
}

pub async fn sync_season_with_credentials_and_target_paths_base_url(
    db: &OwDatabase,
    project_root: &Path,
    credential_path: &Path,
    target_path: &Path,
    season: i64,
    customer_base_url: &str,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    sync_season_with_credentials_and_target_paths_output_dir_base_url(
        db,
        &project_root.join("data").join("players"),
        credential_path,
        target_path,
        season,
        customer_base_url,
        progress,
    )
    .await
}

pub async fn sync_season_with_credentials_and_target_paths_output_dir_base_url(
    db: &OwDatabase,
    season_output_dir: &Path,
    credential_path: &Path,
    target_path: &Path,
    season: i64,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    db.initialize()?;
    let credentials = read_credentials(credential_path)?;
    let target = crate::player_target::read_player_target_for_path(target_path)?;
    progress(SyncProgress {
        season,
        phase: "拉取赛季 JSON".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    let fetch = crate::season_fetch::fetch_full_season_with_base_url(
        crate::season_fetch::FullSeasonRequest {
            credentials: crate::dashen_client::DashenCredentials {
                role_id: credentials.role_id,
                token: credentials.token,
                dts: credentials.dts,
                server: credentials.server.unwrap_or(1),
            },
            customer_token: target.customer_token,
            input_bnet: target.input_bnet,
            resolved_bnet: target.resolved_bnet,
            season,
            customer_base_url: customer_base_url.to_string(),
            pages_per_batch: 5,
            detail_concurrency: 2,
            include_fight: true,
        },
    )
    .await?;

    write_season_payload(season_output_dir, season, &fetch.payload)?;
    if fetch.entries == 0 {
        return Err(anyhow!(
            "未拉取到 S{season} 赛季对局；已保存本地 JSON，请确认该赛季有战绩，或稍后重新登录后再试。"
        ));
    }

    let entries = fetch
        .payload
        .pointer("/matches/entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let details = fetch
        .payload
        .pointer("/matches/details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_errors = fetch
        .payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    progress(SyncProgress {
        season,
        phase: "写入 SQLite".to_string(),
        processed: fetch.entries,
        total: fetch.entries,
        current_match_id: None,
        failed_details: detail_errors.len(),
        latest_errors: Vec::new(),
    });
    let summary = sync_import_parts(
        db,
        season_output_dir,
        season,
        &fetch.payload,
        entries,
        details,
        detail_errors,
        0,
    )?;
    progress(SyncProgress {
        season,
        phase: "完成".to_string(),
        processed: summary.processed,
        total: summary.total,
        current_match_id: None,
        failed_details: summary.failed_details,
        latest_errors: summary.latest_errors.clone(),
    });
    Ok(summary)
}

pub async fn dump_season_json(
    project_root: &Path,
    season: i64,
    output_dir: &Path,
    progress: impl FnMut(SyncProgress),
) -> Result<SeasonJsonDump> {
    let credential_path = default_project_credential_path(project_root);
    dump_season_json_with_base_url(
        project_root,
        &credential_path,
        season,
        output_dir,
        DASHEN_CUSTOMER_BASE,
        progress,
    )
    .await
}

pub async fn dump_season_json_with_base_url(
    _project_root: &Path,
    credential_path: &Path,
    season: i64,
    output_dir: &Path,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SeasonJsonDump> {
    let parts =
        fetch_season_parts_with_base_url(credential_path, season, customer_base_url, &mut progress)
            .await?;
    let entries = parts.entries.len();
    let details = parts.details.len();
    let failed_details = parts.detail_errors.len();
    let skipped_duplicates = parts.skipped_duplicates;
    let payload = build_full_season_payload(
        season,
        &parts.cached_player,
        parts.entries,
        parts.details,
        parts.detail_errors,
        parts.list_pages,
    );
    let path = write_season_payload(output_dir, season, &payload)?;
    Ok(SeasonJsonDump {
        path,
        entries,
        details,
        failed_details,
        skipped_duplicates,
    })
}

fn write_season_payload(output_dir: &Path, season: i64, payload: &Value) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;
    let path = output_dir.join(format!("local_s{season}_full_season.json"));
    let text = serde_json::to_string_pretty(payload)?;
    std::fs::write(&path, text)
        .with_context(|| format!("failed to write season payload {}", path.display()))?;
    Ok(path)
}

async fn fetch_season_parts_with_base_url(
    credential_path: &Path,
    season: i64,
    customer_base_url: &str,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<SeasonFetchParts> {
    if season <= 0 {
        return Err(anyhow!("season must be greater than 0"));
    }
    progress(SyncProgress {
        season,
        phase: "读取本地凭证".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    let credentials = read_credentials(credential_path)?;
    let customer_token = credentials
        .customer_token
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("请先绑定目标玩家，再同步赛季数据"))?;
    let cached_player = json!({
        "input_bnet": Value::Null,
        "resolved_bnet": Value::Null,
        "profile_card": Value::Null
    });
    let client = reqwest::Client::builder()
        .default_headers(auth_headers(&credentials)?)
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()?;

    let mut entries = Vec::<Value>::new();
    let mut list_pages = Vec::<Value>::new();
    progress(SyncProgress {
        season,
        phase: "拉取赛季列表".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    for mode in NORMAL_MODES {
        let fetched = fetch_mode_pages(
            &client,
            customer_base_url,
            &customer_token,
            mode,
            season,
            false,
        )
        .await?;
        entries.extend(fetched.entries);
        list_pages.extend(fetched.list_pages);
    }
    for mode in FIGHT_MODES {
        let fetched = fetch_mode_pages(
            &client,
            customer_base_url,
            &customer_token,
            mode,
            season,
            true,
        )
        .await?;
        entries.extend(fetched.entries);
        list_pages.extend(fetched.list_pages);
    }
    let before_dedupe = entries.len();
    entries = dedupe_entries(entries, season);
    let skipped_duplicates = before_dedupe.saturating_sub(entries.len());

    let mut details = Vec::<Value>::new();
    let mut detail_errors = Vec::<Value>::new();
    let total = entries.len();
    progress(SyncProgress {
        season,
        phase: "拉取单局详情".to_string(),
        processed: 0,
        total,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    for entry in &entries {
        let match_id = string_field(entry, "matchId").unwrap_or_default();
        if match_id.is_empty() {
            continue;
        }
        match fetch_detail(&client, customer_base_url, &customer_token, entry).await {
            Ok(detail) => details.push(detail),
            Err(error) => detail_errors.push(json!({
                "match_id": match_id,
                "error_type": "DetailFetchError",
                "message": safe_error_message(&error.to_string()),
            })),
        }
        let latest_errors = detail_errors
            .iter()
            .rev()
            .take(5)
            .filter_map(|item| string_field(item, "message"))
            .collect::<Vec<_>>();
        progress(SyncProgress {
            season,
            phase: "拉取单局详情".to_string(),
            processed: details.len() + detail_errors.len(),
            total,
            current_match_id: Some(match_id),
            failed_details: detail_errors.len(),
            latest_errors,
        });
    }

    Ok(SeasonFetchParts {
        cached_player,
        entries,
        details,
        detail_errors,
        list_pages,
        skipped_duplicates,
    })
}

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

fn build_full_season_payload(
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

fn strip_sensitive_fields(value: Value) -> Value {
    match value {
        Value::Array(items) => {
            Value::Array(items.into_iter().map(strip_sensitive_fields).collect())
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter_map(|(key, value)| {
                    if is_sensitive_field_name(&key) {
                        None
                    } else {
                        Some((key, strip_sensitive_fields(value)))
                    }
                })
                .collect(),
        ),
        other => other,
    }
}

fn is_sensitive_field_name(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("token")
        || key.contains("cookie")
        || key.contains("authorization")
        || key.contains("credential")
}

fn sanitize_detail_errors(detail_errors: Vec<Value>) -> Vec<Value> {
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

fn default_project_credential_path(project_root: &Path) -> PathBuf {
    project_root.join("data").join(".dashen_credentials.json")
}

fn read_credentials(path: &Path) -> Result<Credentials> {
    let text = std::fs::read_to_string(path)
        .with_context(|| "local credentials not found at data/.dashen_credentials.json")?;
    let credentials: Credentials =
        serde_json::from_str(&text).with_context(|| "failed to parse local credentials")?;
    if credentials.role_id <= 0 || credentials.token.trim().is_empty() {
        return Err(anyhow!("local credentials are incomplete"));
    }
    Ok(credentials)
}

pub(crate) fn read_credentials_for_command(path: &Path) -> Result<Credentials> {
    read_credentials(path)
}

fn auth_headers(credentials: &Credentials) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        "GL-Bigdata-Auth-Token",
        HeaderValue::from_str(&credentials.token)?,
    );
    headers.insert(
        "GL-Bigdata-Dts",
        HeaderValue::from_str(&credentials.dts.unwrap_or(2026).to_string())?,
    );
    headers.insert(
        "GL-Bigdata-Role-Id",
        HeaderValue::from_str(&credentials.role_id.to_string())?,
    );
    headers.insert(
        "GL-Bigdata-Server",
        HeaderValue::from_str(&credentials.server.unwrap_or(1).to_string())?,
    );
    headers.insert("GL-ClientType", HeaderValue::from_static(CLIENT_TYPE));
    headers.insert("Origin", HeaderValue::from_static(ORIGIN));
    headers.insert("Referer", HeaderValue::from_static(REFERER));
    headers.insert("User-Agent", HeaderValue::from_static(USER_AGENT));
    Ok(headers)
}

async fn fetch_mode_pages(
    client: &reqwest::Client,
    customer_base_url: &str,
    customer_token: &str,
    mode: &str,
    season: i64,
    fight: bool,
) -> Result<ModeFetchPages> {
    let mut values = Vec::new();
    let mut list_pages = Vec::new();
    for page in 1..=MAX_PAGES_PER_MODE {
        let endpoint = if fight {
            format!("{customer_base_url}/fight/queryMatchList")
        } else {
            format!("{customer_base_url}/queryMatchList")
        };
        let payload = get_json_with_retry(
            client,
            &endpoint,
            &[
                ("token", customer_token.to_string()),
                ("gameMode", mode.to_string()),
                ("page", page.to_string()),
                ("season", season.to_string()),
            ],
            "match list",
        )
        .await?;
        list_pages.push(json!({
            "kind": if fight { "fight" } else { "normal" },
            "gameMode": mode,
            "page": page,
            "payload": payload.clone(),
        }));
        let mut page_entries = extract_match_entries(&payload);
        if page_entries.is_empty() {
            break;
        }
        for item in &mut page_entries {
            if fight {
                item["gameMode"] = Value::String(mode.to_string());
            } else if item.get("gameMode").is_none() {
                item["gameMode"] = Value::String(mode.to_string());
            }
        }
        values.extend(page_entries);
    }
    Ok(ModeFetchPages {
        entries: values,
        list_pages,
    })
}

async fn fetch_detail(
    client: &reqwest::Client,
    customer_base_url: &str,
    customer_token: &str,
    entry: &Value,
) -> Result<Value> {
    let match_id = string_field(entry, "matchId").ok_or_else(|| anyhow!("missing matchId"))?;
    let fight = string_field(entry, "gameMode")
        .map(|mode| mode.to_ascii_lowercase().contains("fight"))
        .unwrap_or(false);
    let endpoint = if fight {
        format!("{customer_base_url}/fight/queryMatchInfo")
    } else {
        format!("{customer_base_url}/queryMatchInfo")
    };
    let payload = get_json_with_retry(
        client,
        &endpoint,
        &[
            ("matchId", match_id.clone()),
            ("token", customer_token.to_string()),
        ],
        "match detail",
    )
    .await?;
    Ok(json!({
        "match_id": match_id,
        "match_kind": if fight { "fight" } else { "normal" },
        "source_match": entry,
        "payload": payload,
    }))
}

async fn get_json_with_retry(
    client: &reqwest::Client,
    endpoint: &str,
    query: &[(&str, String)],
    context: &str,
) -> Result<Value> {
    for attempt in 1..=MAX_HTTP_ATTEMPTS {
        let response = match client.get(endpoint).query(query).send().await {
            Ok(response) => response,
            Err(error) => {
                if attempt == MAX_HTTP_ATTEMPTS {
                    return Err(anyhow!(safe_request_error(context, &error)));
                }
                tokio::time::sleep(retry_delay(attempt)).await;
                continue;
            }
        };
        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS && attempt < MAX_HTTP_ATTEMPTS {
            tokio::time::sleep(retry_delay(attempt)).await;
            continue;
        }
        if !status.is_success() {
            return Err(anyhow!(safe_status_error(context, status)));
        }
        let payload = response
            .json()
            .await
            .with_context(|| format!("{context} response was not valid JSON"))?;
        if should_throttle_remote(endpoint) {
            tokio::time::sleep(std::time::Duration::from_millis(REMOTE_REQUEST_SPACING_MS)).await;
        }
        return Ok(payload);
    }
    Err(anyhow!("{context} request failed"))
}

fn retry_delay(attempt: usize) -> std::time::Duration {
    std::time::Duration::from_secs((attempt as u64) * 5)
}

fn should_throttle_remote(endpoint: &str) -> bool {
    !(endpoint.starts_with("http://127.0.0.1:")
        || endpoint.starts_with("http://localhost:")
        || endpoint.starts_with("https://127.0.0.1:")
        || endpoint.starts_with("https://localhost:"))
}

fn safe_status_error(context: &str, status: StatusCode) -> String {
    format!("{context} request returned HTTP {}", status.as_u16())
}

fn safe_request_error(context: &str, error: &reqwest::Error) -> String {
    if error.is_timeout() {
        format!("{context} request timed out")
    } else if error.is_connect() {
        format!("{context} request could not connect")
    } else if error.is_decode() {
        format!("{context} response could not be decoded")
    } else {
        format!("{context} request failed")
    }
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
