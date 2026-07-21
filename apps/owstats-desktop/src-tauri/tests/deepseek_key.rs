use anyhow::Result;
use owstats_desktop_lib::deepseek_key::{
    build_http_client, normalize_base_url, validate_api_key, validate_models_status,
    DeepSeekCredentialStore, DeepSeekKeyManager, DEEPSEEK_REQUEST_TIMEOUT,
    DEFAULT_DEEPSEEK_BASE_URL,
};
use owstats_desktop_lib::DeepSeekKeyStatus;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct FakeStore {
    key: Mutex<Option<String>>,
    writes: AtomicUsize,
    unreadable: AtomicBool,
}

impl FakeStore {
    fn with_key(key: Option<&str>) -> Self {
        Self {
            key: Mutex::new(key.map(str::to_owned)),
            writes: AtomicUsize::new(0),
            unreadable: AtomicBool::new(false),
        }
    }
}

impl DeepSeekCredentialStore for FakeStore {
    fn read(&self) -> Result<Option<String>> {
        if self.unreadable.load(Ordering::SeqCst) {
            anyhow::bail!("synthetic credential read failure");
        }
        Ok(self.key.lock().unwrap().clone())
    }

    fn write(&self, api_key: &str) -> Result<()> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        *self.key.lock().unwrap() = Some(api_key.to_owned());
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
    let (base_url, server) = spawn_server(200, "synthetic-key").await;
    validate_api_key(&build_http_client().unwrap(), &base_url, "synthetic-key")
        .await
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn manager_reports_ready_or_missing_and_writes_only_after_200() {
    let missing_store = Arc::new(FakeStore::with_key(None));
    let missing_manager = DeepSeekKeyManager::with_store(
        build_http_client().unwrap(),
        DEFAULT_DEEPSEEK_BASE_URL,
        missing_store.clone(),
    )
    .unwrap();
    assert_eq!(missing_manager.status().unwrap().state, "missing");
    let missing_error = missing_manager.backend_api_key().unwrap_err().to_string();
    assert!(missing_error.contains("DeepSeek API Key"));
    assert!(!missing_error.contains(DEFAULT_DEEPSEEK_BASE_URL));

    let (ok_base_url, ok_server) = spawn_server(200, "synthetic-key").await;
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
    assert_eq!(ok_manager.backend_api_key().unwrap(), "synthetic-key");
    let serialized_status = serde_json::to_string(&ok_manager.status().unwrap()).unwrap();
    assert!(!serialized_status.contains("synthetic-key"));
    ok_server.await.unwrap();

    let rejected_store = Arc::new(FakeStore::with_key(Some("old-synthetic-key")));
    let (rejected_base_url, rejected_server) = spawn_server(401, "synthetic-key").await;
    let rejected_manager = DeepSeekKeyManager::with_store(
        build_http_client().unwrap(),
        &rejected_base_url,
        rejected_store.clone(),
    )
    .unwrap();
    assert!(rejected_manager.save("synthetic-key").await.is_err());
    assert_eq!(rejected_store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(
        rejected_manager.backend_api_key().unwrap(),
        "old-synthetic-key"
    );
    assert_eq!(rejected_manager.status().unwrap().state, "ready");
    rejected_server.await.unwrap();

    let corrupt_store = Arc::new(FakeStore::with_key(Some("synthetic-key")));
    corrupt_store.unreadable.store(true, Ordering::SeqCst);
    let corrupt_manager = DeepSeekKeyManager::with_store(
        build_http_client().unwrap(),
        DEFAULT_DEEPSEEK_BASE_URL,
        corrupt_store,
    )
    .unwrap();
    let status_error = corrupt_manager.status().unwrap_err().to_string();
    for forbidden in [
        "synthetic-key",
        DEFAULT_DEEPSEEK_BASE_URL,
        "authorization",
        "body",
    ] {
        assert!(!status_error.to_ascii_lowercase().contains(forbidden));
    }

    let empty_manager = DeepSeekKeyManager::with_store(
        build_http_client().unwrap(),
        DEFAULT_DEEPSEEK_BASE_URL,
        Arc::new(FakeStore::with_key(Some("  "))),
    )
    .unwrap();
    assert!(empty_manager.status().is_err());
    assert!(empty_manager.backend_api_key().is_err());
}

#[tokio::test]
async fn successful_validation_overwrites_existing_key() {
    let store = Arc::new(FakeStore::with_key(Some("old-synthetic-key")));
    let (base_url, server) = spawn_server(200, "new-synthetic-key").await;
    let manager =
        DeepSeekKeyManager::with_store(build_http_client().unwrap(), &base_url, store).unwrap();

    manager.save("new-synthetic-key").await.unwrap();
    assert_eq!(manager.backend_api_key().unwrap(), "new-synthetic-key");
    server.await.unwrap();
}

async fn spawn_server(
    status: u16,
    expected_key: &'static str,
) -> (String, tokio::task::JoinHandle<()>) {
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
            .contains(&format!("authorization: bearer {expected_key}\r\n")));
        assert!(request.ends_with("\r\n\r\n"));
        let response = format!("HTTP/1.1 {status} Test\r\nContent-Length: 13\r\n\r\nresponse-body");
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    (format!("http://{address}"), server)
}
