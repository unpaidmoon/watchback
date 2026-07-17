use crate::app_paths::AccountPaths;
use crate::dashen_client::DashenCredentials;
use crate::season_fetch::FullSeasonRequest;
use crate::sync::{run_background_job_once, SeasonSyncStore};
use crate::OwDatabase;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

const DASHEN_CUSTOMER_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool/customer";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AccountSeasonKey {
    account_scope: String,
    season: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackgroundEvent {
    account_scope: String,
    season: i64,
    phase: String,
    status: String,
    new_matches: usize,
    pending_details: usize,
}

#[derive(Clone)]
pub struct BackgroundSeasonSyncManager {
    running: Arc<Mutex<HashMap<AccountSeasonKey, Arc<Notify>>>>,
    operation_coordinator: crate::sync_coordinator::SyncCoordinator,
}

pub type BackgroundEventSink = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;

impl BackgroundSeasonSyncManager {
    pub(crate) fn new(operation_coordinator: crate::sync_coordinator::SyncCoordinator) -> Self {
        Self {
            running: Arc::default(),
            operation_coordinator,
        }
    }

    pub fn wake(
        &self,
        account_scope: &str,
        paths: AccountPaths,
        season: i64,
        events: BackgroundEventSink,
    ) -> bool {
        self.wake_internal(
            account_scope,
            paths,
            season,
            DASHEN_CUSTOMER_BASE.to_string(),
            Some(events),
        )
    }

    #[doc(hidden)]
    pub fn wake_headless(
        &self,
        account_scope: &str,
        paths: AccountPaths,
        season: i64,
        customer_base_url: String,
    ) -> bool {
        self.wake_internal(account_scope, paths, season, customer_base_url, None)
    }

    fn wake_internal(
        &self,
        account_scope: &str,
        paths: AccountPaths,
        season: i64,
        customer_base_url: String,
        events: Option<BackgroundEventSink>,
    ) -> bool {
        let key = AccountSeasonKey {
            account_scope: account_scope.to_string(),
            season,
        };
        let notify = Arc::new(Notify::new());
        {
            let Ok(mut running) = self.running.lock() else {
                return false;
            };
            if running.contains_key(&key) {
                return false;
            }
            running.insert(key.clone(), Arc::clone(&notify));
        }
        let manager = self.clone();
        tokio::spawn(async move {
            manager
                .run_task(&key, paths, customer_base_url, events.as_ref())
                .await;
            if let Ok(mut running) = manager.running.lock() {
                running.remove(&key);
            }
            notify.notify_waiters();
        });
        true
    }

    async fn run_task(
        &self,
        key: &AccountSeasonKey,
        paths: AccountPaths,
        customer_base_url: String,
        events: Option<&BackgroundEventSink>,
    ) {
        let _permit = loop {
            match self.operation_coordinator.try_acquire_sync() {
                Ok(permit) => break permit,
                Err(_) => {
                    emit(
                        events,
                        "season-background-progress",
                        key,
                        "queued",
                        "queued",
                        0,
                        0,
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        };
        let db = OwDatabase::for_account(&paths);
        let store = SeasonSyncStore::new(db);
        let _ = store.requeue_orphaned_running();
        emit(
            events,
            "season-background-progress",
            key,
            "continuePages",
            "running",
            0,
            store
                .season_status(key.season)
                .map(|status| status.pending_details)
                .unwrap_or(0),
        );
        let request = match request_for_account(&paths, key.season, customer_base_url) {
            Ok(request) => request,
            Err(_) => {
                emit(
                    events,
                    "season-background-progress",
                    key,
                    "prepare",
                    "failed",
                    0,
                    0,
                );
                return;
            }
        };
        match run_background_job_once(&store, &request, &paths.seasons()).await {
            Ok(summary) => {
                if summary.new_list_matches > 0 {
                    emit(
                        events,
                        "season-list-updated",
                        key,
                        "continuePages",
                        "running",
                        summary.new_list_matches,
                        summary.details_written + summary.failed_details,
                    );
                }
                if summary.details_written > 0 {
                    emit(
                        events,
                        "season-detail-updated",
                        key,
                        "fetchDetails",
                        "running",
                        0,
                        summary.failed_details,
                    );
                }
                if summary.failed_details > 0 {
                    emit(
                        events,
                        "season-background-progress",
                        key,
                        "fetchDetails",
                        "failed",
                        summary.new_list_matches,
                        summary.failed_details,
                    );
                } else {
                    emit(
                        events,
                        "season-background-completed",
                        key,
                        "completed",
                        "completed",
                        summary.new_list_matches,
                        0,
                    );
                }
            }
            Err(_) => emit(
                events,
                "season-background-progress",
                key,
                "background",
                "failed",
                0,
                store
                    .season_status(key.season)
                    .map(|status| status.pending_details)
                    .unwrap_or(0),
            ),
        }
    }

    pub async fn wait_for_idle(&self, account_scope: &str, season: i64) {
        let key = AccountSeasonKey {
            account_scope: account_scope.to_string(),
            season,
        };
        loop {
            let notify = self
                .running
                .lock()
                .ok()
                .and_then(|running| running.get(&key).cloned());
            let Some(notify) = notify else {
                return;
            };
            notify.notified().await;
        }
    }

    pub async fn stop_account(&self, account_scope: &str) {
        loop {
            let notifies = self
                .running
                .lock()
                .map(|running| {
                    running
                        .iter()
                        .filter(|(key, _)| key.account_scope == account_scope)
                        .map(|(_, notify)| Arc::clone(notify))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if notifies.is_empty() {
                return;
            }
            for notify in notifies {
                notify.notified().await;
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.running
            .lock()
            .map(|running| running.len())
            .unwrap_or(0)
    }
}

impl Default for BackgroundSeasonSyncManager {
    fn default() -> Self {
        Self::new(crate::sync_coordinator::SyncCoordinator::default())
    }
}

fn request_for_account(
    paths: &AccountPaths,
    season: i64,
    customer_base_url: String,
) -> anyhow::Result<FullSeasonRequest> {
    let credentials = crate::credentials::read_saved_credentials_for_account(paths)?;
    let target = crate::player_target::read_player_target_for_path(&paths.player_target())?;
    if !crate::player_target::target_matches_login_role(&target, credentials.role_id) {
        anyhow::bail!(crate::login_target::SYNC_TARGET_CONFIRMATION_ERROR);
    }
    Ok(FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: credentials.role_id,
            token: credentials.token,
            dts: credentials.dts.and_then(|value| value.parse().ok()),
            server: credentials.server,
        },
        customer_token: target.customer_token,
        input_bnet: target.input_bnet,
        resolved_bnet: target.resolved_bnet,
        season,
        customer_base_url,
        pages_per_batch: 5,
        detail_concurrency: 2,
        include_fight: true,
    })
}

fn emit(
    events: Option<&BackgroundEventSink>,
    event: &str,
    key: &AccountSeasonKey,
    phase: &str,
    status: &str,
    new_matches: usize,
    pending_details: usize,
) {
    let Some(events) = events else {
        return;
    };
    if let Ok(payload) = serde_json::to_value(BackgroundEvent {
        account_scope: key.account_scope.clone(),
        season: key.season,
        phase: phase.to_string(),
        status: status.to_string(),
        new_matches,
        pending_details,
    }) {
        events(event, payload);
    }
}

#[cfg(test)]
mod tests {
    use super::BackgroundEvent;

    #[test]
    fn event_payloads_are_anonymous_and_bounded() {
        let value = serde_json::to_value(BackgroundEvent {
            account_scope: "anonymous-account-scope".to_string(),
            season: 22,
            phase: "fetchDetails".to_string(),
            status: "running".to_string(),
            new_matches: 3,
            pending_details: 2,
        })
        .unwrap();
        let keys = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "accountScope",
                "newMatches",
                "pendingDetails",
                "phase",
                "season",
                "status"
            ]
        );
        let serialized = value.to_string().to_ascii_lowercase();
        for forbidden in [
            "matchid",
            "playerid",
            "bnet",
            "token",
            "authorization",
            "cookie",
            "raw",
            "url",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }
}
