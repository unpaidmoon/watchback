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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static TARGET_TEMP_NONCE: AtomicU64 = AtomicU64::new(0);

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
    generation: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
struct LoginTargetSession {
    source_role_id: i64,
    detected_bnet: Option<String>,
    status: LoginTargetStatus,
}

#[derive(Debug, Clone)]
struct BindingTicket {
    generation: u64,
    source_role_id: i64,
    detected_bnet: String,
    expected_state: &'static str,
}

impl LoginTargetCoordinator {
    pub fn clear(&self) {
        let mut guard = self.session.lock().expect("login target session poisoned");
        self.next_generation();
        *guard = None;
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
        self.next_generation();
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
        {
            let guard = self.session.lock().expect("login target session poisoned");
            if let Some(session) = guard.as_ref() {
                if credentials
                    .map(|credentials| credentials.role_id == session.source_role_id)
                    .unwrap_or(false)
                {
                    return session.status.clone();
                }
                return verification_status(
                    session.status.current_bnet.as_deref(),
                    "当前登录凭据与内存目标状态不匹配，请重新识别。",
                );
            }
        }

        let Some(credentials) = credentials else {
            return verification_status(
                persisted_display_bnet(target_path).as_deref(),
                "请先登录并重新识别当前账号。",
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

    fn begin_existing_binding(&self, role_id: i64, required_state: &str) -> Result<BindingTicket> {
        let mut guard = self
            .session
            .lock()
            .map_err(|_| anyhow!("登录目标状态不可用"))?;
        let session = guard
            .as_mut()
            .filter(|session| session.source_role_id == role_id)
            .filter(|session| session.status.state == required_state)
            .ok_or_else(|| anyhow!("本轮没有可用的已识别账号"))?;
        let detected_bnet = session
            .detected_bnet
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("本轮没有可用的已识别账号"))?;
        let current_bnet = session.status.current_bnet.clone();
        let generation = self.next_generation();
        session.status = status(
            "binding",
            Some(&detected_bnet),
            current_bnet.as_deref(),
            "正在验证并绑定当前登录账号。",
            false,
            false,
            false,
        );
        Ok(BindingTicket {
            generation,
            source_role_id: role_id,
            detected_bnet,
            expected_state: "binding",
        })
    }

    fn begin_new_binding(
        &self,
        role_id: i64,
        detected_bnet: &str,
        current_bnet: Option<&str>,
    ) -> BindingTicket {
        let mut guard = self.session.lock().expect("login target session poisoned");
        let generation = self.next_generation();
        *guard = Some(LoginTargetSession {
            source_role_id: role_id,
            detected_bnet: Some(detected_bnet.to_string()),
            status: status(
                "binding",
                Some(detected_bnet),
                current_bnet,
                "正在验证并绑定当前登录账号。",
                false,
                false,
                false,
            ),
        });
        BindingTicket {
            generation,
            source_role_id: role_id,
            detected_bnet: detected_bnet.to_string(),
            expected_state: "binding",
        }
    }

    fn commit_target(
        &self,
        ticket: &BindingTicket,
        target_path: &Path,
        target: &PlayerTarget,
    ) -> LoginTargetStatus {
        let mut guard = self.session.lock().expect("login target session poisoned");
        if !self.ticket_matches(ticket, guard.as_ref()) {
            return stale_operation_status();
        }
        if let Err(error) = save_target_atomic(target_path, target) {
            let failed = bind_failed_status(
                &ticket.detected_bnet,
                &redact_sensitive_text(&error.to_string()),
            );
            self.next_generation();
            *guard = Some(LoginTargetSession {
                source_role_id: ticket.source_role_id,
                detected_bnet: Some(ticket.detected_bnet.clone()),
                status: failed.clone(),
            });
            return failed;
        }

        let current_bnet = target_display_bnet(target);
        let ready = ready_status(&ticket.detected_bnet, &current_bnet);
        self.next_generation();
        *guard = Some(LoginTargetSession {
            source_role_id: ticket.source_role_id,
            detected_bnet: Some(ticket.detected_bnet.clone()),
            status: ready.clone(),
        });
        ready
    }

    fn commit_failure(&self, ticket: &BindingTicket, error: &anyhow::Error) -> LoginTargetStatus {
        let mut guard = self.session.lock().expect("login target session poisoned");
        if !self.ticket_matches(ticket, guard.as_ref()) {
            return stale_operation_status();
        }
        let failed = bind_failed_status(
            &ticket.detected_bnet,
            &redact_sensitive_text(&error.to_string()),
        );
        self.next_generation();
        *guard = Some(LoginTargetSession {
            source_role_id: ticket.source_role_id,
            detected_bnet: Some(ticket.detected_bnet.clone()),
            status: failed.clone(),
        });
        failed
    }

    fn ticket_matches(&self, ticket: &BindingTicket, session: Option<&LoginTargetSession>) -> bool {
        self.generation.load(Ordering::SeqCst) == ticket.generation
            && session
                .map(|session| {
                    session.source_role_id == ticket.source_role_id
                        && session.status.state == ticket.expected_state
                        && session.detected_bnet.as_deref() == Some(ticket.detected_bnet.as_str())
                })
                .unwrap_or(false)
    }

    fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn replace(
        &self,
        source_role_id: i64,
        detected_bnet: Option<String>,
        status: LoginTargetStatus,
    ) -> LoginTargetStatus {
        let mut guard = self.session.lock().expect("login target session poisoned");
        self.next_generation();
        *guard = Some(LoginTargetSession {
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
    let target =
        resolve_detected_target_with_base_url(credentials, detected_bnet, root_base_url).await?;
    save_target_atomic(target_path, &target)?;
    player_target_status_for_path(target_path)
}

async fn resolve_detected_target_with_base_url(
    credentials: &DashenCredentials,
    detected_bnet: &str,
    root_base_url: &str,
) -> Result<PlayerTarget> {
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
    ensure_business_success(&payload, "搜索当前账号")?;
    let target = target_from_search_payload(
        detected_bnet,
        &payload,
        credentials.role_id,
        &Utc::now().to_rfc3339(),
    )?;
    if target
        .resolved_bnet
        .as_deref()
        .map(|resolved| !same_bnet(resolved, detected_bnet))
        .unwrap_or(false)
    {
        return Err(anyhow!("搜索结果与识别账号不一致"));
    }
    let card = client.query_card(&target.customer_token).await?;
    ensure_business_success(&card, "读取资料卡")?;
    ensure_card_identity_matches(&card, detected_bnet)?;
    Ok(target)
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
            let ticket = coordinator.begin_new_binding(credentials.role_id, detected_bnet, None);
            resolve_and_commit(coordinator, credentials, target_path, root_base_url, ticket).await
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
            let current_bnet = target_display_bnet(target);
            let ticket = coordinator.begin_new_binding(
                credentials.role_id,
                detected_bnet,
                Some(&current_bnet),
            );
            resolve_and_commit(coordinator, credentials, target_path, root_base_url, ticket).await
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
    match coordinator.begin_existing_binding(credentials.role_id, "switchRequired") {
        Ok(ticket) => {
            resolve_and_commit(coordinator, credentials, target_path, root_base_url, ticket).await
        }
        Err(_) => verification_status(
            persisted_display_bnet(target_path).as_deref(),
            "本轮没有可用的待确认账号，请重新识别。",
        ),
    }
}

pub async fn retry_binding(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    target_path: &Path,
    root_base_url: &str,
) -> LoginTargetStatus {
    match coordinator.begin_existing_binding(credentials.role_id, "bindFailed") {
        Ok(ticket) => {
            resolve_and_commit(coordinator, credentials, target_path, root_base_url, ticket).await
        }
        Err(_) => verification_status(
            persisted_display_bnet(target_path).as_deref(),
            "本轮没有可重试的已识别账号，请重新识别。",
        ),
    }
}

async fn resolve_and_commit(
    coordinator: &LoginTargetCoordinator,
    credentials: &DashenCredentials,
    target_path: &Path,
    root_base_url: &str,
    ticket: BindingTicket,
) -> LoginTargetStatus {
    match resolve_detected_target_with_base_url(credentials, &ticket.detected_bnet, root_base_url)
        .await
    {
        Ok(target) => coordinator.commit_target(&ticket, target_path, &target),
        Err(error) => coordinator.commit_failure(&ticket, &error),
    }
}

fn normalize_bnet_for_compare(value: &str) -> String {
    value.trim().replace('＃', "#").to_lowercase()
}

fn same_bnet(left: &str, right: &str) -> bool {
    normalize_bnet_for_compare(left) == normalize_bnet_for_compare(right)
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

fn ensure_business_success(payload: &serde_json::Value, operation: &str) -> Result<()> {
    let succeeded = payload.get("code").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
    }) == Some(0);
    if succeeded {
        Ok(())
    } else {
        Err(anyhow!("{operation}返回业务错误"))
    }
}

fn ensure_card_identity_matches(payload: &serde_json::Value, detected_bnet: &str) -> Result<()> {
    let Some(data) = payload.get("data").and_then(serde_json::Value::as_object) else {
        return Ok(());
    };
    for key in [
        "bnetId",
        "bnet",
        "battleTag",
        "battletag",
        "BattleTag",
        "name",
    ] {
        let Some(candidate) = data
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|candidate| looks_like_battletag(candidate))
        else {
            continue;
        };
        if !same_bnet(candidate, detected_bnet) {
            return Err(anyhow!("资料卡账号与识别账号不一致"));
        }
    }
    Ok(())
}

fn looks_like_battletag(value: &str) -> bool {
    let normalized = value.trim().replace('＃', "#");
    let mut parts = normalized.split('#');
    let name = parts.next().unwrap_or_default();
    let suffix = parts.next().unwrap_or_default();
    !name.is_empty()
        && !suffix.is_empty()
        && suffix.chars().all(|character| character.is_ascii_digit())
        && parts.next().is_none()
}

fn ready_status(detected_bnet: &str, current_bnet: &str) -> LoginTargetStatus {
    status(
        "ready",
        Some(detected_bnet),
        Some(current_bnet),
        "已识别并绑定当前登录账号，可以同步赛季数据。",
        true,
        false,
        false,
    )
}

fn bind_failed_status(detected_bnet: &str, message: &str) -> LoginTargetStatus {
    status(
        "bindFailed",
        Some(detected_bnet),
        None,
        &format!("自动绑定失败：{}", redact_sensitive_text(message)),
        false,
        false,
        true,
    )
}

fn verification_status(current_bnet: Option<&str>, message: &str) -> LoginTargetStatus {
    status(
        "verificationRequired",
        None,
        current_bnet,
        message,
        false,
        false,
        false,
    )
}

fn stale_operation_status() -> LoginTargetStatus {
    verification_status(
        None,
        "登录账号已发生变化，本次旧操作未提交。请使用当前账号重新识别。",
    )
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
    let temp_path = target_temp_path(path);
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

fn target_temp_path(path: &Path) -> PathBuf {
    let nonce = TARGET_TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!("json.{}.{}.tmp", std::process::id(), nonce))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_target_temp_paths_are_unique_within_one_process() {
        let target = Path::new("target.json");

        let first = target_temp_path(target);
        let second = target_temp_path(target);

        assert_ne!(first, second);
        assert!(first.to_string_lossy().ends_with(".tmp"));
        assert!(second.to_string_lossy().ends_with(".tmp"));
    }
}
