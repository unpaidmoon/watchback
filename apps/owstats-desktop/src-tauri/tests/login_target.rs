use owstats_desktop_lib::credential_store::DashenCredentials;
use owstats_desktop_lib::login_target::{
    bind_detected_target_with_base_url, confirm_switch, decide_target_action,
    ensure_sync_target_ready, reconcile_detected_target, retry_binding, LoginTargetCoordinator,
    TargetAction, SYNC_TARGET_CONFIRMATION_ERROR,
};
use owstats_desktop_lib::player_target::{
    clear_player_target_for_path, read_player_target_for_path, save_player_target_for_path,
    target_matches_login_role, PlayerTarget,
};
use owstats_desktop_lib::{CredentialLoginResult, CredentialStatus, LoginTargetStatus};
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

#[test]
fn login_result_exposes_credential_and_target_status_without_secrets() {
    let result = CredentialLoginResult {
        status: CredentialStatus {
            state: "ready".to_string(),
            message: "已登录".to_string(),
            exposes_secret: false,
            role_id: None,
            saved_at: None,
        },
        target: LoginTargetStatus {
            state: "ready".to_string(),
            detected_bnet: Some("Current#1000".to_string()),
            current_bnet: Some("Current#1000".to_string()),
            message: "已绑定".to_string(),
            sync_allowed: true,
            can_confirm_switch: false,
            can_retry: false,
        },
    };

    let json = serde_json::to_string(&result).unwrap();

    assert!(json.contains("\"target\""));
    for forbidden in ["roleId", "role_id", "token", "customerToken"] {
        assert!(!json
            .to_ascii_lowercase()
            .contains(&forbidden.to_ascii_lowercase()));
    }
}

#[test]
fn persisted_target_from_current_login_role_allows_sync_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "synthetic-player-key");

    let status = ensure_sync_target_ready(
        &LoginTargetCoordinator::default(),
        &credentials(101),
        &target_path,
    )
    .unwrap();

    assert_eq!(status.state, "ready");
    assert!(status.sync_allowed);
}

#[test]
fn target_from_another_login_role_is_rejected_before_sync() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Other#2000", 202, "synthetic-player-key");

    let error = ensure_sync_target_ready(
        &LoginTargetCoordinator::default(),
        &credentials(101),
        &target_path,
    )
    .unwrap_err()
    .to_string();

    assert_eq!(error, SYNC_TARGET_CONFIRMATION_ERROR);
    for secret in ["101", "202", "Other#2000", "synthetic-player-key", "token"] {
        assert!(!error.contains(secret));
    }
}

#[test]
fn legacy_target_without_source_role_is_rejected_before_sync() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    let legacy = PlayerTarget {
        input_bnet: "Legacy#1000".to_string(),
        resolved_bnet: Some("Legacy#1000".to_string()),
        customer_token: "synthetic-player-key".to_string(),
        bound_at: "2026-07-14T00:00:00Z".to_string(),
        source_role_id: None,
    };
    save_player_target_for_path(&target_path, &legacy).unwrap();

    let error = ensure_sync_target_ready(
        &LoginTargetCoordinator::default(),
        &credentials(101),
        &target_path,
    )
    .unwrap_err()
    .to_string();

    assert_eq!(error, SYNC_TARGET_CONFIRMATION_ERROR);
}

#[test]
fn non_ready_login_session_rejects_sync_even_when_persisted_target_matches() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "synthetic-player-key");
    let coordinator = LoginTargetCoordinator::default();
    coordinator.replace_unidentified(101);

    let error = ensure_sync_target_ready(&coordinator, &credentials(101), &target_path)
        .unwrap_err()
        .to_string();

    assert_eq!(error, SYNC_TARGET_CONFIRMATION_ERROR);
}

#[test]
fn clearing_a_target_keeps_retry_only_for_a_reliable_current_session_tag() {
    let coordinator = LoginTargetCoordinator::default();
    coordinator.replace_ready(101, "Current#1000", "Current#1000");

    let retryable = coordinator.target_cleared(Some(101));

    assert_eq!(retryable.state, "bindFailed");
    assert!(retryable.can_retry);
    assert_eq!(retryable.detected_bnet.as_deref(), Some("Current#1000"));

    coordinator.clear();
    let unidentified = coordinator.target_cleared(Some(101));
    assert_eq!(unidentified.state, "unidentified");
    assert!(!unidentified.can_retry);
    assert_eq!(unidentified.detected_bnet, None);

    coordinator.replace_ready(101, "Current#1000", "Current#1000");
    let logged_out = coordinator.target_cleared(None);
    assert_eq!(logged_out.state, "unidentified");
    assert!(!logged_out.can_retry);
    assert_eq!(logged_out.detected_bnet, None);
}

#[tokio::test]
async fn persisted_ready_target_never_becomes_a_retryable_detected_identity() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(
        &target_path,
        "Injected#9999",
        101,
        "synthetic-persisted-key",
    );
    let coordinator = LoginTargetCoordinator::default();
    let credentials = credentials(101);

    let restored = coordinator.status_or_persisted(Some(&credentials), &target_path);
    let repeated = coordinator.status_or_persisted(Some(&credentials), &target_path);
    clear_player_target_for_path(&target_path).unwrap();
    let cleared = coordinator.target_cleared(Some(101));
    let server = spawn_request_counting_server();
    let retry = retry_binding(&coordinator, &credentials, &target_path, &server.base_url).await;
    let requests = server.requests.lock().unwrap().clone();
    server.join();

    assert_eq!(restored.state, "ready");
    assert!(restored.sync_allowed);
    assert_eq!(restored.current_bnet.as_deref(), Some("Injected#9999"));
    assert_eq!(restored.detected_bnet, None);
    assert_eq!(repeated.state, "ready");
    assert_eq!(repeated.detected_bnet, None);
    assert_eq!(cleared.state, "unidentified");
    assert!(!cleared.can_retry);
    assert_eq!(cleared.detected_bnet, None);
    assert_eq!(retry.state, "verificationRequired");
    assert!(
        requests.is_empty(),
        "persisted BattleTag must never be retried"
    );
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
    server.join();
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
    server.join();
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
    server.join();
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
    server.join();
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

#[test]
fn in_memory_ready_status_is_not_reused_for_missing_or_different_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");
    let coordinator = LoginTargetCoordinator::default();
    coordinator.replace_ready(101, "Current#1000", "Current#1000");

    let wrong_role = coordinator.status_or_persisted(Some(&credentials(202)), &target_path);
    assert_eq!(wrong_role.state, "verificationRequired");
    assert!(!wrong_role.sync_allowed);

    coordinator.replace_ready(101, "Current#1000", "Current#1000");
    let missing_credentials = coordinator.status_or_persisted(None, &target_path);
    assert_eq!(missing_credentials.state, "verificationRequired");
    assert!(!missing_credentials.sync_allowed);
}

#[tokio::test]
async fn stale_switch_request_cannot_commit_after_a_new_login_round_starts() {
    let mut server = spawn_blocking_query_server("Second#2000", "second-player-key");
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Current#1000", 101, "old-player-key");
    let coordinator = LoginTargetCoordinator::default();
    coordinator.replace_switch_required(202, "Second#2000", "Current#1000");

    let old_coordinator = coordinator.clone();
    let old_target_path = target_path.clone();
    let old_base_url = server.base_url.clone();
    let old_request = tokio::spawn(async move {
        confirm_switch(
            &old_coordinator,
            &credentials(202),
            &old_target_path,
            &old_base_url,
        )
        .await
    });
    server.wait_until_query().await;

    coordinator.replace_unidentified(303);
    server.release();
    let stale_result = old_request.await.unwrap();
    server.join();

    assert_eq!(stale_result.state, "verificationRequired");
    assert!(!stale_result.sync_allowed);
    let current_session = coordinator.status().unwrap();
    assert_eq!(current_session.state, "unidentified");
    assert_eq!(current_session.detected_bnet, None);
    let persisted = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(persisted.input_bnet, "Current#1000");
    assert_eq!(persisted.source_role_id, Some(101));
    assert_eq!(persisted.customer_token, "old-player-key");
}

#[tokio::test]
async fn concurrent_direct_binding_uses_distinct_atomic_temp_files() {
    let mut first_server = spawn_blocking_query_server("First#1000", "first-player-key");
    let mut second_server = spawn_blocking_query_server("Second#2000", "second-player-key");
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");

    let first_path = target_path.clone();
    let first_url = first_server.base_url.clone();
    let first = tokio::spawn(async move {
        bind_detected_target_with_base_url(&credentials(101), "First#1000", &first_path, &first_url)
            .await
    });
    let second_path = target_path.clone();
    let second_url = second_server.base_url.clone();
    let second = tokio::spawn(async move {
        bind_detected_target_with_base_url(
            &credentials(202),
            "Second#2000",
            &second_path,
            &second_url,
        )
        .await
    });
    first_server.wait_until_query().await;
    second_server.wait_until_query().await;
    first_server.release();
    second_server.release();

    assert!(first.await.unwrap().is_ok());
    assert!(second.await.unwrap().is_ok());
    first_server.join();
    second_server.join();
    let target = read_player_target_for_path(&target_path).unwrap();
    assert!(matches!(
        target.input_bnet.as_str(),
        "First#1000" | "Second#2000"
    ));
    let temp_files = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(temp_files, 0);
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
    server.join();
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
    server.join();
}

#[tokio::test]
async fn search_business_failure_does_not_continue_to_save_a_target() {
    let server = spawn_sequence_server(vec![Response::ok(
        "/searchBnetAccount",
        r#"{"code":403,"data":{"customerToken":"new-player-key","bnetId":"New#2000"}}"#,
    )]);
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
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    server.join();
}

#[tokio::test]
async fn search_identity_mismatch_fails_closed_without_querying_or_overwriting() {
    let server = spawn_sequence_server(vec![Response::ok(
        "/searchBnetAccount",
        r#"{"code":0,"data":{"customerToken":"new-player-key","bnetId":"Other#9999"}}"#,
    )]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Old#1000", 101, "old-player-key");

    let result = bind_detected_target_with_base_url(
        &credentials(202),
        "Current#1000",
        &target_path,
        &server.base_url,
    )
    .await;

    let error = result.unwrap_err().to_string();
    assert!(error.contains("搜索结果与识别账号不一致"));
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.input_bnet, "Old#1000");
    assert_eq!(target.customer_token, "old-player-key");
    server.join();
}

#[tokio::test]
async fn query_card_identity_mismatch_fails_closed_without_overwriting() {
    let server = spawn_sequence_server(vec![
        Response::ok(
            "/searchBnetAccount",
            r#"{"code":0,"data":{"customerToken":"new-player-key","bnetId":"Current#1000"}}"#,
        ),
        Response::ok(
            "/customer/queryCard",
            r#"{"code":0,"data":{"bnetId":"Other#9999"}}"#,
        ),
    ]);
    let temp = tempfile::tempdir().unwrap();
    let target_path = temp.path().join("target.json");
    save_target(&target_path, "Old#1000", 101, "old-player-key");

    let result = bind_detected_target_with_base_url(
        &credentials(202),
        "Current#1000",
        &target_path,
        &server.base_url,
    )
    .await;

    assert!(result.is_err());
    let target = read_player_target_for_path(&target_path).unwrap();
    assert_eq!(target.input_bnet, "Old#1000");
    assert_eq!(target.customer_token, "old-player-key");
    server.join();
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
    handle: Option<thread::JoinHandle<()>>,
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
                stream.set_nonblocking(false).unwrap();
                let request = read_complete_http_request(&mut stream);
                let first_line = String::from_utf8_lossy(&request)
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                captured.lock().unwrap().push(first_line);
                stream
                    .write_all(
                        b"HTTP/1.1 500 Synthetic Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
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

impl SequenceServer {
    fn join(mut self) {
        self.handle.take().unwrap().join().unwrap();
    }
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
    let handle = thread::spawn(move || {
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
        handle: Some(handle),
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

struct BlockingQueryServer {
    base_url: String,
    query_started: Option<tokio::sync::oneshot::Receiver<()>>,
    release_query: Option<std::sync::mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl BlockingQueryServer {
    async fn wait_until_query(&mut self) {
        self.query_started.take().unwrap().await.unwrap();
    }

    fn release(&mut self) {
        self.release_query.take().unwrap().send(()).unwrap();
    }

    fn join(mut self) {
        self.handle.take().unwrap().join().unwrap();
    }
}

fn spawn_blocking_query_server(
    bnet: &'static str,
    customer_token: &'static str,
) -> BlockingQueryServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (query_started_tx, query_started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut search, _) = listener.accept().unwrap();
        let request = read_complete_http_request(&mut search);
        assert!(String::from_utf8_lossy(&request).starts_with("POST /searchBnetAccount "));
        write_json_response(
            &mut search,
            &format!(
                r#"{{"code":0,"data":{{"customerToken":"{customer_token}","bnetId":"{bnet}"}}}}"#
            ),
        );

        let (mut query, _) = listener.accept().unwrap();
        let request = read_complete_http_request(&mut query);
        assert!(String::from_utf8_lossy(&request).starts_with("GET /customer/queryCard?"));
        query_started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        write_json_response(
            &mut query,
            &format!(r#"{{"code":0,"data":{{"bnetId":"{bnet}"}}}}"#),
        );
    });
    BlockingQueryServer {
        base_url: format!("http://{address}"),
        query_started: Some(query_started_rx),
        release_query: Some(release_tx),
        handle: Some(handle),
    }
}

fn write_json_response(stream: &mut std::net::TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).unwrap();
}
