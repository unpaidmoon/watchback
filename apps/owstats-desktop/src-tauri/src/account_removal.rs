use crate::account_store::{
    read_manifest, read_registry, validate_registry, write_registry_atomic, AccountId,
    AccountRegistry,
};
use crate::app_paths::SharedAppPaths;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const JOURNAL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemovalFaultPoint {
    AfterPrepared,
    AfterQuarantined,
    BeforeRegistryWrite,
    AfterRegistryUpdated,
    BeforeDelete,
}

trait RemovalFault {
    fn after(&self, _point: RemovalFaultPoint) -> Result<()> {
        Ok(())
    }
}

struct NoRemovalFault;

impl RemovalFault for NoRemovalFault {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DeletionPhase {
    Prepared,
    Quarantined,
    RegistryUpdated,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeletionJournal {
    version: u32,
    operation_id: String,
    account_id: AccountId,
    phase: DeletionPhase,
    fallback_account_id: Option<AccountId>,
    started_at: String,
    old_registry: AccountRegistry,
    new_registry: AccountRegistry,
}

pub(crate) fn remove_account(
    shared: &SharedAppPaths,
    old_registry: &AccountRegistry,
    account_id: &AccountId,
    now: &str,
) -> Result<AccountRegistry> {
    remove_account_with_fault(shared, old_registry, account_id, now, &NoRemovalFault)
}

fn remove_account_with_fault(
    shared: &SharedAppPaths,
    old_registry: &AccountRegistry,
    account_id: &AccountId,
    now: &str,
    fault: &impl RemovalFault,
) -> Result<AccountRegistry> {
    if !old_registry
        .accounts
        .iter()
        .any(|entry| &entry.account_id == account_id)
    {
        bail!("账号不存在。")
    }
    ensure_safe_shared_roots(shared)?;
    let original = shared.account(account_id).root().to_path_buf();
    ensure_safe_tree(&original).map_err(|_| anyhow!("账号数据不完整，无法退出。"))?;

    let mut new_registry = old_registry.clone();
    new_registry
        .accounts
        .retain(|entry| &entry.account_id != account_id);
    if new_registry.active_account_id.as_ref() == Some(account_id) {
        new_registry.active_account_id = select_fallback(shared, &new_registry);
    }

    let operation_id = Uuid::new_v4().to_string();
    let operation_root = operation_root(shared, &operation_id)?;
    let quarantine = quarantine_path(shared, &operation_id, account_id)?;
    fs::create_dir_all(&operation_root).context("无法创建账号退出事务目录")?;
    ensure_safe_directory(&operation_root)?;
    let mut journal = DeletionJournal {
        version: JOURNAL_VERSION,
        operation_id,
        account_id: account_id.clone(),
        phase: DeletionPhase::Prepared,
        fallback_account_id: new_registry.active_account_id.clone(),
        started_at: now.to_string(),
        old_registry: old_registry.clone(),
        new_registry: new_registry.clone(),
    };
    write_journal(shared, &journal)?;
    if let Err(error) = fault.after(RemovalFaultPoint::AfterPrepared) {
        cleanup_operation(shared, &journal);
        return Err(error);
    }

    if let Err(error) = fs::rename(&original, &quarantine) {
        cleanup_operation(shared, &journal);
        return Err(error).context("账号数据暂时被占用，无法退出。");
    }
    journal.phase = DeletionPhase::Quarantined;
    if let Err(error) = write_journal(shared, &journal) {
        let _ = fs::rename(&quarantine, &original);
        cleanup_operation(shared, &journal);
        return Err(error);
    }
    if let Err(error) = fault.after(RemovalFaultPoint::AfterQuarantined) {
        rollback_removal(shared, &journal, &original, &quarantine, false)?;
        return Err(error);
    }

    if let Err(error) = fault.after(RemovalFaultPoint::BeforeRegistryWrite) {
        rollback_removal(shared, &journal, &original, &quarantine, false)?;
        return Err(error);
    }
    if let Err(error) = write_registry_atomic(&shared.registry(), &new_registry) {
        if fs::rename(&quarantine, &original).is_ok() {
            cleanup_operation(shared, &journal);
        }
        return Err(error).context("无法保存账号索引，账号未退出。");
    }
    journal.phase = DeletionPhase::RegistryUpdated;
    if write_journal(shared, &journal).is_err() {
        // The older Quarantined journal plus the new on-disk registry is enough
        // to finish recovery. The account is already logically removed.
        let _ = recover_pending_removal(shared);
        return Ok(new_registry);
    }
    if let Err(error) = fault.after(RemovalFaultPoint::AfterRegistryUpdated) {
        rollback_removal(shared, &journal, &original, &quarantine, true)?;
        return Err(error);
    }
    if let Err(error) = fault.after(RemovalFaultPoint::BeforeDelete) {
        rollback_removal(shared, &journal, &original, &quarantine, true)?;
        return Err(error);
    }

    ensure_safe_tree(&quarantine).context("隔离账号目录安全校验失败")?;
    if fs::remove_dir_all(&quarantine).is_err() {
        // Keep the committed registry and quarantine. Recovery retries deletion;
        // the registry never points at a missing account directory.
        return Ok(new_registry);
    }
    journal.phase = DeletionPhase::Complete;
    let _ = write_journal(shared, &journal);
    cleanup_operation(shared, &journal);
    Ok(new_registry)
}

fn rollback_removal(
    shared: &SharedAppPaths,
    journal: &DeletionJournal,
    original: &Path,
    quarantine: &Path,
    registry_updated: bool,
) -> Result<()> {
    if registry_updated {
        write_registry_atomic(&shared.registry(), &journal.old_registry)
            .context("无法回滚账号索引")?;
    }
    if let Err(rename_error) = fs::rename(quarantine, original) {
        if registry_updated {
            let _ = write_registry_atomic(&shared.registry(), &journal.new_registry);
        }
        return Err(rename_error).context("无法回滚账号目录");
    }
    cleanup_operation(shared, journal);
    Ok(())
}

pub(crate) fn recover_pending_removal(shared: &SharedAppPaths) -> Result<()> {
    let journal_path = shared.deletion_journal();
    let metadata = match fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
        bail!("unsafe deletion journal")
    }
    let journal: DeletionJournal =
        serde_json::from_slice(&fs::read(&journal_path)?).context("invalid deletion journal")?;
    validate_journal(&journal)?;
    ensure_safe_shared_roots(shared)?;
    let original = shared.account(&journal.account_id).root().to_path_buf();
    let quarantine = quarantine_path(shared, &journal.operation_id, &journal.account_id)?;
    let disk_registry =
        read_registry(&shared.registry()).context("cannot recover account registry")?;
    let original_exists = entry_exists(&original)?;
    let quarantine_exists = entry_exists(&quarantine)?;

    if disk_registry == journal.old_registry {
        match (original_exists, quarantine_exists) {
            (true, false) => cleanup_operation(shared, &journal),
            (false, true) => {
                ensure_safe_tree(&quarantine)?;
                fs::rename(&quarantine, &original)?;
                cleanup_operation(shared, &journal);
            }
            _ => bail!("账号退出恢复状态不一致"),
        }
        return Ok(());
    }
    if disk_registry == journal.new_registry {
        match (original_exists, quarantine_exists) {
            (false, true) => {
                ensure_safe_tree(&quarantine)?;
                fs::remove_dir_all(&quarantine)?;
                cleanup_operation(shared, &journal);
            }
            (false, false) => cleanup_operation(shared, &journal),
            _ => bail!("账号退出恢复状态不一致"),
        }
        return Ok(());
    }
    bail!("账号退出恢复索引不一致")
}

fn select_fallback(shared: &SharedAppPaths, registry: &AccountRegistry) -> Option<AccountId> {
    registry
        .accounts
        .iter()
        .filter(|entry| stored_account_is_complete(shared, entry))
        .max_by(|left, right| {
            left.last_used_at
                .cmp(&right.last_used_at)
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.account_id.cmp(&right.account_id))
        })
        .map(|entry| entry.account_id.clone())
}

pub(crate) fn validate_switch_target(
    shared: &SharedAppPaths,
    registry: &AccountRegistry,
    account_id: &AccountId,
) -> Result<()> {
    let entry = registry
        .accounts
        .iter()
        .find(|entry| &entry.account_id == account_id)
        .ok_or_else(|| anyhow!("账号不存在。"))?;
    ensure_safe_shared_roots(shared).map_err(|_| anyhow!("账号数据不完整，无法切换。"))?;
    let paths = shared.account(account_id);
    ensure_safe_directory(paths.root()).map_err(|_| anyhow!("账号数据不完整，无法切换。"))?;
    let manifest = read_manifest(&paths.manifest(), account_id)
        .map_err(|_| anyhow!("账号数据不完整，无法切换。"))?;
    if manifest.battle_tag != entry.battle_tag {
        bail!("账号数据不完整，无法切换。")
    }
    Ok(())
}

fn stored_account_is_complete(
    shared: &SharedAppPaths,
    entry: &crate::account_store::AccountRegistryEntry,
) -> bool {
    let paths = shared.account(&entry.account_id);
    ensure_safe_directory(paths.root()).is_ok()
        && read_manifest(&paths.manifest(), &entry.account_id)
            .is_ok_and(|manifest| manifest.battle_tag == entry.battle_tag)
}

fn validate_journal(journal: &DeletionJournal) -> Result<()> {
    if journal.version != JOURNAL_VERSION {
        bail!("unsupported deletion journal version")
    }
    let parsed = Uuid::parse_str(&journal.operation_id).context("invalid deletion operation id")?;
    if parsed.to_string() != journal.operation_id {
        bail!("non-canonical deletion operation id")
    }
    if journal.fallback_account_id != journal.new_registry.active_account_id {
        bail!("invalid deletion fallback")
    }
    validate_registry(&journal.old_registry)?;
    validate_registry(&journal.new_registry)?;
    if !journal
        .old_registry
        .accounts
        .iter()
        .any(|entry| entry.account_id == journal.account_id)
        || journal
            .new_registry
            .accounts
            .iter()
            .any(|entry| entry.account_id == journal.account_id)
    {
        bail!("invalid deletion registry transition")
    }
    Ok(())
}

fn ensure_safe_shared_roots(shared: &SharedAppPaths) -> Result<()> {
    ensure_safe_directory(shared.root())?;
    if entry_exists(&shared.accounts_root())? {
        ensure_safe_directory(&shared.accounts_root())?;
    }
    if entry_exists(&shared.operations_root())? {
        ensure_safe_directory(&shared.operations_root())?;
    } else {
        fs::create_dir_all(shared.operations_root())?;
        ensure_safe_directory(&shared.operations_root())?;
    }
    Ok(())
}

fn operation_root(shared: &SharedAppPaths, operation_id: &str) -> Result<PathBuf> {
    let parsed = Uuid::parse_str(operation_id).context("invalid deletion operation id")?;
    if parsed.to_string() != operation_id {
        bail!("invalid deletion operation id")
    }
    let deletions = shared.operations_root().join("deletions");
    if entry_exists(&deletions)? {
        ensure_safe_directory(&deletions)?;
    } else {
        fs::create_dir_all(&deletions)?;
        ensure_safe_directory(&deletions)?;
    }
    Ok(deletions.join(operation_id))
}

fn quarantine_path(
    shared: &SharedAppPaths,
    operation_id: &str,
    account_id: &AccountId,
) -> Result<PathBuf> {
    Ok(operation_root(shared, operation_id)?.join(account_id.as_str()))
}

fn ensure_safe_tree(path: &Path) -> Result<()> {
    ensure_safe_directory(path)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
            bail!("unsafe linked account data")
        }
        if metadata.is_dir() {
            ensure_safe_tree(&entry.path())?;
        } else if !metadata.is_file() {
            bail!("unsafe account data entry")
        }
    }
    Ok(())
}

fn ensure_safe_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
        bail!("unsafe directory")
    }
    Ok(())
}

fn entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn write_journal(shared: &SharedAppPaths, journal: &DeletionJournal) -> Result<()> {
    validate_journal(journal)?;
    let path = shared.deletion_journal();
    let parent = path.parent().context("deletion journal has no parent")?;
    fs::create_dir_all(parent)?;
    ensure_safe_directory(parent)?;
    let temporary = parent.join(format!(".deletion-journal.{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(journal)?;
    let result = (|| -> Result<()> {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn cleanup_operation(shared: &SharedAppPaths, journal: &DeletionJournal) {
    let _ = fs::remove_file(shared.deletion_journal());
    if let Ok(root) = operation_root(shared, &journal.operation_id) {
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(windows)]
fn is_windows_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_store::{write_manifest_atomic, AccountManifest, AccountRegistryEntry};

    struct FailAt(RemovalFaultPoint);

    impl RemovalFault for FailAt {
        fn after(&self, point: RemovalFaultPoint) -> Result<()> {
            if point == self.0 {
                bail!("synthetic removal fault")
            }
            Ok(())
        }
    }

    fn fixture() -> (
        tempfile::TempDir,
        SharedAppPaths,
        AccountId,
        AccountRegistry,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
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
        let registry = AccountRegistry {
            version: 1,
            active_account_id: Some(id.clone()),
            accounts: vec![AccountRegistryEntry {
                account_id: id.clone(),
                battle_tag: "Synthetic#1000".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                last_used_at: "2026-01-02T00:00:00Z".into(),
                credential_state: "ready".into(),
            }],
        };
        write_registry_atomic(&shared.registry(), &registry).unwrap();
        (temp, shared, id, registry)
    }

    #[test]
    fn injected_failure_after_quarantine_restores_directory_and_registry() {
        let (_temp, shared, id, registry) = fixture();
        assert!(remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(RemovalFaultPoint::AfterQuarantined),
        )
        .is_err());
        assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
        assert_eq!(
            fs::read(shared.account(&id).database()).unwrap(),
            b"synthetic-db"
        );
        assert!(!shared.deletion_journal().exists());
    }

    #[test]
    fn injected_failure_after_registry_commit_rolls_back_both_sides() {
        let (_temp, shared, id, registry) = fixture();
        assert!(remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(RemovalFaultPoint::AfterRegistryUpdated),
        )
        .is_err());
        assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
        assert_eq!(
            fs::read(shared.account(&id).database()).unwrap(),
            b"synthetic-db"
        );
        assert!(!shared.deletion_journal().exists());
    }

    #[test]
    fn injected_rename_registry_and_delete_failures_leave_original_state() {
        for point in [
            RemovalFaultPoint::AfterPrepared,
            RemovalFaultPoint::BeforeRegistryWrite,
            RemovalFaultPoint::BeforeDelete,
        ] {
            let (_temp, shared, id, registry) = fixture();
            assert!(remove_account_with_fault(
                &shared,
                &registry,
                &id,
                "2026-07-16T13:00:00Z",
                &FailAt(point),
            )
            .is_err());
            assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
            assert_eq!(
                fs::read(shared.account(&id).database()).unwrap(),
                b"synthetic-db"
            );
            assert!(!shared.deletion_journal().exists());
        }
    }
}
