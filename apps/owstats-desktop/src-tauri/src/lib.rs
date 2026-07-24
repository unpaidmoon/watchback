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

pub fn run() {
    use std::sync::Arc;
    use tauri::{Emitter, Manager};

    let sync_coordinator = sync_coordinator::SyncCoordinator::default();
    let background_manager =
        background_sync::BackgroundSeasonSyncManager::new(sync_coordinator.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(background_manager)
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| anyhow::anyhow!("无法定位应用数据目录: {error}"))?;
            let shared = app_paths::SharedAppPaths::from_app_data_dir(app_data_dir);
            let account_context = account_context::AccountContextManager::load(shared)
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
        .expect("failed to run Watchback");
}
