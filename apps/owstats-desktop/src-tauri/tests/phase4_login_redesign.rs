use owstats_desktop_lib::browser_session::{build_browser_command, choose_debug_port};
use owstats_desktop_lib::credential_store::{
    credential_status_for_path, redact_sensitive_text, save_credentials_atomic, DashenCredentials,
};
use owstats_desktop_lib::dashen_auth::{
    build_collect_candidates_script, build_get_report_token_script, credentials_from_report_result,
    extract_battle_tag, extract_role_id, is_signature_helper_missing, BattleTagExtraction,
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
    let candidates = vec![
        "Other#9999\n战网昵称： 风偏偏呢#5479\n更多页面文本".to_string(),
        "Another#1234".to_string(),
    ];

    assert_eq!(
        extract_battle_tag(&candidates),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
    );
}

#[test]
fn battletag_extraction_accepts_chinese_and_ascii_colons() {
    assert_eq!(
        extract_battle_tag(&["  战网昵称： 风偏偏呢#5479  ".to_string()]),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
    );
    assert_eq!(
        extract_battle_tag(&["\n战网昵称: Tracer#12345\n".to_string()]),
        BattleTagExtraction::Found("Tracer#12345".to_string())
    );
}

#[test]
fn battletag_extraction_reports_conflicting_labeled_values() {
    let candidates = vec![
        "战网昵称： 风偏偏呢#5479".to_string(),
        "战网昵称: Other#9999".to_string(),
    ];

    assert_eq!(
        extract_battle_tag(&candidates),
        BattleTagExtraction::Conflict
    );
}

#[test]
fn battletag_extraction_returns_missing_without_a_valid_labeled_value() {
    let candidates = vec![
        "Other#9999".to_string(),
        "说明 战网昵称： Inline#1234".to_string(),
        "战网昵称：".to_string(),
        "战网昵称: MissingHash".to_string(),
        "战网昵称： Invalid#12A".to_string(),
    ];

    assert_eq!(
        extract_battle_tag(&candidates),
        BattleTagExtraction::Missing
    );
}

#[test]
fn battletag_extraction_deduplicates_identical_labeled_values() {
    let candidates = vec![
        "战网昵称： 风偏偏呢#5479".to_string(),
        "战网昵称: 风偏偏呢#5479".to_string(),
        "战网昵称：风偏偏呢#5479".to_string(),
    ];

    assert_eq!(
        extract_battle_tag(&candidates),
        BattleTagExtraction::Found("风偏偏呢#5479".to_string())
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

    assert!(script.contains("location.href"));
    assert!(script.contains("document.documentElement"));
    assert!(script.contains("performance.getEntriesByType"));
    assert!(script.contains("localStorage"));
    assert!(script.contains("sessionStorage"));
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
