//! Hidden-browser QR account login.  Its public surface deliberately contains
//! only session control and redacted events; CDP details and browser state stay
//! inside this module.

use crate::account_context::AccountContextManager;
use crate::account_login::{capture_and_publish_add, LoginCapture};
use crate::browser_session::{
    choose_available_qr_debug_port, find_windows_browsers, launch_isolated_hidden_browser,
    open_or_find_page, select_browser, wait_for_debug_endpoint, CdpPage,
};
use crate::credentials::{fetch_credentials_from_browser_with_cancel, BrowserLoginOutcome};
use crate::login_flow::{LoginPhase, LoginProgressReporter};
use crate::login_target::LoginTargetCoordinator;
use crate::models::AccountLoginResult;
use crate::sync_coordinator::OperationPermit;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::ImageFormat;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const LOGIN_URL: &str = "https://pay.ds.163.com/activity/ld5?addSkeleton=1&channel=yydc_cps10.cczx&setId=67b466a5b8cf7e3576aa3521";
const DASHEN_ROOT_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool";
const QR_MAX_BYTES: usize = 512 * 1024;

#[derive(Default, Clone)]
pub struct QrLoginCoordinator {
    active: Arc<Mutex<Option<ActiveQrLogin>>>,
}

#[derive(Clone)]
struct ActiveQrLogin {
    session_id: Uuid,
    refresh_generation: u64,
    cancel_requested: bool,
    phase: QrLoginPhase,
    message: String,
    revision: u64,
    image_base64: Option<String>,
    completion: Option<QrLoginResultEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginStartResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginControlResult {
    pub session_id: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrImageEvent {
    pub session_id: String,
    pub revision: u64,
    pub mime_type: &'static str,
    pub image_base64: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginProgressEvent {
    pub session_id: String,
    pub phase: QrLoginPhase,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QrLoginPhase {
    Starting,
    OpeningLogin,
    SelectingQrMode,
    AcceptingPrivacy,
    LoadingQr,
    WaitingForScan,
    Confirming,
    Capturing,
    Saving,
    Ready,
    Expired,
    Cancelling,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginResultEvent {
    pub session_id: String,
    pub result: Option<AccountLoginResult>,
    pub message: Option<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginSnapshot {
    pub session_id: String,
    pub phase: QrLoginPhase,
    pub message: String,
    pub revision: u64,
    pub image_base64: Option<String>,
    pub completion: Option<QrLoginResultEvent>,
}

impl QrLoginCoordinator {
    pub fn start(&self) -> Result<Uuid> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow!("二维码登录状态不可用"))?;
        if active
            .as_ref()
            .is_some_and(|item| item.completion.is_none())
        {
            return Err(anyhow!("二维码登录已在进行中"));
        }
        let session_id = Uuid::new_v4();
        *active = Some(ActiveQrLogin {
            session_id,
            refresh_generation: 0,
            cancel_requested: false,
            phase: QrLoginPhase::Starting,
            message: "正在准备二维码登录…".to_string(),
            revision: 0,
            image_base64: None,
            completion: None,
        });
        Ok(session_id)
    }

    pub fn refresh(&self, session_id: &str) -> Result<()> {
        let id = Uuid::parse_str(session_id).map_err(|_| anyhow!("二维码登录会话无效"))?;
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow!("二维码登录状态不可用"))?;
        let current = active
            .as_mut()
            .filter(|item| {
                item.session_id == id && !item.cancel_requested && item.completion.is_none()
            })
            .ok_or_else(|| anyhow!("二维码登录会话已结束或已过期"))?;
        current.refresh_generation = current.refresh_generation.saturating_add(1);
        current.image_base64 = None;
        current.phase = QrLoginPhase::LoadingQr;
        current.message = "正在刷新二维码…".to_string();
        Ok(())
    }

    pub fn cancel(&self, session_id: &str) -> Result<()> {
        let id = Uuid::parse_str(session_id).map_err(|_| anyhow!("二维码登录会话无效"))?;
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow!("二维码登录状态不可用"))?;
        let current = active
            .as_mut()
            .filter(|item| item.session_id == id)
            .ok_or_else(|| anyhow!("二维码登录会话已结束或已过期"))?;
        current.cancel_requested = true;
        Ok(())
    }

    fn control(&self, session_id: Uuid) -> Option<(u64, bool)> {
        self.active
            .lock()
            .ok()?
            .as_ref()
            .filter(|item| item.session_id == session_id)
            .map(|item| (item.refresh_generation, item.cancel_requested))
    }

    fn update_progress(&self, session_id: Uuid, phase: QrLoginPhase, message: &str) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(current) = active.as_mut().filter(|item| item.session_id == session_id) {
                current.phase = phase;
                current.message = message.to_string();
            }
        }
    }

    fn update_image(&self, session_id: Uuid, revision: u64, image_base64: String) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(current) = active.as_mut().filter(|item| item.session_id == session_id) {
                current.revision = revision;
                current.image_base64 = Some(image_base64);
            }
        }
    }

    fn complete(&self, session_id: Uuid, completion: QrLoginResultEvent) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(current) = active.as_mut().filter(|item| item.session_id == session_id) {
                current.completion = Some(completion);
            }
        }
    }

    pub fn snapshot(&self, session_id: &str) -> Result<QrLoginSnapshot> {
        let id = Uuid::parse_str(session_id).map_err(|_| anyhow!("二维码登录会话无效"))?;
        let active = self
            .active
            .lock()
            .map_err(|_| anyhow!("二维码登录状态不可用"))?;
        let current = active
            .as_ref()
            .filter(|item| item.session_id == id)
            .ok_or_else(|| anyhow!("二维码登录会话已结束或已过期"))?;
        Ok(QrLoginSnapshot {
            session_id: session_id.to_string(),
            phase: current.phase.clone(),
            message: current.message.clone(),
            revision: current.revision,
            image_base64: current.image_base64.clone(),
            completion: current.completion.clone(),
        })
    }
}

pub fn start_login(
    app: AppHandle,
    accounts: AccountContextManager,
    operation_permit: OperationPermit,
    coordinator: QrLoginCoordinator,
    login_target_coordinator: LoginTargetCoordinator,
) -> Result<QrLoginStartResult> {
    let session_id = coordinator.start()?;
    let session = session_id;
    tauri::async_runtime::spawn(async move {
        // The permit's lifetime is intentionally the entire detached login task.
        let permit = operation_permit;
        let completion = run_login(
            app.clone(),
            accounts,
            coordinator.clone(),
            login_target_coordinator,
            session,
        )
        .await;
        coordinator.complete(session, completion.clone());
        drop(permit);
        // A fallback to the visible login must never race this session's permit.
        let _ = app.emit("credential-login-qr-result", completion);
    });
    Ok(QrLoginStartResult {
        session_id: session_id.to_string(),
    })
}

pub fn refresh_login(
    coordinator: &QrLoginCoordinator,
    session_id: String,
) -> Result<QrLoginControlResult> {
    coordinator.refresh(&session_id)?;
    Ok(QrLoginControlResult {
        session_id,
        accepted: true,
    })
}

pub fn cancel_login(
    coordinator: &QrLoginCoordinator,
    session_id: String,
) -> Result<QrLoginControlResult> {
    coordinator.cancel(&session_id)?;
    Ok(QrLoginControlResult {
        session_id,
        accepted: true,
    })
}

async fn run_login(
    app: AppHandle,
    accounts: AccountContextManager,
    coordinator: QrLoginCoordinator,
    login_target_coordinator: LoginTargetCoordinator,
    session_id: Uuid,
) -> QrLoginResultEvent {
    let reporter = QrReporter {
        app: app.clone(),
        session_id,
        coordinator: coordinator.clone(),
    };
    let result =
        async {
            reporter.report_qr(QrLoginPhase::Starting, "正在准备二维码登录…");
            let browser = select_browser(find_windows_browsers())
                .map_err(|_| anyhow!("qr_browser_not_found"))?;
            loop {
                let (generation, cancelled) = coordinator
                    .control(session_id)
                    .ok_or_else(|| anyhow!("qr_cancelled"))?;
                if cancelled {
                    return Err(anyhow!("qr_cancelled"));
                }
                reporter.report_qr(QrLoginPhase::OpeningLogin, "正在打开登录页面…");
                let port = choose_available_qr_debug_port()
                    .map_err(|_| anyhow!("qr_browser_start_failed"))?;
                let browser_session = launch_isolated_hidden_browser(
                    &browser,
                    LOGIN_URL,
                    port,
                    accounts.shared_paths().root(),
                )
                .map_err(|_| anyhow!("qr_browser_start_failed"))?;
                let attempt = async {
                    wait_for_debug_endpoint(port, Duration::from_secs(20))
                        .await
                        .map_err(|_| anyhow!("qr_debug_endpoint_timeout"))?;
                    let websocket_url = open_or_find_page(LOGIN_URL, port)
                        .await
                        .map_err(|_| anyhow!("qr_page_target_failed"))?;
                    let mut page = CdpPage::connect(&websocket_url)
                        .await
                        .map_err(|_| anyhow!("qr_cdp_connect_failed"))?;
                    advance_to_qr(&mut page, &reporter).await?;
                    reporter.report_qr(QrLoginPhase::LoadingQr, "正在获取二维码…");
                    let png = fetch_qr_png(&mut page).await?;
                    reporter.emit_qr(png, generation)?;
                    reporter.report_qr(
                        QrLoginPhase::WaitingForScan,
                        "请使用网易大神扫描二维码并在手机上确认",
                    );
                    let capture_app = reporter.clone();
                    let outcome =
                        fetch_credentials_from_browser_with_cancel(&capture_app, port, || {
                            coordinator
                                .control(session_id)
                                .map(|(current, cancelled)| cancelled || current != generation)
                                .unwrap_or(true)
                        })
                        .await;
                    outcome
                }
                .await;
                drop(browser_session);
                match attempt {
                    Ok(outcome) => {
                        return publish_capture(
                            &accounts,
                            &reporter,
                            outcome,
                            &coordinator,
                            session_id,
                            generation,
                            &login_target_coordinator,
                        )
                        .await
                    }
                    Err(_)
                        if coordinator.control(session_id).is_some_and(
                            |(current, cancelled)| !cancelled && current != generation,
                        ) =>
                    {
                        continue
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        .await;
    match result {
        Ok(result) => {
            reporter.report_qr(QrLoginPhase::Ready, "账号添加成功");
            QrLoginResultEvent {
                session_id: session_id.to_string(),
                result: Some(result),
                message: None,
                cancelled: false,
            }
        }
        Err(error) if error.to_string() == "qr_cancelled" => {
            reporter.report_qr(QrLoginPhase::Cancelled, "已取消二维码登录");
            QrLoginResultEvent {
                session_id: session_id.to_string(),
                result: None,
                message: None,
                cancelled: true,
            }
        }
        Err(error) => {
            let message = qr_failure_message(&error);
            reporter.report_qr(
                QrLoginPhase::Failed,
                "二维码登录暂不可用，请刷新或使用其他登录方式",
            );
            QrLoginResultEvent {
                session_id: session_id.to_string(),
                result: None,
                message: Some(message),
                cancelled: false,
            }
        }
    }
}

fn qr_failure_message(error: &anyhow::Error) -> String {
    const ALLOWED_CODES: &[&str] = &[
        "qr_browser_not_found",
        "qr_browser_start_failed",
        "qr_debug_endpoint_timeout",
        "qr_page_target_failed",
        "qr_cdp_connect_failed",
        "qr_login_button_missing",
        "qr_mode_switch_missing",
        "qr_image_missing",
        "qr_dom_fetch_failed",
        "qr_qr_bounds_failed",
        "qr_screenshot_failed",
        "qr_image_invalid",
        "qr_capture_timeout",
        "credential_login_target_failed",
        "credential_login_connect_failed",
        "credential_token_evaluate_failed",
        "credential_token_request_timeout",
        "credential_auth_target_check_failed",
        "credential_login_url_failed",
        "credential_login_reload_failed",
        "credential_sign_target_failed",
        "credential_sign_connect_failed",
        "qr_publish_failed",
    ];
    let raw = error.to_string();
    let code = ALLOWED_CODES
        .iter()
        .copied()
        .find(|candidate| raw == *candidate)
        .unwrap_or("qr_internal_failure");
    if cfg!(debug_assertions) {
        format!("二维码登录暂不可用，请刷新或使用其他登录方式（调试码：{code}）")
    } else {
        "二维码登录暂不可用，请刷新或使用其他登录方式".to_string()
    }
}

async fn publish_capture(
    accounts: &AccountContextManager,
    reporter: &QrReporter,
    outcome: BrowserLoginOutcome,
    coordinator: &QrLoginCoordinator,
    session_id: Uuid,
    generation: u64,
    login_target_coordinator: &LoginTargetCoordinator,
) -> Result<AccountLoginResult> {
    reporter.report_qr(QrLoginPhase::Capturing, "正在验证登录信息…");
    let result = capture_and_publish_add(
        accounts,
        reporter,
        || async move {
            Ok(LoginCapture {
                credentials: outcome.credentials,
                battle_tag: outcome.detected_battle_tag,
            })
        },
        || {
            coordinator
                .control(session_id)
                .map(|(current, cancelled)| cancelled || current != generation)
                .unwrap_or(true)
        },
    )
    .await
    .map_err(|_| anyhow!("qr_publish_failed"))?;
    reporter.report_qr(QrLoginPhase::Saving, "正在安全保存账号登录信息…");
    // This is exactly the post-publication target reconciliation used by the visible flow.
    reconcile_target(accounts, login_target_coordinator, &result).await?;
    Ok(result)
}

async fn reconcile_target(
    accounts: &AccountContextManager,
    coordinator: &LoginTargetCoordinator,
    result: &AccountLoginResult,
) -> Result<()> {
    if !matches!(
        result.kind.as_str(),
        "newAccountAdded" | "existingAccountUpdated"
    ) {
        return Ok(());
    }
    let account_id = result
        .account_id
        .as_deref()
        .and_then(|value| crate::account_store::AccountId::parse(value).ok())
        .ok_or_else(|| anyhow!("账号登录结果不完整，请重试"))?;
    let account = accounts.snapshot_for(&account_id)?;
    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())?;
    let target_path = crate::player_target::player_target_path_for_account(account.paths());
    crate::login_target::reconcile_detected_target(
        coordinator,
        account.account_id(),
        &credentials,
        result.battle_tag.as_deref(),
        &target_path,
        DASHEN_ROOT_BASE,
    )
    .await;
    let _ = crate::profile_card::refresh_for_account(&account, DASHEN_ROOT_BASE).await;
    Ok(())
}

async fn advance_to_qr(page: &mut CdpPage, reporter: &QrReporter) -> Result<()> {
    reporter.report_qr(QrLoginPhase::OpeningLogin, "正在打开网易登录窗口…");
    wait_click(page, MAIN_LOGIN_CLICK, None, "qr_login_button_missing").await?;
    reporter.report_qr(QrLoginPhase::SelectingQrMode, "正在切换扫码登录…");
    wait_click(page, QR_MODE_CLICK, None, "qr_mode_switch_missing").await?;
    reporter.report_qr(QrLoginPhase::LoadingQr, "正在准备扫码登录…");
    // A fresh isolated profile cannot inherit the phone-side acknowledgement.
    // Keep this entirely internal: dismiss the first-party gate only when it is
    // actually present, while the app itself never asks the user to tick it.
    for _ in 0..35 {
        if page
            .evaluate(QR_READY, false)
            .await
            .unwrap_or(Value::Bool(false))
            == Value::Bool(true)
        {
            return Ok(());
        }
        let _ = page.evaluate(PRIVACY_CLICK, false).await;
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    Err(anyhow!("qr_image_missing"))
}

async fn wait_click(
    page: &mut CdpPage,
    script: &str,
    context_id: Option<i64>,
    error: &str,
) -> Result<()> {
    for _ in 0..20 {
        if page
            .evaluate_in_context(script, context_id, false)
            .await
            .unwrap_or(Value::Bool(false))
            == Value::Bool(true)
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(350)).await;
    }
    Err(anyhow!("{error}"))
}

async fn fetch_qr_png(page: &mut CdpPage) -> Result<Vec<u8>> {
    for _ in 0..10 {
        let result = page
            .evaluate(QR_FETCH, true)
            .await
            .unwrap_or_else(|_| json!({ "state": "retry" }));
        if result.get("state").and_then(Value::as_str) == Some("ready") {
            let mime = result
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let Some(encoded) = result.get("base64").and_then(Value::as_str) {
                if let Ok(png) = normalise_qr_png(mime, encoded) {
                    return Ok(png);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    // A DOM image is present but its source fetch can occasionally be blocked by
    // an upstream change. Capture only that element's validated square bounds,
    // never the whole login page.
    let bounds = page
        .evaluate(QR_BOUNDS, false)
        .await
        .map_err(|_| anyhow!("qr_qr_bounds_failed"))?;
    let x = bounds
        .get("x")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("qr_dom_fetch_failed"))?;
    let y = bounds
        .get("y")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("qr_dom_fetch_failed"))?;
    let width = bounds
        .get("width")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("qr_dom_fetch_failed"))?;
    let height = bounds
        .get("height")
        .and_then(Value::as_f64)
        .ok_or_else(|| anyhow!("qr_dom_fetch_failed"))?;
    if !(64.0..=1024.0).contains(&width)
        || !(64.0..=1024.0).contains(&height)
        || (width - height).abs() > width.max(height) / 20.0
    {
        return Err(anyhow!("qr_image_invalid"));
    }
    let shot = page.call("Page.captureScreenshot", json!({ "format": "png", "clip": { "x": x, "y": y, "width": width, "height": height, "scale": 2.0 }, "captureBeyondViewport": false })).await.map_err(|_| anyhow!("qr_screenshot_failed"))?;
    let encoded = shot
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("qr_dom_fetch_failed"))?;
    normalise_qr_png("image/png", encoded)
}

fn normalise_qr_png(mime: &str, encoded: &str) -> Result<Vec<u8>> {
    if !matches!(mime, "image/jpeg" | "image/jpg" | "image/png") {
        return Err(anyhow!("qr_image_invalid"));
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| anyhow!("qr_image_invalid"))?;
    if bytes.is_empty() || bytes.len() > QR_MAX_BYTES {
        return Err(anyhow!("qr_image_invalid"));
    }
    let image = image::load_from_memory(&bytes).map_err(|_| anyhow!("qr_image_invalid"))?;
    let (width, height) = (image.width(), image.height());
    if !(128..=1024).contains(&width)
        || !(128..=1024).contains(&height)
        || width.abs_diff(height) > width.max(height) / 20
    {
        return Err(anyhow!("qr_image_invalid"));
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|_| anyhow!("qr_image_invalid"))?;
    Ok(png)
}

#[derive(Clone)]
struct QrReporter {
    app: AppHandle,
    session_id: Uuid,
    coordinator: QrLoginCoordinator,
}
impl QrReporter {
    fn report_qr(&self, phase: QrLoginPhase, message: &str) {
        self.coordinator
            .update_progress(self.session_id, phase.clone(), message);
        let _ = self.app.emit(
            "credential-login-qr-progress",
            QrLoginProgressEvent {
                session_id: self.session_id.to_string(),
                phase,
                message: message.to_string(),
            },
        );
    }
    fn emit_qr(&self, png: Vec<u8>, revision: u64) -> Result<()> {
        let _hash = Sha256::digest(&png); // no image bytes or source leave this scope except the event.
        let image_base64 = STANDARD.encode(png);
        self.coordinator
            .update_image(self.session_id, revision + 1, image_base64.clone());
        // Polling the coordinator snapshot is the reliable delivery path. Event
        // delivery is only an optimisation and must never abort QR login.
        let _ = self.app.emit(
            "credential-login-qr",
            QrImageEvent {
                session_id: self.session_id.to_string(),
                revision: revision + 1,
                mime_type: "image/png",
                image_base64,
            },
        );
        Ok(())
    }
}
impl LoginProgressReporter for QrReporter {
    fn report(&self, phase: LoginPhase, message: &str) {
        let qr_phase = match phase {
            LoginPhase::Saving => QrLoginPhase::Saving,
            LoginPhase::Ready => QrLoginPhase::Ready,
            LoginPhase::Failed => QrLoginPhase::Failed,
            LoginPhase::Cancelled => QrLoginPhase::Cancelled,
            LoginPhase::RoleFound | LoginPhase::LoadingSignPage | LoginPhase::TokenPending => {
                QrLoginPhase::Capturing
            }
            _ => QrLoginPhase::WaitingForScan,
        };
        self.report_qr(qr_phase, message);
    }
}

const MAIN_LOGIN_CLICK: &str = r#"(() => { const normalize = value => value.replace(/\s+/g, ''); const candidates = [...document.querySelectorAll('button[class*=exchangeBtn]')].filter(node => normalize(node.textContent || '') === '登录' && node instanceof HTMLElement && node.getClientRects().length > 0); if (candidates.length !== 1) return false; candidates[0].click(); return true; })()"#;
const QR_MODE_CLICK: &str = r#"(() => { const icons = [...document.querySelectorAll('.btn__login_type-login .icon_qrcode')].filter(node => node instanceof HTMLElement && node.getClientRects().length > 0); if (icons.length !== 1) return false; const target = icons[0].closest('.btn__login_type-login'); if (!(target instanceof HTMLElement)) return false; target.click(); return true; })()"#;
const PRIVACY_CLICK: &str = r#"(() => { const normalize = value => value.replace(/\s+/g, ''); const candidates = [...document.querySelectorAll('.protocol-display-popup .agree-continue')].filter(node => normalize(node.textContent || '') === '同意并继续' && node instanceof HTMLElement && node.getClientRects().length > 0); if (candidates.length !== 1) return false; candidates[0].click(); return true; })()"#;
const QR_READY: &str = r#"Boolean([...document.querySelectorAll('img.qrcode__image')].find((item) => { const rect = item.getBoundingClientRect(); const style = getComputedStyle(item); return item instanceof HTMLImageElement && item.complete && item.naturalWidth >= 64 && item.naturalHeight >= 64 && rect.width >= 64 && rect.height >= 64 && style.display !== 'none' && style.visibility !== 'hidden' && Number(style.opacity) !== 0; }))"#;
const QR_FETCH: &str = r#"(async () => { const image = [...document.querySelectorAll('img.qrcode__image')].find((item) => { const rect = item.getBoundingClientRect(); const style = getComputedStyle(item); return item instanceof HTMLImageElement && item.complete && item.naturalWidth >= 64 && item.naturalHeight >= 64 && rect.width >= 64 && rect.height >= 64 && style.display !== 'none' && style.visibility !== 'hidden' && Number(style.opacity) !== 0; }); if (!image) return { state: 'missing' }; try { await image.decode(); const response = await fetch(image.currentSrc, { credentials: 'include', cache: 'no-store' }); if (!response.ok) return { state: 'fetchFailed' }; const blob = await response.blob(); const bytes = new Uint8Array(await blob.arrayBuffer()); let binary = ''; for (let offset = 0; offset < bytes.length; offset += 0x8000) binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000)); return { state: 'ready', mimeType: blob.type, width: image.naturalWidth, height: image.naturalHeight, base64: btoa(binary) }; } catch (_) { return { state: 'fetchFailed' }; } })()"#;
const QR_BOUNDS: &str = r#"(() => { const image = [...document.querySelectorAll('img.qrcode__image')].find((item) => { const rect = item.getBoundingClientRect(); const style = getComputedStyle(item); return item instanceof HTMLImageElement && item.complete && item.naturalWidth >= 64 && item.naturalHeight >= 64 && rect.width >= 64 && rect.height >= 64 && style.display !== 'none' && style.visibility !== 'hidden' && Number(style.opacity) !== 0; }); if (!image) return null; const rect = image.getBoundingClientRect(); return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }; })()"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinator_rejects_stale_controls() {
        let coordinator = QrLoginCoordinator::default();
        let id = coordinator.start().unwrap();
        assert!(coordinator.refresh(&id.to_string()).is_ok());
        assert!(coordinator.refresh(&Uuid::new_v4().to_string()).is_err());
        coordinator.cancel(&id.to_string()).unwrap();
        assert!(coordinator.refresh(&id.to_string()).is_err());
    }

    #[test]
    fn failure_messages_only_expose_whitelisted_debug_codes() {
        let known = qr_failure_message(&anyhow!("qr_mode_switch_missing"));
        let unknown = qr_failure_message(&anyhow!("secret=https://example.invalid/?token=private"));
        if cfg!(debug_assertions) {
            assert!(known.contains("qr_mode_switch_missing"));
            assert!(unknown.contains("qr_internal_failure"));
        }
        assert!(!unknown.contains("example.invalid"));
        assert!(!unknown.contains("private"));
    }
}
