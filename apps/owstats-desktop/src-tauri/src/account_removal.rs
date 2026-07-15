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
    CommitNewAfterRollbackFailure,
    AfterRollbackRename,
    AfterOldRegistryWrite,
    AfterQuarantined,
    BeforeRegistryWrite,
    AfterRegistryUpdated,
    BeforeDelete,
    UnsafeTreeAfterRegistry,
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
    RollbackRestoreCommitsRemoval,
    RollbackRestoreAndRemovalCommit,
}

#[cfg(test)]
struct SingleTestFault(TestRemovalFault);

#[cfg(test)]
impl RemovalFault for SingleTestFault {
    fn after(&self, point: RemovalFaultPoint) -> Result<()> {
        let should_fail = match self.0 {
            TestRemovalFault::BeforeAnyDelete => point == RemovalFaultPoint::BeforeAnyDelete,
            TestRemovalFault::AfterFirstDeleteMutation => {
                point == RemovalFaultPoint::AfterFirstDeleteMutation
            }
            TestRemovalFault::JournalAfterRegistryUpdated => {
                point == RemovalFaultPoint::JournalAfterRegistryUpdated
            }
            TestRemovalFault::RollbackRestoreCommitsRemoval => matches!(
                point,
                RemovalFaultPoint::JournalAfterQuarantine | RemovalFaultPoint::RollbackRename
            ),
            TestRemovalFault::RollbackRestoreAndRemovalCommit => matches!(
                point,
                RemovalFaultPoint::JournalAfterQuarantine
                    | RemovalFaultPoint::RollbackRename
                    | RemovalFaultPoint::CommitNewAfterRollbackFailure
            ),
        };
        if should_fail {
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
    RestoreFailed,
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
            return commit_removal_when_restore_failed(shared, &journal, fault);
        }
        let _ = cleanup_operation(shared, &journal, fault);
        return Err(anyhow!("账号退出失败，账号未更改。"));
    }
    if let Err(error) = fault.after(RemovalFaultPoint::AfterQuarantined) {
        return match rollback_before_registry(shared, &journal, &original, &quarantine, fault)? {
            PreRegistryRollback::Restored => Err(error),
            PreRegistryRollback::RemovalCommitted(outcome) => Ok(outcome),
        };
    }

    if let Err(error) = fault.after(RemovalFaultPoint::BeforeRegistryWrite) {
        return match rollback_before_registry(shared, &journal, &original, &quarantine, fault)? {
            PreRegistryRollback::Restored => Err(error),
            PreRegistryRollback::RemovalCommitted(outcome) => Ok(outcome),
        };
    }
    if let Err(error) = write_registry_atomic(&shared.registry(), &new_registry) {
        return match rollback_before_registry(shared, &journal, &original, &quarantine, fault)? {
            PreRegistryRollback::Restored => Err(error).context("无法保存账号索引，账号未退出。"),
            PreRegistryRollback::RemovalCommitted(outcome) => Ok(outcome),
        };
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

    if fault
        .after(RemovalFaultPoint::UnsafeTreeAfterRegistry)
        .is_err()
        || ensure_safe_tree(&quarantine).is_err()
    {
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

enum PreRegistryRollback {
    Restored,
    RemovalCommitted(RemovalOutcome),
}

fn rollback_before_registry(
    shared: &SharedAppPaths,
    journal: &DeletionJournal,
    original: &Path,
    quarantine: &Path,
    fault: &impl RemovalFault,
) -> Result<PreRegistryRollback> {
    if fault.after(RemovalFaultPoint::RollbackRename).is_err()
        || fs::rename(quarantine, original).is_err()
    {
        return commit_removal_when_restore_failed(shared, journal, fault)
            .map(PreRegistryRollback::RemovalCommitted);
    }
    cleanup_operation(shared, journal, fault)?;
    Ok(PreRegistryRollback::Restored)
}

fn commit_removal_when_restore_failed(
    shared: &SharedAppPaths,
    journal: &DeletionJournal,
    fault: &impl RemovalFault,
) -> Result<RemovalOutcome> {
    let mut restore_failed = journal.clone();
    restore_failed.phase = DeletionPhase::RestoreFailed;
    let _ = write_journal(shared, &restore_failed);
    if fault
        .after(RemovalFaultPoint::CommitNewAfterRollbackFailure)
        .is_err()
        || write_registry_atomic(&shared.registry(), &journal.new_registry).is_err()
    {
        bail!("账号删除恢复尚未完成，请重启应用后重试。")
    }
    let mut committed = journal.clone();
    committed.phase = DeletionPhase::RegistryUpdated;
    let _ = write_journal(shared, &committed);
    Ok(RemovalOutcome {
        registry: journal.new_registry.clone(),
        completion: RemovalCompletion::RemovedPendingCleanup,
    })
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
        journal.phase = DeletionPhase::RegistryUpdated;
        let _ = write_journal(shared, journal);
        return Ok(RollbackDisposition::PendingRemoval);
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

    if journal.phase == DeletionPhase::RestoreFailed {
        return recover_restore_failed(shared, &journal, &disk_registry, &original, &quarantine);
    }

    match (
        disk_registry == journal.old_registry,
        disk_registry == journal.new_registry,
        original_exists,
        quarantine_exists,
        journal.phase,
    ) {
        (true, false, false, true, DeletionPhase::RestoreFailed) => {
            write_registry_atomic(&shared.registry(), &journal.new_registry)?;
            ensure_safe_tree(&quarantine)?;
            delete_tree_tracked(&quarantine, &NoRemovalFault)
                .map_err(|_| anyhow!("账号退出恢复失败，请重试。"))?;
            let mut complete = journal.clone();
            complete.phase = DeletionPhase::Complete;
            write_journal(shared, &complete)?;
            cleanup_operation(shared, &complete, &NoRemovalFault)
        }
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
        (false, true, true, false, DeletionPhase::RollingBack) => {
            write_registry_atomic(&shared.registry(), &journal.old_registry)?;
            cleanup_operation(shared, &journal, &NoRemovalFault)
        }
        (false, true, true, true, DeletionPhase::RegistryUpdated) => {
            validate_quarantined_target(&quarantine, &journal)?;
            ensure_safe_tree(&original)?;
            let conflict = conflicting_original_path(shared, &journal)?;
            if entry_exists(&conflict)? {
                bail!("账号退出恢复目录冲突")
            }
            fs::rename(&original, &conflict)?;
            delete_conflicting_original(shared, &journal)?;
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
            true,
            DeletionPhase::Prepared
            | DeletionPhase::Quarantined
            | DeletionPhase::RestoreFailed
            | DeletionPhase::RegistryUpdated
            | DeletionPhase::RollingBack,
        ) => {
            delete_conflicting_original(shared, &journal)?;
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
            DeletionPhase::Prepared
            | DeletionPhase::Quarantined
            | DeletionPhase::RestoreFailed
            | DeletionPhase::RegistryUpdated
            | DeletionPhase::Complete,
        ) => {
            delete_conflicting_original(shared, &journal)?;
            let mut complete = journal.clone();
            complete.phase = DeletionPhase::Complete;
            write_journal(shared, &complete)?;
            cleanup_operation(shared, &complete, &NoRemovalFault)
        }
        _ => bail!("账号退出恢复状态不一致"),
    }
}

fn recover_restore_failed(
    shared: &SharedAppPaths,
    journal: &DeletionJournal,
    disk_registry: &AccountRegistry,
    original: &Path,
    quarantine: &Path,
) -> Result<()> {
    if disk_registry != &journal.old_registry && disk_registry != &journal.new_registry {
        bail!("账号退出恢复索引不一致")
    }
    validate_quarantined_target(quarantine, journal)?;
    let original_exists = entry_exists(original)?;
    if original_exists {
        ensure_safe_tree(original)?;
    }
    let conflict = conflicting_original_path(shared, journal)?;
    let conflict_exists = entry_exists(&conflict)?;
    if conflict_exists {
        ensure_safe_tree(&conflict)?;
    }
    if original_exists && conflict_exists {
        bail!("账号退出恢复目录冲突")
    }

    if disk_registry == &journal.old_registry {
        write_registry_atomic(&shared.registry(), &journal.new_registry)?;
    }
    let mut committed = journal.clone();
    committed.phase = DeletionPhase::RegistryUpdated;
    write_journal(shared, &committed)?;

    if original_exists {
        fs::rename(original, &conflict)?;
    }
    delete_conflicting_original(shared, &committed)?;
    if entry_exists(quarantine)? {
        validate_quarantined_target(quarantine, journal)?;
        delete_tree_tracked(quarantine, &NoRemovalFault)
            .map_err(|_| anyhow!("账号退出恢复失败，请重试。"))?;
    }
    committed.phase = DeletionPhase::Complete;
    write_journal(shared, &committed)?;
    cleanup_operation(shared, &committed, &NoRemovalFault)
}

fn validate_quarantined_target(path: &Path, journal: &DeletionJournal) -> Result<()> {
    ensure_safe_tree(path)?;
    let manifest = read_manifest(&path.join("account.json"), &journal.account_id)?;
    let entry = journal
        .old_registry
        .accounts
        .iter()
        .find(|entry| entry.account_id == journal.account_id)
        .ok_or_else(|| anyhow!("账号退出恢复目标无效"))?;
    if manifest.battle_tag != entry.battle_tag {
        bail!("账号退出恢复目标无效")
    }
    Ok(())
}

fn conflicting_original_path(
    shared: &SharedAppPaths,
    journal: &DeletionJournal,
) -> Result<PathBuf> {
    Ok(operation_root(shared, &journal.operation_id)?.join("conflicting-original"))
}

fn delete_conflicting_original(shared: &SharedAppPaths, journal: &DeletionJournal) -> Result<()> {
    let conflict = conflicting_original_path(shared, journal)?;
    if entry_exists(&conflict)? {
        ensure_safe_tree(&conflict)?;
        delete_tree_tracked(&conflict, &NoRemovalFault)
            .map_err(|_| anyhow!("账号退出恢复失败，请重试。"))?;
    }
    Ok(())
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

pub(crate) fn deletion_recovery_required(
    shared: &SharedAppPaths,
    registry: &AccountRegistry,
) -> bool {
    let journal_path = shared.deletion_journal();
    let metadata = match fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        Err(_) => return true,
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
        return true;
    }
    let journal: DeletionJournal = match fs::read(&journal_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    {
        Some(journal) => journal,
        None => return true,
    };
    if validate_journal(shared, &journal).is_err() || journal.phase == DeletionPhase::RestoreFailed
    {
        return true;
    }

    let operation_id = match Uuid::parse_str(&journal.operation_id) {
        Ok(operation_id) if operation_id.to_string() == journal.operation_id => operation_id,
        _ => return true,
    };
    let original = shared.account(&journal.account_id).root().to_path_buf();
    let quarantine = shared
        .operations_root()
        .join("deletions")
        .join(operation_id.to_string())
        .join(journal.account_id.as_str());
    let original_exists = match entry_exists(&original) {
        Ok(exists) => exists,
        Err(_) => return true,
    };
    let quarantine_exists = match entry_exists(&quarantine) {
        Ok(exists) => exists,
        Err(_) => return true,
    };
    if original_exists && quarantine_exists {
        return true;
    }

    registry
        .accounts
        .iter()
        .any(|entry| !stored_account_is_complete(shared, entry))
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
    fn journal_failure_after_quarantine_and_rollback_rename_failure_commits_removal() {
        let (_temp, shared, id, registry) = fixture();
        let outcome = remove_account_with_fault(
            &shared,
            &registry,
            &id,
            "2026-07-16T13:00:00Z",
            &FailAt(vec![
                RemovalFaultPoint::JournalAfterQuarantine,
                RemovalFaultPoint::RollbackRename,
            ]),
        )
        .unwrap();
        assert_eq!(outcome.completion, RemovalCompletion::RemovedPendingCleanup);
        assert_eq!(read_registry(&shared.registry()).unwrap(), outcome.registry);
        assert!(outcome.registry.accounts.is_empty());
        assert!(shared.deletion_journal().is_file());
        assert!(!shared.account(&id).root().exists());
        let journal: DeletionJournal =
            serde_json::from_slice(&fs::read(shared.deletion_journal()).unwrap()).unwrap();
        assert!(matches!(
            journal.phase,
            DeletionPhase::Prepared | DeletionPhase::RestoreFailed | DeletionPhase::RegistryUpdated
        ));
        let quarantine = quarantine_path(&shared, &journal.operation_id, &id).unwrap();
        assert_eq!(
            fs::read(quarantine.join("owstats.sqlite3")).unwrap(),
            b"synthetic-db"
        );

        recover_pending_removal(&shared).unwrap();
        assert!(!shared.account(&id).root().exists());
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
    fn recovery_required_checks_manifest_restore_failure_and_double_directory_states() {
        let (_temp, shared, journal, _) = strict_journal_fixture();
        assert!(!deletion_recovery_required(&shared, &journal.old_registry));

        fs::write(
            shared
                .account(&journal.new_registry.accounts[0].account_id)
                .manifest(),
            b"{}",
        )
        .unwrap();
        assert!(deletion_recovery_required(&shared, &journal.old_registry));

        let (_temp, shared, mut journal, _) = strict_journal_fixture();
        journal.phase = DeletionPhase::RestoreFailed;
        fs::write(
            shared.deletion_journal(),
            serde_json::to_vec_pretty(&journal).unwrap(),
        )
        .unwrap();
        assert!(deletion_recovery_required(&shared, &journal.old_registry));

        let (_temp, shared, journal, _) = strict_journal_fixture();
        let quarantine =
            quarantine_path(&shared, &journal.operation_id, &journal.account_id).unwrap();
        fs::create_dir_all(&quarantine).unwrap();
        assert!(deletion_recovery_required(&shared, &journal.old_registry));
    }

    #[test]
    fn rolling_back_with_new_registry_and_quarantine_finishes_deletion() {
        let (_temp, shared, mut journal, _) = strict_journal_fixture();
        let original = shared.account(&journal.account_id).root().to_path_buf();
        let quarantine =
            quarantine_path(&shared, &journal.operation_id, &journal.account_id).unwrap();
        fs::create_dir_all(quarantine.parent().unwrap()).unwrap();
        fs::rename(&original, &quarantine).unwrap();
        write_registry_atomic(&shared.registry(), &journal.new_registry).unwrap();
        journal.phase = DeletionPhase::RollingBack;
        fs::write(
            shared.deletion_journal(),
            serde_json::to_vec_pretty(&journal).unwrap(),
        )
        .unwrap();

        recover_pending_removal(&shared).unwrap();

        assert_eq!(
            read_registry(&shared.registry()).unwrap(),
            journal.new_registry
        );
        assert!(!original.exists());
        assert!(!quarantine.exists());
        assert!(!shared.deletion_journal().exists());
    }

    #[test]
    fn registry_updated_with_original_and_quarantine_finishes_both_directories() {
        let (_temp, shared, mut journal, _) = strict_journal_fixture();
        let original = shared.account(&journal.account_id).root().to_path_buf();
        let quarantine =
            quarantine_path(&shared, &journal.operation_id, &journal.account_id).unwrap();
        fs::create_dir_all(quarantine.parent().unwrap()).unwrap();
        fs::rename(&original, &quarantine).unwrap();
        fs::create_dir_all(&original).unwrap();
        fs::write(original.join("conflict-marker"), b"synthetic-conflict").unwrap();
        write_registry_atomic(&shared.registry(), &journal.new_registry).unwrap();
        journal.phase = DeletionPhase::RegistryUpdated;
        fs::write(
            shared.deletion_journal(),
            serde_json::to_vec_pretty(&journal).unwrap(),
        )
        .unwrap();

        recover_pending_removal(&shared).unwrap();

        assert_eq!(
            read_registry(&shared.registry()).unwrap(),
            journal.new_registry
        );
        assert!(!original.exists());
        assert!(!quarantine.exists());
        assert!(!shared.deletion_journal().exists());
    }

    #[test]
    fn impossible_post_registry_restore_finishes_as_pending_deletion_for_every_caller() {
        for trigger in [
            RemovalFaultPoint::AfterRegistryUpdated,
            RemovalFaultPoint::BeforeDelete,
            RemovalFaultPoint::UnsafeTreeAfterRegistry,
            RemovalFaultPoint::BeforeAnyDelete,
        ] {
            let (_temp, shared, id, registry) = fixture();
            let outcome = remove_account_with_fault(
                &shared,
                &registry,
                &id,
                "2026-07-16T13:00:00Z",
                &FailAt(vec![trigger, RemovalFaultPoint::RollbackRename]),
            )
            .unwrap();
            assert_eq!(outcome.completion, RemovalCompletion::RemovedPendingCleanup);
            assert_eq!(read_registry(&shared.registry()).unwrap(), outcome.registry);
            assert!(outcome.registry.accounts.is_empty());
            assert!(!shared.account(&id).root().exists());
            assert!(shared.deletion_journal().is_file());
            recover_pending_removal(&shared).unwrap();
            assert!(!shared.deletion_journal().exists());
            assert!(!shared.account(&id).root().exists());
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
