import { invoke } from "@tauri-apps/api/core";
import type {
  FilterOptions,
  BindPlayerTargetResult,
  ClearPlayerTargetResult,
  CredentialLoginResult,
  CredentialStatus,
  FriendAnalytics,
  FriendAnalyticsFilters,
  FriendDetail,
  MatchDetailView,
  MatchFilters,
  MatchSummary,
  OnboardingStatus,
  Overview,
  PersonalSeasonReview,
  PlayerTargetStatus,
  PlayerProfile,
  PlayerSearchOption,
  RankingResponse,
  SeasonAnalysis,
  SyncSummary,
  LogoutResult,
  UpdateCheckResult,
  UserSettings
} from "./types";

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

export function refreshMatchEvaluations() {
  return invoke<void>("refresh_match_evaluations");
}

export function getFriendAnalytics(filters: FriendAnalyticsFilters) {
  return invoke<FriendAnalytics>("get_friend_analytics", { filters });
}

export function getFriendDetails(bnetId: string, filters: FriendAnalyticsFilters) {
  return invoke<FriendDetail>("get_friend_details", { bnetId, filters });
}

export function syncSeason(season: number) {
  return invoke<SyncSummary>("sync_season", { request: { season } });
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

export function getPlayerTargetStatus() {
  return invoke<PlayerTargetStatus>("get_player_target_status");
}

export function clearPlayerTarget() {
  return invoke<ClearPlayerTargetResult>("clear_player_target");
}

export function bindPlayerTarget(bnet: string) {
  return invoke<BindPlayerTargetResult>("bind_player_target", { request: { bnet } });
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

export function logout() {
  return invoke<LogoutResult>("logout");
}

export function checkAppUpdate() {
  return invoke<UpdateCheckResult>("check_app_update");
}

export function startDashenLogin() {
  return invoke<CredentialLoginResult>("start_dashen_login");
}

export function cancelDashenLogin() {
  return invoke<{ cancelled: boolean; message: string }>("cancel_dashen_login");
}

export function getRankings(season?: number) {
  return invoke<RankingResponse>("get_rankings", { season });
}

export function getSeasonAnalysis(season?: number) {
  return invoke<SeasonAnalysis>("get_season_analysis", { season });
}
