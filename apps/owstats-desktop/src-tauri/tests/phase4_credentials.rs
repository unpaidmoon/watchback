use owstats_desktop_lib::credential_store::save_credentials_atomic;
use owstats_desktop_lib::credentials::{
    credential_status_for_path, select_browser, BrowserCandidate, DashenCredentials,
};
use owstats_desktop_lib::login_flow::{LoginPhase, LoginProgress};
use owstats_desktop_lib::AccountLoginResult;

#[test]
fn credential_status_accepts_saved_login_without_bound_player_target() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(".dashen_credentials.json");
    std::fs::write(
        &path,
        r#"{"role_id":114514191,"token":"secret-token","server":1,"dts":null,"saved_at":"2026-06-27T00:00:00Z"}"#,
    )
    .unwrap();

    let status = credential_status_for_path(&path).unwrap();

    assert_eq!(status.state, "ready");
    assert_eq!(status.role_id, Some(114514191));
    assert_eq!(status.saved_at.as_deref(), Some("2026-06-27T00:00:00Z"));
    assert!(!status.message.contains("secret-token"));
    assert!(!status.exposes_secret);
}

#[test]
fn save_credentials_writes_atomically_and_redacts_status() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(".dashen_credentials.json");
    let credentials = DashenCredentials {
        role_id: 42,
        token: "token-value".to_string(),
        customer_token: Some("customer-token-value".to_string()),
        dts: None,
        server: 1,
        saved_at: "2026-06-27T00:00:00Z".to_string(),
    };

    save_credentials_atomic(&path, &credentials).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("token-value"));
    assert!(text.contains("customerToken"));
    assert!(text.contains("customer-token-value"));

    let status = credential_status_for_path(&path).unwrap();
    assert_eq!(status.state, "ready");
    assert!(!status.message.contains("token-value"));
    assert!(!status.message.contains("customer-token-value"));
}

#[test]
fn browser_selection_prefers_chrome_then_edge() {
    let candidates = vec![
        BrowserCandidate::edge("C:\\Edge\\msedge.exe"),
        BrowserCandidate::chrome("C:\\Chrome\\chrome.exe"),
    ];
    let selected = select_browser(candidates).unwrap();
    assert_eq!(selected.kind, "chrome");
}

#[test]
fn login_progress_preserves_error_text_without_adding_fields() {
    let source = "token=abc customerToken=def cookie=ghi";
    let progress = LoginProgress::new(LoginPhase::TokenPending, source);
    let value = serde_json::to_value(progress).unwrap();

    assert_eq!(value["message"], source);
    assert_eq!(value.as_object().unwrap().len(), 2);
    assert!(value.get("token").is_none());
    assert!(value.get("customerToken").is_none());
    assert!(value.get("credential").is_none());
    assert!(value.get("roleId").is_none());
}

#[test]
fn account_login_result_schema_has_no_credential_fields() {
    let value = serde_json::to_value(AccountLoginResult {
        kind: "newAccountAdded".to_string(),
        account_id: Some("synthetic-account".to_string()),
        battle_tag: Some("Synthetic#1000".to_string()),
        message: "saved".to_string(),
        should_offer_switch: false,
        warnings: Vec::new(),
    })
    .unwrap();

    assert_eq!(value.as_object().unwrap().len(), 5);
    for field in ["token", "customerToken", "credential", "roleId"] {
        assert!(
            value.get(field).is_none(),
            "login result must not expose {field}"
        );
    }
}
