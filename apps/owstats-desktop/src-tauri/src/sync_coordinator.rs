use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

const SYNC_IN_PROGRESS_MESSAGE: &str = "账号操作或赛季同步正在进行，请等待完成后再试";
pub const QUERY_BLOCKS_SYNC_MESSAGE: &str = "请先退出玩家查询";
pub const SYNC_BLOCKS_QUERY_MESSAGE: &str = "当前账号正在同步，请等待完成";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveOperation {
    Idle,
    AccountOperation,
    ForegroundSync,
    BackgroundQueued,
    BackgroundRunning,
    QuerySession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationStatus {
    Idle,
    Account,
    ForegroundSync,
    BackgroundQueued,
    BackgroundRunning,
    Query,
}

#[derive(Clone)]
pub struct SyncCoordinator {
    active: Arc<Mutex<ActiveOperation>>,
    idle: Arc<Notify>,
}

impl Default for SyncCoordinator {
    fn default() -> Self {
        Self {
            active: Arc::new(Mutex::new(ActiveOperation::Idle)),
            idle: Arc::new(Notify::new()),
        }
    }
}

pub struct OperationPermit {
    active: Arc<Mutex<ActiveOperation>>,
    idle: Arc<Notify>,
}

pub struct BackgroundSyncPermit {
    permit: OperationPermit,
}

pub struct QuerySessionPermit {
    #[allow(dead_code)] // consumed by the player-query session module in T2
    permit: OperationPermit,
}

impl SyncCoordinator {
    /// Waits without polling until no account, query, or sync operation owns
    /// the coordinator. The pre-registered notification avoids a lost wakeup.
    pub async fn wait_until_idle(&self) {
        loop {
            let notified = self.idle.notified();
            if self.status() == OperationStatus::Idle {
                return;
            }
            notified.await;
        }
    }

    pub fn status(&self) -> OperationStatus {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match *active {
            ActiveOperation::Idle => OperationStatus::Idle,
            ActiveOperation::AccountOperation => OperationStatus::Account,
            ActiveOperation::ForegroundSync => OperationStatus::ForegroundSync,
            ActiveOperation::BackgroundQueued => OperationStatus::BackgroundQueued,
            ActiveOperation::BackgroundRunning => OperationStatus::BackgroundRunning,
            ActiveOperation::QuerySession => OperationStatus::Query,
        }
    }
    pub fn try_acquire_account_operation(&self) -> Result<OperationPermit, &'static str> {
        self.try_acquire(ActiveOperation::AccountOperation, |_| {
            SYNC_IN_PROGRESS_MESSAGE
        })
    }

    pub fn try_acquire_sync(&self) -> Result<OperationPermit, &'static str> {
        self.try_acquire_sync_kind(ActiveOperation::ForegroundSync)
    }

    /// Notification-driven acquisition for automatic work. Unlike a fixed
    /// retry loop, contention cannot make the refresh disappear.
    pub async fn acquire_sync(&self) -> OperationPermit {
        loop {
            self.wait_until_idle().await;
            if let Ok(permit) = self.try_acquire_sync() {
                return permit;
            }
        }
    }

    /// Atomically reserves the coordinator before a background task is spawned.
    /// Holding this permit represents both its queued and running lifetime.
    pub fn try_register_background_sync(&self) -> Result<BackgroundSyncPermit, &'static str> {
        self.try_acquire_sync_kind(ActiveOperation::BackgroundQueued)
            .map(|permit| BackgroundSyncPermit { permit })
    }

    /// Acquires the permit that must live for the complete in-memory query session.
    pub fn try_acquire_query_session(&self) -> Result<QuerySessionPermit, &'static str> {
        self.try_acquire(ActiveOperation::QuerySession, |_| SYNC_BLOCKS_QUERY_MESSAGE)
            .map(|permit| QuerySessionPermit { permit })
    }

    fn try_acquire_sync_kind(
        &self,
        operation: ActiveOperation,
    ) -> Result<OperationPermit, &'static str> {
        self.try_acquire(operation, |active| {
            if active == ActiveOperation::QuerySession {
                QUERY_BLOCKS_SYNC_MESSAGE
            } else {
                SYNC_IN_PROGRESS_MESSAGE
            }
        })
    }

    fn try_acquire<F>(
        &self,
        operation: ActiveOperation,
        rejection: F,
    ) -> Result<OperationPermit, &'static str>
    where
        F: FnOnce(ActiveOperation) -> &'static str,
    {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *active != ActiveOperation::Idle {
            return Err(rejection(*active));
        }
        *active = operation;
        Ok(OperationPermit {
            active: Arc::clone(&self.active),
            idle: Arc::clone(&self.idle),
        })
    }
}

impl BackgroundSyncPermit {
    pub fn mark_running(&mut self) {
        let mut active = self
            .permit
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert_eq!(*active, ActiveOperation::BackgroundQueued);
        if *active == ActiveOperation::BackgroundQueued {
            *active = ActiveOperation::BackgroundRunning;
        }
    }
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = ActiveOperation::Idle;
        drop(active);
        self.idle.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn account_operation_blocks_sync_until_permit_is_dropped() {
        let coordinator = SyncCoordinator::default();
        let guard = coordinator.try_acquire_account_operation().unwrap();
        assert!(coordinator.try_acquire_sync().is_err());
        drop(guard);
        assert!(coordinator.try_acquire_sync().is_ok());
    }

    #[test]
    fn sync_blocks_query_until_permit_is_dropped() {
        let coordinator = SyncCoordinator::default();
        let guard = coordinator.try_acquire_sync().unwrap();
        assert!(matches!(
            coordinator.try_acquire_query_session(),
            Err(SYNC_BLOCKS_QUERY_MESSAGE)
        ));
        drop(guard);
        assert!(coordinator.try_acquire_query_session().is_ok());
    }

    #[test]
    fn status_is_a_secret_free_operation_category() {
        let coordinator = SyncCoordinator::default();
        assert_eq!(coordinator.status(), OperationStatus::Idle);
        let query = coordinator.try_acquire_query_session().unwrap();
        assert_eq!(coordinator.status(), OperationStatus::Query);
        let json = serde_json::to_string(&coordinator.status()).unwrap();
        assert_eq!(json, "\"query\"");
        drop(query);
        assert_eq!(coordinator.status(), OperationStatus::Idle);
    }

    #[test]
    fn query_blocks_foreground_and_background_sync_until_released() {
        let coordinator = SyncCoordinator::default();
        let query = coordinator.try_acquire_query_session().unwrap();
        assert!(matches!(
            coordinator.try_acquire_sync(),
            Err(QUERY_BLOCKS_SYNC_MESSAGE)
        ));
        assert!(matches!(
            coordinator.try_register_background_sync(),
            Err(QUERY_BLOCKS_SYNC_MESSAGE)
        ));
        drop(query);
        assert!(coordinator.try_acquire_sync().is_ok());
    }

    #[test]
    fn queued_and_running_background_sync_both_block_query() {
        let coordinator = SyncCoordinator::default();
        let mut background = coordinator.try_register_background_sync().unwrap();
        assert!(coordinator.try_acquire_query_session().is_err());
        background.mark_running();
        assert!(coordinator.try_acquire_query_session().is_err());
        drop(background);
        assert!(coordinator.try_acquire_query_session().is_ok());
    }

    #[test]
    fn background_registration_is_atomic_with_query_acquisition() {
        for _ in 0..100 {
            let coordinator = Arc::new(SyncCoordinator::default());
            let barrier = Arc::new(Barrier::new(3));
            let background = {
                let coordinator = Arc::clone(&coordinator);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    coordinator.try_register_background_sync().ok()
                })
            };
            let query = {
                let coordinator = Arc::clone(&coordinator);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    coordinator.try_acquire_query_session().ok()
                })
            };
            barrier.wait();
            assert_ne!(
                background.join().unwrap().is_some(),
                query.join().unwrap().is_some()
            );
        }
    }

    #[test]
    fn panic_unwind_releases_permits() {
        let coordinator = SyncCoordinator::default();
        let copy = coordinator.clone();
        assert!(std::panic::catch_unwind(move || {
            let _permit = copy.try_acquire_query_session().unwrap();
            panic!("synthetic failure");
        })
        .is_err());
        let copy = coordinator.clone();
        assert!(std::panic::catch_unwind(move || {
            let _permit = copy.try_acquire_sync().unwrap();
            panic!("synthetic failure");
        })
        .is_err());
        assert!(coordinator.try_acquire_query_session().is_ok());
    }

    #[tokio::test]
    async fn idle_waiter_is_notified_when_the_active_permit_is_released() {
        let coordinator = SyncCoordinator::default();
        let permit = coordinator.try_acquire_query_session().unwrap();
        let waiter = {
            let coordinator = coordinator.clone();
            tokio::spawn(async move { coordinator.wait_until_idle().await })
        };
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        drop(permit);

        tokio::time::timeout(std::time::Duration::from_millis(100), waiter)
            .await
            .expect("permit release should wake queued automatic update")
            .unwrap();
    }
}
