import type { PlayerQueryMatchRowView } from "../matches/MatchListCard";
import type { ArenaMatchDetail, PlayerGameAction, PlayerRow } from "../../shared/types";

export const PLAYER_QUERY_SEASONS = [23, 22, 21, 20, 15] as const;
export type PlayerQuerySeason = (typeof PLAYER_QUERY_SEASONS)[number];

export const PLAYER_QUERY_MODES = [
  "LeisurePreset",
  "Leisure6v6",
  "SportPreset",
  "SportOpen",
  "SportFight",
  "LeisureFight",
] as const;
export type PlayerQueryMode = (typeof PLAYER_QUERY_MODES)[number];
export type PlayerQueryListScopeKey = "normal:leisure" | "normal:sport" | "arena:SportFight" | "arena:LeisureFight";

export const PLAYER_QUERY_MODE_LABELS: Readonly<Record<PlayerQueryMode, string>> = {
  LeisurePreset: "快速预设",
  Leisure6v6: "快速开放",
  SportPreset: "预设竞技",
  SportOpen: "开放竞技",
  SportFight: "角斗竞技",
  LeisureFight: "角斗快速",
};

export type PlayerQueryMatchKind = "normal" | "arena";

export type PlayerQuerySessionStart = {
  sessionId: string;
  generation: number;
  battleTag: string;
};

export type PlayerQueryMemoryImage = {
  mimeType: string;
  dataBase64: string;
};

export type PlayerQueryCard = {
  battleTag: string;
  level: number | null;
  gameTimeSeconds: number | null;
  title: string | null;
  gameAction: PlayerGameAction | null;
};

export type PlayerQueryCardImages = {
  icon: PlayerQueryMemoryImage | null;
  titleIcon: PlayerQueryMemoryImage | null;
};

export type PlayerQueryRankRow = {
  role: string;
  rank: string | null;
  division: number | null;
  wins: number | null;
  games: number | null;
  winRate: number | null;
  kda: number | null;
};

export type PlayerQuerySeasonOverview = {
  /** Preset competitive ranks. Legacy backends may still combine preset/open here. */
  competitive: PlayerQueryRankRow[];
  /** Independently queried open competitive ranks; absent on legacy backends. */
  openCompetitive?: PlayerQueryRankRow[];
  stadium: PlayerQueryRankRow[];
};

export type PlayerQueryHeroUsage = {
  heroGuid: string;
  heroName: string;
  gameTimeSeconds: number;
  wins: number | null;
  games: number | null;
  winRate: number | null;
};

/** The query DTO and the shared query-row component intentionally share one contract. */
export type PlayerQueryMatchRow = PlayerQueryMatchRowView;

export type PlayerQueryModePage = {
  listScopeKey?: PlayerQueryListScopeKey;
  page: number;
  matches: PlayerQueryMatchRow[];
  endReached: boolean;
};

/** Pure display data accepted by MatchDetailContent; local-only capabilities are absent. */
export type PlayerQueryMatchDetail = {
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
  evaluation: null;
  teamAwards: [];
  sameRoleComparison: null;
  favorite: null;
  aiRoast: null;
};
