import { invoke } from "@tauri-apps/api/core";
import type {
  PlayerQueryCard,
  PlayerQueryCardImages,
  PlayerQueryHeroUsage,
  PlayerQueryMatchDetail,
  PlayerQueryMatchKind,
  PlayerQueryMode,
  PlayerQueryModePage,
  PlayerQuerySeason,
  PlayerQuerySeasonOverview,
  PlayerQuerySessionStart,
} from "./playerQueryTypes";

export type DecodedPlayerQueryError = { kind: string; message: string };
export function decodePlayerQueryError(cause: unknown, fallback = "请求失败，请稍后重试"): DecodedPlayerQueryError {
  if (cause instanceof Error && cause.message) return { kind: "error", message: cause.message };
  if (typeof cause === "string" && cause) return { kind: "error", message: cause };
  if (cause && typeof cause === "object") {
    const value = cause as Record<string, unknown>;
    if (typeof value.message === "string" && value.message) {
      return { kind: typeof value.kind === "string" ? value.kind : "error", message: value.message };
    }
  }
  return { kind: "unknown", message: fallback };
}

export const PLAYER_QUERY_COMMANDS = {
  start: "player_query_start",
  card: "player_query_card",
  cardImages: "player_query_card_images",
  season: "player_query_season",
  modePage: "player_query_mode_page",
  modeHeroes: "player_query_mode_heroes",
  matchDetail: "player_query_match_detail",
  releaseMoreData: "player_query_release_more_data",
  end: "player_query_end",
} as const;

export const startPlayerQuery = (battleTag: string) =>
  invoke<PlayerQuerySessionStart>(PLAYER_QUERY_COMMANDS.start, { battleTag });

export const loadPlayerQueryCard = (sessionId: string) =>
  invoke<PlayerQueryCard>(PLAYER_QUERY_COMMANDS.card, { sessionId });

export const loadPlayerQueryCardImages = (sessionId: string) =>
  invoke<PlayerQueryCardImages>(PLAYER_QUERY_COMMANDS.cardImages, { sessionId });

export const loadPlayerQuerySeason = (sessionId: string, season: PlayerQuerySeason) =>
  invoke<PlayerQuerySeasonOverview>(PLAYER_QUERY_COMMANDS.season, { sessionId, season });

export const loadPlayerQueryModePage = (
  sessionId: string,
  season: PlayerQuerySeason,
  mode: PlayerQueryMode,
  page: number,
) => invoke<PlayerQueryModePage>(PLAYER_QUERY_COMMANDS.modePage, { sessionId, season, mode, page });

export const loadPlayerQueryModeHeroes = (
  sessionId: string,
  season: PlayerQuerySeason,
  mode: PlayerQueryMode,
) => invoke<PlayerQueryHeroUsage[]>(PLAYER_QUERY_COMMANDS.modeHeroes, { sessionId, season, mode });

export const loadPlayerQueryMatchDetail = (
  sessionId: string,
  matchKind: PlayerQueryMatchKind,
  matchId: string,
) => invoke<PlayerQueryMatchDetail>(PLAYER_QUERY_COMMANDS.matchDetail, { sessionId, matchKind, matchId });

export const releasePlayerQueryMoreData = (sessionId: string) =>
  invoke<void>(PLAYER_QUERY_COMMANDS.releaseMoreData, { sessionId });

export const endPlayerQuery = (sessionId: string) =>
  invoke<void>(PLAYER_QUERY_COMMANDS.end, { sessionId });
