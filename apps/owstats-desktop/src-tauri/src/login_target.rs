use crate::credential_store::{redact_sensitive_text, DashenCredentials};
use crate::dashen_client::{DashenClient, DashenCredentials as ClientCredentials};
use crate::models::{LoginTargetStatus, PlayerTargetStatus};
use crate::player_target::{
    player_target_status_for_path, read_player_target_for_path, target_from_search_payload,
    target_matches_login_role, PlayerTarget,
};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetAction {
    AutoBind,
    KeepCurrent,
    ConfirmSwitch,
}

pub fn decide_target_action(detected: &str, current: Option<&str>) -> TargetAction {
    match current {
        None => TargetAction::AutoBind,
        Some(value)
            if normalize_bnet_for_compare(value) == normalize_bnet_for_compare(detected) =>
        {
            TargetAction::KeepCurrent
        }
        Some(_) => TargetAction::ConfirmSwitch,
    }
}

#[derive(Clone, Default)]
pub struct LoginTargetCoordinator {
    session: Arc<Mutex<Option<LoginTargetSession>>>,
}

#[derive(Debug, Clone)]
struct LoginTargetSession {
    source_role_id: i64,
    detected_bnet: Option<String>,
    status: LoginTargetStatus,
}

impl LoginTargetCoordinator {
    pub fn clear(&self) {
        *self.session.lock().expect("login target session poisoned") = None;
    }

    pub fn status(&self) -> Option<LoginTargetStatus> {
        self.session
            .lock()
            .expect("login target session poisoned")
            .as_ref()
            .map(|session| session.status.clone())
    }

    pub fn replace_unidentified(&self, source_role_id: i64) -> LoginTargetStatus {
        self.replace(
            source_role_id,
            None,
            status(
                "unidentified",
                None,
                None,
                "登录成功，但未能自动识别战网昵称。请进入显示“战网昵称”的页面后重新识别。",
                false,
                false,
                false,
            ),
        )
    }

    pub fn replace_ready(
        &self,
        source_role_id: i64,
        detected_bnet: &str,
        current_bnet: &str,
    ) -> LoginTargetStatus {
        self.replace(
            source_role_id,
            Some(detected_bnet.to_string()),
            status(
                "ready",
                Some(detected_bnet),
                Some(current_bnet),
                "已识别并绑定当前登录账号，可以同步赛季数据。",
                true,
                false,
                false,
            ),
        )
    }

    pub fn replace_switch_required(
        &self,
        source_role_id: i64,
        detected_bnet: &str,
        current_bnet: &str,
    ) -> LoginTargetStatus {
        self.replace(
            source_role_id,
            Some(detected_bnet.to_string()),
            status(
                "switchRequired",
                Some(detected_bnet),
                Some(current_bnet),
                "检测到当前登录账号与已保存目标不同，请确认是否切换。",
                false,
                true,
                false,
            ),
        )
    }

    pub fn replace_bind_failed(
        &self,
        source_role_id: i64,
        detected_bnet: &str,
        message: &str,
    ) -> LoginTargetStatus {
        self.replace(
            source_role_id,
            Some(detected_bnet.to_string()),
            status(
                "bindFailed",
                Some(detected_bnet),
                None,
                &format!("自动绑定失败：{}", redact_sensitive_text(message)),
                false,
                false,
                true,
            ),
        )
    }

    pub fn keep_current(&self) -> Result<LoginTargetStatus> {
        let mut guard = self
            .session
            .lock()
            .map_err(|_| anyhow!("登录目标状态不可用"))?;
        let session = guard
            .as_mut()
            .filter(|session| session.status.state == "switchRequired")
            .ok_or_else(|| anyhow!("当前没有待确认的账号切换"))?;
        session.status = status(
            "mismatchKept",
            session.detected_bnet.as_deref(),
            session.status.current_bnet.as_deref(),
            "已保留原目标；当前登录账号与目标不同，赛季同步已禁用。",
            false,
            false,
            false,
        );
        Ok(session.status.clone())
    }

    pub fn status_or_persisted(
        &self,
        credentials: Option<&DashenCredentials>,
        target_path: &Path,
    ) -> LoginTargetStatus {
        if let Some(current) = self.status() {
            return current;
        }

        let Some(credentials) = credentials else {
            return status(
                "verificationRequired",
                None,
                persisted_display_bnet(target_path).as_deref(),
                "请先登录并重新识别当前账号。",
                false,
                false,
                false,
            );
        };
        let Ok(target) = read_player_target_for_path(target_path) else {
            return self.replace_verification_required(
                credentials.role_id,
                None,
                "当前目标尚未通过本次登录验证，请重新识别。",
            );
        };
        let current_bnet = target_display_bnet(&target);
        if target_matches_login_role(&target, credentials.role_id) {
            self.replace_ready(credentials.role_id, &current_bnet, &current_bnet)
        } else {
            self.replace_verification_required(
                credentials.role_id,
                Some(&current_bnet),
                "当前目标与登录账号的关联尚未验证，请重新识别。",
            )
        }
    }

    fn replace_verification_required(
        &self,
        source_role_id: i64,
        current_bnet: Option<&str>,
        message: &str,
    ) -> LoginTargetStatus {
        self.replace(
            source_role_id,
            None,
            status(
                "verificationRequired",
                None,
                current_bnet,
                message,
                false,
                false,
                false,
            ),
        )
    }

    fn binding_candidate(&self, role_id: i64, required_state: &str) -> Result<String> {
        let guard = self
            .session
            .lock()
            .map_err(|_| anyhow!("登录目标状态不可用"))?;
        let session = guard
            .as_ref()
            .filter(|session| session.source_role_id == role_id)
            .filter(|session| session.status.state == required_state)
            .ok_or_else(|| anyhow!("本轮没有可用的已识别账号"))?;
        session
            .detected_bnet
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("本轮没有可用的已识别账号"))
    }

    fn replace(
        &self,
        source_role_id: i64,
        detected_bnet: Option<String>,
        status: LoginTargetStatus,
    ) -> LoginTargetStatus {
        *self.session.lock().expect("login target session poisoned") = Some(LoginTargetSession {
            source_role_id,
            detected_bnet,
            status: status.clone(),
        });
        status
    }
}

pub async fn bind_detected_target_with_base_url(
    credentials: &DashenCredentials,
    detected_bnet: &str,
    target_path: &Path,
    root_base_url: &str,
) -> Result<PlayerTargetStatus> {
    let client = DashenClient::new(
        ClientCredentials {
            role_id: credentials.role_id,
            token: credentials.token.clone(),
            dts: credentials
                .dts
                .as_deref()
                .and_then(|value| value.parse().ok()),
            server: credentials.server,
        },
        root_base_url.to_string(),
    )?;
    let payload = client.search_bnet_account(detected_bnet).await?;
    let target = target_from_search_payload(
        detected_bnet,
        &payload,
        credentials.role_id,
        &Utc::now().to_rfc3339(),
    )?;
    client.query_card(&target.customer_token).await?;
    save_target_atomic(target_path, &target)?;
    player_target_status_for_path(target_path)
}

pub async fn reconcile_detected_target(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    detected_bnet: Option<&str>,
    target_path: &Path,
    root_base_url: &str,
) -> LoginTargetStatus {
    let Some(detected_bnet) = detected_bnet
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return coordinator.replace_unidentified(credentials.role_id);
    };

    let current = read_player_target_for_path(target_path).ok();
    match current.as_ref() {
        None => {
            bind_or_record_failure(
                coordinator,
                credentials,
                detected_bnet,
                target_path,
                root_base_url,
            )
            .await
        }
        Some(target)
            if decide_target_action(detected_bnet, Some(&target_display_bnet(target)))
                == TargetAction::KeepCurrent
                && target_matches_login_role(target, credentials.role_id) =>
        {
            let current_bnet = target_display_bnet(target);
            coordinator.replace_ready(credentials.role_id, detected_bnet, &current_bnet)
        }
        Some(target)
            if decide_target_action(detected_bnet, Some(&target_display_bnet(target)))
                == TargetAction::KeepCurrent =>
        {
            bind_or_record_failure(
                coordinator,
                credentials,
                detected_bnet,
                target_path,
                root_base_url,
            )
            .await
        }
        Some(target) => coordinator.replace_switch_required(
            credentials.role_id,
            detected_bnet,
            &target_display_bnet(target),
        ),
    }
}

pub async fn confirm_switch(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    target_path: &Path,
    root_base_url: &str,
) -> LoginTargetStatus {
    match coordinator.binding_candidate(credentials.role_id, "switchRequired") {
        Ok(detected_bnet) => {
            bind_or_record_failure(
                coordinator,
                credentials,
                &detected_bnet,
                target_path,
                root_base_url,
            )
            .await
        }
        Err(error) => coordinator.replace_verification_required(
            credentials.role_id,
            persisted_display_bnet(target_path).as_deref(),
            &redact_sensitive_text(&error.to_string()),
        ),
    }
}

pub async fn retry_binding(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    target_path: &Path,
    root_base_url: &str,
) -> LoginTargetStatus {
    match coordinator.binding_candidate(credentials.role_id, "bindFailed") {
        Ok(detected_bnet) => {
            bind_or_record_failure(
                coordinator,
                credentials,
                &detected_bnet,
                target_path,
                root_base_url,
            )
            .await
        }
        Err(error) => coordinator.replace_verification_required(
            credentials.role_id,
            persisted_display_bnet(target_path).as_deref(),
            &redact_sensitive_text(&error.to_string()),
        ),
    }
}

async fn bind_or_record_failure(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    detected_bnet: &str,
    target_path: &Path,
    root_base_url: &str,
) -> LoginTargetStatus {
    match bind_detected_target_with_base_url(credentials, detected_bnet, target_path, root_base_url)
        .await
    {
        Ok(saved) => coordinator.replace_ready(
            credentials.role_id,
            detected_bnet,
            saved
                .resolved_bnet
                .as_deref()
                .or(saved.input_bnet.as_deref())
                .unwrap_or(detected_bnet),
        ),
        Err(error) => coordinator.replace_bind_failed(
            credentials.role_id,
            detected_bnet,
            &redact_sensitive_text(&error.to_string()),
        ),
    }
}

fn normalize_bnet_for_compare(value: &str) -> String {
    value.trim().replace('＃', "#").to_lowercase()
}

fn target_display_bnet(target: &PlayerTarget) -> String {
    target
        .resolved_bnet
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| target.input_bnet.trim())
        .to_string()
}

fn persisted_display_bnet(path: &Path) -> Option<String> {
    read_player_target_for_path(path)
        .ok()
        .map(|target| target_display_bnet(&target))
}

fn status(
    state: &str,
    detected_bnet: Option<&str>,
    current_bnet: Option<&str>,
    message: &str,
    sync_allowed: bool,
    can_confirm_switch: bool,
    can_retry: bool,
) -> LoginTargetStatus {
    LoginTargetStatus {
        state: state.to_string(),
        detected_bnet: detected_bnet.map(str::to_string),
        current_bnet: current_bnet.map(str::to_string),
        message: message.to_string(),
        sync_allowed,
        can_confirm_switch,
        can_retry,
    }
}

fn save_target_atomic(path: &Path, target: &PlayerTarget) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp_path = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let write_result = (|| -> Result<()> {
        let mut file = std::fs::File::create(&temp_path)?;
        let json = serde_json::to_vec_pretty(target)?;
        file.write_all(&json)?;
        file.flush()?;
        file.sync_all()?;
        std::fs::rename(&temp_path, path).with_context(|| "保存当前账号目标失败")?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result
}
