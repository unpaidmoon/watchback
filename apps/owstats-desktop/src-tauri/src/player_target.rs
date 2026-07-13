use crate::models::PlayerTargetStatus;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTarget {
    pub input_bnet: String,
    pub resolved_bnet: Option<String>,
    pub customer_token: String,
    pub bound_at: String,
    #[serde(default)]
    pub source_role_id: Option<i64>,
}

pub fn target_matches_login_role(target: &PlayerTarget, role_id: i64) -> bool {
    target.source_role_id == Some(role_id)
}

pub fn player_target_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(crate::app_paths::AppPaths::from_app(app)?.legacy_player_target())
}

pub fn player_target_status_for_path(path: &Path) -> Result<PlayerTargetStatus> {
    if !path.exists() {
        return Ok(missing_status());
    }

    let text = std::fs::read_to_string(path).with_context(|| "读取目标玩家失败")?;
    let target: PlayerTarget = serde_json::from_str(&text).with_context(|| "解析目标玩家失败")?;
    if target.customer_token.trim().is_empty() {
        return Ok(missing_status());
    }

    Ok(PlayerTargetStatus {
        state: "bound".to_string(),
        input_bnet: Some(target.input_bnet),
        resolved_bnet: target.resolved_bnet,
        bound_at: Some(target.bound_at),
    })
}

pub fn read_player_target_for_path(path: &Path) -> Result<PlayerTarget> {
    let text = std::fs::read_to_string(path).with_context(|| "请先绑定目标玩家")?;
    let target: PlayerTarget = serde_json::from_str(&text).with_context(|| "目标玩家记录不可用")?;
    if target.customer_token.trim().is_empty() {
        return Err(anyhow!("请先绑定目标玩家"));
    }
    Ok(target)
}

pub fn save_player_target_for_path(path: &Path, target: &PlayerTarget) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(target)?;
    std::fs::write(path, text).with_context(|| "保存目标玩家失败")
}

pub fn clear_player_target_for_path(path: &Path) -> Result<bool> {
    if path.exists() {
        std::fs::remove_file(path).with_context(|| "清除目标玩家失败")?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn target_from_search_payload(
    input_bnet: &str,
    payload: &serde_json::Value,
    source_role_id: i64,
    bound_at: &str,
) -> Result<PlayerTarget> {
    let data = payload
        .get("data")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("未找到目标玩家"))?;
    let customer_token = data
        .get("customerToken")
        .or_else(|| data.get("customer_token"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("搜索结果缺少玩家标识"))?;
    let resolved_bnet = data
        .get("bnetId")
        .or_else(|| data.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Ok(PlayerTarget {
        input_bnet: input_bnet.trim().to_string(),
        resolved_bnet,
        customer_token: customer_token.to_string(),
        bound_at: bound_at.to_string(),
        source_role_id: Some(source_role_id),
    })
}

fn missing_status() -> PlayerTargetStatus {
    PlayerTargetStatus {
        state: "missing".to_string(),
        input_bnet: None,
        resolved_bnet: None,
        bound_at: None,
    }
}
