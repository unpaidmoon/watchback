use futures_util::future::join_all;
use owstats_desktop_lib::player_target::{
    clear_player_target_for_path, player_target_status_for_path, save_player_target_for_path,
    PlayerTarget,
};
use owstats_desktop_lib::OwDatabase;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn player_target_status_does_not_expose_customer_token() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(".dashen_player_target.json");
    save_player_target_for_path(
        &path,
        &PlayerTarget {
            input_bnet: "Tester#12345".to_string(),
            resolved_bnet: Some("Tester#12345".to_string()),
            customer_token: "secret-customer-token".to_string(),
            bound_at: "2026-07-03T00:00:00Z".to_string(),
        },
    )
    .unwrap();

    let status = player_target_status_for_path(&path).unwrap();
    let status_text = serde_json::to_string(&status).unwrap();

    assert_eq!(status.state, "bound");
    assert_eq!(status.input_bnet.as_deref(), Some("Tester#12345"));
    assert!(!status_text.contains("secret-customer-token"));
    assert!(!status_text.contains("customerToken"));
}

#[test]
fn clearing_player_target_removes_saved_target() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(".dashen_player_target.json");
    save_player_target_for_path(
        &path,
        &PlayerTarget {
            input_bnet: "Tester#12345".to_string(),
            resolved_bnet: None,
            customer_token: "secret-customer-token".to_string(),
            bound_at: "2026-07-03T00:00:00Z".to_string(),
        },
    )
    .unwrap();

    assert!(clear_player_target_for_path(&path).unwrap());
    let status = player_target_status_for_path(&path).unwrap();

    assert_eq!(status.state, "missing");
    assert_eq!(status.input_bnet, None);
}

#[test]
fn extracts_customer_token_from_search_payload() {
    let payload = serde_json::json!({
        "code": 0,
        "data": {
            "customerToken": "player-customer-token",
            "bnetId": "Tester#12345"
        }
    });

    let parsed = owstats_desktop_lib::player_target::target_from_search_payload(
        "Tester#12345",
        &payload,
        "2026-07-03T00:00:00Z",
    )
    .unwrap();

    assert_eq!(parsed.input_bnet, "Tester#12345");
    assert_eq!(parsed.resolved_bnet.as_deref(), Some("Tester#12345"));
    assert_eq!(parsed.customer_token, "player-customer-token");
}

#[tokio::test]
async fn dashen_client_uses_account_token_for_auth_and_customer_token_for_query() {
    use owstats_desktop_lib::dashen_client::{DashenClient, DashenCredentials};

    let server = spawn_recording_server(vec![(
        "/customer/queryMatchList",
        r#"{"code":0,"data":{"matchList":[]}}"#,
    )]);
    let client = DashenClient::new(
        DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: None,
            server: 1,
        },
        server.base_url.clone(),
    )
    .unwrap();

    let _ = client
        .query_match_list("player-customer-token", "sport", 1, Some(23))
        .await
        .unwrap();

    let request = server.requests.lock().unwrap().remove(0);
    assert!(request.first_line.contains("/customer/queryMatchList"));
    assert!(request.first_line.contains("token=player-customer-token"));
    assert!(request.first_line.contains("gameMode=sport"));
    assert!(request.first_line.contains("page=1"));
    assert!(request.first_line.contains("season=23"));
    let headers = request.headers.to_ascii_lowercase();
    assert!(headers.contains("gl-bigdata-auth-token: account-auth-token"));
    assert!(headers.contains("gl-bigdata-role-id: 123"));
    assert!(headers.contains("gl-bigdata-server: 1"));
    assert!(headers.contains("gl-clienttype: 60"));
}

#[tokio::test]
async fn dashen_client_searches_battletag_with_python_equivalent_post_body() {
    use owstats_desktop_lib::dashen_client::{DashenClient, DashenCredentials};

    let server = spawn_recording_server(vec![(
        "/searchBnetAccount",
        r#"{"code":0,"data":{"customerToken":"player-customer-token","bnetId":"Tester#12345"}}"#,
    )]);
    let client = DashenClient::new(
        DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        format!("{}/customer", server.base_url),
    )
    .unwrap();

    let _ = client.search_bnet_account("Tester#12345").await.unwrap();

    let request = server.requests.lock().unwrap().remove(0);
    assert!(request.first_line.starts_with("POST /searchBnetAccount "));
    assert!(request.body.contains(r#""name":"Tester#12345""#));
    assert!(request.body.contains(r#""token":"account-auth-token""#));
    assert!(request.body.contains(r#""roleId":123"#));
    assert!(request.body.contains(r#""dts":2026"#));
    assert!(request.body.contains(r#""server":1"#));
}

#[tokio::test]
async fn dashen_client_limits_datamsapi_domain_concurrency() {
    use owstats_desktop_lib::dashen_client::{
        DashenClient, DashenCredentials, DashenRateLimitConfig,
    };

    let server = spawn_limited_detail_server(200, 200);
    let client = DashenClient::new_with_rate_limit(
        DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        format!("{}/customer", server.base_url),
        DashenRateLimitConfig {
            datamsapi_max_concurrent: 2,
            account_max_requests_per_window: 100,
            account_window_ms: 1_000,
            max_http_attempts: 1,
            retry_base_ms: 10,
        },
    )
    .unwrap();

    let match_ids = (0..4)
        .map(|index| format!("detail-{index}"))
        .collect::<Vec<_>>();
    let tasks = match_ids
        .iter()
        .map(|match_id| client.query_match_info("player-customer-token", match_id));
    let results = join_all(tasks).await;

    assert!(results.iter().all(|result| result.is_ok()));
    assert_eq!(
        *server.max_active_details.lock().unwrap(),
        2,
        "DashenClient should cap simultaneous datamsapi requests like Python SafeClient"
    );
}

#[tokio::test]
async fn dashen_client_applies_account_sliding_window_before_sending() {
    use owstats_desktop_lib::dashen_client::{
        DashenClient, DashenCredentials, DashenRateLimitConfig,
    };

    let server = spawn_limited_detail_server(0, 200);
    let client = DashenClient::new_with_rate_limit(
        DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        format!("{}/customer", server.base_url),
        DashenRateLimitConfig {
            datamsapi_max_concurrent: 10,
            account_max_requests_per_window: 1,
            account_window_ms: 200,
            max_http_attempts: 1,
            retry_base_ms: 10,
        },
    )
    .unwrap();

    let started = Instant::now();
    let results = join_all([
        client.query_match_info("player-customer-token", "detail-a"),
        client.query_match_info("player-customer-token", "detail-b"),
    ])
    .await;

    assert!(results.iter().all(|result| result.is_ok()));
    assert!(
        started.elapsed() >= Duration::from_millis(180),
        "second authenticated request should wait for the account limiter window"
    );
}

#[tokio::test]
async fn dashen_client_retries_http_429_with_backoff() {
    use owstats_desktop_lib::dashen_client::{
        DashenClient, DashenCredentials, DashenRateLimitConfig,
    };

    let server = spawn_retry_after_server();
    let client = DashenClient::new_with_rate_limit(
        DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        format!("{}/customer", server.base_url),
        DashenRateLimitConfig {
            datamsapi_max_concurrent: 2,
            account_max_requests_per_window: 100,
            account_window_ms: 1_000,
            max_http_attempts: 2,
            retry_base_ms: 10,
        },
    )
    .unwrap();

    let payload = client
        .query_match_info("player-customer-token", "retry-me")
        .await
        .unwrap();

    assert_eq!(payload["code"], 0);
    assert_eq!(server.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn full_season_fetch_never_persists_local_or_upstream_tokens() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::{fetch_full_season_with_base_url, FullSeasonRequest};

    let server = spawn_full_season_server();

    let fetch = fetch_full_season_with_base_url(FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "player-customer-token".to_string(),
        input_bnet: "Tester#12345".to_string(),
        resolved_bnet: Some("Tester#12345".to_string()),
        season: 23,
        customer_base_url: format!("{}/customer", server.base_url),
        pages_per_batch: 5,
        detail_concurrency: 2,
        include_fight: true,
    })
    .await
    .unwrap();

    let text = serde_json::to_string(&fetch.payload).unwrap();
    assert_eq!(fetch.entries, 3);
    assert_eq!(fetch.details, 2);
    assert_eq!(fetch.failed_details, 1);
    assert_eq!(fetch.payload["input_bnet"], "Tester#12345");
    assert_eq!(fetch.payload["resolved_bnet"], "Tester#12345");
    assert_eq!(fetch.payload["fetch_mode"], "rust_sync");
    assert_eq!(fetch.payload["season"], 23);
    assert_eq!(fetch.payload["count_info"]["sport"]["code"], 0);
    assert_eq!(fetch.payload["count_info"]["leisure"]["code"], 0);
    assert_eq!(
        fetch.payload["matches"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        fetch.payload["matches"]["details"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        fetch.payload["matches"]["detail_errors"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(!fetch.payload["matches"]["list_pages"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(fetch.payload.get("customer_token").is_none());
    assert!(!text.contains("player-customer-token"));
    assert!(!text.contains("detail-secret"));
    assert!(!text.contains("account-auth-token"));
}

#[tokio::test]
async fn full_season_fetch_fetches_details_with_configured_concurrency() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::{fetch_full_season_with_base_url, FullSeasonRequest};

    let server = spawn_delayed_detail_server();

    let fetch = fetch_full_season_with_base_url(FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "player-customer-token".to_string(),
        input_bnet: "Tester#12345".to_string(),
        resolved_bnet: Some("Tester#12345".to_string()),
        season: 22,
        customer_base_url: format!("{}/customer", server.base_url),
        pages_per_batch: 5,
        detail_concurrency: 3,
        include_fight: false,
    })
    .await
    .unwrap();

    assert_eq!(fetch.entries, 3);
    assert_eq!(fetch.details, 3);
    assert_eq!(
        *server.max_active_details.lock().unwrap(),
        2,
        "detail requests should respect the Python-equivalent datamsapi domain cap"
    );
}

#[tokio::test]
async fn full_season_fetch_tries_current_season_without_season_param_first() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::{fetch_full_season_with_base_url, FullSeasonRequest};

    let server = spawn_full_season_server();

    let fetch = fetch_full_season_with_base_url(FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "player-customer-token".to_string(),
        input_bnet: "Tester#12345".to_string(),
        resolved_bnet: Some("Tester#12345".to_string()),
        season: 23,
        customer_base_url: format!("{}/customer", server.base_url),
        pages_per_batch: 2,
        detail_concurrency: 2,
        include_fight: false,
    })
    .await
    .unwrap();

    assert_eq!(fetch.entries, 2);
    let requests = server.requests.lock().unwrap();
    let first_sport_page = requests
        .iter()
        .find(|request| {
            request.first_line.contains("/customer/queryMatchList")
                && request.first_line.contains("gameMode=sport")
                && request.first_line.contains("page=1")
        })
        .expect("missing sport page 1 request");
    assert!(!first_sport_page.first_line.contains("season=23"));
}

#[tokio::test]
async fn desktop_sync_uses_current_account_credentials_and_writes_explicit_season_directory() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let season_dir = app_data.join("seasons");
    std::fs::create_dir_all(&app_data).unwrap();

    std::fs::write(
        app_data.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"account-auth-token","customer_token":"current-account-customer-token","dts":2026,"server":1}"#,
    )
    .unwrap();

    let server = spawn_full_season_server();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));

    let summary =
        owstats_desktop_lib::sync_season_with_current_account_credentials_path_output_dir_base_url(
            &db,
            &season_dir,
            &app_data.join(".dashen_credentials.json"),
            23,
            &format!("{}/customer", server.base_url),
            |_| {},
        )
        .await
        .unwrap();

    let raw_path = season_dir.join("local_s23_full_season.json");
    assert!(raw_path.exists());
    assert_eq!(summary.imported_matches, 3);
}

#[derive(Clone, Debug)]
struct RecordedRequest {
    first_line: String,
    headers: String,
    body: String,
}

struct RecordingServer {
    base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

struct DelayedDetailServer {
    base_url: String,
    max_active_details: Arc<Mutex<usize>>,
}

struct RetryAfterServer {
    base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

fn spawn_recording_server(routes: Vec<(&'static str, &'static str)>) -> RecordingServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(32) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = [0_u8; 8192];
            let Ok(read) = stream.read(&mut buffer) else {
                continue;
            };
            if read == 0 {
                continue;
            }
            let mut request_bytes = buffer[..read].to_vec();
            read_remaining_body(&mut stream, &mut request_bytes);
            let request = String::from_utf8_lossy(&request_bytes);
            let mut lines = request.lines();
            let first_line = lines.next().unwrap_or_default().to_string();
            let headers = lines
                .take_while(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let body = request
                .split_once("\r\n\r\n")
                .map(|(_, body)| body.to_string())
                .unwrap_or_default();
            captured.lock().unwrap().push(RecordedRequest {
                first_line: first_line.clone(),
                headers,
                body,
            });
            let body = routes
                .iter()
                .find(|(path, _)| first_line.contains(path))
                .map(|(_, body)| *body)
                .unwrap_or(r#"{"code":404,"message":"not found"}"#);
            let status = if body.contains("\"code\":404") {
                "404 Not Found"
            } else {
                "200 OK"
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    RecordingServer {
        base_url: format!("http://{addr}"),
        requests,
    }
}

fn read_remaining_body(stream: &mut std::net::TcpStream, request_bytes: &mut Vec<u8>) {
    let Some(header_end) = request_bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
    else {
        return;
    };
    let headers = String::from_utf8_lossy(&request_bytes[..header_end]);
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    while request_bytes.len().saturating_sub(body_start) < content_length {
        let mut buffer = [0_u8; 4096];
        let Ok(read) = stream.read(&mut buffer) else {
            break;
        };
        if read == 0 {
            break;
        }
        request_bytes.extend_from_slice(&buffer[..read]);
    }
}

fn spawn_full_season_server() -> RecordingServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(256) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let captured = captured.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 8192];
                let Ok(read) = stream.read(&mut buffer) else {
                    return;
                };
                if read == 0 {
                    return;
                }
                let mut request_bytes = buffer[..read].to_vec();
                read_remaining_body(&mut stream, &mut request_bytes);
                let request = String::from_utf8_lossy(&request_bytes);
                let mut lines = request.lines();
                let first_line = lines.next().unwrap_or_default().to_string();
                let headers = lines
                    .take_while(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                let body = request
                    .split_once("\r\n\r\n")
                    .map(|(_, body)| body.to_string())
                    .unwrap_or_default();
                captured.lock().unwrap().push(RecordedRequest {
                    first_line: first_line.clone(),
                    headers,
                    body,
                });
                let (status, body) = full_season_response(&first_line);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    RecordingServer {
        base_url: format!("http://{addr}"),
        requests,
    }
}

fn full_season_response(request_line: &str) -> (&'static str, &'static str) {
    if request_line.contains("/customer/queryCard") {
        return (
            "200 OK",
            r#"{"code":0,"data":{"bnetId":"Tester#12345","level":42}}"#,
        );
    }
    if request_line.contains("/customer/queryCountInfo") && request_line.contains("gameMode=sport")
    {
        return ("200 OK", r#"{"code":0,"data":{"mode":"sport","total":2}}"#);
    }
    if request_line.contains("/customer/queryCountInfo")
        && request_line.contains("gameMode=leisure")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"mode":"leisure","total":0}}"#,
        );
    }
    if request_line.contains("/customer/queryMatchList")
        && request_line.contains("gameMode=sport")
        && request_line.contains("page=1")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"matchList":[
              {"matchId":"normal-ok","matchRet":1,"mapGuid":"map-a","heroGuid":"hero-a","beginTs":1700000000000},
              {"matchId":"normal-fail","matchRet":2,"mapGuid":"map-b","heroGuid":"hero-b","beginTs":1699999900000}
            ]}}"#,
        );
    }
    if request_line.contains("/customer/fight/queryMatchList")
        && request_line.contains("gameMode=QuickFight")
        && request_line.contains("page=1")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"matchList":[
              {"matchId":"fight-ok","matchRet":1,"mapGuid":"arena-a","heroGuid":"hero-c","beginTs":1699999800000}
            ]}}"#,
        );
    }
    if request_line.contains("/customer/queryMatchInfo")
        && request_line.contains("matchId=normal-ok")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"matchRet":1,"customerToken":"detail-secret","mapGuid":"map-a"}}"#,
        );
    }
    if request_line.contains("/customer/fight/queryMatchInfo")
        && request_line.contains("matchId=fight-ok")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"totalCount":{"matchRet":1,"mapGuid":"arena-a"},"roundCountList":[]}}"#,
        );
    }
    if request_line.contains("/customer/queryMatchInfo")
        && request_line.contains("matchId=normal-fail")
    {
        return (
            "500 Internal Server Error",
            r#"{"code":500,"message":"detail failed"}"#,
        );
    }
    ("200 OK", r#"{"code":0,"data":{"matchList":[]}}"#)
}

fn spawn_delayed_detail_server() -> DelayedDetailServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let active_details = Arc::new(Mutex::new(0_usize));
    let max_active_details = Arc::new(Mutex::new(0_usize));
    let active_for_thread = active_details.clone();
    let max_for_thread = max_active_details.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(128) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let active = active_for_thread.clone();
            let max_active = max_for_thread.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 8192];
                let Ok(read) = stream.read(&mut buffer) else {
                    return;
                };
                if read == 0 {
                    return;
                }
                let mut request_bytes = buffer[..read].to_vec();
                read_remaining_body(&mut stream, &mut request_bytes);
                let request = String::from_utf8_lossy(&request_bytes);
                let first_line = request.lines().next().unwrap_or_default().to_string();
                let (status, body) = delayed_detail_response(&first_line, &active, &max_active);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    DelayedDetailServer {
        base_url: format!("http://{addr}"),
        max_active_details,
    }
}

fn spawn_limited_detail_server(delay_ms: u64, take: usize) -> DelayedDetailServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let active_details = Arc::new(Mutex::new(0_usize));
    let max_active_details = Arc::new(Mutex::new(0_usize));
    let active_for_thread = active_details.clone();
    let max_for_thread = max_active_details.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(take) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let active = active_for_thread.clone();
            let max_active = max_for_thread.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 8192];
                let Ok(read) = stream.read(&mut buffer) else {
                    return;
                };
                if read == 0 {
                    return;
                }
                {
                    let mut active = active.lock().unwrap();
                    *active += 1;
                    let mut max_active = max_active.lock().unwrap();
                    *max_active = (*max_active).max(*active);
                }
                if delay_ms > 0 {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
                {
                    let mut active = active.lock().unwrap();
                    *active -= 1;
                }
                let body = r#"{"code":0,"data":{"matchRet":1}}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    DelayedDetailServer {
        base_url: format!("http://{addr}"),
        max_active_details,
    }
}

fn spawn_retry_after_server() -> RetryAfterServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(4) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = [0_u8; 8192];
            let Ok(read) = stream.read(&mut buffer) else {
                continue;
            };
            if read == 0 {
                continue;
            }
            let request = String::from_utf8_lossy(&buffer[..read]);
            let first_line = request.lines().next().unwrap_or_default().to_string();
            captured.lock().unwrap().push(RecordedRequest {
                first_line,
                headers: String::new(),
                body: String::new(),
            });
            let attempt = captured.lock().unwrap().len();
            let (status, extra_headers, body) = if attempt == 1 {
                (
                    "429 Too Many Requests",
                    "Retry-After: 0\r\n",
                    r#"{"code":429,"message":"too many requests"}"#,
                )
            } else {
                ("200 OK", "", r#"{"code":0,"data":{"matchRet":1}}"#)
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    RetryAfterServer {
        base_url: format!("http://{addr}"),
        requests,
    }
}

fn delayed_detail_response(
    request_line: &str,
    active_details: &Arc<Mutex<usize>>,
    max_active_details: &Arc<Mutex<usize>>,
) -> (&'static str, &'static str) {
    if request_line.contains("/customer/queryCard") {
        return ("200 OK", r#"{"code":0,"data":{"bnetId":"Tester#12345"}}"#);
    }
    if request_line.contains("/customer/queryCountInfo") {
        return ("200 OK", r#"{"code":0,"data":{"total":3}}"#);
    }
    if request_line.contains("/customer/queryMatchList")
        && request_line.contains("gameMode=sport")
        && request_line.contains("page=1")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"matchList":[
              {"matchId":"detail-a","beginTs":1700000000000},
              {"matchId":"detail-b","beginTs":1699999900000},
              {"matchId":"detail-c","beginTs":1699999800000}
            ]}}"#,
        );
    }
    if request_line.contains("/customer/queryMatchInfo") {
        {
            let mut active = active_details.lock().unwrap();
            *active += 1;
            let mut max_active = max_active_details.lock().unwrap();
            *max_active = (*max_active).max(*active);
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        {
            let mut active = active_details.lock().unwrap();
            *active -= 1;
        }
        return ("200 OK", r#"{"code":0,"data":{"matchRet":1}}"#);
    }
    ("200 OK", r#"{"code":0,"data":{"matchList":[]}}"#)
}
