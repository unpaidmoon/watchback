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
};

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
  roleId: number | null;
  savedAt: string | null;
};

export type CredentialLoginResult = {
  status: CredentialStatus;
};

export type PlayerTargetStatus = {
  state: "missing" | "bound" | string;
  inputBnet?: string | null;
  resolvedBnet?: string | null;
  boundAt?: string | null;
};

export type BindPlayerTargetResult = {
  status: PlayerTargetStatus;
};

export type ClearPlayerTargetResult = {
  cleared: boolean;
  status: PlayerTargetStatus;
};

export type PlayerProfile = {
  roleId: number | null;
  playerId: string | null;
  avatarUrl: string | null;
  endorsementLevel: number | null;
  playTimeHours: number | null;
  title: string | null;
  refreshedAt: string | null;
};

export type UserSettings = {
  theme: "system" | "light" | "dark";
};

export type LogoutResult = {
  credentialCleared: boolean;
  message: string;
};

export type UpdateCheckResult = {
  status: string;
  currentVersion: string;
  latestVersion: string | null;
  message: string;
};

export type CredentialLoginProgress = {
  phase: string;
  message: string;
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

export type SyncRequest = {
  season: number;
};

export type SyncSummary = {
  season: number;
  phase: string;
  processed: number;
  total: number;
  currentMatchId: string | null;
  importedMatches: number;
  importedDetails: number;
  skippedDuplicates: number;
  failedDetails: number;
  latestErrors: string[];
  importSummary: ImportSummary;
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

export type PersonalReviewRequest = {
  season?: number;
};

export type OnboardingStatusRequest = {
  season?: number;
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
  summary: ReviewSummary;
  baseline: SeasonBaseline;
  roleSummaries: RoleRankSummary[];
  phase5Highlights: Phase5ReviewHighlights;
  topConclusions: ReviewConclusion[];
  personalBests: PersonalBest[];
  streaks: StreakSummary;
  heroPool: HeroPoolStability;
  maps: MapAdaptability;
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
};

export type OnboardingStatus = {
  selectedSeason: number | null;
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

export type CareerRole = "tank" | "damage" | "support";

export type CareerMmrRoleSummary = {
  role: CareerRole;
  currentMmr: number | null;
  confidence: number;
  confidenceLabel: string;
  peakMmr: number | null;
  peakAt: number | null;
  effectiveMatches: number;
  effectiveHours: number;
};

export type CareerMmrOverview = {
  modelVersion: number | null;
  compositeMmr: number | null;
  roles: CareerMmrRoleSummary[];
  discoveredSeasons: number[];
  disclaimer: string;
};

export type CareerMmrHistoryRequest = {
  role: CareerRole;
  season?: number;
  from?: number;
  to?: number;
  granularity?: "match" | "day" | "week";
  limit?: number;
  before?: number;
  beforeHash?: string;
  beforeBucket?: number;
  beforeSeason?: number;
};

export type CareerMmrHistoryPoint = {
  timestamp: number;
  season: number;
  mmr: number;
  confidence: number;
  delta: number;
};

export type CareerMmrHistory = {
  role: CareerRole;
  granularity: string;
  points: CareerMmrHistoryPoint[];
};

export type CareerMmrModeItem = {
  modeFamily: string;
  matchesCount: number;
  durationSec: number;
  wins: number;
  losses: number;
  draws: number;
  cumulativeDelta: number;
};

export type CareerMmrModeBreakdown = { role: CareerRole; items: CareerMmrModeItem[] };

export type CareerMmrSeasonItem = {
  season: number;
  matchesCount: number;
  startMmr: number;
  endMmr: number;
  peakMmr: number;
  netDelta: number;
  firstMatchAt: number | null;
  lastMatchAt: number | null;
};

export type CareerMmrSeasonBreakdown = { role: CareerRole; items: CareerMmrSeasonItem[] };

export type CareerMmrBuildStatus = {
  modelVersion: number | null;
  summaryCount: number;
  isBuilt: boolean;
};

export type CareerMmrExplanationFactor = {
  kind: string;
  label: string;
  value: number;
  unit: string;
};

export type CareerMmrRecentExplanation = {
  timestamp: number;
  season: number;
  delta: number;
  summary: string;
  factors: CareerMmrExplanationFactor[];
  dataQuality: number;
};

export type CareerMmrRecentExplanations = {
  role: CareerRole;
  items: CareerMmrRecentExplanation[];
};
