use owstats_desktop_lib::browser_session::{build_browser_command, choose_debug_port};
use owstats_desktop_lib::credential_store::{
    credential_status_for_path, redact_sensitive_text, save_credentials_atomic, DashenCredentials,
};
use owstats_desktop_lib::credentials::{
    LoginCaptureIdentity, LoginIdentityTracker, PairedIdentityStatus,
};
use owstats_desktop_lib::dashen_auth::{
    build_collect_candidates_script, build_get_report_token_script, credentials_from_report_result,
    extract_battle_tag, extract_login_role_id, extract_role_id, is_signature_helper_missing,
    BattleTagExtraction, RoleIdExtraction,
};
use owstats_desktop_lib::login_flow::{LoginPhase, LoginProgress};

#[test]
fn login_progress_never_exposes_sensitive_values() {
    let progress = LoginProgress::new(
        LoginPhase::TokenPending,
        "token=abc customerToken=def cookie=ghi role_id=123456",
    );

    assert_eq!(progress.phase, LoginPhase::TokenPending);
    assert!(!progress.message.contains("abc"));
    assert!(!progress.message.contains("def"));
    assert!(!progress.message.contains("ghi"));
    assert!(progress.message.contains("role_id=123456"));
}

#[test]
fn browser_command_uses_dedicated_profile_and_debug_endpoint() {
    let command = build_browser_command(
        "C:\\Chrome\\chrome.exe",
        "https://ds.163.com/",
        9233,
        Some(std::path::Path::new("D:\\owstats-profile")),
    );

    assert_eq!(command[0], "C:\\Chrome\\chrome.exe");
    assert!(command.contains(&"--remote-debugging-port=9233".to_string()));
    assert!(command.contains(&"--remote-allow-origins=http://127.0.0.1:9233".to_string()));
    assert!(command.contains(&"--user-data-dir=D:\\owstats-profile".to_string()));
    assert_eq!(
        command.last().map(String::as_str),
        Some("https://ds.163.com/")
    );
}

#[test]
fn debug_port_is_stable_per_process_input() {
    let port = choose_debug_port(3440);
    assert!((9300..=9499).contains(&port));
    assert_eq!(port, choose_debug_port(3440));
}

#[test]
fn extracts_role_id_from_url_json_and_text_candidates() {
    assert_eq!(
        extract_role_id(&[
            "https://pay.ds.163.com/activity?role_id=123456".to_string(),
            "ignored".to_string(),
        ]),
        Some("123456".to_string())
    );
    assert_eq!(
        extract_role_id(&[r#"{"roleId":"987654321"}"#.to_string()]),
        Some("987654321".to_string())
    );
    assert_eq!(
        extract_role_id(&["页面文本 roleid: 555555".to_string()]),
        Some("555555".to_string())
    );
    assert_eq!(extract_role_id(&["no identifier".to_string()]), None);
}

#[test]
fn battletag_extraction_uses_only_the_labeled_page_line() {
    let page_text = "Other#9999\n战网昵称： 风偏偏呢#5479\nAnother#1234\n更多页面文本";

    assert_eq!(
        extract_battle_tag(page_text),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
    );
}

#[test]
fn battletag_extraction_accepts_unicode_whitespace_around_both_colons() {
    assert_eq!(
        extract_battle_tag("  战网昵称　：　风偏偏呢#5479  "),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
    );
    assert_eq!(
        extract_battle_tag("\n战网昵称\t:\tTracer#12345\n"),
        BattleTagExtraction::Found("Tracer#12345".to_string())
    );
}

#[test]
fn battletag_extraction_reports_conflicting_labeled_values() {
    let page_text = "战网昵称： 风偏偏呢#5479\n战网昵称: Other#9999";

    assert_eq!(extract_battle_tag(page_text), BattleTagExtraction::Conflict);
}

#[test]
fn battletag_extraction_returns_missing_without_a_valid_labeled_value() {
    let page_text = concat!(
        "Other#9999\n",
        "说明 战网昵称： Inline#1234\n",
        "战网昵称：\n",
        "战网昵称: MissingHash\n",
        "战网昵称： Invalid#12A\n",
        "战网昵称： #1234\n",
        "战网昵称： Foo#Bar#1234\n",
        "战网昵称： Foo##1234\n",
    );

    assert_eq!(extract_battle_tag(page_text), BattleTagExtraction::Missing);
}

#[test]
fn battletag_extraction_deduplicates_identical_labeled_values() {
    let page_text = "战网昵称： 风偏偏呢#5479\n战网昵称: 风偏偏呢#5479\n战网昵称：风偏偏呢#5479";

    assert_eq!(
        extract_battle_tag(page_text),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
    );
}

#[test]
fn battletag_extraction_reads_only_the_first_token_after_the_colon() {
    assert_eq!(
        extract_battle_tag("战网昵称： Player#1234 切换"),
        BattleTagExtraction::Found("Player#1234".to_string())
    );
}

#[test]
fn token_script_builds_signed_request_without_embedding_existing_secret() {
    let script = build_get_report_token_script("123456", 1);

    assert!(script.contains("getReportToken"));
    assert!(script.contains("\"roleId\":\"123456\""));
    assert!(script.contains("window.sig.default"));
    assert!(script.contains("GL-CheckSum"));
    assert!(!script.contains("findCustomerTokenFromCustomerRequests"));
    assert!(!script.contains("performance.getEntriesByType('resource')"));
}

#[test]
fn collect_script_reads_page_storage_and_resource_urls() {
    let script = build_collect_candidates_script();

    assert!(script.contains("pageText:"));
    assert!(script.contains("roleCandidates:"));
    assert!(script.contains("current:"));
    assert!(script.contains("storage:"));
    assert!(script.contains("resources:"));
    assert!(script.contains("truncated:"));
    assert!(script.contains("MAX_ROLE_EVIDENCE_VALUES"));
    assert!(script.contains("group.truncated = true"));
    assert!(!script.contains("slice(0, 500)"));
    assert!(script.contains("location.href"));
    assert!(script.contains("document.documentElement"));
    assert!(script.contains("performance.getEntriesByType"));
    assert!(script.contains("localStorage"));
    assert!(script.contains("sessionStorage"));
}

#[test]
fn login_role_rejects_conflicting_storage_and_resource_without_current_evidence() {
    let current = vec![
        "https://ds.163.com/".to_string(),
        "战网昵称：Second#2222".to_string(),
    ];
    let storage = vec![r#"{"roleId":"101101"}"#.to_string()];
    let resources = vec!["https://new.example/?role_id=202202".to_string()];

    assert_eq!(
        extract_login_role_id(&current, &storage, &resources, false),
        RoleIdExtraction::Conflict
    );
}

#[test]
fn login_role_rejects_auxiliary_only_evidence_even_when_it_agrees() {
    let current = vec![
        "https://ds.163.com/".to_string(),
        "战网昵称：Second#2222".to_string(),
    ];
    let storage = vec![r#"{"roleId":"101101"}"#.to_string()];
    let resources = vec!["https://old.example/?role_id=101101".to_string()];

    assert_eq!(
        extract_login_role_id(&current, &storage, &resources, false),
        RoleIdExtraction::Missing
    );
}

#[test]
fn login_role_rejects_auxiliary_role_that_conflicts_with_current_role() {
    let current = vec![
        "https://ds.163.com/?role_id=202202".to_string(),
        "战网昵称：Second#2222".to_string(),
    ];
    let storage = vec![r#"{"roleId":"101101"}"#.to_string()];

    assert_eq!(
        extract_login_role_id(&current, &storage, &[], false),
        RoleIdExtraction::Conflict
    );
}

#[test]
fn login_role_rejects_conflicting_roles_within_current_page_evidence() {
    let current = vec![
        "https://ds.163.com/".to_string(),
        "role_id=101101 role_id=202202 战网昵称：Second#2222".to_string(),
    ];

    assert_eq!(
        extract_login_role_id(&current, &[], &[], false),
        RoleIdExtraction::Conflict
    );
}

#[test]
fn login_role_accepts_current_role_when_auxiliary_evidence_agrees() {
    let current = vec![
        "https://ds.163.com/?role_id=202202".to_string(),
        "战网昵称：Second#2222".to_string(),
    ];
    let storage = vec![r#"{"roleId":"202202"}"#.to_string()];

    assert_eq!(
        extract_login_role_id(&current, &storage, &[], false),
        RoleIdExtraction::Found("202202".to_string())
    );
}

#[test]
fn login_role_rejects_conflict_hidden_after_bounded_auxiliary_evidence() {
    let current = vec!["https://ds.163.com/?role_id=101101".to_string()];
    let mut all_storage = vec![r#"{"roleId":"101101"}"#.to_string(); 500];
    all_storage.push(r#"{"roleId":"202202"}"#.to_string());
    let returned_storage = all_storage[..500].to_vec();

    assert_eq!(
        extract_login_role_id(&current, &returned_storage, &[], true),
        RoleIdExtraction::Conflict
    );
}

#[test]
fn login_capture_keeps_storage_credential_role_when_battletag_is_missing() {
    let current = vec!["https://ds.163.com/".to_string()];
    let storage = vec![r#"{"roleId":"101101"}"#.to_string()];
    let credential_role_id = extract_role_id(&storage);
    let paired_role_id = extract_login_role_id(&current, &storage, &[], false);
    let mut identity = LoginCaptureIdentity::default();

    identity.observe(
        credential_role_id,
        paired_role_id,
        BattleTagExtraction::Missing,
    );

    assert_eq!(identity.credential_role_id(), Some("101101"));
    assert_eq!(identity.paired_status(), PairedIdentityStatus::Unavailable);
    assert_eq!(identity.detected_battle_tag("101101"), None);
}

#[test]
fn login_capture_keeps_resource_credential_role_when_battletag_conflicts() {
    let current = vec!["https://ds.163.com/".to_string()];
    let resources = vec!["https://api.example/?role_id=202202".to_string()];
    let credential_role_id = extract_role_id(&resources);
    let paired_role_id = extract_login_role_id(&current, &[], &resources, false);
    let mut identity = LoginCaptureIdentity::default();

    identity.observe(
        credential_role_id,
        paired_role_id,
        BattleTagExtraction::Conflict,
    );

    assert_eq!(identity.credential_role_id(), Some("202202"));
    assert_eq!(identity.paired_status(), PairedIdentityStatus::Unavailable);
    assert_eq!(identity.detected_battle_tag("202202"), None);
}

#[test]
fn login_capture_confirms_pending_pair_before_immediate_token_success() {
    let mut identity = LoginCaptureIdentity::default();
    identity.observe(
        Some("101101".to_string()),
        RoleIdExtraction::Found("101101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    assert_eq!(identity.paired_status(), PairedIdentityStatus::Pending);

    identity.confirm_pending_pair(
        RoleIdExtraction::Found("101101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    assert_eq!(
        identity.paired_status(),
        PairedIdentityStatus::Confirmed {
            role_id: "101101".to_string(),
            battle_tag: "First#1111".to_string(),
        }
    );
    assert_eq!(identity.detected_battle_tag("101101"), Some("First#1111"));
}

#[test]
fn login_capture_confirms_pending_pair_before_signature_lock() {
    let mut identity = LoginCaptureIdentity::default();
    identity.observe(
        Some("101101".to_string()),
        RoleIdExtraction::Found("101101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    assert_eq!(identity.paired_status(), PairedIdentityStatus::Pending);
    identity.confirm_pending_pair(
        RoleIdExtraction::Found("101101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );
    let locked_pair = identity
        .confirmed_pair()
        .map(|(role_id, battle_tag)| (role_id.to_string(), battle_tag.to_string()));

    assert_eq!(
        locked_pair,
        Some(("101101".to_string(), "First#1111".to_string()))
    );
}

#[test]
fn login_capture_drops_changed_pending_pair_without_losing_credential_role() {
    let mut identity = LoginCaptureIdentity::default();
    identity.observe(
        Some("101101".to_string()),
        RoleIdExtraction::Found("101101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    identity.confirm_pending_pair(
        RoleIdExtraction::Found("202202".to_string()),
        BattleTagExtraction::Found("Second#2222".to_string()),
    );

    assert_eq!(identity.credential_role_id(), Some("101101"));
    assert_eq!(identity.paired_status(), PairedIdentityStatus::Unavailable);
    assert_eq!(identity.detected_battle_tag("101101"), None);
}

#[test]
fn login_capture_drops_failed_or_conflicting_confirmation_without_losing_credential_role() {
    let confirmations = [
        (RoleIdExtraction::Missing, BattleTagExtraction::Missing),
        (
            RoleIdExtraction::Found("101101".to_string()),
            BattleTagExtraction::Conflict,
        ),
    ];

    for (paired_role_id, battle_tag) in confirmations {
        let mut identity = LoginCaptureIdentity::default();
        identity.observe(
            Some("101101".to_string()),
            RoleIdExtraction::Found("101101".to_string()),
            BattleTagExtraction::Found("First#1111".to_string()),
        );

        identity.confirm_pending_pair(paired_role_id, battle_tag);

        assert_eq!(identity.credential_role_id(), Some("101101"));
        assert_eq!(identity.paired_status(), PairedIdentityStatus::Unavailable);
        assert_eq!(identity.detected_battle_tag("101101"), None);
    }
}

#[test]
fn login_identity_tracker_requires_two_stable_complete_observations() {
    let mut tracker = LoginIdentityTracker::default();

    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    assert_eq!(tracker.role_id(), Some("101"));
    assert_eq!(tracker.battle_tag(), Some("First#1111"));
    assert_eq!(tracker.complete(), None);

    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    assert_eq!(tracker.complete(), Some(("101", "First#1111")));
}

#[test]
fn login_identity_tracker_invalidates_unlocked_pair_on_missing_observation() {
    let mut tracker = LoginIdentityTracker::default();
    for _ in 0..2 {
        tracker.observe(
            Some("101".to_string()),
            BattleTagExtraction::Found("First#1111".to_string()),
        );
    }
    let locked = tracker
        .complete()
        .map(|(role_id, battle_tag)| (role_id.to_string(), battle_tag.to_string()));

    tracker.observe(None, BattleTagExtraction::Missing);

    assert_eq!(tracker.complete(), None);
    assert_eq!(tracker.role_id(), None);
    assert_eq!(tracker.battle_tag(), None);
    assert_eq!(locked, Some(("101".to_string(), "First#1111".to_string())));
}

#[test]
fn login_identity_tracker_never_pairs_new_tag_with_old_role() {
    let mut tracker = LoginIdentityTracker::default();
    for _ in 0..2 {
        tracker.observe(
            Some("101".to_string()),
            BattleTagExtraction::Found("First#1111".to_string()),
        );
    }

    tracker.observe(None, BattleTagExtraction::Found("Second#2222".to_string()));

    assert_eq!(tracker.role_id(), None);
    assert_eq!(tracker.battle_tag(), None);
    assert_eq!(tracker.complete(), None);
}

#[test]
fn login_identity_tracker_clears_old_battletag_as_soon_as_role_changes() {
    let mut tracker = LoginIdentityTracker::default();
    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );
    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    tracker.observe(Some("202".to_string()), BattleTagExtraction::Missing);

    assert_eq!(tracker.role_id(), None);
    assert_eq!(tracker.battle_tag(), None);
    assert_eq!(tracker.complete(), None);
}

#[test]
fn login_identity_tracker_conflict_clears_current_battletag() {
    let mut tracker = LoginIdentityTracker::default();
    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );
    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    tracker.observe(None, BattleTagExtraction::Conflict);

    assert_eq!(tracker.role_id(), None);
    assert_eq!(tracker.battle_tag(), None);
    assert_eq!(tracker.complete(), None);
}

#[test]
fn login_identity_tracker_pairs_new_role_and_battletag_from_same_observation() {
    let mut tracker = LoginIdentityTracker::default();
    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );
    tracker.observe(
        Some("101".to_string()),
        BattleTagExtraction::Found("First#1111".to_string()),
    );

    tracker.observe(
        Some("202".to_string()),
        BattleTagExtraction::Found("Second#2222".to_string()),
    );

    assert_eq!(tracker.complete(), None);

    tracker.observe(
        Some("202".to_string()),
        BattleTagExtraction::Found("Second#2222".to_string()),
    );

    assert_eq!(tracker.complete(), Some(("202", "Second#2222")));
}

#[test]
fn login_identity_tracker_does_not_use_storage_candidate_as_battletag() {
    let page_text = "当前页面没有战网昵称标签";
    let candidates = vec![
        "https://ds.163.com/?role_id=101101".to_string(),
        "战网昵称：Other#9999".to_string(),
    ];
    let mut tracker = LoginIdentityTracker::default();

    tracker.observe(extract_role_id(&candidates), extract_battle_tag(page_text));

    assert_eq!(tracker.role_id(), None);
    assert_eq!(tracker.battle_tag(), None);
    assert_eq!(tracker.complete(), None);
}

#[test]
fn parses_successful_report_token_result_into_credentials() {
    let result = serde_json::json!({
        "ok": true,
        "role_id": "123456",
        "token": "secret-token"
    });

    let credentials = credentials_from_report_result(&result, "111111", 1, "2026-06-27T00:00:00Z")
        .unwrap()
        .unwrap();

    assert_eq!(credentials.role_id, 123456);
    assert_eq!(credentials.token, "secret-token");
    assert_eq!(credentials.customer_token, None);
    assert_eq!(credentials.server, 1);
    assert_eq!(credentials.saved_at, "2026-06-27T00:00:00Z");
}

#[test]
fn report_token_result_does_not_define_player_target() {
    let result = serde_json::json!({
        "ok": true,
        "roleId": "123456",
        "token": "secret-token",
        "customerToken": "customer-secret"
    });

    let credentials = credentials_from_report_result(&result, "111111", 1, "2026-06-27T00:00:00Z")
        .unwrap()
        .unwrap();

    assert_eq!(credentials.role_id, 123456);
    assert_eq!(credentials.customer_token, None);
}

#[test]
fn missing_signature_helper_is_detected_as_recoverable() {
    let result = serde_json::json!({
        "ok": false,
        "error": "window.sig.default is not available on this page yet"
    });

    assert!(is_signature_helper_missing(&result));
    assert!(!is_signature_helper_missing(
        &serde_json::json!({"error": "token missing"})
    ));
}

#[test]
fn credential_status_and_progress_redact_secrets() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(".dashen_credentials.json");
    save_credentials_atomic(
        &path,
        &DashenCredentials {
            role_id: 123456,
            token: "secret-token".to_string(),
            customer_token: Some("customer-secret".to_string()),
            dts: None,
            server: 1,
            saved_at: "2026-06-27T00:00:00Z".to_string(),
        },
    )
    .unwrap();

    let status = credential_status_for_path(&path).unwrap();
    assert_eq!(status.state, "ready");
    assert_eq!(status.role_id, Some(123456));
    assert!(!status.message.contains("secret-token"));
    assert!(!status.message.contains("customer-secret"));
    assert_eq!(
        redact_sensitive_text("token=secret-token cookie=abc customerToken=def"),
        "token=***REDACTED*** cookie=***REDACTED*** customerToken=***REDACTED***"
    );
}
