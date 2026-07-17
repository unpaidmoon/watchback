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
                    return Err(original_request_error(context, error));
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
            return Err(non_success_response_error(context, &response));
        }
        let payload = response
            .json()
            .await
            .map_err(|error| original_request_error(context, error))?;
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

fn original_request_error(context: &str, error: reqwest::Error) -> anyhow::Error {
    let message = format!("{context}: {error}");
    anyhow::Error::new(error).context(message)
}

fn non_success_response_error(context: &str, response: &reqwest::Response) -> anyhow::Error {
    match response.error_for_status_ref() {
        Err(error) => original_request_error(context, error),
        Ok(_) => anyhow!("{context}: unexpected HTTP status {}", response.status()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn spawn_response_server(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let body = body.to_string();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer).unwrap();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        format!("http://{address}/synthetic")
    }

    #[tokio::test]
    async fn unexpected_redirect_returns_context_and_status_without_panicking() {
        let endpoint = spawn_response_server("302 Found", "");
        let error = get_json_with_retry(
            &reqwest::Client::new(),
            &endpoint,
            &[],
            "synthetic sync redirect",
        )
        .await
        .unwrap_err()
        .to_string();

        assert!(error.contains("synthetic sync redirect"));
        assert!(error.contains("unexpected HTTP status 302 Found"));
    }

    #[tokio::test]
    async fn malformed_json_error_includes_context_and_original_decode_display() {
        let endpoint = spawn_response_server("200 OK", "not-json");
        let error = get_json_with_retry(
            &reqwest::Client::new(),
            &endpoint,
            &[],
            "synthetic malformed JSON",
        )
        .await
        .unwrap_err()
        .to_string();

        assert!(error.contains("synthetic malformed JSON"));
        assert!(error.contains("error decoding response body"));
    }
}
