import { invoke } from "@tauri-apps/api/core";
import type {
  FilterOptions,
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
  RankingResponse,
  SeasonAnalysis,
  SyncSummary
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

export function getMatchDetails(matchId: string) {
  return invoke<MatchDetailView>("get_match_details", { matchId });
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
