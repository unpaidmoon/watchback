use crate::OwDatabase;
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct LiveSeasonKey {
    account_scope: String,
    season: i64,
}

impl LiveSeasonKey {
    pub(crate) fn new(account_scope: impl Into<String>, season: i64) -> Self {
        Self {
            account_scope: account_scope.into(),
            season,
        }
    }

    pub(crate) fn account_scope(&self) -> &str {
        &self.account_scope
    }

    pub(crate) fn season(&self) -> i64 {
        self.season
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum LiveSeasonRunStatus {
    Queued,
    Running,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveSeasonRunReceipt {
    id: u64,
    key: LiveSeasonKey,
}

impl LiveSeasonRunReceipt {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn key(&self) -> &LiveSeasonKey {
        &self.key
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct LiveSeasonQueueStatus {
    pub(crate) running: usize,
    pub(crate) queued: usize,
}

struct LiveSeasonSlot {
    gate: Arc<Semaphore>,
    queued: AtomicUsize,
    running: AtomicUsize,
    cancel_epoch: AtomicU64,
    cancelled: Notify,
}

impl Default for LiveSeasonSlot {
    fn default() -> Self {
        Self {
            gate: Arc::new(Semaphore::new(1)),
            queued: AtomicUsize::new(0),
            running: AtomicUsize::new(0),
            cancel_epoch: AtomicU64::new(0),
            cancelled: Notify::new(),
        }
    }
}

#[derive(Default)]
struct LiveSeasonUpdateState {
    slots: Mutex<HashMap<LiveSeasonKey, Arc<LiveSeasonSlot>>>,
    runs: Mutex<HashMap<u64, LiveSeasonRunStatus>>,
    next_run_id: AtomicU64,
}

/// Admission and lifecycle authority for live season refreshes.
///
/// Calls with the same account/season key run FIFO, while distinct keys are
/// independently admitted. Dropping the returned permit completes the run and
/// releases the next queued request; callers do not implement queue logic.
#[derive(Clone, Default)]
pub(crate) struct LiveSeasonUpdateManager {
    state: Arc<LiveSeasonUpdateState>,
}

impl LiveSeasonUpdateManager {
    pub(crate) async fn admit(&self, key: LiveSeasonKey) -> Option<LiveSeasonRunPermit> {
        self.enqueue(key)?.wait().await
    }

    /// Enqueue at most one follow-up refresh for a key. Repeated startup,
    /// account-switch, or manual triggers coalesce into that pending refresh.
    pub(crate) fn enqueue(&self, key: LiveSeasonKey) -> Option<LiveSeasonAdmission> {
        let slot = {
            self.state
                .slots
                .lock()
                .expect("live season slots lock poisoned")
                .entry(key.clone())
                .or_default()
                .clone()
        };
        if slot
            .queued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |queued| {
                (queued == 0).then_some(1)
            })
            .is_err()
        {
            return None;
        }
        let id = self.state.next_run_id.fetch_add(1, Ordering::Relaxed) + 1;
        let receipt = LiveSeasonRunReceipt { id, key };
        self.state
            .runs
            .lock()
            .expect("live season runs lock poisoned")
            .insert(id, LiveSeasonRunStatus::Queued);
        let cancel_epoch = slot.cancel_epoch.load(Ordering::Acquire);
        Some(LiveSeasonAdmission {
            state: Arc::clone(&self.state),
            slot,
            receipt,
            queued: true,
            cancel_epoch,
        })
    }

    /// Cancel pending refreshes belonging to an account that is no longer
    /// active. A running transaction remains account-scoped and may finish.
    pub(crate) fn cancel_queued_account(&self, account_scope: &str) {
        let slots = self
            .state
            .slots
            .lock()
            .expect("live season slots lock poisoned")
            .iter()
            .filter(|(key, _)| key.account_scope() == account_scope)
            .map(|(_, slot)| Arc::clone(slot))
            .collect::<Vec<_>>();
        for slot in slots {
            slot.cancel_epoch.fetch_add(1, Ordering::AcqRel);
            slot.cancelled.notify_waiters();
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn status(&self, key: &LiveSeasonKey) -> LiveSeasonQueueStatus {
        let slot = self
            .state
            .slots
            .lock()
            .expect("live season slots lock poisoned")
            .get(key)
            .cloned();
        slot.map_or_else(LiveSeasonQueueStatus::default, |slot| {
            LiveSeasonQueueStatus {
                running: slot.running.load(Ordering::Acquire),
                queued: slot.queued.load(Ordering::Acquire),
            }
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn run_status(&self, receipt: &LiveSeasonRunReceipt) -> LiveSeasonRunStatus {
        self.state
            .runs
            .lock()
            .expect("live season runs lock poisoned")
            .get(&receipt.id)
            .copied()
            .unwrap_or(LiveSeasonRunStatus::Completed)
    }

    pub(crate) fn list_commit_event(
        &self,
        db: &OwDatabase,
        key: &LiveSeasonKey,
        commit: &crate::models::QuickListCommitSummary,
    ) -> Result<Option<crate::models::SeasonDataRevisionEvent>> {
        if !commit.data_changed {
            return Ok(None);
        }
        Ok(Some(crate::models::SeasonDataRevisionEvent {
            account_scope: key.account_scope().to_string(),
            season: key.season(),
            revision: season_revision(db, key.season())?,
            phase: "list".into(),
            new_matches: commit.new_list_matches,
            pending_details: commit.pending_details,
            status: if commit.partial_failure {
                "partialFailure"
            } else {
                "running"
            }
            .into(),
        }))
    }
}

pub(crate) struct LiveSeasonAdmission {
    state: Arc<LiveSeasonUpdateState>,
    slot: Arc<LiveSeasonSlot>,
    receipt: LiveSeasonRunReceipt,
    queued: bool,
    cancel_epoch: u64,
}

impl LiveSeasonAdmission {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn receipt(&self) -> &LiveSeasonRunReceipt {
        &self.receipt
    }

    pub(crate) async fn wait(mut self) -> Option<LiveSeasonRunPermit> {
        let permit = loop {
            if self.slot.cancel_epoch.load(Ordering::Acquire) != self.cancel_epoch {
                return None;
            }
            tokio::select! {
                permit = Arc::clone(&self.slot.gate).acquire_owned() => {
                    break permit.expect("live season admission gate is never closed");
                }
                _ = self.slot.cancelled.notified() => {}
            }
        };
        self.slot.queued.fetch_sub(1, Ordering::AcqRel);
        self.queued = false;
        self.slot.running.fetch_add(1, Ordering::AcqRel);
        self.state
            .runs
            .lock()
            .expect("live season runs lock poisoned")
            .insert(self.receipt.id, LiveSeasonRunStatus::Running);
        Some(LiveSeasonRunPermit {
            state: Arc::clone(&self.state),
            slot: Arc::clone(&self.slot),
            receipt: self.receipt.clone(),
            _permit: permit,
        })
    }
}

impl Drop for LiveSeasonAdmission {
    fn drop(&mut self) {
        if !self.queued {
            return;
        }
        self.slot.queued.fetch_sub(1, Ordering::AcqRel);
        self.state
            .runs
            .lock()
            .expect("live season runs lock poisoned")
            .remove(&self.receipt.id);
    }
}

pub(crate) struct LiveSeasonRunPermit {
    state: Arc<LiveSeasonUpdateState>,
    slot: Arc<LiveSeasonSlot>,
    receipt: LiveSeasonRunReceipt,
    _permit: OwnedSemaphorePermit,
}

impl LiveSeasonRunPermit {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn receipt(&self) -> &LiveSeasonRunReceipt {
        &self.receipt
    }
}

impl Drop for LiveSeasonRunPermit {
    fn drop(&mut self) {
        self.slot.running.fetch_sub(1, Ordering::AcqRel);
        self.state
            .runs
            .lock()
            .expect("live season runs lock poisoned")
            .remove(&self.receipt.id);
    }
}

/// Read after a per-mode transaction commits, so an emitted revision always
/// refers to rows already visible to a fresh SQLite read transaction.
pub(crate) fn season_revision(db: &OwDatabase, season: i64) -> Result<i64> {
    db.initialize()?;
    Ok(db.connect()?.query_row(
        "SELECT season_revision FROM season_sync_state WHERE season=?1",
        [season],
        |row| row.get(0),
    )?)
}

/// A stable database-backed handshake token for the seasons represented by a
/// timeline query. Events reduce latency; this token recovers missed events.
pub(crate) fn revision_token(db: &OwDatabase, season: Option<i64>) -> Result<String> {
    db.initialize()?;
    let conn = db.connect()?;
    if let Some(season) = season {
        let revision = conn
            .query_row(
                "SELECT season_revision FROM season_sync_state WHERE season=?1",
                params![season],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        return Ok(format!("{season}:{revision}"));
    }
    let mut statement =
        conn.prepare("SELECT season, season_revision FROM season_sync_state ORDER BY season ASC")?;
    let tokens = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|(season, revision)| format!("{season}:{revision}"))
        .collect::<Vec<_>>();
    Ok(tokens.join(";"))
}

#[cfg(test)]
mod tests {
    use super::{revision_token, LiveSeasonKey, LiveSeasonRunStatus, LiveSeasonUpdateManager};
    use crate::OwDatabase;

    #[test]
    fn revision_handshake_is_season_specific_and_collision_free() {
        let temp = tempfile::tempdir().unwrap();
        let db = OwDatabase::new(temp.path().join("revision.sqlite3"));
        db.initialize().unwrap();
        let conn = db.connect().unwrap();
        conn.execute(
            "INSERT INTO season_sync_state(season, season_revision, snapshot_revision, background_status, updated_at) VALUES(21, 4, 0, 'idle', 'now')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO season_sync_state(season, season_revision, snapshot_revision, background_status, updated_at) VALUES(22, 9, 0, 'idle', 'now')",
            [],
        ).unwrap();
        drop(conn);

        assert_eq!(revision_token(&db, Some(21)).unwrap(), "21:4");
        assert_eq!(revision_token(&db, Some(23)).unwrap(), "23:0");
        assert_eq!(revision_token(&db, None).unwrap(), "21:4;22:9");
    }

    #[tokio::test]
    async fn manager_queues_the_same_account_and_season_without_dropping_the_request() {
        let manager = LiveSeasonUpdateManager::default();
        let key = LiveSeasonKey::new("account-a", 23);
        let first = manager.admit(key.clone()).await.unwrap();
        let first_receipt = first.receipt().clone();

        let queued_manager = manager.clone();
        let queued_key = key.clone();
        let queued = tokio::spawn(async move { queued_manager.admit(queued_key).await });
        tokio::task::yield_now().await;

        assert_eq!(manager.status(&key).running, 1);
        assert_eq!(manager.status(&key).queued, 1);
        assert_eq!(
            manager.run_status(&first_receipt),
            LiveSeasonRunStatus::Running
        );

        drop(first);
        let second = queued.await.unwrap().unwrap();
        assert_eq!(second.receipt().key(), &key);
        assert_eq!(manager.status(&key).running, 1);
        assert_eq!(manager.status(&key).queued, 0);
        drop(second);
        assert_eq!(manager.status(&key).running, 0);
    }

    #[tokio::test]
    async fn manager_admits_distinct_accounts_independently() {
        let manager = LiveSeasonUpdateManager::default();
        let first = manager
            .admit(LiveSeasonKey::new("account-a", 23))
            .await
            .unwrap();
        let second = manager
            .admit(LiveSeasonKey::new("account-b", 23))
            .await
            .unwrap();
        assert_eq!(
            manager.run_status(first.receipt()),
            LiveSeasonRunStatus::Running
        );
        assert_eq!(
            manager.run_status(second.receipt()),
            LiveSeasonRunStatus::Running
        );
    }

    #[tokio::test]
    async fn enqueue_returns_a_receipt_before_the_run_is_admitted() {
        let manager = LiveSeasonUpdateManager::default();
        let key = LiveSeasonKey::new("account-a", 23);
        let first = manager.admit(key.clone()).await.unwrap();
        let queued = manager.enqueue(key).unwrap();
        assert_eq!(
            manager.run_status(queued.receipt()),
            LiveSeasonRunStatus::Queued
        );
        drop(first);
        let running = queued.wait().await.unwrap();
        assert_eq!(
            manager.run_status(running.receipt()),
            LiveSeasonRunStatus::Running
        );
    }

    #[tokio::test]
    async fn dropping_a_queued_admission_cancels_it_cleanly() {
        let manager = LiveSeasonUpdateManager::default();
        let key = LiveSeasonKey::new("account-a", 23);
        let first = manager.admit(key.clone()).await.unwrap();
        let queued = manager.enqueue(key.clone()).unwrap();
        let receipt = queued.receipt().clone();
        drop(queued);
        assert_eq!(manager.status(&key).queued, 0);
        assert_eq!(manager.run_status(&receipt), LiveSeasonRunStatus::Completed);
        drop(first);
    }

    #[tokio::test]
    async fn manager_coalesces_more_than_one_pending_refresh() {
        let manager = LiveSeasonUpdateManager::default();
        let key = LiveSeasonKey::new("account-a", 23);
        let running = manager.admit(key.clone()).await.unwrap();
        let queued = manager.enqueue(key.clone()).unwrap();

        assert!(manager.enqueue(key.clone()).is_none());
        assert_eq!(manager.status(&key).running, 1);
        assert_eq!(manager.status(&key).queued, 1);

        drop(running);
        drop(queued.wait().await.unwrap());
    }

    #[tokio::test]
    async fn account_switch_cancels_the_old_accounts_pending_refresh() {
        let manager = LiveSeasonUpdateManager::default();
        let key = LiveSeasonKey::new("account-a", 23);
        let running = manager.admit(key.clone()).await.unwrap();
        let queued = manager.enqueue(key.clone()).unwrap();
        manager.cancel_queued_account("account-a");

        assert!(queued.wait().await.is_none());
        assert_eq!(manager.status(&key).queued, 0);
        drop(running);
    }
}
