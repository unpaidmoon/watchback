import type { FilterOptions, ImportSummary, MatchFilters, MatchSummary, Overview } from "./types";

export type MatchesPageCache = {
  overview: Overview | null;
  summary: ImportSummary | null;
  options: FilterOptions;
  filters: MatchFilters;
  matches: MatchSummary[];
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

const filterParamKeys = ["season", "hero", "map", "result", "mode", "primaryTag", "startDate", "endDate"];

let matchesPageCache: MatchesPageCache | null = null;
let preloadPromise: Promise<MatchesPageCache> | null = null;
let preloadFilterKey = "";

export function getMatchesPageCache() {
  return matchesPageCache;
}

export function updateMatchesPageCache(patch: Partial<MatchesPageCache>) {
  matchesPageCache = {
    overview: null,
    summary: null,
    options: { seasons: [], heroes: [], maps: [], results: [], modes: [] },
    filters: {},
    matches: [],
    loadedFilterKey: "",
    scrollTop: 0,
    scrollAnchor: null,
    hasLoaded: false,
    ...matchesPageCache,
    ...patch,
  };
  return matchesPageCache;
}

export function clearMatchesPageCache() {
  matchesPageCache = null;
  preloadPromise = null;
  preloadFilterKey = "";
}

export function hasExplicitMatchFilterParams(params: URLSearchParams) {
  return filterParamKeys.some((key) => Boolean(params.get(key)));
}

export function chooseInitialMatchFilters(urlFilters: MatchFilters, cachedFilters: MatchFilters | undefined, hasExplicitFilters: boolean) {
  if (hasExplicitFilters) return urlFilters;
  return cachedFilters ?? urlFilters;
}

export function canHydrateMatchesPageCache(cache: MatchesPageCache | null, filters: MatchFilters, hasExplicitFilters: boolean) {
  if (!cache?.hasLoaded) return false;
  if (!hasExplicitFilters) return true;
  return cache.loadedFilterKey !== "" && cache.loadedFilterKey === matchFiltersKey(filters);
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
      .reduce<Record<string, string | number>>((result, key) => {
        const value = filters[key as keyof MatchFilters];
        if (value !== undefined && value !== "") result[key] = value;
        return result;
      }, {}),
  );
}

export type MatchesPagePreloadLoaders = {
  getOverview: () => Promise<Overview>;
  listMatchFilters: () => Promise<FilterOptions>;
  listMatchTimeline: (filters: MatchFilters) => Promise<MatchSummary[]>;
};

export function preloadMatchesPageCache(loaders: MatchesPagePreloadLoaders, filters: MatchFilters = {}) {
  const filterKey = matchFiltersKey(filters);
  if (matchesPageCache?.hasLoaded && matchesPageCache.loadedFilterKey === filterKey) {
    return Promise.resolve(matchesPageCache);
  }
  if (preloadPromise && preloadFilterKey === filterKey) return preloadPromise;

  preloadFilterKey = filterKey;
  preloadPromise = Promise.all([loaders.getOverview(), loaders.listMatchFilters(), loaders.listMatchTimeline(filters)])
    .then(([overview, options, matches]) =>
      updateMatchesPageCache({
        overview,
        summary: overview.importSummary,
        options,
        filters,
        matches,
        loadedFilterKey: filterKey,
        hasLoaded: true,
      }),
    )
    .finally(() => {
      preloadPromise = null;
      preloadFilterKey = "";
    });

  return preloadPromise;
}
