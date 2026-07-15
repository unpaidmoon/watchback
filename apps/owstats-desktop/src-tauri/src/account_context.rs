use crate::account_store::{read_registry, write_registry_atomic, AccountId, AccountRegistry};
use crate::app_paths::{AccountPaths, SharedAppPaths};
use crate::models::AccountSummary;
use anyhow::{anyhow, Context, Result};
use std::sync::{Arc, RwLock};

const NO_ACTIVE_ACCOUNT_MESSAGE: &str = "尚未添加可用账号，请先在指挥台添加账号。";

#[derive(Clone)]
pub struct AccountContextManager {
    shared: SharedAppPaths,
    registry: Arc<RwLock<AccountRegistry>>,
    recovery_failed: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct AccountContextSnapshot {
    account_id: AccountId,
    paths: AccountPaths,
}

impl AccountContextManager {
    pub fn load(shared: SharedAppPaths) -> Result<Self> {
        let recovery_failed =
            crate::account_migration::recover_interrupted_migration(&shared).is_err();
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
        })
    }

    pub fn active_snapshot(&self) -> Result<AccountContextSnapshot> {
        let registry = self.registry_snapshot()?;
        let account_id = registry
            .active_account_id
            .ok_or_else(|| anyhow!(NO_ACTIVE_ACCOUNT_MESSAGE))?;
        let paths = self.shared.account(&account_id);
        Ok(AccountContextSnapshot { account_id, paths })
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>> {
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

    pub fn registry_snapshot(&self) -> Result<AccountRegistry> {
        self.registry
            .read()
            .map(|registry| registry.clone())
            .map_err(|_| anyhow!("账号状态暂不可用，请重试。"))
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

    pub fn shared_paths(&self) -> &SharedAppPaths {
        &self.shared
    }

    pub fn migration_state(&self) -> Result<crate::account_migration::LegacyMigrationState> {
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
        crate::account_migration::adopt_legacy_account(&self.shared, account)
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
