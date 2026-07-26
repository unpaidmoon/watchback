import { useCallback, useEffect, useRef, useState } from "react";
import { decodePlayerQueryError, loadPlayerQueryModePage, releasePlayerQueryMoreData } from "./playerQueryApi";
import type { PlayerQueryListScopeKey, PlayerQueryMatchRow, PlayerQueryMode, PlayerQueryModePage, PlayerQuerySeason } from "./playerQueryTypes";
import { isAuthenticationError } from "./playerQueryState";

export const PLAYER_QUERY_PAGE_SIZE = 12;
export const PLAYER_QUERY_SENTINEL_THRESHOLD_PX = 96;

export type PlayerQueryPaginationApi = {
  loadPage: typeof loadPlayerQueryModePage;
  releaseMoreData: typeof releasePlayerQueryMoreData;
};

const defaultApi: PlayerQueryPaginationApi = {
  loadPage: loadPlayerQueryModePage,
  releaseMoreData: releasePlayerQueryMoreData,
};

export function mergePlayerQueryMatches(
  current: readonly PlayerQueryMatchRow[],
  incoming: readonly PlayerQueryMatchRow[],
): PlayerQueryMatchRow[] {
  const seen = new Set(current.map((match) => match.matchId));
  const merged = [...current];
  for (const match of incoming) {
    if (seen.has(match.matchId)) continue;
    seen.add(match.matchId);
    merged.push(match);
  }
  return merged;
}

export function hasNewDownwardScrollIntent(previousScrollTop: number, nextScrollTop: number): boolean {
  return nextScrollTop > previousScrollTop;
}

export function isPlayerQuerySentinelNear(
  scrollTop: number,
  clientHeight: number,
  scrollHeight: number,
  threshold = PLAYER_QUERY_SENTINEL_THRESHOLD_PX,
): boolean {
  return scrollHeight - (scrollTop + clientHeight) <= threshold;
}

export function nextPaginationPage(page: number, response: PlayerQueryModePage) {
  return { nextPage: page + 1, endReached: response.endReached, retryPage: null };
}

export function failedPaginationPage(page: number) {
  return { nextPage: page, retryPage: page };
}

export function initialPaginationEndReached(initialEndReached?: boolean): boolean {
  return initialEndReached ?? false;
}

export type UsePlayerQueryPaginationOptions = {
  sessionId: string;
  season: PlayerQuerySeason;
  mode: PlayerQueryMode;
  listScopeKey: PlayerQueryListScopeKey;
  initialMatches: readonly PlayerQueryMatchRow[];
  initialEndReached?: boolean;
  api?: PlayerQueryPaginationApi;
  onAuthenticationInvalid?: () => void;
};

export function usePlayerQueryPagination({
  sessionId,
  season,
  mode,
  listScopeKey,
  initialMatches,
  initialEndReached,
  api = defaultApi,
  onAuthenticationInvalid,
}: UsePlayerQueryPaginationOptions) {
  const [matches, setMatches] = useState<PlayerQueryMatchRow[]>(() => mergePlayerQueryMatches([], initialMatches));
  const [nextPage, setNextPage] = useState(2);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retryPage, setRetryPage] = useState<number | null>(null);
  const initialEnded = initialPaginationEndReached(initialEndReached);
  const [endReached, setEndReached] = useState(initialEnded);
  const previousScrollTop = useRef(0);
  const requestGeneration = useRef(0);
  const loadingRef = useRef(false);
  const endReachedRef = useRef(initialEnded);
  const nextPageRef = useRef(2);
  const releaseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadNextPage = useCallback(async () => {
    if (loadingRef.current || endReachedRef.current) return;
    const page = nextPageRef.current;
    const generation = ++requestGeneration.current;
    loadingRef.current = true;
    setLoading(true);
    setError(null);
    try {
      const response = await api.loadPage(sessionId, season, mode, page);
      if (generation !== requestGeneration.current) return;
      setMatches((current) => mergePlayerQueryMatches(current, response.matches));
      const { nextPage: next, endReached: reached, retryPage: retryTarget } = nextPaginationPage(page, response);
      endReachedRef.current = reached;
      setEndReached(reached);
      nextPageRef.current = next;
      setNextPage(next);
      setRetryPage(retryTarget);
    } catch (cause) {
      if (generation !== requestGeneration.current) return;
      const failed = failedPaginationPage(page);
      nextPageRef.current = failed.nextPage;
      setNextPage(failed.nextPage);
      setRetryPage(failed.retryPage);
      const message = decodePlayerQueryError(cause, "加载失败，请稍后重试").message;
      if (isAuthenticationError(cause)) onAuthenticationInvalid?.();
      else setError(message);
    } finally {
      if (generation === requestGeneration.current) {
        loadingRef.current = false;
        setLoading(false);
      }
    }
  }, [api, listScopeKey, mode, onAuthenticationInvalid, season, sessionId]);

  const onListScroll = useCallback((element: Pick<HTMLElement, "scrollTop" | "clientHeight" | "scrollHeight">) => {
    const isDownward = hasNewDownwardScrollIntent(previousScrollTop.current, element.scrollTop);
    previousScrollTop.current = element.scrollTop;
    if (isDownward && isPlayerQuerySentinelNear(element.scrollTop, element.clientHeight, element.scrollHeight)) {
      void loadNextPage();
    }
  }, [loadNextPage]);

  const retry = useCallback(() => { void loadNextPage(); }, [loadNextPage]);

  useEffect(() => {
    // React StrictMode performs a synthetic setup/cleanup/setup cycle. Deferring the
    // release lets the second setup cancel that synthetic cleanup, while a real
    // route exit still releases page 2+ data on the next task.
    if (releaseTimer.current) clearTimeout(releaseTimer.current);
    releaseTimer.current = null;
    return () => {
      requestGeneration.current += 1;
      releaseTimer.current = setTimeout(() => {
        void api.releaseMoreData(sessionId).catch(() => undefined);
        releaseTimer.current = null;
      }, 0);
    };
  }, [api, sessionId]);

  return { listScopeKey, matches, nextPage, retryPage, loading, error, endReached, onListScroll, retry };
}
