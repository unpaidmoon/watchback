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
    match fs::symlink_metadata(staging) {
        Ok(_) => remove_tree_without_links(staging).context("无法重置迁移临时目录"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("无法读取迁移临时目录"),
    }
}

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
