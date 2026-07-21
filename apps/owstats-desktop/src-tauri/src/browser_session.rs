use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

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
    ];
    if let Some(user_data_dir) = user_data_dir {
        command.push(format!("--user-data-dir={}", user_data_dir.display()));
    }
    command.push(login_url.as_ref().to_string());
    command
}

pub fn launch_browser(
    browser: &BrowserCandidate,
    login_url: &str,
    port: u16,
    profile_dir: &Path,
) -> Result<()> {
    std::fs::create_dir_all(profile_dir)?;
    let command = build_browser_command(
        browser.path.to_string_lossy(),
        login_url,
        port,
        Some(profile_dir),
    );
    let mut process = std::process::Command::new(&command[0]);
    process.args(&command[1..]);
    process.spawn()?;
    Ok(())
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
        let message_id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "id": message_id,
            "method": "Runtime.evaluate",
            "params": {
                "expression": expression,
                "awaitPromise": await_promise,
                "returnByValue": true,
            }
        });
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
            if let Some(exception) = payload.get("exceptionDetails") {
                return Err(anyhow!(exception.to_string()));
            }
            return Ok(payload
                .get("result")
                .and_then(|item| item.get("result"))
                .and_then(|item| item.get("value"))
                .cloned()
                .unwrap_or(serde_json::Value::Null));
        }
        Err(anyhow!("浏览器调试连接已关闭。"))
    }

    pub async fn add_script_to_evaluate_on_new_document(&mut self, source: &str) -> Result<()> {
        let message_id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "id": message_id,
            "method": "Page.addScriptToEvaluateOnNewDocument",
            "params": {
                "source": source
            }
        });
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
            if let Some(error) = payload.get("error") {
                return Err(anyhow!("{}", error));
            }
            return Ok(());
        }
        Err(anyhow!("浏览器调试连接已关闭。"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
