export type ImportSummary = {
  season: number;
  matchesSeen: number;
  detailsSeen: number;
  presetMatches: number;
  stadiumMatches: number;
  importedAt: string;
};

export type Overview = {
  databasePath: string;
  importSummary: ImportSummary | null;
};

export type UpdateCheckStatus =
  | "updateAvailable"
  | "upToDate"
  | "disabled"
  | "unavailable";

export type UpdateAnnouncement = {
  latestVersion: string;
  publishedAt: string;
  title: string;
  notes: string[];
};

export type UpdateCheckResult = {
  status: UpdateCheckStatus;
  currentVersion: string;
  announcement: UpdateAnnouncement | null;
};

export type UpdateDownloadSource = "github" | "baidu" | "quark";

export type MatchFilters = {
  season?: number;
  hero?: string;
  map?: string;
  result?: string;
  mode?: string;
  primaryTag?: string;
  playerName?: string;
  dateFrom?: string;
  dateTo?: string;
  favoriteOnly?: boolean;
};

export type MatchFavoriteView = {
  note: string;
};

export type MatchFavoriteRequest = {
  matchId: string;
  note: string;
};

export type AiRoastClaimType =
  | "direct_fact"
  | "statistical_interpretation"
  | "game_context_inference";

export type AiRoastInsight = {
  title: string;
  commentary: string;
  claimType: AiRoastClaimType;
  evidenceRefs: string[];
};

export type AiRoastTeamInsight = AiRoastInsight & {
  type: "positive_player" | "negative_player" | "role_matchup" | "arena_trend";
};

export type AiRoastReport = {
  verdict: {
    title: string;
    summary: string;
  };
  decisiveFactors: AiRoastInsight[];
  currentPlayer: AiRoastInsight;
  teamAndMatchups: AiRoastTeamInsight[];
  closingRoast: string;
};

export type AiMatchRoastView = {
  award: string;
  commentary: string;
  report: AiRoastReport | null;
};

export type DeepSeekKeyStatus = {
  state: 'ready' | 'missing';
  message: string;
};

export type AiRoastGenerationResult = {
  aiRoast: AiMatchRoastView;
};

export type AiRoastStreamEvent =
  | { type: "attemptStarted" }
  | { type: "content"; title: string; commentary: string };

export type MatchEvaluationSummary = {
  grade: string;
  primaryTag: string;
  evidenceTags: string[];
  confidence: number;
};

export type MatchPerformanceAward = {
  code: "kill" | "damage" | "healing" | "mitigation" | "death";
  label: string;
  tone: "gold" | "danger";
};

export type FilterOptions = {
  seasons: number[];
  heroes: FilterOption[];
  maps: FilterOption[];
  results: string[];
  modes: string[];
};

export type FilterOption = {
  id: string;
  label: string;
};

export type PlayerSearchOption = {
  playerName: string;
  matches: number;
  latestBeginTs: number | null;
};

export type CredentialStatus = {
  state: string;
  message: string;
  exposesSecret: boolean;
  savedAt: string | null;
};

export type AccountSummary = {
  accountId: string;
  battleTag: string;
  isActive: boolean;
  credentialState: "ready" | "stale" | "incomplete" | "missing" | string;
  createdAt: string;
  lastUsedAt: string;
};

export type AccountManagerStatus = {
  activeAccountId: string | null;
  accounts: AccountSummary[];
  message: string;
};

export type AccountLoginResult = {
  kind:
    | "newAccountAdded"
    | "existingAccountUpdated"
    | "differentExistingAccount"
    | "differentNewAccount"
    | string;
  accountId: string | null;
  battleTag: string | null;
  message: string;
  shouldOfferSwitch: boolean;
  warnings?: string[];
};

export type RemoveAccountResult = {
  removed: boolean;
  activeAccountId: string | null;
  message: string;
  reloadRequired: boolean;
};

export type PlayerGameAction = {
  comment: string | null;
  sendHonorsCnt: number | null;
  getHonorsCnt: number | null;
  reportedCnt: number | null;
};

export type PlayerProfile = {
  playerId: string | null;
  avatarUrl: string | null;
  titleIconUrl: string | null;
  endorsementLevel: number | null;
  playTimeHours: number | null;
  title: string | null;
  gameAction: PlayerGameAction | null;
  refreshedAt: string | null;
};

export type UserSettings = {
  theme: "system" | "light" | "dark";
  autoUpdateSeason: boolean;
  playerQueryModeOrder: import("../features/player-query/playerQueryTypes").PlayerQueryMode[];
};

export type CredentialLoginProgress = {
  phase: string;
  message: string;
};

export type QrLoginViewState =
  | "starting"
  | "loadingQr"
  | "waitingForScan"
  | "confirming"
  | "capturing"
  | "saving"
  | "expired"
  | "failed"
  | "ready"
  | "cancelling";

export type QrLoginControllerPhase = QrLoginViewState | "recovering" | "cancelled";

export type QrAccountLoginStartResult = {
  requestId: string;
  sessionId: string;
  reused: boolean;
};

export type QrAccountLoginSettleResult = {
  sessionId: string;
  terminalPhase: string;
  released: boolean;
};

export type QrAccountLoginActiveResult = {
  requestId: string;
  sessionId: string;
  generation: number;
};

export const ACCOUNT_OPERATION_STATUSES = [
  "idle",
  "account",
  "foregroundSync",
  "backgroundQueued",
  "backgroundRunning",
  "query",
] as const;

export type AccountOperationStatus = (typeof ACCOUNT_OPERATION_STATUSES)[number];

export type QrAccountLoginQrEvent = {
  sessionId: string;
  revision: number;
  mimeType: "image/png";
  imageBase64: string;
};

export type QrAccountLoginProgressEvent = {
  sessionId: string;
  phase: string;
  message: string | null;
};

export type QrAccountLoginSnapshot = {
  requestId: string;
  sessionId: string;
  generation: number;
  phase: string;
  message: string;
  revision: number;
  imageBase64: string | null;
  settled: boolean;
  released: boolean;
  completion: {
    sessionId: string;
    result: AccountLoginResult | null;
    message?: string | null;
    errorCode?: string | null;
    cancelled: boolean;
  } | null;
};

export type MatchSummary = {
  matchId: string;
  season: number;
  bucket: string;
  matchKind?: string;
  gameMode: string;
  modeLabel: string;
  result: string;
  resultLabel: string;
  mapGuid: string;
  mapPreviewGuid?: string | null;
  mapName: string;
  heroGuid: string;
  heroName: string;
  beginTs: number;
  beginLabel: string;
  teamScore: number;
  opponentScore: number;
  kills: number;
  assists: number;
  deaths: number;
  damage: number;
  healing: number;
  mitigation: number;
  durationSec: number;
  evaluation: MatchEvaluationSummary | null;
  teamAwards: MatchPerformanceAward[];
  isFavorite: boolean;
};

export type MatchCursor = {
  beginTs: number;
  matchId: string;
};

export type MatchPage = {
  matches: MatchSummary[];
  nextCursor: MatchCursor | null;
  total: number | null;
  revisionToken: string;
  accountScope: string;
};

export type MatchTimelineRevision = {
  accountScope: string;
  revisionToken: string;
};

export type PlayerRow = {
  side: "teammate" | "enemy";
  name: string;
  bnetId: string;
  rankLabel: string | null;
  rankKey: string | null;
  heroGuid: string;
  heroName: string;
  kills: number;
  assists: number;
  deaths: number;
  damage: number;
  healing: number;
  mitigation: number;
  finalHits: number;
  targetTimeSec: number;
  healingTaken: number;
  damageTaken: number;
  worth: number | null;
  endWorth: number | null;
  worthMax: boolean;
  modGuids: string[];
  traitGuids: string[];
  hasSourceMaxFlags: boolean;
  killMax: boolean;
  heroDamageMax: boolean;
  cureMax: boolean;
  resistDamageMax: boolean;
  perks: PlayerPerk[];
  heroStats: HeroStat[];
  partyGroup?: number | null;
};

export type PlayerPerk = {
  guid: string;
  perkLevel: number;
};

export type HeroStat = {
  guid: string;
  value: number;
};

export type MatchDetailView = {
  matchId: string;
  gameMode: string | null;
  modeLabel: string | null;
  result: string | null;
  resultLabel: string | null;
  mapGuid: string | null;
  mapName: string | null;
  heroGuid: string | null;
  heroName: string | null;
  beginTs: number | null;
  beginLabel: string | null;
  teamScore: number | null;
  opponentScore: number | null;
  durationSec: number | null;
  teammates: PlayerRow[];
  enemies: PlayerRow[];
  arenaDetail: ArenaMatchDetail | null;
  evaluation: MatchEvaluationSummary | null;
  teamAwards: MatchPerformanceAward[];
  sameRoleComparison: SameRoleComparison | null;
  favorite: MatchFavoriteView | null;
  aiRoast: AiMatchRoastView | null;
  rawJson: unknown;
};

export type ArenaMatchDetail = {
  summary: ArenaDetailSlice;
  rounds: ArenaDetailSlice[];
};

export type ArenaDetailSlice = {
  key: string;
  label: string;
  result: string | null;
  resultLabel: string | null;
  teamScore: number | null;
  opponentScore: number | null;
  durationSec: number | null;
  beginTs: number | null;
  beginLabel: string | null;
  mapGuid: string | null;
  mapName: string | null;
  teammates: PlayerRow[];
  enemies: PlayerRow[];
};

export type ComparisonMetrics = {
  kills: number | null;
  deaths: number | null;
  damage: number | null;
  healing: number | null;
};

export type SameRoleComparison = {
  rawRole: string | null;
  roleLabel: string | null;
  current: ComparisonMetrics;
  average: ComparisonMetrics;
};

export type FriendAnalyticsFilters = MatchFilters;

export type FriendAnalyticsSummary = {
  season: number | null;
  presetMatches: number;
  friendRows: number;
  eligibleFriends: number;
  minSample: number;
};

export type FriendBoardEntry = {
  bnetId: string;
  name: string;
  boardKey: string;
  boardLabel: string;
  sameTeamMatches: number;
  winRate: number;
  avgDamageTaken: number;
  avgHealingTaken: number;
  avgMitigation: number;
  avgDeaths: number;
  avgKills: number;
  avgAssists: number;
  avgDamage: number;
  avgHealing: number;
  score: number;
  reasons: string[];
};

export type FriendBoard = {
  key: string;
  label: string;
  description: string;
  friends: FriendBoardEntry[];
};

export type FriendAnalytics = {
  summary: FriendAnalyticsSummary;
  boards: FriendBoard[];
  commonFriendHeroes: CommonFriendHero[];
  lineupCombinations: LineupCombination[];
};

export type CommonFriendHero = {
  bnetId: string;
  name: string;
  representativeHeroGuid: string;
  representativeHeroName: string;
  matches: number;
  wins: number;
  winRate: number;
  latestBeginTs: number;
  intimacyScore: number;
};

export type LineupMember = {
  bnetId: string | null;
  name: string;
  heroGuid: string;
  heroName: string;
  isCurrentPlayer: boolean;
};

export type LineupCombination = {
  key: string;
  members: LineupMember[];
  matches: number;
  wins: number;
  winRate: number;
};

export type FriendBreakdown = {
  label: string;
  count: number;
};

export type FriendRecentMatch = {
  matchId: string;
  beginLabel: string;
  result: string;
  mapName: string;
  heroName: string;
  kills: number;
  assists: number;
  deaths: number;
  damage: number;
  healing: number;
  damageTaken: number;
  healingTaken: number;
};

export type FriendDetail = {
  bnetId: string;
  name: string;
  boards: FriendBoardEntry[];
  commonHeroes: FriendBreakdown[];
  commonMaps: FriendBreakdown[];
  recentMatches: FriendRecentMatch[];
};

export type SyncMode = "quickUpdate" | "fullFetch";

export type SyncPageDiagnostic = {
  matchKind: string;
  gameMode: string;
  page: number;
  entries: number;
  uniqueValidEntries: number;
  newMatches: number;
  missingDetails: number;
  existingCompleteMatches: number;
  earliestBeginTs: number | null;
  latestBeginTs: number | null;
  stopReason: string | null;
  watermarkReached: boolean;
};

export type SyncDiagnostics = {
  listDurationMs: number;
  detailDurationMs: number;
  persistDurationMs: number;
  pages: SyncPageDiagnostic[];
  quickScanLimited: boolean;
};

export type SyncSummary = {
  season: number;
  mode: SyncMode;
  phase: string;
  processed: number;
  total: number;
  currentMatchId: string | null;
  listEntriesSeen: number;
  newMatches: number;
  missingDetails: number;
  detailsRequested: number;
  repairedDetails: number;
  databaseMatches: number;
  databaseDetails: number;
  diagnostics: SyncDiagnostics;
  quickScanLimited: boolean;
  importedMatches: number;
  importedDetails: number;
  skippedDuplicates: number;
  failedDetails: number;
  latestErrors: string[];
  importSummary: ImportSummary;
};

export type QuickModeResult = {
  matchKind: string;
  gameMode: string;
  outcome: "unchangedBoundary" | "emptyBoundary" | "newEntries" | "unsafe" | "failed";
  newMatches: number;
  errorClass: string | null;
};

export type QuickListCommitSummary = {
  season: number;
  newListMatches: number;
  dataChanged: boolean;
  partialFailure: boolean;
  pendingDetails: number;
  backgroundStatus: string;
  checkedAt: string;
  modes: QuickModeResult[];
};

export type SeasonSyncResult =
  | ({ kind: "quickList" } & QuickListCommitSummary)
  | ({ kind: "full" } & SyncSummary);

export type SeasonBackgroundStatus = {
  season: number;
  phase: string;
  status: string;
  pendingDetails: number;
  retryable: boolean;
  updatedAt: string;
};

export type SeasonBackgroundEvent = {
  accountScope: string;
  season: number;
  phase: string;
  status: string;
  newMatches: number;
  pendingDetails: number;
};

export type SyncProgress = {
  season: number;
  phase: string;
  processed: number;
  total: number;
  currentMatchId: string | null;
  failedDetails: number;
  latestErrors: string[];
};

export type EvidenceLink = {
  kind: string;
  label: string;
  route: string;
  matchId: string | null;
  hero: string | null;
  map: string | null;
};

export type ReviewConclusion = {
  kind: string;
  tone: string;
  title: string;
  body: string;
  priority: number;
  evidence: EvidenceLink;
};

export type ReviewSummary = {
  season: number | null;
  totalMatches: number;
  baselineMatches: number;
  wins: number;
  losses: number;
  winRate: number;
  avgKills: number;
  avgDeaths: number;
  avgDamage: number;
  avgHealing: number;
};

export type SeasonBaseline = {
  matchCount: number;
  excludedTotal: number;
  shortMatchThresholdSec: number;
  exclusionReasons: { reason: string; label: string; count: number }[];
  avgKills: number;
  avgDeaths: number;
  avgDamage: number;
  avgHealing: number;
};

export type PersonalBest = {
  metric: string;
  label: string;
  value: number;
  matchId: string;
  result: string;
  resultLabel: string;
  heroName: string;
  mapName: string;
  evidence: EvidenceLink;
};

export type StreakSummary = {
  longestWinStreak: number;
  longestLossStreak: number;
};

export type HeroPoolStability = {
  label: string;
  totalHeroes: number;
  top1Share: number;
  top3Share: number;
  avgHeroesPerMatch: number;
  switchMatchRate: number;
  mainHeroes: { hero: string; matches: number; winRate: number; evidence: EvidenceLink }[];
  clues: ReviewConclusion[];
};

export type MapReview = {
  map: string;
  matches: number;
  wins: number;
  winRate: number;
  seasonWinRate: number;
  sampleLabel: string;
  avgDeathsDelta: number;
  avgDamageDelta: number;
  avgHealingDelta: number;
  supportMatchId: string | null;
  evidence: EvidenceLink;
};

export type MapAdaptability = {
  minSample: number;
  bestMap: MapReview | null;
  maps: MapReview[];
  conclusions: ReviewConclusion[];
};

export type PersonalSeasonReview = {
  availableSeasons: number[];
  summary: ReviewSummary;
  baseline: SeasonBaseline;
  roleSummaries: RoleRankSummary[];
  stadiumRoleSummaries: RoleRankSummary[];
  phase5Highlights: Phase5ReviewHighlights;
  topConclusions: ReviewConclusion[];
  personalBests: PersonalBest[];
  streaks: StreakSummary;
  heroPool: HeroPoolStability;
  maps: MapAdaptability;
};

export type CareerAchievementTier = {
  level: number;
  requirement: string;
  unlockedAt: number | null;
};

export type CareerAchievement = {
  key: string;
  title: string;
  detail: string;
  currentLevel: number;
  tiers: CareerAchievementTier[];
};

export type RoleRankSummary = {
  roleType: string;
  roleLabel: string;
  matches: number;
  wins: number;
  winRate: number;
  kda: number;
  rankLevel: number;
  rankSubTier: number;
  rankKey: string;
};

export type Phase5ReviewHighlights = {
  carryCount: number;
  tryHardCount: number;
  sGradeCount: number;
  longestWinStreak: number;
  mostCommonHighlight: string | null;
};

export type RankingRow = {
  key: string;
  name: string;
  matches: number;
  winRate: number;
  playTimeHours: number;
  kda: number;
  rating: string | null;
  bestHeroGuid: string | null;
  bestHeroName: string | null;
  adaptabilityLabel: string | null;
};

export type RankingResponse = {
  season: number | null;
  availableSeasons: number[];
  heroes: RankingRow[];
  maps: RankingRow[];
};

export type MapAnalysisCard = {
  key: string;
  title: string;
  name: string;
  matches: number;
  winRate: number;
  route: string;
};

export type MapAnalysisCards = {
  minSample: number;
  aceMap: MapAnalysisCard | null;
  throwZone: MapAnalysisCard | null;
  homeField: MapAnalysisCard | null;
  rareGuest: MapAnalysisCard | null;
};

export type SeasonAnalysis = {
  season: number | null;
  friend: FriendAnalytics;
  mapCards: MapAnalysisCards;
  choiceRankings: ChoiceRankings;
};

export type ChoiceRankingEntry = {
  key: string;
  matches: number;
  wins: number;
  winRate: number;
};

export type ChoiceRankings = {
  minSample: number;
  confidenceLevel: number;
  normalPerks: ChoiceRankingEntry[];
  stadiumTraits: ChoiceRankingEntry[];
  stadiumMods: ChoiceRankingEntry[];
};

export type OnboardingStatus = {
  selectedSeason: number | null;
  minimumSyncSeason: number;
  defaultSeason: number;
  availableSeasons: number[];
  canEnterReview: boolean;
  credentialStatus: CredentialStatus;
  localData: {
    dataSource: string;
    season: number | null;
    presetMatches: number;
    detailMatches: number;
    lastSyncedOrImportedAt: string | null;
    freshnessRisk: string;
  };
  seasonStatuses: SeasonDataStatus[];
  health: { readable: boolean; warnings: string[] };
};

export type SeasonDataStatus = {
  season: number;
  matchCount: number;
  detailCount: number;
  normalMatchCount: number;
  arenaMatchCount: number;
  failedDetailCount: number;
  lastSyncedOrImportedAt: string | null;
  coveragePercent: number;
};
