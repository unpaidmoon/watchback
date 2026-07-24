use std::{
    ffi::OsString,
    fs::{self, Metadata},
    path::{Component, Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::{
    account_store::{read_manifest, read_registry},
    app_paths::SharedAppPaths,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDataMigrationOutcome {
    Created,
    ExistingData,
    Migrated,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationMarker {
    version: u32,
    source_retained: bool,
}

pub fn prepare_install_data(
    source: Option<&Path>,
    destination: &SharedAppPaths,
) -> Result<InstallDataMigrationOutcome> {
    match fs::symlink_metadata(destination.root()) {
        Ok(metadata) if is_supported_directory(&metadata) => {
            return Ok(InstallDataMigrationOutcome::ExistingData);
        }
        Ok(_) => return Err(anyhow!("安装目录中的数据路径不是目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("无法读取安装数据目录"),
    }

    let staging = destination.install_migration_staging();
    let source = match source {
        Some(source) => Some(validate_source_and_staging(source, &staging)?),
        None => None,
    };
    if source.is_none() {
        canonicalize_future_path(&staging)?;
    }

    #[cfg(windows)]
    {
        let marker = source
            .as_ref()
            .map(|_| {
                serde_json::to_vec_pretty(&MigrationMarker {
                    version: 1,
                    source_retained: true,
                })
            })
            .transpose()?;
        return windows_secure::prepare_install_data(
            source.as_deref(),
            &staging,
            destination.root(),
            marker.as_deref(),
        );
    }

    #[cfg(not(windows))]
    {
        let Some(source) = source else {
            return create_empty_data_directory(&staging, destination.root());
        };

        reset_staging_by_path(&staging)?;
        fs::create_dir_all(
            staging
                .parent()
                .ok_or_else(|| anyhow!("无法定位迁移临时目录"))?,
        )
        .context("无法创建迁移临时目录的父目录")?;
        copy_tree_by_path(&source, &staging)?;
        validate_staged_data(&staging)?;
        let marker = serde_json::to_vec_pretty(&MigrationMarker {
            version: 1,
            source_retained: true,
        })?;
        fs::write(staging.join(".install-data-migration-v1.json"), marker)
            .context("无法写入迁移完成标记")?;
        invoke_non_windows_migration_hook(
            NonWindowsMigrationHookPoint::BeforePublish,
            &staging,
            destination.root(),
        );
        publish_staged_data(&staging, destination.root())?;
        Ok(InstallDataMigrationOutcome::Migrated)
    }
}

fn validate_source_and_staging(source: &Path, staging: &Path) -> Result<PathBuf> {
    let metadata = match fs::symlink_metadata(source) {
        Ok(metadata) if is_supported_directory(&metadata) => metadata,
        Ok(metadata) if is_unsupported_link_or_reparse_point(&metadata) => {
            return Err(anyhow!("迁移源包含不受支持的链接"));
        }
        Ok(_) => return Err(anyhow!("迁移源包含不受支持的文件类型")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(anyhow!("迁移源不存在"));
        }
        Err(error) => return Err(error).context("无法读取迁移源"),
    };
    debug_assert!(is_supported_directory(&metadata));

    let source = fs::canonicalize(source).context("无法规范化迁移源")?;
    let staging = canonicalize_future_path(staging)?;
    if source.starts_with(&staging) || staging.starts_with(&source) {
        return Err(anyhow!("迁移源与迁移临时目录重叠"));
    }
    Ok(source)
}

fn canonicalize_future_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(anyhow!("迁移临时目录路径无效"));
    }

    let mut current = path;
    let mut missing = Vec::<OsString>::new();
    loop {
        match fs::symlink_metadata(current) {
            Ok(metadata) if is_supported_directory(&metadata) => {
                let mut canonical = fs::canonicalize(current).context("无法规范化迁移临时目录")?;
                for component in missing.iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Ok(_) => return Err(anyhow!("迁移临时目录路径无效")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = current
                    .file_name()
                    .ok_or_else(|| anyhow!("迁移临时目录路径无效"))?;
                missing.push(name.to_os_string());
                current = current
                    .parent()
                    .ok_or_else(|| anyhow!("迁移临时目录路径无效"))?;
            }
            Err(error) => return Err(error).context("无法读取迁移临时目录"),
        }
    }
}

#[cfg(not(windows))]
fn create_empty_data_directory(
    staging: &Path,
    destination: &Path,
) -> Result<InstallDataMigrationOutcome> {
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("无法定位安装数据目录"))?;
    fs::create_dir_all(parent).context("无法创建安装数据目录的父目录")?;
    invoke_non_windows_migration_hook(
        NonWindowsMigrationHookPoint::BeforePublish,
        staging,
        destination,
    );
    fs::create_dir(destination).context("无法排他创建安装数据目录")?;
    Ok(InstallDataMigrationOutcome::Created)
}

#[cfg(not(windows))]
fn copy_tree_by_path(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source).context("无法读取迁移源")?;
    if is_unsupported_link_or_reparse_point(&metadata) {
        return Err(anyhow!("迁移源包含不受支持的链接"));
    }
    if metadata.is_file() {
        fs::copy(source, target).context("无法复制迁移文件")?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(anyhow!("迁移源包含不受支持的文件类型"));
    }

    fs::create_dir(target).context("无法创建迁移目录")?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(source).context("无法枚举迁移源")? {
        entries.push(entry.context("无法读取迁移目录项")?);
    }
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        copy_tree_by_path(&entry.path(), &target.join(entry.file_name()))?;
    }
    Ok(())
}

#[cfg(all(
    not(windows),
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
fn publish_staged_data(staging: &Path, destination: &Path) -> Result<()> {
    use rustix::fs::{renameat_with, RenameFlags};

    let parent = staging
        .parent()
        .ok_or_else(|| anyhow!("无法定位迁移临时目录"))?;
    if destination.parent() != Some(parent) {
        return Err(anyhow!("迁移发布路径不在同一目录"));
    }
    let staging_name = staging
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("迁移临时目录名称无效"))?;
    let destination_name = destination
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("安装数据目录名称无效"))?;
    let parent = fs::File::open(parent).context("无法固定迁移临时目录的父目录")?;

    renameat_with(
        &parent,
        staging_name,
        &parent,
        destination_name,
        RenameFlags::NOREPLACE,
    )
    .context("无法排他发布安装数据目录")
}

#[cfg(all(
    not(windows),
    not(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    ))
))]
fn publish_staged_data(_staging: &Path, _destination: &Path) -> Result<()> {
    Err(anyhow!("当前平台不支持安全发布安装数据目录"))
}

#[cfg(not(windows))]
fn reset_staging_by_path(staging: &Path) -> Result<()> {
    match fs::symlink_metadata(staging) {
        Ok(_) => remove_tree_without_links(staging).context("无法重置迁移临时目录"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("无法读取迁移临时目录"),
    }
}

#[cfg(not(windows))]
fn remove_tree_without_links(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).context("无法读取迁移临时目录")?;
    if is_unsupported_link_or_reparse_point(&metadata) {
        return Err(anyhow!("迁移临时目录包含不受支持的链接"));
    }
    if metadata.is_file() {
        return fs::remove_file(path).context("无法删除迁移临时文件");
    }
    if !metadata.is_dir() {
        return Err(anyhow!("迁移临时目录包含不受支持的文件类型"));
    }

    for entry in fs::read_dir(path).context("无法枚举迁移临时目录")? {
        remove_tree_without_links(&entry.context("无法读取迁移临时目录项")?.path())?;
    }
    fs::remove_dir(path).context("无法删除迁移临时目录")
}

fn validate_staged_data(staging: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(staging).context("无法读取迁移临时目录")?;
    if !is_supported_directory(&metadata) {
        Err(anyhow!("迁移临时目录无效"))
    } else {
        let registry_path = staging.join("accounts.json");
        let registry = match fs::symlink_metadata(&registry_path) {
            Ok(_) => read_registry(&registry_path).context("旧账户索引校验失败")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("无法读取旧账户索引"),
        };
        let accounts_root = staging.join("accounts");
        if !registry.accounts.is_empty() {
            require_supported_directory(&accounts_root, "旧账户目录校验失败")?;
        }

        for entry in registry.accounts {
            let account = accounts_root.join(entry.account_id.as_str());
            require_supported_directory(&account, "旧账户目录校验失败")?;
            read_manifest(&account.join("account.json"), &entry.account_id)
                .context("旧账户清单校验失败")?;

            let database = account.join("owstats.sqlite3");
            match fs::symlink_metadata(&database) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink()
                        || is_unsupported_link_or_reparse_point(&metadata)
                        || !metadata.is_file()
                    {
                        return Err(anyhow!("旧账户数据库校验失败"));
                    }
                    let connection =
                        Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)
                            .context("旧账户数据库校验失败")?;
                    let result: String = connection
                        .query_row("PRAGMA quick_check", [], |row| row.get(0))
                        .context("旧账户数据库完整性校验失败")?;
                    if result != "ok" {
                        return Err(anyhow!("旧账户数据库完整性校验失败"));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("无法读取旧账户数据库"),
            }
        }
        Ok(())
    }
}

fn require_supported_directory(path: &Path, context: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).with_context(|| context.to_owned())?;
    if is_supported_directory(&metadata) {
        Ok(())
    } else {
        Err(anyhow!(context.to_owned()))
    }
}

fn is_supported_directory(metadata: &Metadata) -> bool {
    metadata.is_dir() && !is_unsupported_link_or_reparse_point(metadata)
}

fn is_unsupported_link_or_reparse_point(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        metadata.file_attributes() & 0x400 != 0
    }

    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
#[doc(hidden)]
pub fn set_windows_copy_open_hook(hook: Option<fn(&Path)>) {
    windows_secure::set_copy_open_hook(hook);
}

#[cfg(windows)]
#[doc(hidden)]
pub fn set_windows_copy_enumeration_hook(hook: Option<fn()>) {
    windows_secure::set_copy_enumeration_hook(hook);
}

#[cfg(windows)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsCopyLimits {
    max_depth: usize,
    max_entries: u64,
    max_total_bytes: u64,
}

#[cfg(windows)]
impl WindowsCopyLimits {
    pub const fn new(max_depth: usize, max_entries: u64, max_total_bytes: u64) -> Self {
        Self {
            max_depth,
            max_entries,
            max_total_bytes,
        }
    }
}

#[cfg(windows)]
#[doc(hidden)]
pub fn set_windows_copy_limits_for_test(limits: Option<WindowsCopyLimits>) {
    windows_secure::set_copy_limits(limits);
}

#[cfg(windows)]
#[doc(hidden)]
pub fn set_windows_cleanup_enumeration_hook(hook: Option<fn()>) {
    windows_secure::set_cleanup_enumeration_hook(hook);
}

#[cfg(windows)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsCleanupLimits {
    max_depth: usize,
    max_entries: u64,
}

#[cfg(windows)]
impl WindowsCleanupLimits {
    pub const fn new(max_depth: usize, max_entries: u64) -> Self {
        Self {
            max_depth,
            max_entries,
        }
    }
}

#[cfg(windows)]
#[doc(hidden)]
pub fn set_windows_cleanup_limits_for_test(limits: Option<WindowsCleanupLimits>) {
    windows_secure::set_cleanup_limits(limits);
}

#[cfg(windows)]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsMigrationHookPoint {
    StagingCreated,
    BeforeValidation,
    BeforePublish,
}

#[cfg(windows)]
#[doc(hidden)]
pub fn set_windows_migration_hook(hook: Option<fn(WindowsMigrationHookPoint, &Path, &Path)>) {
    windows_secure::set_migration_hook(hook);
}

#[cfg(not(windows))]
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonWindowsMigrationHookPoint {
    BeforePublish,
}

#[cfg(not(windows))]
std::thread_local! {
    static NON_WINDOWS_MIGRATION_HOOK: std::cell::Cell<
        Option<fn(NonWindowsMigrationHookPoint, &Path, &Path)>,
    > = const { std::cell::Cell::new(None) };
}

#[cfg(not(windows))]
#[doc(hidden)]
pub fn set_non_windows_migration_hook(
    hook: Option<fn(NonWindowsMigrationHookPoint, &Path, &Path)>,
) {
    NON_WINDOWS_MIGRATION_HOOK.with(|value| value.set(hook));
}

#[cfg(not(windows))]
fn invoke_non_windows_migration_hook(
    point: NonWindowsMigrationHookPoint,
    staging: &Path,
    destination: &Path,
) {
    NON_WINDOWS_MIGRATION_HOOK.with(|value| {
        if let Some(hook) = value.get() {
            hook(point, staging, destination);
        }
    });
}

#[cfg(windows)]
mod windows_secure {
    use std::{
        cell::Cell,
        ffi::{OsStr, OsString},
        fs::{self, File},
        io::{Read, Write},
        mem::{offset_of, size_of},
        os::windows::{
            ffi::{OsStrExt, OsStringExt},
            io::FromRawHandle,
        },
        path::{Path, PathBuf},
    };

    use anyhow::{anyhow, Context, Result};
    use rusqlite::{Connection, OpenFlags};
    use serde_json::from_slice;
    use uuid::Uuid;
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                NtCreateFile, FILE_CREATE, FILE_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
                FILE_SYNCHRONOUS_IO_NONALERT,
            },
        },
        Win32::{
            Foundation::{
                CloseHandle, LocalFree, ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE,
                ERROR_INVALID_PARAMETER, ERROR_SUCCESS, HANDLE, INVALID_HANDLE_VALUE,
                STATUS_OBJECT_NAME_NOT_FOUND, STATUS_OBJECT_PATH_NOT_FOUND, UNICODE_STRING,
            },
            Security::{
                Authorization::{
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, SetSecurityInfo,
                    SDDL_REVISION_1, SE_FILE_OBJECT,
                },
                GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION,
                PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
            },
            Storage::FileSystem::{
                CreateFileW, FileDispositionInfo, FileRenameInfo, GetFileInformationByHandle,
                GetFileType, GetFinalPathNameByHandleW, SetFileInformationByHandle,
                BY_HANDLE_FILE_INFORMATION, CREATE_NEW, DELETE, FILE_ATTRIBUTE_DIRECTORY,
                FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO,
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
                FILE_GENERIC_WRITE, FILE_LIST_DIRECTORY, FILE_NAME_NORMALIZED,
                FILE_READ_ATTRIBUTES, FILE_RENAME_INFO, FILE_SHARE_READ, FILE_SHARE_WRITE,
                FILE_TRAVERSE, FILE_TYPE_DISK, OPEN_EXISTING, READ_CONTROL, SYNCHRONIZE,
                VOLUME_NAME_DOS, WRITE_DAC,
            },
            System::{
                Kernel::{OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE},
                IO::IO_STATUS_BLOCK,
            },
        },
    };

    use super::{
        InstallDataMigrationOutcome, WindowsCleanupLimits, WindowsCopyLimits,
        WindowsMigrationHookPoint,
    };
    use crate::account_store::{
        validate_manifest, validate_registry, AccountManifest, AccountRegistry,
    };

    const DEFAULT_CLEANUP_LIMITS: WindowsCleanupLimits = WindowsCleanupLimits::new(64, 100_000);
    const DEFAULT_COPY_LIMITS: WindowsCopyLimits =
        WindowsCopyLimits::new(64, 100_000, 20 * 1024 * 1024 * 1024);

    thread_local! {
        static CLEANUP_ENUMERATION_HOOK: Cell<Option<fn()>> = const { Cell::new(None) };
        static CLEANUP_LIMITS: Cell<Option<WindowsCleanupLimits>> = const { Cell::new(None) };
        static COPY_OPEN_HOOK: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        static COPY_ENUMERATION_HOOK: Cell<Option<fn()>> = const { Cell::new(None) };
        static COPY_LIMITS: Cell<Option<WindowsCopyLimits>> = const { Cell::new(None) };
        static MIGRATION_HOOK: Cell<
            Option<fn(WindowsMigrationHookPoint, &Path, &Path)>,
        > = const { Cell::new(None) };
    }

    pub(super) fn set_cleanup_enumeration_hook(hook: Option<fn()>) {
        CLEANUP_ENUMERATION_HOOK.with(|value| value.set(hook));
    }

    pub(super) fn set_cleanup_limits(limits: Option<WindowsCleanupLimits>) {
        CLEANUP_LIMITS.with(|value| value.set(limits));
    }

    pub(super) fn set_copy_open_hook(hook: Option<fn(&Path)>) {
        COPY_OPEN_HOOK.with(|value| value.set(hook));
    }

    pub(super) fn set_copy_enumeration_hook(hook: Option<fn()>) {
        COPY_ENUMERATION_HOOK.with(|value| value.set(hook));
    }

    pub(super) fn set_copy_limits(limits: Option<WindowsCopyLimits>) {
        COPY_LIMITS.with(|value| value.set(limits));
    }

    pub(super) fn set_migration_hook(hook: Option<fn(WindowsMigrationHookPoint, &Path, &Path)>) {
        MIGRATION_HOOK.with(|value| value.set(hook));
    }

    fn invoke_copy_open_hook(path: &Path) {
        COPY_OPEN_HOOK.with(|value| {
            if let Some(hook) = value.get() {
                hook(path);
            }
        });
    }

    fn invoke_cleanup_enumeration_hook() {
        CLEANUP_ENUMERATION_HOOK.with(|value| {
            if let Some(hook) = value.get() {
                hook();
            }
        });
    }

    fn invoke_copy_enumeration_hook() {
        COPY_ENUMERATION_HOOK.with(|value| {
            if let Some(hook) = value.get() {
                hook();
            }
        });
    }

    fn invoke_migration_hook(point: WindowsMigrationHookPoint, staging: &Path, destination: &Path) {
        MIGRATION_HOOK.with(|value| {
            if let Some(hook) = value.get() {
                hook(point, staging, destination);
            }
        });
    }

    struct FixedHandle(HANDLE);

    impl FixedHandle {
        fn into_file(self) -> File {
            let handle = self.0;
            std::mem::forget(self);
            unsafe { File::from_raw_handle(handle as _) }
        }
    }

    impl Drop for FixedHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    struct FixedEntry {
        handle: FixedHandle,
        information: BY_HANDLE_FILE_INFORMATION,
    }

    struct LocalSecurityDescriptor(PSECURITY_DESCRIPTOR);

    impl Drop for LocalSecurityDescriptor {
        fn drop(&mut self) {
            unsafe {
                let _ = LocalFree(self.0);
            }
        }
    }

    #[derive(Default)]
    struct ValidationGuards {
        directories: Vec<FixedHandle>,
        files: Vec<File>,
    }

    struct DeleteFrame {
        handle: FixedHandle,
        entries: fs::ReadDir,
        depth: usize,
    }

    #[derive(Default)]
    struct CleanupBudget {
        entries: u64,
    }

    impl CleanupBudget {
        fn record_entry(&mut self, limits: WindowsCleanupLimits) -> Result<()> {
            let entries = self
                .entries
                .checked_add(1)
                .ok_or_else(|| anyhow!("迁移临时目录项数量超过安全限制"))?;
            if entries > limits.max_entries {
                return Err(anyhow!("迁移临时目录项数量超过安全限制"));
            }
            self.entries = entries;
            Ok(())
        }
    }

    #[derive(Default)]
    struct CopyBudget {
        // Only untrusted source entries and source bytes are charged. The
        // fixed-size marker is generated locally after the copy completes.
        entries: u64,
        bytes: u64,
    }

    impl CopyBudget {
        fn record_entry(&mut self, limits: WindowsCopyLimits) -> Result<()> {
            let entries = self
                .entries
                .checked_add(1)
                .ok_or_else(|| anyhow!("迁移目录项数量超过安全限制"))?;
            if entries > limits.max_entries {
                return Err(anyhow!("迁移目录项数量超过安全限制"));
            }
            self.entries = entries;
            Ok(())
        }

        fn record_bytes(&mut self, bytes: usize, limits: WindowsCopyLimits) -> Result<()> {
            let bytes = u64::try_from(bytes).map_err(|_| anyhow!("迁移数据大小超过安全限制"))?;
            let total = self
                .bytes
                .checked_add(bytes)
                .ok_or_else(|| anyhow!("迁移数据大小超过安全限制"))?;
            if total > limits.max_total_bytes {
                return Err(anyhow!("迁移数据大小超过安全限制"));
            }
            self.bytes = total;
            Ok(())
        }
    }

    pub(super) fn prepare_install_data(
        source: Option<&Path>,
        staging: &Path,
        destination: &Path,
        marker: Option<&[u8]>,
    ) -> Result<InstallDataMigrationOutcome> {
        let parent = staging
            .parent()
            .ok_or_else(|| anyhow!("无法定位迁移临时目录"))?;
        if destination.parent() != Some(parent) {
            return Err(anyhow!("迁移发布路径不在同一目录"));
        }
        let staging_name = child_name(staging)?;
        let destination_name = child_name(destination)?;
        if staging_name == destination_name {
            return Err(anyhow!("迁移临时目录与数据目录冲突"));
        }

        fs::create_dir_all(parent).context("无法创建迁移临时目录的父目录")?;
        let parent_entry = open_fixed(
            parent,
            FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        )
        .context("无法固定迁移临时目录的父目录")?;
        require_target_directory(&parent_entry.information, "迁移临时目录的父目录无效")?;
        // open_fixed omits FILE_SHARE_DELETE. Derive the publish path from that
        // fixed object's canonical name instead of reusing the caller's path.
        let pinned_parent = final_path(&parent_entry.handle)?;
        let pinned_staging = pinned_parent.join(staging_name);
        let pinned_destination = pinned_parent.join(destination_name);

        match fs::symlink_metadata(&pinned_destination) {
            Ok(metadata) if super::is_supported_directory(&metadata) => {
                return Ok(InstallDataMigrationOutcome::ExistingData);
            }
            Ok(_) => return Err(anyhow!("安装目录中的数据路径不是目录")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("无法读取安装数据目录"),
        }

        let Some(source) = source else {
            invoke_migration_hook(
                WindowsMigrationHookPoint::BeforePublish,
                staging,
                destination,
            );
            let data_entry = create_directory_relative(&parent_entry.handle, destination_name)?;
            require_target_directory(&data_entry.information, "安装数据目录无效")?;
            return Ok(InstallDataMigrationOutcome::Created);
        };
        let marker = marker.ok_or_else(|| anyhow!("无法生成迁移完成标记"))?;

        reset_staging(&pinned_staging)?;
        let staging_entry =
            create_protected_directory_relative(&parent_entry.handle, staging_name)?;
        require_target_directory(&staging_entry.information, "迁移临时目录无效")?;
        // Test seam at the first instant after the atomic create returns. The
        // create-time descriptor must already exclude ordinary users here.
        invoke_migration_hook(
            WindowsMigrationHookPoint::StagingCreated,
            staging,
            destination,
        );
        // Reapply through the fixed handle as an explicit fail-closed check
        // before production code writes any descendants.
        protect_staging(&staging_entry.handle)?;

        copy_tree(source, &pinned_staging, &staging_entry.handle)?;
        invoke_migration_hook(
            WindowsMigrationHookPoint::BeforeValidation,
            staging,
            destination,
        );
        let validation_guards = validate_staged_data(&staging_entry.handle)?;
        write_marker(
            &pinned_staging.join(".install-data-migration-v1.json"),
            marker,
        )?;
        let staged_information = information(&staging_entry.handle)?;
        require_target_directory(&staged_information, "迁移临时目录无效")?;

        // Registry, manifest, database, WAL, and SHM guards intentionally omit
        // DELETE sharing so validation observes fixed objects. Windows cannot
        // rename their parent while those handles remain open, so release them
        // only after every staged write/check. The protected root DACL then
        // excludes ordinary users during the short hook-to-FileRenameInfo
        // window; the fixed root handle still prevents root replacement.
        drop(validation_guards);
        invoke_migration_hook(
            WindowsMigrationHookPoint::BeforePublish,
            staging,
            destination,
        );
        rename_without_replace(
            &staging_entry.handle,
            &parent_entry.handle,
            &pinned_destination,
        )?;

        let published_information = information(&staging_entry.handle)?;
        require_target_directory(&published_information, "安装数据目录无效")?;
        Ok(InstallDataMigrationOutcome::Migrated)
    }

    fn protected_staging_security_descriptor() -> Result<LocalSecurityDescriptor> {
        let sddl = OsStr::new("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut descriptor = std::ptr::null_mut();
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if converted == 0 || descriptor.is_null() {
            return Err(anyhow!("无法构造迁移临时目录权限"));
        }
        Ok(LocalSecurityDescriptor(descriptor))
    }

    fn protect_staging(staging: &FixedHandle) -> Result<()> {
        let descriptor = protected_staging_security_descriptor()?;
        let mut dacl_present = 0;
        let mut dacl_defaulted = 0;
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let read_dacl = unsafe {
            GetSecurityDescriptorDacl(
                descriptor.0,
                &mut dacl_present,
                &mut dacl,
                &mut dacl_defaulted,
            )
        };
        if read_dacl == 0 || dacl_present == 0 || dacl.is_null() {
            return Err(anyhow!("无法读取迁移临时目录权限"));
        }
        let status = unsafe {
            SetSecurityInfo(
                staging.0,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                dacl,
                std::ptr::null(),
            )
        };
        if status != ERROR_SUCCESS {
            let message = match status {
                ERROR_ACCESS_DENIED => "设置迁移临时目录权限被拒绝",
                ERROR_INVALID_HANDLE => "迁移临时目录权限句柄无效",
                ERROR_INVALID_PARAMETER => "迁移临时目录权限参数无效",
                _ => "无法设置迁移临时目录权限",
            };
            return Err(anyhow!(message));
        }
        Ok(())
    }

    fn validate_staged_data(staging: &FixedHandle) -> Result<ValidationGuards> {
        let mut guards = ValidationGuards::default();
        let Some(registry) =
            open_optional_child_fixed(staging, OsStr::new("accounts.json"), FILE_GENERIC_READ)
                .context("旧账户索引校验失败")?
        else {
            return Ok(guards);
        };
        let bytes = read_and_hold(registry, &mut guards, "旧账户索引校验失败")?;
        let registry: AccountRegistry = from_slice(&bytes).context("旧账户索引校验失败")?;
        validate_registry(&registry).context("旧账户索引校验失败")?;
        if registry.accounts.is_empty() {
            return Ok(guards);
        }
        let accounts = open_child_fixed(
            staging,
            OsStr::new("accounts"),
            FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
        )
        .context("旧账户目录校验失败")?;
        require_target_directory(&accounts.information, "旧账户目录校验失败")?;
        guards.directories.push(accounts.handle);

        for entry in registry.accounts {
            let accounts = guards.directories.last().expect("账户目录句柄已固定");
            let account = open_child_fixed(
                accounts,
                OsStr::new(entry.account_id.as_str()),
                FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES,
            )
            .context("旧账户目录校验失败")?;
            require_target_directory(&account.information, "旧账户目录校验失败")?;
            let manifest = open_child_fixed(
                &account.handle,
                OsStr::new("account.json"),
                FILE_GENERIC_READ,
            )
            .context("旧账户清单校验失败")?;
            let bytes = read_and_hold(manifest, &mut guards, "旧账户清单校验失败")?;
            let manifest: AccountManifest = from_slice(&bytes).context("旧账户清单校验失败")?;
            validate_manifest(&manifest, &entry.account_id).context("旧账户清单校验失败")?;
            validate_database_snapshot(&account.handle, &mut guards)?;
            guards.directories.push(account.handle);
        }
        Ok(guards)
    }

    fn validate_database_snapshot(
        account: &FixedHandle,
        guards: &mut ValidationGuards,
    ) -> Result<()> {
        let Some(database) =
            open_optional_child_fixed(account, OsStr::new("owstats.sqlite3"), FILE_GENERIC_READ)
                .context("旧账户数据库校验失败")?
        else {
            return Ok(());
        };
        require_regular_file(&database.information).context("旧账户数据库校验失败")?;

        let snapshot_name = OsString::from(format!(".install-data-validation-{}", Uuid::new_v4()));
        let snapshot =
            create_directory_relative(account, &snapshot_name).context("旧账户数据库校验失败")?;
        let snapshot_path = final_path(&snapshot.handle)?.join("owstats.sqlite3");
        let mut snapshot_files = Vec::new();
        let result = (|| -> Result<()> {
            copy_validation_entry(
                database,
                &snapshot.handle,
                OsStr::new("owstats.sqlite3"),
                guards,
                &mut snapshot_files,
            )?;
            for sidecar in ["owstats.sqlite3-wal", "owstats.sqlite3-shm"] {
                copy_optional_validation_file(
                    account,
                    OsStr::new(sidecar),
                    &snapshot.handle,
                    OsStr::new(sidecar),
                    guards,
                    &mut snapshot_files,
                )?;
            }
            let connection =
                Connection::open_with_flags(&snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .context("旧账户数据库校验失败")?;
            let value: String = connection
                .query_row("PRAGMA quick_check", [], |row| row.get(0))
                .context("旧账户数据库完整性校验失败")?;
            if value != "ok" {
                return Err(anyhow!("旧账户数据库完整性校验失败"));
            }
            Ok(())
        })();
        drop(snapshot_files);
        drop(snapshot.handle);
        delete_tree(snapshot_path.parent().expect("验证目录存在"))
            .context("无法清理数据库验证目录")?;
        result
    }

    fn copy_optional_validation_file(
        source_parent: &FixedHandle,
        source_name: &OsStr,
        target_parent: &FixedHandle,
        target_name: &OsStr,
        guards: &mut ValidationGuards,
        snapshot_files: &mut Vec<File>,
    ) -> Result<()> {
        let Some(source) = open_optional_child_fixed(source_parent, source_name, FILE_GENERIC_READ)
            .context("旧账户数据库校验失败")?
        else {
            return Ok(());
        };
        copy_validation_entry(source, target_parent, target_name, guards, snapshot_files)
    }

    fn copy_validation_entry(
        source: FixedEntry,
        target_parent: &FixedHandle,
        target_name: &OsStr,
        guards: &mut ValidationGuards,
        snapshot_files: &mut Vec<File>,
    ) -> Result<()> {
        require_regular_file(&source.information).context("旧账户数据库校验失败")?;
        let mut source = source.handle.into_file();
        let target_path = final_path(target_parent)?.join(target_name);
        let target = create_file(&target_path).context("旧账户数据库校验失败")?;
        let mut target = target.handle.into_file();
        std::io::copy(&mut source, &mut target).context("旧账户数据库校验失败")?;
        target.sync_all().context("旧账户数据库校验失败")?;
        guards.files.push(source);
        snapshot_files.push(target);
        Ok(())
    }

    fn read_and_hold(
        entry: FixedEntry,
        guards: &mut ValidationGuards,
        message: &'static str,
    ) -> Result<Vec<u8>> {
        require_regular_file(&entry.information).context(message)?;
        let mut file = entry.handle.into_file();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).context(message)?;
        guards.files.push(file);
        Ok(bytes)
    }

    fn copy_tree(source: &Path, target: &Path, target_root: &FixedHandle) -> Result<()> {
        let source_entry =
            open_source_fixed(source, FILE_GENERIC_READ).context("无法固定迁移源根目录")?;
        require_directory(&source_entry.information)?;
        let limits = COPY_LIMITS.with(|value| value.get().unwrap_or(DEFAULT_COPY_LIMITS));
        let mut budget = CopyBudget::default();
        copy_directory(
            source,
            source_entry.handle,
            target,
            target_root,
            0,
            &mut budget,
            limits,
        )
    }

    fn copy_directory(
        source: &Path,
        source_guard: FixedHandle,
        target: &Path,
        _target_guard: &FixedHandle,
        depth: usize,
        budget: &mut CopyBudget,
        limits: WindowsCopyLimits,
    ) -> Result<()> {
        let _source_guard = source_guard;

        for entry in fs::read_dir(source).context("无法枚举迁移源")? {
            let entry = entry.context("无法读取迁移目录项")?;
            invoke_copy_enumeration_hook();
            budget.record_entry(limits)?;
            let source_child = entry.path();
            let target_child = target.join(entry.file_name());
            invoke_copy_open_hook(&source_child);
            let source_entry = open_source_fixed(&source_child, FILE_GENERIC_READ)?;
            if is_reparse(&source_entry.information) {
                return Err(anyhow!("迁移源包含不受支持的链接"));
            }
            if is_directory(&source_entry.information) {
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("迁移目录层级超过安全限制"))?;
                if child_depth > limits.max_depth {
                    return Err(anyhow!("迁移目录层级超过安全限制"));
                }
                create_directory(&target_child)?;
                let target_entry = open_fixed(&target_child, FILE_GENERIC_READ)?;
                require_directory(&target_entry.information)?;
                copy_directory(
                    &source_child,
                    source_entry.handle,
                    &target_child,
                    &target_entry.handle,
                    child_depth,
                    budget,
                    limits,
                )?;
                continue;
            }

            require_regular_file(&source_entry.information)?;
            let expected_size = file_size(&source_entry.information);
            let target_entry = create_file(&target_child)?;
            copy_open_file(
                source_entry.handle,
                target_entry.handle,
                expected_size,
                budget,
                limits,
            )?;
        }
        Ok(())
    }

    fn child_name(path: &Path) -> Result<&OsStr> {
        path.file_name()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| anyhow!("迁移目录名称无效"))
    }

    fn encode_child_name(name: &OsStr) -> Result<Vec<u16>> {
        let encoded = name.encode_wide().collect::<Vec<_>>();
        if encoded.is_empty() || encoded.contains(&0) {
            return Err(anyhow!("迁移目录名称无效"));
        }
        Ok(encoded)
    }

    fn create_protected_directory_relative(
        parent: &FixedHandle,
        name: &OsStr,
    ) -> Result<FixedEntry> {
        let descriptor = protected_staging_security_descriptor()?;
        create_directory_relative_with_security(parent, name, descriptor.0)
    }

    fn create_directory_relative(parent: &FixedHandle, name: &OsStr) -> Result<FixedEntry> {
        create_directory_relative_with_security(parent, name, std::ptr::null_mut())
    }

    fn create_directory_relative_with_security(
        parent: &FixedHandle,
        name: &OsStr,
        security_descriptor: PSECURITY_DESCRIPTOR,
    ) -> Result<FixedEntry> {
        const FILE_CREATED_INFORMATION: usize = 2;

        let mut encoded = encode_child_name(name)?;
        let name_bytes = encoded
            .len()
            .checked_mul(size_of::<u16>())
            .ok_or_else(|| anyhow!("迁移目录名称过长"))?;
        let name_length = u16::try_from(name_bytes).map_err(|_| anyhow!("迁移目录名称过长"))?;
        let unicode_name = UNICODE_STRING {
            Length: name_length,
            MaximumLength: name_length,
            Buffer: encoded.as_mut_ptr(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.0,
            ObjectName: &unicode_name,
            Attributes: (OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE) as u32,
            SecurityDescriptor: security_descriptor,
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut io_status = unsafe { std::mem::zeroed::<IO_STATUS_BLOCK>() };
        let mut raw_handle: HANDLE = std::ptr::null_mut();
        // FILE_CREATE makes creation and handle acquisition one kernel operation.
        // A competing object at this basename causes failure instead of being opened.
        let status = unsafe {
            NtCreateFile(
                &mut raw_handle,
                FILE_LIST_DIRECTORY
                    | FILE_TRAVERSE
                    | FILE_READ_ATTRIBUTES
                    | SYNCHRONIZE
                    | DELETE
                    | READ_CONTROL
                    | WRITE_DAC,
                &attributes,
                &mut io_status,
                std::ptr::null(),
                FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_CREATE,
                FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if status < 0 || raw_handle.is_null() || raw_handle == INVALID_HANDLE_VALUE {
            return Err(anyhow!("无法安全创建迁移目录"));
        }

        let handle = FixedHandle(raw_handle);
        if io_status.Information != FILE_CREATED_INFORMATION {
            let _ = mark_for_delete(handle);
            return Err(anyhow!("无法排他创建迁移目录"));
        }
        let entry_result = (|| {
            let information = information(&handle)?;
            if get_file_type_safe(&handle)? != FILE_TYPE_DISK {
                return Err(anyhow!("迁移目录包含不受支持的文件类型"));
            }
            require_target_directory(&information, "迁移目录无效")?;
            Ok(information)
        })();
        match entry_result {
            Ok(information) => Ok(FixedEntry {
                handle,
                information,
            }),
            Err(error) => {
                let _ = mark_for_delete(handle);
                Err(error)
            }
        }
    }

    fn write_marker(path: &Path, contents: &[u8]) -> Result<()> {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                let entry = open_fixed(path, FILE_GENERIC_WRITE | DELETE)
                    .context("无法安全打开迁移完成标记")?;
                require_regular_file(&entry.information).context("迁移完成标记类型无效")?;
                mark_for_delete(entry.handle).context("无法重置迁移完成标记")?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("无法读取迁移完成标记"),
        }

        let entry = create_file(path).context("无法创建迁移完成标记")?;
        let mut file = entry.handle.into_file();
        file.write_all(contents).context("无法写入迁移完成标记")?;
        file.sync_all().context("无法写入迁移完成标记")
    }

    fn final_path(handle: &FixedHandle) -> Result<PathBuf> {
        let flags = FILE_NAME_NORMALIZED | VOLUME_NAME_DOS;
        let required =
            unsafe { GetFinalPathNameByHandleW(handle.0, std::ptr::null_mut(), 0, flags) };
        if required == 0 {
            return Err(anyhow!("无法定位迁移临时目录的父目录"));
        }
        let capacity = required
            .checked_add(1)
            .ok_or_else(|| anyhow!("迁移临时目录的父目录路径过长"))?;
        let mut buffer = vec![0_u16; capacity as usize];
        let length =
            unsafe { GetFinalPathNameByHandleW(handle.0, buffer.as_mut_ptr(), capacity, flags) };
        if length == 0 || length >= capacity {
            return Err(anyhow!("无法定位迁移临时目录的父目录"));
        }
        buffer.truncate(length as usize);
        Ok(PathBuf::from(OsString::from_wide(&buffer)))
    }

    fn rename_without_replace(
        staging: &FixedHandle,
        parent_guard: &FixedHandle,
        destination: &Path,
    ) -> Result<()> {
        let _keep_parent_fixed = parent_guard;
        let encoded = destination.as_os_str().encode_wide().collect::<Vec<_>>();
        if encoded.is_empty() || encoded.contains(&0) {
            return Err(anyhow!("安装数据目录名称无效"));
        }
        let name_bytes = encoded
            .len()
            .checked_mul(size_of::<u16>())
            .ok_or_else(|| anyhow!("安装数据目录名称过长"))?;
        let name_length = u32::try_from(name_bytes).map_err(|_| anyhow!("安装数据目录名称过长"))?;
        let buffer_size = size_of::<FILE_RENAME_INFO>()
            .checked_add(name_bytes)
            .ok_or_else(|| anyhow!("安装数据目录名称过长"))?;
        let storage_words = buffer_size.div_ceil(size_of::<usize>());
        // FILE_RENAME_INFO requires native alignment and a trailing variable-size
        // UTF-16 name. usize storage provides alignment on both Windows targets.
        let mut storage = vec![0_usize; storage_words];
        let information = storage.as_mut_ptr() as *mut FILE_RENAME_INFO;
        unsafe {
            (*information).Anonymous.ReplaceIfExists = 0;
            (*information).RootDirectory = std::ptr::null_mut();
            (*information).FileNameLength = name_length;
            std::ptr::copy_nonoverlapping(
                encoded.as_ptr(),
                (information as *mut u8).add(offset_of!(FILE_RENAME_INFO, FileName)) as *mut u16,
                encoded.len(),
            );
        }
        let buffer_size =
            u32::try_from(buffer_size).map_err(|_| anyhow!("安装数据目录名称过长"))?;
        let renamed = unsafe {
            SetFileInformationByHandle(staging.0, FileRenameInfo, information as _, buffer_size)
        };
        if renamed == 0 {
            return Err(anyhow!("无法发布安装数据目录"));
        }
        Ok(())
    }

    fn reset_staging(staging: &Path) -> Result<()> {
        match fs::symlink_metadata(staging) {
            Ok(_) => delete_tree(staging).context("无法重置迁移临时目录"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("无法读取迁移临时目录"),
        }
    }

    fn delete_tree(path: &Path) -> Result<()> {
        let root = open_fixed(path, FILE_GENERIC_READ | DELETE)?;
        if is_reparse(&root.information) {
            return Err(anyhow!("迁移临时目录包含不受支持的链接"));
        }
        if !is_directory(&root.information) {
            return Err(anyhow!("迁移临时目录包含不受支持的文件类型"));
        }

        let limits = CLEANUP_LIMITS.with(|value| value.get().unwrap_or(DEFAULT_CLEANUP_LIMITS));
        let mut budget = CleanupBudget::default();
        let mut frames = vec![DeleteFrame {
            handle: root.handle,
            entries: fs::read_dir(path).context("无法枚举迁移临时目录")?,
            depth: 0,
        }];

        while !frames.is_empty() {
            let next = frames
                .last_mut()
                .expect("清理栈已检查为非空")
                .entries
                .next();
            match next {
                Some(child) => {
                    let child = child.context("无法读取迁移临时目录项")?;
                    invoke_cleanup_enumeration_hook();
                    budget.record_entry(limits)?;
                    let child_path = child.path();
                    let child = open_fixed(&child_path, FILE_GENERIC_READ | DELETE)?;
                    if is_reparse(&child.information) {
                        return Err(anyhow!("迁移临时目录包含不受支持的链接"));
                    }
                    if is_directory(&child.information) {
                        let child_depth = frames
                            .last()
                            .expect("清理栈已检查为非空")
                            .depth
                            .checked_add(1)
                            .ok_or_else(|| anyhow!("迁移临时目录层级超过安全限制"))?;
                        if child_depth > limits.max_depth {
                            return Err(anyhow!("迁移临时目录层级超过安全限制"));
                        }
                        frames.push(DeleteFrame {
                            handle: child.handle,
                            entries: fs::read_dir(&child_path).context("无法枚举迁移临时目录")?,
                            depth: child_depth,
                        });
                    } else {
                        require_regular_file(&child.information)?;
                        mark_for_delete(child.handle)?;
                    }
                }
                None => {
                    let DeleteFrame {
                        handle, entries, ..
                    } = frames.pop().expect("清理栈已检查为非空");
                    drop(entries);
                    mark_for_delete(handle)?;
                }
            }
        }
        Ok(())
    }

    fn open_fixed(path: &Path, access: u32) -> Result<FixedEntry> {
        open_fixed_with_share(path, access, FILE_SHARE_READ | FILE_SHARE_WRITE)
    }

    fn open_source_fixed(path: &Path, access: u32) -> Result<FixedEntry> {
        open_fixed_with_share(path, access, FILE_SHARE_READ)
    }

    fn open_child_fixed(parent: &FixedHandle, name: &OsStr, access: u32) -> Result<FixedEntry> {
        open_optional_child_fixed(parent, name, access)?
            .ok_or_else(|| anyhow!("无法安全打开迁移文件"))
    }

    fn open_optional_child_fixed(
        parent: &FixedHandle,
        name: &OsStr,
        access: u32,
    ) -> Result<Option<FixedEntry>> {
        let mut encoded = encode_child_name(name)?;
        let byte_len = u16::try_from(
            encoded
                .len()
                .checked_mul(size_of::<u16>())
                .ok_or_else(|| anyhow!("账户文件名无效"))?,
        )
        .map_err(|_| anyhow!("账户文件名无效"))?;
        let unicode_name = UNICODE_STRING {
            Length: byte_len,
            MaximumLength: byte_len,
            Buffer: encoded.as_mut_ptr(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.0,
            ObjectName: &unicode_name,
            Attributes: (OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE) as u32,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut status = unsafe { std::mem::zeroed::<IO_STATUS_BLOCK>() };
        let mut raw: HANDLE = std::ptr::null_mut();
        let result = unsafe {
            NtCreateFile(
                &mut raw,
                access | SYNCHRONIZE,
                &attributes,
                &mut status,
                std::ptr::null(),
                FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_READ,
                FILE_OPEN,
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if result < 0 {
            if !raw.is_null() && raw != INVALID_HANDLE_VALUE {
                unsafe {
                    let _ = CloseHandle(raw);
                }
            }
            if result == STATUS_OBJECT_NAME_NOT_FOUND || result == STATUS_OBJECT_PATH_NOT_FOUND {
                return Ok(None);
            }
            return Err(anyhow!("无法安全打开迁移文件"));
        }
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(anyhow!("无法安全打开迁移文件"));
        }
        let handle = FixedHandle(raw);
        let information = information(&handle)?;
        if get_file_type_safe(&handle)? != FILE_TYPE_DISK {
            return Err(anyhow!("账户文件类型无效"));
        }
        Ok(Some(FixedEntry {
            handle,
            information,
        }))
    }

    fn open_fixed_with_share(path: &Path, access: u32, share_mode: u32) -> Result<FixedEntry> {
        let wide = wide(path);
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                access,
                share_mode,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(anyhow!("无法安全打开迁移文件"));
        }
        let handle = FixedHandle(handle);
        let information = information(&handle)?;
        if get_file_type_safe(&handle)? != FILE_TYPE_DISK {
            return Err(anyhow!("迁移源包含不受支持的文件类型"));
        }
        Ok(FixedEntry {
            handle,
            information,
        })
    }

    fn create_directory(path: &Path) -> Result<()> {
        fs::create_dir(path).context("无法创建迁移目录")
    }

    fn create_file(path: &Path) -> Result<FixedEntry> {
        let wide = wide(path);
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(anyhow!("无法创建迁移文件"));
        }
        let handle = FixedHandle(handle);
        let information = information(&handle)?;
        require_regular_file(&information)?;
        Ok(FixedEntry {
            handle,
            information,
        })
    }

    fn copy_open_file(
        source: FixedHandle,
        target: FixedHandle,
        expected_size: u64,
        budget: &mut CopyBudget,
        limits: WindowsCopyLimits,
    ) -> Result<()> {
        let mut source = source.into_file();
        let mut target = target.into_file();
        copy_reader_to_writer(&mut source, &mut target, expected_size, budget, limits)?;
        target.sync_all().context("无法写入迁移文件")
    }

    fn copy_reader_to_writer<R: Read, W: Write>(
        source: &mut R,
        target: &mut W,
        expected_size: u64,
        budget: &mut CopyBudget,
        limits: WindowsCopyLimits,
    ) -> Result<()> {
        let mut buffer = [0_u8; 64 * 1024];
        let mut copied = 0_u64;
        loop {
            let read = source.read(&mut buffer).context("无法读取迁移文件")?;
            if read == 0 {
                break;
            }
            budget.record_bytes(read, limits)?;
            copied = copied
                .checked_add(u64::try_from(read).map_err(|_| anyhow!("迁移数据大小超过安全限制"))?)
                .ok_or_else(|| anyhow!("迁移数据大小超过安全限制"))?;
            if copied > expected_size {
                return Err(anyhow!("迁移源在复制期间发生变化"));
            }
            target
                .write_all(&buffer[..read])
                .context("无法写入迁移文件")?;
        }
        if copied != expected_size {
            return Err(anyhow!("迁移源在复制期间发生变化"));
        }
        Ok(())
    }

    fn mark_for_delete(handle: FixedHandle) -> Result<()> {
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: 1 };
        let result = unsafe {
            SetFileInformationByHandle(
                handle.0,
                FileDispositionInfo,
                &disposition as *const _ as _,
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if result == 0 {
            return Err(anyhow!("无法删除迁移临时目录"));
        }
        drop(handle);
        Ok(())
    }

    fn information(handle: &FixedHandle) -> Result<BY_HANDLE_FILE_INFORMATION> {
        let mut information = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
        if unsafe { GetFileInformationByHandle(handle.0, &mut information) } == 0 {
            return Err(anyhow!("无法读取迁移文件属性"));
        }
        Ok(information)
    }

    fn get_file_type_safe(handle: &FixedHandle) -> Result<u32> {
        let file_type = unsafe { GetFileType(handle.0) };
        if file_type == 0 {
            return Err(anyhow!("无法读取迁移文件类型"));
        }
        Ok(file_type)
    }

    fn require_directory(information: &BY_HANDLE_FILE_INFORMATION) -> Result<()> {
        if is_reparse(information) {
            return Err(anyhow!("迁移源包含不受支持的链接"));
        }
        if !is_directory(information) {
            return Err(anyhow!("迁移源包含不受支持的文件类型"));
        }
        Ok(())
    }

    fn require_target_directory(
        information: &BY_HANDLE_FILE_INFORMATION,
        message: &'static str,
    ) -> Result<()> {
        if is_reparse(information) || !is_directory(information) {
            return Err(anyhow!(message));
        }
        Ok(())
    }

    fn require_regular_file(information: &BY_HANDLE_FILE_INFORMATION) -> Result<()> {
        if is_reparse(information) || is_directory(information) || information.nNumberOfLinks != 1 {
            return Err(anyhow!("迁移源包含不受支持的文件类型"));
        }
        Ok(())
    }

    fn is_directory(information: &BY_HANDLE_FILE_INFORMATION) -> bool {
        information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0
    }

    fn is_reparse(information: &BY_HANDLE_FILE_INFORMATION) -> bool {
        information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    fn file_size(information: &BY_HANDLE_FILE_INFORMATION) -> u64 {
        ((information.nFileSizeHigh as u64) << 32) | information.nFileSizeLow as u64
    }

    fn wide(path: &Path) -> Vec<u16> {
        OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use std::{
            collections::VecDeque,
            io::{self, Read},
        };

        use super::*;

        struct ChunkedReader {
            chunks: VecDeque<Vec<u8>>,
        }

        impl Read for ChunkedReader {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let Some(chunk) = self.chunks.pop_front() else {
                    return Ok(0);
                };
                assert!(chunk.len() <= buffer.len());
                buffer[..chunk.len()].copy_from_slice(&chunk);
                Ok(chunk.len())
            }
        }

        #[test]
        fn rejects_growth_over_the_actual_byte_budget_before_writing_the_excess_chunk() {
            let mut source = ChunkedReader {
                chunks: VecDeque::from([b"12".to_vec(), b"345".to_vec()]),
            };
            let mut target = Vec::new();
            let mut budget = CopyBudget::default();
            let limits = WindowsCopyLimits::new(64, 8, 4);

            let result = copy_reader_to_writer(&mut source, &mut target, 3, &mut budget, limits);

            assert!(result.is_err());
            assert_eq!(target, b"12");
            assert_eq!(budget.bytes, 2);
        }
    }
}
