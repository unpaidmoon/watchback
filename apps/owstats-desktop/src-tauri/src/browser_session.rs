use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

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

impl BrowserLoginSession {
    pub fn profile_dir(&self) -> &Path {
        &self.profile_dir
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
        if client.get(&version_url).send().await.is_ok() {
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
    let response = client.get(url).send().await?.error_for_status()?;
    Ok(response.json::<serde_json::Value>().await?)
}

async fn json_put(client: &reqwest::Client, url: &str) -> Result<serde_json::Value> {
    let response = client.put(url).send().await?.error_for_status()?;
    Ok(response.json::<serde_json::Value>().await?)
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

impl CdpPage {
    pub async fn connect(websocket_url: &str) -> Result<Self> {
        let (socket, _) = tokio_tungstenite::connect_async(websocket_url).await?;
        Ok(Self { socket, next_id: 1 })
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
}
