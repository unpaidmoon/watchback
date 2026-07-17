import { invoke } from "@tauri-apps/api/core";
import type {
  AccountLoginResult,
  AccountManagerStatus,
  CareerMmrBuildStatus,
  CareerMmrHistory,
  CareerMmrHistoryRequest,
  CareerMmrModeBreakdown,
  CareerMmrOverview,
  CareerMmrRecentExplanations,
  CareerMmrSeasonBreakdown,
  CareerRole,
  FilterOptions,
  CredentialStatus,
  FriendAnalytics,
  FriendAnalyticsFilters,
  FriendDetail,
  MatchDetailView,
  MatchFavoriteRequest,
  MatchFavoriteView,
  MatchFilters,
  MatchSummary,
  OnboardingStatus,
  Overview,
  PersonalSeasonReview,
  PlayerProfile,
  PlayerTargetStatus,
  LoginTargetStatus,
  ClearPlayerTargetResult,
  PlayerSearchOption,
  RankingResponse,
  RemoveAccountResult,
  SeasonAnalysis,
  SyncMode,
  SeasonBackgroundStatus,
  SeasonSyncResult,
  SyncSummary,
  UpdateCheckResult,
  UserSettings,
} from "./types";

export const listAccounts = () => invoke<AccountManagerStatus>("list_accounts");

export const addAccount = () => invoke<AccountLoginResult>("add_account");

export const switchAccount = (accountId: string) =>
  invoke<AccountManagerStatus>("switch_account", { accountId });

export const reloginAccount = (accountId: string) =>
  invoke<AccountLoginResult>("relogin_account", { accountId });

export const removeAccount = (accountId: string) =>
  invoke<RemoveAccountResult>("remove_account", { accountId });

export function getOverview() {
  return invoke<Overview>("get_overview");
}

export function listMatchFilters() {
  return invoke<FilterOptions>("list_match_filters");
}

export function listMatchTimeline(filters: MatchFilters) {
  return invoke<MatchSummary[]>("list_match_timeline", { filters });
}

export function searchMatchPlayerOptions(query: string) {
  return invoke<PlayerSearchOption[]>("search_match_player_options", { query });
}

export function getMatchDetails(matchId: string) {
  return invoke<MatchDetailView>("get_match_details", { matchId });
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

export function getFriendAnalytics(filters: FriendAnalyticsFilters) {
  return invoke<FriendAnalytics>("get_friend_analytics", { filters });
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

export function getCredentialStatus() {
  return invoke<CredentialStatus>("get_credential_status");
}

export function getPlayerProfile() {
  return invoke<PlayerProfile>("get_player_profile");
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

export function getPlayerTargetStatus() {
  return invoke<PlayerTargetStatus>("get_player_target_status");
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

export function getCareerMmrOverview() {
  return invoke<CareerMmrOverview>("get_career_mmr_overview");
}

export function getRoleMmrHistory(request: CareerMmrHistoryRequest) {
  return invoke<CareerMmrHistory>("get_role_mmr_history", { ...request });
}

export function getRoleMmrModeBreakdown(role: CareerRole) {
  return invoke<CareerMmrModeBreakdown>("get_role_mmr_mode_breakdown", { role });
}

export function getRoleMmrSeasonBreakdown(role: CareerRole) {
  return invoke<CareerMmrSeasonBreakdown>("get_role_mmr_season_breakdown", { role });
}

export function getCareerMmrBuildStatus() {
  return invoke<CareerMmrBuildStatus>("get_career_mmr_build_status");
}

export function rebuildCareerMmr() {
  return invoke("rebuild_career_mmr");
}

export function getRecentMmrExplanations(role: CareerRole, limit = 4) {
  return invoke<CareerMmrRecentExplanations>("get_recent_mmr_explanations", { role, limit });
}
