#![allow(
    clippy::items_after_test_module,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

pub mod account_context;
pub mod account_login;
pub mod account_migration;
pub mod account_removal;
pub mod account_store;
pub mod account_sync_identity;
pub mod ai_roast;
mod analytics;
pub mod app_paths;
mod auto_season_update;
mod background_sync;
pub mod browser_session;
mod commands;
pub mod credential_store;
pub mod credentials;
pub mod dashen_auth;
pub mod dashen_client;
mod database;
mod db;
pub mod deepseek_key;
mod diagnostics;
mod importer;
pub mod install_data_migration;
mod live_season_update;
pub mod login_flow;
mod match_evaluation;
mod match_favorites;
mod models;
pub mod player_query;
pub mod player_target;
mod profile_card;
pub mod qr_login;
mod queries;
pub mod ranking;
mod review;
pub mod runtime_config;
pub mod season_fetch;
mod settings;
mod sync;
mod sync_coordinator;
pub mod update_check;

pub use ai_roast::coordinator::AiRoastGenerationResult;
pub use analytics::{get_friend_analytics, get_friend_detail, get_season_analysis};
pub use background_sync::{BackgroundEventSink, BackgroundSeasonSyncManager};
pub use commands::runtime_error_message;
pub use db::OwDatabase;
pub use importer::{
    import_full_season_payload, import_incremental_payload_value_embedded, import_payload_value,
    import_payload_value_embedded,
};
pub use match_evaluation::{refresh_match_evaluations, PHASE5_SCORING_RULE_VERSION};
pub use match_favorites::{delete_match_favorite, get_match_favorite, save_match_favorite};
pub use models::*;
pub use queries::{
    get_match_detail, list_filter_options, list_matches, list_matches_page, search_match_players,
};
pub use review::{get_career_achievements, get_onboarding_status, get_personal_season_review};
#[doc(hidden)]
pub use sync::write_season_payload;
#[doc(hidden)]
pub use sync::{
    benchmark_snapshot, rebuild_complete_season_payload, rebuild_snapshot_for_revision,
    rebuild_snapshot_for_revision_with_hook, SnapshotBenchmark, SnapshotOutcome,
};

pub fn initialize_log_file(paths: &app_paths::SharedAppPaths) -> anyhow::Result<()> {
    diagnostics::ensure_log_file(&paths.log_file())
}
pub use sync::{
    build_page_diagnostics, classify_quick_page, commit_quick_list_probe, dump_season_json,
    dump_season_json_with_base_url, load_season_sync_inventory,
    quick_list_update_with_credentials_and_target_paths_base_url,
    quick_list_update_with_credentials_and_target_paths_base_url_observed, run_background_job_once,
    select_detail_entries, sync_import_parts, sync_season, sync_season_with_base_url,
    sync_season_with_credentials_and_target_paths_base_url,
    sync_season_with_credentials_and_target_paths_output_dir_base_url,
    sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url,
    sync_season_with_credentials_and_target_paths_with_mode_base_url,
    sync_season_with_credentials_path_base_url, sync_season_with_credentials_path_progress,
    sync_season_with_current_account_credentials_path_output_dir_base_url,
    sync_season_with_current_account_credentials_path_output_dir_with_mode_base_url,
    sync_season_with_progress, BackgroundRunSummary, ClaimedSeasonJob, DetailSelection,
    ModeSyncInventory, ModeWork, QuickPageDecision, RequestShape, SeasonBackgroundStatus,
    SeasonModeKey, SeasonSyncInventory, SeasonSyncStore,
};

#[derive(Debug, PartialEq, Eq)]
pub enum StartupMode {
    Desktop,
    PrepareInstallData {
        source: std::option::Option<std::path::PathBuf>,
        destination: std::path::PathBuf,
    },
    DeleteInstallData {
        destination: std::path::PathBuf,
    },
    ProbeInstallData {
        destination: std::path::PathBuf,
    },
    PrepareInstallRoot {
        install_root: std::path::PathBuf,
        user_sid: std::ffi::OsString,
    },
    SecureInstallData {
        source: std::option::Option<std::path::PathBuf>,
        destination: std::path::PathBuf,
        user_sid: std::ffi::OsString,
    },
    TrustedDeleteInstallData {
        install_root: std::path::PathBuf,
        destination: std::path::PathBuf,
    },
    VerifyUpgradeRoot {
        install_root: std::path::PathBuf,
        user_sid: std::ffi::OsString,
    },
}

pub fn parse_startup_mode<I, S>(args: I) -> anyhow::Result<StartupMode>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let Some(mode) = args.next() else {
        return Ok(StartupMode::Desktop);
    };
    if mode == "--prepare-install-root" {
        let install_root = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少安装根目录参数"))?;
        let user_sid = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少用户 SID 参数"))?;
        if args.next().is_some() {
            return Err(anyhow::anyhow!("安装根目录参数过多"));
        }
        return Ok(StartupMode::PrepareInstallRoot {
            install_root: install_root.into(),
            user_sid,
        });
    }
    if mode == "--verify-upgrade-root" {
        let install_root = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少旧安装根目录参数"))?;
        let user_sid = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少用户 SID 参数"))?;
        if args.next().is_some() {
            return Err(anyhow::anyhow!("旧安装根目录校验参数过多"));
        }
        return Ok(StartupMode::VerifyUpgradeRoot {
            install_root: install_root.into(),
            user_sid,
        });
    }
    if mode == "--secure-install-data" {
        let source = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少迁移源参数"))?;
        let destination = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少迁移目标参数"))?;
        let user_sid = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少用户 SID 参数"))?;
        if args.next().is_some() {
            return Err(anyhow::anyhow!("安全迁移参数过多"));
        }
        return Ok(StartupMode::SecureInstallData {
            source: if source == "-" {
                None
            } else {
                Some(source.into())
            },
            destination: destination.into(),
            user_sid,
        });
    }
    if mode == "--trusted-delete-install-data" {
        let install_root = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少安装根目录参数"))?;
        let destination = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少删除目标参数"))?;
        if args.next().is_some() {
            return Err(anyhow::anyhow!("安全删除参数过多"));
        }
        return Ok(StartupMode::TrustedDeleteInstallData {
            install_root: install_root.into(),
            destination: destination.into(),
        });
    }
    if mode == "--delete-install-data" {
        let destination = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少删除目标参数"))?;
        if args.next().is_some() {
            return Err(anyhow::anyhow!("删除参数过多"));
        }
        return Ok(StartupMode::DeleteInstallData {
            destination: destination.into(),
        });
    }
    if mode == "--probe-install-data" {
        let destination = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("缺少写入探针目标参数"))?;
        if args.next().is_some() {
            return Err(anyhow::anyhow!("写入探针参数过多"));
        }
        return Ok(StartupMode::ProbeInstallData {
            destination: destination.into(),
        });
    }
    if mode != "--prepare-install-data" {
        return Err(anyhow::anyhow!("不支持的启动参数"));
    }

    let source = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("缺少迁移源参数"))?;
    let destination = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("缺少迁移目标参数"))?;
    if args.next().is_some() {
        return Err(anyhow::anyhow!("迁移参数过多"));
    }

    Ok(StartupMode::PrepareInstallData {
        source: if source == "-" {
            None
        } else {
            Some(source.into())
        },
        destination: destination.into(),
    })
}

pub fn run_prepare_install_root_mode(
    install_root: &std::path::Path,
    user_sid: &std::ffi::OsStr,
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        if !strict_absolute_path(install_root) {
            return Err(anyhow::anyhow!("安装根目录无效"));
        }
        install_data_migration::prepare_install_root(install_root, user_sid)
    }
    #[cfg(not(windows))]
    {
        let _ = (install_root, user_sid);
        Err(anyhow::anyhow!("当前平台不支持安装根目录准备命令"))
    }
}

pub fn run_verify_upgrade_root_mode(
    install_root: &std::path::Path,
    user_sid: &std::ffi::OsStr,
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        if !strict_absolute_path(install_root) {
            return Err(anyhow::anyhow!("旧安装根目录无效"));
        }
        install_data_migration::verify_upgrade_root(install_root, user_sid)
    }
    #[cfg(not(windows))]
    {
        let _ = (install_root, user_sid);
        Err(anyhow::anyhow!("当前平台不支持旧安装根目录校验"))
    }
}

pub fn run_secure_install_data_mode(
    source: Option<&std::path::Path>,
    destination: &std::path::Path,
    user_sid: &std::ffi::OsStr,
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        if !strict_absolute_path(destination) || !is_data_directory_name(destination) {
            return Err(anyhow::anyhow!("安全迁移目标无效"));
        }
        let parent = destination
            .parent()
            .ok_or_else(|| anyhow::anyhow!("安全迁移目标无效"))?;
        let install_parent = install_data_migration::open_install_parent(parent)?;
        if !install_data_migration::destination_belongs_to_install_parent(
            &install_parent,
            destination,
        )? {
            return Err(anyhow::anyhow!("安全迁移目标无效"));
        }
        install_data_migration::prepare_install_data_in_install_parent(source, &install_parent)?;
        install_data_migration::protect_install_data_for_user(&install_parent, user_sid)?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (source, destination, user_sid);
        Err(anyhow::anyhow!("当前平台不支持安全安装数据命令"))
    }
}

pub fn run_trusted_delete_install_data_mode(
    install_root: &std::path::Path,
    destination: &std::path::Path,
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        if !strict_absolute_path(install_root)
            || !strict_absolute_path(destination)
            || !is_data_directory_name(destination)
        {
            return Err(anyhow::anyhow!("安全删除目标无效"));
        }
        let install_parent = install_data_migration::open_install_parent(install_root)?;
        if !install_data_migration::destination_belongs_to_install_parent(
            &install_parent,
            destination,
        )? {
            return Err(anyhow::anyhow!("安全删除目标无效"));
        }
        install_data_migration::delete_install_data_in_install_parent(&install_parent)
    }
    #[cfg(not(windows))]
    {
        let _ = (install_root, destination);
        Err(anyhow::anyhow!("当前平台不支持安全删除命令"))
    }
}

pub fn run_delete_install_data_mode(
    executable: &std::path::Path,
    destination: &std::path::Path,
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let install_parent = fixed_install_parent_from_executable(executable)?;
        if !strict_absolute_path(destination)
            || !is_data_directory_name(destination)
            || !install_data_migration::destination_belongs_to_install_parent(
                &install_parent,
                destination,
            )?
        {
            return Err(anyhow::anyhow!("删除目标不是当前安装目录的数据目录"));
        }
        install_data_migration::delete_install_data_in_install_parent(&install_parent)
    }

    #[cfg(not(windows))]
    {
        let _ = (executable, destination);
        Err(anyhow::anyhow!("当前平台不支持安装数据删除命令"))
    }
}

pub fn run_probe_install_data_mode(
    executable: &std::path::Path,
    destination: &std::path::Path,
) -> anyhow::Result<()> {
    use std::io::Write;

    if !strict_absolute_path(destination) {
        return Err(anyhow::anyhow!("写入探针目标无效"));
    }
    destination
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|name| name.strip_prefix(".write-test-"))
        .filter(|suffix| !suffix.is_empty() && suffix.bytes().all(|value| value.is_ascii_digit()))
        .ok_or_else(|| anyhow::anyhow!("写入探针目标无效"))?;

    let published = open_published_install_data(executable)?;
    let expected_root = std::fs::canonicalize(published.paths().root())
        .map_err(|_| anyhow::anyhow!("写入探针目标无效"))?;
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("写入探针目标无效"))?;
    let parent = std::fs::canonicalize(parent).map_err(|_| anyhow::anyhow!("写入探针目标无效"))?;
    if parent != expected_root {
        return Err(anyhow::anyhow!("写入探针目标无效"));
    }

    let mut probe = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| anyhow::anyhow!("写入探针失败"))?;
    probe
        .write_all(b"ok")
        .and_then(|_| probe.sync_all())
        .map_err(|_| anyhow::anyhow!("写入探针失败"))
}

pub fn run_install_data_mode(
    executable: &std::path::Path,
    source: Option<&std::path::Path>,
    destination: &std::path::Path,
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let install_parent = fixed_install_parent_from_executable(executable)?;
        if !strict_absolute_path(destination)
            || !is_data_directory_name(destination)
            || !install_data_migration::destination_belongs_to_install_parent(
                &install_parent,
                destination,
            )?
        {
            return Err(anyhow::anyhow!("迁移目标不是当前安装目录的数据目录"));
        }
        install_data_migration::prepare_install_data_in_install_parent(source, &install_parent)?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let expected = normalized_install_data_root(executable)?;
        let destination = normalized_destination(destination, expected.parent())?;
        if destination != expected {
            return Err(anyhow::anyhow!("迁移目标不是当前安装目录的数据目录"));
        }

        let paths = app_paths::SharedAppPaths::from_data_root(destination);
        install_data_migration::prepare_install_data(source, &paths)?;
        Ok(())
    }
}

#[cfg(windows)]
fn fixed_install_parent_from_executable(
    executable: &std::path::Path,
) -> anyhow::Result<install_data_migration::InstallParentGuard> {
    let data_root = app_paths::data_root_from_executable(executable)?;
    let install_parent = data_root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法定位安装目录"))?;
    install_data_migration::open_install_parent(install_parent)
}

#[cfg(not(windows))]
fn normalized_install_data_root(
    executable: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    let data_root = app_paths::data_root_from_executable(executable)?;
    let install_directory = data_root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法定位安装目录"))?;
    let install_directory = std::fs::canonicalize(install_directory)
        .map_err(|_| anyhow::anyhow!("无法定位安装目录"))?;
    if !install_directory.is_dir() {
        return Err(anyhow::anyhow!("无法定位安装目录"));
    }
    Ok(install_directory.join("data"))
}

#[cfg(not(windows))]
fn normalized_destination(
    destination: &std::path::Path,
    expected_parent: Option<&std::path::Path>,
) -> anyhow::Result<std::path::PathBuf> {
    if !destination.is_absolute()
        || destination.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(anyhow::anyhow!("迁移目标不是当前安装目录的数据目录"));
    }

    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("迁移目标不是当前安装目录的数据目录"))?;
    let parent = std::fs::canonicalize(parent)
        .map_err(|_| anyhow::anyhow!("迁移目标不是当前安装目录的数据目录"))?;
    if Some(parent.as_path()) != expected_parent || !is_data_directory_name(destination) {
        return Err(anyhow::anyhow!("迁移目标不是当前安装目录的数据目录"));
    }
    Ok(parent.join("data"))
}

fn strict_absolute_path(path: &std::path::Path) -> bool {
    path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
}

#[cfg(windows)]
fn is_data_directory_name(destination: &std::path::Path) -> bool {
    destination
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("data"))
}

#[cfg(not(windows))]
fn is_data_directory_name(destination: &std::path::Path) -> bool {
    destination.file_name().is_some_and(|name| name == "data")
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
enum StartupFailure {
    #[error("安装数据目录缺失，请重新安装 Watchback")]
    MissingInstallData,
    #[error("安装数据目录权限不足，请修复数据目录权限或重新安装 Watchback")]
    DataDirectoryPermissions,
    #[error("安装数据目录无效，请重新安装 Watchback")]
    InvalidInstallData,
    #[error("安装数据标记损坏，请重新安装 Watchback")]
    CorruptInstallDataMarker,
    #[error("无法清理数据目录写入测试，请检查数据目录权限")]
    ProbeCleanup,
    #[cfg(not(windows))]
    #[error("此平台不支持从安装目录启动")]
    UnsupportedPlatform,
    #[error("启动失败，请重新安装 Watchback 或检查数据目录权限")]
    GenericStartup,
}

impl StartupFailure {
    fn message(self) -> &'static str {
        match self {
            Self::MissingInstallData => "安装数据目录缺失，请重新安装 Watchback",
            Self::DataDirectoryPermissions => {
                "安装数据目录权限不足，请修复数据目录权限或重新安装 Watchback"
            }
            Self::InvalidInstallData => "安装数据目录无效，请重新安装 Watchback",
            Self::CorruptInstallDataMarker => "安装数据标记损坏，请重新安装 Watchback",
            Self::ProbeCleanup => "无法清理数据目录写入测试，请检查数据目录权限",
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => "此平台不支持从安装目录启动",
            Self::GenericStartup => "启动失败，请重新安装 Watchback 或检查数据目录权限",
        }
    }
}

pub fn startup_failure_message(error: &anyhow::Error) -> &'static str {
    error
        .downcast_ref::<StartupFailure>()
        .map(|failure| failure.message())
        .unwrap_or("启动失败，请重新安装 Watchback 或检查数据目录权限")
}

pub fn handle_desktop_startup_result(
    result: anyhow::Result<()>,
    present: impl FnOnce(&str),
) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            present(startup_failure_message(&error));
            1
        }
    }
}

#[derive(Debug)]
pub struct PublishedInstallData {
    paths: app_paths::SharedAppPaths,
    #[cfg(windows)]
    _guard: PublishedInstallDataGuard,
}

impl PublishedInstallData {
    pub fn paths(&self) -> &app_paths::SharedAppPaths {
        &self.paths
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct PublishedInstallDataGuard {
    _install_directory: std::fs::File,
    _data_directory: std::fs::File,
    _root_marker: std::fs::File,
}

pub fn open_published_install_data(
    executable: &std::path::Path,
) -> anyhow::Result<PublishedInstallData> {
    #[cfg(windows)]
    {
        let root = app_paths::data_root_from_executable(executable)
            .map_err(|_| anyhow::Error::new(StartupFailure::InvalidInstallData))?;
        let (root, guard) = open_windows_published_install_data(&root)?;
        Ok(PublishedInstallData {
            paths: app_paths::SharedAppPaths::from_data_root(root),
            _guard: guard,
        })
    }

    #[cfg(not(windows))]
    {
        let _ = executable;
        Err(anyhow::Error::new(StartupFailure::UnsupportedPlatform))
    }
}

#[cfg(windows)]
fn open_windows_published_install_data(
    root: &std::path::Path,
) -> anyhow::Result<(std::path::PathBuf, PublishedInstallDataGuard)> {
    use std::{
        ffi::{OsStr, OsString},
        fs::OpenOptions,
        io::{Read, Seek, SeekFrom, Write},
        os::windows::{
            ffi::{OsStrExt, OsStringExt},
            fs::{MetadataExt, OpenOptionsExt},
            io::{AsRawHandle, FromRawHandle},
        },
    };
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                NtCreateFile, FILE_CREATE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
                FILE_SYNCHRONOUS_IO_NONALERT,
            },
        },
        Win32::{
            Foundation::{HANDLE, INVALID_HANDLE_VALUE, UNICODE_STRING},
            Storage::FileSystem::{
                FileDispositionInfo, GetFinalPathNameByHandleW, SetFileInformationByHandle,
                FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
                FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
                FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE, VOLUME_NAME_DOS,
            },
            System::{
                Kernel::{OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE},
                IO::IO_STATUS_BLOCK,
            },
        },
    };

    const STATUS_OBJECT_NAME_NOT_FOUND: i32 = 0xc000_0034_u32 as i32;
    const STATUS_OBJECT_PATH_NOT_FOUND: i32 = 0xc000_003a_u32 as i32;
    const DELETE: u32 = 0x0001_0000;

    struct RelativeFile {
        file: std::fs::File,
    }

    struct Probe {
        file: Option<std::fs::File>,
    }

    impl Probe {
        fn cleanup(mut self) -> anyhow::Result<()> {
            let Some(file) = self.file.take() else {
                return Ok(());
            };
            let disposition = FILE_DISPOSITION_INFO { DeleteFile: 1 };
            let deleted = unsafe {
                SetFileInformationByHandle(
                    file.as_raw_handle() as _,
                    FileDispositionInfo,
                    &disposition as *const _ as _,
                    std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
                )
            };
            drop(file);
            if deleted == 0 {
                return Err(anyhow::Error::new(StartupFailure::ProbeCleanup));
            }
            Ok(())
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            if let Some(file) = self.file.take() {
                let disposition = FILE_DISPOSITION_INFO { DeleteFile: 1 };
                unsafe {
                    let _ = SetFileInformationByHandle(
                        file.as_raw_handle() as _,
                        FileDispositionInfo,
                        &disposition as *const _ as _,
                        std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
                    );
                }
            }
        }
    }

    fn open_optional_relative_file(
        parent: &std::fs::File,
        name: &OsStr,
        access: u32,
        share_access: u32,
        disposition: u32,
        failure: StartupFailure,
    ) -> anyhow::Result<Option<RelativeFile>> {
        let mut name = name.encode_wide().collect::<Vec<_>>();
        if name.is_empty() || name.contains(&0) {
            return Err(anyhow::Error::new(StartupFailure::CorruptInstallDataMarker));
        }
        let byte_length = u16::try_from(
            name.len()
                .checked_mul(std::mem::size_of::<u16>())
                .ok_or_else(|| anyhow::Error::new(StartupFailure::CorruptInstallDataMarker))?,
        )
        .map_err(|_| anyhow::Error::new(StartupFailure::CorruptInstallDataMarker))?;
        let object_name = UNICODE_STRING {
            Length: byte_length,
            MaximumLength: byte_length,
            Buffer: name.as_mut_ptr(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.as_raw_handle() as HANDLE,
            ObjectName: &object_name,
            Attributes: (OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE) as u32,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut status = unsafe { std::mem::zeroed::<IO_STATUS_BLOCK>() };
        let mut raw = std::ptr::null_mut();
        let result = unsafe {
            NtCreateFile(
                &mut raw,
                access | SYNCHRONIZE,
                &attributes,
                &mut status,
                std::ptr::null(),
                FILE_ATTRIBUTE_NORMAL,
                share_access,
                disposition,
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if matches!(
            result,
            STATUS_OBJECT_NAME_NOT_FOUND | STATUS_OBJECT_PATH_NOT_FOUND
        ) {
            return Ok(None);
        }
        if result < 0 || raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(anyhow::Error::new(failure));
        }
        let file = unsafe { std::fs::File::from_raw_handle(raw as _) };
        Ok(Some(RelativeFile { file }))
    }

    fn open_relative_file(
        parent: &std::fs::File,
        name: &OsStr,
        access: u32,
        share_access: u32,
        disposition: u32,
        failure: StartupFailure,
    ) -> anyhow::Result<RelativeFile> {
        open_optional_relative_file(parent, name, access, share_access, disposition, failure)?
            .ok_or_else(|| anyhow::Error::new(failure))
    }

    fn validate_regular_file(file: &std::fs::File, failure: StartupFailure) -> anyhow::Result<()> {
        let attributes = file
            .metadata()
            .map_err(|_| anyhow::Error::new(failure))?
            .file_attributes();
        if attributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0 {
            return Err(anyhow::Error::new(failure));
        }
        Ok(())
    }

    fn final_dos_path(file: &std::fs::File) -> anyhow::Result<std::path::PathBuf> {
        fn ascii_prefix(value: &[u16], prefix: &[u16]) -> bool {
            fn ascii_lowercase(value: u16) -> u16 {
                if (b'A' as u16..=b'Z' as u16).contains(&value) {
                    value + (b'a' - b'A') as u16
                } else {
                    value
                }
            }

            value.len() >= prefix.len()
                && value[..prefix.len()]
                    .iter()
                    .zip(prefix)
                    .all(|(left, right)| ascii_lowercase(*left) == ascii_lowercase(*right))
        }

        let mut buffer = vec![0_u16; 512];
        let length = loop {
            let length = unsafe {
                GetFinalPathNameByHandleW(
                    file.as_raw_handle() as _,
                    buffer.as_mut_ptr(),
                    u32::try_from(buffer.len())
                        .map_err(|_| anyhow::Error::new(StartupFailure::InvalidInstallData))?,
                    FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
                )
            };
            if length == 0 {
                return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
            }
            let length = usize::try_from(length)
                .map_err(|_| anyhow::Error::new(StartupFailure::InvalidInstallData))?;
            if length < buffer.len() {
                break length;
            }
            let capacity = length
                .checked_add(1)
                .ok_or_else(|| anyhow::Error::new(StartupFailure::InvalidInstallData))?;
            buffer.resize(capacity, 0);
        };
        buffer.truncate(length);

        let extended_unc = r"\\?\UNC\".encode_utf16().collect::<Vec<_>>();
        let extended = r"\\?\".encode_utf16().collect::<Vec<_>>();
        let path = if ascii_prefix(&buffer, &extended_unc) {
            let mut normalized = r"\\".encode_utf16().collect::<Vec<_>>();
            normalized.extend_from_slice(&buffer[extended_unc.len()..]);
            normalized
        } else if ascii_prefix(&buffer, &extended) {
            buffer[extended.len()..].to_vec()
        } else {
            return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
        };
        let path = std::path::PathBuf::from(OsString::from_wide(&path));
        if !path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
        }
        Ok(path)
    }

    let install_root = root
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| anyhow::Error::new(StartupFailure::InvalidInstallData))?;
    let install_directory = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(install_root)
        .map_err(|_| anyhow::Error::new(StartupFailure::InvalidInstallData))?;
    let install_attributes = install_directory
        .metadata()
        .map_err(|_| anyhow::Error::new(StartupFailure::InvalidInstallData))?
        .file_attributes();
    if install_attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
    }
    let physical_install_root = final_dos_path(&install_directory)?;
    let physical_root = physical_install_root.join("data");

    match std::fs::symlink_metadata(&physical_root) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(anyhow::Error::new(StartupFailure::MissingInstallData));
        }
        Err(_) => return Err(anyhow::Error::new(StartupFailure::DataDirectoryPermissions)),
    }

    let data_directory = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&physical_root)
        .map_err(|_| anyhow::Error::new(StartupFailure::DataDirectoryPermissions))?;
    let attributes = data_directory
        .metadata()
        .map_err(|_| anyhow::Error::new(StartupFailure::DataDirectoryPermissions))?
        .file_attributes();
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
    }
    if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
    }
    let physical_root = final_dos_path(&data_directory)?;
    if physical_root.parent() != Some(physical_install_root.as_path()) {
        return Err(anyhow::Error::new(StartupFailure::InvalidInstallData));
    }

    let root_marker = open_optional_relative_file(
        &data_directory,
        OsStr::new(install_data_migration::INSTALL_DATA_ROOT_MARKER),
        FILE_GENERIC_READ | FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ,
        FILE_OPEN,
        StartupFailure::DataDirectoryPermissions,
    )?;
    let (marker, migration_marker) = match root_marker {
        Some(marker) => (marker, false),
        None => (
            open_optional_relative_file(
                &data_directory,
                OsStr::new(install_data_migration::INSTALL_DATA_MIGRATION_MARKER),
                FILE_GENERIC_READ | FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ,
                FILE_OPEN,
                StartupFailure::DataDirectoryPermissions,
            )?
            .ok_or_else(|| anyhow::Error::new(StartupFailure::CorruptInstallDataMarker))?,
            true,
        ),
    };
    let mut marker = marker.file;
    validate_regular_file(&marker, StartupFailure::CorruptInstallDataMarker)?;
    let marker_contents = (|| -> anyhow::Result<Vec<u8>> {
        marker
            .seek(SeekFrom::Start(0))
            .map_err(|_| anyhow::Error::new(StartupFailure::CorruptInstallDataMarker))?;
        let limit = u64::try_from(if migration_marker {
            256
        } else {
            install_data_migration::INSTALL_DATA_ROOT_MARKER_CONTENTS.len()
        })
        .map_err(|_| anyhow::Error::new(StartupFailure::CorruptInstallDataMarker))?
        .checked_add(1)
        .ok_or_else(|| anyhow::Error::new(StartupFailure::CorruptInstallDataMarker))?;
        let mut contents = Vec::new();
        (&mut marker)
            .take(limit)
            .read_to_end(&mut contents)
            .map_err(|_| anyhow::Error::new(StartupFailure::CorruptInstallDataMarker))?;
        Ok(contents)
    })();
    let marker_contents = marker_contents?;
    let marker_valid = if migration_marker {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct MigrationProof {
            version: u8,
            source_retained: bool,
        }
        serde_json::from_slice::<MigrationProof>(&marker_contents)
            .map(|proof| proof.version == 1 && proof.source_retained)
            .unwrap_or(false)
    } else {
        marker_contents.is_empty()
            || marker_contents == install_data_migration::INSTALL_DATA_ROOT_MARKER_CONTENTS
    };
    if !marker_valid {
        return Err(anyhow::Error::new(StartupFailure::CorruptInstallDataMarker));
    }

    let probe_name = format!(".runtime-write-test-{}", uuid::Uuid::new_v4());
    let probe = open_relative_file(
        &data_directory,
        OsStr::new(&probe_name),
        FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES | DELETE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_CREATE,
        StartupFailure::DataDirectoryPermissions,
    )?;
    let mut probe = Probe {
        file: Some(probe.file),
    };
    if let Err(error) = validate_regular_file(
        probe.file.as_ref().expect("写入测试句柄存在"),
        StartupFailure::InvalidInstallData,
    ) {
        return match probe.cleanup() {
            Ok(()) => Err(error),
            Err(error) => Err(error),
        };
    }
    let write_result = probe
        .file
        .as_mut()
        .expect("写入测试句柄存在")
        .write_all(b"ok")
        .and_then(|_| probe.file.as_ref().expect("写入测试句柄存在").sync_all());
    let cleanup_result = probe.cleanup();
    if write_result.is_err() {
        return match cleanup_result {
            Ok(()) => Err(anyhow::Error::new(StartupFailure::DataDirectoryPermissions)),
            Err(error) => Err(error),
        };
    }
    cleanup_result?;

    Ok((
        physical_root,
        PublishedInstallDataGuard {
            _install_directory: install_directory,
            _data_directory: data_directory,
            _root_marker: marker,
        },
    ))
}

pub fn run() -> anyhow::Result<()> {
    use std::sync::Arc;
    use tauri::{Emitter, Manager};

    let executable = std::env::current_exe()
        .map_err(|_| anyhow::Error::new(StartupFailure::InvalidInstallData))?;
    let published_data = open_published_install_data(&executable)?;
    let shared = published_data.paths().clone();
    diagnostics::initialize(shared.log_file())?;
    diagnostics::record(
        diagnostics::Level::Info,
        "application",
        "started",
        &[
            ("version", env!("CARGO_PKG_VERSION").to_string()),
            (
                "build",
                if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }
                .to_string(),
            ),
            ("os", std::env::consts::OS.to_string()),
            ("arch", std::env::consts::ARCH.to_string()),
        ],
    );
    let sync_coordinator = sync_coordinator::SyncCoordinator::default();
    let background_manager =
        background_sync::BackgroundSeasonSyncManager::new(sync_coordinator.clone());
    let live_update_manager = live_season_update::LiveSeasonUpdateManager::default();
    let player_query_service = player_query::PlayerQueryService::new(sync_coordinator.clone());
    let startup_sync_coordinator = sync_coordinator.clone();
    let startup_background_manager = background_manager.clone();
    let startup_live_update_manager = live_update_manager.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(published_data)
        .manage(background_manager)
        .manage(live_update_manager)
        .manage(player_query_service)
        .manage(update_check::UpdateChecker::new())
        .setup(move |app| {
            let account_context = account_context::AccountContextManager::load(shared.clone())
                .map_err(|error| anyhow::anyhow!("无法初始化账号管理: {error}"))?;
            app.manage(account_context.clone());
            app.manage(
                deepseek_key::DeepSeekKeyManager::new()
                    .map_err(|error| anyhow::anyhow!("无法初始化 DeepSeek 配置: {error}"))?,
            );
            let manager = app.state::<background_sync::BackgroundSeasonSyncManager>();
            let registry = account_context.registry_snapshot()?;
            let account_count = registry.accounts.len();
            let mut recovered_jobs = 0usize;
            for entry in registry.accounts {
                if entry.credential_state != "ready" {
                    continue;
                }
                let paths = account_context.account_paths(&entry.account_id);
                let store = sync::SeasonSyncStore::new(db::OwDatabase::for_account(&paths));
                let _ = store.requeue_orphaned_running();
                for season in 21..=23 {
                    if store.active_job_count(season).unwrap_or(0) == 0 {
                        continue;
                    }
                    recovered_jobs += 1;
                    let event_app = app.handle().clone();
                    let events: background_sync::BackgroundEventSink =
                        Arc::new(move |event, payload| {
                            let _ = event_app.emit(event, payload);
                        });
                    let observed_accounts = account_context.clone();
                    let observed_account_id = entry.account_id.clone();
                    manager.wake_observed(
                        entry.account_id.as_str(),
                        paths.clone(),
                        season,
                        events,
                        Some(Arc::new(move |error| {
                            let failure =
                                account_context::CredentialUseFailure::from_upstream_error(error);
                            let _ = observed_accounts
                                .record_credential_use_failure(&observed_account_id, failure);
                        })),
                    );
                }
            }
            auto_season_update::spawn(
                app.handle().clone(),
                account_context,
                startup_sync_coordinator.clone(),
                startup_background_manager.clone(),
                startup_live_update_manager.clone(),
            );
            diagnostics::record(
                diagnostics::Level::Info,
                "application",
                "ready",
                &[
                    ("accountCount", account_count.to_string()),
                    ("recoveredBackgroundJobs", recovered_jobs.to_string()),
                ],
            );
            Ok(())
        })
        .manage(sync_coordinator)
        .manage(ai_roast::coordinator::AiRoastCoordinator::default())
        .manage(qr_login::QrLoginCoordinator::default())
        .invoke_handler(tauri::generate_handler![
            commands::record_frontend_diagnostic,
            commands::check_app_update,
            commands::get_app_version,
            commands::open_update_download,
            commands::get_overview,
            commands::open_product_feedback,
            commands::open_project_repository,
            commands::open_support_directory,
            commands::list_match_filters,
            commands::list_match_timeline,
            commands::list_match_timeline_page,
            commands::get_match_timeline_revision,
            commands::search_match_player_options,
            commands::get_match_details,
            commands::save_match_favorite,
            commands::delete_match_favorite,
            commands::refresh_match_evaluations,
            commands::get_friend_details,
            commands::get_onboarding_status,
            commands::get_personal_season_review,
            commands::get_career_achievements,
            commands::get_deepseek_key_status,
            commands::save_deepseek_key,
            commands::generate_match_ai_roast,
            commands::get_player_profile,
            commands::refresh_profile_card,
            commands::get_player_game_action,
            commands::get_user_settings,
            commands::save_user_settings,
            commands::add_account,
            commands::add_account_with_qr,
            commands::list_accounts,
            commands::switch_account,
            commands::remove_account,
            commands::cancel_dashen_login,
            commands::refresh_dashen_login_qr,
            commands::get_dashen_login_qr_snapshot,
            commands::get_active_dashen_login_qr,
            commands::cancel_dashen_login_qr,
            commands::get_rankings,
            commands::get_season_analysis,
            commands::season_sync_status,
            commands::retry_season_background_sync,
            commands::sync_season,
            commands::get_operation_status,
            commands::player_query_start,
            commands::player_query_card,
            commands::player_query_card_images,
            commands::player_query_season,
            commands::player_query_mode_page,
            commands::player_query_mode_heroes,
            commands::player_query_match_detail,
            commands::player_query_release_more_data,
            commands::player_query_end,
        ])
        .run(tauri::generate_context!())
        .map(|()| {
            diagnostics::record(diagnostics::Level::Info, "application", "stopped", &[]);
        })
        .map_err(|error| {
            diagnostics::record_error("application", "runtime_failed", &error, &[]);
            anyhow::Error::new(StartupFailure::GenericStartup)
        })
}
