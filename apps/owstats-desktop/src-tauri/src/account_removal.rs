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
    JournalAfterQuarantine,
    JournalAfterRegistryUpdated,
    JournalRollingBack,
    RollbackRename,
    AfterRollbackRename,
    AfterOldRegistryWrite,
    AfterQuarantined,
    BeforeRegistryWrite,
    AfterRegistryUpdated,
    BeforeDelete,
    BeforeAnyDelete,
    AfterFirstDeleteMutation,
    CleanupOperation,
}

trait RemovalFault {
    fn after(&self, _point: RemovalFaultPoint) -> Result<()> {
        Ok(())
    }
}

struct NoRemovalFault;

impl RemovalFault for NoRemovalFault {}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum TestRemovalFault {
    BeforeAnyDelete,
    AfterFirstDeleteMutation,
    JournalAfterRegistryUpdated,
}

#[cfg(test)]
struct SingleTestFault(TestRemovalFault);

#[cfg(test)]
impl RemovalFault for SingleTestFault {
    fn after(&self, point: RemovalFaultPoint) -> Result<()> {
        let selected = match self.0 {
            TestRemovalFault::BeforeAnyDelete => RemovalFaultPoint::BeforeAnyDelete,
            TestRemovalFault::AfterFirstDeleteMutation => {
                RemovalFaultPoint::AfterFirstDeleteMutation
            }
            TestRemovalFault::JournalAfterRegistryUpdated => {
                RemovalFaultPoint::JournalAfterRegistryUpdated
            }
        };
        if point == selected {
            bail!("synthetic removal fault")
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DeletionPhase {
    Prepared,
    Quarantined,
    RegistryUpdated,
    RollingBack,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemovalCompletion {
    Removed,
    RemovedPendingCleanup,
    NotRemoved,
}

#[derive(Debug, Clone)]
pub(crate) struct RemovalOutcome {
    pub registry: AccountRegistry,
    pub completion: RemovalCompletion,
}

pub(crate) fn remove_account(
    shared: &SharedAppPaths,
    old_registry: &AccountRegistry,
    account_id: &AccountId,
    now: &str,
) -> Result<RemovalOutcome> {
    remove_account_with_fault(shared, old_registry, account_id, now, &NoRemovalFault)
}

#[cfg(test)]
pub(crate) fn remove_account_with_test_fault(
    shared: &SharedAppPaths,
    old_registry: &AccountRegistry,
    account_id: &AccountId,
    now: &str,
    fault: TestRemovalFault,
) -> Result<RemovalOutcome> {
    remove_account_with_fault(
        shared,
        old_registry,
        account_id,
        now,
        &SingleTestFault(fault),
    )
}

fn remove_account_with_fault(
    shared: &SharedAppPaths,
    old_registry: &AccountRegistry,
    account_id: &AccountId,
    now: &str,
    fault: &impl RemovalFault,
) -> Result<RemovalOutcome> {
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
        if !new_registry.accounts.is_empty() && new_registry.active_account_id.is_none() {
            bail!("没有完整的备用账号，无法退出当前账号。")
        }
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
        let _ = cleanup_operation(shared, &journal, fault);
        return Err(error);
    }

    if let Err(error) = fs::rename(&original, &quarantine) {
        let _ = cleanup_operation(shared, &journal, fault);
        return Err(error).context("账号数据暂时被占用，无法退出。");
    }
    journal.phase = DeletionPhase::Quarantined;
    let quarantine_journal_result = fault
        .after(RemovalFaultPoint::JournalAfterQuarantine)
        .and_then(|_| write_journal(shared, &journal));
    if quarantine_journal_result.is_err() {
        if fault.after(RemovalFaultPoint::RollbackRename).is_err()
            || fs::rename(&quarantine, &original).is_err()
        {
            return Err(anyhow!("账号退出恢复失败，请重试。"));
        }
        let _ = cleanup_operation(shared, &journal, fault);
        return Err(anyhow!("账号退出失败，账号未更改。"));
    }
    if let Err(error) = fault.after(RemovalFaultPoint::AfterQuarantined) {
        rollback_before_registry(shared, &journal, &original, &quarantine, fault)?;
        return Err(error);
    }

    if let Err(error) = fault.after(RemovalFaultPoint::BeforeRegistryWrite) {
        rollback_before_registry(shared, &journal, &original, &quarantine, fault)?;
        return Err(error);
    }
    if let Err(error) = write_registry_atomic(&shared.registry(), &new_registry) {
        rollback_before_registry(shared, &journal, &original, &quarantine, fault)?;
        return Err(error).context("无法保存账号索引，账号未退出。");
    }
    journal.phase = DeletionPhase::RegistryUpdated;
    if fault
        .after(RemovalFaultPoint::JournalAfterRegistryUpdated)
        .and_then(|_| write_journal(shared, &journal))
        .is_err()
    {
        return Ok(RemovalOutcome {
            registry: new_registry,
            completion: RemovalCompletion::RemovedPendingCleanup,
        });
    }
    if let Err(error) = fault.after(RemovalFaultPoint::AfterRegistryUpdated) {
        return match rollback_after_registry(shared, &mut journal, &original, &quarantine, fault)? {
            RollbackDisposition::RolledBack => Err(error),
            RollbackDisposition::PendingRemoval => Ok(RemovalOutcome {
                registry: new_registry,
                completion: RemovalCompletion::RemovedPendingCleanup,
            }),
        };
    }
    if let Err(error) = fault.after(RemovalFaultPoint::BeforeDelete) {
        return match rollback_after_registry(shared, &mut journal, &original, &quarantine, fault)? {
            RollbackDisposition::RolledBack => Err(error),
            RollbackDisposition::PendingRemoval => Ok(RemovalOutcome {
                registry: new_registry,
                completion: RemovalCompletion::RemovedPendingCleanup,
            }),
        };
    }

    if ensure_safe_tree(&quarantine).is_err() {
        return match rollback_after_registry(shared, &mut journal, &original, &quarantine, fault)? {
            RollbackDisposition::RolledBack => Ok(RemovalOutcome {
                registry: old_registry.clone(),
                completion: RemovalCompletion::NotRemoved,
            }),
            RollbackDisposition::PendingRemoval => Ok(RemovalOutcome {
                registry: new_registry,
                completion: RemovalCompletion::RemovedPendingCleanup,
            }),
        };
    }
    if let Err(delete_failure) = delete_tree_tracked(&quarantine, fault) {
        if delete_failure.mutations == 0 {
            return match rollback_after_registry(
                shared,
                &mut journal,
                &original,
                &quarantine,
                fault,
            )? {
                RollbackDisposition::RolledBack => Ok(RemovalOutcome {
                    registry: old_registry.clone(),
                    completion: RemovalCompletion::NotRemoved,
                }),
                RollbackDisposition::PendingRemoval => Ok(RemovalOutcome {
                    registry: new_registry,
                    completion: RemovalCompletion::RemovedPendingCleanup,
                }),
            };
        }
        return Ok(RemovalOutcome {
            registry: new_registry,
            completion: RemovalCompletion::RemovedPendingCleanup,
        });
    }
    journal.phase = DeletionPhase::Complete;
    if write_journal(shared, &journal).is_err()
        || cleanup_operation(shared, &journal, fault).is_err()
    {
        return Ok(RemovalOutcome {
            registry: new_registry,
            completion: RemovalCompletion::RemovedPendingCleanup,
        });
    }
    Ok(RemovalOutcome {
        registry: new_registry,
        completion: RemovalCompletion::Removed,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RollbackDisposition {
    RolledBack,
    PendingRemoval,
}

fn rollback_before_registry(
    shared: &SharedAppPaths,
    journal: &DeletionJournal,
    original: &Path,
    quarantine: &Path,
    fault: &impl RemovalFault,
) -> Result<()> {
    if fault.after(RemovalFaultPoint::RollbackRename).is_err()
        || fs::rename(quarantine, original).is_err()
    {
        bail!("账号退出恢复失败，请重试。")
    }
    cleanup_operation(shared, journal, fault)?;
    Ok(())
}

fn rollback_after_registry(
    shared: &SharedAppPaths,
    journal: &mut DeletionJournal,
    original: &Path,
    quarantine: &Path,
    fault: &impl RemovalFault,
) -> Result<RollbackDisposition> {
    journal.phase = DeletionPhase::RollingBack;
    if fault
        .after(RemovalFaultPoint::JournalRollingBack)
        .and_then(|_| write_journal(shared, journal))
        .is_err()
    {
        return Ok(RollbackDisposition::PendingRemoval);
    }
    if fault.after(RemovalFaultPoint::RollbackRename).is_err()
        || fs::rename(quarantine, original).is_err()
    {
        bail!("账号退出恢复失败，请重试。")
    }
    if fault.after(RemovalFaultPoint::AfterRollbackRename).is_err() {
        bail!("账号退出恢复失败，请重试。")
    }
    write_registry_atomic(&shared.registry(), &journal.old_registry)
        .map_err(|_| anyhow!("账号退出恢复失败，请重试。"))?;
    if fault
        .after(RemovalFaultPoint::AfterOldRegistryWrite)
        .is_err()
    {
        bail!("账号退出恢复失败，请重试。")
    }
    cleanup_operation(shared, journal, fault)?;
    Ok(RollbackDisposition::RolledBack)
}

#[derive(Debug)]
struct DeleteFailure {
    mutations: usize,
}

fn delete_tree_tracked(
    path: &Path,
    fault: &impl RemovalFault,
) -> std::result::Result<(), DeleteFailure> {
    if fault.after(RemovalFaultPoint::BeforeAnyDelete).is_err() {
        return Err(DeleteFailure { mutations: 0 });
    }
    let mut mutations = 0usize;
    if delete_directory_contents(path, fault, &mut mutations).is_err() {
        return Err(DeleteFailure { mutations });
    }
    if fs::remove_dir(path).is_err() {
        return Err(DeleteFailure { mutations });
    }
    mutations += 1;
    if mutations == 1
        && fault
            .after(RemovalFaultPoint::AfterFirstDeleteMutation)
            .is_err()
    {
        return Err(DeleteFailure { mutations });
    }
    Ok(())
}

fn delete_directory_contents(
    path: &Path,
    fault: &impl RemovalFault,
    mutations: &mut usize,
) -> Result<()> {
    let mut entries = fs::read_dir(path)?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for child in entries {
        let metadata = fs::symlink_metadata(&child)?;
        if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
            bail!("unsafe linked account data")
        }
        if metadata.is_dir() {
            delete_directory_contents(&child, fault, mutations)?;
            fs::remove_dir(&child)?;
        } else if metadata.is_file() {
            fs::remove_file(&child)?;
        } else {
            bail!("unsafe account data entry")
        }
        *mutations += 1;
        if *mutations == 1
            && fault
                .after(RemovalFaultPoint::AfterFirstDeleteMutation)
                .is_err()
        {
            bail!("synthetic partial deletion")
        }
    }
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
    ensure_safe_shared_roots(shared)?;
    validate_journal(shared, &journal)?;
    let original = shared.account(&journal.account_id).root().to_path_buf();
    let quarantine = quarantine_path(shared, &journal.operation_id, &journal.account_id)?;
    let disk_registry =
        read_registry(&shared.registry()).context("cannot recover account registry")?;
    let original_exists = entry_exists(&original)?;
    let quarantine_exists = entry_exists(&quarantine)?;

    match (
        disk_registry == journal.old_registry,
        disk_registry == journal.new_registry,
        original_exists,
        quarantine_exists,
        journal.phase,
    ) {
        (
            true,
            false,
            true,
            false,
            DeletionPhase::Prepared | DeletionPhase::Quarantined | DeletionPhase::RollingBack,
        ) => cleanup_operation(shared, &journal, &NoRemovalFault),
        (true, false, false, true, DeletionPhase::Prepared | DeletionPhase::Quarantined) => {
            ensure_safe_tree(&quarantine)?;
            fs::rename(&quarantine, &original)?;
            cleanup_operation(shared, &journal, &NoRemovalFault)
        }
        (false, true, false, true, DeletionPhase::RollingBack) => {
            ensure_safe_tree(&quarantine)?;
            fs::rename(&quarantine, &original)?;
            write_registry_atomic(&shared.registry(), &journal.old_registry)?;
            cleanup_operation(shared, &journal, &NoRemovalFault)
        }
        (false, true, true, false, DeletionPhase::RollingBack) => {
            write_registry_atomic(&shared.registry(), &journal.old_registry)?;
            cleanup_operation(shared, &journal, &NoRemovalFault)
        }
        (false, true, false, true, DeletionPhase::Quarantined | DeletionPhase::RegistryUpdated) => {
            ensure_safe_tree(&quarantine)?;
            delete_tree_tracked(&quarantine, &NoRemovalFault)
                .map_err(|_| anyhow!("账号退出恢复失败，请重试。"))?;
            let mut complete = journal.clone();
            complete.phase = DeletionPhase::Complete;
            write_journal(shared, &complete)?;
            cleanup_operation(shared, &complete, &NoRemovalFault)
        }
        (
            false,
            true,
            false,
            false,
            DeletionPhase::Quarantined | DeletionPhase::RegistryUpdated | DeletionPhase::Complete,
        ) => {
            let mut complete = journal.clone();
            complete.phase = DeletionPhase::Complete;
            write_journal(shared, &complete)?;
            cleanup_operation(shared, &complete, &NoRemovalFault)
        }
        _ => bail!("账号退出恢复状态不一致"),
    }
}

fn select_fallback(shared: &SharedAppPaths, registry: &AccountRegistry) -> Option<AccountId> {
    use chrono::{DateTime, Utc};
    fn parsed(value: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|value| value.with_timezone(&Utc))
    }
    registry
        .accounts
        .iter()
        .filter(|entry| stored_account_is_complete(shared, entry))
        .max_by(|left, right| {
            parsed(&left.last_used_at)
                .cmp(&parsed(&right.last_used_at))
                .then_with(|| parsed(&left.created_at).cmp(&parsed(&right.created_at)))
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

fn validate_journal(shared: &SharedAppPaths, journal: &DeletionJournal) -> Result<()> {
    if journal.version != JOURNAL_VERSION {
        bail!("unsupported deletion journal version")
    }
    let parsed = Uuid::parse_str(&journal.operation_id).context("invalid deletion operation id")?;
    if parsed.to_string() != journal.operation_id {
        bail!("non-canonical deletion operation id")
    }
    validate_registry(&journal.old_registry)?;
    validate_registry(&journal.new_registry)?;
    let target = journal
        .old_registry
        .accounts
        .iter()
        .find(|entry| entry.account_id == journal.account_id)
        .ok_or_else(|| anyhow!("invalid deletion target"))?;
    let expected_accounts = journal
        .old_registry
        .accounts
        .iter()
        .filter(|entry| entry.account_id != journal.account_id)
        .cloned()
        .collect::<Vec<_>>();
    if journal.new_registry.accounts != expected_accounts {
        bail!("invalid deletion registry transition")
    }
    let expected_active =
        if journal.old_registry.active_account_id.as_ref() == Some(&target.account_id) {
            if expected_accounts.is_empty() {
                None
            } else {
                let candidate_registry = AccountRegistry {
                    version: journal.old_registry.version,
                    active_account_id: None,
                    accounts: expected_accounts,
                };
                select_fallback(shared, &candidate_registry)
                    .ok_or_else(|| anyhow!("invalid deletion fallback"))?
                    .into()
            }
        } else {
            journal.old_registry.active_account_id.clone()
        };
    if journal.new_registry.active_account_id != expected_active
        || journal.fallback_account_id != expected_active
    {
        bail!("invalid deletion fallback")
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
    validate_journal(shared, journal)?;
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

fn cleanup_operation(
    shared: &SharedAppPaths,
    journal: &DeletionJournal,
    fault: &impl RemovalFault,
) -> Result<()> {
    fault.after(RemovalFaultPoint::CleanupOperation)?;
    let root = operation_root(shared, &journal.operation_id)?;
    match fs::remove_dir(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    match fs::remove_file(shared.deletion_journal()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
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

    struct FailAt(Vec<RemovalFaultPoint>);

    impl RemovalFault for FailAt {
        fn after(&self, point: RemovalFaultPoint) -> Result<()> {
            if self.0.contains(&point) {
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
            &FailAt(vec![RemovalFaultPoint::AfterQuarantined]),
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
            &FailAt(vec![RemovalFaultPoint::AfterRegistryUpdated]),
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
                &FailAt(vec![point]),
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

    #[test]
    fn journal_failure_after_quarantine_and_rollback_rename_failure_preserves_recovery_state() {
        let (_temp, shared, id, registry) = fixture();
        let error = remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(vec![
                RemovalFaultPoint::JournalAfterQuarantine,
                RemovalFaultPoint::RollbackRename,
            ]),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "账号退出恢复失败，请重试。");
        assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
        assert!(shared.deletion_journal().is_file());
        assert!(!shared.account(&id).root().exists());
        let journal: DeletionJournal =
            serde_json::from_slice(&fs::read(shared.deletion_journal()).unwrap()).unwrap();
        assert_eq!(journal.phase, DeletionPhase::Prepared);
        let quarantine = quarantine_path(&shared, &journal.operation_id, &id).unwrap();
        assert_eq!(
            fs::read(quarantine.join("owstats.sqlite3")).unwrap(),
            b"synthetic-db"
        );

        recover_pending_removal(&shared).unwrap();
        assert_eq!(
            fs::read(shared.account(&id).database()).unwrap(),
            b"synthetic-db"
        );
        assert!(!shared.deletion_journal().exists());
    }

    #[test]
    fn tracked_delete_distinguishes_not_started_from_partial() {
        let (_temp, shared, id, registry) = fixture();
        let not_started = remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(vec![RemovalFaultPoint::BeforeAnyDelete]),
        )
        .unwrap();
        assert_eq!(not_started.completion, RemovalCompletion::NotRemoved);
        assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
        assert_eq!(
            fs::read(shared.account(&id).database()).unwrap(),
            b"synthetic-db"
        );
        assert!(!shared.deletion_journal().exists());

        let (_temp, shared, id, registry) = fixture();
        let partial = remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(vec![RemovalFaultPoint::AfterFirstDeleteMutation]),
        )
        .unwrap();
        assert_eq!(partial.completion, RemovalCompletion::RemovedPendingCleanup);
        assert_eq!(read_registry(&shared.registry()).unwrap(), partial.registry);
        assert!(!shared.account(&id).root().exists());
        assert!(shared.deletion_journal().is_file());
        recover_pending_removal(&shared).unwrap();
        assert!(!shared.deletion_journal().exists());
    }

    #[test]
    fn rolling_back_crash_points_recover_in_safe_directory_then_registry_order() {
        for point in [
            RemovalFaultPoint::AfterRollbackRename,
            RemovalFaultPoint::AfterOldRegistryWrite,
        ] {
            let (_temp, shared, id, registry) = fixture();
            assert!(remove_account_with_fault(
                &shared,
                &registry,
                &id,
                "2026-07-16T13:00:00Z",
                &FailAt(vec![RemovalFaultPoint::AfterRegistryUpdated, point]),
            )
            .is_err());
            assert!(shared.account(&id).root().is_dir());
            assert!(shared.deletion_journal().is_file());
            recover_pending_removal(&shared).unwrap();
            assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
            assert_eq!(
                fs::read(shared.account(&id).database()).unwrap(),
                b"synthetic-db"
            );
            assert!(!shared.deletion_journal().exists());
        }
    }

    #[test]
    fn rolling_back_journal_failure_keeps_logical_removal_pending() {
        let (_temp, shared, id, registry) = fixture();
        let outcome = remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(vec![
                RemovalFaultPoint::AfterRegistryUpdated,
                RemovalFaultPoint::JournalRollingBack,
            ]),
        )
        .unwrap();
        assert_eq!(outcome.completion, RemovalCompletion::RemovedPendingCleanup);
        assert_eq!(read_registry(&shared.registry()).unwrap(), outcome.registry);
        assert!(!shared.account(&id).root().exists());
        assert!(shared.deletion_journal().is_file());
        recover_pending_removal(&shared).unwrap();
        assert!(!shared.deletion_journal().exists());
    }

    #[test]
    fn complete_cleanup_failure_keeps_complete_journal_for_retry() {
        let (_temp, shared, id, registry) = fixture();
        let outcome = remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(vec![RemovalFaultPoint::CleanupOperation]),
        )
        .unwrap();
        assert_eq!(outcome.completion, RemovalCompletion::RemovedPendingCleanup);
        let journal: DeletionJournal =
            serde_json::from_slice(&fs::read(shared.deletion_journal()).unwrap()).unwrap();
        assert_eq!(journal.phase, DeletionPhase::Complete);
        recover_pending_removal(&shared).unwrap();
        assert!(!shared.deletion_journal().exists());
    }

    fn strict_journal_fixture() -> (
        tempfile::TempDir,
        SharedAppPaths,
        DeletionJournal,
        Vec<(AccountId, Vec<u8>)>,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let ids = [
            AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap(),
            AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap(),
            AccountId::parse("33333333-3333-4333-8333-333333333333").unwrap(),
        ];
        let mut entries = Vec::new();
        let mut bytes = Vec::new();
        for (index, id) in ids.iter().enumerate() {
            let tag = format!("Synthetic#{index}000");
            let paths = shared.account(id);
            fs::create_dir_all(paths.root()).unwrap();
            write_manifest_atomic(
                &paths.manifest(),
                &AccountManifest {
                    version: 1,
                    account_id: id.clone(),
                    role_id: 101101 + index as i64,
                    battle_tag: tag.clone(),
                    created_at: format!("2026-01-0{}T00:00:00Z", index + 1),
                    updated_at: "2026-01-04T00:00:00Z".into(),
                },
            )
            .unwrap();
            let content = format!("db-{index}").into_bytes();
            fs::write(paths.database(), &content).unwrap();
            bytes.push((id.clone(), content));
            entries.push(AccountRegistryEntry {
                account_id: id.clone(),
                battle_tag: tag,
                created_at: format!("2026-01-0{}T00:00:00Z", index + 1),
                last_used_at: format!("2026-02-0{}T00:00:00Z", index + 1),
                credential_state: "ready".into(),
            });
        }
        let old_registry = AccountRegistry {
            version: 1,
            active_account_id: Some(ids[0].clone()),
            accounts: entries,
        };
        let new_registry = AccountRegistry {
            version: 1,
            active_account_id: Some(ids[2].clone()),
            accounts: old_registry.accounts[1..].to_vec(),
        };
        write_registry_atomic(&shared.registry(), &old_registry).unwrap();
        let journal = DeletionJournal {
            version: 1,
            operation_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
            account_id: ids[0].clone(),
            phase: DeletionPhase::Prepared,
            fallback_account_id: Some(ids[2].clone()),
            started_at: "2026-07-16T13:00:00Z".into(),
            old_registry,
            new_registry,
        };
        write_journal(&shared, &journal).unwrap();
        (temp, shared, journal, bytes)
    }

    #[test]
    fn strict_journal_transition_rejects_tampered_fields_without_touching_accounts() {
        for mutate in 0..5 {
            let (_temp, shared, mut journal, expected) = strict_journal_fixture();
            match mutate {
                0 => journal.new_registry.accounts[0].battle_tag = "Tampered#9999".into(),
                1 => journal.new_registry.accounts.swap(0, 1),
                2 => {
                    journal.new_registry.active_account_id =
                        Some(journal.new_registry.accounts[0].account_id.clone());
                    journal.fallback_account_id = journal.new_registry.active_account_id.clone();
                }
                3 => {
                    journal.fallback_account_id =
                        Some(journal.new_registry.accounts[0].account_id.clone())
                }
                4 => journal.new_registry.accounts[1].last_used_at = "2099-01-01T00:00:00Z".into(),
                _ => unreachable!(),
            }
            fs::write(
                shared.deletion_journal(),
                serde_json::to_vec_pretty(&journal).unwrap(),
            )
            .unwrap();
            assert!(recover_pending_removal(&shared).is_err());
            for (id, bytes) in &expected {
                assert_eq!(fs::read(shared.account(id).database()).unwrap(), *bytes);
            }
            assert!(shared.deletion_journal().is_file());
        }
    }

    #[test]
    fn fallback_uses_real_rfc3339_instants_and_stable_invalid_ties() {
        let (_temp, shared, mut journal, _) = strict_journal_fixture();
        journal.new_registry.accounts[0].last_used_at = "2026-07-16T08:00:00+08:00".into();
        journal.new_registry.accounts[0].created_at = "2026-01-02T00:00:00+08:00".into();
        journal.new_registry.accounts[1].last_used_at = "2026-07-16T00:00:00Z".into();
        journal.new_registry.accounts[1].created_at = "2026-01-01T17:00:00Z".into();
        assert_eq!(
            select_fallback(&shared, &journal.new_registry),
            Some(journal.new_registry.accounts[1].account_id.clone())
        );

        journal.new_registry.accounts[1].last_used_at = "2026-07-16T00:30:00Z".into();
        assert_eq!(
            select_fallback(&shared, &journal.new_registry),
            Some(journal.new_registry.accounts[1].account_id.clone())
        );

        journal.new_registry.accounts[0].last_used_at = "invalid-latest-looking".into();
        journal.new_registry.accounts[1].last_used_at = "2020-01-01T00:00:00Z".into();
        assert_eq!(
            select_fallback(&shared, &journal.new_registry),
            Some(journal.new_registry.accounts[1].account_id.clone())
        );

        journal.new_registry.accounts[0].last_used_at = "invalid-a".into();
        journal.new_registry.accounts[1].last_used_at = "invalid-b".into();
        journal.new_registry.accounts[0].created_at = "invalid-a".into();
        journal.new_registry.accounts[1].created_at = "invalid-b".into();
        let expected = std::cmp::max(
            journal.new_registry.accounts[0].account_id.clone(),
            journal.new_registry.accounts[1].account_id.clone(),
        );
        assert_eq!(
            select_fallback(&shared, &journal.new_registry),
            Some(expected)
        );
    }
}
