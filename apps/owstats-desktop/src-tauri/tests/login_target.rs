use owstats_desktop_lib::credential_store::DashenCredentials;
use owstats_desktop_lib::login_target::{
    bind_detected_target_with_base_url, confirm_switch, decide_target_action,
    reconcile_detected_target, retry_binding, LoginTargetCoordinator, TargetAction,
};
use owstats_desktop_lib::player_target::{
    read_player_target_for_path, save_player_target_for_path, target_matches_login_role,
    PlayerTarget,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

#[test]
fn target_only_matches_the_login_role_that_created_it() {
    let target: PlayerTarget = serde_json::from_value(serde_json::json!({
        "inputBnet": "Tester#12345",
        "resolvedBnet": "Tester#12345",
        "customerToken": "synthetic-player-key",
        "boundAt": "2026-07-14T00:00:00Z",
        "sourceRoleId": 101
    }))
    .unwrap();

    assert_eq!(target.source_role_id, Some(101));
    assert!(target_matches_login_role(&target, 101));
    assert!(!target_matches_login_role(&target, 202));
}

#[test]
fn legacy_target_without_source_role_is_not_verified_for_any_login() {
    let target: PlayerTarget = serde_json::from_value(serde_json::json!({
        "inputBnet": "Legacy#12345",
        "resolvedBnet": null,
        "customerToken": "synthetic-legacy-player-key",
        "boundAt": "2026-07-03T00:00:00Z"
    }))
    .unwrap();

    assert_eq!(target.source_role_id, None);
    assert!(!target_matches_login_role(&target, 101));
    assert!(!target_matches_login_role(&target, 202));
}

#[test]
fn source_role_id_serializes_with_camel_case_field_name() {
    let target = PlayerTarget {
        input_bnet: "Tester#12345".to_string(),
        resolved_bnet: Some("Tester#12345".to_string()),
        customer_token: "synthetic-player-key".to_string(),
        bound_at: "2026-07-14T00:00:00Z".to_string(),
        source_role_id: Some(101),
    };

    let value = serde_json::to_value(target).unwrap();

    assert_eq!(
        value.get("sourceRoleId").and_then(|value| value.as_i64()),
        Some(101)
    );
    assert!(value.get("source_role_id").is_none());
}

#[test]
fn target_action_normalizes_whitespace_fullwidth_hash_and_case() {
    assert_eq!(
        decide_target_action(" Current＃1000 ", None),
        TargetAction::AutoBind
    );
    assert_eq!(
        decide_target_action(" Current＃1000 ", Some("current#1000")),
        TargetAction::KeepCurrent
    );
    assert_eq!(
        decide_target_action("Second#2000", Some("Current#1000")),
        TargetAction::ConfirmSwitch
    );
}

#[test]
fn keeping_a_different_target_disables_sync() {
    let coordinator = LoginTargetCoordinator::default();
    coordinator.replace_switch_required(202, "Second#2000", "Current#1000");

    let status = coordinator.keep_current().unwrap();

    assert_eq!(status.state, "mismatchKept");
    assert_eq!(status.detected_bnet.as_deref(), Some("Second#2000"));
    assert_eq!(status.current_bnet.as_deref(), Some("Current#1000"));
    assert!(!status.sync_allowed);
    assert!(!status.can_confirm_switch);
}

#[test]
fn a_new_login_replaces_the_entire_previous_session() {
    let coordinator = LoginTargetCoordinator::default();
    coordinator.replace_bind_failed(101, "First#1000", "first failure");

    let status = coordinator.replace_unidentified(202);

    assert_eq!(status.state, "unidentified");
    assert_eq!(status.detected_bnet, None);
    assert_eq!(status.current_bnet, None);
    assert!(!status.can_retry);
}

#[test]
fn public_status_json_contains_no_role_or_token_fields() {
    let coordinator = LoginTargetCoordinator::default();
    let status = coordinator.replace_ready(101, "Visible#1000", "Visible#1000");

    let json = serde_json::to_string(&status).unwrap();

    assert!(json.contains("\"detectedBnet\":\"Visible#1000\""));
    assert!(json.contains("\"syncAllowed\":true"));
    for forbidden in ["roleId", "role_id", "token", "customerToken"] {
        assert!(!json
            .to_ascii_lowercase()
            .contains(&forbidden.to_ascii_lowercase()));
    }
}

#[tokio::test]
async fn missing_detected_battletag_is_unidentified_and_cannot_sync() {
    let temp = tempfile::tempdir().unwrap();
    let coordinator = LoginTargetCoordinator::default();
    let status = reconcile_detected_target(
        &coordinator,
        &credentials(101),
        None,
        &temp.path().join("target.json"),
        "http://127.0.0.1:1",
    )
    .await;

    assert_eq!(status.state, "unidentified");
    assert!(!status.sync_allowed);
    assert!(!status.can_retry);
}

#[tokio::test]
async fn automatic_binding_searches_then_queries_and_persists_only_target_secret() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"synthetic-player-key","bnetId":"Fresh#1000"}}"#,
        ),
        Response::ok(
            "/customer/queryCard",
            r#"{"code":0,"data":{"name":"Fresh"}}"#,
        ),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    let coordinator = LoginTargetCoordinator::default();

    let status = reconcile_detected_target(
        &coordinator,
        &credentials(101),
        Some("Fresh#1000"),
        &target_path,
        &server.base_url,
    )
    .await;

    assert_eq!(status.state, "ready");
    assert!(status.sync_allowed);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("POST /searchBnetAccount "));
    assert!(requests[1].starts_with("GET /customer/queryCard?"));
    drop(requests);
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.source_role_id, Some(101));
    assert_eq!(target.resolved_bnet.as_deref(), Some("Fresh#1000"));
    let persisted = std::fs::read_to_string(&target_path).unwrap();
    assert!(persisted.contains("synthetic-player-key"));
    assert!(!persisted.contains("synthetic-account-secret"));
}

#[tokio::test]
async fn same_target_and_role_reuses_persisted_target_without_http() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");
    let coordinator = LoginTargetCoordinator::default();

    let status = reconcile_detected_target(
        &coordinator,
        &credentials(101),
        Some(" current＃1000 "),
        &target_path,
        "http://127.0.0.1:1",
    )
    .await;

    assert_eq!(status.state, "ready");
    assert!(status.sync_allowed);
    assert_eq!(
        read_player_target_for_path(&target_path)
            .unwrap()
            .customer_token,
        "old-player-key"
    );
}

#[tokio::test]
async fn same_tag_from_a_different_role_is_resolved_again_for_current_login() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"new-player-key","bnetId":"Current#1000"}}"#,
        ),
        Response::ok("/customer/queryCard", r#"{"code":0,"data":{}}"#),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");

    let status = reconcile_detected_target(
        &LoginTargetCoordinator::default(),
        &credentials(202),
        Some("current#1000"),
        &target_path,
        &server.base_url,
    )
    .await;

    assert_eq!(status.state, "ready");
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.source_role_id, Some(202));
    assert_eq!(target.customer_token, "new-player-key");
}

#[tokio::test]
async fn different_target_waits_for_confirmation_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");
    let coordinator = LoginTargetCoordinator::default();

    let status = reconcile_detected_target(
        &coordinator,
        &credentials(202),
        Some("Second#2000"),
        &target_path,
        "http://127.0.0.1:1",
    )
    .await;

    assert_eq!(status.state, "switchRequired");
    assert!(status.can_confirm_switch);
    assert!(!status.sync_allowed);
    assert_eq!(
        read_player_target_for_path(&target_path)
            .unwrap()
            .customer_token,
        "old-player-key"
    );
}

#[tokio::test]
async fn confirming_switch_uses_only_the_current_session_detected_tag() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"second-player-key","bnetId":"Second#2000"}}"#,
        ),
        Response::ok("/customer/queryCard", r#"{"code":0,"data":{}}"#),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");
    let coordinator = LoginTargetCoordinator::default();
    coordinator.replace_switch_required(202, "Second#2000", "Current#1000");

    let status = confirm_switch(
        &coordinator,
        &credentials(202),
        &target_path,
        &server.base_url,
    )
    .await;

    assert_eq!(status.state, "ready");
    assert_eq!(status.detected_bnet.as_deref(), Some("Second#2000"));
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.input_bnet, "Second#2000");
    assert_eq!(target.source_role_id, Some(202));
}

#[tokio::test]
async fn bind_failure_keeps_old_target_and_can_retry_with_session_tag() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"new-player-key","bnetId":"Current#1000"}}"#,
        ),
        Response::error("/customer/queryCard", 500),
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"new-player-key","bnetId":"Current#1000"}}"#,
        ),
        Response::ok("/customer/queryCard", r#"{"code":0,"data":{}}"#),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");
    let coordinator = LoginTargetCoordinator::default();

    let failed = reconcile_detected_target(
        &coordinator,
        &credentials(202),
        Some("Current#1000"),
        &target_path,
        &server.base_url,
    )
    .await;

    assert_eq!(failed.state, "bindFailed");
    assert!(failed.can_retry);
    assert!(!failed.sync_allowed);
    assert_eq!(
        read_player_target_for_path(&target_path)
            .unwrap()
            .customer_token,
        "old-player-key"
    );
    assert!(!failed.message.contains("synthetic-account-secret"));

    let retried = retry_binding(
        &coordinator,
        &credentials(202),
        &target_path,
        &server.base_url,
    )
    .await;

    assert_eq!(retried.state, "ready");
    assert_eq!(
        read_player_target_for_path(&target_path)
            .unwrap()
            .customer_token,
        "new-player-key"
    );
}

#[test]
fn restart_requires_verified_role_association() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");
    let coordinator = LoginTargetCoordinator::default();

    let ready = coordinator.status_or_persisted(Some(&credentials(101)), &target_path);
    assert_eq!(ready.state, "ready");
    assert!(ready.sync_allowed);

    coordinator.clear();
    let mismatch = coordinator.status_or_persisted(Some(&credentials(202)), &target_path);
    assert_eq!(mismatch.state, "verificationRequired");
    assert!(!mismatch.sync_allowed);

    let mut legacy = read_player_target_for_path(&target_path).unwrap();
    legacy.source_role_id = None;
    save_player_target_for_path(&target_path, &legacy).unwrap();
    coordinator.clear();
    let legacy_status = coordinator.status_or_persisted(Some(&credentials(101)), &target_path);
    assert_eq!(legacy_status.state, "verificationRequired");
    assert!(!legacy_status.sync_allowed);
}

#[tokio::test]
async fn direct_binding_does_not_replace_target_when_query_card_fails() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"new-player-key","bnetId":"New#2000"}}"#,
        ),
        Response::error("/customer/queryCard", 500),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Old#1000", 101, "old-player-key");

    let result = bind_detected_target_with_base_url(
        &credentials(202),
        "New#2000",
        &target_path,
        &server.base_url,
    )
    .await;

    assert!(result.is_err());
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.input_bnet, "Old#1000");
    assert_eq!(target.customer_token, "old-player-key");
}

#[tokio::test]
async fn query_card_business_failure_does_not_replace_the_existing_target() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"new-player-key","bnetId":"New#2000"}}"#,
        ),
        Response::ok(
            "/customer/queryCard",
            r#"{"code":401,"message":"synthetic business rejection"}"#,
        ),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Old#1000", 101, "old-player-key");

    let result = bind_detected_target_with_base_url(
        &credentials(202),
        "New#2000",
        &target_path,
        &server.base_url,
    )
    .await;

    assert!(result.is_err());
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.input_bnet, "Old#1000");
    assert_eq!(target.customer_token, "old-player-key");
}

#[tokio::test]
async fn search_business_failure_does_not_continue_to_save_a_target() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":403,"data":{"customerToken":"new-player-key","bnetId":"New#2000"}}"#,
        ),
        Response::ok("/customer/queryCard", r#"{"code":0,"data":{}}"#),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Old#1000", 101, "old-player-key");

    let result = bind_detected_target_with_base_url(
        &credentials(202),
        "New#2000",
        &target_path,
        &server.base_url,
    )
    .await;

    assert!(result.is_err());
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.input_bnet, "Old#1000");
    assert_eq!(target.customer_token, "old-player-key");
}

fn credentials(role_id: i64) -> DashenCredentials {
    DashenCredentials {
        role_id,
        token: "synthetic-account-secret".to_string(),
        customer_token: None,
        dts: Some("2026".to_string()),
        server: 1,
        saved_at: "2026-07-14T00:00:00Z".to_string(),
    }
}

fn save_target(path: &std::path::Path, bnet: &str, role_id: i64, customer_token: &str) {
    save_player_target_for_path(
        path,
        &PlayerTarget {
            input_bnet: bnet.to_string(),
            resolved_bnet: Some(bnet.to_string()),
            customer_token: customer_token.to_string(),
            bound_at: "2026-07-14T00:00:00Z".to_string(),
            source_role_id: Some(role_id),
        },
    )
    .unwrap();
}

struct SequenceServer {
    base_url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

struct Response {
    path: &'static str,
    status: u16,
    body: &'static str,
}

impl Response {
    fn ok(path: &'static str, body: &'static str) -> Self {
        Self {
            path,
            status: 200,
            body,
        }
    }

    fn error(path: &'static str, status: u16) -> Self {
        Self {
            path,
            status,
            body: r#"{"code":500,"message":"synthetic failure"}"#,
        }
    }
}

fn spawn_sequence_server(responses: Vec<Response>) -> SequenceServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let request_bytes = read_complete_http_request(&mut stream);
            let request = String::from_utf8_lossy(&request_bytes);
            let first_line = request.lines().next().unwrap_or_default().to_string();
            assert!(
                first_line.contains(response.path),
                "expected path {}, got {first_line}",
                response.path
            );
            captured.lock().unwrap().push(first_line);
            let reason = if response.status == 200 {
                "OK"
            } else {
                "Synthetic Error"
            };
            let wire = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.status,
                reason,
                response.body.len(),
                response.body
            );
            stream.write_all(wire.as_bytes()).unwrap();
        }
    });
    SequenceServer {
        base_url: format!("http://{address}"),
        requests,
    }
}

fn read_complete_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    loop {
        let mut buffer = [0_u8; 4096];
        let read = stream.read(&mut buffer).unwrap();
        assert!(read > 0, "connection closed before request was complete");
        request.extend_from_slice(&buffer[..read]);

        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            return request;
        }
    }
}
