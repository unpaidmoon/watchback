use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

const BROWSER_IO_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn run_with_timeout<T>(
    timeout: Duration,
    operation: &str,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| anyhow!("{operation} timed out after {} ms", timeout.as_millis()))?
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserCandidate {
    pub kind: String,
    pub path: PathBuf,
}

impl BrowserCandidate {
    pub fn chrome(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: "chrome".to_string(),
            path: path.into(),
        }
    }

    pub fn edge(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: "edge".to_string(),
            path: path.into(),
        }
    }
}

pub fn select_browser(candidates: Vec<BrowserCandidate>) -> Result<BrowserCandidate> {
    candidates
        .iter()
        .find(|item| item.kind == "chrome")
        .cloned()
        .or_else(|| candidates.into_iter().find(|item| item.kind == "edge"))
        .ok_or_else(|| anyhow!("未找到 Chrome 或 Edge"))
}

pub fn find_windows_browsers() -> Vec<BrowserCandidate> {
    let mut candidates = Vec::new();
    for name in ["chrome.exe", "chrome", "msedge.exe", "msedge"] {
        if let Some(path) = find_on_path(name) {
            candidates.push(if name.starts_with("chrome") {
                BrowserCandidate::chrome(path)
            } else {
                BrowserCandidate::edge(path)
            });
        }
    }
    for (kind, path) in [
        (
            "chrome",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        ),
        (
            "chrome",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ),
        (
            "edge",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        ),
        (
            "edge",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ),
    ] {
        let path = PathBuf::from(path);
        if path.exists() {
            candidates.push(if kind == "chrome" {
                BrowserCandidate::chrome(path)
            } else {
                BrowserCandidate::edge(path)
            });
        }
    }
    candidates
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|path| path.exists())
}

pub fn choose_debug_port(process_id: u32) -> u16 {
    9300 + (process_id % 200) as u16
}

pub fn build_browser_command(
    browser: impl AsRef<str>,
    login_url: impl AsRef<str>,
    port: u16,
    user_data_dir: Option<&Path>,
) -> Vec<String> {
    let mut command = vec![
        browser.as_ref().to_string(),
        format!("--remote-debugging-port={port}"),
        format!("--remote-allow-origins=http://127.0.0.1:{port}"),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--new-window".to_string(),
        "--disable-background-mode".to_string(),
    ];
    if let Some(user_data_dir) = user_data_dir {
        command.push(format!("--user-data-dir={}", user_data_dir.display()));
    }
    command.push(login_url.as_ref().to_string());
    command
}

pub struct BrowserLoginSession {
    child: Option<Child>,
    profile_dir: PathBuf,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BrowserShutdownReport {
    pub process_exited: bool,
    pub process_killed: bool,
    pub profile_removed: bool,
}

impl BrowserLoginSession {
    pub fn profile_dir(&self) -> &Path {
        &self.profile_dir
    }

    #[cfg(test)]
    pub(crate) fn detached_for_test(profile_dir: PathBuf) -> Self {
        Self {
            child: None,
            profile_dir,
        }
    }

    /// Explicit bounded cleanup used by every normal/cancel/timeout terminal path.
    /// `Drop` remains a synchronous last-resort fallback for abnormal unwinding.
    pub async fn shutdown(&mut self, timeout: Duration) -> BrowserShutdownReport {
        let mut report = BrowserShutdownReport::default();
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + timeout;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        report.process_exited = true;
                        break;
                    }
                    Ok(None) if Instant::now() < deadline => {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    _ => {
                        report.process_killed = child.kill().is_ok();
                        report.process_exited = child.wait().is_ok();
                        break;
                    }
                }
            }
        } else {
            report.process_exited = true;
        }
        report.profile_removed = cleanup_profile_dir(&self.profile_dir).is_ok();
        report
    }
}

/// Supervisor-visible ownership for a browser login process and its isolated
/// profile. Registration is synchronous so there is no cancellation point
/// between launching the process and transferring cleanup responsibility.
#[derive(Clone, Default)]
pub struct BrowserLoginSessionOwner {
    session: Arc<Mutex<Option<BrowserLoginSession>>>,
}

impl BrowserLoginSessionOwner {
    pub fn register(&self, session: BrowserLoginSession) -> Result<()> {
        let mut current = self
            .session
            .lock()
            .map_err(|_| anyhow!("browser cleanup owner is unavailable"))?;
        if current.is_some() {
            return Err(anyhow!("browser cleanup owner already has a session"));
        }
        *current = Some(session);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.session
            .lock()
            .map(|current| current.is_none())
            .unwrap_or(false)
    }

    pub async fn shutdown(&self, timeout: Duration) -> BrowserShutdownReport {
        let session = self
            .session
            .lock()
            .ok()
            .and_then(|mut current| current.take());
        match session {
            Some(mut session) => session.shutdown(timeout).await,
            None => BrowserShutdownReport {
                process_exited: true,
                process_killed: false,
                profile_removed: true,
            },
        }
    }
}

impl Drop for BrowserLoginSession {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        for _ in 0..5 {
            match cleanup_profile_dir(&self.profile_dir) {
                Ok(()) => break,
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    }
}

pub fn create_isolated_profile_dir(parent: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(parent)?;
    for _ in 0..10 {
        let profile_dir = parent.join(format!(".dashen-login-{}", Uuid::new_v4()));
        match std::fs::create_dir(&profile_dir) {
            Ok(()) => return Ok(profile_dir),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(anyhow!("无法创建独立登录会话目录。"))
}

pub fn cleanup_profile_dir(profile_dir: &Path) -> Result<()> {
    match std::fs::remove_dir_all(profile_dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StaleProfileCleanupReport {
    pub removed: usize,
    pub rejected: usize,
    pub failed: usize,
}

pub fn cleanup_stale_login_profiles(root: &Path) -> Result<StaleProfileCleanupReport> {
    let mut report = StaleProfileCleanupReport::default();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                report.failed += 1;
                continue;
            }
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(suffix) = name.strip_prefix(".dashen-login-") else {
            continue;
        };
        let Ok(id) = Uuid::parse_str(suffix) else {
            continue;
        };
        if id.to_string() != suffix {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => {
                report.failed += 1;
                continue;
            }
        };
        if !is_safe_ordinary_directory(&metadata) {
            report.rejected += 1;
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => report.removed += 1,
            Err(_) => report.failed += 1,
        }
    }
    Ok(report)
}

#[cfg(not(windows))]
fn is_safe_ordinary_directory(metadata: &std::fs::Metadata) -> bool {
    metadata.is_dir() && !metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_safe_ordinary_directory(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
}

pub fn launch_isolated_browser(
    browser: &BrowserCandidate,
    login_url: &str,
    port: u16,
    profile_parent: &Path,
) -> Result<BrowserLoginSession> {
    let profile_dir = create_isolated_profile_dir(profile_parent)?;
    let command = build_browser_command(
        browser.path.to_string_lossy(),
        login_url,
        port,
        Some(&profile_dir),
    );
    let mut process = std::process::Command::new(&command[0]);
    process.args(&command[1..]);
    match process.spawn() {
        Ok(child) => Ok(BrowserLoginSession {
            child: Some(child),
            profile_dir,
        }),
        Err(error) => {
            let _ = cleanup_profile_dir(&profile_dir);
            Err(error.into())
        }
    }
}

/// Starts a fresh, non-visible Chromium session used only for QR login.
/// Keeping this separate preserves the visible-browser login command unchanged.
pub fn launch_isolated_hidden_browser(
    browser: &BrowserCandidate,
    login_url: &str,
    port: u16,
    profile_parent: &Path,
) -> Result<BrowserLoginSession> {
    let profile_dir = create_isolated_profile_dir(profile_parent)?;
    let mut command = build_browser_command(
        browser.path.to_string_lossy(),
        login_url,
        port,
        Some(&profile_dir),
    );
    command.insert(1, "--headless=new".to_string());
    command.insert(2, "--window-size=1280,900".to_string());
    command.insert(3, "--hide-scrollbars".to_string());
    command.insert(4, "--disable-gpu".to_string());
    command.retain(|argument| argument != "--new-window");
    let mut process = std::process::Command::new(&command[0]);
    process.args(&command[1..]);
    match process.spawn() {
        Ok(child) => Ok(BrowserLoginSession {
            child: Some(child),
            profile_dir,
        }),
        Err(error) => {
            let _ = cleanup_profile_dir(&profile_dir);
            Err(error.into())
        }
    }
}

/// Reserves an ephemeral loopback port for a QR browser debugging session.
pub fn choose_available_qr_debug_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

pub async fn wait_for_debug_endpoint(port: u16, timeout: Duration) -> Result<()> {
    let client = reqwest::Client::new();
    let version_url = format!("http://127.0.0.1:{port}/json/version");
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if run_with_timeout(
            BROWSER_IO_TIMEOUT.min(remaining),
            "DevTools endpoint",
            async { Ok(client.get(&version_url).send().await?) },
        )
        .await
        .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(anyhow!("Chrome DevTools 调试端口启动超时。"))
}

pub async fn open_or_find_page(login_url: &str, port: u16) -> Result<String> {
    let client = reqwest::Client::new();
    let list_url = format!("http://127.0.0.1:{port}/json/list");
    let pages = json_get(&client, &list_url).await?;
    if let Some(websocket_url) = find_login_websocket_url(&pages, login_url) {
        return Ok(websocket_url);
    }

    if let Ok(websocket_url) = open_new_page(login_url, port).await {
        return Ok(websocket_url);
    }

    let pages = json_get(&client, &list_url).await?;
    find_login_websocket_url(&pages, login_url)
        .ok_or_else(|| anyhow!("未找到本次登录对应的浏览器页面。"))
}

/// Finds an already-open page without creating or navigating a target.
pub async fn find_existing_page(page_url: &str, port: u16) -> Result<Option<String>> {
    let client = reqwest::Client::new();
    let list_url = format!("http://127.0.0.1:{port}/json/list");
    let pages = json_get(&client, &list_url).await?;
    Ok(find_login_websocket_url(&pages, page_url))
}

pub async fn open_new_page(page_url: &str, port: u16) -> Result<String> {
    let client = reqwest::Client::new();
    let new_url = format!(
        "http://127.0.0.1:{port}/json/new?{}",
        percent_encode_for_query(page_url)
    );
    let page = match json_put(&client, &new_url).await {
        Ok(page) => page,
        Err(_) => json_get(&client, &new_url).await?,
    };
    page.get("webSocketDebuggerUrl")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("新建浏览器页面后未取得调试连接。"))
}

async fn json_get(client: &reqwest::Client, url: &str) -> Result<serde_json::Value> {
    run_with_timeout(BROWSER_IO_TIMEOUT, "DevTools HTTP GET", async {
        let response = client.get(url).send().await?.error_for_status()?;
        Ok(response.json::<serde_json::Value>().await?)
    })
    .await
}

async fn json_put(client: &reqwest::Client, url: &str) -> Result<serde_json::Value> {
    run_with_timeout(BROWSER_IO_TIMEOUT, "DevTools HTTP PUT", async {
        let response = client.put(url).send().await?.error_for_status()?;
        Ok(response.json::<serde_json::Value>().await?)
    })
    .await
}

fn find_login_websocket_url(pages: &serde_json::Value, login_url: &str) -> Option<String> {
    let login_route = login_url
        .split_once('?')
        .map_or(login_url, |(route, _)| route);
    pages.as_array()?.iter().find_map(|page| {
        let page_type = page.get("type").and_then(|item| item.as_str())?;
        let url = page.get("url").and_then(|item| item.as_str()).unwrap_or("");
        let websocket_url = page
            .get("webSocketDebuggerUrl")
            .and_then(|item| item.as_str())?;
        if page_type == "page" && url.starts_with(login_route) {
            Some(websocket_url.to_string())
        } else {
            None
        }
    })
}

fn percent_encode_for_query(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

pub struct CdpPage {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    next_id: i64,
}

/// Supervisor-visible ownership of the login page's DevTools connection.
///
/// The worker keeps the mutex guard while it uses the page. Aborting the worker
/// drops that guard, after which the supervisor can take and explicitly close
/// the connection before it publishes a terminal session state.
#[derive(Clone, Default)]
pub struct CdpPageOwner {
    page: Arc<tokio::sync::Mutex<Option<CdpPage>>>,
}

impl CdpPageOwner {
    /// Registration is deliberately synchronous. Once `CdpPage::connect`
    /// returns there is no cancellation point before ownership is transferred.
    pub fn register(&self, page: CdpPage) -> Result<()> {
        let mut current = self
            .page
            .try_lock()
            .map_err(|_| anyhow!("CDP cleanup owner is busy"))?;
        if current.is_some() {
            return Err(anyhow!("CDP cleanup owner already has a page"));
        }
        *current = Some(page);
        Ok(())
    }

    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, Option<CdpPage>> {
        self.page.lock().await
    }

    pub fn is_empty(&self) -> bool {
        self.page
            .try_lock()
            .map(|current| current.is_none())
            .unwrap_or(false)
    }

    /// Takes ownership before awaiting the close handshake, making repeated
    /// cleanup calls idempotent even if the first handshake times out.
    pub async fn close(&self, timeout: Duration) -> Result<()> {
        let page = {
            let mut current = self.page.lock().await;
            current.take()
        };
        if let Some(mut page) = page {
            page.close(timeout).await?;
        }
        Ok(())
    }
}

impl CdpPage {
    pub async fn connect(websocket_url: &str) -> Result<Self> {
        let (socket, _) = run_with_timeout(BROWSER_IO_TIMEOUT, "CDP websocket connect", async {
            Ok(tokio_tungstenite::connect_async(websocket_url).await?)
        })
        .await?;
        Ok(Self { socket, next_id: 1 })
    }

    pub async fn close(&mut self, timeout: Duration) -> Result<()> {
        run_with_timeout(timeout, "CDP websocket close", async {
            self.socket.close(None).await?;
            Ok(())
        })
        .await
    }

    pub async fn evaluate(
        &mut self,
        expression: &str,
        await_promise: bool,
    ) -> Result<serde_json::Value> {
        self.evaluate_in_context(expression, None, await_promise)
            .await
    }

    pub async fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.call_with_timeout(method, params, BROWSER_IO_TIMEOUT)
            .await
    }

    pub async fn call_with_timeout(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value> {
        let operation = format!("CDP {method}");
        tokio::time::timeout(timeout, self.call_unbounded(method, params))
            .await
            .map_err(|_| anyhow!("{operation} timed out after {} ms", timeout.as_millis()))?
    }

    async fn call_unbounded(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let message_id = self.next_id;
        self.next_id += 1;
        let request = json!({ "id": message_id, "method": method, "params": params });
        self.socket.send(Message::Text(request.to_string())).await?;
        while let Some(message) = self.socket.next().await {
            let message = message?;
            let Message::Text(text) = message else {
                continue;
            };
            let payload: serde_json::Value = serde_json::from_str(&text)?;
            if payload.get("id").and_then(|item| item.as_i64()) != Some(message_id) {
                continue;
            }
            if let Some(exception) = payload
                .get("error")
                .or_else(|| payload.get("exceptionDetails"))
            {
                return Err(anyhow!(exception.to_string()));
            }
            return Ok(payload
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null));
        }
        Err(anyhow!("浏览器调试连接已关闭。"))
    }

    pub async fn evaluate_in_context(
        &mut self,
        expression: &str,
        context_id: Option<i64>,
        await_promise: bool,
    ) -> Result<serde_json::Value> {
        let mut params = json!({
            "expression": expression,
            "awaitPromise": await_promise,
            "returnByValue": true,
        });
        if let Some(context_id) = context_id {
            params["contextId"] = json!(context_id);
        }
        let result = self.call("Runtime.evaluate", params).await?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(anyhow!(exception.to_string()));
        }
        Ok(result
            .get("result")
            .and_then(|item| item.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    pub async fn add_script_to_evaluate_on_new_document(&mut self, source: &str) -> Result<()> {
        self.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": source }),
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bounded_step_times_out_a_never_completing_cdp_operation() {
        let started = Instant::now();
        let error = run_with_timeout(
            Duration::from_millis(20),
            "synthetic CDP call",
            std::future::pending::<Result<()>>(),
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("synthetic CDP call"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn explicit_shutdown_removes_profile_without_a_process() {
        let parent = tempfile::tempdir().unwrap();
        let profile = create_isolated_profile_dir(parent.path()).unwrap();
        let mut session = BrowserLoginSession {
            child: None,
            profile_dir: profile.clone(),
        };

        let report = session.shutdown(Duration::from_millis(20)).await;

        assert!(report.process_exited);
        assert!(report.profile_removed);
        assert!(!profile.exists());
    }

    #[tokio::test]
    async fn shared_owner_releases_registered_session_before_reporting_empty() {
        let parent = tempfile::tempdir().unwrap();
        let profile = create_isolated_profile_dir(parent.path()).unwrap();
        let owner = BrowserLoginSessionOwner::default();
        owner
            .register(BrowserLoginSession {
                child: None,
                profile_dir: profile.clone(),
            })
            .unwrap();

        assert!(!owner.is_empty());
        let report = owner.shutdown(Duration::from_millis(20)).await;

        assert!(report.process_exited);
        assert!(report.profile_removed);
        assert!(owner.is_empty());
        assert!(!profile.exists());
    }

    #[tokio::test]
    async fn cdp_owner_close_is_explicit_idempotent_and_leaves_owner_empty() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            while let Some(message) = socket.next().await {
                if matches!(message.unwrap(), Message::Close(_)) {
                    return true;
                }
            }
            false
        });
        let owner = CdpPageOwner::default();
        owner
            .register(CdpPage::connect(&format!("ws://{address}")).await.unwrap())
            .unwrap();

        assert!(!owner.is_empty());
        owner.close(Duration::from_secs(1)).await.unwrap();
        owner.close(Duration::from_secs(1)).await.unwrap();

        assert!(owner.is_empty());
        assert!(tokio::time::timeout(Duration::from_secs(1), peer)
            .await
            .unwrap()
            .unwrap());
    }

    #[tokio::test]
    async fn cdp_owner_can_close_and_empty_after_its_worker_is_aborted() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            while let Some(message) = socket.next().await {
                if matches!(message.unwrap(), Message::Close(_)) {
                    return true;
                }
            }
            false
        });
        let owner = CdpPageOwner::default();
        owner
            .register(CdpPage::connect(&format!("ws://{address}")).await.unwrap())
            .unwrap();
        let worker_owner = owner.clone();
        let locked = Arc::new(tokio::sync::Notify::new());
        let worker_locked = Arc::clone(&locked);
        let worker = tokio::spawn(async move {
            let _page = worker_owner.lock().await;
            worker_locked.notify_one();
            std::future::pending::<()>().await;
        });
        locked.notified().await;

        worker.abort();
        let _ = worker.await;
        owner.close(Duration::from_secs(1)).await.unwrap();

        assert!(owner.is_empty());
        assert!(tokio::time::timeout(Duration::from_secs(1), peer)
            .await
            .unwrap()
            .unwrap());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn explicit_shutdown_forces_a_hung_process_after_the_deadline() {
        let parent = tempfile::tempdir().unwrap();
        let profile = create_isolated_profile_dir(parent.path()).unwrap();
        let child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .spawn()
            .unwrap();
        let owner = BrowserLoginSessionOwner::default();
        owner
            .register(BrowserLoginSession {
                child: Some(child),
                profile_dir: profile.clone(),
            })
            .unwrap();

        let report = owner.shutdown(Duration::from_millis(20)).await;

        assert!(report.process_killed);
        assert!(report.process_exited);
        assert!(report.profile_removed);
        assert!(owner.is_empty());
        assert!(!profile.exists());
    }

    #[test]
    fn startup_cleanup_removes_only_strict_login_profile_names() {
        let root = tempfile::tempdir().unwrap();
        let stale = root
            .path()
            .join(".dashen-login-11111111-1111-4111-8111-111111111111");
        let near_matches = [
            ".dashen-login-not-a-uuid",
            ".dashen-login-11111111-1111-4111-8111-111111111111-extra",
            "dashen-login-11111111-1111-4111-8111-111111111111",
        ];
        std::fs::create_dir(&stale).unwrap();
        for name in near_matches {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }

        let report = cleanup_stale_login_profiles(root.path()).unwrap();

        assert_eq!(report.removed, 1);
        assert!(!stale.exists());
        for name in near_matches {
            assert!(root.path().join(name).exists(), "changed {name}");
        }
    }

    #[test]
    fn startup_cleanup_rejects_linked_profile_and_preserves_external_sentinel() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("do-not-change");
        std::fs::write(&sentinel, b"sentinel").unwrap();
        let linked = root
            .path()
            .join(".dashen-login-22222222-2222-4222-8222-222222222222");
        create_directory_link(&linked, outside.path());

        let report = cleanup_stale_login_profiles(root.path()).unwrap();

        assert_eq!(report.removed, 0);
        assert_eq!(report.rejected, 1);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");
        assert!(std::fs::symlink_metadata(&linked).is_ok());
    }

    #[cfg(windows)]
    fn create_directory_link(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[cfg(unix)]
    fn create_directory_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[test]
    fn isolated_login_profiles_are_unique_and_cleanup_removes_them() {
        let parent = tempfile::tempdir().unwrap();
        let first = create_isolated_profile_dir(parent.path()).unwrap();
        let second = create_isolated_profile_dir(parent.path()).unwrap();

        assert_ne!(first, second);
        assert!(first.starts_with(parent.path()));
        assert!(second.starts_with(parent.path()));
        std::fs::write(first.join("synthetic-cookie"), b"old-session").unwrap();

        drop(BrowserLoginSession {
            child: None,
            profile_dir: first.clone(),
        });
        cleanup_profile_dir(&second).unwrap();
        assert!(!first.exists());
        assert!(!second.exists());
    }

    #[test]
    fn repeated_login_selects_the_new_login_page_instead_of_a_stale_dashen_tab() {
        let pages = serde_json::json!([
            {
                "type": "page",
                "url": "https://ds.163.com/",
                "webSocketDebuggerUrl": "ws://stale-home"
            },
            {
                "type": "page",
                "url": "https://pay.ds.163.com/activity/ld5?setId=synthetic",
                "webSocketDebuggerUrl": "ws://current-login"
            }
        ]);

        assert_eq!(
            find_login_websocket_url(
                &pages,
                "https://pay.ds.163.com/activity/ld5?setId=synthetic"
            ),
            Some("ws://current-login".to_string())
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "launches locally installed Chrome/Edge; run as a release smoke test"]
    async fn live_installed_browsers_start_devtools_and_cleanup_profile() {
        let mut tested_kinds = std::collections::BTreeSet::new();
        for browser in find_windows_browsers() {
            if tested_kinds.contains(&browser.kind) {
                continue;
            }
            let parent = tempfile::tempdir().unwrap();
            let port = choose_available_qr_debug_port().unwrap();
            let mut session =
                launch_isolated_hidden_browser(&browser, "about:blank", port, parent.path())
                    .unwrap_or_else(|error| panic!("{} failed to launch: {error}", browser.kind));
            let profile = session.profile_dir().to_path_buf();

            wait_for_debug_endpoint(port, Duration::from_secs(20))
                .await
                .unwrap_or_else(|error| panic!("{} DevTools unavailable: {error}", browser.kind));
            open_or_find_page("about:blank", port)
                .await
                .unwrap_or_else(|error| panic!("{} page unavailable: {error}", browser.kind));

            let report = session.shutdown(Duration::from_secs(2)).await;
            assert!(report.process_exited, "{} did not exit", browser.kind);
            assert!(report.profile_removed, "{} profile remained", browser.kind);
            assert!(!profile.exists(), "{} profile path remained", browser.kind);
            tested_kinds.insert(browser.kind);
        }
        assert!(
            !tested_kinds.is_empty(),
            "no supported browser was installed"
        );
    }
}
