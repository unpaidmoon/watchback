use std::{
    ffi::OsString,
    fs::{self, Metadata},
    path::{Component, Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::app_paths::SharedAppPaths;

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

    let Some(source) = source else {
        return create_empty_data_directory(destination);
    };

    reset_staging(&staging)?;
    fs::create_dir_all(
        staging
            .parent()
            .ok_or_else(|| anyhow!("无法定位迁移临时目录"))?,
    )
    .context("无法创建迁移临时目录的父目录")?;
    copy_tree(&source, &staging)?;
    validate_staged_data(&staging)?;
    let marker = serde_json::to_vec_pretty(&MigrationMarker {
        version: 1,
        source_retained: true,
    })?;
    fs::write(staging.join(".install-data-migration-v1.json"), marker)
        .context("无法写入迁移完成标记")?;
    fs::rename(&staging, destination.root()).context("无法发布安装数据目录")?;
    Ok(InstallDataMigrationOutcome::Migrated)
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

fn create_empty_data_directory(
    destination: &SharedAppPaths,
) -> Result<InstallDataMigrationOutcome> {
    fs::create_dir_all(destination.root()).context("无法创建安装数据目录")?;
    Ok(InstallDataMigrationOutcome::Created)
}

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        return windows_secure::copy_tree(source, target);
    }

    #[cfg(not(windows))]
    copy_tree_by_path(source, target)
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
        copy_tree(&entry.path(), &target.join(entry.file_name()))?;
    }
    Ok(())
}

fn reset_staging(staging: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        return windows_secure::reset_staging(staging);
    }

    #[cfg(not(windows))]
    reset_staging_by_path(staging)
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
    if is_supported_directory(&metadata) {
        Ok(())
    } else {
        Err(anyhow!("迁移临时目录无效"))
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
mod windows_secure {
    use std::{
        ffi::OsStr,
        fs::{self, File},
        io::{Read, Write},
        mem::size_of,
        os::windows::{ffi::OsStrExt, io::FromRawHandle},
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    use anyhow::{anyhow, Context, Result};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, FileDispositionInfo, GetFileInformationByHandle, GetFileType,
            SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, DELETE,
            FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
            FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            FILE_TYPE_DISK, OPEN_EXISTING,
        },
    };

    const MAX_DEPTH: usize = 64;
    const MAX_FILES: u64 = 100_000;
    const MAX_TOTAL_BYTES: u64 = 20 * 1024 * 1024 * 1024;

    static COPY_OPEN_HOOK: OnceLock<Mutex<Option<fn(&Path)>>> = OnceLock::new();

    pub(super) fn set_copy_open_hook(hook: Option<fn(&Path)>) {
        *COPY_OPEN_HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("迁移测试钩子锁不可用") = hook;
    }

    fn invoke_copy_open_hook(path: &Path) {
        let hook = *COPY_OPEN_HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("迁移测试钩子锁不可用");
        if let Some(hook) = hook {
            hook(path);
        }
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

    struct DirectoryJob {
        source: PathBuf,
        source_handle: FixedHandle,
        target: PathBuf,
        target_handle: FixedHandle,
        depth: usize,
    }

    #[derive(Default)]
    struct CopyBudget {
        files: u64,
        bytes: u64,
    }

    pub(super) fn copy_tree(source: &Path, target: &Path) -> Result<()> {
        create_directory(target)?;
        let source_entry = open_fixed(source, FILE_GENERIC_READ).context("无法固定迁移源根目录")?;
        require_directory(&source_entry.information)?;
        let target_entry = open_fixed(target, FILE_GENERIC_READ).context("无法固定迁移临时目录")?;
        require_directory(&target_entry.information)?;

        let mut jobs = vec![DirectoryJob {
            source: source.to_path_buf(),
            source_handle: source_entry.handle,
            target: target.to_path_buf(),
            target_handle: target_entry.handle,
            depth: 0,
        }];
        let mut budget = CopyBudget::default();

        while let Some(job) = jobs.pop() {
            let _source_lock = job.source_handle;
            let _target_lock = job.target_handle;
            if job.depth >= MAX_DEPTH {
                return Err(anyhow!("迁移目录层级超过安全限制"));
            }

            let mut entries = Vec::new();
            for entry in fs::read_dir(&job.source).context("无法枚举迁移源")? {
                entries.push(entry.context("无法读取迁移目录项")?);
            }
            entries.sort_by_key(|entry| entry.file_name());

            for entry in entries {
                let source_child = entry.path();
                let target_child = job.target.join(entry.file_name());
                invoke_copy_open_hook(&source_child);
                let source_entry = open_fixed(&source_child, FILE_GENERIC_READ)?;
                if is_reparse(&source_entry.information) {
                    return Err(anyhow!("迁移源包含不受支持的链接"));
                }
                if is_directory(&source_entry.information) {
                    create_directory(&target_child)?;
                    let target_entry = open_fixed(&target_child, FILE_GENERIC_READ)?;
                    require_directory(&target_entry.information)?;
                    jobs.push(DirectoryJob {
                        source: source_child,
                        source_handle: source_entry.handle,
                        target: target_child,
                        target_handle: target_entry.handle,
                        depth: job.depth + 1,
                    });
                    continue;
                }

                require_regular_file(&source_entry.information)?;
                budget.files = budget
                    .files
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("迁移文件数量超过安全限制"))?;
                if budget.files > MAX_FILES {
                    return Err(anyhow!("迁移文件数量超过安全限制"));
                }
                let size = file_size(&source_entry.information);
                budget.bytes = budget
                    .bytes
                    .checked_add(size)
                    .ok_or_else(|| anyhow!("迁移数据大小超过安全限制"))?;
                if budget.bytes > MAX_TOTAL_BYTES {
                    return Err(anyhow!("迁移数据大小超过安全限制"));
                }

                let target_entry = create_file(&target_child)?;
                copy_open_file(source_entry.handle, target_entry.handle)?;
            }
        }
        Ok(())
    }

    pub(super) fn reset_staging(staging: &Path) -> Result<()> {
        match fs::symlink_metadata(staging) {
            Ok(_) => delete_tree(staging).context("无法重置迁移临时目录"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("无法读取迁移临时目录"),
        }
    }

    fn delete_tree(path: &Path) -> Result<()> {
        let entry = open_fixed(path, FILE_GENERIC_READ | DELETE)?;
        if is_reparse(&entry.information) {
            return Err(anyhow!("迁移临时目录包含不受支持的链接"));
        }
        if !is_directory(&entry.information) {
            return Err(anyhow!("迁移临时目录包含不受支持的文件类型"));
        }

        let mut children = Vec::new();
        for child in fs::read_dir(path).context("无法枚举迁移临时目录")? {
            children.push(child.context("无法读取迁移临时目录项")?.path());
        }
        children.sort();
        for child in children {
            delete_entry(&child)?;
        }
        mark_for_delete(entry.handle)
    }

    fn delete_entry(path: &Path) -> Result<()> {
        let entry = open_fixed(path, FILE_GENERIC_READ | DELETE)?;
        if is_reparse(&entry.information) {
            return Err(anyhow!("迁移临时目录包含不受支持的链接"));
        }
        if is_directory(&entry.information) {
            let mut children = Vec::new();
            for child in fs::read_dir(path).context("无法枚举迁移临时目录")? {
                children.push(child.context("无法读取迁移临时目录项")?.path());
            }
            children.sort();
            for child in children {
                delete_entry(&child)?;
            }
        } else {
            require_regular_file(&entry.information)?;
        }
        mark_for_delete(entry.handle)
    }

    fn open_fixed(path: &Path, access: u32) -> Result<FixedEntry> {
        let wide = wide(path);
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
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
                1,
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

    fn copy_open_file(source: FixedHandle, target: FixedHandle) -> Result<()> {
        let mut source = source.into_file();
        let mut target = target.into_file();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = source.read(&mut buffer).context("无法读取迁移文件")?;
            if read == 0 {
                break;
            }
            target
                .write_all(&buffer[..read])
                .context("无法写入迁移文件")?;
        }
        target.sync_all().context("无法写入迁移文件")
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
}
