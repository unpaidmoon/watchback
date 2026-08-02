//! Hidden-browser QR account login.  Its public surface deliberately contains
//! only session control and redacted events; CDP details and browser state stay
//! inside this module.

use crate::account_context::AccountContextManager;
use crate::account_login::{capture_verify_and_publish_add, LoginCapture};
use crate::browser_session::{
    choose_available_qr_debug_port, find_windows_browsers, launch_isolated_hidden_browser,
    open_or_find_page, select_browser, wait_for_debug_endpoint, BrowserLoginSessionOwner, CdpPage,
    CdpPageOwner,
};
use crate::credentials::{fetch_credentials_from_browser_with_cancel, BrowserLoginOutcome};
use crate::login_flow::{LoginPhase, LoginProgressReporter};
use crate::models::AccountLoginResult;
use crate::sync_coordinator::OperationPermit;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::ImageFormat;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::future::Future;
use std::io::Cursor;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;
use uuid::Uuid;

const LOGIN_URL: &str = "https://pay.ds.163.com/activity/ld5?addSkeleton=1&channel=yydc_cps10.cczx&setId=67b466a5b8cf7e3576aa3521";
const DASHEN_ROOT_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool";
const QR_MAX_BYTES: usize = 512 * 1024;
const QR_CANCEL_GRACE: Duration = if cfg!(test) {
    Duration::from_millis(25)
} else {
    Duration::from_secs(2)
};
const QR_CANCEL_WAIT_BUDGET: Duration = if cfg!(test) {
    Duration::from_secs(1)
} else {
    Duration::from_secs(8)
};
const QR_CDP_CLOSE_BUDGET: Duration = if cfg!(test) {
    Duration::from_millis(25)
} else {
    Duration::from_secs(1)
};
const QR_BROWSER_SHUTDOWN_BUDGET: Duration = if cfg!(test) {
    Duration::from_millis(25)
} else {
    Duration::from_secs(2)
};
const QR_ADAPTER_SHUTDOWN_BUDGET: Duration = if cfg!(test) {
    Duration::from_millis(100)
} else {
    Duration::from_secs(4)
};
const QR_SESSION_DEADLINE: Duration = if cfg!(test) {
    Duration::from_millis(75)
} else {
    Duration::from_secs(190)
};
const QR_HISTORY_LIMIT: usize = 4;
const QR_HISTORY_TTL: Duration = if cfg!(test) {
    Duration::from_millis(250)
} else {
    Duration::from_secs(10 * 60)
};

#[derive(Default, Clone)]
pub struct QrLoginCoordinator {
    active: Arc<Mutex<Option<ActiveQrLogin>>>,
    history: Arc<Mutex<VecDeque<HistoricalQrLogin>>>,
}

#[derive(Clone)]
struct HistoricalQrLogin {
    session: ActiveQrLogin,
    completed_at: Instant,
}

#[derive(Clone)]
struct ActiveQrLogin {
    request_id: String,
    session_id: Uuid,
    refresh_generation: u64,
    cancel_requested: bool,
    phase: QrLoginPhase,
    message: String,
    revision: u64,
    image_base64: Option<String>,
    completion: Option<QrLoginResultEvent>,
    committed_result: Option<AccountLoginResult>,
    released: bool,
    predecessor_session_id: Option<Uuid>,
    finished: Arc<Notify>,
    cancellation: Arc<Notify>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginStartResult {
    pub request_id: String,
    pub session_id: String,
    pub reused: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginControlResult {
    pub session_id: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginSettleResult {
    pub session_id: String,
    pub terminal_phase: QrLoginPhase,
    pub released: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginCommandError {
    pub code: &'static str,
    pub message: String,
}

impl QrLoginCommandError {
    pub fn from_error(error: anyhow::Error) -> Self {
        let raw = error.to_string();
        let code = match raw.as_str() {
            "qr_busy" => "qr_busy",
            "qr_not_found" => "qr_not_found",
            "qr_timeout" => "qr_timeout",
            _ => "qr_internal_failure",
        };
        let message = match code {
            "qr_busy" => "二维码登录正在进行中",
            "qr_not_found" => "二维码登录会话不存在或已过期",
            "qr_timeout" => "二维码登录操作超时，请重试",
            _ => "二维码登录暂不可用，请重试",
        };
        Self {
            code,
            message: message.to_string(),
        }
    }
}

enum StartDecision {
    New {
        session_id: Uuid,
        permit: OperationPermit,
    },
    Reused {
        session_id: Uuid,
    },
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
    pub error_code: Option<&'static str>,
    pub cancelled: bool,
    #[serde(skip)]
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrLoginSnapshot {
    pub request_id: String,
    pub session_id: String,
    pub generation: u64,
    pub phase: QrLoginPhase,
    pub message: String,
    pub revision: u64,
    pub image_base64: Option<String>,
    pub completion: Option<QrLoginResultEvent>,
    pub settled: bool,
    pub released: bool,
}

impl QrLoginCoordinator {
    pub fn recover_request(&self, request_id: &str) -> Option<QrLoginStartResult> {
        let active = self.active.lock().ok()?;
        let current = active
            .as_ref()
            .filter(|item| item.request_id == request_id)?;
        Some(QrLoginStartResult {
            request_id: current.request_id.clone(),
            session_id: current.session_id.to_string(),
            reused: true,
        })
    }

    pub fn start(&self) -> Result<Uuid> {
        self.start_request(Uuid::new_v4().to_string())
            .map(|(session_id, _)| session_id)
    }

    pub fn start_request(&self, request_id: String) -> Result<(Uuid, bool)> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow!("二维码登录状态不可用"))?;
        if let Some(current) = active.as_ref() {
            if current.request_id == request_id {
                return Ok((current.session_id, true));
            }
            if current.completion.is_none() {
                return Err(anyhow!("二维码登录已在进行中"));
            }
            self.archive_locked(current.clone());
        }
        let session_id = Uuid::new_v4();
        *active = Some(ActiveQrLogin {
            request_id,
            session_id,
            refresh_generation: 0,
            cancel_requested: false,
            phase: QrLoginPhase::Starting,
            message: "正在准备二维码登录…".to_string(),
            revision: 0,
            image_base64: None,
            completion: None,
            committed_result: None,
            released: false,
            predecessor_session_id: None,
            finished: Arc::new(Notify::new()),
            cancellation: Arc::new(Notify::new()),
        });
        Ok((session_id, false))
    }

    fn start_or_recover(
        &self,
        request_id: String,
        operations: &crate::sync_coordinator::SyncCoordinator,
    ) -> Result<StartDecision> {
        self.start_or_recover_successor(request_id, None, 0, operations)
    }

    fn start_or_recover_successor(
        &self,
        request_id: String,
        predecessor: Option<Uuid>,
        generation: u64,
        operations: &crate::sync_coordinator::SyncCoordinator,
    ) -> Result<StartDecision> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow!("qr_internal_failure"))?;
        if let Some(current) = active.as_ref() {
            let same_start = predecessor.is_none()
                && current.request_id == request_id
                && current.predecessor_session_id.is_none();
            let same_refresh = predecessor.is_some()
                && current.request_id == request_id
                && current.predecessor_session_id == predecessor;
            if same_start || same_refresh {
                return Ok(StartDecision::Reused {
                    session_id: current.session_id,
                });
            }
        }
        // A retried IPC start must recover its recent terminal session even if
        // another logical request has since become active. Check history while
        // holding the active lock so the decision stays atomic with replacement.
        {
            let mut history = self
                .history
                .lock()
                .map_err(|_| anyhow!("qr_internal_failure"))?;
            let now = Instant::now();
            history.retain(|item| now.duration_since(item.completed_at) <= QR_HISTORY_TTL);
            let recovered = history.iter().find(|item| {
                item.session.request_id == request_id
                    && match predecessor {
                        Some(expected) => item.session.predecessor_session_id == Some(expected),
                        None => item.session.predecessor_session_id.is_none(),
                    }
            });
            if let Some(recovered) = recovered {
                return Ok(StartDecision::Reused {
                    session_id: recovered.session.session_id,
                });
            }
        }
        if let Some(current) = active.as_ref() {
            if current.completion.is_none() {
                return Err(anyhow!("qr_busy"));
            }
            self.archive_locked(current.clone());
        }
        let permit = operations
            .try_acquire_account_operation()
            .map_err(|_| anyhow!("qr_busy"))?;
        let session_id = Uuid::new_v4();
        *active = Some(ActiveQrLogin {
            request_id,
            session_id,
            refresh_generation: generation,
            cancel_requested: false,
            phase: QrLoginPhase::Starting,
            message: "正在准备二维码登录…".to_string(),
            revision: 0,
            image_base64: None,
            completion: None,
            committed_result: None,
            released: false,
            predecessor_session_id: predecessor,
            finished: Arc::new(Notify::new()),
            cancellation: Arc::new(Notify::new()),
        });
        Ok(StartDecision::New { session_id, permit })
    }

    fn archive_locked(&self, mut session: ActiveQrLogin) {
        session.image_base64 = None;
        if let Ok(mut history) = self.history.lock() {
            let now = Instant::now();
            history.retain(|item| now.duration_since(item.completed_at) <= QR_HISTORY_TTL);
            history.push_front(HistoricalQrLogin {
                session,
                completed_at: now,
            });
            history.truncate(QR_HISTORY_LIMIT);
        }
    }

    fn successor_context(&self, session_id: &str) -> Result<(Uuid, String, u64)> {
        let id = Uuid::parse_str(session_id).map_err(|_| anyhow!("qr_not_found"))?;
        let active = self
            .active
            .lock()
            .map_err(|_| anyhow!("qr_internal_failure"))?;
        if let Some(current) = active.as_ref().filter(|item| item.session_id == id) {
            return Ok((
                id,
                current.request_id.clone(),
                current.refresh_generation + 1,
            ));
        }
        drop(active);
        let history = self
            .history
            .lock()
            .map_err(|_| anyhow!("qr_internal_failure"))?;
        let current = history
            .iter()
            .find(|item| item.session.session_id == id)
            .ok_or_else(|| anyhow!("qr_not_found"))?;
        Ok((
            id,
            current.session.request_id.clone(),
            current.session.refresh_generation + 1,
        ))
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
        let cancellation = Arc::clone(&current.cancellation);
        if current.completion.is_none() {
            current.phase = QrLoginPhase::Cancelling;
            current.message = "正在取消二维码登录…".to_string();
        }
        drop(active);
        cancellation.notify_waiters();
        Ok(())
    }

    /// Requests cancellation and does not return until the session has reached
    /// its terminal state. Session finalisation releases the account-operation
    /// permit before publishing that terminal state.
    pub async fn cancel_and_wait(&self, session_id: &str) -> Result<()> {
        let id = Uuid::parse_str(session_id).map_err(|_| anyhow!("二维码登录会话无效"))?;
        let (finished, cancellation) = {
            let mut active = self
                .active
                .lock()
                .map_err(|_| anyhow!("二维码登录状态不可用"))?;
            let Some(current) = active.as_mut().filter(|item| item.session_id == id) else {
                drop(active);
                let snapshot = self.snapshot(session_id)?;
                return if snapshot.settled && snapshot.released {
                    Ok(())
                } else {
                    Err(anyhow!("qr_not_found"))
                };
            };
            current.cancel_requested = true;
            if current.completion.is_some() && current.released {
                return Ok(());
            }
            current.phase = QrLoginPhase::Cancelling;
            current.message = "正在取消二维码登录…".to_string();
            (
                Arc::clone(&current.finished),
                Arc::clone(&current.cancellation),
            )
        };
        cancellation.notify_waiters();

        tokio::time::timeout(QR_CANCEL_WAIT_BUDGET, async {
            let notified = finished.notified();
            // A replacement may archive this terminal session before the
            // waiter observes the notification. Snapshot spans active and
            // history, preventing a valid replacement from causing timeout.
            if self
                .snapshot(session_id)
                .is_ok_and(|snapshot| snapshot.settled && snapshot.released)
            {
                return Ok(());
            }
            // `finished` is notified only after completion and permit release.
            // Registering before the snapshot closes the missed-wakeup window.
            notified.await;
            Ok(())
        })
        .await
        .map_err(|_| anyhow!("qr_timeout"))?
    }

    fn cancellation(&self, session_id: Uuid) -> Option<Arc<Notify>> {
        self.active
            .lock()
            .ok()?
            .as_ref()
            .filter(|item| item.session_id == session_id)
            .map(|item| Arc::clone(&item.cancellation))
    }

    fn control(&self, session_id: Uuid) -> Option<(u64, bool)> {
        self.active
            .lock()
            .ok()?
            .as_ref()
            .filter(|item| item.session_id == session_id)
            .map(|item| (item.refresh_generation, item.cancel_requested))
    }

    fn mark_committed(&self, session_id: Uuid, result: AccountLoginResult) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(current) = active.as_mut().filter(|item| item.session_id == session_id) {
                current.committed_result = Some(result);
            }
        }
    }

    fn committed_result(&self, session_id: Uuid) -> Option<AccountLoginResult> {
        self.active
            .lock()
            .ok()?
            .as_ref()
            .filter(|item| item.session_id == session_id)
            .and_then(|item| item.committed_result.clone())
    }

    fn update_progress(&self, session_id: Uuid, phase: QrLoginPhase, message: &str) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(current) = active.as_mut().filter(|item| item.session_id == session_id) {
                current.phase = phase;
                current.message = message.to_string();
                current.revision = current.revision.saturating_add(1);
            }
        }
    }

    fn update_image(&self, session_id: Uuid, image_base64: String) -> Option<u64> {
        if let Ok(mut active) = self.active.lock() {
            if let Some(current) = active.as_mut().filter(|item| item.session_id == session_id) {
                current.revision = current.revision.saturating_add(1);
                current.image_base64 = Some(image_base64);
                return Some(current.revision);
            }
        }
        None
    }

    fn complete(&self, session_id: Uuid, completion: QrLoginResultEvent) {
        let finished = if let Ok(mut active) = self.active.lock() {
            if let Some(current) = active.as_mut().filter(|item| item.session_id == session_id) {
                current.phase = if completion.cancelled {
                    QrLoginPhase::Cancelled
                } else if completion.expired {
                    QrLoginPhase::Expired
                } else if completion.result.is_some() {
                    QrLoginPhase::Ready
                } else {
                    QrLoginPhase::Failed
                };
                current.image_base64 = None;
                current.revision = current.revision.saturating_add(1);
                current.completion = Some(completion);
                current.committed_result = None;
                current.released = true;
                Some(Arc::clone(&current.finished))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(finished) = finished {
            finished.notify_waiters();
        }
    }

    pub fn snapshot(&self, session_id: &str) -> Result<QrLoginSnapshot> {
        let id = Uuid::parse_str(session_id).map_err(|_| anyhow!("二维码登录会话无效"))?;
        let active = self
            .active
            .lock()
            .map_err(|_| anyhow!("二维码登录状态不可用"))?;
        let historical;
        let current = if let Some(current) = active.as_ref().filter(|item| item.session_id == id) {
            current
        } else {
            drop(active);
            let mut history = self
                .history
                .lock()
                .map_err(|_| anyhow!("qr_internal_failure"))?;
            let now = Instant::now();
            history.retain(|item| now.duration_since(item.completed_at) <= QR_HISTORY_TTL);
            historical = history
                .iter()
                .find(|item| item.session.session_id == id)
                .map(|item| item.session.clone())
                .ok_or_else(|| anyhow!("qr_not_found"))?;
            return Ok(Self::snapshot_of(&historical));
        };
        Ok(Self::snapshot_of(current))
    }

    fn snapshot_of(current: &ActiveQrLogin) -> QrLoginSnapshot {
        QrLoginSnapshot {
            request_id: current.request_id.clone(),
            session_id: current.session_id.to_string(),
            generation: current.refresh_generation,
            phase: current.phase.clone(),
            message: current.message.clone(),
            revision: current.revision,
            image_base64: current.image_base64.clone(),
            completion: current.completion.clone(),
            settled: current.completion.is_some(),
            released: current.released,
        }
    }

    pub fn active_for_request(&self, _request_id: &str) -> Option<QrLoginSnapshot> {
        let active = self.active.lock().ok()?;
        active
            .as_ref()
            .filter(|item| item.completion.is_none())
            .map(Self::snapshot_of)
    }
}

type QrLoginRunFuture = Pin<Box<dyn Future<Output = QrLoginResultEvent> + Send + 'static>>;
type QrLoginShutdownFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Internal seam between lifecycle supervision and the browser/CDP pipeline.
///
/// The supervisor deliberately learns only how to run and shut down an adapter.
/// It alone owns terminalisation and the account-operation permit, so every
/// adapter outcome (including panic and forced abort) follows the same barrier.
trait QrLoginAdapter: Send + Sync + 'static {
    fn run(&self) -> QrLoginRunFuture;
    fn shutdown(&self) -> QrLoginShutdownFuture;
}

#[derive(Clone)]
struct ChromiumQrLoginAdapter {
    app: AppHandle,
    accounts: AccountContextManager,
    coordinator: QrLoginCoordinator,
    session_id: Uuid,
    resources: BrowserLoginSessionOwner,
    cdp: CdpPageOwner,
}

impl QrLoginAdapter for ChromiumQrLoginAdapter {
    fn run(&self) -> QrLoginRunFuture {
        let app = self.app.clone();
        let accounts = self.accounts.clone();
        let coordinator = self.coordinator.clone();
        let session_id = self.session_id;
        let resources = self.resources.clone();
        let cdp = self.cdp.clone();
        Box::pin(
            async move { run_login(app, accounts, coordinator, session_id, resources, cdp).await },
        )
    }

    fn shutdown(&self) -> QrLoginShutdownFuture {
        let resources = self.resources.clone();
        let cdp = self.cdp.clone();
        Box::pin(async move {
            let _ = cdp.close(QR_CDP_CLOSE_BUDGET).await;
            let _ = resources.shutdown(QR_BROWSER_SHUTDOWN_BUDGET).await;
        })
    }
}

async fn execute_adapter_session<F>(
    operation_permit: OperationPermit,
    coordinator: QrLoginCoordinator,
    session_id: Uuid,
    adapter: Arc<dyn QrLoginAdapter>,
    publish_terminal: F,
) where
    F: FnOnce(&QrLoginResultEvent) -> Result<()> + Send + 'static,
{
    let worker_adapter = Arc::clone(&adapter);
    let worker = tauri::async_runtime::spawn(async move { worker_adapter.run().await });
    let completion = supervise_session(&coordinator, session_id, worker, adapter.as_ref()).await;
    // The terminal snapshot is also the public release barrier: no observer can
    // see `released=true` while the account-operation permit is still held.
    drop(operation_permit);
    coordinator.complete(session_id, completion.clone());
    // Emission is intentionally after terminalisation. A closed window or event
    // transport failure cannot strand the session or re-acquire its permit.
    let _ = publish_terminal(&completion);
}

pub fn start_login(
    app: AppHandle,
    accounts: AccountContextManager,
    operations: &crate::sync_coordinator::SyncCoordinator,
    coordinator: QrLoginCoordinator,
    request_id: String,
) -> Result<QrLoginStartResult> {
    let decision = coordinator.start_or_recover(request_id.clone(), operations)?;
    let (session_id, operation_permit) = match decision {
        StartDecision::Reused { session_id } => {
            return Ok(QrLoginStartResult {
                request_id,
                session_id: session_id.to_string(),
                reused: true,
            })
        }
        StartDecision::New { session_id, permit } => (session_id, permit),
    };
    spawn_login(app, accounts, operation_permit, coordinator, session_id);
    Ok(QrLoginStartResult {
        request_id,
        session_id: session_id.to_string(),
        reused: false,
    })
}

fn spawn_login(
    app: AppHandle,
    accounts: AccountContextManager,
    operation_permit: OperationPermit,
    coordinator: QrLoginCoordinator,
    session_id: Uuid,
) {
    let adapter: Arc<dyn QrLoginAdapter> = Arc::new(ChromiumQrLoginAdapter {
        app: app.clone(),
        accounts,
        coordinator: coordinator.clone(),
        session_id,
        resources: BrowserLoginSessionOwner::default(),
        cdp: CdpPageOwner::default(),
    });
    tauri::async_runtime::spawn(async move {
        execute_adapter_session(
            operation_permit,
            coordinator,
            session_id,
            adapter,
            move |completion| {
                app.emit("credential-login-qr-result", completion.clone())?;
                Ok(())
            },
        )
        .await;
    });
}

pub async fn refresh_with_successor(
    app: AppHandle,
    accounts: AccountContextManager,
    operations: &crate::sync_coordinator::SyncCoordinator,
    coordinator: QrLoginCoordinator,
    session_id: String,
) -> Result<QrLoginStartResult> {
    let (predecessor, request_id, generation) = coordinator.successor_context(&session_id)?;
    if let Err(error) = coordinator.cancel_and_wait(&session_id).await {
        if error.to_string() != "qr_not_found" {
            return Err(error);
        }
    }
    let decision = coordinator.start_or_recover_successor(
        request_id.clone(),
        Some(predecessor),
        generation,
        operations,
    )?;
    match decision {
        StartDecision::Reused { session_id } => Ok(QrLoginStartResult {
            request_id,
            session_id: session_id.to_string(),
            reused: true,
        }),
        StartDecision::New { session_id, permit } => {
            spawn_login(app, accounts, permit, coordinator, session_id);
            Ok(QrLoginStartResult {
                request_id,
                session_id: session_id.to_string(),
                reused: false,
            })
        }
    }
}

async fn supervise_session(
    coordinator: &QrLoginCoordinator,
    session_id: Uuid,
    mut worker: tauri::async_runtime::JoinHandle<QrLoginResultEvent>,
    adapter: &dyn QrLoginAdapter,
) -> QrLoginResultEvent {
    let cancellation = coordinator
        .cancellation(session_id)
        .expect("a supervised QR session must still be active");
    let cancellation_requested = cancellation.notified();
    tokio::pin!(cancellation_requested);

    enum Outcome {
        Joined(Result<QrLoginResultEvent, tauri::Error>),
        Cancelled,
        Expired,
    }

    let outcome = if coordinator
        .control(session_id)
        .is_some_and(|(_, cancelled)| cancelled)
    {
        Outcome::Cancelled
    } else {
        tokio::select! {
            biased;
            result = &mut worker => Outcome::Joined(result),
            _ = &mut cancellation_requested => Outcome::Cancelled,
            _ = tokio::time::sleep(QR_SESSION_DEADLINE) => Outcome::Expired,
        }
    };

    let completion = match outcome {
        Outcome::Joined(Ok(completion)) => completion,
        Outcome::Joined(Err(_)) => failed_completion(session_id),
        Outcome::Cancelled | Outcome::Expired => {
            let expired = matches!(outcome, Outcome::Expired);
            match tokio::time::timeout(QR_CANCEL_GRACE, &mut worker).await {
                Ok(Ok(completion)) => completion,
                Ok(Err(_)) => coordinator
                    .committed_result(session_id)
                    .map(|result| committed_completion(session_id, result))
                    .unwrap_or_else(|| failed_completion(session_id)),
                Err(_) => {
                    worker.abort();
                    let _ = worker.await;
                    coordinator
                        .committed_result(session_id)
                        .map(|result| committed_completion(session_id, result))
                        .unwrap_or_else(|| {
                            if expired {
                                expired_completion(session_id)
                            } else {
                                cancelled_completion(session_id)
                            }
                        })
                }
            }
        }
    };
    // This is the single terminalization barrier. Even a panic or forced abort
    // cannot become externally terminal until process/profile ownership is empty.
    // Adapter implementations use bounded cleanup operations themselves; this
    // outer budget is the last-resort guard against a broken adapter future.
    let _ = tokio::time::timeout(QR_ADAPTER_SHUTDOWN_BUDGET, adapter.shutdown()).await;
    completion
}

fn cancelled_completion(session_id: Uuid) -> QrLoginResultEvent {
    QrLoginResultEvent {
        session_id: session_id.to_string(),
        result: None,
        message: None,
        error_code: None,
        cancelled: true,
        expired: false,
    }
}

fn expired_completion(session_id: Uuid) -> QrLoginResultEvent {
    QrLoginResultEvent {
        session_id: session_id.to_string(),
        result: None,
        message: Some(qr_failure_message(&anyhow!("qr_capture_timeout"))),
        error_code: Some("qr_expired"),
        cancelled: false,
        expired: true,
    }
}

fn failed_completion(session_id: Uuid) -> QrLoginResultEvent {
    QrLoginResultEvent {
        session_id: session_id.to_string(),
        result: None,
        message: Some(qr_failure_message(&anyhow!("qr_internal_failure"))),
        error_code: Some("qr_worker_failed"),
        cancelled: false,
        expired: false,
    }
}

#[cfg(test)]
fn error_completion(session_id: Uuid, error: &anyhow::Error) -> QrLoginResultEvent {
    QrLoginResultEvent {
        session_id: session_id.to_string(),
        result: None,
        message: Some(qr_failure_message(error)),
        error_code: Some(qr_failure_code(error)),
        cancelled: false,
        expired: false,
    }
}

fn committed_completion(session_id: Uuid, mut result: AccountLoginResult) -> QrLoginResultEvent {
    crate::account_login::add_post_commit_warning(
        &mut result,
        crate::account_login::POST_COMMIT_REFRESH_WARNING,
    );
    QrLoginResultEvent {
        session_id: session_id.to_string(),
        result: Some(result),
        message: None,
        error_code: None,
        cancelled: false,
        expired: false,
    }
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

pub async fn cancel_login(
    coordinator: &QrLoginCoordinator,
    session_id: String,
) -> Result<QrLoginSettleResult> {
    coordinator.cancel_and_wait(&session_id).await?;
    let snapshot = coordinator.snapshot(&session_id)?;
    Ok(QrLoginSettleResult {
        session_id,
        terminal_phase: snapshot.phase,
        released: snapshot.released,
    })
}

async fn run_login(
    app: AppHandle,
    accounts: AccountContextManager,
    coordinator: QrLoginCoordinator,
    session_id: Uuid,
    resources: BrowserLoginSessionOwner,
    cdp: CdpPageOwner,
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
                resources
                    .register(browser_session)
                    .map_err(|_| anyhow!("qr_browser_start_failed"))?;
                let attempt = async {
                    wait_for_debug_endpoint(port, Duration::from_secs(20))
                        .await
                        .map_err(|_| anyhow!("qr_debug_endpoint_timeout"))?;
                    let websocket_url = open_or_find_page(LOGIN_URL, port)
                        .await
                        .map_err(|_| anyhow!("qr_page_target_failed"))?;
                    let page = CdpPage::connect(&websocket_url)
                        .await
                        .map_err(|_| anyhow!("qr_cdp_connect_failed"))?;
                    cdp.register(page)
                        .map_err(|_| anyhow!("qr_cdp_connect_failed"))?;
                    let page_result = {
                        let mut owned_page = cdp.lock().await;
                        let page = owned_page
                            .as_mut()
                            .ok_or_else(|| anyhow!("qr_cdp_connect_failed"))?;
                        async {
                            advance_to_qr(page, &reporter).await?;
                            reporter.report_qr(QrLoginPhase::LoadingQr, "正在获取二维码…");
                            let png = fetch_qr_png(page).await?;
                            reporter.emit_qr(png, generation)?;
                            reporter.report_qr(
                                QrLoginPhase::WaitingForScan,
                                "请使用网易大神扫描二维码并在手机上确认",
                            );
                            let capture_app = reporter.clone();
                            fetch_credentials_from_browser_with_cancel(&capture_app, port, || {
                                coordinator
                                    .control(session_id)
                                    .map(|(current, cancelled)| cancelled || current != generation)
                                    .unwrap_or(true)
                            })
                            .await
                        }
                        .await
                    };
                    let _ = cdp.close(Duration::from_secs(1)).await;
                    page_result
                }
                .await;
                let _ = resources.shutdown(Duration::from_secs(2)).await;
                match attempt {
                    Ok(outcome) => {
                        return publish_capture(
                            &accounts,
                            &reporter,
                            outcome,
                            &coordinator,
                            session_id,
                            generation,
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
                error_code: None,
                cancelled: false,
                expired: false,
            }
        }
        Err(error) if error.to_string() == "qr_cancelled" => {
            reporter.report_qr(QrLoginPhase::Cancelled, "已取消二维码登录");
            QrLoginResultEvent {
                session_id: session_id.to_string(),
                result: None,
                message: None,
                error_code: None,
                cancelled: true,
                expired: false,
            }
        }
        Err(error) if error.to_string() == "qr_capture_timeout" => {
            reporter.report_qr(QrLoginPhase::Expired, "二维码已过期，请刷新后重试");
            expired_completion(session_id)
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
                error_code: Some(qr_failure_code(&error)),
                cancelled: false,
                expired: false,
            }
        }
    }
}

const QR_FAILURE_CODES: &[&str] = &[
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
    "credential_login_page_failed",
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

fn qr_failure_code(error: &anyhow::Error) -> &'static str {
    let raw = error.to_string();
    QR_FAILURE_CODES
        .iter()
        .copied()
        .find(|candidate| raw == *candidate)
        .unwrap_or("qr_internal_failure")
}

fn qr_failure_message(error: &anyhow::Error) -> String {
    let code = qr_failure_code(error);
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
) -> Result<AccountLoginResult> {
    reporter.report_qr(QrLoginPhase::Capturing, "正在验证登录信息…");
    let mut result = capture_verify_and_publish_add(
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
        |capture| {
            let credentials = capture.credentials.clone();
            let battle_tag = capture.battle_tag.clone();
            async move {
                Ok(
                    crate::account_sync_identity::verify_credentials_with_base_url(
                        &credentials,
                        &battle_tag,
                        DASHEN_ROOT_BASE,
                    )
                    .await,
                )
            }
        },
    )
    .await
    .map_err(|_| anyhow!("qr_publish_failed"))?;
    if matches!(
        result.kind.as_str(),
        "newAccountAdded" | "existingAccountUpdated"
    ) {
        coordinator.mark_committed(session_id, result.clone());
    }
    reporter.report_qr(QrLoginPhase::Saving, "正在安全保存账号登录信息…");
    if refresh_account_sync_identity(accounts, &result)
        .await
        .is_err()
    {
        crate::account_login::add_post_commit_warning(
            &mut result,
            crate::account_login::POST_COMMIT_REFRESH_WARNING,
        );
    }
    Ok(result)
}

async fn refresh_account_sync_identity(
    accounts: &AccountContextManager,
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
    let battle_tag = result
        .battle_tag
        .as_deref()
        .ok_or_else(|| anyhow!("账号登录结果缺少 BattleTag，请重试"))?;
    crate::account_sync_identity::refresh_with_base_url(
        &credentials,
        battle_tag,
        &account.paths().player_target(),
        DASHEN_ROOT_BASE,
    )
    .await?;
    let profile = crate::profile_card::refresh_for_account(&account, DASHEN_ROOT_BASE).await;
    accounts
        .observe_authenticated_result(account.account_id(), profile)
        .map_err(|_| anyhow!("post-commit profile refresh failed"))?;
    Ok(())
}

async fn advance_to_qr(page: &mut CdpPage, reporter: &QrReporter) -> Result<()> {
    advance_to_qr_core(page, Some(reporter)).await
}

async fn advance_to_qr_core(page: &mut CdpPage, reporter: Option<&QrReporter>) -> Result<()> {
    if let Some(reporter) = reporter {
        reporter.report_qr(QrLoginPhase::OpeningLogin, "正在打开网易登录窗口…");
    }
    wait_click(page, MAIN_LOGIN_CLICK, None, "qr_login_button_missing").await?;
    if let Some(reporter) = reporter {
        reporter.report_qr(QrLoginPhase::SelectingQrMode, "正在切换扫码登录…");
    }
    wait_click(page, QR_MODE_CLICK, None, "qr_mode_switch_missing").await?;
    if let Some(reporter) = reporter {
        reporter.report_qr(QrLoginPhase::LoadingQr, "正在准备扫码登录…");
    }
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
    fn emit_qr(&self, png: Vec<u8>, _generation: u64) -> Result<()> {
        let _hash = Sha256::digest(&png); // no image bytes or source leave this scope except the event.
        let image_base64 = STANDARD.encode(png);
        let revision = self
            .coordinator
            .update_image(self.session_id, image_base64.clone())
            .ok_or_else(|| anyhow!("qr_cancelled"))?;
        // Polling the coordinator snapshot is the reliable delivery path. Event
        // delivery is only an optimisation and must never abort QR login.
        let _ = self.app.emit(
            "credential-login-qr",
            QrImageEvent {
                session_id: self.session_id.to_string(),
                revision,
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
    use crate::sync_coordinator::{OperationStatus, SyncCoordinator};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Barrier;
    use std::thread;

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "loads the live NetEase login page in installed Chrome/Edge"]
    async fn live_installed_browsers_reach_and_extract_a_qr_image() {
        let mut tested_kinds = std::collections::BTreeSet::new();
        for browser in find_windows_browsers() {
            if tested_kinds.contains(&browser.kind) {
                continue;
            }
            let parent = tempfile::tempdir().unwrap();
            let port = choose_available_qr_debug_port().unwrap();
            let mut session =
                launch_isolated_hidden_browser(&browser, LOGIN_URL, port, parent.path())
                    .unwrap_or_else(|error| panic!("{} failed to launch: {error}", browser.kind));
            let profile = session.profile_dir().to_path_buf();

            wait_for_debug_endpoint(port, Duration::from_secs(20))
                .await
                .unwrap_or_else(|error| panic!("{} DevTools unavailable: {error}", browser.kind));
            let websocket = open_or_find_page(LOGIN_URL, port)
                .await
                .unwrap_or_else(|error| panic!("{} login page unavailable: {error}", browser.kind));
            let mut page = CdpPage::connect(&websocket)
                .await
                .unwrap_or_else(|error| panic!("{} CDP unavailable: {error}", browser.kind));
            advance_to_qr_core(&mut page, None)
                .await
                .unwrap_or_else(|error| panic!("{} QR flow failed: {error}", browser.kind));
            let png = fetch_qr_png(&mut page)
                .await
                .unwrap_or_else(|error| panic!("{} QR image failed: {error}", browser.kind));
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

            let _ = page.close(Duration::from_secs(1)).await;
            let report = session.shutdown(Duration::from_secs(2)).await;
            assert!(report.process_exited, "{} did not exit", browser.kind);
            assert!(report.profile_removed, "{} profile remained", browser.kind);
            assert!(!profile.exists(), "{} profile path remained", browser.kind);
            tested_kinds.insert(browser.kind);
        }
        assert!(
            !tested_kinds.is_empty(),
            "no supported browser was installed"
        );
    }

    #[derive(Default)]
    struct ScriptedQrResources {
        browsers: AtomicUsize,
        profiles: AtomicUsize,
        shutdowns: AtomicUsize,
    }

    enum ScriptedQrBehavior {
        PauseAt(QrLoginPhase),
        BusinessError,
        CommitThenHang,
        Panic,
        Hang,
    }

    struct ScriptedQrLoginAdapter {
        coordinator: QrLoginCoordinator,
        session_id: Uuid,
        behavior: ScriptedQrBehavior,
        reached: Arc<Notify>,
        resources: Arc<ScriptedQrResources>,
    }

    impl QrLoginAdapter for ScriptedQrLoginAdapter {
        fn run(&self) -> QrLoginRunFuture {
            let coordinator = self.coordinator.clone();
            let session_id = self.session_id;
            let behavior = match &self.behavior {
                ScriptedQrBehavior::PauseAt(phase) => ScriptedQrBehavior::PauseAt(phase.clone()),
                ScriptedQrBehavior::BusinessError => ScriptedQrBehavior::BusinessError,
                ScriptedQrBehavior::CommitThenHang => ScriptedQrBehavior::CommitThenHang,
                ScriptedQrBehavior::Panic => ScriptedQrBehavior::Panic,
                ScriptedQrBehavior::Hang => ScriptedQrBehavior::Hang,
            };
            let reached = Arc::clone(&self.reached);
            let resources = Arc::clone(&self.resources);
            Box::pin(async move {
                resources.browsers.store(1, Ordering::SeqCst);
                resources.profiles.store(1, Ordering::SeqCst);
                match behavior {
                    ScriptedQrBehavior::PauseAt(phase) => {
                        coordinator.update_progress(session_id, phase, "scripted phase");
                        reached.notify_one();
                        std::future::pending::<QrLoginResultEvent>().await
                    }
                    ScriptedQrBehavior::BusinessError => {
                        coordinator.update_progress(
                            session_id,
                            QrLoginPhase::Capturing,
                            "scripted business error",
                        );
                        reached.notify_one();
                        error_completion(session_id, &anyhow!("qr_publish_failed"))
                    }
                    ScriptedQrBehavior::CommitThenHang => {
                        coordinator.mark_committed(
                            session_id,
                            AccountLoginResult {
                                kind: "newAccountAdded".to_string(),
                                account_id: Some("scripted-account".to_string()),
                                battle_tag: Some("Scripted#1234".to_string()),
                                message: "account saved".to_string(),
                                should_offer_switch: false,
                                warnings: Vec::new(),
                            },
                        );
                        coordinator.update_progress(
                            session_id,
                            QrLoginPhase::Saving,
                            "scripted post-commit work",
                        );
                        reached.notify_one();
                        std::future::pending::<QrLoginResultEvent>().await
                    }
                    ScriptedQrBehavior::Panic => {
                        reached.notify_one();
                        panic!("scripted QR adapter panic")
                    }
                    ScriptedQrBehavior::Hang => {
                        reached.notify_one();
                        std::future::pending::<QrLoginResultEvent>().await
                    }
                }
            })
        }

        fn shutdown(&self) -> QrLoginShutdownFuture {
            let resources = Arc::clone(&self.resources);
            Box::pin(async move {
                resources.browsers.store(0, Ordering::SeqCst);
                resources.profiles.store(0, Ordering::SeqCst);
                resources.shutdowns.fetch_add(1, Ordering::SeqCst);
            })
        }
    }

    struct BrowserOwnerTestAdapter(BrowserLoginSessionOwner);

    impl QrLoginAdapter for BrowserOwnerTestAdapter {
        fn run(&self) -> QrLoginRunFuture {
            Box::pin(std::future::pending())
        }

        fn shutdown(&self) -> QrLoginShutdownFuture {
            let resources = self.0.clone();
            Box::pin(async move {
                let _ = resources.shutdown(Duration::from_secs(2)).await;
            })
        }
    }

    struct OwnedResourcesTestAdapter {
        browser: BrowserLoginSessionOwner,
        cdp: CdpPageOwner,
    }

    struct HangingShutdownAdapter {
        session_id: Uuid,
        resources: Arc<AtomicUsize>,
    }

    impl QrLoginAdapter for HangingShutdownAdapter {
        fn run(&self) -> QrLoginRunFuture {
            let session_id = self.session_id;
            let resources = Arc::clone(&self.resources);
            Box::pin(async move {
                resources.store(1, Ordering::SeqCst);
                error_completion(session_id, &anyhow!("qr_browser_start_failed"))
            })
        }

        fn shutdown(&self) -> QrLoginShutdownFuture {
            let resources = Arc::clone(&self.resources);
            Box::pin(async move {
                // Taking ownership is synchronous; a broken cleanup tail must
                // still be bounded by the supervisor's outer timeout.
                resources.store(0, Ordering::SeqCst);
                std::future::pending::<()>().await;
            })
        }
    }

    impl QrLoginAdapter for OwnedResourcesTestAdapter {
        fn run(&self) -> QrLoginRunFuture {
            Box::pin(std::future::pending())
        }

        fn shutdown(&self) -> QrLoginShutdownFuture {
            let browser = self.browser.clone();
            let cdp = self.cdp.clone();
            Box::pin(async move {
                let _ = cdp.close(Duration::from_secs(1)).await;
                let _ = browser.shutdown(Duration::from_secs(2)).await;
            })
        }
    }

    struct ScriptedQrLoginHarness {
        coordinator: QrLoginCoordinator,
        operations: SyncCoordinator,
        session_id: Uuid,
        resources: Arc<ScriptedQrResources>,
        published: Arc<AtomicUsize>,
    }

    impl ScriptedQrLoginHarness {
        async fn start(pause_at: QrLoginPhase) -> Self {
            Self::start_behavior(ScriptedQrBehavior::PauseAt(pause_at), false).await
        }

        async fn start_behavior(behavior: ScriptedQrBehavior, publisher_failed: bool) -> Self {
            let coordinator = QrLoginCoordinator::default();
            let operations = SyncCoordinator::default();
            let (session_id, operation_permit) = match coordinator
                .start_or_recover("scripted-request".to_string(), &operations)
                .unwrap()
            {
                StartDecision::New { session_id, permit } => (session_id, permit),
                StartDecision::Reused { .. } => unreachable!("fresh harness cannot reuse"),
            };
            let reached = Arc::new(Notify::new());
            let resources = Arc::new(ScriptedQrResources::default());
            let published = Arc::new(AtomicUsize::new(0));
            let adapter: Arc<dyn QrLoginAdapter> = Arc::new(ScriptedQrLoginAdapter {
                coordinator: coordinator.clone(),
                session_id,
                behavior,
                reached: Arc::clone(&reached),
                resources: Arc::clone(&resources),
            });
            let task_coordinator = coordinator.clone();
            let task_published = Arc::clone(&published);
            tokio::spawn(async move {
                execute_adapter_session(
                    operation_permit,
                    task_coordinator,
                    session_id,
                    adapter,
                    move |_| {
                        task_published.fetch_add(1, Ordering::SeqCst);
                        if publisher_failed {
                            Err(anyhow!("scripted event transport failure"))
                        } else {
                            Ok(())
                        }
                    },
                )
                .await;
            });
            tokio::time::timeout(Duration::from_secs(1), reached.notified())
                .await
                .expect("scripted adapter did not reach requested phase");
            Self {
                coordinator,
                operations,
                session_id,
                resources,
                published,
            }
        }

        async fn wait_and_assert_failed(self, expected_error_code: &'static str) {
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if self
                        .coordinator
                        .snapshot(&self.session_id.to_string())
                        .is_ok_and(|snapshot| snapshot.settled)
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("scripted adapter did not terminalise");
            let snapshot = self
                .coordinator
                .snapshot(&self.session_id.to_string())
                .unwrap();
            assert!(matches!(snapshot.phase, QrLoginPhase::Failed));
            assert!(snapshot.released);
            assert_eq!(
                snapshot
                    .completion
                    .as_ref()
                    .and_then(|item| item.error_code),
                Some(expected_error_code)
            );
            assert_eq!(self.resources.browsers.load(Ordering::SeqCst), 0);
            assert_eq!(self.resources.profiles.load(Ordering::SeqCst), 0);
            assert_eq!(self.resources.shutdowns.load(Ordering::SeqCst), 1);
            assert_eq!(self.operations.status(), OperationStatus::Idle);
            assert_eq!(self.published.load(Ordering::SeqCst), 1);
            let visible_login = self.operations.try_acquire_account_operation().unwrap();
            drop(visible_login);
            let replacement = self
                .coordinator
                .start_or_recover("replacement-after-failure".to_string(), &self.operations)
                .unwrap();
            assert!(matches!(replacement, StartDecision::New { .. }));
        }

        async fn wait_and_assert_expired(self) {
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if self
                        .coordinator
                        .snapshot(&self.session_id.to_string())
                        .is_ok_and(|snapshot| snapshot.settled)
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("scripted adapter did not expire");
            let snapshot = self
                .coordinator
                .snapshot(&self.session_id.to_string())
                .unwrap();
            assert!(matches!(snapshot.phase, QrLoginPhase::Expired));
            assert!(snapshot.released);
            assert_eq!(
                snapshot
                    .completion
                    .as_ref()
                    .and_then(|item| item.error_code),
                Some("qr_expired")
            );
            assert_eq!(self.resources.browsers.load(Ordering::SeqCst), 0);
            assert_eq!(self.resources.profiles.load(Ordering::SeqCst), 0);
            assert_eq!(self.resources.shutdowns.load(Ordering::SeqCst), 1);
            assert_eq!(self.operations.status(), OperationStatus::Idle);
            assert_eq!(self.published.load(Ordering::SeqCst), 1);
            let visible_login = self.operations.try_acquire_account_operation().unwrap();
            drop(visible_login);
            let replacement = self
                .coordinator
                .start_or_recover("replacement-after-expiry".to_string(), &self.operations)
                .unwrap();
            assert!(matches!(replacement, StartDecision::New { .. }));
        }

        async fn cancel_and_assert_committed_ready(self) {
            self.coordinator
                .cancel_and_wait(&self.session_id.to_string())
                .await
                .unwrap();
            let snapshot = self
                .coordinator
                .snapshot(&self.session_id.to_string())
                .unwrap();
            assert!(matches!(snapshot.phase, QrLoginPhase::Ready));
            assert!(snapshot.released);
            let result = snapshot
                .completion
                .and_then(|completion| completion.result)
                .expect("committed result must remain a success");
            assert_eq!(result.kind, "newAccountAdded");
            assert!(!result.warnings.is_empty());
            assert_eq!(self.resources.browsers.load(Ordering::SeqCst), 0);
            assert_eq!(self.resources.profiles.load(Ordering::SeqCst), 0);
            assert_eq!(self.operations.status(), OperationStatus::Idle);
            assert_eq!(self.published.load(Ordering::SeqCst), 1);
            let visible_login = self.operations.try_acquire_account_operation().unwrap();
            drop(visible_login);
        }

        async fn cancel_and_assert_reusable(self) {
            self.coordinator
                .cancel_and_wait(&self.session_id.to_string())
                .await
                .unwrap();
            let snapshot = self
                .coordinator
                .snapshot(&self.session_id.to_string())
                .unwrap();
            assert!(matches!(snapshot.phase, QrLoginPhase::Cancelled));
            assert!(snapshot.settled);
            assert!(snapshot.released);
            assert_eq!(
                snapshot
                    .completion
                    .as_ref()
                    .and_then(|item| item.error_code),
                None
            );
            assert!(snapshot
                .completion
                .is_some_and(|completion| completion.cancelled));
            assert_eq!(self.resources.browsers.load(Ordering::SeqCst), 0);
            assert_eq!(self.resources.profiles.load(Ordering::SeqCst), 0);
            assert_eq!(self.resources.shutdowns.load(Ordering::SeqCst), 1);
            assert_eq!(self.operations.status(), OperationStatus::Idle);
            assert_eq!(self.published.load(Ordering::SeqCst), 1);

            let visible_login = self.operations.try_acquire_account_operation().unwrap();
            drop(visible_login);
            let replacement = self
                .coordinator
                .start_or_recover("replacement-request".to_string(), &self.operations)
                .unwrap();
            assert!(matches!(replacement, StartDecision::New { .. }));
        }
    }

    #[tokio::test]
    async fn scripted_adapter_cancel_at_starting_releases_everything_before_terminal() {
        let harness = ScriptedQrLoginHarness::start(QrLoginPhase::Starting).await;
        harness.cancel_and_assert_reusable().await;
    }

    #[test]
    fn cancel_wait_budget_has_margin_over_forced_abort_and_cleanup() {
        assert!(QR_ADAPTER_SHUTDOWN_BUDGET > QR_CDP_CLOSE_BUDGET + QR_BROWSER_SHUTDOWN_BUDGET);
        assert!(QR_CANCEL_WAIT_BUDGET > QR_CANCEL_GRACE + QR_ADAPTER_SHUTDOWN_BUDGET);
    }

    #[tokio::test]
    async fn cancellation_matrix_releases_everything_at_each_pre_commit_phase() {
        for phase in [
            QrLoginPhase::Starting,
            QrLoginPhase::OpeningLogin,
            QrLoginPhase::SelectingQrMode,
            QrLoginPhase::AcceptingPrivacy,
            QrLoginPhase::LoadingQr,
            QrLoginPhase::WaitingForScan,
            QrLoginPhase::Confirming,
            QrLoginPhase::Capturing,
            QrLoginPhase::Saving,
        ] {
            let harness = ScriptedQrLoginHarness::start(phase).await;
            harness.cancel_and_assert_reusable().await;
        }
    }

    #[tokio::test]
    async fn scripted_business_error_terminalises_and_allows_immediate_retry() {
        ScriptedQrLoginHarness::start_behavior(ScriptedQrBehavior::BusinessError, false)
            .await
            .wait_and_assert_failed("qr_publish_failed")
            .await;
    }

    #[tokio::test]
    async fn scripted_adapter_panic_terminalises_and_allows_immediate_retry() {
        ScriptedQrLoginHarness::start_behavior(ScriptedQrBehavior::Panic, false)
            .await
            .wait_and_assert_failed("qr_worker_failed")
            .await;
    }

    #[tokio::test]
    async fn hanging_adapter_is_aborted_and_cleaned_after_cancel() {
        ScriptedQrLoginHarness::start_behavior(ScriptedQrBehavior::Hang, false)
            .await
            .cancel_and_assert_reusable()
            .await;
    }

    #[tokio::test]
    async fn cancel_after_commit_reports_ready_and_releases_everything() {
        ScriptedQrLoginHarness::start_behavior(ScriptedQrBehavior::CommitThenHang, false)
            .await
            .cancel_and_assert_committed_ready()
            .await;
    }

    #[tokio::test]
    async fn event_transport_failure_cannot_undo_terminalisation_or_release() {
        ScriptedQrLoginHarness::start_behavior(ScriptedQrBehavior::BusinessError, true)
            .await
            .wait_and_assert_failed("qr_publish_failed")
            .await;
    }

    #[tokio::test]
    async fn hanging_adapter_expires_and_releases_everything_without_user_action() {
        ScriptedQrLoginHarness::start_behavior(ScriptedQrBehavior::Hang, false)
            .await
            .wait_and_assert_expired()
            .await;
    }

    #[tokio::test]
    async fn externally_aborted_adapter_terminalises_and_releases_resources() {
        let operations = SyncCoordinator::default();
        let coordinator = QrLoginCoordinator::default();
        let (session_id, permit) = match coordinator
            .start_or_recover("externally-aborted".to_string(), &operations)
            .unwrap()
        {
            StartDecision::New { session_id, permit } => (session_id, permit),
            StartDecision::Reused { .. } => unreachable!(),
        };
        let reached = Arc::new(Notify::new());
        let resources = Arc::new(ScriptedQrResources::default());
        let adapter = Arc::new(ScriptedQrLoginAdapter {
            coordinator: coordinator.clone(),
            session_id,
            behavior: ScriptedQrBehavior::Hang,
            reached: Arc::clone(&reached),
            resources: Arc::clone(&resources),
        });
        let worker_adapter = Arc::clone(&adapter);
        let worker = tauri::async_runtime::spawn(async move { worker_adapter.run().await });
        reached.notified().await;
        worker.abort();

        let completion =
            supervise_session(&coordinator, session_id, worker, adapter.as_ref()).await;
        drop(permit);
        coordinator.complete(session_id, completion);

        let snapshot = coordinator.snapshot(&session_id.to_string()).unwrap();
        assert!(matches!(snapshot.phase, QrLoginPhase::Failed));
        assert!(snapshot.released);
        assert_eq!(
            snapshot.completion.and_then(|item| item.error_code),
            Some("qr_worker_failed")
        );
        assert_eq!(resources.browsers.load(Ordering::SeqCst), 0);
        assert_eq!(resources.profiles.load(Ordering::SeqCst), 0);
        assert_eq!(resources.shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(operations.status(), OperationStatus::Idle);
        assert!(coordinator.start().is_ok());
    }

    #[tokio::test]
    async fn hanging_shutdown_is_bounded_before_terminal_and_permit_release() {
        let operations = SyncCoordinator::default();
        let coordinator = QrLoginCoordinator::default();
        let (session_id, permit) = match coordinator
            .start_or_recover("hanging-shutdown".to_string(), &operations)
            .unwrap()
        {
            StartDecision::New { session_id, permit } => (session_id, permit),
            StartDecision::Reused { .. } => unreachable!(),
        };
        let resources = Arc::new(AtomicUsize::new(0));
        let adapter: Arc<dyn QrLoginAdapter> = Arc::new(HangingShutdownAdapter {
            session_id,
            resources: Arc::clone(&resources),
        });
        let task_coordinator = coordinator.clone();
        tokio::spawn(async move {
            execute_adapter_session(permit, task_coordinator, session_id, adapter, |_| Ok(()))
                .await;
        });

        tokio::time::timeout(Duration::from_millis(500), async {
            loop {
                if coordinator
                    .snapshot(&session_id.to_string())
                    .is_ok_and(|snapshot| snapshot.settled && snapshot.released)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("supervisor exceeded its cleanup budget");
        assert_eq!(resources.load(Ordering::SeqCst), 0);
        assert_eq!(operations.status(), OperationStatus::Idle);
        assert!(coordinator.start().is_ok());
    }

    #[test]
    fn concurrent_same_request_atomically_creates_one_session() {
        let coordinator = QrLoginCoordinator::default();
        let operations = SyncCoordinator::default();
        let barrier = Arc::new(Barrier::new(100));
        let mut workers = Vec::new();
        for _ in 0..100 {
            let coordinator = coordinator.clone();
            let operations = operations.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                coordinator
                    .start_or_recover("same-request".to_string(), &operations)
                    .unwrap()
            }));
        }
        let decisions: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        let sessions: Vec<_> = decisions
            .iter()
            .map(|decision| match decision {
                StartDecision::New { session_id, .. } | StartDecision::Reused { session_id } => {
                    *session_id
                }
            })
            .collect();
        assert!(sessions.iter().all(|session| *session == sessions[0]));
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| matches!(decision, StartDecision::New { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn concurrent_refresh_reuses_one_generation_successor_and_preserves_old_snapshot() {
        let coordinator = QrLoginCoordinator::default();
        let operations = SyncCoordinator::default();
        let old = coordinator.start().unwrap();
        coordinator.complete(old, failed_completion(old));

        let first = coordinator
            .start_or_recover_successor("logical-request".to_string(), Some(old), 1, &operations)
            .unwrap();
        let successor = match &first {
            StartDecision::New { session_id, .. } => *session_id,
            StartDecision::Reused { .. } => panic!("first refresh must create"),
        };
        for _ in 0..20 {
            let repeated = coordinator
                .start_or_recover_successor(
                    "logical-request".to_string(),
                    Some(old),
                    1,
                    &operations,
                )
                .unwrap();
            assert!(
                matches!(repeated, StartDecision::Reused { session_id } if session_id == successor)
            );
        }
        assert_eq!(
            coordinator
                .snapshot(&successor.to_string())
                .unwrap()
                .generation,
            1
        );
        let old_snapshot = coordinator.snapshot(&old.to_string()).unwrap();
        assert!(old_snapshot.settled);
        assert!(old_snapshot.image_base64.is_none());
    }

    #[test]
    fn terminal_history_keeps_four_redacted_snapshots() {
        let coordinator = QrLoginCoordinator::default();
        let mut sessions = Vec::new();
        for _ in 0..6 {
            let id = coordinator.start().unwrap();
            coordinator.complete(id, failed_completion(id));
            sessions.push(id);
        }
        let _next = coordinator.start().unwrap();

        assert!(coordinator.snapshot(&sessions[0].to_string()).is_err());
        assert!(coordinator.snapshot(&sessions[1].to_string()).is_err());
        for id in &sessions[2..] {
            let snapshot = coordinator.snapshot(&id.to_string()).unwrap();
            assert!(snapshot.settled);
            assert!(snapshot.released);
            assert!(snapshot.image_base64.is_none());
        }
    }

    #[tokio::test]
    async fn terminal_history_expires_after_ttl() {
        let coordinator = QrLoginCoordinator::default();
        let old = coordinator.start().unwrap();
        coordinator.complete(old, failed_completion(old));
        let _next = coordinator.start().unwrap();
        assert!(coordinator.snapshot(&old.to_string()).is_ok());
        tokio::time::sleep(QR_HISTORY_TTL + Duration::from_millis(20)).await;
        assert!(coordinator.snapshot(&old.to_string()).is_err());
    }

    #[test]
    fn structured_errors_expose_only_safe_codes() {
        assert_eq!(
            QrLoginCommandError::from_error(anyhow!("qr_busy")).code,
            "qr_busy"
        );
        assert_eq!(
            QrLoginCommandError::from_error(anyhow!("qr_not_found")).code,
            "qr_not_found"
        );
        assert_eq!(
            QrLoginCommandError::from_error(anyhow!("qr_timeout")).code,
            "qr_timeout"
        );
        let unknown = QrLoginCommandError::from_error(anyhow!("token=secret"));
        assert_eq!(unknown.code, "qr_internal_failure");
        assert!(!unknown.message.contains("secret"));
    }

    #[test]
    fn revision_increases_for_progress_image_and_terminal_changes() {
        let coordinator = QrLoginCoordinator::default();
        let session = coordinator.start().unwrap();
        let initial = coordinator.snapshot(&session.to_string()).unwrap().revision;
        coordinator.update_progress(session, QrLoginPhase::WaitingForScan, "waiting");
        let waiting = coordinator.snapshot(&session.to_string()).unwrap().revision;
        let image = coordinator
            .update_image(session, "redacted-image".to_string())
            .unwrap();
        coordinator.update_progress(session, QrLoginPhase::Confirming, "confirming");
        let confirming = coordinator.snapshot(&session.to_string()).unwrap().revision;
        coordinator.complete(session, cancelled_completion(session));
        let terminal = coordinator.snapshot(&session.to_string()).unwrap().revision;

        assert!(initial < waiting);
        assert!(waiting < image);
        assert!(image < confirming);
        assert!(confirming < terminal);
    }

    #[tokio::test]
    async fn cancel_returns_only_after_the_account_operation_permit_is_released() {
        let operations = SyncCoordinator::default();
        let permit = operations.try_acquire_account_operation().unwrap();
        let coordinator = QrLoginCoordinator::default();
        let session_id = coordinator.start().unwrap();
        let worker = tauri::async_runtime::spawn(async move {
            let _permit = permit;
            std::future::pending::<QrLoginResultEvent>().await
        });
        let supervisor_coordinator = coordinator.clone();
        tokio::spawn(async move {
            let adapter = BrowserOwnerTestAdapter(BrowserLoginSessionOwner::default());
            let completion =
                supervise_session(&supervisor_coordinator, session_id, worker, &adapter).await;
            supervisor_coordinator.complete(session_id, completion);
        });

        coordinator
            .cancel_and_wait(&session_id.to_string())
            .await
            .unwrap();

        assert_eq!(operations.status(), OperationStatus::Idle);
        assert!(operations.try_acquire_account_operation().is_ok());
        assert!(coordinator
            .snapshot(&session_id.to_string())
            .unwrap()
            .completion
            .is_some_and(|completion| completion.cancelled));
    }

    #[tokio::test]
    async fn repeated_cancel_is_idempotent_and_both_callers_observe_terminal_state() {
        let coordinator = QrLoginCoordinator::default();
        let session_id = coordinator.start().unwrap();
        let worker = tauri::async_runtime::spawn(std::future::pending::<QrLoginResultEvent>());
        let supervisor_coordinator = coordinator.clone();
        tokio::spawn(async move {
            let adapter = BrowserOwnerTestAdapter(BrowserLoginSessionOwner::default());
            let completion =
                supervise_session(&supervisor_coordinator, session_id, worker, &adapter).await;
            supervisor_coordinator.complete(session_id, completion);
        });

        let id = session_id.to_string();
        let (first, second) = tokio::join!(
            coordinator.cancel_and_wait(&id),
            coordinator.cancel_and_wait(&id)
        );
        assert!(first.is_ok());
        assert!(second.is_ok());
        assert!(coordinator.cancel_and_wait(&id).await.is_ok());
    }

    #[tokio::test]
    async fn repeated_cancel_of_archived_terminal_session_is_immediate_and_does_not_cancel_new() {
        let coordinator = QrLoginCoordinator::default();
        let old = coordinator.start().unwrap();
        coordinator.complete(old, cancelled_completion(old));
        let current = coordinator.start().unwrap();

        tokio::time::timeout(
            Duration::from_millis(50),
            coordinator.cancel_and_wait(&old.to_string()),
        )
        .await
        .expect("archived terminal cancellation must not wait")
        .unwrap();

        assert!(coordinator
            .control(current)
            .is_some_and(|(_, cancelled)| !cancelled));
    }

    #[tokio::test]
    async fn worker_panic_releases_permit_and_publishes_failed_terminal_state() {
        let operations = SyncCoordinator::default();
        let permit = operations.try_acquire_account_operation().unwrap();
        let coordinator = QrLoginCoordinator::default();
        let session_id = coordinator.start().unwrap();
        let worker = tauri::async_runtime::spawn(async move {
            let _permit = permit;
            panic!("injected QR worker panic");
        });
        let adapter = BrowserOwnerTestAdapter(BrowserLoginSessionOwner::default());
        let completion = supervise_session(&coordinator, session_id, worker, &adapter).await;
        coordinator.complete(session_id, completion);

        assert_eq!(operations.status(), OperationStatus::Idle);
        let snapshot = coordinator.snapshot(&session_id.to_string()).unwrap();
        assert!(snapshot.completion.is_some());
        assert!(coordinator.start().is_ok());
    }

    #[tokio::test]
    async fn worker_panic_cleans_browser_resources_before_terminal_is_published() {
        let root = tempfile::tempdir().unwrap();
        let profile = crate::browser_session::create_isolated_profile_dir(root.path()).unwrap();
        let resources = BrowserLoginSessionOwner::default();
        resources
            .register(
                crate::browser_session::BrowserLoginSession::detached_for_test(profile.clone()),
            )
            .unwrap();
        let coordinator = QrLoginCoordinator::default();
        let session_id = coordinator.start().unwrap();
        let worker = tauri::async_runtime::spawn(async move {
            panic!("injected QR worker panic with a live browser resource");
        });

        let adapter = BrowserOwnerTestAdapter(resources.clone());
        let completion = supervise_session(&coordinator, session_id, worker, &adapter).await;

        assert!(resources.is_empty());
        assert!(!profile.exists());
        assert!(coordinator
            .snapshot(&session_id.to_string())
            .unwrap()
            .completion
            .is_none());
        coordinator.complete(session_id, completion);
        assert!(coordinator
            .snapshot(&session_id.to_string())
            .unwrap()
            .completion
            .is_some());
    }

    #[tokio::test]
    async fn forced_abort_explicitly_closes_cdp_before_terminal_is_published() {
        use futures_util::StreamExt as _;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let close_observed = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            while let Some(message) = socket.next().await {
                if matches!(
                    message.unwrap(),
                    tokio_tungstenite::tungstenite::Message::Close(_)
                ) {
                    return true;
                }
            }
            false
        });
        let coordinator = QrLoginCoordinator::default();
        let session_id = coordinator.start().unwrap();
        let browser = BrowserLoginSessionOwner::default();
        let cdp = crate::browser_session::CdpPageOwner::default();
        let worker_cdp = cdp.clone();
        let registered = Arc::new(Notify::new());
        let worker_registered = Arc::clone(&registered);
        let worker = tauri::async_runtime::spawn(async move {
            let page = CdpPage::connect(&format!("ws://{address}")).await.unwrap();
            worker_cdp.register(page).unwrap();
            worker_registered.notify_one();
            let _page = worker_cdp.lock().await;
            std::future::pending::<QrLoginResultEvent>().await
        });
        registered.notified().await;
        coordinator.cancel(&session_id.to_string()).unwrap();

        let adapter = OwnedResourcesTestAdapter {
            browser,
            cdp: cdp.clone(),
        };
        let completion = supervise_session(&coordinator, session_id, worker, &adapter).await;

        assert!(cdp.is_empty());
        assert!(tokio::time::timeout(Duration::from_secs(1), close_observed)
            .await
            .expect("CDP close frame was not observed")
            .unwrap());
        assert!(coordinator
            .snapshot(&session_id.to_string())
            .unwrap()
            .completion
            .is_none());
        coordinator.complete(session_id, completion);
        assert!(coordinator
            .snapshot(&session_id.to_string())
            .unwrap()
            .completion
            .is_some());
    }

    #[tokio::test]
    async fn expired_session_releases_permit_and_allows_refresh_successor() {
        let operations = SyncCoordinator::default();
        let permit = operations.try_acquire_account_operation().unwrap();
        let coordinator = QrLoginCoordinator::default();
        let (expired_session, reused) = coordinator
            .start_request("expired-request".to_string())
            .unwrap();
        assert!(!reused);
        let worker = tauri::async_runtime::spawn(async move {
            let _permit = permit;
            std::future::pending::<QrLoginResultEvent>().await
        });

        let adapter = BrowserOwnerTestAdapter(BrowserLoginSessionOwner::default());
        let completion = supervise_session(&coordinator, expired_session, worker, &adapter).await;
        coordinator.complete(expired_session, completion);

        let snapshot = coordinator.snapshot(&expired_session.to_string()).unwrap();
        assert!(matches!(snapshot.phase, QrLoginPhase::Expired));
        assert!(snapshot.settled);
        assert!(snapshot.released);
        assert_eq!(operations.status(), OperationStatus::Idle);
        assert!(operations.try_acquire_account_operation().is_ok());

        let (successor, successor_reused) = coordinator
            .start_request("refresh-request".to_string())
            .unwrap();
        assert!(!successor_reused);
        assert_ne!(expired_session, successor);
    }

    #[tokio::test]
    async fn archived_terminal_session_cancel_cannot_cancel_the_new_session() {
        let coordinator = QrLoginCoordinator::default();
        let old = coordinator.start().unwrap();
        coordinator.complete(old, cancelled_completion(old));
        let current = coordinator.start().unwrap();

        assert!(coordinator.cancel_and_wait(&old.to_string()).await.is_ok());
        assert!(coordinator
            .control(current)
            .is_some_and(|(_, cancelled)| !cancelled));
    }

    #[test]
    fn repeated_start_request_recovers_the_same_session() {
        let coordinator = QrLoginCoordinator::default();
        let (first, reused) = coordinator
            .start_request("start-request-1".to_string())
            .unwrap();
        let (second, second_reused) = coordinator
            .start_request("start-request-1".to_string())
            .unwrap();

        assert!(!reused);
        assert!(second_reused);
        assert_eq!(first, second);
        assert!(coordinator
            .recover_request("start-request-1")
            .is_some_and(|result| result.reused && result.session_id == first.to_string()));
    }

    #[test]
    fn repeated_request_recovers_recent_terminal_history_while_another_request_runs() {
        let coordinator = QrLoginCoordinator::default();
        let operations = SyncCoordinator::default();
        let first = match coordinator
            .start_or_recover("lost-response-request".to_string(), &operations)
            .unwrap()
        {
            StartDecision::New { session_id, permit } => {
                drop(permit);
                session_id
            }
            StartDecision::Reused { .. } => unreachable!(),
        };
        coordinator.complete(first, failed_completion(first));
        let running_permit = match coordinator
            .start_or_recover("another-request".to_string(), &operations)
            .unwrap()
        {
            StartDecision::New { permit, .. } => permit,
            StartDecision::Reused { .. } => unreachable!(),
        };

        let recovered = coordinator
            .start_or_recover("lost-response-request".to_string(), &operations)
            .unwrap();
        assert!(matches!(
            recovered,
            StartDecision::Reused { session_id } if session_id == first
        ));
        assert_eq!(operations.status(), OperationStatus::Account);
        drop(running_permit);
    }

    #[test]
    fn failed_session_allows_a_new_request_to_start_immediately() {
        let coordinator = QrLoginCoordinator::default();
        let failed = coordinator.start().unwrap();
        coordinator.complete(failed, failed_completion(failed));

        let replacement = coordinator.start().unwrap();
        assert_ne!(failed, replacement);
    }

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
