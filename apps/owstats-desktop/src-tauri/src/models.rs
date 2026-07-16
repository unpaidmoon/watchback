use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrOverview {
    pub model_version: Option<i64>,
    pub composite_mmr: Option<i64>,
    pub roles: Vec<CareerMmrRoleSummary>,
    pub discovered_seasons: Vec<i64>,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrRoleSummary {
    pub role: String,
    pub current_mmr: Option<i64>,
    pub confidence: f64,
    pub confidence_label: String,
    pub peak_mmr: Option<i64>,
    pub peak_at: Option<i64>,
    pub effective_matches: f64,
    pub effective_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrHistoryRequest {
    pub role: String,
    pub season: Option<i64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub granularity: Option<String>,
    pub limit: Option<usize>,
    pub before: Option<i64>,
    pub before_hash: Option<String>,
    pub before_bucket: Option<i64>,
    pub before_season: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrHistory {
    pub role: String,
    pub granularity: String,
    pub points: Vec<CareerMmrHistoryPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrHistoryPoint {
    pub timestamp: i64,
    pub season: i64,
    pub mmr: i64,
    pub confidence: f64,
    pub delta: f64,
    pub cursor_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrModeBreakdown {
    pub role: String,
    pub items: Vec<CareerMmrModeItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrModeItem {
    pub mode_family: String,
    pub matches_count: i64,
    pub duration_sec: i64,
    pub wins: i64,
    pub losses: i64,
    pub draws: i64,
    pub cumulative_delta: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrSeasonBreakdown {
    pub role: String,
    pub items: Vec<CareerMmrSeasonItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrSeasonItem {
    pub season: i64,
    pub matches_count: i64,
    pub start_mmr: i64,
    pub end_mmr: i64,
    pub peak_mmr: i64,
    pub net_delta: f64,
    pub first_match_at: Option<i64>,
    pub last_match_at: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrMatchExplanation {
    pub role: String,
    pub summary: String,
    pub factors: Vec<CareerMmrExplanationFactor>,
    pub data_quality: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrRecentExplanations {
    pub role: String,
    pub items: Vec<CareerMmrRecentExplanation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrRecentExplanation {
    pub timestamp: i64,
    pub season: i64,
    pub delta: f64,
    pub summary: String,
    pub factors: Vec<CareerMmrExplanationFactor>,
    pub data_quality: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrExplanationFactor {
    pub kind: String,
    pub label: String,
    pub value: f64,
    pub unit: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrBuildStatus {
    pub model_version: Option<i64>,
    pub summary_count: i64,
    pub is_built: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub season: i64,
    pub matches_seen: usize,
    pub details_seen: usize,
    pub preset_matches: i64,
    pub stadium_matches: i64,
    pub imported_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mmr_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchFilters {
    pub season: Option<i64>,
    pub hero: Option<String>,
    pub map: Option<String>,
    pub result: Option<String>,
    pub mode: Option<String>,
    pub primary_tag: Option<String>,
    pub player_name: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchEvaluationSummary {
    pub grade: String,
    pub primary_tag: String,
    pub evidence_tags: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchPerformanceAward {
    pub code: String,
    pub label: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    pub match_id: String,
    pub season: i64,
    pub bucket: String,
    pub match_kind: String,
    pub game_mode: String,
    pub mode_label: String,
    pub result: String,
    pub result_label: String,
    pub map_guid: String,
    pub map_preview_guid: Option<String>,
    pub map_name: String,
    pub hero_guid: String,
    pub hero_name: String,
    pub begin_ts: i64,
    pub begin_label: String,
    pub team_score: i64,
    pub opponent_score: i64,
    pub kills: i64,
    pub assists: i64,
    pub deaths: i64,
    pub damage: i64,
    pub healing: i64,
    pub mitigation: i64,
    pub duration_sec: i64,
    pub evaluation: Option<MatchEvaluationSummary>,
    pub team_awards: Vec<MatchPerformanceAward>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRow {
    pub side: String,
    pub name: String,
    pub bnet_id: String,
    pub rank_label: Option<String>,
    pub rank_key: Option<String>,
    pub hero_guid: String,
    pub hero_name: String,
    pub kills: i64,
    pub assists: i64,
    pub deaths: i64,
    pub damage: i64,
    pub healing: i64,
    pub mitigation: i64,
    pub final_hits: i64,
    pub target_time_sec: i64,
    pub healing_taken: i64,
    pub damage_taken: i64,
    pub worth: Option<i64>,
    pub end_worth: Option<i64>,
    pub worth_max: bool,
    pub mod_guids: Vec<String>,
    pub trait_guids: Vec<String>,
    pub has_source_max_flags: bool,
    pub kill_max: bool,
    pub hero_damage_max: bool,
    pub cure_max: bool,
    pub resist_damage_max: bool,
    pub perks: Vec<PlayerPerk>,
    pub hero_stats: Vec<HeroStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPerk {
    pub guid: String,
    pub perk_level: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeroStat {
    pub guid: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetailView {
    pub match_id: String,
    pub game_mode: Option<String>,
    pub mode_label: Option<String>,
    pub result: Option<String>,
    pub result_label: Option<String>,
    pub map_guid: Option<String>,
    pub map_name: Option<String>,
    pub hero_guid: Option<String>,
    pub hero_name: Option<String>,
    pub begin_ts: Option<i64>,
    pub begin_label: Option<String>,
    pub team_score: Option<i64>,
    pub opponent_score: Option<i64>,
    pub duration_sec: Option<i64>,
    pub teammates: Vec<PlayerRow>,
    pub enemies: Vec<PlayerRow>,
    pub arena_detail: Option<ArenaMatchDetail>,
    pub evaluation: Option<MatchEvaluationSummary>,
    pub team_awards: Vec<MatchPerformanceAward>,
    pub same_role_comparison: Option<SameRoleComparison>,
    pub raw_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArenaMatchDetail {
    pub summary: ArenaDetailSlice,
    pub rounds: Vec<ArenaDetailSlice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArenaDetailSlice {
    pub key: String,
    pub label: String,
    pub result: Option<String>,
    pub result_label: Option<String>,
    pub team_score: Option<i64>,
    pub opponent_score: Option<i64>,
    pub duration_sec: Option<i64>,
    pub begin_ts: Option<i64>,
    pub begin_label: Option<String>,
    pub map_guid: Option<String>,
    pub map_name: Option<String>,
    pub teammates: Vec<PlayerRow>,
    pub enemies: Vec<PlayerRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SameRoleComparison {
    pub raw_role: Option<String>,
    pub role_label: Option<String>,
    pub current: ComparisonMetrics,
    pub average: ComparisonMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonMetrics {
    pub kills: Option<f64>,
    pub deaths: Option<f64>,
    pub damage: Option<f64>,
    pub healing: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptions {
    pub seasons: Vec<i64>,
    pub heroes: Vec<FilterOption>,
    pub maps: Vec<FilterOption>,
    pub results: Vec<String>,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FilterOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSearchOption {
    pub player_name: String,
    pub matches: i64,
    pub latest_begin_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FriendAnalyticsFilters {
    pub season: Option<i64>,
    pub hero: Option<String>,
    pub map: Option<String>,
    pub result: Option<String>,
    pub mode: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendAnalyticsSummary {
    pub season: Option<i64>,
    pub preset_matches: i64,
    pub friend_rows: i64,
    pub eligible_friends: usize,
    pub min_sample: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendAnalytics {
    pub summary: FriendAnalyticsSummary,
    pub boards: Vec<FriendBoard>,
    pub common_friend_heroes: Vec<CommonFriendHero>,
    pub lineup_combinations: Vec<LineupCombination>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommonFriendHero {
    pub bnet_id: String,
    pub name: String,
    pub representative_hero_guid: String,
    pub representative_hero_name: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f64,
    pub latest_begin_ts: i64,
    pub intimacy_score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineupCombination {
    pub key: String,
    pub members: Vec<LineupMember>,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineupMember {
    pub bnet_id: Option<String>,
    pub name: String,
    pub hero_guid: String,
    pub hero_name: String,
    pub is_current_player: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendBoard {
    pub key: String,
    pub label: String,
    pub description: String,
    pub friends: Vec<FriendBoardEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendBoardEntry {
    pub bnet_id: String,
    pub name: String,
    pub board_key: String,
    pub board_label: String,
    pub same_team_matches: i64,
    pub win_rate: f64,
    pub avg_damage_taken: f64,
    pub avg_healing_taken: f64,
    pub avg_mitigation: f64,
    pub avg_deaths: f64,
    pub avg_kills: f64,
    pub avg_assists: f64,
    pub avg_damage: f64,
    pub avg_healing: f64,
    pub score: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendDetail {
    pub bnet_id: String,
    pub name: String,
    pub boards: Vec<FriendBoardEntry>,
    pub common_heroes: Vec<FriendBreakdown>,
    pub common_maps: Vec<FriendBreakdown>,
    pub recent_matches: Vec<FriendRecentMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendBreakdown {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendRecentMatch {
    pub match_id: String,
    pub begin_label: String,
    pub result: String,
    pub map_name: String,
    pub hero_name: String,
    pub kills: i64,
    pub assists: i64,
    pub deaths: i64,
    pub damage: i64,
    pub healing: i64,
    pub damage_taken: i64,
    pub healing_taken: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    pub season: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub season: i64,
    pub phase: String,
    pub processed: usize,
    pub total: usize,
    pub current_match_id: Option<String>,
    pub imported_matches: usize,
    pub imported_details: usize,
    pub skipped_duplicates: usize,
    pub failed_details: usize,
    pub latest_errors: Vec<String>,
    pub import_summary: ImportSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub season: i64,
    pub phase: String,
    pub processed: usize,
    pub total: usize,
    pub current_match_id: Option<String>,
    pub failed_details: usize,
    pub latest_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTargetStatus {
    pub state: String,
    pub input_bnet: Option<String>,
    pub resolved_bnet: Option<String>,
    pub bound_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginTargetStatus {
    pub state: String,
    pub detected_bnet: Option<String>,
    pub current_bnet: Option<String>,
    pub message: String,
    pub sync_allowed: bool,
    pub can_confirm_switch: bool,
    pub can_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClearPlayerTargetResult {
    pub cleared: bool,
    pub target: LoginTargetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PersonalReviewRequest {
    pub season: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatusRequest {
    pub season: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonalSeasonReview {
    pub available_seasons: Vec<i64>,
    pub summary: ReviewSummary,
    pub baseline: SeasonBaseline,
    pub role_summaries: Vec<RoleRankSummary>,
    pub stadium_role_summaries: Vec<RoleRankSummary>,
    pub phase5_highlights: Phase5ReviewHighlights,
    pub top_conclusions: Vec<ReviewConclusion>,
    pub personal_bests: Vec<PersonalBest>,
    pub streaks: StreakSummary,
    pub hero_pool: HeroPoolStability,
    pub maps: MapAdaptability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleRankSummary {
    pub role_type: String,
    pub role_label: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f64,
    pub kda: f64,
    pub rank_level: i64,
    pub rank_sub_tier: i64,
    pub rank_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Phase5ReviewHighlights {
    pub carry_count: i64,
    pub try_hard_count: i64,
    pub s_grade_count: i64,
    pub longest_win_streak: i64,
    pub most_common_highlight: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    pub season: Option<i64>,
    pub total_matches: i64,
    pub baseline_matches: i64,
    pub wins: i64,
    pub losses: i64,
    pub win_rate: f64,
    pub avg_kills: f64,
    pub avg_deaths: f64,
    pub avg_damage: f64,
    pub avg_healing: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeasonBaseline {
    pub match_count: i64,
    pub excluded_total: i64,
    pub short_match_threshold_sec: i64,
    pub exclusion_reasons: Vec<BaselineExclusionReason>,
    pub avg_kills: f64,
    pub avg_deaths: f64,
    pub avg_damage: f64,
    pub avg_healing: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BaselineExclusionReason {
    pub reason: String,
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewConclusion {
    pub kind: String,
    pub tone: String,
    pub title: String,
    pub body: String,
    pub priority: i64,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceLink {
    pub kind: String,
    pub label: String,
    pub route: String,
    pub match_id: Option<String>,
    pub hero: Option<String>,
    pub map: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonalBest {
    pub metric: String,
    pub label: String,
    pub value: i64,
    pub match_id: String,
    pub result: String,
    pub result_label: String,
    pub hero_name: String,
    pub map_name: String,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreakSummary {
    pub longest_win_streak: i64,
    pub longest_loss_streak: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeroPoolStability {
    pub label: String,
    pub total_heroes: i64,
    pub top1_share: f64,
    pub top3_share: f64,
    pub avg_heroes_per_match: f64,
    pub switch_match_rate: f64,
    pub main_heroes: Vec<HeroPoolHero>,
    pub clues: Vec<ReviewConclusion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeroPoolHero {
    pub hero: String,
    pub matches: i64,
    pub win_rate: f64,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapAdaptability {
    pub min_sample: i64,
    pub best_map: Option<MapReview>,
    pub maps: Vec<MapReview>,
    pub conclusions: Vec<ReviewConclusion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapReview {
    pub map: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f64,
    pub season_win_rate: f64,
    pub sample_label: String,
    pub avg_deaths_delta: f64,
    pub avg_damage_delta: f64,
    pub avg_healing_delta: f64,
    pub support_match_id: Option<String>,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatus {
    pub selected_season: Option<i64>,
    pub current_season: i64,
    pub syncable_seasons: Vec<i64>,
    pub available_seasons: Vec<i64>,
    pub can_enter_review: bool,
    pub credential_status: CredentialStatus,
    pub local_data: LocalDataStatus,
    pub season_statuses: Vec<SeasonDataStatus>,
    pub health: DataHealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub state: String,
    pub message: String,
    pub exposes_secret: bool,
    #[serde(skip_serializing)]
    pub role_id: Option<i64>,
    pub saved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub account_id: String,
    pub battle_tag: String,
    pub is_active: bool,
    pub credential_state: String,
    pub created_at: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountManagerStatus {
    pub active_account_id: Option<String>,
    pub accounts: Vec<AccountSummary>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveAccountResult {
    pub removed: bool,
    pub active_account_id: Option<String>,
    pub message: String,
    pub reload_required: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginResult {
    pub kind: String,
    pub account_id: Option<String>,
    pub battle_tag: Option<String>,
    pub message: String,
    pub should_offer_switch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialLoginCancelResult {
    pub cancelled: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerProfile {
    #[serde(skip_serializing)]
    pub role_id: Option<i64>,
    pub player_id: Option<String>,
    pub avatar_url: Option<String>,
    pub endorsement_level: Option<i64>,
    pub play_time_hours: Option<f64>,
    pub title: Option<String>,
    pub refreshed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    pub theme: String,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub status: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDataStatus {
    pub data_source: String,
    pub season: Option<i64>,
    pub preset_matches: i64,
    pub detail_matches: i64,
    pub last_synced_or_imported_at: Option<String>,
    pub freshness_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SeasonDataStatus {
    pub season: i64,
    pub match_count: i64,
    pub detail_count: i64,
    pub normal_match_count: i64,
    pub arena_match_count: i64,
    pub failed_detail_count: i64,
    pub last_synced_or_imported_at: Option<String>,
    pub coverage_percent: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DataHealthStatus {
    pub readable: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RankingResponse {
    pub season: Option<i64>,
    pub available_seasons: Vec<i64>,
    pub heroes: Vec<RankingRow>,
    pub maps: Vec<RankingRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RankingRow {
    pub key: String,
    pub name: String,
    pub matches: i64,
    pub win_rate: f64,
    pub play_time_hours: f64,
    pub kda: f64,
    pub rating: Option<String>,
    pub best_hero_guid: Option<String>,
    pub best_hero_name: Option<String>,
    pub adaptability_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeasonAnalysis {
    pub season: Option<i64>,
    pub friend: FriendAnalytics,
    pub map_cards: MapAnalysisCards,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapAnalysisCards {
    pub min_sample: i64,
    pub ace_map: Option<MapAnalysisCard>,
    pub throw_zone: Option<MapAnalysisCard>,
    pub home_field: Option<MapAnalysisCard>,
    pub rare_guest: Option<MapAnalysisCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapAnalysisCard {
    pub key: String,
    pub title: String,
    pub name: String,
    pub matches: i64,
    pub win_rate: f64,
    pub route: String,
}
