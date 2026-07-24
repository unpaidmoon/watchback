pub mod account_context;
pub mod account_login;
pub mod account_migration;
pub mod account_removal;
pub mod account_store;
pub mod ai_roast;
mod analytics;
pub mod app_paths;
mod background_sync;
pub mod browser_session;
pub mod career_mmr;
mod commands;
pub mod credential_store;
pub mod credentials;
pub mod dashen_auth;
pub mod dashen_client;
mod database;
mod db;
pub mod deepseek_key;
mod importer;
pub mod install_data_migration;
pub mod login_flow;
pub mod login_target;
mod match_evaluation;
mod match_favorites;
mod models;
pub mod player_target;
mod queries;
pub mod ranking;
mod review;
pub mod season_fetch;
mod settings;
mod sync;
mod sync_coordinator;

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
pub use queries::{get_match_detail, list_filter_options, list_matches, search_match_players};
pub use review::{get_career_achievements, get_onboarding_status, get_personal_season_review};
#[doc(hidden)]
pub use sync::write_season_payload;
#[doc(hidden)]
pub use sync::{
    benchmark_snapshot, rebuild_complete_season_payload, rebuild_snapshot_for_revision,
    rebuild_snapshot_for_revision_with_hook, SnapshotBenchmark, SnapshotOutcome,
};
pub use sync::{
    build_page_diagnostics, classify_quick_page, commit_quick_list_probe, dump_season_json,
    dump_season_json_with_base_url, load_season_sync_inventory,
    quick_list_update_with_credentials_and_target_paths_base_url, run_background_job_once,
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
        return install_data_migration::delete_install_data_in_install_parent(&install_parent);
    }

    #[cfg(not(windows))]
    {
        let _ = (executable, destination);
        Err(anyhow::anyhow!("当前平台不支持安装数据删除命令"))
    }
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
        return Ok(());
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
        let guard = open_windows_published_install_data(&root)?;
        return Ok(PublishedInstallData {
            paths: app_paths::SharedAppPaths::from_data_root(root),
            _guard: guard,
        });
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
) -> anyhow::Result<PublishedInstallDataGuard> {
    use std::{
        ffi::OsStr,
        fs::OpenOptions,
        io::Write,
        os::windows::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
            io::{AsRawHandle, FromRawHandle},
        },
    };
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                NtCreateFile, FILE_CREATE, FILE_OPEN_IF, FILE_OPEN_REPARSE_POINT,
                FILE_SYNCHRONOUS_IO_NONALERT,
            },
        },
        Win32::{
            Foundation::{HANDLE, INVALID_HANDLE_VALUE, UNICODE_STRING},
            Storage::FileSystem::{
                FileDispositionInfo, SetFileInformationByHandle, FILE_ATTRIBUTE_DIRECTORY,
                FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO,
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
                FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
                SYNCHRONIZE,
            },
            System::{
                Kernel::{OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE},
                IO::IO_STATUS_BLOCK,
            },
        },
    };

    const ROOT_MARKER: &str = ".watchback-data-root-v1";
    const ROOT_MARKER_CONTENTS: &[u8] = b"v1";
    const FILE_CREATED_INFORMATION: usize = 2;
    const DELETE: u32 = 0x0001_0000;

    struct RelativeFile {
        file: std::fs::File,
        created: bool,
    }

    struct Probe {
        file: Option<std::fs::File>,
    }

    impl Probe {
        fn disarm(mut self) -> std::fs::File {
            self.file.take().expect("写入测试句柄存在")
        }

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

    fn open_relative_file(
        parent: &std::fs::File,
        name: &OsStr,
        access: u32,
        disposition: u32,
        failure: StartupFailure,
    ) -> anyhow::Result<RelativeFile> {
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
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                disposition,
                FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if result < 0 || raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(anyhow::Error::new(failure));
        }
        let file = unsafe { std::fs::File::from_raw_handle(raw as _) };
        Ok(RelativeFile {
            file,
            created: status.Information == FILE_CREATED_INFORMATION,
        })
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

    match std::fs::symlink_metadata(root) {
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
        .open(root)
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

    let marker = open_relative_file(
        &data_directory,
        OsStr::new(ROOT_MARKER),
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES | DELETE,
        FILE_OPEN_IF,
        StartupFailure::DataDirectoryPermissions,
    )?;
    let marker_created = marker.created;
    let mut marker = Probe {
        file: Some(marker.file),
    };
    if let Err(error) = validate_regular_file(
        marker.file.as_ref().expect("写入测试句柄存在"),
        StartupFailure::CorruptInstallDataMarker,
    ) {
        return match marker.cleanup() {
            Ok(()) => Err(error),
            Err(error) => Err(error),
        };
    }
    if marker_created {
        if marker
            .file
            .as_mut()
            .expect("写入测试句柄存在")
            .write_all(ROOT_MARKER_CONTENTS)
            .and_then(|_| marker.file.as_ref().expect("写入测试句柄存在").sync_all())
            .is_err()
        {
            return match marker.cleanup() {
                Ok(()) => Err(anyhow::Error::new(StartupFailure::DataDirectoryPermissions)),
                Err(error) => Err(error),
            };
        }
    }
    let marker = marker.disarm();

    let probe_name = format!(".runtime-write-test-{}", uuid::Uuid::new_v4());
    let probe = open_relative_file(
        &data_directory,
        OsStr::new(&probe_name),
        FILE_GENERIC_WRITE | FILE_READ_ATTRIBUTES | DELETE,
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

    Ok(PublishedInstallDataGuard {
        _data_directory: data_directory,
        _root_marker: marker,
    })
}

pub fn run() -> anyhow::Result<()> {
    use std::sync::Arc;
    use tauri::{Emitter, Manager};

    let executable = std::env::current_exe()
        .map_err(|_| anyhow::Error::new(StartupFailure::InvalidInstallData))?;
    let published_data = open_published_install_data(&executable)?;
    let shared = published_data.paths().clone();
    let sync_coordinator = sync_coordinator::SyncCoordinator::default();
    let background_manager =
        background_sync::BackgroundSeasonSyncManager::new(sync_coordinator.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(published_data)
        .manage(background_manager)
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
            for entry in registry.accounts {
                let paths = account_context.account_paths(&entry.account_id);
                let store = sync::SeasonSyncStore::new(db::OwDatabase::for_account(&paths));
                let _ = store.requeue_orphaned_running();
                for season in 21..=23 {
                    if store.active_job_count(season).unwrap_or(0) == 0 {
                        continue;
                    }
                    let event_app = app.handle().clone();
                    let events: background_sync::BackgroundEventSink =
                        Arc::new(move |event, payload| {
                            let _ = event_app.emit(event, payload);
                        });
                    manager.wake(entry.account_id.as_str(), paths.clone(), season, events);
                }
            }
            Ok(())
        })
        .manage(sync_coordinator)
        .manage(ai_roast::coordinator::AiRoastCoordinator::default())
        .manage(login_target::LoginTargetCoordinator::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_overview,
            commands::list_match_filters,
            commands::list_match_timeline,
            commands::search_match_player_options,
            commands::get_match_details,
            commands::save_match_favorite,
            commands::delete_match_favorite,
            commands::refresh_match_evaluations,
            commands::get_friend_analytics,
            commands::get_friend_details,
            commands::get_onboarding_status,
            commands::get_personal_season_review,
            commands::get_career_achievements,
            commands::get_credential_status,
            commands::get_deepseek_key_status,
            commands::save_deepseek_key,
            commands::generate_match_ai_roast,
            commands::get_player_profile,
            commands::get_user_settings,
            commands::save_user_settings,
            commands::check_app_update,
            commands::add_account,
            commands::relogin_account,
            commands::list_accounts,
            commands::switch_account,
            commands::remove_account,
            commands::cancel_dashen_login,
            commands::get_player_target_status,
            commands::get_login_target_status,
            commands::confirm_login_target_switch,
            commands::keep_current_player_target,
            commands::retry_login_target_binding,
            commands::clear_player_target,
            commands::get_rankings,
            commands::get_season_analysis,
            commands::get_career_mmr_overview,
            commands::get_role_mmr_history,
            commands::get_role_mmr_mode_breakdown,
            commands::get_role_mmr_season_breakdown,
            commands::get_mmr_match_explanation,
            commands::get_recent_mmr_explanations,
            commands::rebuild_career_mmr,
            commands::get_career_mmr_build_status,
            commands::season_sync_status,
            commands::retry_season_background_sync,
            commands::sync_season,
        ])
        .run(tauri::generate_context!())
        .map_err(|_| anyhow::Error::new(StartupFailure::GenericStartup))
}
