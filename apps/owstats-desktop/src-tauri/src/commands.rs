use crate::account_context::{AccountContextManager, AccountContextSnapshot};
use crate::ai_roast::client::DeepSeekRoastClient;
use crate::ai_roast::coordinator::{
    AiRoastCoordinator, AiRoastGenerationResult, CapturedRoastRequest,
};
use crate::deepseek_key::DeepSeekKeyManager;
use crate::importer::existing_import_summary;
use crate::{
    get_friend_analytics as query_friend_analytics, get_friend_detail as query_friend_detail,
    get_match_detail, list_filter_options, list_matches,
    quick_list_update_with_credentials_and_target_paths_base_url,
    refresh_match_evaluations as refresh_evaluations, search_match_players,
    sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url,
    AccountLoginResult, ClearPlayerTargetResult, CredentialLoginCancelResult, CredentialStatus,
    DeepSeekKeyStatus, FilterOptions, FriendAnalytics, FriendAnalyticsFilters, FriendDetail,
    ImportSummary, LoginTargetStatus, MatchDetailView, MatchFavoriteRequest, MatchFavoriteView,
    MatchFilters, MatchSummary, OnboardingStatus, OnboardingStatusRequest, PersonalReviewRequest,
    PersonalSeasonReview, PlayerProfile, PlayerSearchOption, PlayerTargetStatus, RankingResponse,
    SeasonAnalysis, SeasonSyncResult, SyncRequest, UpdateCheckResult, UserSettings,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

const DASHEN_ROOT_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub database_path: String,
    pub import_summary: Option<ImportSummary>,
}

#[tauri::command]
pub fn get_overview(accounts: State<'_, AccountContextManager>) -> Result<Overview, String> {
    let account = active_account(&accounts)?;
    let db = account.database();
    Ok(Overview {
        database_path: db.path().display().to_string(),
        import_summary: existing_import_summary(&db).map_err(to_message)?,
    })
}

#[tauri::command]
pub fn list_match_filters(
    accounts: State<'_, AccountContextManager>,
) -> Result<FilterOptions, String> {
    let account = active_account(&accounts)?;
    list_filter_options(&account.database()).map_err(to_message)
}

#[tauri::command]
pub fn list_match_timeline(
    accounts: State<'_, AccountContextManager>,
    filters: MatchFilters,
) -> Result<Vec<MatchSummary>, String> {
    let account = active_account(&accounts)?;
    list_matches(&account.database(), filters).map_err(to_message)
}

#[tauri::command]
pub fn search_match_player_options(
    accounts: State<'_, AccountContextManager>,
    query: String,
) -> Result<Vec<PlayerSearchOption>, String> {
    let account = active_account(&accounts)?;
    search_match_players(&account.database(), &query).map_err(to_message)
}

#[tauri::command]
pub fn get_match_details(
    accounts: State<'_, AccountContextManager>,
    match_id: String,
) -> Result<MatchDetailView, String> {
    let account = active_account(&accounts)?;
    get_match_detail(&account.database(), &match_id).map_err(to_message)
}

#[tauri::command]
pub fn save_match_favorite(
    accounts: State<'_, AccountContextManager>,
    request: MatchFavoriteRequest,
) -> Result<MatchFavoriteView, String> {
    let account = active_account(&accounts)?;
    crate::save_match_favorite(&account.database(), &request.match_id, &request.note)
        .map_err(to_message)
}

#[tauri::command]
pub fn delete_match_favorite(
    accounts: State<'_, AccountContextManager>,
    match_id: String,
) -> Result<(), String> {
    let account = active_account(&accounts)?;
    crate::delete_match_favorite(&account.database(), &match_id).map_err(to_message)
}

#[tauri::command]
pub fn refresh_match_evaluations(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
) -> Result<(), String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account = active_account(&accounts)?;
    refresh_evaluations(&account.database()).map_err(to_message)
}

#[tauri::command]
pub fn get_friend_analytics(
    accounts: State<'_, AccountContextManager>,
    filters: FriendAnalyticsFilters,
) -> Result<FriendAnalytics, String> {
    let account = active_account(&accounts)?;
    query_friend_analytics(&account.database(), filters).map_err(to_message)
}

#[tauri::command]
pub fn get_friend_details(
    accounts: State<'_, AccountContextManager>,
    bnet_id: String,
    filters: FriendAnalyticsFilters,
) -> Result<FriendDetail, String> {
    let account = active_account(&accounts)?;
    query_friend_detail(&account.database(), &bnet_id, filters).map_err(to_message)
}

#[tauri::command]
pub fn get_onboarding_status(
    accounts: State<'_, AccountContextManager>,
    request: OnboardingStatusRequest,
) -> Result<OnboardingStatus, String> {
    let account = active_account(&accounts)?;
    let mut status =
        crate::get_onboarding_status(&account.database(), request).map_err(to_message)?;
    status.credential_status =
        crate::credentials::get_credential_status_for_account(account.paths())
            .map_err(to_message)?;
    Ok(status)
}

#[tauri::command]
pub fn get_personal_season_review(
    accounts: State<'_, AccountContextManager>,
    request: PersonalReviewRequest,
) -> Result<PersonalSeasonReview, String> {
    let account = active_account(&accounts)?;
    crate::get_personal_season_review(&account.database(), request).map_err(to_message)
}

#[tauri::command]
pub fn get_credential_status(
    accounts: State<'_, AccountContextManager>,
) -> Result<CredentialStatus, String> {
    let account = active_account(&accounts)?;
    crate::credentials::get_credential_status_for_account(account.paths()).map_err(to_message)
}

#[tauri::command]
pub fn get_deepseek_key_status(
    manager: State<'_, DeepSeekKeyManager>,
) -> Result<DeepSeekKeyStatus, String> {
    manager.status().map_err(to_message)
}

#[tauri::command]
pub async fn save_deepseek_key(
    api_key: String,
    manager: State<'_, DeepSeekKeyManager>,
) -> Result<DeepSeekKeyStatus, String> {
    manager.save(&api_key).await.map_err(to_message)
}

#[tauri::command]
pub async fn generate_match_ai_roast(
    accounts: State<'_, AccountContextManager>,
    keys: State<'_, DeepSeekKeyManager>,
    coordinator: State<'_, AiRoastCoordinator>,
    match_id: String,
) -> Result<AiRoastGenerationResult, String> {
    let account = active_account(&accounts)?;
    let request = CapturedRoastRequest::new(account.database(), match_id)?;
    let client = DeepSeekRoastClient::new().map_err(|_| {
        "\u{65e0}\u{6cd5}\u{521d}\u{59cb}\u{5316} DeepSeek \u{670d}\u{52a1}\u{3002}".to_string()
    })?;
    coordinator.generate(request, &keys, &client).await
}

#[tauri::command]
pub fn get_player_profile(
    accounts: State<'_, AccountContextManager>,
) -> Result<PlayerProfile, String> {
    let account = active_account(&accounts)?;
    let role_id = crate::credentials::get_credential_status_for_account(account.paths())
        .map_err(to_message)?
        .role_id;
    crate::settings::get_player_profile(
        &account.database(),
        &account.paths().player_profile(),
        role_id,
    )
    .map_err(to_message)
}

#[tauri::command]
pub fn get_user_settings(
    accounts: State<'_, AccountContextManager>,
) -> Result<UserSettings, String> {
    crate::settings::get_user_settings_for_path(&accounts.shared_paths().user_settings())
        .map_err(to_message)
}

#[tauri::command]
pub fn save_user_settings(
    accounts: State<'_, AccountContextManager>,
    settings: UserSettings,
) -> Result<UserSettings, String> {
    crate::settings::save_user_settings_for_path(&accounts.shared_paths().user_settings(), settings)
        .map_err(to_message)
}

#[tauri::command]
pub fn check_app_update(
    accounts: State<'_, AccountContextManager>,
) -> Result<UpdateCheckResult, String> {
    let _shared = accounts.shared_paths();
    Ok(crate::settings::check_app_update_result(env!(
        "CARGO_PKG_VERSION"
    )))
}

#[tauri::command]
pub async fn add_account(
    app: AppHandle,
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_target_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
) -> Result<AccountLoginResult, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let shared = accounts.shared_paths().clone();
    let capture_app = app.clone();
    let result = crate::account_login::capture_and_publish_add(
        &accounts,
        &app,
        move || async move {
            let outcome = crate::credentials::capture_dashen_login(capture_app, &shared).await?;
            Ok(crate::account_login::LoginCapture {
                credentials: outcome.credentials,
                battle_tag: outcome.detected_battle_tag,
            })
        },
        crate::credentials::login_cancelled,
    )
    .await
    .map_err(to_message)?;
    reconcile_published_login_target(
        &accounts,
        login_target_coordinator.inner(),
        &result,
        DASHEN_ROOT_BASE,
    )
    .await?;
    Ok(result)
}

#[tauri::command]
pub async fn relogin_account(
    app: AppHandle,
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_target_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
    account_id: String,
) -> Result<AccountLoginResult, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account_id = crate::account_store::AccountId::parse(&account_id)
        .map_err(|_| "账号标识无效。".to_string())?;
    let shared = accounts.shared_paths().clone();
    let capture_app = app.clone();
    let result = crate::account_login::capture_and_publish_relogin(
        &accounts,
        &account_id,
        &app,
        move || async move {
            let outcome = crate::credentials::capture_dashen_login(capture_app, &shared).await?;
            Ok(crate::account_login::LoginCapture {
                credentials: outcome.credentials,
                battle_tag: outcome.detected_battle_tag,
            })
        },
        crate::credentials::login_cancelled,
    )
    .await
    .map_err(to_message)?;
    reconcile_published_login_target(
        &accounts,
        login_target_coordinator.inner(),
        &result,
        DASHEN_ROOT_BASE,
    )
    .await?;
    Ok(result)
}

async fn reconcile_published_login_target(
    accounts: &AccountContextManager,
    coordinator: &crate::login_target::LoginTargetCoordinator,
    result: &AccountLoginResult,
    root_base_url: &str,
) -> Result<(), String> {
    if !matches!(
        result.kind.as_str(),
        "newAccountAdded" | "existingAccountUpdated"
    ) {
        return Ok(());
    }
    let account_id = result
        .account_id
        .as_deref()
        .and_then(|value| crate::account_store::AccountId::parse(value).ok())
        .ok_or_else(|| "账号登录结果不完整，请重试。".to_string())?;
    let account = accounts.snapshot_for(&account_id).map_err(to_message)?;
    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())
        .map_err(to_message)?;
    let target_path = crate::player_target::player_target_path_for_account(account.paths());
    crate::login_target::reconcile_detected_target(
        coordinator,
        account.account_id(),
        &credentials,
        result.battle_tag.as_deref(),
        &target_path,
        root_base_url,
    )
    .await;
    Ok(())
}

#[tauri::command]
pub fn list_accounts(
    accounts: State<'_, AccountContextManager>,
) -> Result<crate::AccountManagerStatus, String> {
    accounts.status().map_err(to_message)
}

#[tauri::command]
pub fn switch_account(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    account_id: String,
) -> Result<crate::AccountManagerStatus, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account_id = crate::account_store::AccountId::parse(&account_id)
        .map_err(|_| "账号标识无效。".to_string())?;
    accounts
        .switch_active(&account_id, &chrono::Utc::now().to_rfc3339())
        .map_err(to_message)
}

#[tauri::command]
pub async fn remove_account(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_target_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
    background_sync: State<'_, crate::BackgroundSeasonSyncManager>,
    account_id: String,
) -> Result<crate::RemoveAccountResult, String> {
    let account_id = crate::account_store::AccountId::parse(&account_id)
        .map_err(|_| "账号标识无效。".to_string())?;
    background_sync.stop_account(account_id.as_str()).await;
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let result = accounts
        .remove_account(&account_id, &chrono::Utc::now().to_rfc3339())
        .map_err(to_message)?;
    clear_removed_account_target(&login_target_coordinator, &account_id, &result);
    Ok(result)
}

fn clear_removed_account_target(
    coordinator: &crate::login_target::LoginTargetCoordinator,
    account_id: &crate::account_store::AccountId,
    result: &crate::RemoveAccountResult,
) {
    if result.removed {
        coordinator.clear(account_id);
    }
}

#[tauri::command]
pub fn cancel_dashen_login(app: AppHandle) -> Result<CredentialLoginCancelResult, String> {
    crate::credentials::cancel_dashen_login(app).map_err(to_message)
}

#[tauri::command]
pub fn get_player_target_status(
    accounts: State<'_, AccountContextManager>,
) -> Result<PlayerTargetStatus, String> {
    let account = active_account(&accounts)?;
    crate::player_target::get_player_target_status_for_account(account.paths()).map_err(to_message)
}

#[tauri::command]
pub fn get_login_target_status(
    accounts: State<'_, AccountContextManager>,
    coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String> {
    let account = active_account(&accounts)?;
    let credentials =
        crate::credentials::read_optional_saved_credentials_for_account(account.paths())
            .map_err(to_message)?;
    let target_path = crate::player_target::player_target_path_for_account(account.paths());
    Ok(coordinator.status_or_persisted(account.account_id(), credentials.as_ref(), &target_path))
}

#[tauri::command]
pub async fn confirm_login_target_switch(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_target_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account = active_account(&accounts)?;
    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())
        .map_err(to_message)?;
    let target_path = crate::player_target::player_target_path_for_account(account.paths());
    Ok(crate::login_target::confirm_switch(
        login_target_coordinator.inner(),
        account.account_id(),
        &credentials,
        &target_path,
        DASHEN_ROOT_BASE,
    )
    .await)
}

#[tauri::command]
pub fn keep_current_player_target(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_target_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account = active_account(&accounts)?;
    login_target_coordinator
        .keep_current(account.account_id())
        .map_err(to_message)
}

#[tauri::command]
pub async fn retry_login_target_binding(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_target_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
) -> Result<LoginTargetStatus, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account = active_account(&accounts)?;
    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())
        .map_err(to_message)?;
    let target_path = crate::player_target::player_target_path_for_account(account.paths());
    Ok(crate::login_target::retry_binding(
        login_target_coordinator.inner(),
        account.account_id(),
        &credentials,
        &target_path,
        DASHEN_ROOT_BASE,
    )
    .await)
}

#[tauri::command]
pub fn clear_player_target(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_target_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
) -> Result<ClearPlayerTargetResult, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account = active_account(&accounts)?;
    let cleared = crate::player_target::clear_player_target_for_account(account.paths())
        .map_err(to_message)?;
    let source_role_id =
        crate::credentials::read_optional_saved_credentials_for_account(account.paths())
            .map_err(to_message)?
            .as_ref()
            .map(|credentials| credentials.role_id);
    Ok(ClearPlayerTargetResult {
        cleared,
        target: login_target_coordinator.target_cleared(account.account_id(), source_role_id),
    })
}

#[tauri::command]
pub fn get_rankings(
    accounts: State<'_, AccountContextManager>,
    season: Option<i64>,
) -> Result<RankingResponse, String> {
    let account = active_account(&accounts)?;
    crate::ranking::get_rankings(&account.database(), season).map_err(to_message)
}

#[tauri::command]
pub fn get_season_analysis(
    accounts: State<'_, AccountContextManager>,
    season: Option<i64>,
) -> Result<SeasonAnalysis, String> {
    let account = active_account(&accounts)?;
    crate::get_season_analysis(&account.database(), season).map_err(to_message)
}

#[tauri::command]
pub fn get_career_mmr_overview(
    accounts: State<'_, AccountContextManager>,
) -> Result<crate::CareerMmrOverview, String> {
    let account = active_account(&accounts)?;
    crate::career_mmr::query::get_career_mmr_overview(&account.database()).map_err(to_message)
}

#[tauri::command]
pub fn get_role_mmr_history(
    accounts: State<'_, AccountContextManager>,
    role: String,
    season: Option<i64>,
    from: Option<i64>,
    to: Option<i64>,
    granularity: Option<String>,
    limit: Option<usize>,
    before: Option<i64>,
    before_hash: Option<String>,
    before_bucket: Option<i64>,
    before_season: Option<i64>,
) -> Result<crate::CareerMmrHistory, String> {
    let account = active_account(&accounts)?;
    crate::career_mmr::query::get_role_mmr_history(
        &account.database(),
        crate::CareerMmrHistoryRequest {
            role,
            season,
            from,
            to,
            granularity,
            limit,
            before,
            before_hash,
            before_bucket,
            before_season,
        },
    )
    .map_err(to_message)
}

#[tauri::command]
pub fn get_role_mmr_mode_breakdown(
    accounts: State<'_, AccountContextManager>,
    role: String,
) -> Result<crate::CareerMmrModeBreakdown, String> {
    let account = active_account(&accounts)?;
    crate::career_mmr::query::get_role_mmr_mode_breakdown(&account.database(), &role)
        .map_err(to_message)
}

#[tauri::command]
pub fn get_role_mmr_season_breakdown(
    accounts: State<'_, AccountContextManager>,
    role: String,
) -> Result<crate::CareerMmrSeasonBreakdown, String> {
    let account = active_account(&accounts)?;
    crate::career_mmr::query::get_role_mmr_season_breakdown(&account.database(), &role)
        .map_err(to_message)
}

#[tauri::command]
pub fn get_mmr_match_explanation(
    accounts: State<'_, AccountContextManager>,
    match_id: String,
    role: String,
) -> Result<crate::CareerMmrMatchExplanation, String> {
    let account = active_account(&accounts)?;
    crate::career_mmr::query::get_mmr_match_explanation(&account.database(), &match_id, &role)
        .map_err(to_message)
}

#[tauri::command]
pub fn get_recent_mmr_explanations(
    accounts: State<'_, AccountContextManager>,
    role: String,
    limit: usize,
) -> Result<crate::CareerMmrRecentExplanations, String> {
    let account = active_account(&accounts)?;
    crate::career_mmr::query::get_recent_mmr_explanations(&account.database(), &role, limit)
        .map_err(to_message)
}

#[tauri::command]
pub fn rebuild_career_mmr(
    accounts: State<'_, AccountContextManager>,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
) -> Result<crate::career_mmr::replay::CareerMmrBuildSummary, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account = active_account(&accounts)?;
    crate::career_mmr::replay::rebuild_career_mmr_active(&account.database()).map_err(to_message)
}

#[tauri::command]
pub fn get_career_mmr_build_status(
    accounts: State<'_, AccountContextManager>,
) -> Result<crate::CareerMmrBuildStatus, String> {
    let account = active_account(&accounts)?;
    crate::career_mmr::query::get_career_mmr_build_status(&account.database()).map_err(to_message)
}

#[tauri::command]
pub async fn sync_season(
    app: AppHandle,
    accounts: State<'_, AccountContextManager>,
    request: SyncRequest,
    operation_coordinator: State<'_, crate::sync_coordinator::SyncCoordinator>,
    login_coordinator: State<'_, crate::login_target::LoginTargetCoordinator>,
    background_sync: State<'_, crate::BackgroundSeasonSyncManager>,
) -> Result<SeasonSyncResult, String> {
    let _permit = operation_coordinator
        .try_acquire_account_operation()
        .map_err(str::to_string)?;
    let account = active_account(&accounts)?;
    let root = account.paths().seasons();
    let credential_path = account.paths().credentials();
    let target_path = account.paths().player_target();
    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())
        .map_err(to_message)?;
    crate::login_target::ensure_sync_target_ready(
        login_coordinator.inner(),
        account.account_id(),
        &credentials,
        &target_path,
    )
    .map_err(to_message)?;
    let db = account.database();
    let store = crate::SeasonSyncStore::new(db.clone());
    if store.active_job_count(request.season).map_err(to_message)? > 0 {
        return Err("该赛季仍在后台处理中，请等待完成后再试。".to_string());
    }
    let emit_app = app.clone();
    match request.mode {
        crate::SyncMode::QuickUpdate => {
            let summary = quick_list_update_with_credentials_and_target_paths_base_url(
                &db,
                &credential_path,
                &target_path,
                request.season,
                "https://datamsapi.ds.163.com/v1/a19ld5tool/customer",
            )
            .await
            .map_err(to_message)?;
            drop(_permit);
            background_sync.wake(
                account.account_id().as_str(),
                account.paths().clone(),
                request.season,
                background_event_sink(&app),
            );
            Ok(SeasonSyncResult::QuickList(summary))
        }
        crate::SyncMode::FullFetch => {
            sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
                &db,
                &root,
                &credential_path,
                &target_path,
                request.season,
                request.mode,
                "https://datamsapi.ds.163.com/v1/a19ld5tool/customer",
                move |progress| {
                    let _ = emit_app.emit("sync-progress", progress);
                },
            )
            .await
            .map(SeasonSyncResult::Full)
            .map_err(to_message)
        }
    }
}

#[tauri::command]
pub fn season_sync_status(
    accounts: State<'_, AccountContextManager>,
    season: i64,
) -> Result<crate::SeasonBackgroundStatus, String> {
    let account = active_account(&accounts)?;
    crate::SeasonSyncStore::new(account.database())
        .season_status(season)
        .map_err(to_message)
}

#[tauri::command]
pub fn retry_season_background_sync(
    app: AppHandle,
    accounts: State<'_, AccountContextManager>,
    background_sync: State<'_, crate::BackgroundSeasonSyncManager>,
    season: i64,
) -> Result<bool, String> {
    let account = active_account(&accounts)?;
    let queued = crate::SeasonSyncStore::new(account.database())
        .retry_season(season)
        .map_err(to_message)?;
    if queued {
        background_sync.wake(
            account.account_id().as_str(),
            account.paths().clone(),
            season,
            background_event_sink(&app),
        );
    }
    Ok(queued)
}

fn background_event_sink(app: &AppHandle) -> crate::BackgroundEventSink {
    let app = app.clone();
    Arc::new(move |event, payload| {
        let _ = app.emit(event, payload);
    })
}

fn active_account(
    accounts: &State<'_, AccountContextManager>,
) -> Result<AccountContextSnapshot, String> {
    accounts.active_snapshot().map_err(to_message)
}

pub fn runtime_error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn to_message(error: impl std::fmt::Display) -> String {
    runtime_error_message(error)
}

#[cfg(test)]
mod account_management_tests {
    use super::*;
    use crate::app_paths::SharedAppPaths;

    #[tokio::test]
    async fn published_login_updates_the_account_scoped_target_session() {
        let temp = tempfile::tempdir().unwrap();
        let manager =
            AccountContextManager::load(<SharedAppPaths>::from_app_data_dir(temp.path())).unwrap();
        let result = crate::account_login::publish_add_account(
            &manager,
            crate::account_login::LoginCapture {
                credentials: crate::credential_store::DashenCredentials {
                    role_id: 101101,
                    token: "synthetic-account-token".to_string(),
                    customer_token: None,
                    dts: None,
                    server: 1,
                    saved_at: "2026-07-16T00:00:00Z".to_string(),
                },
                battle_tag: "Synthetic#1011".to_string(),
            },
        )
        .unwrap();
        let account_id = result
            .account_id
            .as_deref()
            .and_then(|value| crate::account_store::AccountId::parse(value).ok())
            .unwrap();
        let account = manager.snapshot_for(&account_id).unwrap();
        crate::player_target::save_player_target_for_path(
            &account.paths().player_target(),
            &crate::player_target::PlayerTarget {
                input_bnet: "Synthetic#1011".to_string(),
                resolved_bnet: Some("Synthetic#1011".to_string()),
                customer_token: "synthetic-player-token".to_string(),
                bound_at: "2026-07-16T00:00:00Z".to_string(),
                source_role_id: Some(101101),
            },
        )
        .unwrap();
        let coordinator = crate::login_target::LoginTargetCoordinator::default();

        reconcile_published_login_target(&manager, &coordinator, &result, "http://127.0.0.1:1")
            .await
            .unwrap();

        let status = coordinator.status(&account_id).unwrap();
        assert_eq!(status.state, "ready");
        assert!(status.sync_allowed);
        assert_eq!(status.detected_bnet.as_deref(), Some("Synthetic#1011"));
    }

    #[test]
    fn removing_one_account_clears_only_its_login_target_session() {
        let coordinator = crate::login_target::LoginTargetCoordinator::default();
        let first =
            crate::account_store::AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let second =
            crate::account_store::AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
        coordinator.target_cleared(&first, Some(101101));
        coordinator.target_cleared(&second, Some(202202));
        let removed = crate::RemoveAccountResult {
            removed: true,
            active_account_id: Some(second.as_str().into()),
            message: "账号已退出。".into(),
            reload_required: true,
        };
        clear_removed_account_target(&coordinator, &first, &removed);
        assert!(coordinator.status(&first).is_none());
        assert!(coordinator.status(&second).is_some());

        coordinator.target_cleared(&first, Some(101101));
        let not_removed = crate::RemoveAccountResult {
            removed: false,
            active_account_id: Some(first.as_str().into()),
            message: "账号未退出。".into(),
            reload_required: false,
        };
        clear_removed_account_target(&coordinator, &first, &not_removed);
        assert!(coordinator.status(&first).is_some());
        assert!(coordinator.status(&second).is_some());
    }
}
