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

export type CredentialLoginProgress = {
  phase: string;
  message: string;
};

export type MatchSummary = {
  matchId: string;
  season: number;
  bucket: string;
  gameMode: string;
  modeLabel: string;
  result: string;
  resultLabel: string;
  mapGuid: string;
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
  evaluation: MatchEvaluationSummary | null;
  teamAwards: MatchPerformanceAward[];
  sameRoleComparison: SameRoleComparison | null;
  rawJson: unknown;
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
  phase5Highlights: Phase5ReviewHighlights;
  topConclusions: ReviewConclusion[];
  personalBests: PersonalBest[];
  streaks: StreakSummary;
  heroPool: HeroPoolStability;
  maps: MapAdaptability;
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
  wins: number;
  losses: number;
  winRate: number;
  playTimeHours: number;
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
  health: { readable: boolean; warnings: string[] };
};
