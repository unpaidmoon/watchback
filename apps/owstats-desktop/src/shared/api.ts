import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AccountLoginResult,
  AccountManagerStatus,
  AiRoastGenerationResult,
  AiRoastStreamEvent,
  CareerAchievement,
  FilterOptions,
  DeepSeekKeyStatus,
  FriendAnalyticsFilters,
  FriendDetail,
  MatchDetailView,
  MatchCursor,
  MatchFavoriteRequest,
  MatchFavoriteView,
  MatchFilters,
  MatchPage,
  MatchSummary,
  OnboardingStatus,
  Overview,
  PersonalSeasonReview,
  PlayerGameAction,
  PlayerProfile,
  QrAccountLoginStartResult,
  QrAccountLoginSettleResult,
  QrAccountLoginActiveResult,
  AccountOperationStatus,
  QrAccountLoginSnapshot,
  LoginTargetStatus,
  ClearPlayerTargetResult,
  PlayerSearchOption,
  RankingResponse,
  RemoveAccountResult,
  SeasonAnalysis,
  SyncMode,
  SeasonBackgroundStatus,
  SeasonSyncResult,
  UpdateCheckResult,
  UserSettings,
} from "./types";

export const listAccounts = () => invoke<AccountManagerStatus>("list_accounts");

export const addAccount = () => invoke<AccountLoginResult>("add_account");

export const addAccountWithQr = (requestId: string) =>
  invoke<QrAccountLoginStartResult>("add_account_with_qr", { requestId });

export const refreshQrAccountLogin = (sessionId: string, requestId: string) =>
  invoke<QrAccountLoginStartResult>("refresh_dashen_login_qr", { sessionId, requestId });

export const getQrAccountLoginSnapshot = (sessionId: string) =>
  invoke<QrAccountLoginSnapshot>("get_dashen_login_qr_snapshot", { sessionId });

export const cancelQrAccountLogin = (sessionId: string) =>
  invoke<QrAccountLoginSettleResult>("cancel_dashen_login_qr", { sessionId });

export const getActiveQrAccountLogin = (requestId: string) =>
  invoke<QrAccountLoginActiveResult | null>("get_active_dashen_login_qr", { requestId });

export const getAccountOperationStatus = () =>
  invoke<AccountOperationStatus>("get_operation_status");

export const switchAccount = (accountId: string) =>
  invoke<AccountManagerStatus>("switch_account", { accountId });

export const removeAccount = (accountId: string) =>
  invoke<RemoveAccountResult>("remove_account", { accountId });

export function getOverview() {
  return invoke<Overview>("get_overview");
}

export function openSupportDirectory(kind: "logs" | "data") {
  return invoke<void>("open_support_directory", { kind });
}

export function openProjectRepository() {
  return invoke<void>("open_project_repository");
}

export function openProductFeedback() {
  return invoke<void>("open_product_feedback");
}

export function listMatchFilters() {
  return invoke<FilterOptions>("list_match_filters");
}

export function listMatchTimeline(filters: MatchFilters) {
  return invoke<MatchSummary[]>("list_match_timeline", { filters });
}

export function listMatchTimelinePage(filters: MatchFilters, cursor: MatchCursor | null = null, limit = 50) {
  return invoke<MatchPage>("list_match_timeline_page", { filters, cursor, limit });
}

export function searchMatchPlayerOptions(query: string) {
  return invoke<PlayerSearchOption[]>("search_match_player_options", { query });
}

export function getMatchDetails(matchId: string) {
  return invoke<MatchDetailView>("get_match_details", { matchId });
}

export function getDeepSeekKeyStatus() {
  return invoke<DeepSeekKeyStatus>("get_deepseek_key_status");
}

export function saveDeepSeekKey(apiKey: string) {
  return invoke<DeepSeekKeyStatus>("save_deepseek_key", { apiKey });
}

export function generateMatchAiRoast(
  matchId: string,
  regenerate = false,
  onEvent?: (event: AiRoastStreamEvent) => void,
) {
  const stream = new Channel<AiRoastStreamEvent>(onEvent);
  return invoke<AiRoastGenerationResult>("generate_match_ai_roast", {
    matchId,
    regenerate,
    onEvent: stream,
  });
}

export function saveMatchFavorite(request: MatchFavoriteRequest) {
  return invoke<MatchFavoriteView>("save_match_favorite", { request });
}

export function deleteMatchFavorite(matchId: string) {
  return invoke<void>("delete_match_favorite", { matchId });
}

export function refreshMatchEvaluations() {
  return invoke<void>("refresh_match_evaluations");
}

export function getFriendDetails(bnetId: string, filters: FriendAnalyticsFilters) {
  return invoke<FriendDetail>("get_friend_details", { bnetId, filters });
}

export function syncSeason(season: number, mode: SyncMode) {
  return invoke<SeasonSyncResult>("sync_season", { request: { season, mode } });
}

export function getSeasonSyncStatus(season: number) {
  return invoke<SeasonBackgroundStatus>("season_sync_status", { season });
}

export function retrySeasonBackgroundSync(season: number) {
  return invoke<boolean>("retry_season_background_sync", { season });
}

export function getOnboardingStatus(season?: number) {
  return invoke<OnboardingStatus>("get_onboarding_status", { request: { season } });
}

export function getPersonalSeasonReview(season?: number) {
  return invoke<PersonalSeasonReview>("get_personal_season_review", { request: { season } });
}

export function getCareerAchievements() {
  return invoke<CareerAchievement[]>("get_career_achievements");
}

export function getPlayerProfile() {
  return invoke<PlayerProfile>("get_player_profile");
}

export function refreshProfileCard() {
  return invoke<PlayerProfile>("refresh_profile_card");
}

export function getPlayerGameAction() {
  return invoke<PlayerGameAction>("get_player_game_action");
}

export function getUserSettings() {
  return invoke<UserSettings>("get_user_settings");
}

export function saveUserSettings(settings: UserSettings) {
  return invoke<UserSettings>("save_user_settings", { settings });
}

export function checkAppUpdate() {
  return invoke<UpdateCheckResult>("check_app_update");
}

export function cancelDashenLogin() {
  return invoke<{ cancelled: boolean; message: string }>("cancel_dashen_login");
}

export function getLoginTargetStatus() {
  return invoke<LoginTargetStatus>("get_login_target_status");
}

export function confirmLoginTargetSwitch() {
  return invoke<LoginTargetStatus>("confirm_login_target_switch");
}

export function keepCurrentPlayerTarget() {
  return invoke<LoginTargetStatus>("keep_current_player_target");
}

export function retryLoginTargetBinding() {
  return invoke<LoginTargetStatus>("retry_login_target_binding");
}

export function clearPlayerTarget() {
  return invoke<ClearPlayerTargetResult>("clear_player_target");
}

export function getRankings(season?: number) {
  return invoke<RankingResponse>("get_rankings", { season });
}

export function getSeasonAnalysis(season?: number) {
  return invoke<SeasonAnalysis>("get_season_analysis", { season });
}
