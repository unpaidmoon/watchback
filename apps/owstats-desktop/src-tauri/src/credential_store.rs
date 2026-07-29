use crate::models::CredentialStatus;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

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
    if !path_entry_exists(path)? {
        return Ok(CredentialStatus {
            state: "missing".to_string(),
            message: "尚未保存网易大神登录凭证。".to_string(),
            exposes_secret: false,
            role_id: None,
            saved_at: None,
        });
    }
    ensure_regular_file_or_missing(path)?;
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
        state: if complete { "ready" } else { "incomplete" }.to_string(),
        message: if !complete {
            "本地凭证不完整，请在指挥台添加账号以更新登录。".to_string()
        } else {
            "已保存网易大神登录凭证，请绑定 BattleTag 后同步赛季数据。".to_string()
        },
        exposes_secret: false,
        role_id,
        saved_at,
    })
}

pub fn save_credentials_atomic(path: &Path, credentials: &DashenCredentials) -> Result<()> {
    ensure_regular_file_or_missing(path)?;
    if let Some(parent) = path.parent() {
        if path_entry_exists(parent)? {
            ensure_directory(parent)?;
        }
        std::fs::create_dir_all(parent)?;
        ensure_directory(parent)?;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".credentials.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        let text = serde_json::to_string_pretty(credentials)?;
        file.write_all(text.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn read_credentials(path: &Path) -> Result<DashenCredentials> {
    ensure_regular_file_or_missing(path)?;
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn path_entry_exists(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn ensure_regular_file_or_missing(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata)
            if !metadata.file_type().is_symlink()
                && !is_windows_reparse(&metadata)
                && metadata.is_file() =>
        {
            Ok(())
        }
        Ok(_) => anyhow::bail!("unsafe credential file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn ensure_directory(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) || !metadata.is_dir() {
        anyhow::bail!("unsafe credential directory")
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse(_metadata: &std::fs::Metadata) -> bool {
    false
}
