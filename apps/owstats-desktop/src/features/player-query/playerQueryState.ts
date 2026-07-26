import type {
  PlayerQueryCard,
  PlayerQueryCardImages,
  PlayerQueryHeroUsage,
  PlayerQueryMatchRow,
  PlayerQueryMode,
  PlayerQueryListScopeKey,
  PlayerQuerySeason,
  PlayerQuerySeasonOverview,
  PlayerQuerySessionStart,
} from "./playerQueryTypes";
import { decodePlayerQueryError } from "./playerQueryApi";

export const PLAYER_QUERY_HOME_MATCH_LIMIT = 12;
export const PLAYER_QUERY_HOME_HERO_LIMIT = 9;

export type QueryResource<T> = {
  status: "idle" | "loading" | "success" | "error";
  data: T | null;
  error: string | null;
};

export type PlayerQueryState = {
  search: QueryResource<PlayerQuerySessionStart>;
  battleTag: string | null;
  season: PlayerQuerySeason;
  mode: PlayerQueryMode;
  listScopeKey: PlayerQueryListScopeKey;
  generation: number;
  listGeneration: number;
  heroGeneration: number;
  card: QueryResource<PlayerQueryCard>;
  cardImages: QueryResource<PlayerQueryCardImages>;
  ranks: QueryResource<PlayerQuerySeasonOverview>;
  matches: QueryResource<PlayerQueryMatchRow[]>;
  matchesEndReached: boolean;
  heroes: QueryResource<PlayerQueryHeroUsage[]>;
  authenticationInvalid: boolean;
};

export const idleResource = <T,>(): QueryResource<T> => ({ status: "idle", data: null, error: null });
export const loadingResource = <T,>(): QueryResource<T> => ({ status: "loading", data: null, error: null });

export function listScopeKeyForMode(mode: PlayerQueryMode): PlayerQueryListScopeKey {
  if (mode === "LeisurePreset" || mode === "Leisure6v6") return "normal:leisure";
  if (mode === "SportPreset" || mode === "SportOpen") return "normal:sport";
  return mode === "SportFight" ? "arena:SportFight" : "arena:LeisureFight";
}

export function normalizeBattleTag(input: string): string {
  return input.trim().replace(/＃/g, "#");
}

export function isValidBattleTag(input: string): boolean {
  const normalized = normalizeBattleTag(input);
  const separator = normalized.lastIndexOf("#");
  return separator > 0 && /^\d+$/.test(normalized.slice(separator + 1));
}

export function sameBattleTag(left: string | null, right: string): boolean {
  return left !== null && normalizeBattleTag(left).toLocaleLowerCase() === normalizeBattleTag(right).toLocaleLowerCase();
}

export function initialPlayerQueryState(firstMode: PlayerQueryMode): PlayerQueryState {
  return {
    search: idleResource(), battleTag: null, season: 23, mode: firstMode, listScopeKey: listScopeKeyForMode(firstMode), generation: 0, listGeneration: 0, heroGeneration: 0,
    card: idleResource(), cardImages: idleResource(), ranks: idleResource(), matches: idleResource(), matchesEndReached: false, heroes: idleResource(),
    authenticationInvalid: false,
  };
}

export function alignIdlePlayerQueryMode(state: PlayerQueryState, firstMode: PlayerQueryMode): PlayerQueryState {
  if (state.search.status !== "idle" || state.mode === firstMode) return state;
  return { ...state, mode: firstMode, listScopeKey: listScopeKeyForMode(firstMode) };
}

export function beginPlayerSearch(state: PlayerQueryState, rawBattleTag: string, firstMode: PlayerQueryMode): PlayerQueryState {
  const battleTag = normalizeBattleTag(rawBattleTag);
  const refresh = sameBattleTag(state.battleTag, battleTag);
  return {
    ...state,
    search: loadingResource(),
    battleTag,
    season: refresh ? state.season : 23,
    mode: refresh ? state.mode : firstMode,
    listScopeKey: listScopeKeyForMode(refresh ? state.mode : firstMode),
    generation: state.generation + 1,
    listGeneration: state.listGeneration + 1,
    heroGeneration: state.heroGeneration + 1,
    card: idleResource(), cardImages: idleResource(), ranks: idleResource(), matches: idleResource(), matchesEndReached: false, heroes: idleResource(),
    authenticationInvalid: false,
  };
}

export function changePlayerQuerySeason(state: PlayerQueryState, season: PlayerQuerySeason): PlayerQueryState {
  if (state.season === season) return state;
  return { ...state, season, generation: state.generation + 1, listGeneration: state.listGeneration + 1, heroGeneration: state.heroGeneration + 1, ranks: loadingResource(), matches: loadingResource(), matchesEndReached: false, heroes: loadingResource() };
}

export function changePlayerQueryMode(state: PlayerQueryState, mode: PlayerQueryMode): PlayerQueryState {
  if (state.mode === mode) return state;
  const listScopeKey = listScopeKeyForMode(mode);
  const sameListScope = listScopeKey === state.listScopeKey;
  return {
    ...state, mode, listScopeKey, heroGeneration: state.heroGeneration + 1, heroes: loadingResource(),
    ...(sameListScope ? {} : { listGeneration: state.listGeneration + 1, matches: loadingResource<PlayerQueryMatchRow[]>(), matchesEndReached: false }),
  };
}

export function visibleQueryHeroes(heroes: readonly PlayerQueryHeroUsage[]): PlayerQueryHeroUsage[] {
  return heroes.filter((hero) => hero.gameTimeSeconds > 0).slice(0, PLAYER_QUERY_HOME_HERO_LIMIT);
}

export function errorMessage(error: unknown): string {
  return decodePlayerQueryError(error).message;
}

export function isAuthenticationError(error: unknown): boolean {
  const decoded = decodePlayerQueryError(error);
  if (decoded.kind.toLowerCase() === "sessioninvalid") return true;
  const message = decoded.message.toLowerCase();
  return /(^|\D)(401|403)(\D|$)|unauthori[sz]ed|forbidden|凭据|登录.*失效/.test(message);
}
