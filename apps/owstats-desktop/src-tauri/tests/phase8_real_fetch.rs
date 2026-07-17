use futures_util::future::join_all;
use owstats_desktop_lib::login_target::SYNC_TARGET_CONFIRMATION_ERROR;
use owstats_desktop_lib::player_target::{
    clear_player_target_for_path, player_target_status_for_path, save_player_target_for_path,
    PlayerTarget,
};
use owstats_desktop_lib::{
    quick_list_update_with_credentials_and_target_paths_base_url,
    sync_season_with_credentials_and_target_paths_base_url,
    sync_season_with_credentials_and_target_paths_output_dir_base_url,
    sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url,
    sync_season_with_current_account_credentials_path_output_dir_base_url, write_season_payload,
    OwDatabase, SyncMode,
};
use rusqlite::params;
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
            source_role_id: None,
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
            source_role_id: None,
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
        123,
        "2026-07-03T00:00:00Z",
    )
    .unwrap();

    assert_eq!(parsed.input_bnet, "Tester#12345");
    assert_eq!(parsed.resolved_bnet.as_deref(), Some("Tester#12345"));
    assert_eq!(parsed.customer_token, "player-customer-token");
    assert_eq!(parsed.source_role_id, Some(123));
}

#[tokio::test]
async fn desktop_target_sync_rejects_a_different_login_role_before_db_or_network_work() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    let database_path = temp.path().join("sync.sqlite3");
    std::fs::write(
        &credential_path,
        r#"{"role_id":101,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Other#2000".to_string(),
            resolved_bnet: Some("Other#2000".to_string()),
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-14T00:00:00Z".to_string(),
            source_role_id: Some(202),
        },
    )
    .unwrap();
    let db = OwDatabase::new(&database_path);
    let server = spawn_request_counting_server();

    let error = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap_err()
    .to_string();

    assert_eq!(error, SYNC_TARGET_CONFIRMATION_ERROR);
    assert!(
        server.requests.lock().unwrap().is_empty(),
        "rejected sync must not send an upstream request"
    );
    assert!(
        !database_path.exists(),
        "gate must run before db initialization"
    );
    assert!(
        !season_dir.exists(),
        "gate must run before season output writes"
    );
    for secret in ["101", "202", "Other#2000", "synthetic-player-key", "token"] {
        assert!(!error.contains(secret));
    }
    server.join();
}

#[tokio::test]
async fn legacy_target_path_facades_accept_their_original_signatures() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    let credential_path = temp.path().join("missing-credentials.json");
    let target_path = temp.path().join("missing-target.json");

    let project_error = sync_season_with_credentials_and_target_paths_base_url(
        &db,
        temp.path(),
        &credential_path,
        &target_path,
        23,
        "http://127.0.0.1:1/customer",
        |_| {},
    )
    .await
    .unwrap_err();
    let output_error = sync_season_with_credentials_and_target_paths_output_dir_base_url(
        &db,
        &temp.path().join("seasons"),
        &credential_path,
        &target_path,
        23,
        "http://127.0.0.1:1/customer",
        |_| {},
    )
    .await
    .unwrap_err();

    assert!(!project_error.to_string().is_empty());
    assert!(!output_error.to_string().is_empty());
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
async fn full_season_fetch_preserves_naturally_occurring_upstream_fields() {
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

    let temp = tempfile::tempdir().unwrap();
    let written = write_season_payload(temp.path(), 23, &fetch.payload).unwrap();
    let written_payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(written).unwrap()).unwrap();
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
    assert_eq!(
        fetch.payload["matches"]["details"][0]["payload"]["data"]["customerToken"],
        "detail-secret"
    );
    assert_eq!(
        written_payload["matches"]["details"][0]["payload"]["data"]["customerToken"],
        "detail-secret"
    );
    assert!(fetch.payload.get("customer_token").is_none());
    assert!(!text.contains("player-customer-token"));
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
async fn incremental_fetch_scans_past_known_pages_and_requests_only_missing_details() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::{
        fetch_season_increment_with_base_url, FullSeasonRequest,
    };
    use std::collections::HashSet;

    let server = spawn_incremental_season_server();
    let known_detail_ids = HashSet::from(["synthetic-known".to_string()]);

    let fetch = fetch_season_increment_with_base_url(
        FullSeasonRequest {
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
            pages_per_batch: 1,
            detail_concurrency: 2,
            include_fight: false,
        },
        &known_detail_ids,
    )
    .await
    .unwrap();

    assert_eq!(fetch.entries, 2);
    assert_eq!(fetch.details_requested, 1);
    assert_eq!(fetch.details, 1);
    assert_eq!(
        fetch.payload["matches"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let requests = server.requests.lock().unwrap();
    let detail_requests = requests
        .iter()
        .filter(|request| {
            request_path(&request.first_line)
                .map(|path| path.ends_with("/queryMatchInfo"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    assert_eq!(detail_requests.len(), 1);
    assert_eq!(
        request_query_param(&detail_requests[0].first_line, "matchId"),
        Some("synthetic-new")
    );

    let mut sport_pages = Vec::new();
    let mut leisure_pages = Vec::new();
    for request in requests.iter().filter(|request| {
        request_path(&request.first_line)
            .map(|path| path.ends_with("/queryMatchList"))
            .unwrap_or(false)
    }) {
        let page = request_query_param(&request.first_line, "page")
            .unwrap()
            .parse::<i64>()
            .unwrap();
        match request_query_param(&request.first_line, "gameMode") {
            Some("sport") => sport_pages.push(page),
            Some("leisure") => leisure_pages.push(page),
            mode => panic!("unexpected game mode: {mode:?}"),
        }
    }
    sport_pages.sort_unstable();
    leisure_pages.sort_unstable();
    assert_eq!(sport_pages, vec![1, 2, 3]);
    assert_eq!(leisure_pages, vec![1]);
}

#[tokio::test]
async fn quick_fetch_stops_each_mode_at_its_local_watermark_without_details() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::{fetch_quick_season_with_base_url, FullSeasonRequest};
    use owstats_desktop_lib::{ModeSyncInventory, SeasonModeKey, SeasonSyncInventory};
    use std::collections::{HashMap, HashSet};

    let server = spawn_quick_scan_server(QuickScanScenario::Known);
    let inventory = quick_inventory();
    let fetch = fetch_quick_season_with_base_url(
        FullSeasonRequest {
            credentials: DashenCredentials {
                role_id: 123,
                token: "synthetic-account-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            customer_token: "synthetic-customer-token".to_string(),
            input_bnet: "Synthetic#1000".to_string(),
            resolved_bnet: Some("Synthetic#1000".to_string()),
            season: 22,
            customer_base_url: format!("{}/customer", server.base_url),
            pages_per_batch: 99,
            detail_concurrency: 2,
            include_fight: true,
        },
        &inventory,
    )
    .await
    .unwrap();

    assert_eq!(
        quick_list_pages(&server),
        HashMap::from([
            ("sport".to_string(), vec![1]),
            ("leisure".to_string(), vec![1]),
            ("QuickFight".to_string(), vec![1]),
            ("LeisureFight".to_string(), vec![1]),
            ("SportFight".to_string(), vec![1]),
        ])
    );
    assert_eq!(fetch.details_requested, 0);
    assert!(!fetch.quick_scan_limited);
    assert!(fetch
        .quick_scan_stop_reasons
        .values()
        .all(|reason| reason == "watermark_reached"));

    fn quick_inventory() -> SeasonSyncInventory {
        let mut inventory = SeasonSyncInventory::default();
        for (kind, mode) in [
            ("normal", "sport"),
            ("normal", "leisure"),
            ("fight", "QuickFight"),
            ("fight", "LeisureFight"),
            ("fight", "SportFight"),
        ] {
            let id = format!("known-{mode}");
            inventory.match_ids.insert(id.clone());
            inventory.detail_ids.insert(id.clone());
            inventory.modes.insert(
                SeasonModeKey::new(kind, mode),
                ModeSyncInventory {
                    match_ids: HashSet::from([id.clone()]),
                    detail_ids: HashSet::from([id]),
                    latest_begin_ts: Some(1_700_000_000),
                },
            );
        }
        inventory
    }
}

#[tokio::test]
async fn quick_fetch_scans_new_sport_until_known_boundary_and_only_fetches_scanned_details() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    let server = spawn_quick_scan_server(QuickScanScenario::NewSport);
    let inventory = quick_test_inventory();
    let fetch = fetch_quick_season_with_base_url(
        quick_request(
            &server,
            DashenCredentials {
                role_id: 123,
                token: "synthetic-account-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            22,
        ),
        &inventory,
    )
    .await
    .unwrap();

    assert_eq!(quick_list_pages(&server)["sport"], vec![1, 2, 3]);
    for mode in ["leisure", "QuickFight", "LeisureFight", "SportFight"] {
        assert_eq!(quick_list_pages(&server)[mode], vec![1]);
    }
    assert_eq!(fetch.details_requested, 2);
    let requests = server.requests.lock().unwrap();
    let details = requests
        .iter()
        .filter_map(|request| request_query_param(&request.first_line, "matchId"))
        .collect::<Vec<_>>();
    assert_eq!(details.len(), 2);
    assert!(details.contains(&"new-sport-1"));
    assert!(details.contains(&"new-sport-2"));
}

#[tokio::test]
async fn quick_fetch_caps_unsafe_mode_at_five_pages_and_scans_modes_concurrently() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    let server = spawn_quick_scan_server(QuickScanScenario::UnsafeSportDelayed);
    let fetch = fetch_quick_season_with_base_url(
        quick_request(
            &server,
            DashenCredentials {
                role_id: 123,
                token: "synthetic-account-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            22,
        ),
        &quick_test_inventory(),
    )
    .await
    .unwrap();

    assert_eq!(quick_list_pages(&server)["sport"], vec![1, 2, 3, 4, 5]);
    assert!(!quick_list_pages(&server)["sport"].contains(&6));
    assert!(fetch.quick_scan_limited);
    assert_eq!(
        fetch.quick_scan_stop_reasons["normal:sport"],
        "unsafe_response"
    );
    assert!(
        *server.max_active_lists.lock().unwrap() > 1,
        "different modes should overlap while each mode remains sequential"
    );
}

#[tokio::test]
async fn quick_fetch_stops_on_legal_empty_pages_and_preserves_current_season_fallback() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    let credentials = DashenCredentials {
        role_id: 123,
        token: "synthetic-account-token".to_string(),
        dts: Some(2026),
        server: 1,
    };
    let empty_server = spawn_quick_scan_server(QuickScanScenario::Empty);
    let empty_fetch = fetch_quick_season_with_base_url(
        quick_request(&empty_server, credentials.clone(), 22),
        &quick_test_inventory(),
    )
    .await
    .unwrap();
    assert!(quick_list_pages(&empty_server)
        .values()
        .all(|pages| pages == &[1]));
    assert!(empty_fetch
        .quick_scan_stop_reasons
        .values()
        .all(|reason| reason == "empty"));
    assert!(!empty_fetch.quick_scan_limited);

    let fallback_server = spawn_quick_scan_server(QuickScanScenario::CurrentFallback);
    let fallback_fetch = fetch_quick_season_with_base_url(
        quick_request(&fallback_server, credentials, 23),
        &quick_test_inventory(),
    )
    .await
    .unwrap();
    assert_eq!(fallback_fetch.details_requested, 0);
    let requests = fallback_server.requests.lock().unwrap();
    for mode in [
        "sport",
        "leisure",
        "QuickFight",
        "LeisureFight",
        "SportFight",
    ] {
        let pages = requests
            .iter()
            .filter(|request| {
                request_path(&request.first_line)
                    .is_some_and(|path| path.ends_with("/queryMatchList"))
                    && request_query_param(&request.first_line, "gameMode") == Some(mode)
            })
            .collect::<Vec<_>>();
        assert_eq!(pages.len(), 2);
        assert!(!pages[0].first_line.contains("season=23"));
        assert!(pages[1].first_line.contains("season=23"));
    }
}

#[tokio::test]
async fn quick_fetch_uses_canonical_watermark_from_imported_normal_mode() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("quick.sqlite3"));
    owstats_desktop_lib::import_payload_value_embedded(
        &db,
        &serde_json::json!({
            "season": 22,
            "matches": {
                "entries": [{
                    "matchId": "known-sport",
                    "gameMode": "sport",
                    "instanceType": "IT_RANKED",
                    "matchRet": 1,
                    "beginTs": 1700000000
                }],
                "details": [{
                    "match_id": "known-sport",
                    "match_kind": "normal",
                    "payload": {
                        "code": 0,
                        "data": {
                            "gameTimeSec": 60,
                            "totalCount": {"teammateList": [], "enemyList": []}
                        }
                    }
                }],
                "detail_errors": []
            }
        }),
    )
    .unwrap();
    let stored_mode: String = db
        .connect()
        .unwrap()
        .query_row(
            "SELECT game_mode FROM matches WHERE match_id='known-sport'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_mode, "SportPreset");
    let inventory = owstats_desktop_lib::load_season_sync_inventory(&db, 22).unwrap();
    let server = spawn_quick_scan_server(QuickScanScenario::Known);
    let mut request = quick_request(
        &server,
        DashenCredentials {
            role_id: 123,
            token: "synthetic-account-token".to_string(),
            dts: Some(2026),
            server: 1,
        },
        22,
    );
    request.include_fight = false;

    let fetch = fetch_quick_season_with_base_url(request, &inventory)
        .await
        .unwrap();

    assert_eq!(quick_list_pages(&server)["sport"], vec![1]);
    assert_eq!(
        fetch.quick_scan_stop_reasons["normal:sport"],
        "watermark_reached"
    );
}

#[tokio::test]
async fn quick_fetch_shares_five_page_budget_across_current_season_candidates() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    let server = spawn_quick_scan_server(QuickScanScenario::CurrentUnsafeSport);
    let fetch = fetch_quick_season_with_base_url(
        quick_request(
            &server,
            DashenCredentials {
                role_id: 123,
                token: "synthetic-account-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            23,
        ),
        &quick_test_inventory(),
    )
    .await
    .unwrap();

    let requests = server.requests.lock().unwrap();
    let sport_requests = requests
        .iter()
        .filter(|request| {
            request_path(&request.first_line).is_some_and(|path| path.ends_with("/queryMatchList"))
                && request_query_param(&request.first_line, "gameMode") == Some("sport")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sport_requests
            .iter()
            .map(|request| request_query_param(&request.first_line, "page").unwrap())
            .collect::<Vec<_>>(),
        vec!["1", "2", "3", "4", "5"]
    );
    assert!(sport_requests
        .iter()
        .all(|request| !request.first_line.contains("season=23")));
    assert!(fetch.quick_scan_limited);
    assert_eq!(
        fetch.quick_scan_stop_reasons["normal:sport"],
        "unsafe_response"
    );
}

#[tokio::test]
async fn quick_fetch_preserves_unsafe_outcome_after_later_empty_or_watermark() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    for scenario in [
        QuickScanScenario::UnsafeThenEmpty,
        QuickScanScenario::UnsafeThenKnown,
    ] {
        let server = spawn_quick_scan_server(scenario);
        let fetch = fetch_quick_season_with_base_url(
            quick_request(
                &server,
                DashenCredentials {
                    role_id: 123,
                    token: "synthetic-account-token".to_string(),
                    dts: Some(2026),
                    server: 1,
                },
                22,
            ),
            &quick_test_inventory(),
        )
        .await
        .unwrap();

        assert_eq!(quick_list_pages(&server)["sport"], vec![1, 2]);
        assert_eq!(
            fetch.quick_scan_stop_reasons["normal:sport"],
            "unsafe_response"
        );
        assert!(fetch.quick_scan_limited);
    }
}

#[tokio::test]
async fn quick_fetch_trusts_fifth_page_empty_after_collecting_current_candidate_entries() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    let server = spawn_quick_scan_server(QuickScanScenario::CurrentEmptyAtBudget);
    let fetch = fetch_quick_season_with_base_url(
        quick_request(
            &server,
            DashenCredentials {
                role_id: 123,
                token: "synthetic-account-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            23,
        ),
        &quick_test_inventory(),
    )
    .await
    .unwrap();

    let requests = server.requests.lock().unwrap();
    let sport_requests = requests
        .iter()
        .filter(|request| {
            request_path(&request.first_line).is_some_and(|path| path.ends_with("/queryMatchList"))
                && request_query_param(&request.first_line, "gameMode") == Some("sport")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sport_requests
            .iter()
            .map(|request| request_query_param(&request.first_line, "page").unwrap())
            .collect::<Vec<_>>(),
        vec!["1", "2", "3", "4", "5"]
    );
    assert!(sport_requests
        .iter()
        .all(|request| !request.first_line.contains("season=23")));
    assert_eq!(fetch.quick_scan_stop_reasons["normal:sport"], "empty");
    assert!(!fetch.quick_scan_limited);
}

#[tokio::test]
async fn quick_fetch_discards_valid_shaped_entries_from_business_error_pages() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::fetch_quick_season_with_base_url;

    let server = spawn_quick_scan_server(QuickScanScenario::BusinessErrorEntryThenKnown);
    let fetch = fetch_quick_season_with_base_url(
        quick_request(
            &server,
            DashenCredentials {
                role_id: 123,
                token: "synthetic-account-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            22,
        ),
        &quick_test_inventory(),
    )
    .await
    .unwrap();

    assert_eq!(quick_list_pages(&server)["sport"], vec![1, 2]);
    assert_eq!(fetch.details_requested, 0);
    assert_eq!(
        fetch.quick_scan_stop_reasons["normal:sport"],
        "unsafe_response"
    );
    assert!(fetch.quick_scan_limited);
    let persisted = serde_json::to_string(&fetch.payload).unwrap();
    assert!(!persisted.contains("poisoned-new-entry"));
    assert!(fetch.payload["matches"]["list_pages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|page| page["gameMode"] == "sport" && page["page"] == 1));
}

#[tokio::test]
async fn quick_fetch_rejects_empty_customer_token_with_chinese_message() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::{fetch_quick_season_with_base_url, FullSeasonRequest};

    let error = fetch_quick_season_with_base_url(
        FullSeasonRequest {
            credentials: DashenCredentials {
                role_id: 123,
                token: "synthetic-account-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            customer_token: " ".to_string(),
            input_bnet: "Synthetic#1000".to_string(),
            resolved_bnet: None,
            season: 23,
            customer_base_url: "http://127.0.0.1:1/customer".to_string(),
            pages_per_batch: 1,
            detail_concurrency: 1,
            include_fight: true,
        },
        &quick_test_inventory(),
    )
    .await
    .unwrap_err();

    assert_eq!(error.to_string(), "请先绑定目标玩家");
}

#[tokio::test]
async fn quick_update_without_local_watermark_rejects_before_network_or_database_write() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db_path = temp.path().join("sync.sqlite3");
    let db = OwDatabase::new(&db_path);

    let error = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::QuickUpdate,
        "http://127.0.0.1:1/customer",
        |_| {},
    )
    .await
    .unwrap_err()
    .to_string();

    assert_eq!(error, "该赛季尚无可用本地水位，请先使用全量抓取。");
    assert!(!db_path.exists());
    assert!(!season_dir.exists());
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
    let mut progresses = Vec::new();

    let summary = sync_season_with_current_account_credentials_path_output_dir_base_url(
        &db,
        &season_dir,
        &app_data.join(".dashen_credentials.json"),
        23,
        &format!("{}/customer", server.base_url),
        |progress| progresses.push(progress),
    )
    .await
    .unwrap();

    let raw_path = season_dir.join("local_s23_full_season.json");
    assert!(raw_path.exists());
    assert_eq!(summary.imported_matches, 3);
    let detail_progress = progresses
        .iter()
        .find(|progress| progress.phase == "正在重新获取全部详情")
        .expect("missing completed detail progress");
    assert_eq!(detail_progress.phase, "正在重新获取全部详情");
    assert_eq!(detail_progress.processed, 3);
    assert_eq!(detail_progress.total, 3);
    assert_eq!(detail_progress.failed_details, 1);
    assert!(progresses
        .iter()
        .filter(|progress| progress.total > 0)
        .all(|progress| !progress.phase.contains('/')));
}

#[tokio::test]
async fn desktop_sync_dispatches_full_then_quick_and_full_refetches_all_details() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: Some("Synthetic#1234".to_string()),
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let server = spawn_successful_desktop_sync_server(false);
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    let mut phases = Vec::new();

    let first = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", server.base_url),
        |progress| phases.push(progress.phase),
    )
    .await
    .unwrap();

    assert_eq!(first.list_entries_seen, 3);
    assert_eq!(first.mode, SyncMode::FullFetch);
    assert!(!first.quick_scan_limited);
    assert_eq!(first.new_matches, 3);
    assert_eq!(first.missing_details, 0);
    assert_eq!(first.details_requested, 3);
    assert_eq!(first.repaired_details, 0);
    assert_eq!(first.database_matches, 3);
    assert_eq!(first.database_details, 3);
    assert!(!first.diagnostics.pages.is_empty());
    assert!(first
        .diagnostics
        .pages
        .iter()
        .all(|page| page.earliest_begin_ts <= page.latest_begin_ts));
    assert_eq!(
        phases,
        vec![
            "正在准备全量抓取",
            "正在扫描完整赛季列表",
            "正在重新获取全部详情",
            "正在更新本地数据库",
            "正在重建完整快照",
            "全量抓取完成",
        ]
    );
    let first_requests = server.requests.lock().unwrap().clone();
    assert!(first_requests
        .iter()
        .any(|request| request.first_line.contains("/queryMatchList")));
    assert_eq!(
        first_requests
            .iter()
            .filter(|request| request.first_line.contains("/queryMatchInfo"))
            .count(),
        3
    );
    server.requests.lock().unwrap().clear();

    let mut quick_progresses = Vec::new();
    let second = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::QuickUpdate,
        &format!("{}/customer", server.base_url),
        |progress| quick_progresses.push(progress),
    )
    .await
    .unwrap();

    assert_eq!(second.list_entries_seen, 3);
    assert_eq!(second.mode, SyncMode::QuickUpdate);
    assert!(!second.quick_scan_limited);
    assert!(second
        .diagnostics
        .pages
        .iter()
        .any(|page| page.stop_reason.as_deref() == Some("watermark_reached")));
    assert_eq!(second.new_matches, 0);
    assert_eq!(second.missing_details, 0);
    assert_eq!(second.details_requested, 0);
    assert_eq!(second.database_matches, 3);
    assert_eq!(second.database_details, 3);
    assert_eq!(
        quick_progresses
            .iter()
            .map(|progress| progress.phase.as_str())
            .collect::<Vec<_>>(),
        vec![
            "正在检查本地水位",
            "正在快速检查最新对局",
            "发现 0 场新对局",
            "正在获取最新对局详情",
            "正在更新本地数据库",
            "更新完成",
        ]
    );
    let quick_detail_progress = quick_progresses
        .iter()
        .find(|progress| progress.phase == "正在获取最新对局详情")
        .unwrap();
    assert_eq!(quick_detail_progress.processed, 0);
    assert_eq!(quick_detail_progress.total, 0);
    let discovered_progress = quick_progresses
        .iter()
        .find(|progress| progress.phase == "发现 0 场新对局")
        .unwrap();
    assert_eq!(discovered_progress.processed, 0);
    assert_eq!(discovered_progress.total, 0);
    let second_requests = server.requests.lock().unwrap().clone();
    assert!(second_requests
        .iter()
        .any(|request| request.first_line.contains("/queryMatchList")));
    assert_eq!(
        second_requests
            .iter()
            .filter(|request| request.first_line.contains("/queryMatchInfo"))
            .count(),
        0
    );
    server.requests.lock().unwrap().clear();

    let refreshed = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(refreshed.mode, SyncMode::FullFetch);
    assert_eq!(refreshed.details_requested, 3);
    let refreshed_requests = server.requests.lock().unwrap().clone();
    assert_eq!(
        refreshed_requests
            .iter()
            .filter(|request| request.first_line.contains("/queryMatchInfo"))
            .count(),
        3
    );
    assert!(refreshed_requests.iter().any(|request| {
        request.first_line.contains("/queryMatchList")
            && request.first_line.contains("gameMode=sport")
            && request.first_line.contains("page=2")
    }));
    let snapshot: serde_json::Value = serde_json::from_slice(
        &std::fs::read(season_dir.join("local_s23_full_season.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot["matches"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(snapshot["matches"]["details"].as_array().unwrap().len(), 3);
    let summary_text = serde_json::to_string(&second).unwrap();
    for secret in [
        "normal-one",
        "normal-two",
        "normal-three",
        "Synthetic#1234",
        "synthetic-player-key",
        "synthetic-account-secret",
    ] {
        assert!(!summary_text.contains(secret));
    }
}

#[tokio::test]
async fn unchanged_quick_update_uses_one_list_request_per_canonical_mode_and_no_details() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    seed_complete_canonical_mode_watermarks(&db);
    let server = spawn_quick_scan_server(QuickScanScenario::Known);

    let summary = quick_list_update_with_credentials_and_target_paths_base_url(
        &db,
        &credential_path,
        &target_path,
        23,
        &format!("{}/customer", server.base_url),
    )
    .await
    .unwrap();

    assert!(!summary.data_changed);
    assert_eq!(summary.new_list_matches, 0);
    assert_eq!(summary.pending_details, 0);
    assert!(!season_dir.exists());
    let pages = quick_list_pages(&server);
    assert_eq!(
        pages.len(),
        5,
        "quick update must not request unknown modes"
    );
    for mode in [
        "sport",
        "leisure",
        "QuickFight",
        "LeisureFight",
        "SportFight",
    ] {
        assert_eq!(
            pages.get(mode).map(Vec::as_slice),
            Some([1_i64].as_slice()),
            "unchanged quick update must request exactly page one for {mode}"
        );
    }
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.first_line.contains("/queryMatchList"))
            .count(),
        5,
        "unchanged quick update must make exactly five list requests"
    );
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.first_line.contains("/queryMatchInfo"))
            .count(),
        0
    );
    assert_eq!(
        server.requests.lock().unwrap().len(),
        5,
        "quick foreground must issue only five page-one list requests"
    );
}

#[tokio::test]
async fn quick_budget_defers_deep_history_until_full_fetch_and_rebuilds_a_safe_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    seed_complete_canonical_mode_watermarks(&db);
    let server = spawn_quick_scan_server(QuickScanScenario::DeepHistory);

    let quick = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::QuickUpdate,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(quick.mode, SyncMode::QuickUpdate);
    assert!(quick.quick_scan_limited);
    assert_eq!(
        db.connect()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM matches WHERE match_id='deep-late-match'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0,
        "page-six history is outside the quick-update budget"
    );
    let quick_sport_pages = server
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| {
            request.first_line.contains("/queryMatchList")
                && request.first_line.contains("gameMode=sport")
        })
        .filter_map(|request| request_query_param(&request.first_line, "page"))
        .map(|page| page.parse::<i64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(quick_sport_pages, vec![1, 2, 3, 4, 5]);
    server.requests.lock().unwrap().clear();

    let full = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(full.mode, SyncMode::FullFetch);
    assert_eq!(
        db.connect()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM matches WHERE match_id='deep-late-match'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1,
        "full fetch must continue through page six"
    );
    let snapshot = std::fs::read_to_string(season_dir.join("local_s23_full_season.json")).unwrap();
    assert!(snapshot.contains("known-sport"));
    assert!(snapshot.contains("deep-late-match"));
    for sensitive in [
        "synthetic-account-secret",
        "synthetic-player-key",
        "synthetic-detail-token",
        "synthetic-authorization-secret",
        "customerToken",
        "authorization",
    ] {
        assert!(
            !snapshot.contains(sensitive),
            "rebuilt snapshot leaked {sensitive}"
        );
    }
}

#[tokio::test]
async fn desktop_incremental_empty_fetch_preserves_database_and_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    let populated = spawn_successful_desktop_sync_server(false);
    sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", populated.base_url),
        |_| {},
    )
    .await
    .unwrap();
    let snapshot_path = season_dir.join("local_s23_full_season.json");
    let before: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&snapshot_path).unwrap()).unwrap();

    let empty = spawn_successful_desktop_sync_server(true);
    let summary = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::QuickUpdate,
        &format!("{}/customer", empty.base_url),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(summary.mode, SyncMode::QuickUpdate);
    assert_eq!(summary.list_entries_seen, 0);
    assert_eq!(summary.new_matches, 0);
    assert_eq!(summary.missing_details, 0);
    assert_eq!(summary.details_requested, 0);
    assert_eq!(summary.database_matches, 3);
    assert_eq!(summary.database_details, 3);
    let after: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&snapshot_path).unwrap()).unwrap();
    assert_eq!(after["matches"]["entries"], before["matches"]["entries"]);
    assert_eq!(after["matches"]["details"], before["matches"]["details"]);
    let conn = db.connect().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM matches WHERE season=23", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        3
    );
    let requests = empty.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.first_line.contains("/queryMatchList"))
            .count(),
        10,
        "recognized empty quick update should check the season fallbacks for each canonical mode"
    );
    for mode in [
        "sport",
        "leisure",
        "QuickFight",
        "LeisureFight",
        "SportFight",
    ] {
        assert_eq!(
            requests
                .iter()
                .filter(|request| {
                    request.first_line.contains("/queryMatchList")
                        && request.first_line.contains(&format!("gameMode={mode}"))
                        && request.first_line.contains("page=1")
                })
                .count(),
            2,
            "recognized empty quick update should try both season candidates for {mode}"
        );
    }
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.first_line.contains("/queryMatchInfo"))
            .count(),
        0,
        "recognized empty quick update must not request details"
    );
}

#[tokio::test]
async fn desktop_full_fetch_still_rejects_a_zero_entry_season() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    let empty = spawn_successful_desktop_sync_server(true);

    let error = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", empty.base_url),
        |_| {},
    )
    .await
    .unwrap_err()
    .to_string();

    assert!(error.contains("未拉取到 S23 赛季对局"));
    assert!(!season_dir.join("local_s23_full_season.json").exists());
}

#[tokio::test]
async fn desktop_incremental_snapshot_failure_reports_committed_database_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let blocked_output = temp.path().join("blocked-output");
    std::fs::write(&blocked_output, b"not a directory").unwrap();
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    let server = spawn_successful_desktop_sync_server(false);

    let error = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &blocked_output,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap_err()
    .to_string();

    assert!(error.contains("数据库更新成功，但本地快照更新失败："));
    assert!(!error.contains("synthetic-account-secret"));
    assert!(!error.contains("synthetic-player-key"));
    let conn = db.connect().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM matches WHERE season=23", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        3
    );
}

#[tokio::test]
async fn desktop_incremental_counts_only_valid_database_details_after_failed_repair() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
            begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
            mitigation, total_worth, duration_sec, raw_entry_json, updated_at
         ) VALUES (
            'metric-invalid', 23, 'preset', 'sport', 'normal', 'win', '', '',
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, '{}', 'now'
         )",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
         VALUES ('metric-invalid', 'normal', '{}', 'now')",
        [],
    )
    .unwrap();
    drop(conn);
    let server = spawn_failed_detail_metric_server();

    let summary = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::QuickUpdate,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(summary.failed_details, 1);
    assert_eq!(summary.database_matches, 1);
    assert_eq!(summary.database_details, 0);
    let conn = db.connect().unwrap();
    let details_count: i64 = conn
        .query_row(
            "SELECT details_count FROM seasons WHERE season=23",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(details_count, 0);
    drop(conn);
    let inventory = owstats_desktop_lib::load_season_sync_inventory(&db, 23).unwrap();
    assert!(inventory.match_ids.contains("metric-invalid"));
    assert!(!inventory.detail_ids.contains("metric-invalid"));
}

#[tokio::test]
async fn full_fetch_invalid_refresh_preserves_old_detail_and_player_index_and_records_error() {
    let temp = tempfile::tempdir().unwrap();
    let credential_path = temp.path().join("credentials.json");
    let target_path = temp.path().join("target.json");
    let season_dir = temp.path().join("seasons");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"synthetic-account-secret","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &target_path,
        &PlayerTarget {
            input_bnet: "Synthetic#1234".to_string(),
            resolved_bnet: None,
            customer_token: "synthetic-player-key".to_string(),
            bound_at: "2026-07-16T00:00:00Z".to_string(),
            source_role_id: Some(123),
        },
    )
    .unwrap();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));
    owstats_desktop_lib::import_payload_value_embedded(
        &db,
        &serde_json::json!({
            "season": 23,
            "matches": {
                "entries": [{
                    "matchId": "metric-invalid",
                    "gameMode": "sport",
                    "matchRet": 1,
                    "beginTs": 1700000000000_i64
                }],
                "details": [{
                    "match_id": "metric-invalid",
                    "match_kind": "normal",
                    "payload": {"code": 0, "data": {
                        "gameTimeSec": 60,
                        "totalCount": {
                            "teammateList": [{"bnetId": "old-player", "name": "旧玩家"}],
                            "enemyList": []
                        }
                    }}
                }],
                "detail_errors": []
            }
        }),
    )
    .unwrap();
    let old_detail: String = db
        .connect()
        .unwrap()
        .query_row(
            "SELECT raw_detail_json FROM match_details WHERE match_id='metric-invalid'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let server = spawn_failed_detail_metric_server();

    let summary = sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        SyncMode::FullFetch,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(summary.mode, SyncMode::FullFetch);
    assert_eq!(summary.details_requested, 1);
    assert_eq!(summary.failed_details, 1);
    let conn = db.connect().unwrap();
    let refreshed_detail: String = conn
        .query_row(
            "SELECT raw_detail_json FROM match_details WHERE match_id='metric-invalid'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(refreshed_detail, old_detail);
    let indexed_player: String = conn
        .query_row(
            "SELECT bnet_id FROM match_players WHERE match_id='metric-invalid'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed_player, "old-player");
    let error_message: String = conn
        .query_row(
            "SELECT message FROM sync_errors WHERE season=23 AND match_id='metric-invalid'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!error_message.contains("synthetic-account-secret"));
    assert!(!error_message.contains("synthetic-player-key"));
    let snapshot = std::fs::read_to_string(season_dir.join("local_s23_full_season.json")).unwrap();
    assert!(snapshot.contains("metric-invalid"));
    assert!(snapshot.contains("old-player"));
    assert!(!snapshot.contains("synthetic-account-secret"));
    assert!(!snapshot.contains("synthetic-player-key"));
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

struct RequestCountingServer {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Option<std::sync::mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl RequestCountingServer {
    fn join(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        self.handle.take().unwrap().join().unwrap();
    }
}

impl Drop for RequestCountingServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn request_path(first_line: &str) -> Option<&str> {
    first_line
        .split_whitespace()
        .nth(1)?
        .split_once('?')
        .map(|(path, _)| path)
}

fn request_query_param<'a>(first_line: &'a str, name: &str) -> Option<&'a str> {
    let (_, query) = first_line.split_whitespace().nth(1)?.split_once('?')?;
    query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == name).then_some(value)
    })
}

fn spawn_request_counting_server() -> RequestCountingServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let first_line = String::from_utf8_lossy(&buffer[..read])
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                captured.lock().unwrap().push(first_line);
                let _ = stream.write_all(
                    b"HTTP/1.1 500 Synthetic Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::yield_now();
            }
            Err(error) => panic!("request counting server failed: {error}"),
        }
    });
    RequestCountingServer {
        base_url: format!("http://{address}"),
        requests,
        stop: Some(stop_tx),
        handle: Some(handle),
    }
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

fn seed_complete_canonical_mode_watermarks(db: &OwDatabase) {
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for (match_id, match_kind, game_mode) in [
        ("known-sport", "normal", "sport"),
        ("known-leisure", "normal", "leisure"),
        ("known-QuickFight", "fight", "QuickFight"),
        ("known-LeisureFight", "fight", "LeisureFight"),
        ("known-SportFight", "fight", "SportFight"),
    ] {
        let raw_entry = serde_json::json!({
            "matchId": match_id,
            "gameMode": game_mode,
            "matchRet": 1,
            "beginTs": 1_700_000_000_000_i64,
            "_dashenSeason": 23
        })
        .to_string();
        conn.execute(
            "INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
                begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
                mitigation, total_worth, duration_sec, raw_entry_json, updated_at
             ) VALUES (?1, 23, 'preset', ?2, ?3, 'win', '', '',
                1700000000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 60, ?4, 'now')",
            params![match_id, game_mode, match_kind, raw_entry],
        )
        .unwrap();
        let raw_detail = serde_json::json!({
            "match_id": match_id,
            "match_kind": match_kind,
            "payload": {"code": 0, "data": {"matchRet": 1, "duration": 60}}
        })
        .to_string();
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
             VALUES (?1, ?2, ?3, 'now')",
            params![match_id, match_kind, raw_detail],
        )
        .unwrap();
    }
}

fn spawn_successful_desktop_sync_server(empty: bool) -> RecordingServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(512) {
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
                let request = String::from_utf8_lossy(&buffer[..read]);
                let first_line = request.lines().next().unwrap_or_default().to_string();
                captured.lock().unwrap().push(RecordedRequest {
                    first_line: first_line.clone(),
                    headers: String::new(),
                    body: String::new(),
                });
                let body = if first_line.contains("/queryCard") {
                    r#"{"code":0,"data":{"level":42}}"#
                } else if first_line.contains("/queryCountInfo") {
                    r#"{"code":0,"data":{"total":3}}"#
                } else if !empty
                    && first_line.contains("/queryMatchList")
                    && first_line.contains("gameMode=sport")
                    && first_line.contains("page=1")
                {
                    r#"{"code":0,"data":{"matchList":[
                        {"matchId":"normal-one","matchRet":1,"beginTs":1700000000000},
                        {"matchId":"normal-two","matchRet":2,"beginTs":1699999900000},
                        {"matchId":"normal-three","matchRet":1,"beginTs":1699999800000}
                    ]}}"#
                } else if first_line.contains("/queryMatchInfo") {
                    r#"{"code":0,"data":{"matchRet":1,"duration":120}}"#
                } else {
                    r#"{"code":0,"data":{"matchList":[]}}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

fn spawn_failed_detail_metric_server() -> RecordingServer {
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
                let request = String::from_utf8_lossy(&buffer[..read]);
                let first_line = request.lines().next().unwrap_or_default().to_string();
                captured.lock().unwrap().push(RecordedRequest {
                    first_line: first_line.clone(),
                    headers: String::new(),
                    body: String::new(),
                });
                let (status, body) = if first_line.contains("/queryCard") {
                    ("200 OK", r#"{"code":0,"data":{"level":42}}"#)
                } else if first_line.contains("/queryCountInfo") {
                    ("200 OK", r#"{"code":0,"data":{"total":1}}"#)
                } else if first_line.contains("/queryMatchList")
                    && first_line.contains("gameMode=sport")
                    && first_line.contains("page=1")
                {
                    (
                        "200 OK",
                        r#"{"code":0,"data":{"matchList":[
                            {"matchId":"metric-invalid","matchRet":1,"beginTs":1700000000000}
                        ]}}"#,
                    )
                } else if first_line.contains("/queryMatchInfo")
                    && first_line.contains("matchId=metric-invalid")
                {
                    (
                        "500 Internal Server Error",
                        r#"{"code":500,"message":"synthetic detail failure"}"#,
                    )
                } else {
                    ("200 OK", r#"{"code":0,"data":{"matchList":[]}}"#)
                };
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

fn spawn_incremental_season_server() -> RecordingServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(64) {
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
                let request = String::from_utf8_lossy(&buffer[..read]);
                let mut lines = request.lines();
                let first_line = lines.next().unwrap_or_default().to_string();
                let headers = lines
                    .take_while(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                captured.lock().unwrap().push(RecordedRequest {
                    first_line: first_line.clone(),
                    headers,
                    body: String::new(),
                });
                let body = incremental_season_response(&first_line);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

fn incremental_season_response(request_line: &str) -> &'static str {
    if request_line.contains("/customer/queryCard") {
        return r#"{"code":0,"data":{"bnetId":"Tester#12345"}}"#;
    }
    if request_line.contains("/customer/queryCountInfo") {
        return r#"{"code":0,"data":{"total":2}}"#;
    }
    if request_line.contains("/customer/queryMatchList")
        && request_line.contains("gameMode=sport")
        && request_line.contains("page=1")
    {
        return r#"{"code":0,"data":{"matchList":[{"matchId":"synthetic-known","beginTs":1700000000000}]}}"#;
    }
    if request_line.contains("/customer/queryMatchList")
        && request_line.contains("gameMode=sport")
        && request_line.contains("page=2")
    {
        return r#"{"code":0,"data":{"matchList":[{"matchId":"synthetic-new","beginTs":1699999900000}]}}"#;
    }
    if request_line.contains("/customer/queryMatchInfo")
        && request_line.contains("matchId=synthetic-new")
    {
        return r#"{"code":0,"data":{"matchRet":1}}"#;
    }
    r#"{"code":0,"data":{"matchList":[]}}"#
}

#[derive(Clone, Copy)]
enum QuickScanScenario {
    Known,
    NewSport,
    UnsafeSportDelayed,
    Empty,
    CurrentFallback,
    CurrentUnsafeSport,
    UnsafeThenEmpty,
    UnsafeThenKnown,
    CurrentEmptyAtBudget,
    BusinessErrorEntryThenKnown,
    DeepHistory,
}

struct QuickScanServer {
    base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    max_active_lists: Arc<Mutex<usize>>,
}

fn quick_request(
    server: &QuickScanServer,
    credentials: owstats_desktop_lib::dashen_client::DashenCredentials,
    season: i64,
) -> owstats_desktop_lib::season_fetch::FullSeasonRequest {
    owstats_desktop_lib::season_fetch::FullSeasonRequest {
        credentials,
        customer_token: "synthetic-customer-token".to_string(),
        input_bnet: "Synthetic#1000".to_string(),
        resolved_bnet: Some("Synthetic#1000".to_string()),
        season,
        customer_base_url: format!("{}/customer", server.base_url),
        pages_per_batch: 99,
        detail_concurrency: 2,
        include_fight: true,
    }
}

fn quick_test_inventory() -> owstats_desktop_lib::SeasonSyncInventory {
    use owstats_desktop_lib::{ModeSyncInventory, SeasonModeKey, SeasonSyncInventory};
    use std::collections::HashSet;

    let mut inventory = SeasonSyncInventory::default();
    for (kind, mode) in [
        ("normal", "sport"),
        ("normal", "leisure"),
        ("fight", "QuickFight"),
        ("fight", "LeisureFight"),
        ("fight", "SportFight"),
    ] {
        let id = format!("known-{mode}");
        inventory.match_ids.insert(id.clone());
        inventory.detail_ids.insert(id.clone());
        inventory.modes.insert(
            SeasonModeKey::new(kind, mode),
            ModeSyncInventory {
                match_ids: HashSet::from([id.clone()]),
                detail_ids: HashSet::from([id]),
                latest_begin_ts: Some(1_700_000_000),
            },
        );
    }
    inventory
}

fn quick_list_pages(server: &QuickScanServer) -> std::collections::HashMap<String, Vec<i64>> {
    let mut pages = std::collections::HashMap::<String, Vec<i64>>::new();
    for request in server.requests.lock().unwrap().iter().filter(|request| {
        request_path(&request.first_line).is_some_and(|path| path.ends_with("/queryMatchList"))
    }) {
        let mode = request_query_param(&request.first_line, "gameMode")
            .unwrap()
            .to_string();
        let page = request_query_param(&request.first_line, "page")
            .unwrap()
            .parse()
            .unwrap();
        pages.entry(mode).or_default().push(page);
    }
    for mode_pages in pages.values_mut() {
        mode_pages.sort_unstable();
    }
    pages
}

fn spawn_quick_scan_server(scenario: QuickScanScenario) -> QuickScanServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let active_lists = Arc::new(Mutex::new(0_usize));
    let max_active_lists = Arc::new(Mutex::new(0_usize));
    let captured = requests.clone();
    let active_for_thread = active_lists.clone();
    let max_for_thread = max_active_lists.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(128) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let captured = captured.clone();
            let active_lists = active_for_thread.clone();
            let max_active_lists = max_for_thread.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 8192];
                let Ok(read) = stream.read(&mut buffer) else {
                    return;
                };
                let first_line = String::from_utf8_lossy(&buffer[..read])
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                captured.lock().unwrap().push(RecordedRequest {
                    first_line: first_line.clone(),
                    headers: String::new(),
                    body: String::new(),
                });
                let is_list =
                    request_path(&first_line).is_some_and(|path| path.ends_with("/queryMatchList"));
                if is_list {
                    let mut active = active_lists.lock().unwrap();
                    *active += 1;
                    let mut peak = max_active_lists.lock().unwrap();
                    *peak = (*peak).max(*active);
                }
                if is_list && matches!(scenario, QuickScanScenario::UnsafeSportDelayed) {
                    thread::sleep(Duration::from_millis(40));
                }
                let body = quick_scan_response(&first_line, scenario);
                if is_list {
                    *active_lists.lock().unwrap() -= 1;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    QuickScanServer {
        base_url: format!("http://{addr}"),
        requests,
        max_active_lists,
    }
}

fn quick_scan_response(request_line: &str, scenario: QuickScanScenario) -> String {
    if request_line.contains("/queryCard") {
        return r#"{"code":0,"data":{"level":1}}"#.to_string();
    }
    if request_line.contains("/queryCountInfo") {
        return r#"{"code":0,"data":{"total":1}}"#.to_string();
    }
    if request_line.contains("/queryMatchInfo")
        && matches!(scenario, QuickScanScenario::DeepHistory)
    {
        return r#"{"code":0,"data":{"matchRet":1,"duration":120,"customerToken":"synthetic-detail-token","authorization":"synthetic-authorization-secret"}}"#.to_string();
    }
    if request_line.contains("/queryMatchInfo") {
        return r#"{"code":0,"data":{"matchRet":1}}"#.to_string();
    }
    let Some(mode) = request_query_param(request_line, "gameMode") else {
        return r#"{"code":0,"data":{"matchList":[]}}"#.to_string();
    };
    let page = request_query_param(request_line, "page")
        .and_then(|page| page.parse::<i64>().ok())
        .unwrap_or_default();
    if matches!(scenario, QuickScanScenario::Empty)
        || (matches!(scenario, QuickScanScenario::CurrentFallback)
            && request_query_param(request_line, "season").is_none())
    {
        return r#"{"code":0,"data":{"matchList":[]}}"#.to_string();
    }
    if matches!(scenario, QuickScanScenario::CurrentUnsafeSport) && mode == "sport" {
        return r#"{"code":0,"data":{"matchList":[{"unexpected":"shape"}]}}"#.to_string();
    }
    if matches!(
        scenario,
        QuickScanScenario::UnsafeThenEmpty | QuickScanScenario::UnsafeThenKnown
    ) && mode == "sport"
        && page == 1
    {
        return r#"{"code":500,"data":{"matchList":[]}}"#.to_string();
    }
    if matches!(scenario, QuickScanScenario::BusinessErrorEntryThenKnown)
        && mode == "sport"
        && page == 1
    {
        return r#"{"code":500,"data":{"matchList":[{"matchId":"poisoned-new-entry","beginTs":1700000200}]}}"#
            .to_string();
    }
    if matches!(scenario, QuickScanScenario::BusinessErrorEntryThenKnown)
        && mode == "sport"
        && page == 2
    {
        return r#"{"code":0,"data":{"matchList":[{"matchId":"known-sport","beginTs":1700000000}]}}"#
            .to_string();
    }
    if matches!(scenario, QuickScanScenario::UnsafeThenEmpty) && mode == "sport" && page == 2 {
        return r#"{"code":0,"data":{"matchList":[]}}"#.to_string();
    }
    if matches!(scenario, QuickScanScenario::UnsafeThenKnown) && mode == "sport" && page == 2 {
        return r#"{"code":0,"data":{"matchList":[{"matchId":"known-sport","beginTs":1700000000}]}}"#
            .to_string();
    }
    if matches!(scenario, QuickScanScenario::CurrentEmptyAtBudget) && mode == "sport" {
        if page == 5 {
            return r#"{"code":0,"data":{"matchList":[]}}"#.to_string();
        }
        return format!(
            r#"{{"code":0,"data":{{"matchList":[{{"matchId":"new-sport-{page}","beginTs":1700000200}}]}}}}"#
        );
    }
    if matches!(scenario, QuickScanScenario::UnsafeSportDelayed) && mode == "sport" {
        return r#"{"code":0,"data":{"matchList":[{"unexpected":"shape"}]}}"#.to_string();
    }
    if matches!(scenario, QuickScanScenario::DeepHistory) && mode == "sport" {
        return match page {
            1..=5 => format!(
                r#"{{"code":0,"data":{{"matchList":[{{"matchId":"quick-sport-{page}","matchRet":1,"beginTs":1700000100000}}]}}}}"#
            ),
            6 => r#"{"code":0,"data":{"matchList":[{"matchId":"deep-late-match","matchRet":1,"beginTs":1600000000000}]}}"#.to_string(),
            _ => r#"{"code":0,"data":{"matchList":[]}}"#.to_string(),
        };
    }
    if matches!(scenario, QuickScanScenario::DeepHistory) {
        if page == 1 {
            return format!(
                r#"{{"code":0,"data":{{"matchList":[{{"matchId":"known-{mode}","matchRet":1,"beginTs":1700000000000}}]}}}}"#
            );
        }
        return r#"{"code":0,"data":{"matchList":[]}}"#.to_string();
    }
    if matches!(scenario, QuickScanScenario::NewSport) && mode == "sport" {
        return match page {
            1 => r#"{"code":0,"data":{"matchList":[{"matchId":"new-sport-1","beginTs":1700000200}]}}"#,
            2 => r#"{"code":0,"data":{"matchList":[{"matchId":"new-sport-2","beginTs":1700000100}]}}"#,
            _ => r#"{"code":0,"data":{"matchList":[{"matchId":"known-sport","beginTs":1700000000}]}}"#,
        }
        .to_string();
    }
    format!(
        r#"{{"code":0,"data":{{"matchList":[{{"matchId":"known-{mode}","beginTs":1700000000}}]}}}}"#
    )
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
