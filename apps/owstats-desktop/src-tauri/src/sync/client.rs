use super::*;

pub(super) fn default_project_credential_path(project_root: &Path) -> PathBuf {
    project_root.join("data").join(".dashen_credentials.json")
}

pub(super) fn read_credentials(path: &Path) -> Result<Credentials> {
    let text = std::fs::read_to_string(path)
        .with_context(|| "local credentials not found at data/.dashen_credentials.json")?;
    let credentials: Credentials =
        serde_json::from_str(&text).with_context(|| "failed to parse local credentials")?;
    if credentials.role_id <= 0 || credentials.token.trim().is_empty() {
        return Err(anyhow!("local credentials are incomplete"));
    }
    Ok(credentials)
}

pub(super) fn auth_headers(credentials: &Credentials) -> Result<HeaderMap> {
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

pub(super) async fn fetch_mode_pages(
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
            if fight || item.get("gameMode").is_none() {
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

pub(super) async fn fetch_detail(
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

pub(super) async fn get_json_with_retry(
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

pub(super) fn retry_delay(attempt: usize) -> std::time::Duration {
    std::time::Duration::from_secs((attempt as u64) * 5)
}

pub(super) fn should_throttle_remote(endpoint: &str) -> bool {
    !(endpoint.starts_with("http://127.0.0.1:")
        || endpoint.starts_with("http://localhost:")
        || endpoint.starts_with("https://127.0.0.1:")
        || endpoint.starts_with("https://localhost:"))
}

pub(super) fn safe_status_error(context: &str, status: StatusCode) -> String {
    format!("{context} request returned HTTP {}", status.as_u16())
}

pub(super) fn safe_request_error(context: &str, error: &reqwest::Error) -> String {
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
