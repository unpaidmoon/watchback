use crate::account_store::{
    read_manifest, read_registry, write_registry_atomic, AccountId, AccountRegistry,
};
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
    _storage_lease: Arc<ActiveStorageLease>,
}

#[derive(Debug)]
struct ActiveStorageLease {
    #[cfg(windows)]
    _handles: Vec<std::fs::File>,
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

        let manager = Self {
            shared,
            registry: Arc::new(RwLock::new(registry)),
            recovery_failed: Arc::new(RwLock::new(recovery_failed)),
            migration_lock: Arc::new(Mutex::new(())),
            login_lock: Arc::new(Mutex::new(())),
            deletion_recovery_required: Arc::new(RwLock::new(false)),
        };
        let registry = manager.registry_snapshot()?;
        if let Some(account_id) = registry.active_account_id.as_ref() {
            manager.validate_account_storage(
                &registry,
                account_id,
                "账号数据不完整，无法使用。",
            )?;
        }
        Ok(manager)
    }

    pub fn active_snapshot(&self) -> Result<AccountContextSnapshot> {
        self.ensure_deletion_ready()?;
        let registry = self.registry_snapshot()?;
        let account_id = registry
            .active_account_id
            .clone()
            .ok_or_else(|| anyhow!(NO_ACTIVE_ACCOUNT_MESSAGE))?;
        let paths = self.shared.account(&account_id);
        let storage_lease =
            self.validate_account_storage(&registry, &account_id, "账号数据不完整，无法使用。")?;
        Ok(AccountContextSnapshot {
            account_id,
            paths,
            _storage_lease: storage_lease,
        })
    }

    pub(crate) fn account_paths(&self, account_id: &AccountId) -> AccountPaths {
        self.shared.account(account_id)
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
        let _storage_lease =
            self.validate_account_storage(&registry, account_id, "账号数据不完整，无法切换。")?;
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

    pub(crate) fn snapshot_for(&self, account_id: &AccountId) -> Result<AccountContextSnapshot> {
        let registry = self.registry_snapshot()?;
        let storage_lease =
            self.validate_account_storage(&registry, account_id, "账号数据不完整，无法使用。")?;
        Ok(AccountContextSnapshot {
            account_id: account_id.clone(),
            paths: self.shared.account(account_id),
            _storage_lease: storage_lease,
        })
    }

    fn validate_account_storage(
        &self,
        registry: &AccountRegistry,
        account_id: &AccountId,
        invalid_message: &'static str,
    ) -> Result<Arc<ActiveStorageLease>> {
        let entry = registry
            .accounts
            .iter()
            .find(|entry| &entry.account_id == account_id)
            .ok_or_else(|| anyhow!("账号不存在。"))?;
        let lease = pin_and_validate_account_storage(&self.shared, account_id)
            .map_err(|_| anyhow!(invalid_message))?;
        let manifest = read_manifest(&self.shared.account(account_id).manifest(), account_id)
            .map_err(|_| anyhow!(invalid_message))?;
        if manifest.battle_tag != entry.battle_tag {
            return Err(anyhow!(invalid_message));
        }
        Ok(Arc::new(lease))
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

#[cfg(not(windows))]
fn pin_and_validate_account_storage(
    shared: &SharedAppPaths,
    account_id: &AccountId,
) -> Result<ActiveStorageLease> {
    fn ordinary_directory(path: &std::path::Path) -> Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(anyhow!("unsafe account directory"));
        }
        Ok(())
    }

    ordinary_directory(shared.root())?;
    ordinary_directory(&shared.accounts_root())?;
    let paths = shared.account(account_id);
    ordinary_directory(paths.root())?;
    let canonical_shared = std::fs::canonicalize(shared.root())?;
    let canonical_accounts = std::fs::canonicalize(shared.accounts_root())?;
    let canonical_account = std::fs::canonicalize(paths.root())?;
    if canonical_accounts.parent() != Some(canonical_shared.as_path())
        || canonical_account.parent() != Some(canonical_accounts.as_path())
    {
        return Err(anyhow!("account directory escaped storage root"));
    }
    let manifest_metadata = std::fs::symlink_metadata(paths.manifest())?;
    if !manifest_metadata.is_file() || manifest_metadata.file_type().is_symlink() {
        return Err(anyhow!("unsafe account manifest"));
    }
    let canonical_manifest = std::fs::canonicalize(paths.manifest())?;
    if canonical_manifest.parent() != Some(canonical_account.as_path()) {
        return Err(anyhow!("account manifest escaped account directory"));
    }
    Ok(ActiveStorageLease {})
}

#[cfg(windows)]
fn pin_and_validate_account_storage(
    shared: &SharedAppPaths,
    account_id: &AccountId,
) -> Result<ActiveStorageLease> {
    const FILE_LIST_DIRECTORY: u32 = 0x1;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    fn pin(path: &std::path::Path, expect_directory: bool, access: u32) -> Result<std::fs::File> {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

        let before = std::fs::symlink_metadata(path)?;
        if before.file_type().is_symlink()
            || before.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || before.is_dir() != expect_directory
            || before.is_file() == expect_directory
        {
            return Err(anyhow!("unsafe active account entry"));
        }
        let mut options = std::fs::OpenOptions::new();
        options
            .access_mode(access)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        let handle = options.open(path)?;
        let opened = handle.metadata()?;
        if opened.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || opened.is_dir() != expect_directory
            || opened.is_file() == expect_directory
        {
            return Err(anyhow!("unsafe active account entry"));
        }
        Ok(handle)
    }

    let paths = shared.account(account_id);
    let handles = vec![
        pin(
            shared.root(),
            true,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
        )?,
        pin(
            &shared.accounts_root(),
            true,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
        )?,
        pin(
            paths.root(),
            true,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
        )?,
        pin(
            &paths.manifest(),
            false,
            GENERIC_READ | FILE_READ_ATTRIBUTES,
        )?,
    ];
    let canonical_shared = std::fs::canonicalize(shared.root())?;
    let canonical_accounts = std::fs::canonicalize(shared.accounts_root())?;
    let canonical_account = std::fs::canonicalize(paths.root())?;
    let canonical_manifest = std::fs::canonicalize(paths.manifest())?;
    if canonical_accounts.parent() != Some(canonical_shared.as_path())
        || canonical_account.parent() != Some(canonical_accounts.as_path())
        || canonical_manifest.parent() != Some(canonical_account.as_path())
    {
        return Err(anyhow!("active account storage escaped its boundary"));
    }
    Ok(ActiveStorageLease { _handles: handles })
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

    fn write_account_fixture(
        shared: &SharedAppPaths,
        id: &AccountId,
        battle_tag: &str,
    ) -> AccountRegistryEntry {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(
            &paths.manifest(),
            &AccountManifest {
                version: 1,
                account_id: id.clone(),
                role_id: 101101,
                battle_tag: battle_tag.into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
        fs::write(paths.database(), b"synthetic-db").unwrap();
        AccountRegistryEntry {
            account_id: id.clone(),
            battle_tag: battle_tag.into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            last_used_at: "2026-01-02T00:00:00Z".into(),
            credential_state: "ready".into(),
        }
    }

    #[cfg(windows)]
    fn link_directory(link: &std::path::Path, target: &std::path::Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[cfg(unix)]
    fn link_directory(link: &std::path::Path, target: &std::path::Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    fn manager_with_registry(
        shared: &SharedAppPaths,
        active: AccountId,
        accounts: Vec<AccountRegistryEntry>,
    ) -> AccountContextManager {
        write_registry_atomic(
            &shared.registry(),
            &AccountRegistry {
                version: 1,
                active_account_id: Some(active),
                accounts,
            },
        )
        .unwrap();
        AccountContextManager::load(shared.clone()).unwrap()
    }

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
    fn active_snapshot_accepts_a_complete_ordinary_account_directory() {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let entry = write_account_fixture(&shared, &id, "Synthetic#1000");
        let manager = manager_with_registry(&shared, id.clone(), vec![entry]);

        assert_eq!(manager.active_snapshot().unwrap().account_id(), &id);
    }

    #[test]
    fn load_and_active_snapshot_fail_closed_when_active_storage_is_missing_or_mismatched() {
        for case in ["missing", "manifest-mismatch"] {
            let temp = tempfile::tempdir().unwrap();
            let shared = SharedAppPaths::from_app_data_dir(temp.path());
            fs::create_dir_all(shared.root()).unwrap();
            let id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
            let entry = if case == "missing" {
                AccountRegistryEntry {
                    account_id: id.clone(),
                    battle_tag: "Synthetic#1000".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    last_used_at: "2026-01-02T00:00:00Z".into(),
                    credential_state: "ready".into(),
                }
            } else {
                let entry = write_account_fixture(&shared, &id, "Synthetic#1000");
                let other = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
                write_manifest_atomic(
                    &shared.account(&id).manifest(),
                    &AccountManifest {
                        version: 1,
                        account_id: other,
                        role_id: 202202,
                        battle_tag: "Synthetic#1000".into(),
                        created_at: "2026-01-01T00:00:00Z".into(),
                        updated_at: "2026-01-01T00:00:00Z".into(),
                    },
                )
                .unwrap_err();
                fs::write(
                    shared.account(&id).manifest(),
                    br#"{"version":1,"accountId":"22222222-2222-4222-8222-222222222222","roleId":202202,"battleTag":"Synthetic#1000","createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z"}"#,
                )
                .unwrap();
                entry
            };
            write_registry_atomic(
                &shared.registry(),
                &AccountRegistry {
                    version: 1,
                    active_account_id: Some(id.clone()),
                    accounts: vec![entry],
                },
            )
            .unwrap();

            assert!(AccountContextManager::load(shared).is_err(), "case={case}");
        }

        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let entry = write_account_fixture(&shared, &id, "Synthetic#1000");
        let manager = manager_with_registry(&shared, id.clone(), vec![entry]);
        fs::remove_dir_all(shared.account(&id).root()).unwrap();
        assert!(manager.active_snapshot().is_err());
    }

    #[test]
    fn load_and_active_snapshot_reject_reparse_active_storage_without_mutating_target() {
        for target_kind in ["other-account", "outside-sentinel"] {
            let temp = tempfile::tempdir().unwrap();
            let shared = SharedAppPaths::from_app_data_dir(temp.path());
            let active = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
            let other = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
            let active_entry = write_account_fixture(&shared, &active, "Active#1000");
            let other_entry = write_account_fixture(&shared, &other, "Other#2000");
            let manager = manager_with_registry(
                &shared,
                active.clone(),
                vec![active_entry.clone(), other_entry.clone()],
            );
            let active_root = shared.account(&active).root().to_path_buf();
            fs::remove_dir_all(&active_root).unwrap();
            let target = if target_kind == "other-account" {
                shared.account(&other).root().to_path_buf()
            } else {
                let sentinel = temp.path().join("outside-sentinel");
                fs::create_dir_all(&sentinel).unwrap();
                fs::write(
                    sentinel.join("do-not-read-or-change"),
                    b"synthetic-sentinel",
                )
                .unwrap();
                sentinel
            };
            link_directory(&active_root, &target);
            let before = fs::symlink_metadata(target.join("do-not-read-or-change")).ok();

            assert!(
                manager.active_snapshot().is_err(),
                "target_kind={target_kind}"
            );
            assert!(
                AccountContextManager::load(shared.clone()).is_err(),
                "target_kind={target_kind}"
            );
            let after = fs::symlink_metadata(target.join("do-not-read-or-change")).ok();
            assert_eq!(before.map(|m| m.len()), after.map(|m| m.len()));
            assert!(
                fs::symlink_metadata(&active_root)
                    .unwrap()
                    .file_type()
                    .is_symlink()
                    || cfg!(windows)
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn active_snapshot_pins_validated_storage_against_path_replacement() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AccountContextSnapshot>();

        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let entry = write_account_fixture(&shared, &id, "Synthetic#1000");
        let manager = manager_with_registry(&shared, id.clone(), vec![entry]);
        let snapshot = manager.active_snapshot().unwrap();
        let cloned_snapshot = snapshot.clone();
        let root = shared.account(&id).root().to_path_buf();
        let replacement = shared.accounts_root().join("replacement");

        assert!(fs::rename(&root, &replacement).is_err());
        drop(snapshot);
        assert!(fs::rename(&root, &replacement).is_err());
        drop(cloned_snapshot);
        fs::rename(&root, &replacement).unwrap();
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
