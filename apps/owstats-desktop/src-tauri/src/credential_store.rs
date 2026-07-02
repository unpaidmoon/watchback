use crate::models::CredentialStatus;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashenCredentials {
    pub role_id: i64,
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_token: Option<String>,
    pub dts: Option<String>,
    pub server: i64,
    pub saved_at: String,
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
    let role_id = json
        .get("role_id")
        .or_else(|| json.get("roleId"))
        .and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
        });
    let token = json
        .get("token")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let saved_at = json
        .get("saved_at")
        .or_else(|| json.get("savedAt"))
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

pub fn clear_credentials_for_path(path: &Path) -> Result<bool> {
    if path.exists() {
        std::fs::remove_file(path)?;
        return Ok(true);
    }

    Ok(false)
}

pub fn redact_sensitive_text(message: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_credentials_removes_saved_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".dashen_credentials.json");
        let credentials = DashenCredentials {
            role_id: 5479,
            token: "token-secret".to_string(),
            customer_token: Some("customer-secret".to_string()),
            dts: Some("2026".to_string()),
            server: 1,
            saved_at: "2026-07-01T00:00:00Z".to_string(),
        };
        save_credentials_atomic(&path, &credentials).unwrap();

        clear_credentials_for_path(&path).unwrap();

        assert!(!path.exists());
        let status = credential_status_for_path(&path).unwrap();
        assert_eq!(status.state, "missing");
        assert_eq!(status.role_id, None);
    }
}
