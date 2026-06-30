mod analytics;
pub mod browser_session;
mod commands;
pub mod credential_store;
pub mod credentials;
pub mod dashen_auth;
mod db;
mod importer;
pub mod login_flow;
mod match_evaluation;
mod models;
mod queries;
pub mod ranking;
mod redaction;
mod review;
mod sync;

pub use analytics::{get_friend_analytics, get_friend_detail, get_season_analysis};
pub use db::OwDatabase;
pub use importer::{import_full_season_payload, import_payload_value};
pub use match_evaluation::{refresh_match_evaluations, PHASE5_SCORING_RULE_VERSION};
pub use models::*;
pub use queries::{get_match_detail, list_filter_options, list_matches};
pub use redaction::redact_sensitive_json;
pub use review::{get_onboarding_status, get_personal_season_review};
pub use sync::{
    sync_import_parts, sync_season, sync_season_with_base_url, sync_season_with_progress,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_overview,
            commands::list_match_filters,
            commands::list_match_timeline,
            commands::get_match_details,
            commands::refresh_match_evaluations,
            commands::get_friend_analytics,
            commands::get_friend_details,
            commands::get_onboarding_status,
            commands::get_personal_season_review,
            commands::get_credential_status,
            commands::start_dashen_login,
            commands::cancel_dashen_login,
            commands::get_rankings,
            commands::get_season_analysis,
            commands::sync_season,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run OWStatsLocal");
}
