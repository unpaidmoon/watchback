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
mod importer;
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
pub use review::{get_onboarding_status, get_personal_season_review};
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
            commands::get_credential_status,
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
        .expect("failed to run OWStatsLocal");
}
