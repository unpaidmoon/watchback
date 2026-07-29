import { useCallback, useEffect, useRef, useState } from "react";
import {
  endPlayerQuery, loadPlayerQueryCard, loadPlayerQueryCardImages, loadPlayerQueryModeHeroes, loadPlayerQueryModePage, loadPlayerQuerySeason, startPlayerQuery,
} from "./playerQueryApi";
import type { PlayerQueryMode, PlayerQuerySeason } from "./playerQueryTypes";
import {
  alignIdlePlayerQueryMode, beginPlayerSearch, changePlayerQueryMode, changePlayerQuerySeason, errorMessage, initialPlayerQueryState,
  isAuthenticationError, isValidBattleTag, loadingResource, normalizeBattleTag, visibleQueryHeroes,
  type PlayerQueryState, type QueryResource,
} from "./playerQueryState";

export type PlayerQueryRetryScope = "card" | "cardImages" | "ranks" | "matches" | "heroes";
export type PlayerQueryApi = {
  start: typeof startPlayerQuery; card: typeof loadPlayerQueryCard; cardImages: typeof loadPlayerQueryCardImages; season: typeof loadPlayerQuerySeason;
  modePage: typeof loadPlayerQueryModePage; modeHeroes: typeof loadPlayerQueryModeHeroes; end: typeof endPlayerQuery;
};
const defaultApi: PlayerQueryApi = { start: startPlayerQuery, card: loadPlayerQueryCard, cardImages: loadPlayerQueryCardImages, season: loadPlayerQuerySeason, modePage: loadPlayerQueryModePage, modeHeroes: loadPlayerQueryModeHeroes, end: endPlayerQuery };

export function requestPlayerQueryModeScope(
  api: Pick<PlayerQueryApi, "modePage" | "modeHeroes">,
  scope: "matches" | "heroes",
  sessionId: string,
  season: PlayerQuerySeason,
  mode: PlayerQueryMode,
) {
  return scope === "matches" ? api.modePage(sessionId, season, mode, 1) : api.modeHeroes(sessionId, season, mode);
}

export type UsePlayerQuerySessionOptions = {
  firstMode: PlayerQueryMode;
  api?: PlayerQueryApi;
  onAuthenticationInvalid?: () => void;
  onSessionChange?: (sessionId: string | null) => void;
};

export function usePlayerQuerySession({ firstMode, api = defaultApi, onAuthenticationInvalid, onSessionChange }: UsePlayerQuerySessionOptions) {
  const [state, setState] = useState<PlayerQueryState>(() => initialPlayerQueryState(firstMode));
  const stateRef = useRef(state);
  const set = useCallback((updater: (current: PlayerQueryState) => PlayerQueryState) => {
    setState((current) => { const next = updater(current); stateRef.current = next; return next; });
  }, []);
  useEffect(() => {
    set((current) => alignIdlePlayerQueryMode(current, firstMode));
  }, [firstMode, set]);

  const invalidateAuthentication = useCallback(async (sessionId?: string) => {
    if (sessionId) void api.end(sessionId).catch(() => undefined);
    onSessionChange?.(null);
    set((current) => ({ ...initialPlayerQueryState(current.mode), authenticationInvalid: true }));
    onAuthenticationInvalid?.();
  }, [api, onAuthenticationInvalid, onSessionChange, set]);

  const settleError = useCallback((scope: PlayerQueryRetryScope, generation: number, error: unknown, sessionId: string) => {
    if (isAuthenticationError(error)) { void invalidateAuthentication(sessionId); return; }
    set((current) => current.generation !== generation ? current : {
      ...current, [scope]: { status: "error", data: null, error: errorMessage(error) } as QueryResource<never>,
    });
  }, [invalidateAuthentication, set]);

  const loadCard = useCallback(async (sessionId: string, generation: number) => {
    set((current) => current.generation === generation ? { ...current, card: loadingResource() } : current);
    try { const data = await api.card(sessionId); set((current) => current.generation === generation ? { ...current, card: { status: "success", data, error: null } } : current); }
    catch (error) { settleError("card", generation, error, sessionId); }
  }, [api, set, settleError]);

  const loadCardImages = useCallback(async (sessionId: string, generation: number) => {
    set((current) => current.generation === generation ? { ...current, cardImages: loadingResource() } : current);
    try {
      const data = await api.cardImages(sessionId);
      set((current) => current.generation === generation ? { ...current, cardImages: { status: "success", data, error: null } } : current);
    } catch (error) {
      // Image transport failure is local to the optional images and never invalidates card text.
      set((current) => current.generation === generation ? { ...current, cardImages: { status: "error", data: null, error: errorMessage(error) } } : current);
    }
  }, [api, set]);

  const loadRanks = useCallback(async (sessionId: string, season: PlayerQuerySeason, generation: number) => {
    set((current) => current.generation === generation ? { ...current, ranks: loadingResource() } : current);
    try { const data = await api.season(sessionId, season); set((current) => current.generation === generation ? { ...current, ranks: { status: "success", data, error: null } } : current); }
    catch (error) { settleError("ranks", generation, error, sessionId); }
  }, [api, set, settleError]);

  const loadMatches = useCallback(async (sessionId: string, season: PlayerQuerySeason, mode: PlayerQueryMode, generation: number) => {
    set((current) => current.listGeneration === generation ? { ...current, matches: loadingResource() } : current);
    try {
      const data = await requestPlayerQueryModeScope(api, "matches", sessionId, season, mode);
      if (!("matches" in data)) return;
      set((current) => current.listGeneration === generation ? {
        ...current,
        matches: { status: "success", data: data.matches.slice(0, 12), error: null },
        matchesEndReached: data.endReached,
      } : current);
    } catch (error) {
      if (isAuthenticationError(error)) { void invalidateAuthentication(sessionId); return; }
      set((current) => current.listGeneration === generation ? { ...current, matches: { status: "error", data: null, error: errorMessage(error) } } : current);
    }
  }, [api, invalidateAuthentication, set]);

  const loadHeroes = useCallback(async (sessionId: string, season: PlayerQuerySeason, mode: PlayerQueryMode, generation: number) => {
    set((current) => current.heroGeneration === generation ? { ...current, heroes: loadingResource() } : current);
    try {
      const data = await requestPlayerQueryModeScope(api, "heroes", sessionId, season, mode);
      if (!Array.isArray(data)) return;
      set((current) => current.heroGeneration === generation ? { ...current, heroes: { status: "success", data: visibleQueryHeroes(data), error: null } } : current);
    } catch (error) {
      if (isAuthenticationError(error)) { void invalidateAuthentication(sessionId); return; }
      set((current) => current.heroGeneration === generation ? { ...current, heroes: { status: "error", data: null, error: errorMessage(error) } } : current);
    }
  }, [api, invalidateAuthentication, set]);

  const startQuery = useCallback(async (rawBattleTag: string) => {
    if (!isValidBattleTag(rawBattleTag) || stateRef.current.search.status === "loading") return false;
    const previousSession = stateRef.current.search.data?.sessionId;
    const next = beginPlayerSearch(stateRef.current, rawBattleTag, firstMode);
    stateRef.current = next;
    set(() => next);
    if (previousSession) {
      onSessionChange?.(null);
      await api.end(previousSession).catch(() => undefined);
    }
    try {
      const session = await api.start(normalizeBattleTag(rawBattleTag));
      if (stateRef.current.generation !== next.generation) { void api.end(session.sessionId).catch(() => undefined); return false; }
      const established = { ...stateRef.current, search: { status: "success", data: session, error: null }, battleTag: session.battleTag } as PlayerQueryState;
      stateRef.current = established;
      set(() => established);
      onSessionChange?.(session.sessionId);
      void loadCard(session.sessionId, next.generation);
      void loadCardImages(session.sessionId, next.generation);
      void loadRanks(session.sessionId, established.season, next.generation);
      void loadMatches(session.sessionId, established.season, established.mode, established.listGeneration);
      void loadHeroes(session.sessionId, established.season, established.mode, established.heroGeneration);
      return true;
    } catch (error) {
      if (isAuthenticationError(error)) { await invalidateAuthentication(); return false; }
      set((current) => current.generation === next.generation ? { ...current, search: { status: "error", data: null, error: errorMessage(error) } } : current);
      return false;
    }
  }, [api, firstMode, invalidateAuthentication, loadCard, loadCardImages, loadHeroes, loadMatches, loadRanks, onSessionChange, set]);

  const changeSeason = useCallback((season: PlayerQuerySeason) => {
    const current = stateRef.current; if (current.season === season) return;
    const sessionId = current.search.data?.sessionId;
    const next = sessionId ? changePlayerQuerySeason(current, season) : { ...current, season };
    stateRef.current = next; set(() => next);
    if (!sessionId) return;
    void loadRanks(sessionId, season, next.generation);
    void loadMatches(sessionId, season, next.mode, next.listGeneration);
    void loadHeroes(sessionId, season, next.mode, next.heroGeneration);
  }, [loadHeroes, loadMatches, loadRanks, set]);

  const selectMode = useCallback((mode: PlayerQueryMode) => {
    const sessionId = stateRef.current.search.data?.sessionId; if (!sessionId || stateRef.current.mode === mode) return;
    const previousListGeneration = stateRef.current.listGeneration;
    const next = changePlayerQueryMode(stateRef.current, mode); stateRef.current = next; set(() => next);
    if (next.listGeneration !== previousListGeneration) void loadMatches(sessionId, next.season, mode, next.listGeneration);
    void loadHeroes(sessionId, next.season, mode, next.heroGeneration);
  }, [loadHeroes, loadMatches, set]);

  const retry = useCallback((scope: PlayerQueryRetryScope) => {
    const current = stateRef.current; const sessionId = current.search.data?.sessionId; if (!sessionId) return;
    if (scope === "card") void loadCard(sessionId, current.generation);
    else if (scope === "cardImages") void loadCardImages(sessionId, current.generation);
    else if (scope === "ranks") void loadRanks(sessionId, current.season, current.generation);
    else if (scope === "matches") void loadMatches(sessionId, current.season, current.mode, current.listGeneration);
    else void loadHeroes(sessionId, current.season, current.mode, current.heroGeneration);
  }, [loadCard, loadCardImages, loadHeroes, loadMatches, loadRanks]);

  const leaveQuery = useCallback(() => {
    const sessionId = stateRef.current.search.data?.sessionId; set(() => initialPlayerQueryState(firstMode));
    if (sessionId) void api.end(sessionId).catch(() => undefined);
    onSessionChange?.(null);
  }, [api, firstMode, onSessionChange, set]);

  return { state, startQuery, changeSeason, selectMode, retry, leaveQuery };
}
