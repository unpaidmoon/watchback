use crate::browser_session::{
    choose_debug_port, find_windows_browsers, launch_browser, open_or_find_page,
    wait_for_debug_endpoint, CdpPage,
};
use crate::dashen_auth::{
    build_collect_candidates_script, build_get_report_token_script, credentials_from_report_result,
    extract_battle_tag_candidates, extract_login_role_id, extract_role_id,
    is_signature_helper_missing, BattleTagExtraction, RoleIdExtraction,
};
use crate::login_flow::{LoginPhase, LoginProgress};
use crate::models::{CredentialLoginCancelResult, CredentialStatus};
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

pub use crate::browser_session::{select_browser, BrowserCandidate};
pub use crate::credential_store::{
    clear_credentials_for_path, credential_status_for_path,
    redact_sensitive_text as redacted_login_error, save_credentials_atomic, DashenCredentials,
};

const LOGIN_URL: &str =
    "https://pay.ds.163.com/activity/ld5?addSkeleton=1&channel=yydc_cps10.cczx&setId=67b466a5b8cf7e3576aa3521";
const SIGN_PAGE_URL: &str = "https://ds.163.com/";
const LOGIN_TIMEOUT_SECONDS: u64 = 180;
const SERVER_ID: i64 = 1;

static LOGIN_CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoginIdentityTracker {
    role_id: Option<String>,
    battle_tag: Option<String>,
    stable_observations: u8,
}

impl LoginIdentityTracker {
    pub fn observe(&mut self, role_id: Option<String>, battle_tag: BattleTagExtraction) {
        match (role_id, battle_tag) {
            (Some(role_id), BattleTagExtraction::Found(battle_tag)) => {
                if self.role_id.as_deref() == Some(role_id.as_str())
                    && self.battle_tag.as_deref() == Some(battle_tag.as_str())
                {
                    self.stable_observations = self.stable_observations.saturating_add(1);
                } else {
                    self.role_id = Some(role_id);
                    self.battle_tag = Some(battle_tag);
                    self.stable_observations = 1;
                }
            }
            _ => {
                self.role_id = None;
                self.battle_tag = None;
                self.stable_observations = 0;
            }
        }
    }

    pub fn role_id(&self) -> Option<&str> {
        self.role_id.as_deref()
    }

    pub fn battle_tag(&self) -> Option<&str> {
        self.battle_tag.as_deref()
    }

    pub fn complete(&self) -> Option<(&str, &str)> {
        if self.stable_observations < 2 {
            return None;
        }
        self.role_id().zip(self.battle_tag())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoginCaptureIdentity {
    credential_role_id: Option<String>,
    paired_identity: LoginIdentityTracker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairedIdentityStatus {
    Unavailable,
    Pending,
    Confirmed { role_id: String, battle_tag: String },
}

impl LoginCaptureIdentity {
    pub fn observe(
        &mut self,
        credential_role_id: Option<String>,
        paired_role_id: RoleIdExtraction,
        battle_tag: BattleTagExtraction,
    ) {
        self.credential_role_id = credential_role_id;
        let paired_role_id = pairing_role_id(self.credential_role_id.as_deref(), paired_role_id);
        self.paired_identity.observe(paired_role_id, battle_tag);
    }

    pub fn credential_role_id(&self) -> Option<&str> {
        self.credential_role_id.as_deref()
    }

    pub fn confirmed_pair(&self) -> Option<(&str, &str)> {
        self.paired_identity.complete()
    }

    pub fn paired_status(&self) -> PairedIdentityStatus {
        if let Some((role_id, battle_tag)) = self.confirmed_pair() {
            return PairedIdentityStatus::Confirmed {
                role_id: role_id.to_string(),
                battle_tag: battle_tag.to_string(),
            };
        }
        if self.paired_identity.role_id().is_some() && self.paired_identity.battle_tag().is_some() {
            PairedIdentityStatus::Pending
        } else {
            PairedIdentityStatus::Unavailable
        }
    }

    pub fn confirm_pending_pair(
        &mut self,
        credential_role_id: Option<String>,
        paired_role_id: RoleIdExtraction,
        battle_tag: BattleTagExtraction,
    ) {
        if self.paired_status() != PairedIdentityStatus::Pending {
            return;
        }
        let paired_role_id = pairing_role_id(credential_role_id.as_deref(), paired_role_id);
        self.paired_identity.observe(paired_role_id, battle_tag);
        if !matches!(self.paired_status(), PairedIdentityStatus::Confirmed { .. }) {
            self.paired_identity
                .observe(None, BattleTagExtraction::Missing);
        }
    }

    pub fn detected_battle_tag(&self, credential_role_id: &str) -> Option<&str> {
        self.confirmed_pair()
            .and_then(|(role_id, battle_tag)| (role_id == credential_role_id).then_some(battle_tag))
    }
}

fn pairing_role_id(
    credential_role_id: Option<&str>,
    paired_role_id: RoleIdExtraction,
) -> Option<String> {
    match paired_role_id {
        RoleIdExtraction::Conflict => None,
        RoleIdExtraction::Found(role_id) => credential_role_id
            .is_none_or(|credential_role_id| credential_role_id == role_id)
            .then_some(role_id),
        RoleIdExtraction::Missing => credential_role_id.map(str::to_string),
    }
}

pub(crate) struct BrowserLoginOutcome {
    pub(crate) credentials: DashenCredentials,
    #[allow(dead_code)]
    pub(crate) detected_battle_tag: Option<String>,
}

pub fn credential_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(crate::app_paths::AppPaths::from_app(app)?.credentials())
}

pub fn get_credential_status(app: &AppHandle) -> Result<CredentialStatus> {
    credential_status_for_path(&credential_path(app)?)
}

pub fn clear_credentials(app: &AppHandle) -> Result<bool> {
    clear_credentials_for_path(&credential_path(app)?)
}

pub(crate) async fn start_dashen_login_capture(app: AppHandle) -> Result<BrowserLoginOutcome> {
    LOGIN_CANCELLED.store(false, Ordering::SeqCst);

    emit_login_progress(
        &app,
        LoginPhase::LocatingBrowser,
        "正在查找 Chrome 或 Edge。",
    );
    let browser = select_browser(find_windows_browsers())?;
    let port = choose_debug_port(std::process::id());
    let profile_dir = crate::app_paths::AppPaths::from_app(&app)?.browser_profile();

    emit_login_progress(&app, LoginPhase::OpeningBrowser, "正在打开网易大神登录页。");
    launch_browser(&browser, LOGIN_URL, port, &profile_dir)
        .map_err(|error| anyhow!(redacted_login_error(&error.to_string())))?;

    wait_for_debug_endpoint(port, Duration::from_secs(20)).await?;
    emit_login_progress(
        &app,
        LoginPhase::WaitingForLogin,
        "请在浏览器中完成登录，应用会自动识别登录状态。",
    );

    let outcome = fetch_credentials_from_browser(&app, port).await?;
    emit_login_progress(&app, LoginPhase::Saving, "正在保存本地登录凭证。");
    let path = credential_path(&app)?;
    save_credentials_atomic(&path, &outcome.credentials)?;
    emit_login_progress(&app, LoginPhase::Ready, "已保存登录凭证，可以开始同步。");

    Ok(outcome)
}

pub fn cancel_dashen_login(_app: AppHandle) -> Result<CredentialLoginCancelResult> {
    LOGIN_CANCELLED.store(true, Ordering::SeqCst);
    Ok(CredentialLoginCancelResult {
        cancelled: true,
        message: "已取消登录等待。".to_string(),
    })
}

pub fn emit_login_progress(app: &AppHandle, phase: LoginPhase, message: &str) {
    let _ = app.emit(
        "credential-login-progress",
        LoginProgress::new(phase, message),
    );
}

pub fn diagnostic_credentials(role_id: i64, token: String) -> DashenCredentials {
    DashenCredentials {
        role_id,
        token,
        customer_token: None,
        dts: None,
        server: SERVER_ID,
        saved_at: Utc::now().to_rfc3339(),
    }
}

async fn fetch_credentials_from_browser(app: &AppHandle, port: u16) -> Result<BrowserLoginOutcome> {
    let websocket_url = open_or_find_page(LOGIN_URL, port).await?;
    let mut page = CdpPage::connect(&websocket_url).await?;
    let deadline = Instant::now() + Duration::from_secs(LOGIN_TIMEOUT_SECONDS);
    let mut identity = LoginCaptureIdentity::default();
    let mut locked_credential_role_id: Option<String> = None;
    let mut locked_paired_identity: Option<(String, String)> = None;

    while Instant::now() < deadline {
        if LOGIN_CANCELLED.load(Ordering::SeqCst) {
            emit_login_progress(app, LoginPhase::Cancelled, "已取消登录等待。");
            return Err(anyhow!("已取消登录等待。"));
        }

        if locked_credential_role_id.is_none() {
            let previous_role_id = identity.credential_role_id().map(str::to_string);
            let (credential_role_id, paired_role_id, battle_tag) =
                collect_login_identity(&mut page).await.unwrap_or((
                    None,
                    RoleIdExtraction::Missing,
                    BattleTagExtraction::Missing,
                ));
            identity.observe(credential_role_id, paired_role_id, battle_tag);
            if identity.paired_status() == PairedIdentityStatus::Pending {
                let confirmation = collect_login_identity(&mut page).await;
                match confirmation {
                    Some((credential_role_id, paired_role_id, battle_tag)) => {
                        identity.confirm_pending_pair(
                            credential_role_id,
                            paired_role_id,
                            battle_tag,
                        );
                    }
                    None => identity.confirm_pending_pair(
                        None,
                        RoleIdExtraction::Missing,
                        BattleTagExtraction::Missing,
                    ),
                }
            }
            if identity.credential_role_id().is_some()
                && identity.credential_role_id() != previous_role_id.as_deref()
            {
                emit_login_progress(
                    app,
                    LoginPhase::RoleFound,
                    "已识别玩家身份，正在换取同步凭证。",
                );
            }
        }

        let request_role_id = locked_credential_role_id
            .clone()
            .or_else(|| identity.credential_role_id().map(str::to_string));

        if let Some(role_id) = request_role_id {
            let paired_identity = if locked_credential_role_id.is_some() {
                locked_paired_identity.clone()
            } else {
                identity
                    .confirmed_pair()
                    .map(|(role_id, battle_tag)| (role_id.to_string(), battle_tag.to_string()))
            };
            let result = page
                .evaluate(&build_get_report_token_script(&role_id, SERVER_ID), true)
                .await?;
            if let Some(credentials) = credentials_from_report_result(
                &result,
                &role_id,
                SERVER_ID,
                &Utc::now().to_rfc3339(),
            )? {
                if credentials.role_id.to_string() != role_id {
                    return Err(anyhow!("凭证角色与当前登录角色不一致。"));
                }
                let detected_battle_tag =
                    paired_identity.and_then(|(paired_role_id, battle_tag)| {
                        (paired_role_id == role_id).then_some(battle_tag)
                    });
                return Ok(BrowserLoginOutcome {
                    credentials,
                    detected_battle_tag,
                });
            }

            if is_signature_helper_missing(&result) {
                if locked_credential_role_id.is_none() {
                    locked_paired_identity =
                        paired_identity.filter(|(paired_role_id, _)| paired_role_id == &role_id);
                    locked_credential_role_id = Some(role_id);
                    emit_login_progress(
                        app,
                        LoginPhase::LoadingSignPage,
                        "正在打开签名页面以完成凭证换取。",
                    );
                    let script = format!(
                        "location.href = {}; true",
                        serde_json::to_string(SIGN_PAGE_URL)?
                    );
                    page.evaluate(&script, false).await?;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            } else {
                let detail = result
                    .get("error")
                    .and_then(|item| item.as_str())
                    .unwrap_or("凭证暂未就绪");
                emit_login_progress(
                    app,
                    LoginPhase::TokenPending,
                    &format!("凭证暂未就绪：{}", redacted_login_error(detail)),
                );
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        } else {
            emit_login_progress(
                app,
                LoginPhase::WaitingForLogin,
                "请完成登录并进入守望先锋相关页面。",
            );
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    emit_login_progress(app, LoginPhase::Failed, "登录超时，未能完成凭证换取。");
    if locked_credential_role_id.is_some() || identity.credential_role_id().is_some() {
        Err(anyhow!("已识别玩家身份，但在超时前未能换取同步凭证。"))
    } else {
        Err(anyhow!(
            "登录超时，未能识别玩家身份。请确认已完成登录并打开网易大神守望先锋页面。"
        ))
    }
}

async fn collect_login_identity(
    page: &mut CdpPage,
) -> Option<(Option<String>, RoleIdExtraction, BattleTagExtraction)> {
    let collected = page
        .evaluate(&build_collect_candidates_script(), false)
        .await
        .ok()?;
    let (battle_tag_candidates, battle_tag_truncated) =
        candidate_group(collected.get("battleTagCandidates")?)?;
    let role_candidates = collected.get("roleCandidates")?;
    let (current_candidates, current_truncated) = candidate_group(role_candidates.get("current")?)?;
    let (storage_candidates, storage_truncated) = candidate_group(role_candidates.get("storage")?)?;
    let (resource_candidates, resource_truncated) =
        candidate_group(role_candidates.get("resources")?)?;
    let credential_candidates = current_candidates
        .iter()
        .chain(&storage_candidates)
        .chain(&resource_candidates)
        .cloned()
        .collect::<Vec<_>>();
    Some((
        extract_role_id(&credential_candidates),
        extract_login_role_id(
            &current_candidates,
            &storage_candidates,
            &resource_candidates,
            current_truncated || storage_truncated || resource_truncated,
        ),
        extract_battle_tag_candidates(&battle_tag_candidates, battle_tag_truncated),
    ))
}

fn candidate_group(value: &serde_json::Value) -> Option<(Vec<String>, bool)> {
    Some((
        string_candidates(value.get("values")?)?,
        value.get("truncated")?.as_bool()?,
    ))
}

fn string_candidates(value: &serde_json::Value) -> Option<Vec<String>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect(),
    )
}
