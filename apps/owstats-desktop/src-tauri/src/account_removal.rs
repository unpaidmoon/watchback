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
    AfterChildValidatedBeforeTraverse,
    AfterIsolatedValidatedBeforeReadDir,
    AfterFirstDeleteMutation,
    CleanupOperation,
}

trait RemovalFault {
    fn after(&self, _point: RemovalFaultPoint) -> Result<()> {
        Ok(())
    }

    fn after_child_validated_before_traverse(&self, _child: &Path) -> Result<()> {
        self.after(RemovalFaultPoint::AfterChildValidatedBeforeTraverse)
    }

    fn after_isolated_validated_before_read_dir(&self, _isolated: &Path) -> Result<()> {
        self.after(RemovalFaultPoint::AfterIsolatedValidatedBeforeReadDir)
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
    preflight_deletion_tree(&original).map_err(|_| anyhow!("账号数据不完整，无法退出。"))?;

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
    let mut delete_preflight = Some(match preflight_deletion_tree(&quarantine) {
        Ok(preflight) => preflight,
        Err(error) => {
            let _ = fs::rename(&quarantine, &original);
            let _ = cleanup_operation(shared, &journal, fault);
            return Err(error).context("账号数据不完整，无法退出。");
        }
    });
    journal.phase = DeletionPhase::Quarantined;
    let quarantine_journal_result = fault
        .after(RemovalFaultPoint::JournalAfterQuarantine)
        .and_then(|_| write_journal(shared, &journal));
    if quarantine_journal_result.is_err() {
        drop(delete_preflight.take());
        if fault.after(RemovalFaultPoint::RollbackRename).is_err()
            || fs::rename(&quarantine, &original).is_err()
        {
            return commit_removal_when_restore_failed(shared, &journal, fault);
        }
        let _ = cleanup_operation(shared, &journal, fault);
        return Err(anyhow!("账号退出失败，账号未更改。"));
    }
    if let Err(error) = fault.after(RemovalFaultPoint::AfterQuarantined) {
        drop(delete_preflight.take());
        return match rollback_before_registry(shared, &journal, &original, &quarantine, fault)? {
            PreRegistryRollback::Restored => Err(error),
            PreRegistryRollback::RemovalCommitted(outcome) => Ok(outcome),
        };
    }

    if let Err(error) = fault.after(RemovalFaultPoint::BeforeRegistryWrite) {
        drop(delete_preflight.take());
        return match rollback_before_registry(shared, &journal, &original, &quarantine, fault)? {
            PreRegistryRollback::Restored => Err(error),
            PreRegistryRollback::RemovalCommitted(outcome) => Ok(outcome),
        };
    }
    if let Err(error) = write_registry_atomic(&shared.registry(), &new_registry) {
        drop(delete_preflight.take());
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
        drop(delete_preflight.take());
        return match rollback_after_registry(shared, &mut journal, &original, &quarantine, fault)? {
            RollbackDisposition::RolledBack => Err(error),
            RollbackDisposition::PendingRemoval => Ok(RemovalOutcome {
                registry: new_registry,
                completion: RemovalCompletion::RemovedPendingCleanup,
            }),
        };
    }
    if let Err(error) = fault.after(RemovalFaultPoint::BeforeDelete) {
        drop(delete_preflight.take());
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
        drop(delete_preflight.take());
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
    drop(delete_preflight.take());
    if let Err(delete_failure) = delete_tree_tracked(&quarantine, fault) {
        if delete_failure.mutations == 0 {
            drop(delete_preflight.take());
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

struct DeletionPreflight {
    #[cfg(windows)]
    _handles: Vec<fs::File>,
}

#[cfg(not(windows))]
fn preflight_deletion_tree(path: &Path) -> Result<DeletionPreflight> {
    ensure_safe_tree(path)?;
    Ok(DeletionPreflight {})
}

#[cfg(windows)]
fn preflight_deletion_tree(path: &Path) -> Result<DeletionPreflight> {
    let mut handles = Vec::new();
    preflight_windows_entry(path, &mut handles)?;
    Ok(DeletionPreflight { _handles: handles })
}

#[cfg(windows)]
fn preflight_windows_entry(path: &Path, handles: &mut Vec<fs::File>) -> Result<()> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_LIST_DIRECTORY: u32 = 0x1;
    const DELETE: u32 = 0x0001_0000;
    const GENERIC_READ: u32 = 0x8000_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink()
        || before.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (!before.is_dir() && !before.is_file())
    {
        bail!("unsafe deletion entry")
    }
    let mut options = fs::OpenOptions::new();
    options
        .access_mode(if before.is_dir() {
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | DELETE
        } else {
            GENERIC_READ | FILE_READ_ATTRIBUTES | DELETE
        })
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    let handle = options.open(path)?;
    let opened = handle.metadata()?;
    if opened.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || opened.is_dir() != before.is_dir()
        || opened.is_file() != before.is_file()
    {
        bail!("unsafe deletion entry")
    }
    handles.push(handle);

    if before.is_dir() {
        let mut entries = fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for child in entries {
            preflight_windows_entry(&child, handles)?;
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn delete_tree_tracked(
    path: &Path,
    fault: &impl RemovalFault,
) -> std::result::Result<(), DeleteFailure> {
    let _preflight = match preflight_deletion_tree(path) {
        Ok(preflight) => preflight,
        Err(_) => return Err(DeleteFailure { mutations: 0 }),
    };
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

#[cfg(not(windows))]
fn delete_directory_contents(
    path: &Path,
    fault: &impl RemovalFault,
    mutations: &mut usize,
) -> Result<()> {
    let directory_metadata = fs::symlink_metadata(path)?;
    if !directory_metadata.is_dir()
        || directory_metadata.file_type().is_symlink()
        || is_windows_reparse(&directory_metadata)
    {
        bail!("unsafe linked account data")
    }
    let mut entries = fs::read_dir(path)?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for child in entries {
        let metadata = fs::symlink_metadata(&child)?;
        if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
            bail!("unsafe linked account data")
        }
        if !metadata.is_dir() && !metadata.is_file() {
            bail!("unsafe account data entry")
        }
        fault.after_child_validated_before_traverse(&child)?;
        let isolated = child.with_file_name(format!(".deleting-{}", Uuid::new_v4()));
        fs::rename(&child, &isolated)?;
        *mutations += 1;
        if *mutations == 1
            && fault
                .after(RemovalFaultPoint::AfterFirstDeleteMutation)
                .is_err()
        {
            bail!("synthetic partial deletion")
        }
        let isolated_metadata = fs::symlink_metadata(&isolated)?;
        if isolated_metadata.file_type().is_symlink() || is_windows_reparse(&isolated_metadata) {
            bail!("unsafe linked account data")
        }
        if isolated_metadata.is_dir() {
            delete_directory_contents(&isolated, fault, mutations)?;
            fs::remove_dir(&isolated)?;
        } else if isolated_metadata.is_file() {
            fs::remove_file(&isolated)?;
        } else {
            bail!("unsafe account data entry")
        }
        *mutations += 1;
    }
    Ok(())
}

#[cfg(windows)]
struct PinnedWindowsEntry {
    handle: fs::File,
    is_dir: bool,
}

#[cfg(windows)]
fn pin_windows_entry(path: &Path) -> Result<PinnedWindowsEntry> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    const FILE_LIST_DIRECTORY: u32 = 0x1;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const GENERIC_READ: u32 = 0x8000_0000;
    const DELETE: u32 = 0x0001_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    let before = fs::symlink_metadata(path).map_err(|_| anyhow!("账号数据清理失败。"))?;
    if before.file_type().is_symlink()
        || before.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (!before.is_dir() && !before.is_file())
    {
        bail!("账号数据清理失败。")
    }
    let mut options = fs::OpenOptions::new();
    options
        .access_mode(if before.is_dir() {
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | DELETE
        } else {
            GENERIC_READ | FILE_READ_ATTRIBUTES | DELETE
        })
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    let handle = options
        .open(path)
        .map_err(|_| anyhow!("账号数据清理失败。"))?;
    let opened = handle
        .metadata()
        .map_err(|_| anyhow!("账号数据清理失败。"))?;
    if opened.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || opened.is_dir() != before.is_dir()
        || opened.is_file() != before.is_file()
    {
        bail!("账号数据清理失败。")
    }
    Ok(PinnedWindowsEntry {
        handle,
        is_dir: before.is_dir(),
    })
}

#[cfg(windows)]
fn delete_tree_tracked(
    path: &Path,
    fault: &impl RemovalFault,
) -> std::result::Result<(), DeleteFailure> {
    let preflight = match preflight_deletion_tree(path) {
        Ok(preflight) => preflight,
        Err(_) => return Err(DeleteFailure { mutations: 0 }),
    };
    drop(preflight);
    if fault.after(RemovalFaultPoint::BeforeAnyDelete).is_err() {
        return Err(DeleteFailure { mutations: 0 });
    }
    let pinned = match pin_windows_entry(path) {
        Ok(pinned) => pinned,
        Err(_) => return Err(DeleteFailure { mutations: 0 }),
    };
    let mut mutations = 0usize;
    if delete_pinned_windows_entry(path, pinned, fault, &mut mutations, false).is_err() {
        return Err(DeleteFailure { mutations });
    }
    Ok(())
}

#[cfg(windows)]
fn delete_pinned_windows_entry(
    path: &Path,
    pinned: PinnedWindowsEntry,
    fault: &impl RemovalFault,
    mutations: &mut usize,
    isolated_directory: bool,
) -> Result<()> {
    if pinned.is_dir {
        if isolated_directory {
            fault.after_isolated_validated_before_read_dir(path)?;
        }
        let mut entries = fs::read_dir(path)
            .map_err(|_| anyhow!("账号数据清理失败。"))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|_| anyhow!("账号数据清理失败。"))?;
        entries.sort();
        for child in entries {
            let metadata =
                fs::symlink_metadata(&child).map_err(|_| anyhow!("账号数据清理失败。"))?;
            if metadata.file_type().is_symlink()
                || is_windows_reparse(&metadata)
                || (!metadata.is_dir() && !metadata.is_file())
            {
                bail!("账号数据清理失败。")
            }
            fault.after_child_validated_before_traverse(&child)?;
            let isolated = child.with_file_name(format!(".deleting-{}", Uuid::new_v4()));
            fs::rename(&child, &isolated).map_err(|_| anyhow!("账号数据清理失败。"))?;
            record_delete_mutation(fault, mutations)?;
            let child_pin = pin_windows_entry(&isolated)?;
            let child_is_dir = child_pin.is_dir;
            delete_pinned_windows_entry(&isolated, child_pin, fault, mutations, child_is_dir)?;
        }
    }
    mark_windows_handle_for_delete(&pinned.handle)?;
    record_delete_mutation(fault, mutations)?;
    drop(pinned);
    Ok(())
}

#[cfg(windows)]
fn record_delete_mutation(fault: &impl RemovalFault, mutations: &mut usize) -> Result<()> {
    *mutations += 1;
    if *mutations == 1 {
        fault.after(RemovalFaultPoint::AfterFirstDeleteMutation)?;
    }
    Ok(())
}

#[cfg(windows)]
fn mark_windows_handle_for_delete(handle: &fs::File) -> Result<()> {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: i32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetFileInformationByHandle(
            file: *mut c_void,
            info_class: i32,
            info: *const c_void,
            size: u32,
        ) -> i32;
    }

    const FILE_DISPOSITION_INFO_CLASS: i32 = 4;
    let info = FileDispositionInfo { delete_file: 1 };
    let succeeded = unsafe {
        SetFileInformationByHandle(
            handle.as_raw_handle(),
            FILE_DISPOSITION_INFO_CLASS,
            (&info as *const FileDispositionInfo).cast(),
            std::mem::size_of::<FileDispositionInfo>() as u32,
        )
    };
    if succeeded == 0 {
        bail!("账号数据清理失败。")
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

pub(crate) fn deletion_journal_proves_removed_account(
    shared: &SharedAppPaths,
    account_id: &AccountId,
    login_old_registry: &AccountRegistry,
    login_intended_registry: &AccountRegistry,
    disk_registry: &AccountRegistry,
) -> bool {
    let journal_path = shared.deletion_journal();
    let metadata = match fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
        return false;
    }
    let journal: DeletionJournal = match fs::read(&journal_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    {
        Some(journal) => journal,
        None => return false,
    };
    validate_journal(shared, &journal).is_ok()
        && &journal.account_id == account_id
        && (&journal.old_registry == login_old_registry
            || &journal.old_registry == login_intended_registry)
        && &journal.new_registry == disk_registry
        && !disk_registry
            .accounts
            .iter()
            .any(|entry| &entry.account_id == account_id)
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

    #[cfg(windows)]
    #[test]
    fn windows_exclusive_database_lock_prevents_every_removal_mutation() {
        use std::os::windows::fs::OpenOptionsExt;

        let (_temp, shared, id, registry) = fixture();
        let original_database = fs::read(shared.account(&id).database()).unwrap();
        let _exclusive = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(shared.account(&id).database())
            .unwrap();
        assert!(preflight_deletion_tree(shared.account(&id).root()).is_err());

        let removal = remove_account(&shared, &registry, &id, "2026-07-16T13:00:00Z");
        drop(_exclusive);
        assert!(removal.is_err());
        assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
        assert!(shared.account(&id).manifest().is_file());
        assert_eq!(
            fs::read(shared.account(&id).database()).unwrap(),
            original_database
        );
        assert!(!shared.deletion_journal().exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_handle_without_delete_share_blocks_preflight_before_mutation() {
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        let (_temp, shared, id, registry) = fixture();
        let original_database = fs::read(shared.account(&id).database()).unwrap();
        let original_manifest = fs::read(shared.account(&id).manifest()).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(shared.account(&id).database())
            .unwrap();
        assert!(preflight_deletion_tree(shared.account(&id).root()).is_err());

        let removal = remove_account(&shared, &registry, &id, "2026-07-16T13:00:00Z");
        drop(lock);

        assert!(removal.is_err());
        assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
        assert_eq!(
            fs::read(shared.account(&id).manifest()).unwrap(),
            original_manifest
        );
        assert_eq!(
            fs::read(shared.account(&id).database()).unwrap(),
            original_database
        );
        assert!(!shared.deletion_journal().exists());
    }

    #[cfg(windows)]
    #[test]
    fn validated_child_replaced_by_junction_is_isolated_without_traversing_target() {
        use std::cell::Cell;
        use std::process::Command;

        struct ReplaceAfterValidation {
            outside: PathBuf,
            replaced: Cell<bool>,
        }

        impl RemovalFault for ReplaceAfterValidation {
            fn after_child_validated_before_traverse(&self, child: &Path) -> Result<()> {
                if self.replaced.get()
                    || child.file_name().and_then(|name| name.to_str()) != Some("nested")
                {
                    return Ok(());
                }
                let moved = child.with_file_name("attacker-moved");
                fs::rename(child, &moved)?;
                let status = Command::new("cmd")
                    .args(["/c", "mklink", "/J"])
                    .arg(child)
                    .arg(&self.outside)
                    .status()?;
                if !status.success() {
                    bail!("failed to create synthetic junction")
                }
                self.replaced.set(true);
                Ok(())
            }
        }

        let (_temp, shared, id, registry) = fixture();
        fs::create_dir(shared.account(&id).root().join("nested")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("sentinel.txt"), b"outside-sentinel").unwrap();
        let fault = ReplaceAfterValidation {
            outside: outside.path().to_path_buf(),
            replaced: Cell::new(false),
        };

        let outcome =
            remove_account_with_fault(&shared, &registry, &id, "2026-07-16T13:00:00Z", &fault)
                .unwrap();

        assert_eq!(outcome.completion, RemovalCompletion::RemovedPendingCleanup);
        assert_eq!(
            fs::read(outside.path().join("sentinel.txt")).unwrap(),
            b"outside-sentinel"
        );
        assert!(shared.deletion_journal().is_file());
        let journal: DeletionJournal =
            serde_json::from_slice(&fs::read(shared.deletion_journal()).unwrap()).unwrap();
        let quarantine = quarantine_path(&shared, &journal.operation_id, &id).unwrap();
        let isolated_junction = fs::read_dir(&quarantine)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| is_windows_reparse(&fs::symlink_metadata(path).unwrap()))
            .expect("replaced junction must remain isolated for safe retry");
        fs::remove_dir(&isolated_junction).unwrap();
        recover_pending_removal(&shared).unwrap();
        assert!(!shared.deletion_journal().exists());
        assert_eq!(
            fs::read(outside.path().join("sentinel.txt")).unwrap(),
            b"outside-sentinel"
        );
    }

    #[cfg(windows)]
    #[test]
    fn pinned_isolated_directory_cannot_be_moved_before_read_dir() {
        use std::cell::Cell;
        use std::process::{Command, Stdio};

        struct TryReplacePinned {
            outside: PathBuf,
            attempted: Cell<bool>,
        }

        impl RemovalFault for TryReplacePinned {
            fn after_isolated_validated_before_read_dir(&self, isolated: &Path) -> Result<()> {
                if self.attempted.get()
                    || !isolated
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with(".deleting-"))
                {
                    return Ok(());
                }
                self.attempted.set(true);
                let moved = isolated.with_file_name("attacker-moved-pinned");
                assert!(fs::rename(isolated, &moved).is_err());
                assert!(!moved.exists());
                let junction = Command::new("cmd")
                    .args(["/c", "mklink", "/J"])
                    .arg(isolated)
                    .arg(&self.outside)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()?;
                assert!(!junction.success());
                assert!(self.outside.join("sentinel.txt").is_file());
                Ok(())
            }
        }

        let (_temp, shared, id, registry) = fixture();
        let nested = shared.account(&id).root().join("nested");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("inside.txt"), b"inside").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("sentinel.txt"), b"outside-sentinel").unwrap();
        let fault = TryReplacePinned {
            outside: outside.path().to_path_buf(),
            attempted: Cell::new(false),
        };

        let outcome =
            remove_account_with_fault(&shared, &registry, &id, "2026-07-16T13:00:00Z", &fault)
                .unwrap();

        assert!(fault.attempted.get());
        assert_eq!(outcome.completion, RemovalCompletion::Removed);
        assert_eq!(
            fs::read(outside.path().join("sentinel.txt")).unwrap(),
            b"outside-sentinel"
        );
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
