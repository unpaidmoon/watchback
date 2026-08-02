use crate::app_paths::{AccountPaths, SharedAppPaths};
use crate::browser_session::{
    choose_debug_port, find_existing_page, find_windows_browsers, launch_isolated_browser,
    open_new_page, open_or_find_page, wait_for_debug_endpoint, CdpPage,
};
use crate::dashen_auth::{
    build_collect_candidates_script, build_get_report_token_script, credentials_from_report_result,
    extract_battle_tag_candidates, extract_login_role_id, extract_role_id,
    extract_role_id_for_battle_tag, is_signature_helper_missing, BattleTagExtraction,
    RoleIdExtraction,
};
use crate::login_flow::{LoginPhase, LoginProgressReporter};
use crate::models::{CredentialLoginCancelResult, CredentialStatus};
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::AppHandle;

pub use crate::browser_session::{select_browser, BrowserCandidate};
pub use crate::credential_store::{credential_status_for_path, DashenCredentials};

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
    pub(crate) detected_battle_tag: String,
}

pub fn get_credential_status_for_account(paths: &AccountPaths) -> Result<CredentialStatus> {
    credential_status_for_path(&paths.credentials())
}

pub fn read_saved_credentials_for_account(paths: &AccountPaths) -> Result<DashenCredentials> {
    crate::credential_store::read_credentials(&paths.credentials())
}

pub fn read_optional_saved_credentials_for_account(
    paths: &AccountPaths,
) -> Result<Option<DashenCredentials>> {
    if !paths.credentials().exists() {
        return Ok(None);
    }
    read_saved_credentials_for_account(paths).map(Some)
}

pub(crate) async fn capture_dashen_login(
    app: AppHandle,
    shared_paths: &SharedAppPaths,
) -> Result<BrowserLoginOutcome> {
    LOGIN_CANCELLED.store(false, Ordering::SeqCst);

    emit_login_progress(
        &app,
        LoginPhase::LocatingBrowser,
        "正在查找 Chrome 或 Edge。",
    );
    let browser = select_browser(find_windows_browsers())?;
    let port = choose_debug_port(std::process::id());

    emit_login_progress(&app, LoginPhase::OpeningBrowser, "正在打开网易大神登录页。");
    let browser_session = launch_isolated_browser(&browser, LOGIN_URL, port, shared_paths.root())
        .map_err(|error| anyhow!(error.to_string()))?;

    wait_for_debug_endpoint(port, Duration::from_secs(20)).await?;
    emit_login_progress(
        &app,
        LoginPhase::WaitingForLogin,
        "请在浏览器中完成登录，应用会自动识别登录状态。",
    );

    let result = capture_dashen_login_with(|| fetch_credentials_from_browser(&app, port)).await;
    drop(browser_session);
    result
}

pub(crate) async fn capture_dashen_login_with<F, Fut>(runner: F) -> Result<BrowserLoginOutcome>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<BrowserLoginOutcome>>,
{
    let outcome = runner().await.map_err(|error| anyhow!(error.to_string()))?;
    if outcome.credentials.role_id <= 0 || outcome.credentials.token.trim().is_empty() {
        return Err(anyhow!("登录凭证不完整，请重新添加账号。"));
    }
    let battle_tag = outcome.detected_battle_tag.trim();
    let valid_battle_tag = battle_tag.split_once('#').is_some_and(|(name, suffix)| {
        !name.is_empty()
            && !suffix.is_empty()
            && !suffix.contains('#')
            && suffix.chars().all(|character| character.is_ascii_digit())
    });
    if !valid_battle_tag {
        return Err(anyhow!("未能稳定识别 BattleTag，请重新添加账号。"));
    }
    Ok(outcome)
}

pub(crate) fn login_cancelled() -> bool {
    LOGIN_CANCELLED.load(Ordering::SeqCst)
}

pub fn cancel_dashen_login(_app: AppHandle) -> Result<CredentialLoginCancelResult> {
    LOGIN_CANCELLED.store(true, Ordering::SeqCst);
    Ok(CredentialLoginCancelResult {
        cancelled: true,
        message: "已取消登录等待。".to_string(),
    })
}

pub fn emit_login_progress(app: &AppHandle, phase: LoginPhase, message: &str) {
    use crate::login_flow::LoginProgressReporter;
    app.report(phase, message);
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

pub(crate) async fn fetch_credentials_from_browser(
    progress: &impl LoginProgressReporter,
    port: u16,
) -> Result<BrowserLoginOutcome> {
    fetch_credentials_from_browser_with_cancel(progress, port, login_cancelled).await
}

pub(crate) async fn fetch_credentials_from_browser_with_cancel<C>(
    progress: &impl LoginProgressReporter,
    port: u16,
    cancelled: C,
) -> Result<BrowserLoginOutcome>
where
    C: Fn() -> bool,
{
    let websocket_url = open_or_find_page(LOGIN_URL, port)
        .await
        .map_err(|_| anyhow!("credential_login_page_failed"))?;
    let mut page = CdpPage::connect(&websocket_url)
        .await
        .map_err(|_| anyhow!("credential_login_connect_failed"))?;
    page.add_script_to_evaluate_on_new_document(&build_collect_candidates_script())
        .await?;
    let deadline = Instant::now() + Duration::from_secs(LOGIN_TIMEOUT_SECONDS);
    let mut identity = LoginCaptureIdentity::default();
    let mut locked_credential_role_id: Option<String> = None;
    let mut locked_paired_identity: Option<(String, String)> = None;
    let mut last_token_failure = "凭证接口请求未完成";
    let mut reloaded_authenticated_login = false;

    while Instant::now() < deadline {
        if cancelled() {
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
                progress.report(LoginPhase::RoleFound, "已识别玩家身份，正在换取同步凭证。");
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
            let result = tokio::time::timeout(
                Duration::from_secs(20),
                page.evaluate(&build_get_report_token_script(&role_id, SERVER_ID), true),
            )
            .await
            .map_err(|_| anyhow!("credential_token_request_timeout"))?
            .map_err(|_| anyhow!("credential_token_evaluate_failed"))?;
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
                if let Some(detected_battle_tag) = detected_battle_tag {
                    return Ok(BrowserLoginOutcome {
                        credentials,
                        detected_battle_tag,
                    });
                }
                progress.report(
                    LoginPhase::WaitingForLogin,
                    "已取得同步凭证，正在稳定识别 BattleTag。",
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            last_token_failure = report_token_failure_category(&result);
            if is_signature_helper_missing(&result) {
                if locked_credential_role_id.is_none() {
                    let Some(locked_identity) =
                        paired_identity.filter(|(paired_role_id, _)| paired_role_id == &role_id)
                    else {
                        last_token_failure = "BattleTag 尚未稳定";
                        progress.report(
                            LoginPhase::WaitingForLogin,
                            "已识别玩家身份，正在稳定识别 BattleTag。",
                        );
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    };
                    locked_paired_identity = Some(locked_identity);
                    locked_credential_role_id = Some(role_id);
                    progress.report(
                        LoginPhase::LoadingSignPage,
                        "正在打开签名页面以完成凭证换取。",
                    );
                    let websocket_url = open_new_page(SIGN_PAGE_URL, port)
                        .await
                        .map_err(|_| anyhow!("credential_sign_target_failed"))?;
                    page = CdpPage::connect(&websocket_url)
                        .await
                        .map_err(|_| anyhow!("credential_sign_connect_failed"))?;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            } else {
                let detail = result
                    .get("error")
                    .and_then(|item| item.as_str())
                    .unwrap_or("凭证暂未就绪");
                progress.report(LoginPhase::TokenPending, &format!("凭证暂未就绪：{detail}"));
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        } else {
            if !reloaded_authenticated_login
                && find_existing_page(SIGN_PAGE_URL, port)
                    .await
                    .map_err(|_| anyhow!("credential_auth_target_check_failed"))?
                    .is_some()
            {
                let login_url = serde_json::to_string(LOGIN_URL)
                    .map_err(|_| anyhow!("credential_login_url_failed"))?;
                page.evaluate(&format!("location.replace({login_url})"), false)
                    .await
                    .map_err(|_| anyhow!("credential_login_reload_failed"))?;
                reloaded_authenticated_login = true;
                progress.report(
                    LoginPhase::WaitingForLogin,
                    "已确认扫码，正在识别登录账号。",
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            progress.report(
                LoginPhase::WaitingForLogin,
                "请完成登录并进入守望先锋相关页面。",
            );
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    if locked_credential_role_id.is_some() || identity.credential_role_id().is_some() {
        Err(anyhow!(
            "已识别玩家身份，但在超时前未能换取同步凭证（{last_token_failure}）。"
        ))
    } else {
        Err(anyhow!(
            "登录超时，未能识别玩家身份。请确认已完成登录并打开网易大神守望先锋页面。"
        ))
    }
}

fn report_token_failure_category(result: &serde_json::Value) -> &'static str {
    if is_signature_helper_missing(result) {
        return "签名页面尚未就绪";
    }
    let status = result.get("status").and_then(serde_json::Value::as_u64);
    if matches!(status, Some(401 | 403)) {
        return "凭证接口拒绝当前登录状态";
    }
    if result
        .get("error")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|error| error.contains("non-JSON response"))
    {
        return "凭证接口返回非预期响应";
    }
    if status.is_some_and(|status| (200..300).contains(&status)) {
        return "凭证接口未返回 token";
    }
    "凭证接口请求未完成"
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
    let battle_tag = extract_battle_tag_candidates(&battle_tag_candidates, battle_tag_truncated);
    let matched_role = match &battle_tag {
        BattleTagExtraction::Found(battle_tag) => {
            extract_role_id_for_battle_tag(&credential_candidates, battle_tag)
        }
        BattleTagExtraction::Missing => RoleIdExtraction::Missing,
        BattleTagExtraction::Conflict => RoleIdExtraction::Conflict,
    };
    let fallback_credential_role = extract_role_id(&credential_candidates);
    let (credential_role_id, paired_role_id) = match matched_role {
        RoleIdExtraction::Found(role_id) => {
            (Some(role_id.clone()), RoleIdExtraction::Found(role_id))
        }
        RoleIdExtraction::Conflict => (fallback_credential_role, RoleIdExtraction::Conflict),
        RoleIdExtraction::Missing => (
            fallback_credential_role,
            extract_login_role_id(
                &current_candidates,
                &storage_candidates,
                &resource_candidates,
                current_truncated || storage_truncated || resource_truncated,
            ),
        ),
    };
    Some((credential_role_id, paired_role_id, battle_tag))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct LiveProgress {
        phases: Mutex<Vec<LoginPhase>>,
    }

    impl LoginProgressReporter for LiveProgress {
        fn report(&self, phase: LoginPhase, _message: &str) {
            self.phases.lock().unwrap().push(phase);
        }
    }

    #[tokio::test]
    async fn injectable_capture_runner_never_writes_a_final_credential_path() {
        let temp = tempfile::tempdir().unwrap();
        let sentinel = temp.path().join("final-credentials.json");
        std::fs::write(&sentinel, b"unchanged").unwrap();
        let outcome = BrowserLoginOutcome {
            credentials: DashenCredentials {
                role_id: 101101,
                token: "synthetic-secret".to_string(),
                customer_token: None,
                dts: None,
                server: 1,
                saved_at: "2026-07-16T00:00:00Z".to_string(),
            },
            detected_battle_tag: "Synthetic#1011".to_string(),
        };

        let captured = capture_dashen_login_with(|| async { Ok(outcome) })
            .await
            .unwrap();
        assert_eq!(captured.detected_battle_tag, "Synthetic#1011");
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");

        let source = "cancelled token=synthetic-secret Authorization: Bearer synthetic-header cookie=synthetic-cookie";
        let cancelled = capture_dashen_login_with(|| async { Err(anyhow::anyhow!(source)) })
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(cancelled, source);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");

        let invalid = capture_dashen_login_with(|| async {
            Ok(BrowserLoginOutcome {
                credentials: DashenCredentials {
                    role_id: 0,
                    token: String::new(),
                    customer_token: None,
                    dts: None,
                    server: 1,
                    saved_at: "2026-07-16T00:00:00Z".to_string(),
                },
                detected_battle_tag: "unstable".to_string(),
            })
        })
        .await;
        assert!(invalid.is_err());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");
    }

    #[test]
    fn report_token_failures_are_reduced_to_safe_fixed_categories() {
        assert_eq!(
            report_token_failure_category(&serde_json::json!({
                "ok": false,
                "status": 401,
                "error": "private upstream detail"
            })),
            "凭证接口拒绝当前登录状态"
        );
        assert_eq!(
            report_token_failure_category(&serde_json::json!({
                "ok": false,
                "status": 200,
                "error": "getReportToken returned non-JSON response",
                "text": "private response body"
            })),
            "凭证接口返回非预期响应"
        );
        assert_eq!(
            report_token_failure_category(&serde_json::json!({
                "ok": false,
                "status": 200,
                "error": "private business message"
            })),
            "凭证接口未返回 token"
        );
    }

    #[tokio::test]
    #[ignore = "requires an already authenticated dedicated login browser"]
    async fn live_authenticated_browser_completes_capture_without_persisting_credentials() {
        let port: u16 = std::env::var("OWSTATS_LIVE_LOGIN_DEBUG_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .expect("set OWSTATS_LIVE_LOGIN_DEBUG_PORT to the dedicated browser port");
        let websocket_url = open_or_find_page(LOGIN_URL, port)
            .await
            .expect("dedicated login page should be available");
        let mut setup_page = CdpPage::connect(&websocket_url)
            .await
            .expect("dedicated login page should accept a CDP connection");
        setup_page
            .add_script_to_evaluate_on_new_document(&build_collect_candidates_script())
            .await
            .expect("transient identity collector should install");
        let login_url = serde_json::to_string(LOGIN_URL).unwrap();
        setup_page
            .evaluate(&format!("location.replace({login_url})"), false)
            .await
            .expect("authenticated login page should reload");

        let progress = LiveProgress::default();
        let capture = tokio::time::timeout(
            Duration::from_secs(30),
            fetch_credentials_from_browser(&progress, port),
        )
        .await;
        let outcome = match capture {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => {
                let last_phase = progress.phases.lock().unwrap().last().cloned();
                panic!("live capture failed at safe phase {last_phase:?}");
            }
            Err(_) => {
                let phases = progress.phases.lock().unwrap().clone();
                let identity_diagnostics = collect_login_identity(&mut setup_page)
                    .await
                    .map(|(credential_role_id, paired_role_id, battle_tag)| {
                        let credential_matches_pair =
                            match (credential_role_id.as_deref(), &paired_role_id) {
                                (
                                    Some(credential_role_id),
                                    RoleIdExtraction::Found(paired_role_id),
                                ) => Some(credential_role_id == paired_role_id),
                                _ => None,
                            };
                        serde_json::json!({
                            "credentialRolePresent": credential_role_id.is_some(),
                            "pairedRoleStatus": match paired_role_id {
                                RoleIdExtraction::Missing => "missing",
                                RoleIdExtraction::Found(_) => "found",
                                RoleIdExtraction::Conflict => "conflict",
                            },
                            "battleTagStatus": match battle_tag {
                                BattleTagExtraction::Missing => "missing",
                                BattleTagExtraction::Found(_) => "found",
                                BattleTagExtraction::Conflict => "conflict",
                            },
                            "credentialMatchesPair": credential_matches_pair,
                        })
                    })
                    .unwrap_or(serde_json::Value::Null);
                let credential_role_for_dom = collect_login_identity(&mut setup_page)
                    .await
                    .and_then(|(credential_role_id, _, _)| credential_role_id);
                let dom_relationship_diagnostics = if let Some(credential_role_id) =
                    credential_role_for_dom
                {
                    let credential_role_id = serde_json::to_string(&credential_role_id).unwrap();
                    setup_page
                        .evaluate(
                            &format!(
                                r#"(() => {{
                                      const credentialRole = {credential_role_id};
                                      const walker = document.createTreeWalker(
                                        document.body,
                                        NodeFilter.SHOW_TEXT
                                      );
                                      let labelNode = null;
                                      while (walker.nextNode()) {{
                                        if (String(walker.currentNode.nodeValue ?? '')
                                          .includes('战网昵称')) {{
                                          labelNode = walker.currentNode;
                                          break;
                                        }}
                                      }}
                                      const levels = [];
                                      let element = labelNode?.parentElement ?? null;
                                      for (let depth = 0; element && depth < 10; depth += 1) {{
                                        const text = element.innerText ?? '';
                                        const roleIds = new Set(
                                          [...text.matchAll(/role_?id\D{{0,12}}([0-9]{{5,}})/gi)]
                                            .map((match) => match[1])
                                        );
                                        levels.push({{
                                          depth,
                                          roleCount: roleIds.size,
                                          credentialPresent: roleIds.has(credentialRole),
                                          textLength: text.length,
                                        }});
                                        element = element.parentElement;
                                      }}
                                      return {{ labelNodePresent: Boolean(labelNode), levels }};
                                    }})()"#
                            ),
                            false,
                        )
                        .await
                        .unwrap_or(serde_json::Value::Null)
                } else {
                    serde_json::Value::Null
                };
                let diagnostics = setup_page
                    .evaluate(
                        r#"(() => {
                          const text = document.documentElement?.innerText ?? '';
                          const capture = window.__owstatsBattleTagCapture;
                          const storageValues = [];
                          for (const storage of [window.localStorage, window.sessionStorage]) {
                            for (let index = 0; index < storage.length; index += 1) {
                              const key = storage.key(index);
                              if (key) storageValues.push(storage.getItem(key) ?? '');
                            }
                          }
                          const battleTagPattern = /[^\s#]+#[0-9]+/g;
                          const pairedPattern = /role_?id[\s\S]{0,500}[^\s#]+#[0-9]+|[^\s#]+#[0-9]+[\s\S]{0,500}role_?id/i;
                          let shallowGlobalPatternCount = 0;
                          for (const key of Object.keys(window)) {
                            try {
                              const value = window[key];
                              const serialized = typeof value === 'string'
                                ? value
                                : JSON.stringify(value);
                              if (serialized && battleTagPattern.test(serialized.slice(0, 100000))) {
                                shallowGlobalPatternCount += 1;
                              }
                              battleTagPattern.lastIndex = 0;
                            } catch (_) {}
                          }
                          return {
                            host: location.hostname,
                            path: location.pathname,
                            readyState: document.readyState,
                            labelPresent: text.includes('战网昵称'),
                            bodyPatternCount: (text.match(battleTagPattern) ?? []).length,
                            capturePresent: Boolean(capture),
                            captureValueCount: Array.isArray(capture?.values)
                              ? capture.values.length
                              : 0,
                            captureObserverPresent: Boolean(capture?.observer),
                            storagePatternValueCount: storageValues.filter((value) =>
                              battleTagPattern.test(value)
                            ).length,
                            storagePairedValueCount: storageValues.filter((value) =>
                              pairedPattern.test(value)
                            ).length,
                            shallowGlobalPatternCount,
                          };
                        })()"#,
                        false,
                    )
                    .await
                    .unwrap_or(serde_json::Value::Null);
                panic!(
                    "live capture timed out at safe phases {phases:?}; identity diagnostics \
                     {identity_diagnostics}; DOM relationship diagnostics \
                     {dom_relationship_diagnostics}; safe diagnostics {diagnostics}"
                );
            }
        };
        assert!(outcome.credentials.role_id > 0);
        assert!(!outcome.credentials.token.trim().is_empty());
        assert!(outcome
            .detected_battle_tag
            .split_once('#')
            .is_some_and(|(name, suffix)| !name.is_empty()
                && !suffix.is_empty()
                && suffix.chars().all(|character| character.is_ascii_digit())));
    }
}
