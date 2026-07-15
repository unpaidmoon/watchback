use crate::account_store::{read_registry, write_registry_atomic, AccountId, AccountRegistry};
use crate::app_paths::{AccountPaths, SharedAppPaths};
use crate::models::{AccountManagerStatus, AccountSummary, RemoveAccountResult};
use anyhow::{anyhow, Context, Result};
use std::sync::{Arc, Mutex, RwLock};

const NO_ACTIVE_ACCOUNT_MESSAGE: &str = "尚未添加可用账号，请先在指挥台添加账号。";
pub(crate) const ACCOUNT_DELETION_RECOVERY_ERROR: &str = "账号退出恢复失败，请重试。";
const ACCOUNT_DELETION_FAIL_CLOSED_ERROR: &str = "账号删除恢复尚未完成，请重启应用后重试。";

#[derive(Clone)]
pub struct AccountContextManager {
    shared: SharedAppPaths,
    registry: Arc<RwLock<AccountRegistry>>,
    recovery_failed: Arc<RwLock<bool>>,
    migration_lock: Arc<Mutex<()>>,
    login_lock: Arc<Mutex<()>>,
    deletion_recovery_required: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct AccountContextSnapshot {
    account_id: AccountId,
    paths: AccountPaths,
}

impl AccountContextManager {
    pub fn load(shared: SharedAppPaths) -> Result<Self> {
        crate::account_login::recover_pending_logins(&shared)
            .map_err(|_| anyhow!("账号登录恢复失败，请重试。"))?;
        crate::account_removal::recover_pending_removal(&shared)
            .map_err(|_| anyhow!("账号退出恢复失败，请重试。"))?;
        let eligibility_failed =
            crate::account_migration::prepare_legacy_eligibility(&shared).is_err();
        let recovery_failed = eligibility_failed
            || crate::account_migration::recover_interrupted_migration(&shared).is_err();
        let registry_path = shared.registry();
        let registry = if registry_path.try_exists().context("无法检查账号索引")? {
            read_registry(&registry_path).context("无法读取账号索引")?
        } else {
            AccountRegistry::default()
        };

        Ok(Self {
            shared,
            registry: Arc::new(RwLock::new(registry)),
            recovery_failed: Arc::new(RwLock::new(recovery_failed)),
            migration_lock: Arc::new(Mutex::new(())),
            login_lock: Arc::new(Mutex::new(())),
            deletion_recovery_required: Arc::new(RwLock::new(false)),
        })
    }

    pub fn active_snapshot(&self) -> Result<AccountContextSnapshot> {
        self.ensure_deletion_ready()?;
        let registry = self.registry_snapshot()?;
        let account_id = registry
            .active_account_id
            .ok_or_else(|| anyhow!(NO_ACTIVE_ACCOUNT_MESSAGE))?;
        let paths = self.shared.account(&account_id);
        Ok(AccountContextSnapshot { account_id, paths })
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>> {
        self.ensure_deletion_ready()?;
        let registry = self.registry_snapshot()?;
        let active_account_id = registry.active_account_id.as_ref();
        Ok(registry
            .accounts
            .into_iter()
            .map(|account| AccountSummary {
                is_active: active_account_id == Some(&account.account_id),
                account_id: account.account_id.as_str().to_string(),
                battle_tag: account.battle_tag,
                credential_state: account.credential_state,
                created_at: account.created_at,
                last_used_at: account.last_used_at,
            })
            .collect())
    }

    pub fn status(&self) -> Result<AccountManagerStatus> {
        self.ensure_deletion_ready()?;
        let registry = self.registry_snapshot()?;
        let active_account_id = registry.active_account_id.as_ref();
        let accounts = registry
            .accounts
            .iter()
            .map(|account| AccountSummary {
                is_active: active_account_id == Some(&account.account_id),
                account_id: account.account_id.as_str().to_string(),
                battle_tag: account.battle_tag.clone(),
                credential_state: account.credential_state.clone(),
                created_at: account.created_at.clone(),
                last_used_at: account.last_used_at.clone(),
            })
            .collect::<Vec<_>>();
        let message = if accounts.is_empty() {
            "暂无账号，请添加账号。"
        } else {
            "账号列表已就绪。"
        };
        Ok(AccountManagerStatus {
            active_account_id: registry
                .active_account_id
                .map(|account_id| account_id.as_str().to_string()),
            accounts,
            message: message.to_string(),
        })
    }

    pub fn switch_active(&self, account_id: &AccountId, now: &str) -> Result<AccountManagerStatus> {
        let _mutation = self.lock_login_publication()?;
        let mut registry = self.registry_snapshot()?;
        crate::account_removal::validate_switch_target(&self.shared, &registry, account_id)?;
        let target = registry
            .accounts
            .iter_mut()
            .find(|entry| &entry.account_id == account_id)
            .ok_or_else(|| anyhow!("账号不存在。"))?;
        target.last_used_at = now.to_string();
        registry.active_account_id = Some(account_id.clone());
        self.replace_registry(registry)?;
        self.status()
    }

    pub fn remove_account(&self, account_id: &AccountId, now: &str) -> Result<RemoveAccountResult> {
        let _mutation = self.lock_login_publication()?;
        let registry = self.registry_snapshot()?;
        let removal =
            crate::account_removal::remove_account(&self.shared, &registry, account_id, now);
        self.finish_removal(removal)
    }

    #[cfg(test)]
    fn remove_account_with_test_fault(
        &self,
        account_id: &AccountId,
        fault: crate::account_removal::TestRemovalFault,
    ) -> Result<RemoveAccountResult> {
        let _mutation = self.lock_login_publication()?;
        let registry = self.registry_snapshot()?;
        let removal = crate::account_removal::remove_account_with_test_fault(
            &self.shared,
            &registry,
            account_id,
            "2026-07-16T13:00:00Z",
            fault,
        );
        self.finish_removal(removal)
    }

    fn finish_removal(
        &self,
        removal: Result<crate::account_removal::RemovalOutcome>,
    ) -> Result<RemoveAccountResult> {
        let disk_registry = match std::fs::symlink_metadata(self.shared.registry()) {
            Ok(_) => read_registry(&self.shared.registry())
                .map_err(|_| anyhow!("账号状态恢复失败，请重试。"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.registry_snapshot()?
            }
            Err(_) => return Err(anyhow!("账号状态恢复失败，请重试。")),
        };
        {
            let mut current = self
                .registry
                .write()
                .map_err(|_| anyhow!("账号状态暂不可用，请重试。"))?;
            *current = disk_registry.clone();
        }
        let fail_closed =
            crate::account_removal::deletion_recovery_required(&self.shared, &disk_registry);
        {
            let mut required = self
                .deletion_recovery_required
                .write()
                .map_err(|_| anyhow!(ACCOUNT_DELETION_FAIL_CLOSED_ERROR))?;
            *required = fail_closed;
        }
        let outcome = removal?;
        if outcome.registry != disk_registry {
            return Err(anyhow!("账号状态恢复失败，请重试。"));
        }
        let (removed, message) = match outcome.completion {
            crate::account_removal::RemovalCompletion::Removed => {
                (true, "账号已退出，本地账号数据已清理。")
            }
            crate::account_removal::RemovalCompletion::RemovedPendingCleanup => {
                (true, "账号已退出，剩余本地文件将在重启后继续清理。")
            }
            crate::account_removal::RemovalCompletion::NotRemoved => {
                (false, "账号退出失败，账号未更改。")
            }
        };
        Ok(RemoveAccountResult {
            removed,
            active_account_id: disk_registry
                .active_account_id
                .map(|id| id.as_str().to_string()),
            message: message.to_string(),
            reload_required: removed,
        })
    }

    pub fn registry_snapshot(&self) -> Result<AccountRegistry> {
        self.registry
            .read()
            .map(|registry| registry.clone())
            .map_err(|_| anyhow!("账号状态暂不可用，请重试。"))
    }

    fn ensure_deletion_ready(&self) -> Result<()> {
        let required = self
            .deletion_recovery_required
            .read()
            .map_err(|_| anyhow!(ACCOUNT_DELETION_FAIL_CLOSED_ERROR))?;
        if *required {
            return Err(anyhow!(ACCOUNT_DELETION_FAIL_CLOSED_ERROR));
        }
        Ok(())
    }

    pub fn replace_registry(&self, registry: AccountRegistry) -> Result<()> {
        let mut current = self
            .registry
            .write()
            .map_err(|_| anyhow!("账号状态暂不可用，请重试。"))?;
        write_registry_atomic(&self.shared.registry(), &registry).context("无法保存账号索引")?;
        *current = registry;
        Ok(())
    }

    pub(crate) fn lock_login_publication(&self) -> Result<std::sync::MutexGuard<'_, ()>> {
        let guard = self
            .login_lock
            .lock()
            .map_err(|_| anyhow!("账号操作暂不可用，请重试。"))?;
        crate::account_login::recover_pending_logins(&self.shared)
            .map_err(|_| anyhow!("账号登录恢复失败，请重试。"))?;
        self.recover_deletion_while_locked()?;
        Ok(guard)
    }

    pub(crate) fn prepare_login_capture(&self) -> Result<()> {
        drop(self.lock_login_publication()?);
        Ok(())
    }

    fn recover_deletion_while_locked(&self) -> Result<()> {
        crate::account_removal::recover_pending_removal(&self.shared)
            .map_err(|_| anyhow!(ACCOUNT_DELETION_RECOVERY_ERROR))?;
        match std::fs::symlink_metadata(self.shared.deletion_journal()) {
            Ok(_) => return Err(anyhow!(ACCOUNT_DELETION_RECOVERY_ERROR)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(anyhow!(ACCOUNT_DELETION_RECOVERY_ERROR)),
        }
        match std::fs::symlink_metadata(self.shared.registry()) {
            Ok(_) => {
                let registry = read_registry(&self.shared.registry())
                    .map_err(|_| anyhow!(ACCOUNT_DELETION_RECOVERY_ERROR))?;
                let mut current = self
                    .registry
                    .write()
                    .map_err(|_| anyhow!("账号状态暂不可用，请重试。"))?;
                *current = registry;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(anyhow!(ACCOUNT_DELETION_RECOVERY_ERROR)),
        }
        let mut required = self
            .deletion_recovery_required
            .write()
            .map_err(|_| anyhow!(ACCOUNT_DELETION_FAIL_CLOSED_ERROR))?;
        *required = false;
        Ok(())
    }

    pub(crate) fn snapshot_for(&self, account_id: &AccountId) -> AccountContextSnapshot {
        AccountContextSnapshot {
            account_id: account_id.clone(),
            paths: self.shared.account(account_id),
        }
    }

    pub fn shared_paths(&self) -> &SharedAppPaths {
        &self.shared
    }

    pub fn migration_state(&self) -> Result<crate::account_migration::LegacyMigrationState> {
        let _migration = self
            .migration_lock
            .lock()
            .map_err(|_| anyhow!("旧数据迁移操作暂不可用，请重试。"))?;
        let mut recovery_failed = self
            .recovery_failed
            .write()
            .map_err(|_| anyhow!("旧数据迁移状态暂不可用，请重试。"))?;
        if *recovery_failed {
            if crate::account_migration::recover_interrupted_migration(&self.shared).is_err() {
                return Ok(crate::account_migration::LegacyMigrationState::RecoveryError);
            }
            *recovery_failed = false;
        }
        let state = crate::account_migration::detect_legacy_state(&self.shared)?;
        if state == crate::account_migration::LegacyMigrationState::RecoveryError {
            *recovery_failed = true;
        }
        Ok(state)
    }

    pub fn adopt_legacy(&self, account: &AccountContextSnapshot) -> Result<()> {
        let _lifecycle = self.lock_login_publication()?;
        let _migration = self
            .migration_lock
            .lock()
            .map_err(|_| anyhow!("旧数据迁移操作暂不可用，请重试。"))?;
        let result = crate::account_migration::adopt_legacy_account(&self.shared, account);
        let journal_remains = match std::fs::symlink_metadata(self.shared.migration_journal()) {
            Ok(_) => true,
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        };
        let mut recovery_failed = self
            .recovery_failed
            .write()
            .map_err(|_| anyhow!("旧数据迁移状态暂不可用，请重试。"))?;
        *recovery_failed = result.is_err() && journal_remains;
        result
    }
}

impl AccountContextSnapshot {
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    pub fn paths(&self) -> &AccountPaths {
        &self.paths
    }

    pub fn database(&self) -> crate::OwDatabase {
        crate::OwDatabase::for_account(&self.paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_removal::TestRemovalFault;
    use crate::account_store::{
        read_registry, write_manifest_atomic, AccountManifest, AccountRegistryEntry,
    };
    use std::fs;

    fn manager_fixture() -> (tempfile::TempDir, AccountContextManager, AccountId) {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let paths = shared.account(&id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(
            &paths.manifest(),
            &AccountManifest {
                version: 1,
                account_id: id.clone(),
                role_id: 101101,
                battle_tag: "Synthetic#1000".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
        fs::write(paths.database(), b"synthetic-db").unwrap();
        manager
            .replace_registry(AccountRegistry {
                version: 1,
                active_account_id: Some(id.clone()),
                accounts: vec![AccountRegistryEntry {
                    account_id: id.clone(),
                    battle_tag: "Synthetic#1000".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    last_used_at: "2026-01-02T00:00:00Z".into(),
                    credential_state: "ready".into(),
                }],
            })
            .unwrap();
        (temp, manager, id)
    }

    #[test]
    fn removal_failures_always_reload_memory_from_disk() {
        for (fault, expected_removed) in [
            (TestRemovalFault::BeforeAnyDelete, false),
            (TestRemovalFault::AfterFirstDeleteMutation, true),
            (TestRemovalFault::JournalAfterRegistryUpdated, true),
        ] {
            let (_temp, manager, id) = manager_fixture();
            let result = manager.remove_account_with_test_fault(&id, fault).unwrap();
            assert_eq!(result.removed, expected_removed);
            assert_eq!(
                manager.registry_snapshot().unwrap(),
                read_registry(&manager.shared.registry()).unwrap()
            );
            assert_eq!(
                manager
                    .registry_snapshot()
                    .unwrap()
                    .accounts
                    .iter()
                    .any(|entry| entry.account_id == id),
                !expected_removed
            );
        }
    }

    #[test]
    fn failed_restore_and_failed_removal_commit_fail_closed_until_recovery() {
        let (_temp, manager, id) = manager_fixture();
        assert!(manager
            .remove_account_with_test_fault(&id, TestRemovalFault::RollbackRestoreAndRemovalCommit,)
            .is_err());
        let expected = "账号删除恢复尚未完成，请重启应用后重试。";
        assert_eq!(manager.active_snapshot().unwrap_err().to_string(), expected);
        assert_eq!(manager.status().unwrap_err().to_string(), expected);
        assert_eq!(manager.list_accounts().unwrap_err().to_string(), expected);
        assert!(manager.shared.deletion_journal().is_file());
        assert!(!manager.shared.account(&id).root().exists());
        fs::create_dir_all(manager.shared.account(&id).root()).unwrap();
        fs::write(
            manager.shared.account(&id).root().join("conflict-marker"),
            b"synthetic-conflict",
        )
        .unwrap();
        assert_eq!(manager.active_snapshot().unwrap_err().to_string(), expected);
        assert_eq!(manager.status().unwrap_err().to_string(), expected);

        let restarted = AccountContextManager::load(manager.shared.clone()).unwrap();
        assert!(restarted.registry_snapshot().unwrap().accounts.is_empty());
        assert!(restarted.list_accounts().unwrap().is_empty());
        assert!(!manager.shared.account(&id).root().exists());
        assert!(!manager.shared.deletion_journal().exists());
    }

    #[test]
    fn failed_restore_commits_new_registry_and_syncs_manager_as_pending_removal() {
        let (_temp, manager, id) = manager_fixture();
        let result = manager
            .remove_account_with_test_fault(&id, TestRemovalFault::RollbackRestoreCommitsRemoval)
            .unwrap();
        assert!(result.removed);
        assert!(result.reload_required);
        assert!(result.message.contains("重启后继续清理"));
        let memory = manager.registry_snapshot().unwrap();
        let disk = read_registry(&manager.shared.registry()).unwrap();
        assert_eq!(memory, disk);
        assert!(memory.accounts.is_empty());
        assert!(!manager.shared.account(&id).root().exists());
        assert!(manager.shared.deletion_journal().is_file());

        let restarted = AccountContextManager::load(manager.shared.clone()).unwrap();
        assert!(restarted.registry_snapshot().unwrap().accounts.is_empty());
        assert!(!manager.shared.deletion_journal().exists());
    }
}
