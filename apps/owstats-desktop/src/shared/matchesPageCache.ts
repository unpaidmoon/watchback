import type { FilterOptions, ImportSummary, MatchCursor, MatchFilters, MatchPage, MatchSummary, Overview } from "./types";

export type MatchesPageCache = {
  accountScope: string | null;
  revisionToken: string | null;
  overview: Overview | null;
  summary: ImportSummary | null;
  options: FilterOptions;
  filters: MatchFilters;
  matches: MatchSummary[];
  nextCursor: MatchCursor | null;
  totalMatches: number;
  selectedMatchId: string | null;
  loadedFilterKey: string;
  scrollTop: number;
  scrollAnchor: MatchScrollAnchor | null;
  hasLoaded: boolean;
};

export type MatchScrollAnchor = {
  matchId: string;
  offsetTop: number;
};

export type MatchScrollAnchorCandidate = {
  matchId: string;
  top: number;
  bottom: number;
};

const filterParamKeys = ["season", "hero", "map", "result", "mode", "primaryTag", "playerName", "startDate", "endDate", "favorite"];

let matchesPageCache: MatchesPageCache | null = null;
let cacheGeneration = 0;
let latestCacheWriteId = 0;
const preloadPromises = new Map<string, Promise<MatchesPageCache>>();

export function createMatchesPageLifetime() {
  let active = false;
  return {
    activate() {
      active = true;
    },
    deactivate() {
      active = false;
    },
    isActive() {
      return active;
    },
  };
}

export function getMatchesPageCache() {
  return matchesPageCache;
}

export function updateMatchesPageCache(patch: Partial<MatchesPageCache>) {
  matchesPageCache = createMatchesPageCache(patch);
  return matchesPageCache;
}

export function updateMatchFavoriteState<T extends { matchId: string; isFavorite: boolean }>(
  matches: T[],
  matchId: string,
  isFavorite: boolean,
) {
  return matches.map((match) =>
    match.matchId === matchId && match.isFavorite !== isFavorite
      ? { ...match, isFavorite }
      : match,
  );
}

function createMatchesPageCache(patch: Partial<MatchesPageCache>) {
  return {
    accountScope: null,
    revisionToken: null,
    overview: null,
    summary: null,
    options: { seasons: [], heroes: [], maps: [], results: [], modes: [] },
    filters: {},
    matches: [],
    nextCursor: null,
    totalMatches: 0,
    selectedMatchId: null,
    loadedFilterKey: "",
    scrollTop: 0,
    scrollAnchor: null,
    hasLoaded: false,
    ...matchesPageCache,
    ...patch,
  };
}

export function clearMatchesPageCache() {
  cacheGeneration += 1;
  matchesPageCache = null;
  preloadPromises.clear();
}

export function hasExplicitMatchFilterParams(params: URLSearchParams) {
  return filterParamKeys.some((key) =>
    key === "favorite" ? params.get(key) === "1" : Boolean(params.get(key)),
  );
}

export function chooseInitialMatchFilters(urlFilters: MatchFilters, cachedFilters: MatchFilters | undefined, hasExplicitFilters: boolean) {
  if (hasExplicitFilters) return urlFilters;
  return cachedFilters ?? urlFilters;
}

export function chooseInitialMatchId(
  urlMatchId: string | null,
  cachedMatchId: string | null | undefined,
  canHydrateCache: boolean,
) {
  return urlMatchId ?? (canHydrateCache ? cachedMatchId ?? null : null);
}

export function chooseMatchesPageNavigationState(
  urlFilters: MatchFilters,
  urlMatchId: string | null,
  cache: MatchesPageCache | null,
  hasExplicitFilters: boolean,
) {
  const filters = chooseInitialMatchFilters(urlFilters, cache?.filters, hasExplicitFilters);
  const canHydrateCache = canHydrateMatchesPageCache(cache, filters, hasExplicitFilters);
  return {
    filters,
    selectedMatchId: chooseInitialMatchId(urlMatchId, cache?.selectedMatchId, canHydrateCache),
    canHydrateCache,
  };
}

export function consumeMatchesSearchParamsNavigation(
  currentSearchParamsKey: string,
  lastWrittenSearchParamsKey: string | null,
) {
  return {
    shouldApplyNavigation: currentSearchParamsKey !== lastWrittenSearchParamsKey,
    nextLastWrittenSearchParamsKey: null,
  };
}

export function chooseMatchIdForLoadedMatches(
  selectedMatchId: string | null,
  matches: Array<{ matchId: string }>,
  isCurrentFilterLoaded: boolean,
) {
  if (!isCurrentFilterLoaded) return selectedMatchId;
  if (matches.length === 0) return null;
  if (selectedMatchId && matches.some((match) => match.matchId === selectedMatchId)) return selectedMatchId;
  return matches[0].matchId;
}

export function isLatestMatchRequest(requestId: number, latestRequestId: number) {
  return requestId === latestRequestId;
}

export function canCommitMatchTimelineRequest(
  requestId: number,
  latestRequestId: number,
  requestedFilterKey: string,
  currentFilterKey: string,
) {
  return isLatestMatchRequest(requestId, latestRequestId) && requestedFilterKey === currentFilterKey;
}

export function shouldLoadMatchesForFilter(
  currentFilterKey: string,
  loadedFilterKey: string,
  initialHydrationFilterKey: string | null,
) {
  if (currentFilterKey === loadedFilterKey) return false;
  return initialHydrationFilterKey !== currentFilterKey;
}

export function isValidMatchesPageCacheSnapshot(cache: MatchesPageCache | null): cache is MatchesPageCache {
  return cache !== null && cache.hasLoaded && cache.loadedFilterKey !== "" && cache.loadedFilterKey === matchFiltersKey(cache.filters);
}

export function canHydrateMatchesPageCache(
  cache: MatchesPageCache | null,
  filters: MatchFilters,
  hasExplicitFilters: boolean,
  contract?: { accountScope: string | null; revisionToken: string | null },
) {
  if (!isValidMatchesPageCacheSnapshot(cache)) return false;
  if (contract && (cache.accountScope !== contract.accountScope || cache.revisionToken !== contract.revisionToken)) return false;
  if (!hasExplicitFilters) return true;
  return cache.loadedFilterKey === matchFiltersKey(filters);
}

export function selectMatchScrollAnchor(candidates: MatchScrollAnchorCandidate[], containerTop: number): MatchScrollAnchor | null {
  const firstVisible = candidates.find((candidate) => candidate.bottom > containerTop);
  if (!firstVisible) return null;
  return {
    matchId: firstVisible.matchId,
    offsetTop: firstVisible.top - containerTop,
  };
}

export function computeMatchAnchorScrollTop({
  currentScrollTop,
  containerTop,
  itemTop,
  offsetTop,
}: {
  currentScrollTop: number;
  containerTop: number;
  itemTop: number;
  offsetTop: number;
}) {
  return currentScrollTop + itemTop - containerTop - offsetTop;
}

export function matchFiltersKey(filters: MatchFilters) {
  return JSON.stringify(
    Object.keys(filters)
      .sort()
      .reduce<Record<string, string | number | boolean>>((result, key) => {
        const value = filters[key as keyof MatchFilters];
        if (value !== undefined && value !== "") result[key] = value;
        return result;
      }, {}),
  );
}

export function guardMatchFilterChange(
  currentFilters: MatchFilters,
  nextFilters: MatchFilters,
  confirmDiscard: () => boolean,
) {
  return (
    matchFiltersKey(currentFilters) === matchFiltersKey(nextFilters) ||
    confirmDiscard()
  );
}

export function guardMatchesPageNavigation(
  transition: {
    currentFilters: MatchFilters;
    nextFilters: MatchFilters;
    currentSelectedMatchId: string | null;
    nextSelectedMatchId: string | null;
  },
  confirmDiscard: () => boolean,
) {
  const filtersUnchanged =
    matchFiltersKey(transition.currentFilters) ===
    matchFiltersKey(transition.nextFilters);
  const matchUnchanged =
    transition.currentSelectedMatchId === transition.nextSelectedMatchId;
  return (filtersUnchanged && matchUnchanged) || confirmDiscard();
}

export type MatchesPagePreloadLoaders = {
  getOverview: () => Promise<Overview>;
  listMatchFilters: () => Promise<FilterOptions>;
  listMatchTimeline: (filters: MatchFilters) => Promise<MatchSummary[] | MatchPage>;
};

export function preloadMatchesPageCache(
  loaders: MatchesPagePreloadLoaders,
  filters: MatchFilters = {},
  options: { cacheResult?: boolean } = {},
) {
  const filterKey = matchFiltersKey(filters);
  const preloadKey = `${filterKey}:${options.cacheResult === false ? "detached" : "cache"}`;
  const cache = matchesPageCache;
  if (isValidMatchesPageCacheSnapshot(cache) && cache.loadedFilterKey === filterKey) {
    return Promise.resolve(cache);
  }
  const pendingPreload = preloadPromises.get(preloadKey);
  if (pendingPreload) return pendingPreload;

  const preloadGeneration = cacheGeneration;
  const cacheWriteId = options.cacheResult === false ? null : ++latestCacheWriteId;
  let preloadPromise!: Promise<MatchesPageCache>;
  preloadPromise = Promise.all([loaders.getOverview(), loaders.listMatchFilters(), loaders.listMatchTimeline(filters)])
    .then(([overview, filterOptions, result]) => {
      const page = Array.isArray(result) ? null : result;
      const matches = page?.matches ?? result as MatchSummary[];
      const snapshot = createMatchesPageCache({
        overview,
        summary: overview.importSummary,
        options: filterOptions,
        filters,
        matches,
        nextCursor: page?.nextCursor ?? null,
        totalMatches: page?.total ?? matches.length,
        accountScope: page?.accountScope ?? null,
        revisionToken: page?.revisionToken ?? null,
        loadedFilterKey: filterKey,
        hasLoaded: true,
      });
      const canCacheResult =
        cacheWriteId !== null && preloadGeneration === cacheGeneration && cacheWriteId === latestCacheWriteId;
      return canCacheResult ? updateMatchesPageCache(snapshot) : snapshot;
    })
    .finally(() => {
      if (preloadPromises.get(preloadKey) === preloadPromise) preloadPromises.delete(preloadKey);
    });

  preloadPromises.set(preloadKey, preloadPromise);
  return preloadPromise;
}
