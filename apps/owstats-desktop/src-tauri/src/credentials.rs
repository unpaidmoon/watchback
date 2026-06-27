use crate::models::{CredentialLoginCancelResult, CredentialLoginResult, CredentialStatus};
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashenCredentials {
    pub role_id: i64,
    pub token: String,
    pub dts: Option<String>,
    pub server: i64,
    pub saved_at: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialLoginProgress {
    phase: String,
    message: String,
}

pub fn credential_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(".dashen_credentials.json"))
}

pub fn get_credential_status(app: &AppHandle) -> Result<CredentialStatus> {
    credential_status_for_path(&credential_path(app)?)
}

pub fn credential_status_for_path(path: &Path) -> Result<CredentialStatus> {
    if !path.exists() {
        return Ok(CredentialStatus {
            state: "missing".to_string(),
            message: "尚未保存网易大神登录凭证。".to_string(),
            exposes_secret: false,
            role_id: None,
            saved_at: None,
        });
    }
    let text = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let role_id = json.get("role_id").and_then(|item| {
        item.as_i64()
            .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
    });
    let token = json
        .get("token")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let saved_at = json
        .get("saved_at")
        .and_then(|item| item.as_str())
        .map(str::to_string);
    let complete = role_id.is_some() && !token.trim().is_empty();
    Ok(CredentialStatus {
        state: if complete { "saved" } else { "incomplete" }.to_string(),
        message: if complete {
            "已保存网易大神登录凭证，可用于同步。".to_string()
        } else {
            "本地凭证不完整，请重新登录。".to_string()
        },
        exposes_secret: false,
        role_id,
        saved_at,
    })
}

pub fn save_credentials_atomic(path: &Path, credentials: &DashenCredentials) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        let text = serde_json::to_string_pretty(credentials)?;
        file.write_all(text.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
    }
    std::fs::rename(tmp, path)?;
    Ok(())
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
    let candidates = [
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
    ];
    candidates
        .into_iter()
        .filter_map(|(kind, path)| {
            let path = PathBuf::from(path);
            if path.exists() {
                Some(if kind == "chrome" {
                    BrowserCandidate::chrome(path)
                } else {
                    BrowserCandidate::edge(path)
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn redacted_login_error(message: &str) -> String {
    let mut parts = Vec::new();
    for part in message.split_whitespace() {
        let lowered = part.to_ascii_lowercase();
        if lowered.contains("token") || lowered.contains("cookie") {
            let key = part.split_once('=').map(|(key, _)| key).unwrap_or(part);
            parts.push(format!("{key}=***REDACTED***"));
        } else {
            parts.push(part.to_string());
        }
    }
    parts.join(" ")
}

pub async fn start_dashen_login(app: AppHandle) -> Result<CredentialLoginResult> {
    emit_login_progress(&app, "locating_browser", "正在查找 Chrome 或 Edge。");
    let browser = select_browser(find_windows_browsers())?;
    emit_login_progress(&app, "opening_browser", "正在打开网易大神登录页。");
    let _child = std::process::Command::new(browser.path)
        .arg("https://ds.163.com/")
        .spawn()
        .map_err(|error| anyhow!(redacted_login_error(&error.to_string())))?;
    emit_login_progress(&app, "waiting_for_login", "请在浏览器完成登录。");
    Err(anyhow!(
        "浏览器登录已打开。自动提取 role_id/token 需要在后续版本完成 CDP 签名流程。"
    ))
}

pub fn cancel_dashen_login(_app: AppHandle) -> Result<CredentialLoginCancelResult> {
    Ok(CredentialLoginCancelResult {
        cancelled: true,
        message: "已取消登录等待。".to_string(),
    })
}

pub fn emit_login_progress(app: &AppHandle, phase: &str, message: &str) {
    let _ = app.emit(
        "credential-login-progress",
        CredentialLoginProgress {
            phase: phase.to_string(),
            message: message.to_string(),
        },
    );
}

pub fn diagnostic_credentials(role_id: i64, token: String) -> DashenCredentials {
    DashenCredentials {
        role_id,
        token,
        dts: None,
        server: 1,
        saved_at: Utc::now().to_rfc3339(),
    }
}
