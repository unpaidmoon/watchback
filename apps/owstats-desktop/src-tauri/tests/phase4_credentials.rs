use owstats_desktop_lib::credentials::{
    credential_status_for_path, redacted_login_error, save_credentials_atomic, select_browser,
    BrowserCandidate, DashenCredentials,
};

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
fn login_error_message_redacts_token_like_values() {
    let message = redacted_login_error("token=abc customerToken=def cookie=ghi");
    assert!(!message.contains("abc"));
    assert!(!message.contains("def"));
    assert!(!message.contains("ghi"));
}
