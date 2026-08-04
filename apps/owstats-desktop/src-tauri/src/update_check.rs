use futures_util::StreamExt;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::OnceCell;

pub const UPDATE_MANIFEST_URL: &str =
    "https://watchback-updates-cn-202608.oss-cn-chengdu.aliyuncs.com/latest.json";
const GITHUB_RELEASES_URL: &str = "https://github.com/fengpianpianne/watchback/releases";
const BAIDU_DOWNLOAD_URL: &str = "https://pan.baidu.com/s/5GbzDPA8NkteOswgwKHN5uw";
const QUARK_DOWNLOAD_URL: &str = "https://pan.quark.cn/s/0d7bae24fca3";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateDownloadSource {
    Github,
    Baidu,
    Quark,
}

pub fn download_url(source: UpdateDownloadSource) -> &'static str {
    match source {
        UpdateDownloadSource::Github => GITHUB_RELEASES_URL,
        UpdateDownloadSource::Baidu => BAIDU_DOWNLOAD_URL,
        UpdateDownloadSource::Quark => QUARK_DOWNLOAD_URL,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAnnouncement {
    pub latest_version: String,
    pub published_at: String,
    pub title: String,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateCheckStatus {
    UpdateAvailable,
    UpToDate,
    Disabled,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub status: UpdateCheckStatus,
    pub current_version: String,
    pub announcement: Option<UpdateAnnouncement>,
}

pub struct UpdateChecker {
    endpoint: String,
    current_version: String,
    request_timeout: Duration,
    result: OnceCell<UpdateCheckResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateManifest {
    schema_version: u32,
    enabled: bool,
    latest_version: Option<String>,
    published_at: Option<String>,
    title: Option<String>,
    notes: Option<Vec<String>>,
}

impl UpdateChecker {
    pub fn new() -> Self {
        Self::for_endpoint(UPDATE_MANIFEST_URL, env!("CARGO_PKG_VERSION"))
    }

    fn for_endpoint(endpoint: impl Into<String>, current_version: impl Into<String>) -> Self {
        Self::for_endpoint_with_timeout(endpoint, current_version, Duration::from_secs(5))
    }

    fn for_endpoint_with_timeout(
        endpoint: impl Into<String>,
        current_version: impl Into<String>,
        request_timeout: Duration,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            current_version: current_version.into(),
            request_timeout,
            result: OnceCell::new(),
        }
    }

    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    pub async fn check_once(&self) -> UpdateCheckResult {
        self.result
            .get_or_init(|| async { self.fetch_result().await })
            .await
            .clone()
    }

    async fn fetch_result(&self) -> UpdateCheckResult {
        let unavailable = |reason: &'static str| {
            crate::diagnostics::record(
                crate::diagnostics::Level::Warn,
                "application_update",
                "check_failed",
                &[
                    ("currentVersion", self.current_version.clone()),
                    ("reason", reason.to_string()),
                ],
            );
            UpdateCheckResult {
                status: UpdateCheckStatus::Unavailable,
                current_version: self.current_version.clone(),
                announcement: None,
            }
        };
        let mut client_builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(self.request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .https_only(self.endpoint.starts_with("https://"))
            .user_agent(format!("Watchback/{}", self.current_version));
        if self.endpoint.starts_with("http://127.0.0.1:") {
            client_builder = client_builder.no_proxy();
        }
        let client = match client_builder.build() {
            Ok(client) => client,
            Err(_) => return unavailable("client_configuration"),
        };
        let response = match client.get(&self.endpoint).send().await {
            Ok(response) if response.status() == reqwest::StatusCode::OK => response,
            Ok(_) => return unavailable("http_status"),
            Err(error) if error.is_timeout() => return unavailable("timeout"),
            Err(_) => return unavailable("request_failed"),
        };
        if response
            .content_length()
            .is_some_and(|length| length > 64 * 1024)
        {
            return unavailable("response_too_large");
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let Ok(chunk) = chunk else {
                return unavailable("response_interrupted");
            };
            if bytes.len().saturating_add(chunk.len()) > 64 * 1024 {
                return unavailable("response_too_large");
            }
            bytes.extend_from_slice(&chunk);
        }
        let Ok(manifest) = serde_json::from_slice::<UpdateManifest>(&bytes) else {
            return unavailable("invalid_json");
        };
        if manifest.schema_version != 1 {
            return unavailable("unsupported_schema");
        }
        if !manifest.enabled {
            crate::diagnostics::record(
                crate::diagnostics::Level::Info,
                "application_update",
                "disabled",
                &[("currentVersion", self.current_version.clone())],
            );
            return UpdateCheckResult {
                status: UpdateCheckStatus::Disabled,
                current_version: self.current_version.clone(),
                announcement: None,
            };
        }
        let (Some(latest_version), Some(published_at), Some(title), Some(notes)) = (
            manifest.latest_version,
            manifest.published_at,
            manifest.title,
            manifest.notes,
        ) else {
            return unavailable("missing_field");
        };
        let (Ok(current), Ok(latest)) = (
            Version::parse(&self.current_version),
            Version::parse(&latest_version),
        ) else {
            return unavailable("invalid_version");
        };
        if !latest.pre.is_empty() {
            return unavailable("prerelease_version");
        }
        let Ok(date) = chrono::NaiveDate::parse_from_str(&published_at, "%Y-%m-%d") else {
            return unavailable("invalid_date");
        };
        if date.format("%Y-%m-%d").to_string() != published_at {
            return unavailable("invalid_date");
        }
        let title = title.trim().to_string();
        if !(1..=80).contains(&title.chars().count()) || !(1..=20).contains(&notes.len()) {
            return unavailable("invalid_content");
        }
        let mut validated_notes = Vec::with_capacity(notes.len());
        for note in notes {
            let note = note.trim().to_string();
            if !(1..=500).contains(&note.chars().count()) {
                return unavailable("invalid_content");
            }
            validated_notes.push(note);
        }
        if latest <= current {
            crate::diagnostics::record(
                crate::diagnostics::Level::Info,
                "application_update",
                "up_to_date",
                &[
                    ("currentVersion", self.current_version.clone()),
                    ("latestVersion", latest_version),
                ],
            );
            return UpdateCheckResult {
                status: UpdateCheckStatus::UpToDate,
                current_version: self.current_version.clone(),
                announcement: None,
            };
        }
        crate::diagnostics::record(
            crate::diagnostics::Level::Info,
            "application_update",
            "update_available",
            &[
                ("currentVersion", self.current_version.clone()),
                ("latestVersion", latest_version.clone()),
            ],
        );
        UpdateCheckResult {
            status: UpdateCheckStatus::UpdateAvailable,
            current_version: self.current_version.clone(),
            announcement: Some(UpdateAnnouncement {
                latest_version,
                published_at,
                title,
                notes: validated_notes,
            }),
        }
    }
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn serve_once(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let body = body.to_string();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{address}/latest.json")
    }

    async fn serve_raw_once(response: impl Into<Vec<u8>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = response.into();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            stream.write_all(&response).await.unwrap();
        });
        format!("http://{address}/latest.json")
    }

    async fn serve_counted_once(body: &str) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let observed = count.clone();
        let body = body.to_string();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            observed.fetch_add(1, Ordering::SeqCst);
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        (format!("http://{address}/latest.json"), count)
    }

    #[tokio::test]
    async fn newer_valid_manifest_is_reported_through_the_checker_interface() {
        let endpoint = serve_once(
            "200 OK",
            r#"{"schemaVersion":1,"enabled":true,"latestVersion":"0.5.3","publishedAt":"2026-08-10","title":"Watchback v0.5.3 已发布","notes":["新增程序内更新公告"]}"#,
        )
        .await;
        let checker = UpdateChecker::for_endpoint(endpoint, "0.5.2");

        let result = checker.check_once().await;

        assert_eq!(result.status, UpdateCheckStatus::UpdateAvailable);
        assert_eq!(
            result.announcement,
            Some(UpdateAnnouncement {
                latest_version: "0.5.3".to_string(),
                published_at: "2026-08-10".to_string(),
                title: "Watchback v0.5.3 已发布".to_string(),
                notes: vec!["新增程序内更新公告".to_string()],
            })
        );
    }

    #[tokio::test]
    async fn disabled_manifest_needs_only_the_protocol_switch() {
        let endpoint = serve_once("200 OK", r#"{"schemaVersion":1,"enabled":false}"#).await;
        let checker = UpdateChecker::for_endpoint(endpoint, "0.5.2");

        let result = checker.check_once().await;

        assert_eq!(result.status, UpdateCheckStatus::Disabled);
        assert_eq!(result.announcement, None);
    }

    #[tokio::test]
    async fn equal_and_older_versions_are_up_to_date() {
        for latest in ["0.5.2", "0.5.1"] {
            let body = serde_json::json!({
                "schemaVersion": 1,
                "enabled": true,
                "latestVersion": latest,
                "publishedAt": "2026-08-10",
                "title": "稳定版本",
                "notes": ["维护更新"]
            })
            .to_string();
            let endpoint = serve_once("200 OK", &body).await;
            let result = UpdateChecker::for_endpoint(endpoint, "0.5.2")
                .check_once()
                .await;
            assert_eq!(result.status, UpdateCheckStatus::UpToDate, "{latest}");
            assert_eq!(result.announcement, None);
        }
    }

    #[tokio::test]
    async fn invalid_enabled_announcements_are_unavailable() {
        let cases = [
            serde_json::json!({"schemaVersion": 2, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-08-10", "title": "更新", "notes": ["内容"]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "v0.5.3", "publishedAt": "2026-08-10", "title": "更新", "notes": ["内容"]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3-beta.1", "publishedAt": "2026-08-10", "title": "更新", "notes": ["内容"]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-02-30", "title": "更新", "notes": ["内容"]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-08-10", "title": "   ", "notes": ["内容"]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-08-10", "title": "x".repeat(81), "notes": ["内容"]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-08-10", "title": "更新", "notes": []}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-08-10", "title": "更新", "notes": [" "]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-08-10", "title": "更新", "notes": ["x".repeat(501)]}),
            serde_json::json!({"schemaVersion": 1, "enabled": true, "latestVersion": "0.5.3", "publishedAt": "2026-08-10", "title": "更新", "notes": (0..21).map(|index| format!("内容 {index}")).collect::<Vec<_>>() }),
        ];
        for (index, body) in cases.into_iter().enumerate() {
            let endpoint = serve_once("200 OK", &body.to_string()).await;
            let result = UpdateChecker::for_endpoint(endpoint, "0.5.2")
                .check_once()
                .await;
            assert_eq!(
                result.status,
                UpdateCheckStatus::Unavailable,
                "invalid manifest case {index}"
            );
            assert_eq!(result.announcement, None);
        }
    }

    #[tokio::test]
    async fn valid_text_is_trimmed_and_unknown_fields_are_ignored() {
        let endpoint = serve_once(
            "200 OK",
            r#"{"schemaVersion":1,"enabled":true,"latestVersion":"0.5.3","publishedAt":"2026-08-10","title":"  更新标题  ","notes":["  第一项  "],"futureField":{"safe":true}}"#,
        )
        .await;
        let result = UpdateChecker::for_endpoint(endpoint, "0.5.2")
            .check_once()
            .await;

        let announcement = result.announcement.unwrap();
        assert_eq!(announcement.title, "更新标题");
        assert_eq!(announcement.notes, ["第一项"]);
    }

    #[tokio::test]
    async fn transport_and_payload_failures_are_unavailable() {
        let oversized = "x".repeat(64 * 1024 + 1);
        let interrupted = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{}".to_vec();
        let endpoints = [
            serve_once("404 Not Found", "{}").await,
            serve_once("500 Internal Server Error", "{}").await,
            serve_once("302 Found", "").await,
            serve_once("200 OK", "not-json").await,
            serve_once("200 OK", &oversized).await,
            serve_raw_once(interrupted).await,
        ];
        for (index, endpoint) in endpoints.into_iter().enumerate() {
            let result = UpdateChecker::for_endpoint(endpoint, "0.5.2")
                .check_once()
                .await;
            assert_eq!(
                result.status,
                UpdateCheckStatus::Unavailable,
                "transport case {index}"
            );
        }
    }

    #[tokio::test]
    async fn concurrent_and_later_calls_share_one_process_result() {
        let body = r#"{"schemaVersion":1,"enabled":true,"latestVersion":"0.5.3","publishedAt":"2026-08-10","title":"更新","notes":["内容"]}"#;
        let (endpoint, requests) = serve_counted_once(body).await;
        let checker = UpdateChecker::for_endpoint(endpoint, "0.5.2");

        let (first, second) = tokio::join!(checker.check_once(), checker.check_once());
        let third = checker.check_once().await;

        assert_eq!(first, second);
        assert_eq!(second, third);
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn request_timeout_is_unavailable_without_blocking_startup() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });
        let checker = UpdateChecker::for_endpoint_with_timeout(
            format!("http://{address}/latest.json"),
            "0.5.2",
            Duration::from_millis(40),
        );

        let started = std::time::Instant::now();
        let result = checker.check_once().await;

        assert_eq!(result.status, UpdateCheckStatus::Unavailable);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn download_sources_map_only_to_the_three_fixed_release_pages() {
        assert_eq!(
            download_url(UpdateDownloadSource::Github),
            "https://github.com/fengpianpianne/watchback/releases"
        );
        assert_eq!(
            download_url(UpdateDownloadSource::Baidu),
            "https://pan.baidu.com/s/5GbzDPA8NkteOswgwKHN5uw"
        );
        assert_eq!(
            download_url(UpdateDownloadSource::Quark),
            "https://pan.quark.cn/s/0d7bae24fca3"
        );
        assert!(serde_json::from_str::<UpdateDownloadSource>(r#""unknown""#).is_err());
    }
}
