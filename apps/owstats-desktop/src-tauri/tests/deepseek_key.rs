use anyhow::Result;
use owstats_desktop_lib::deepseek_key::{
    build_http_client, normalize_base_url, validate_api_key, validate_models_status,
    DeepSeekCredentialStore, DeepSeekKeyManager, DEEPSEEK_REQUEST_TIMEOUT,
    DEFAULT_DEEPSEEK_BASE_URL,
};
use owstats_desktop_lib::DeepSeekKeyStatus;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Default)]
struct FakeStore {
    configured: AtomicBool,
    writes: AtomicUsize,
}

impl DeepSeekCredentialStore for FakeStore {
    fn exists(&self) -> Result<bool> {
        Ok(self.configured.load(Ordering::SeqCst))
    }

    fn write(&self, _api_key: &str) -> Result<()> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.configured.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn status_dto_is_limited_to_ready_or_missing_without_base_url() {
    for state in ["ready", "missing"] {
        let status = DeepSeekKeyStatus {
            state: state.to_string(),
            message: "synthetic status".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains(&format!("\"state\":\"{state}\"")));
        assert!(json.contains("\"message\""));
        for forbidden in ["baseUrl", "apiKey", "authorization", "header", "body"] {
            assert!(!json.contains(forbidden));
        }
    }
}

#[test]
fn status_codes_are_validation_errors_not_dto_states() {
    assert!(validate_models_status(200).is_ok());
    for status in [401, 402, 429, 500, 503, 418] {
        let message = validate_models_status(status).unwrap_err().to_string();
        for forbidden in [
            "invalid_key",
            "rate_limited",
            "service_unavailable",
            "synthetic-key",
            "authorization",
            "response-body",
            DEFAULT_DEEPSEEK_BASE_URL,
        ] {
            assert!(!message.contains(forbidden));
        }
    }
}

#[test]
fn normalizes_base_url_and_client_has_a_bounded_timeout() {
    assert_eq!(DEEPSEEK_REQUEST_TIMEOUT, Duration::from_secs(10));
    assert_eq!(
        normalize_base_url(" https://api.deepseek.com/ ").unwrap(),
        DEFAULT_DEEPSEEK_BASE_URL
    );
    assert_eq!(
        normalize_base_url("http://127.0.0.1:4010/v1/").unwrap(),
        "http://127.0.0.1:4010/v1"
    );
    assert!(normalize_base_url("file:///tmp/deepseek").is_err());
    assert!(normalize_base_url("https://user:secret@example.com").is_err());
    build_http_client().unwrap();
}

#[tokio::test]
async fn validates_with_get_models_bearer_header_and_empty_body() {
    let (base_url, server) = spawn_server(200).await;
    validate_api_key(&build_http_client().unwrap(), &base_url, "synthetic-key")
        .await
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn manager_reports_ready_or_missing_and_writes_only_after_200() {
    let missing_store = Arc::new(FakeStore::default());
    let missing_manager = DeepSeekKeyManager::with_store(
        build_http_client().unwrap(),
        DEFAULT_DEEPSEEK_BASE_URL,
        missing_store.clone(),
    )
    .unwrap();
    assert_eq!(missing_manager.status().unwrap().state, "missing");

    let (ok_base_url, ok_server) = spawn_server(200).await;
    let ok_manager = DeepSeekKeyManager::with_store(
        build_http_client().unwrap(),
        &ok_base_url,
        missing_store.clone(),
    )
    .unwrap();
    assert_eq!(
        ok_manager.save("synthetic-key").await.unwrap().state,
        "ready"
    );
    assert_eq!(missing_store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(ok_manager.status().unwrap().state, "ready");
    ok_server.await.unwrap();

    let rejected_store = Arc::new(FakeStore::default());
    let (rejected_base_url, rejected_server) = spawn_server(401).await;
    let rejected_manager = DeepSeekKeyManager::with_store(
        build_http_client().unwrap(),
        &rejected_base_url,
        rejected_store.clone(),
    )
    .unwrap();
    assert!(rejected_manager.save("synthetic-key").await.is_err());
    assert_eq!(rejected_store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(rejected_manager.status().unwrap().state, "missing");
    rejected_server.await.unwrap();
}

async fn spawn_server(status: u16) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let count = stream.read(&mut chunk).await.unwrap();
            request.extend_from_slice(&chunk[..count]);
            if count == 0 || request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with("GET /models HTTP/1.1\r\n"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic-key\r\n"));
        assert!(request.ends_with("\r\n\r\n"));
        let response = format!("HTTP/1.1 {status} Test\r\nContent-Length: 13\r\n\r\nresponse-body");
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    (format!("http://{address}"), server)
}
