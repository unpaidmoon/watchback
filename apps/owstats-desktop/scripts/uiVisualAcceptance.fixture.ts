import type {
  CareerMmrBuildStatus,
  CareerMmrHistory,
  CareerMmrModeBreakdown,
  CareerMmrOverview,
  CareerMmrSeasonBreakdown,
  FilterOptions,
  MatchDetailView,
  MatchSummary,
  OnboardingStatus,
  Overview,
  PersonalSeasonReview,
  PlayerProfile,
  PlayerRow,
  PlayerTargetStatus,
  RankingResponse,
  SeasonAnalysis,
} from "../src/shared/types.ts";

export const fixtureShell = {
  productName: "OWStatsLocal",
  statusLabel: "本地合成数据",
  navigation: [
    { label: "对局", route: "/matches" },
    { label: "战报", route: "/review?season=23&view=overview" },
    { label: "生涯 MMR", route: "/career-mmr" },
    { label: "排行", route: "/ranking?season=23&tab=heroes" },
    { label: "指挥台", route: "/onboarding" },
  ],
} as const;

export const fixtureOverview = {
  databasePath: "内存合成数据",
  importSummary: {
    season: 23,
    matchesSeen: 18,
    detailsSeen: 18,
    presetMatches: 12,
    stadiumMatches: 6,
    importedAt: "2026-07-14 08:30",
  },
} satisfies Overview;

export const fixtureFilters = {
  seasons: [23, 22, 21],
  heroes: [
    { id: "fixture-hero-ana", label: "安娜" },
    { id: "fixture-hero-tracer", label: "猎空" },
  ],
  maps: [
    { id: "fixture-map-shambali", label: "香巴里寺院" },
    { id: "fixture-map-hanamura", label: "花村" },
  ],
  results: ["win", "loss"],
  modes: ["SportPreset", "Stadium"],
} satisfies FilterOptions;

const normalEvaluation = {
  grade: "A",
  primaryTag: "尽力局",
  evidenceTags: ["支援稳定", "阵地控制"],
  confidence: 0.88,
};

export const fixtureMatches = [
  {
    matchId: "fixture-normal-1",
    season: 23,
    bucket: "preset",
    matchKind: "normal",
    gameMode: "SportPreset",
    modeLabel: "普通对局",
    result: "win",
    resultLabel: "胜利",
    mapGuid: "fixture-map-shambali",
    mapPreviewGuid: "fixture-map-preview-shambali",
    mapName: "香巴里寺院",
    heroGuid: "fixture-hero-ana",
    heroName: "安娜",
    beginTs: 1783992600,
    beginLabel: "07-14 20:10",
    teamScore: 3,
    opponentScore: 1,
    kills: 18,
    assists: 24,
    deaths: 5,
    damage: 8450,
    healing: 12780,
    mitigation: 960,
    durationSec: 842,
    evaluation: normalEvaluation,
    teamAwards: [{ code: "healing", label: "本队治疗最高", tone: "gold" }],
  },
  {
    matchId: "fixture-arena-1",
    season: 23,
    bucket: "stadium",
    matchKind: "arena",
    gameMode: "Stadium",
    modeLabel: "角斗竞技",
    result: "loss",
    resultLabel: "惜败",
    mapGuid: "fixture-map-hanamura",
    mapPreviewGuid: "fixture-map-preview-hanamura",
    mapName: "花村",
    heroGuid: "fixture-hero-tracer",
    heroName: "猎空",
    beginTs: 1783989000,
    beginLabel: "07-14 19:10",
    teamScore: 2,
    opponentScore: 3,
    kills: 31,
    assists: 12,
    deaths: 9,
    damage: 15420,
    healing: 0,
    mitigation: 430,
    durationSec: 1065,
    evaluation: {
      grade: "B+",
      primaryTag: "鏖战局",
      evidenceTags: ["回合拉锯", "装备成型"],
      confidence: 0.81,
    },
    teamAwards: [{ code: "kill", label: "本队击杀最高", tone: "gold" }],
  },
] satisfies MatchSummary[];

const normalTeammates = [
  createPlayer("teammate", "self", "示例玩家", "安娜", "fixture-hero-ana"),
  createPlayer("teammate", "ally-1", "训练队友一", "莱因哈特", "fixture-hero-reinhardt"),
  createPlayer("teammate", "ally-2", "训练队友二", "猎空", "fixture-hero-tracer"),
];
const normalEnemies = [
  createPlayer("enemy", "enemy-1", "训练对手一", "温斯顿", "fixture-hero-winston"),
  createPlayer("enemy", "enemy-2", "训练对手二", "源氏", "fixture-hero-genji"),
  createPlayer("enemy", "enemy-3", "训练对手三", "雾子", "fixture-hero-kiriko"),
];

export const fixtureNormalDetail = {
  matchId: "fixture-normal-1",
  gameMode: "SportPreset",
  modeLabel: "普通对局",
  result: "win",
  resultLabel: "胜利",
  mapGuid: "fixture-map-shambali",
  mapName: "香巴里寺院",
  heroGuid: "fixture-hero-ana",
  heroName: "安娜",
  beginTs: 1783992600,
  beginLabel: "07-14 20:10",
  teamScore: 3,
  opponentScore: 1,
  durationSec: 842,
  teammates: normalTeammates,
  enemies: normalEnemies,
  arenaDetail: null,
  evaluation: normalEvaluation,
  teamAwards: [{ code: "healing", label: "本队治疗最高", tone: "gold" }],
  sameRoleComparison: {
    rawRole: "support",
    roleLabel: "支援",
    current: { kills: 18, deaths: 5, damage: 8450, healing: 12780 },
    average: { kills: 14, deaths: 7, damage: 6910, healing: 10420 },
  },
  rawJson: { source: "synthetic-visual-fixture", branch: "normal" },
} satisfies MatchDetailView;

const arenaTeammates = [
  createPlayer("teammate", "arena-self", "示例玩家", "猎空", "fixture-hero-tracer", true),
  createPlayer("teammate", "arena-ally-1", "训练队友一", "查莉娅", "fixture-hero-zarya", true),
];
const arenaEnemies = [
  createPlayer("enemy", "arena-enemy-1", "训练对手一", "卡西迪", "fixture-hero-cassidy", true),
  createPlayer("enemy", "arena-enemy-2", "训练对手二", "安娜", "fixture-hero-ana", true),
];

export const fixtureArenaDetail = {
  matchId: "fixture-arena-1",
  gameMode: "Stadium",
  modeLabel: "角斗竞技",
  result: "loss",
  resultLabel: "惜败",
  mapGuid: "fixture-map-hanamura",
  mapName: "花村",
  heroGuid: "fixture-hero-tracer",
  heroName: "猎空",
  beginTs: 1783989000,
  beginLabel: "07-14 19:10",
  teamScore: 2,
  opponentScore: 3,
  durationSec: 1065,
  teammates: arenaTeammates,
  enemies: arenaEnemies,
  arenaDetail: {
    summary: {
      key: "summary",
      label: "全场汇总",
      result: "loss",
      resultLabel: "惜败",
      teamScore: 2,
      opponentScore: 3,
      durationSec: 1065,
      beginTs: 1783989000,
      beginLabel: "07-14 19:10",
      mapGuid: "fixture-map-hanamura",
      mapName: "花村",
      teammates: arenaTeammates,
      enemies: arenaEnemies,
    },
    rounds: [
      {
        key: "round-1",
        label: "第 1 回合",
        result: "win",
        resultLabel: "胜利",
        teamScore: 1,
        opponentScore: 0,
        durationSec: 205,
        beginTs: 1783989000,
        beginLabel: "07-14 19:10",
        mapGuid: "fixture-map-hanamura",
        mapName: "花村",
        teammates: arenaTeammates,
        enemies: arenaEnemies,
      },
      {
        key: "round-2",
        label: "第 2 回合",
        result: "loss",
        resultLabel: "失败",
        teamScore: 0,
        opponentScore: 1,
        durationSec: 198,
        beginTs: 1783989240,
        beginLabel: "07-14 19:14",
        mapGuid: "fixture-map-hanamura",
        mapName: "花村",
        teammates: arenaTeammates,
        enemies: arenaEnemies,
      },
    ],
  },
  evaluation: {
    grade: "B+",
    primaryTag: "鏖战局",
    evidenceTags: ["回合拉锯", "装备成型"],
    confidence: 0.81,
  },
  teamAwards: [{ code: "kill", label: "本队击杀最高", tone: "gold" }],
  sameRoleComparison: null,
  rawJson: { source: "synthetic-visual-fixture", branch: "arena" },
} satisfies MatchDetailView;

export const fixtureMatchDetailsById: Record<string, MatchDetailView> = {
  "fixture-normal-1": fixtureNormalDetail,
  "fixture-arena-1": fixtureArenaDetail,
};

const fixtureEvidence = {
  kind: "match",
  label: "查看合成对局",
  route: "/matches?matchId=fixture-normal-1&season=23",
  matchId: "fixture-normal-1",
  hero: "安娜",
  map: "香巴里寺院",
};

export const fixtureReview = {
  summary: {
    season: 23,
    totalMatches: 18,
    baselineMatches: 16,
    wins: 11,
    losses: 7,
    winRate: 0.611,
    avgKills: 17.4,
    avgDeaths: 6.2,
    avgDamage: 8920,
    avgHealing: 7340,
  },
  baseline: {
    matchCount: 16,
    excludedTotal: 2,
    shortMatchThresholdSec: 180,
    exclusionReasons: [{ reason: "short", label: "短时对局", count: 2 }],
    avgKills: 16.8,
    avgDeaths: 6.5,
    avgDamage: 8610,
    avgHealing: 7050,
  },
  roleSummaries: [
    { roleType: "tank", roleLabel: "重装", matches: 4, wins: 2, winRate: 0.5, kda: 2.8, rankLevel: 3, rankSubTier: 2, rankKey: "Gold_2" },
    { roleType: "damage", roleLabel: "输出", matches: 6, wins: 4, winRate: 0.667, kda: 3.6, rankLevel: 4, rankSubTier: 4, rankKey: "Platinum_4" },
    { roleType: "support", roleLabel: "支援", matches: 8, wins: 5, winRate: 0.625, kda: 4.1, rankLevel: 4, rankSubTier: 2, rankKey: "Platinum_2" },
  ],
  phase5Highlights: {
    carryCount: 4,
    tryHardCount: 6,
    sGradeCount: 2,
    longestWinStreak: 4,
    mostCommonHighlight: "支援稳定",
  },
  topConclusions: [
    { kind: "strength", tone: "positive", title: "支援节奏稳定", body: "合成样本中治疗与助攻保持稳定。", priority: 1, evidence: fixtureEvidence },
    { kind: "focus", tone: "neutral", title: "减少阵亡波动", body: "连续交战时优先保留撤退路线。", priority: 2, evidence: fixtureEvidence },
  ],
  personalBests: [
    { metric: "healing", label: "单场治疗", value: 12780, matchId: "fixture-normal-1", result: "win", resultLabel: "胜利", heroName: "安娜", mapName: "香巴里寺院", evidence: fixtureEvidence },
  ],
  streaks: { longestWinStreak: 4, longestLossStreak: 2 },
  heroPool: {
    label: "稳定双核",
    totalHeroes: 5,
    top1Share: 0.44,
    top3Share: 0.82,
    avgHeroesPerMatch: 1.4,
    switchMatchRate: 0.22,
    mainHeroes: [
      { hero: "安娜", matches: 8, winRate: 0.625, evidence: fixtureEvidence },
      { hero: "猎空", matches: 5, winRate: 0.6, evidence: fixtureEvidence },
    ],
    clues: [{ kind: "hero", tone: "positive", title: "核心英雄清晰", body: "合成英雄池集中且保留轮换。", priority: 1, evidence: fixtureEvidence }],
  },
  maps: {
    minSample: 3,
    bestMap: {
      map: "香巴里寺院",
      matches: 5,
      wins: 4,
      winRate: 0.8,
      seasonWinRate: 0.611,
      sampleLabel: "样本充足",
      avgDeathsDelta: -1.2,
      avgDamageDelta: 620,
      avgHealingDelta: 980,
      supportMatchId: "fixture-normal-1",
      evidence: fixtureEvidence,
    },
    maps: [
      {
        map: "香巴里寺院",
        matches: 5,
        wins: 4,
        winRate: 0.8,
        seasonWinRate: 0.611,
        sampleLabel: "样本充足",
        avgDeathsDelta: -1.2,
        avgDamageDelta: 620,
        avgHealingDelta: 980,
        supportMatchId: "fixture-normal-1",
        evidence: fixtureEvidence,
      },
      {
        map: "花村",
        matches: 3,
        wins: 1,
        winRate: 0.333,
        seasonWinRate: 0.611,
        sampleLabel: "观察样本",
        avgDeathsDelta: 1.1,
        avgDamageDelta: -410,
        avgHealingDelta: -260,
        supportMatchId: "fixture-arena-1",
        evidence: { ...fixtureEvidence, route: "/matches?matchId=fixture-arena-1&season=23", matchId: "fixture-arena-1", map: "花村" },
      },
    ],
    conclusions: [{ kind: "map", tone: "neutral", title: "花村仍需观察", body: "合成样本提示回合转换可继续优化。", priority: 2, evidence: fixtureEvidence }],
  },
} satisfies PersonalSeasonReview;

export const fixtureAnalysis = {
  season: 23,
  friend: {
    summary: { season: 23, presetMatches: 12, friendRows: 4, eligibleFriends: 2, minSample: 3 },
    boards: [
      {
        key: "steady",
        label: "稳定搭档",
        description: "合成样本中的稳定协作队友。",
        friends: [
          {
            bnetId: "fixture-friend-1",
            name: "训练队友一",
            boardKey: "steady",
            boardLabel: "稳定搭档",
            sameTeamMatches: 6,
            winRate: 0.667,
            avgDamageTaken: 7020,
            avgHealingTaken: 4650,
            avgMitigation: 8210,
            avgDeaths: 5.2,
            avgKills: 14.8,
            avgAssists: 9.3,
            avgDamage: 8120,
            avgHealing: 1820,
            score: 86,
            reasons: ["协作样本稳定", "承伤节奏平稳"],
          },
        ],
      },
    ],
    commonFriendHeroes: [
      { bnetId: "fixture-friend-1", name: "训练队友一", representativeHeroGuid: "fixture-hero-reinhardt", representativeHeroName: "莱因哈特", matches: 6, wins: 4, winRate: 0.667, latestBeginTs: 1783992600, intimacyScore: 82 },
    ],
    lineupCombinations: [
      {
        key: "fixture-lineup-1",
        members: [
          { bnetId: null, name: "示例玩家", heroGuid: "fixture-hero-ana", heroName: "安娜", isCurrentPlayer: true },
          { bnetId: "fixture-friend-1", name: "训练队友一", heroGuid: "fixture-hero-reinhardt", heroName: "莱因哈特", isCurrentPlayer: false },
        ],
        matches: 4,
        wins: 3,
        winRate: 0.75,
      },
    ],
  },
  mapCards: {
    minSample: 3,
    aceMap: { key: "fixture-map-shambali", title: "王牌地图", name: "香巴里寺院", matches: 5, winRate: 0.8, route: "/ranking?season=23&tab=maps" },
    throwZone: { key: "fixture-map-hanamura", title: "需要复盘", name: "花村", matches: 3, winRate: 0.333, route: "/ranking?season=23&tab=maps" },
    homeField: { key: "fixture-map-shambali", title: "稳定主场", name: "香巴里寺院", matches: 5, winRate: 0.8, route: "/ranking?season=23&tab=maps" },
    rareGuest: { key: "fixture-map-hanamura", title: "低频地图", name: "花村", matches: 3, winRate: 0.333, route: "/ranking?season=23&tab=maps" },
  },
} satisfies SeasonAnalysis;

export const fixtureCareerOverview = {
  modelVersion: 2,
  compositeMmr: 2638,
  roles: [
    { role: "tank", currentMmr: 2510, confidence: 0.76, confidenceLabel: "稳定", peakMmr: 2640, peakAt: 1783906200, effectiveMatches: 14.5, effectiveHours: 4.3 },
    { role: "damage", currentMmr: 2715, confidence: 0.84, confidenceLabel: "较高", peakMmr: 2798, peakAt: 1783992600, effectiveMatches: 18.2, effectiveHours: 5.8 },
    { role: "support", currentMmr: 2688, confidence: 0.89, confidenceLabel: "较高", peakMmr: 2760, peakAt: 1783992600, effectiveMatches: 21.4, effectiveHours: 6.6 },
  ],
  discoveredSeasons: [23, 22, 21],
  disclaimer: "匿名合成模型数据，仅用于可视验收",
} satisfies CareerMmrOverview;

export const fixtureCareerHistory = {
  role: "tank",
  granularity: "match",
  points: [
    { timestamp: 1783733400, season: 23, mmr: 2410, confidence: 0.61, delta: 0 },
    { timestamp: 1783819800, season: 23, mmr: 2475, confidence: 0.69, delta: 65 },
    { timestamp: 1783906200, season: 23, mmr: 2540, confidence: 0.74, delta: 65 },
    { timestamp: 1783992600, season: 23, mmr: 2510, confidence: 0.76, delta: -30 },
  ],
} satisfies CareerMmrHistory;

export const fixtureCareerModes = {
  role: "tank",
  items: [
    { modeFamily: "RoleQueue", matchesCount: 9, durationSec: 6840, wins: 6, losses: 3, draws: 0, cumulativeDelta: 118 },
    { modeFamily: "OpenQueue", matchesCount: 4, durationSec: 2760, wins: 2, losses: 2, draws: 0, cumulativeDelta: 22 },
  ],
} satisfies CareerMmrModeBreakdown;

export const fixtureCareerSeasons = {
  role: "tank",
  items: [
    { season: 23, matchesCount: 13, startMmr: 2410, endMmr: 2510, peakMmr: 2540, netDelta: 100, firstMatchAt: 1783733400, lastMatchAt: 1783992600 },
    { season: 22, matchesCount: 9, startMmr: 2320, endMmr: 2410, peakMmr: 2445, netDelta: 90, firstMatchAt: 1781055000, lastMatchAt: 1783647000 },
  ],
} satisfies CareerMmrSeasonBreakdown;

export const fixtureCareerBuildStatus = {
  modelVersion: 2,
  summaryCount: 3,
  isBuilt: true,
} satisfies CareerMmrBuildStatus;

export const fixtureRanking = {
  season: 23,
  availableSeasons: [23, 22, 21],
  heroes: [
    { key: "fixture-hero-ana", name: "安娜", matches: 8, winRate: 0.625, playTimeHours: 2.7, kda: 4.1, rating: "A", bestHeroGuid: null, bestHeroName: null, adaptabilityLabel: "稳定核心" },
    { key: "fixture-hero-tracer", name: "猎空", matches: 5, winRate: 0.6, playTimeHours: 1.8, kda: 3.5, rating: "B+", bestHeroGuid: null, bestHeroName: null, adaptabilityLabel: "灵活切入" },
  ],
  maps: [
    { key: "fixture-map-shambali", name: "香巴里寺院", matches: 5, winRate: 0.8, playTimeHours: 1.6, kda: 3.9, rating: null, bestHeroGuid: "fixture-hero-ana", bestHeroName: "安娜", adaptabilityLabel: "王牌地图" },
    { key: "fixture-map-hanamura", name: "花村", matches: 3, winRate: 0.333, playTimeHours: 1.1, kda: 2.8, rating: null, bestHeroGuid: "fixture-hero-tracer", bestHeroName: "猎空", adaptabilityLabel: "需要复盘" },
  ],
} satisfies RankingResponse;

export const fixtureOnboarding = {
  selectedSeason: 23,
  currentSeason: 23,
  syncableSeasons: [23, 22, 21, 20, 19, 18, 17, 16, 15],
  availableSeasons: [23, 22, 21],
  canEnterReview: true,
  credentialStatus: {
    state: "ready",
    message: "合成登录状态可用",
    exposesSecret: false,
    savedAt: "2026-07-14 08:20",
  },
  localData: {
    dataSource: "匿名合成数据",
    season: 23,
    presetMatches: 12,
    detailMatches: 12,
    lastSyncedOrImportedAt: "2026-07-14 08:30",
    freshnessRisk: "low",
  },
  seasonStatuses: [
    { season: 23, matchCount: 18, detailCount: 18, normalMatchCount: 12, arenaMatchCount: 6, failedDetailCount: 0, lastSyncedOrImportedAt: "2026-07-14 08:30", coveragePercent: 100 },
    { season: 22, matchCount: 10, detailCount: 9, normalMatchCount: 8, arenaMatchCount: 2, failedDetailCount: 1, lastSyncedOrImportedAt: "2026-07-12 18:00", coveragePercent: 90 },
  ],
  health: { readable: true, warnings: ["当前展示匿名合成视觉状态"] },
} satisfies OnboardingStatus;

export const fixturePlayerProfile = {
  playerId: "示例玩家",
  avatarUrl: null,
  endorsementLevel: 3,
  playTimeHours: 128.5,
  title: "视觉验收示例",
  refreshedAt: "2026-07-14 08:20",
} satisfies PlayerProfile;

export const fixturePlayerTargetStatus = {
  state: "missing",
  inputBnet: null,
  resolvedBnet: null,
  boundAt: null,
} satisfies PlayerTargetStatus;

export const fixtureCommandCenter = {
  accountLabel: "示例玩家",
  loginState: "ready",
  seasonLabel: "S23",
  syncState: "可同步",
  dataHealth: "合成数据健康",
  tools: ["检查更新", "退出登录"],
} as const;

export const fixtureEmptyReview = {
  summary: {
    season: null,
    totalMatches: 0,
    baselineMatches: 0,
    wins: 0,
    losses: 0,
    winRate: 0,
    avgKills: 0,
    avgDeaths: 0,
    avgDamage: 0,
    avgHealing: 0,
  },
  baseline: {
    matchCount: 0,
    excludedTotal: 0,
    shortMatchThresholdSec: 180,
    exclusionReasons: [],
    avgKills: 0,
    avgDeaths: 0,
    avgDamage: 0,
    avgHealing: 0,
  },
  roleSummaries: [],
  phase5Highlights: {
    carryCount: 0,
    tryHardCount: 0,
    sGradeCount: 0,
    longestWinStreak: 0,
    mostCommonHighlight: null,
  },
  topConclusions: [],
  personalBests: [],
  streaks: { longestWinStreak: 0, longestLossStreak: 0 },
  heroPool: {
    label: "暂无数据",
    totalHeroes: 0,
    top1Share: 0,
    top3Share: 0,
    avgHeroesPerMatch: 0,
    switchMatchRate: 0,
    mainHeroes: [],
    clues: [],
  },
  maps: { minSample: 3, bestMap: null, maps: [], conclusions: [] },
} satisfies PersonalSeasonReview;

export const fixtureEmptyCareerOverview = {
  modelVersion: 2,
  compositeMmr: null,
  roles: [],
  discoveredSeasons: [],
  disclaimer: "暂无可用于计算生涯 MMR 的本地对局数据",
} satisfies CareerMmrOverview;

export const fixtureEmptyRanking = {
  season: 23,
  availableSeasons: [23, 22, 21],
  heroes: [],
  maps: [],
} satisfies RankingResponse;

export const fixtureEmptyOnboarding = {
  selectedSeason: 23,
  currentSeason: 23,
  syncableSeasons: [23, 22, 21, 20, 19, 18, 17, 16, 15],
  availableSeasons: [23, 22, 21],
  canEnterReview: false,
  credentialStatus: {
    state: "missing",
    message: "尚未建立合成登录状态",
    exposesSecret: false,
    savedAt: null,
  },
  localData: {
    dataSource: "匿名合成数据",
    season: 23,
    presetMatches: 0,
    detailMatches: 0,
    lastSyncedOrImportedAt: null,
    freshnessRisk: "unknown",
  },
  seasonStatuses: [],
  health: { readable: true, warnings: ["当前没有可展示的合成赛季数据"] },
} satisfies OnboardingStatus;

export const visualPrimaryRequests = {
  matches: "list_match_timeline",
  review: "get_personal_season_review",
  mmr: "get_career_mmr_overview",
  ranking: "get_rankings",
  commandCenter: "get_onboarding_status",
} as const;

type VisualPrimaryRequest = (typeof visualPrimaryRequests)[keyof typeof visualPrimaryRequests];
type VisualScenarioName = "success" | "loading" | "empty" | "error";
type VisualCommandOverride = {
  delayMs?: number;
  response?: unknown;
  errorMessage?: string;
};

type VisualScenario = {
  label: string;
  commands: Record<VisualPrimaryRequest, VisualCommandOverride>;
};

export const visualAcceptanceScenarios: Record<VisualScenarioName, VisualScenario> = {
  success: {
    label: "成功",
    commands: {
      list_match_timeline: {},
      get_personal_season_review: {},
      get_career_mmr_overview: {},
      get_rankings: {},
      get_onboarding_status: {},
    },
  },
  loading: {
    label: "加载",
    commands: {
      list_match_timeline: { delayMs: 450 },
      get_personal_season_review: { delayMs: 450 },
      get_career_mmr_overview: { delayMs: 450 },
      get_rankings: { delayMs: 450 },
      get_onboarding_status: { delayMs: 450 },
    },
  },
  empty: {
    label: "空",
    commands: {
      list_match_timeline: { response: [] },
      get_personal_season_review: { response: fixtureEmptyReview },
      get_career_mmr_overview: { response: fixtureEmptyCareerOverview },
      get_rankings: { response: fixtureEmptyRanking },
      get_onboarding_status: { response: fixtureEmptyOnboarding },
    },
  },
  error: {
    label: "错误",
    commands: {
      list_match_timeline: { errorMessage: "合成视觉验收错误：对局请求失败，请重新加载。" },
      get_personal_season_review: { errorMessage: "合成视觉验收错误：战报请求失败，请重新加载。" },
      get_career_mmr_overview: { errorMessage: "合成视觉验收错误：生涯 MMR 请求失败，请重新加载。" },
      get_rankings: { errorMessage: "合成视觉验收错误：排行请求失败，请重新加载。" },
      get_onboarding_status: { errorMessage: "合成视觉验收错误：指挥台请求失败，请重新加载。" },
    },
  },
};

export const visualInvokeResponses: Record<string, unknown> = {
  get_overview: fixtureOverview,
  list_match_filters: fixtureFilters,
  list_match_timeline: fixtureMatches,
  search_match_player_options: [],
  get_match_details: fixtureMatchDetailsById,
  refresh_match_evaluations: null,
  get_personal_season_review: fixtureReview,
  get_season_analysis: fixtureAnalysis,
  get_career_mmr_overview: fixtureCareerOverview,
  get_role_mmr_history: fixtureCareerHistory,
  get_role_mmr_mode_breakdown: fixtureCareerModes,
  get_role_mmr_season_breakdown: fixtureCareerSeasons,
  get_career_mmr_build_status: fixtureCareerBuildStatus,
  rebuild_career_mmr: null,
  get_rankings: fixtureRanking,
  get_onboarding_status: fixtureOnboarding,
  get_player_profile: fixturePlayerProfile,
  get_player_target_status: fixturePlayerTargetStatus,
  get_user_settings: { theme: "system" },
  check_app_update: { status: "current", currentVersion: "0.1.0", latestVersion: "0.1.0", message: "当前已是合成验收版本" },
};

export type VisualInvokePlan = {
  delayMs: number;
  response: unknown;
  errorMessage?: string;
};

export function resolveVisualInvokePlan(
  command: string,
  args: unknown = {},
  scenarioName: VisualScenarioName = "success",
): VisualInvokePlan {
  const response = resolveSuccessfulVisualResponse(command, args);
  const override = visualAcceptanceScenarios[scenarioName].commands[command as VisualPrimaryRequest];
  return {
    delayMs: override?.delayMs ?? 0,
    response: override && Object.prototype.hasOwnProperty.call(override, "response")
      ? override.response
      : response,
    ...(override?.errorMessage ? { errorMessage: override.errorMessage } : {}),
  };
}

export async function invokeVisualFixture(
  command: string,
  args: unknown = {},
  scenarioName: VisualScenarioName = "success",
  wait: (delayMs: number) => Promise<void> = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs)),
): Promise<unknown> {
  const plan = resolveVisualInvokePlan(command, args, scenarioName);
  if (plan.delayMs > 0) await wait(plan.delayMs);
  if (plan.errorMessage) throw new Error(plan.errorMessage);
  return plan.response;
}

function resolveSuccessfulVisualResponse(command: string, args: unknown): unknown {
  if (command === "get_match_details") {
    const matchId = readMatchId(args);
    if (!matchId || !fixtureMatchDetailsById[matchId]) {
      throw new Error("合成视觉验收错误：没有可用于该请求的对局详情。");
    }
    return fixtureMatchDetailsById[matchId];
  }
  if (!Object.prototype.hasOwnProperty.call(visualInvokeResponses, command)) {
    throw new Error("合成视觉验收错误：没有配置该请求的匿名响应。");
  }
  return visualInvokeResponses[command];
}

function readMatchId(args: unknown): string | null {
  if (!args || typeof args !== "object") return null;
  const direct = (args as { matchId?: unknown }).matchId;
  if (typeof direct === "string") return direct;
  const request = (args as { request?: unknown }).request;
  if (!request || typeof request !== "object") return null;
  const nested = (request as { matchId?: unknown }).matchId;
  return typeof nested === "string" ? nested : null;
}

export const allVisualFixtures = {
  shell: fixtureShell,
  matches: {
    overview: fixtureOverview,
    filters: fixtureFilters,
    rows: fixtureMatches,
    details: fixtureMatchDetailsById,
  },
  review: { review: fixtureReview, analysis: fixtureAnalysis },
  mmr: {
    overview: fixtureCareerOverview,
    history: fixtureCareerHistory,
    modes: fixtureCareerModes,
    seasons: fixtureCareerSeasons,
    buildStatus: fixtureCareerBuildStatus,
  },
  ranking: fixtureRanking,
  commandCenter: {
    visualState: fixtureCommandCenter,
    status: fixtureOnboarding,
    profile: fixturePlayerProfile,
    targetStatus: fixturePlayerTargetStatus,
  },
};

function createPlayer(
  side: PlayerRow["side"],
  id: string,
  name: string,
  heroName: string,
  heroGuid: string,
  arena = false,
): PlayerRow {
  const teammate = side === "teammate";
  return {
    side,
    name,
    bnetId: `fixture-player-${id}`,
    rankLabel: arena ? "角斗黄金 3" : "黄金 3",
    rankKey: arena ? "Stadium_Gold_3" : "Gold_3",
    heroGuid,
    heroName,
    kills: teammate ? 18 : 16,
    assists: teammate ? 12 : 8,
    deaths: teammate ? 5 : 7,
    damage: teammate ? 8450 : 7920,
    healing: heroName === "安娜" ? 12780 : 860,
    mitigation: heroName === "莱因哈特" || heroName === "查莉娅" ? 10340 : 520,
    finalHits: teammate ? 9 : 8,
    targetTimeSec: teammate ? 74 : 62,
    healingTaken: teammate ? 4320 : 3910,
    damageTaken: teammate ? 7060 : 6810,
    worth: arena ? (teammate ? 14200 : 13600) : null,
    endWorth: arena ? (teammate ? 15100 : 14300) : null,
    worthMax: arena && teammate,
    modGuids: arena ? [`fixture-mod-${id}-unmapped`] : [],
    traitGuids: arena ? [`fixture-trait-${id}-unmapped`] : [],
    hasSourceMaxFlags: true,
    killMax: teammate,
    heroDamageMax: !teammate,
    cureMax: heroName === "安娜",
    resistDamageMax: heroName === "莱因哈特" || heroName === "查莉娅",
    perks: arena ? [] : [{ guid: `fixture-perk-${id}`, perkLevel: 2 }],
    heroStats: [{ guid: `fixture-stat-${id}`, value: teammate ? 42 : 36 }],
  };
}
