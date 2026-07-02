use crate::importer::{default_database_path, existing_import_summary};
use crate::{
    get_friend_analytics as query_friend_analytics, get_friend_detail as query_friend_detail,
    get_match_detail, list_filter_options, list_matches,
    refresh_match_evaluations as refresh_evaluations, search_match_players,
    sync_season_with_progress, CredentialLoginCancelResult, CredentialLoginResult,
    CredentialStatus, FilterOptions, FriendAnalytics, FriendAnalyticsFilters, FriendDetail,
    ImportSummary, LogoutResult, MatchDetailView, MatchFilters, MatchSummary, OnboardingStatus,
    OnboardingStatusRequest, OwDatabase, PersonalReviewRequest, PersonalSeasonReview,
    PlayerProfile, PlayerSearchOption, RankingResponse, SeasonAnalysis, SyncRequest, SyncSummary,
    UpdateCheckResult, UserSettings,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub database_path: String,
    pub import_summary: Option<ImportSummary>,
}

#[tauri::command]
pub fn get_overview(app: AppHandle) -> Result<Overview, String> {
    let db = app_database(&app)?;
    Ok(Overview {
        database_path: db.path().display().to_string(),
        import_summary: existing_import_summary(&db).map_err(to_message)?,
    })
}

#[tauri::command]
pub fn list_match_filters(app: AppHandle) -> Result<FilterOptions, String> {
    list_filter_options(&app_database(&app)?).map_err(to_message)
}

#[tauri::command]
pub fn list_match_timeline(
    app: AppHandle,
    filters: MatchFilters,
) -> Result<Vec<MatchSummary>, String> {
    list_matches(&app_database(&app)?, filters).map_err(to_message)
}

#[tauri::command]
pub fn search_match_player_options(
    app: AppHandle,
    query: String,
) -> Result<Vec<PlayerSearchOption>, String> {
    search_match_players(&app_database(&app)?, &query).map_err(to_message)
}

#[tauri::command]
pub fn get_match_details(app: AppHandle, match_id: String) -> Result<MatchDetailView, String> {
    get_match_detail(&app_database(&app)?, &match_id).map_err(to_message)
}

#[tauri::command]
pub fn refresh_match_evaluations(app: AppHandle) -> Result<(), String> {
    refresh_evaluations(&app_database(&app)?).map_err(to_message)
}

#[tauri::command]
pub fn get_friend_analytics(
    app: AppHandle,
    filters: FriendAnalyticsFilters,
) -> Result<FriendAnalytics, String> {
    query_friend_analytics(&app_database(&app)?, filters).map_err(to_message)
}

#[tauri::command]
pub fn get_friend_details(
    app: AppHandle,
    bnet_id: String,
    filters: FriendAnalyticsFilters,
) -> Result<FriendDetail, String> {
    query_friend_detail(&app_database(&app)?, &bnet_id, filters).map_err(to_message)
}

#[tauri::command]
pub fn get_onboarding_status(
    app: AppHandle,
    request: OnboardingStatusRequest,
) -> Result<OnboardingStatus, String> {
    let mut status =
        crate::get_onboarding_status(&app_database(&app)?, request).map_err(to_message)?;
    status.credential_status =
        crate::credentials::get_credential_status(&app).map_err(to_message)?;
    Ok(status)
}

#[tauri::command]
pub fn get_personal_season_review(
    app: AppHandle,
    request: PersonalReviewRequest,
) -> Result<PersonalSeasonReview, String> {
    crate::get_personal_season_review(&app_database(&app)?, request).map_err(to_message)
}

#[tauri::command]
pub fn get_credential_status(app: AppHandle) -> Result<CredentialStatus, String> {
    crate::credentials::get_credential_status(&app).map_err(to_message)
}

#[tauri::command]
pub fn get_player_profile(app: AppHandle) -> Result<PlayerProfile, String> {
    let role_id = crate::credentials::get_credential_status(&app)
        .map_err(to_message)?
        .role_id;
    crate::settings::get_player_profile_with_project_root(
        &app_database(&app)?,
        &player_profile_path(&app)?,
        role_id,
        &project_root()?,
    )
    .map_err(to_message)
}

#[tauri::command]
pub fn get_user_settings(app: AppHandle) -> Result<UserSettings, String> {
    crate::settings::get_user_settings_for_path(&user_settings_path(&app)?).map_err(to_message)
}

#[tauri::command]
pub fn save_user_settings(app: AppHandle, settings: UserSettings) -> Result<UserSettings, String> {
    crate::settings::save_user_settings_for_path(&user_settings_path(&app)?, settings)
        .map_err(to_message)
}

#[tauri::command]
pub fn logout(app: AppHandle) -> Result<LogoutResult, String> {
    let cleared = crate::credentials::clear_credentials(&app).map_err(to_message)?;
    Ok(LogoutResult {
        credential_cleared: cleared,
        message: if cleared {
            "已退出登录，本地凭证已清除。".to_string()
        } else {
            "当前没有可清除的本地凭证。".to_string()
        },
    })
}

#[tauri::command]
pub fn check_app_update() -> Result<UpdateCheckResult, String> {
    Ok(crate::settings::check_app_update_result(env!(
        "CARGO_PKG_VERSION"
    )))
}

#[tauri::command]
pub async fn start_dashen_login(app: AppHandle) -> Result<CredentialLoginResult, String> {
    crate::credentials::start_dashen_login(app)
        .await
        .map_err(to_message)
}

#[tauri::command]
pub fn cancel_dashen_login(app: AppHandle) -> Result<CredentialLoginCancelResult, String> {
    crate::credentials::cancel_dashen_login(app).map_err(to_message)
}

#[tauri::command]
pub fn get_rankings(app: AppHandle, season: Option<i64>) -> Result<RankingResponse, String> {
    crate::ranking::get_rankings(&app_database(&app)?, season).map_err(to_message)
}

#[tauri::command]
pub fn get_season_analysis(app: AppHandle, season: Option<i64>) -> Result<SeasonAnalysis, String> {
    crate::get_season_analysis(&app_database(&app)?, season).map_err(to_message)
}

#[tauri::command]
pub async fn sync_season(app: AppHandle, request: SyncRequest) -> Result<SyncSummary, String> {
    let db = app_database(&app)?;
    let root = project_root()?;
    let emit_app = app.clone();
    sync_season_with_progress(&db, &root, request.season, move |progress| {
        let _ = emit_app.emit("sync-progress", progress);
    })
    .await
    .map_err(to_message)
}

fn app_database(app: &AppHandle) -> Result<OwDatabase, String> {
    let data_dir = app.path().app_data_dir().map_err(to_message)?;
    Ok(OwDatabase::new(default_database_path(data_dir)))
}

fn player_profile_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(to_message)?
        .join("player_profile.json"))
}

fn user_settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(to_message)?
        .join("user_settings.json"))
}

fn project_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|apps_dir| apps_dir.parent())
        .and_then(|repo| repo.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve repository root".to_string())
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
