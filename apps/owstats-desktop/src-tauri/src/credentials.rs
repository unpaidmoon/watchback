use crate::browser_session::{
    choose_debug_port, find_windows_browsers, launch_browser, open_or_find_page,
    wait_for_debug_endpoint, CdpPage,
};
use crate::dashen_auth::{
    build_collect_candidates_script, build_get_report_token_script, credentials_from_report_result,
    extract_role_id, is_signature_helper_missing,
};
use crate::login_flow::{LoginPhase, LoginProgress};
use crate::models::{CredentialLoginCancelResult, CredentialLoginResult, CredentialStatus};
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

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

pub fn credential_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(".dashen_credentials.json"))
}

pub fn get_credential_status(app: &AppHandle) -> Result<CredentialStatus> {
    credential_status_for_path(&credential_path(app)?)
}

pub fn clear_credentials(app: &AppHandle) -> Result<bool> {
    clear_credentials_for_path(&credential_path(app)?)
}

pub async fn start_dashen_login(app: AppHandle) -> Result<CredentialLoginResult> {
    LOGIN_CANCELLED.store(false, Ordering::SeqCst);

    emit_login_progress(
        &app,
        LoginPhase::LocatingBrowser,
        "正在查找 Chrome 或 Edge。",
    );
    let browser = select_browser(find_windows_browsers())?;
    let port = choose_debug_port(std::process::id());
    let profile_dir = app.path().app_data_dir()?.join("chrome-dashen-profile");

    emit_login_progress(&app, LoginPhase::OpeningBrowser, "正在打开网易大神登录页。");
    launch_browser(&browser, LOGIN_URL, port, &profile_dir)
        .map_err(|error| anyhow!(redacted_login_error(&error.to_string())))?;

    wait_for_debug_endpoint(port, Duration::from_secs(20)).await?;
    emit_login_progress(
        &app,
        LoginPhase::WaitingForLogin,
        "请在浏览器中完成登录，应用会自动识别登录状态。",
    );

    let credentials = fetch_credentials_from_browser(&app, port).await?;
    emit_login_progress(&app, LoginPhase::Saving, "正在保存本地登录凭证。");
    let path = credential_path(&app)?;
    save_credentials_atomic(&path, &credentials)?;
    emit_login_progress(&app, LoginPhase::Ready, "已保存登录凭证，可以开始同步。");

    Ok(CredentialLoginResult {
        status: credential_status_for_path(&path)?,
    })
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

async fn fetch_credentials_from_browser(app: &AppHandle, port: u16) -> Result<DashenCredentials> {
    let websocket_url = open_or_find_page(LOGIN_URL, port).await?;
    let mut page = CdpPage::connect(&websocket_url).await?;
    let deadline = Instant::now() + Duration::from_secs(LOGIN_TIMEOUT_SECONDS);
    let mut last_role_id: Option<String> = None;

    while Instant::now() < deadline {
        if LOGIN_CANCELLED.load(Ordering::SeqCst) {
            emit_login_progress(app, LoginPhase::Cancelled, "已取消登录等待。");
            return Err(anyhow!("已取消登录等待。"));
        }

        if let Some(role_id) = collect_role_id(&mut page).await {
            if last_role_id.as_deref() != Some(role_id.as_str()) {
                emit_login_progress(
                    app,
                    LoginPhase::RoleFound,
                    "已识别玩家身份，正在换取同步凭证。",
                );
            }
            last_role_id = Some(role_id);
        }

        if let Some(role_id) = &last_role_id {
            let result = page
                .evaluate(&build_get_report_token_script(role_id, SERVER_ID), true)
                .await?;
            if let Some(credentials) = credentials_from_report_result(
                &result,
                role_id,
                SERVER_ID,
                &Utc::now().to_rfc3339(),
            )? {
                return Ok(credentials);
            }

            if is_signature_helper_missing(&result) {
                emit_login_progress(
                    app,
                    LoginPhase::LoadingSignPage,
                    "正在打开签名页面以完成凭证换取。",
                );
                let script = format!(
                    "location.href = {}; true",
                    serde_json::to_string(SIGN_PAGE_URL)?
                );
                let _ = page.evaluate(&script, false).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
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
    if last_role_id.is_some() {
        Err(anyhow!("已识别玩家身份，但在超时前未能换取同步凭证。"))
    } else {
        Err(anyhow!(
            "登录超时，未能识别玩家身份。请确认已完成登录并打开网易大神守望先锋页面。"
        ))
    }
}

async fn collect_role_id(page: &mut CdpPage) -> Option<String> {
    let candidates = page
        .evaluate(&build_collect_candidates_script(), false)
        .await
        .ok()?;
    let texts = candidates
        .as_array()?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    extract_role_id(&texts)
}
