import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  filtersFromParams,
  restoreRejectedMatchesSearchParams,
  validateFilters,
} from "../src/features/matches/matchFilters.ts";
import * as matchesPageCacheModule from "../src/shared/matchesPageCache.ts";
import {
  canCommitMatchTimelineRequest,
  canHydrateMatchesPageCache,
  chooseInitialMatchId,
  chooseInitialMatchFilters,
  chooseMatchIdForLoadedMatches,
  chooseMatchesPageNavigationState,
  clearMatchesPageCache,
  consumeMatchesSearchParamsNavigation,
  computeMatchAnchorScrollTop,
  getMatchesPageCache,
  guardMatchFilterChange,
  guardMatchesPageNavigation,
  hasExplicitMatchFilterParams,
  isLatestMatchRequest,
  invalidateStandaloneMatchFavoritePersistence,
  matchFiltersKey,
  preloadMatchesPageCache,
  selectMatchScrollAnchor,
  shouldInvalidateStandaloneFavoriteCache,
  shouldLoadMatchesForFilter,
  updateMatchesPageCache,
} from "../src/shared/matchesPageCache.ts";
import type { FilterOptions, MatchFilters, MatchSummary, Overview } from "../src/shared/types.ts";

const updateMatchFavoriteState = (
  matchesPageCacheModule as unknown as {
    updateMatchFavoriteState: <T extends { matchId: string; isFavorite: boolean }>(
      matches: T[],
      matchId: string,
      isFavorite: boolean,
    ) => T[];
  }
).updateMatchFavoriteState;
assert.equal(typeof updateMatchFavoriteState, "function", "match favorite updates should expose an immutable list helper");

const createMatchesPageLifetime = (
  matchesPageCacheModule as unknown as {
    createMatchesPageLifetime: () => {
      activate: () => void;
      deactivate: () => void;
      isActive: () => boolean;
    };
  }
).createMatchesPageLifetime;
assert.equal(typeof createMatchesPageLifetime, "function", "matches page should expose its tested lifetime gate");
const pageLifetime = createMatchesPageLifetime();
assert.equal(pageLifetime.isActive(), false, "a page lifetime starts inactive before its effect setup");
pageLifetime.activate();
assert.equal(pageLifetime.isActive(), true, "effect setup activates the page lifetime");
pageLifetime.deactivate();
assert.equal(pageLifetime.isActive(), false, "effect cleanup deactivates the page lifetime");
pageLifetime.activate();
assert.equal(
  pageLifetime.isActive(),
  true,
  "StrictMode cleanup followed by replayed effect setup reactivates the same page lifetime",
);

function createDeferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((resolvePromise) => {
    resolve = () => resolvePromise();
  });
  return { promise, resolve };
}

clearMatchesPageCache();

const cachedFilters: MatchFilters = { hero: "ana", result: "win", dateFrom: "2026-06-01" };
updateMatchesPageCache({ filters: cachedFilters, selectedMatchId: "cached-match", scrollTop: 420 });

assert.deepEqual(getMatchesPageCache()?.filters, cachedFilters, "cache keeps match filters in memory");
assert.equal(getMatchesPageCache()?.selectedMatchId, "cached-match", "cache keeps the selected match in memory");
assert.equal(getMatchesPageCache()?.scrollTop, 420, "cache keeps scroll position in memory");

assert.equal(chooseInitialMatchId("url-match", "cached-match", true), "url-match", "URL match id wins over cached selection");
assert.equal(chooseInitialMatchId(null, "cached-match", true), "cached-match", "cached selection is restored with the cached list");
assert.equal(chooseInitialMatchId(null, "cached-match", false), null, "cached selection is ignored when the cached list cannot hydrate");

const loadedMatchIds = [{ matchId: "first-match" }, { matchId: "selected-match" }];
assert.equal(
  chooseMatchIdForLoadedMatches("url-match", [], false),
  "url-match",
  "selection reconciliation preserves a deep-linked match before the current filter loads",
);
assert.equal(chooseMatchIdForLoadedMatches("url-match", [], true), null, "a loaded empty list clears selection");
assert.equal(
  chooseMatchIdForLoadedMatches("selected-match", loadedMatchIds, true),
  "selected-match",
  "a loaded list preserves a selection it contains",
);
assert.equal(
  chooseMatchIdForLoadedMatches("missing-match", loadedMatchIds, true),
  "first-match",
  "a loaded list falls back to its first row when selection is missing",
);

const favoriteRows = [
  { matchId: "target", isFavorite: false, label: "目标" },
  { matchId: "other", isFavorite: true, label: "其它" },
];
const favoritedRows = updateMatchFavoriteState(favoriteRows, "target", true);
assert.notEqual(favoritedRows, favoriteRows, "favorite updates always return a new array");
assert.notEqual(favoritedRows[0], favoriteRows[0], "a changed target row is cloned");
assert.deepEqual(favoritedRows[0], { matchId: "target", isFavorite: true, label: "目标" });
assert.equal(favoritedRows[1], favoriteRows[1], "non-target rows preserve their references");

const unchangedFavoriteRows = updateMatchFavoriteState(favoritedRows, "target", true);
assert.notEqual(unchangedFavoriteRows, favoritedRows, "same-value updates still return a new array");
assert.equal(unchangedFavoriteRows[0], favoritedRows[0], "a same-value target preserves its reference");
assert.equal(unchangedFavoriteRows[1], favoritedRows[1], "same-value updates preserve non-target references");

const missingFavoriteRows = updateMatchFavoriteState(favoriteRows, "missing", false);
assert.notEqual(missingFavoriteRows, favoriteRows, "missing-id updates still return a new array");
assert.equal(missingFavoriteRows[0], favoriteRows[0], "missing-id updates preserve the first row");
assert.equal(missingFavoriteRows[1], favoriteRows[1], "missing-id updates preserve the second row");

assert.equal(isLatestMatchRequest(1, 2), false, "an older match-list request cannot apply after a newer request starts");
assert.equal(isLatestMatchRequest(2, 2), true, "the latest match-list request may apply its result");

const initialHydrationFilterKey = matchFiltersKey({ season: 22 });
assert.equal(
  shouldLoadMatchesForFilter(initialHydrationFilterKey, "", initialHydrationFilterKey),
  false,
  "the first StrictMode filter effect lets initial hydration own its filter",
);
assert.equal(
  shouldLoadMatchesForFilter(initialHydrationFilterKey, "", initialHydrationFilterKey),
  false,
  "the replayed StrictMode filter effect still lets initial hydration own its filter",
);
assert.equal(
  shouldLoadMatchesForFilter(matchFiltersKey({ season: 23 }), "", initialHydrationFilterKey),
  true,
  "a genuinely different filter starts a timeline load",
);
assert.equal(
  shouldLoadMatchesForFilter(initialHydrationFilterKey, initialHydrationFilterKey, null),
  false,
  "an already loaded filter never starts another timeline load",
);
assert.equal(
  canCommitMatchTimelineRequest(2, 2, matchFiltersKey({ season: 22 }), matchFiltersKey({ season: 22 })),
  true,
  "the latest timeline request may commit while its filter is still current",
);
assert.equal(
  canCommitMatchTimelineRequest(1, 2, matchFiltersKey({ season: 22 }), matchFiltersKey({ season: 22 })),
  false,
  "an obsolete timeline request cannot commit even when its old filter returns",
);
assert.equal(
  canCommitMatchTimelineRequest(2, 2, matchFiltersKey({ season: 22 }), matchFiltersKey({ season: 23 })),
  false,
  "a hydration timeline for filter A cannot overwrite the current filter B",
);
assert.deepEqual(
  validateFilters(filtersFromParams(new URLSearchParams("season=22&mode=bogus")), {
    seasons: [22],
    heroes: [],
    maps: [],
    results: [],
    modes: ["SportPreset"],
  }),
  { season: 22 },
  "URL filters remove an unsupported mode after options load",
);

assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("hero=ana")), true, "hero query counts as explicit filters");
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("playerName=是糖三分毒%235811")), true, "playerName query counts as explicit filters");
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("season=21")), true, "S21 query counts as an explicit filter");
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("favorite=1")), true, "favorite query counts as an explicit filter");
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("favorite=true")), false, "favorite=true is not an explicit filter");
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("favorite=0")), false, "favorite=0 is not an explicit filter");
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("favorite=")), false, "an empty favorite value is not an explicit filter");
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("qa=1")), false, "qa query does not override cached filters");

assert.deepEqual(
  chooseInitialMatchFilters({ map: "ilios" }, cachedFilters, true),
  { map: "ilios" },
  "explicit URL filters win over cached filters",
);

assert.deepEqual(
  chooseInitialMatchFilters({}, cachedFilters, false),
  cachedFilters,
  "cached filters are restored when URL has no explicit filters",
);
assert.deepEqual(
  chooseInitialMatchFilters({ season: 21 }, { season: 22 }, true),
  { season: 21 },
  "an explicit S21 URL filter wins over a cached S22 filter",
);

assert.equal(
  matchFiltersKey({ result: "win", hero: "ana" }),
  matchFiltersKey({ hero: "ana", result: "win" }),
  "filter cache key is stable regardless of object insertion order",
);
assert.notEqual(
  matchFiltersKey({ hero: "ana" }),
  matchFiltersKey({ hero: "ana", playerName: "是糖三分毒#5811" }),
  "playerName participates in the loaded filter cache key",
);
assert.notEqual(
  matchFiltersKey({ season: 21 }),
  matchFiltersKey({ season: 22 }),
  "S21 and S22 use different match-list cache keys",
);
assert.notEqual(
  matchFiltersKey({}),
  matchFiltersKey({ favoriteOnly: true }),
  "favorites-only filters produce a distinct cache key",
);

assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, false), false, "cache does not hydrate before first successful load");
assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, true), false, "cache without loaded key does not hydrate explicit filters");

updateMatchesPageCache({ loadedFilterKey: matchFiltersKey(cachedFilters), hasLoaded: true });
assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, false), true, "cache hydrates normal route re-entry");
assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, true), true, "cache hydrates explicit filters when loaded key matches");

for (const invalidFavorite of ["favorite=true", "favorite=0", "favorite="]) {
  const params = new URLSearchParams(invalidFavorite);
  const navigation = chooseMatchesPageNavigationState(
    filtersFromParams(params),
    null,
    getMatchesPageCache(),
    hasExplicitMatchFilterParams(params),
  );
  assert.deepEqual(navigation.filters, cachedFilters, `${invalidFavorite} does not replace cached filters`);
  assert.equal(navigation.selectedMatchId, "cached-match", `${invalidFavorite} does not replace the cached selected match`);
  assert.equal(navigation.canHydrateCache, true, `${invalidFavorite} keeps the existing cache hydratable`);
}

const explicitNavigation = chooseMatchesPageNavigationState({ season: 21 }, "url-match", getMatchesPageCache(), true);
assert.deepEqual(explicitNavigation.filters, { season: 21 }, "an external explicit query replaces cached filters");
assert.equal(explicitNavigation.selectedMatchId, "url-match", "an external explicit query keeps its URL match id");
assert.equal(explicitNavigation.canHydrateCache, false, "an external explicit query does not hydrate a different cached list");

const matchOnlyNavigation = chooseMatchesPageNavigationState({}, "url-match", getMatchesPageCache(), false);
assert.deepEqual(matchOnlyNavigation.filters, cachedFilters, "a matchId-only URL keeps cached filter semantics");
assert.equal(matchOnlyNavigation.selectedMatchId, "url-match", "a matchId-only URL wins over cached selection");
assert.equal(matchOnlyNavigation.canHydrateCache, true, "a matchId-only URL can hydrate the valid cached list");

const bareNavigation = chooseMatchesPageNavigationState({}, null, getMatchesPageCache(), false);
assert.deepEqual(bareNavigation.filters, cachedFilters, "a bare URL restores cached filters");
assert.equal(bareNavigation.selectedMatchId, "cached-match", "a bare URL restores valid cached selection");
assert.equal(bareNavigation.canHydrateCache, true, "a bare URL hydrates a valid cached list");

const localBareSearchParamsKey = new URLSearchParams().toString();
const localBareEcho = consumeMatchesSearchParamsNavigation(localBareSearchParamsKey, localBareSearchParamsKey);
assert.equal(localBareEcho.shouldApplyNavigation, false, "a locally written bare URL is consumed as an echo");
assert.equal(localBareEcho.nextLastWrittenSearchParamsKey, null, "consuming a local URL echo clears its source marker");
const locallyClearedFilters: MatchFilters = {};
const filtersAfterLocalEcho = localBareEcho.shouldApplyNavigation
  ? chooseMatchesPageNavigationState({}, null, getMatchesPageCache(), false).filters
  : locallyClearedFilters;
assert.deepEqual(filtersAfterLocalEcho, {}, "a local clear-all action is not replaced by the previous cached filters");
assert.equal(
  shouldLoadMatchesForFilter(matchFiltersKey(filtersAfterLocalEcho), matchFiltersKey(cachedFilters), null),
  true,
  "the retained empty filters start an unfiltered timeline load",
);
const laterExternalBareNavigation = consumeMatchesSearchParamsNavigation(
  localBareSearchParamsKey,
  localBareEcho.nextLastWrittenSearchParamsKey,
);
assert.equal(laterExternalBareNavigation.shouldApplyNavigation, true, "the consumed marker cannot hide a later external bare URL");
assert.deepEqual(
  chooseMatchesPageNavigationState({}, null, getMatchesPageCache(), false).filters,
  cachedFilters,
  "an external bare URL keeps restoring a valid cached filter snapshot",
);

assert.equal(
  canHydrateMatchesPageCache(
    { ...getMatchesPageCache()!, loadedFilterKey: matchFiltersKey({ hero: "genji" }) },
    cachedFilters,
    false,
  ),
  false,
  "cache does not hydrate when its filter snapshot and loaded key disagree",
);
assert.equal(
  canHydrateMatchesPageCache(getMatchesPageCache(), { hero: "genji" }, true),
  false,
  "cache does not hydrate when explicit filters ask for a different list",
);
assert.equal(
  canHydrateMatchesPageCache(getMatchesPageCache(), { season: 21 }, true),
  false,
  "an S22 cache does not hydrate an explicit S21 route",
);

assert.deepEqual(
  selectMatchScrollAnchor(
    [
      { matchId: "m1", top: 80, bottom: 120 },
      { matchId: "m2", top: 128, bottom: 220 },
      { matchId: "m3", top: 232, bottom: 330 },
    ],
    100,
  ),
  { matchId: "m1", offsetTop: -20 },
  "scroll anchor tracks the first visible match and its offset from the container top",
);

assert.deepEqual(
  selectMatchScrollAnchor(
    [
      { matchId: "m1", top: 20, bottom: 90 },
      { matchId: "m2", top: 128, bottom: 220 },
    ],
    100,
  ),
  { matchId: "m2", offsetTop: 28 },
  "scroll anchor skips rows above the visible container",
);

assert.equal(
  computeMatchAnchorScrollTop({ currentScrollTop: 500, containerTop: 100, itemTop: 128, offsetTop: -20 }),
  548,
  "anchor restore keeps the same match offset after remount",
);

clearMatchesPageCache();

let matchListCalls = 0;
const overview: Overview = {
  databasePath: "local.db",
  importSummary: { season: 22, matchesSeen: 2, detailsSeen: 2, presetMatches: 2, stadiumMatches: 0, importedAt: "2026-06-29" },
};
const options: FilterOptions = { seasons: [22], heroes: [], maps: [], results: ["win"], modes: ["SportPreset"] };
const matches = [
  {
    matchId: "m1",
    season: 22,
    bucket: "preset",
    gameMode: "SportPreset",
    modeLabel: "竞技预设",
    result: "win",
    resultLabel: "胜利",
    mapGuid: "map",
    mapName: "地图",
    heroGuid: "hero",
    heroName: "英雄",
    beginTs: 1,
    beginLabel: "06-29 20:00",
    teamScore: 1,
    opponentScore: 0,
    kills: 1,
    assists: 2,
    deaths: 3,
    damage: 4,
    healing: 5,
    mitigation: 6,
    durationSec: 60,
    evaluation: null,
    teamAwards: [],
    isFavorite: false,
  },
] satisfies MatchSummary[];

clearMatchesPageCache();
updateMatchesPageCache({
  filters: { season: 22 },
  matches,
  selectedMatchId: "m1",
  loadedFilterKey: matchFiltersKey({ season: 22 }),
  scrollTop: 420,
  scrollAnchor: { matchId: "m1", offsetTop: 12 },
  hasLoaded: true,
});
const favoriteCacheRows = updateMatchFavoriteState(matches, "m1", true);
updateMatchesPageCache({ matches: favoriteCacheRows });
assert.equal(getMatchesPageCache()?.matches[0]?.isFavorite, true, "favorite updates are persisted in the list cache");
assert.equal(getMatchesPageCache()?.selectedMatchId, "m1", "favorite cache writes preserve selection");
assert.equal(getMatchesPageCache()?.scrollTop, 420, "favorite cache writes preserve scroll position");
assert.deepEqual(
  getMatchesPageCache()?.scrollAnchor,
  { matchId: "m1", offsetTop: 12 },
  "favorite cache writes preserve the scroll anchor",
);
assert.equal(
  Object.hasOwn(getMatchesPageCache()?.matches[0] ?? {}, "note"),
  false,
  "the list cache never receives favorite note text",
);

const requestedPreloadFilters: MatchFilters = { season: 23 };
const freshPreloadMatches = [{ ...matches[0], matchId: "fresh-match", season: 23 }] satisfies MatchSummary[];
updateMatchesPageCache({
  filters: { season: 22 },
  matches,
  loadedFilterKey: matchFiltersKey(requestedPreloadFilters),
  hasLoaded: true,
});
assert.equal(
  canHydrateMatchesPageCache(getMatchesPageCache(), requestedPreloadFilters, true),
  false,
  "an invalid cache snapshot is rejected even when its loaded key matches the requested preload",
);

let invalidCacheLoaderCalls = 0;
const fallbackPreload = await preloadMatchesPageCache(
  {
    getOverview: async () => {
      invalidCacheLoaderCalls += 1;
      return overview;
    },
    listMatchFilters: async () => {
      invalidCacheLoaderCalls += 1;
      return options;
    },
    listMatchTimeline: async (filters) => {
      invalidCacheLoaderCalls += 1;
      assert.deepEqual(filters, requestedPreloadFilters, "fallback preload requests the intended filters");
      return freshPreloadMatches;
    },
  },
  requestedPreloadFilters,
  { cacheResult: false },
);

assert.equal(invalidCacheLoaderCalls, 3, "fallback preload reloads every dependency instead of reusing an invalid snapshot");
assert.deepEqual(fallbackPreload.filters, requestedPreloadFilters, "fallback preload returns the requested filters");
assert.equal(fallbackPreload.loadedFilterKey, matchFiltersKey(requestedPreloadFilters), "fallback preload returns the requested key");
assert.deepEqual(fallbackPreload.matches, freshPreloadMatches, "fallback preload returns the freshly loaded list");

clearMatchesPageCache();

const preloaded = await preloadMatchesPageCache({
  getOverview: async () => overview,
  listMatchFilters: async () => options,
  listMatchTimeline: async (filters) => {
    matchListCalls += 1;
    assert.deepEqual(filters, {}, "startup preload uses empty match filters");
    return matches;
  },
});

assert.equal(preloaded.hasLoaded, true, "startup preload marks cache as loaded");
assert.equal(preloaded.matches.length, 1, "startup preload stores match list");
assert.equal(preloaded.loadedFilterKey, matchFiltersKey({}), "startup preload records loaded filter key");

await preloadMatchesPageCache({
  getOverview: async () => overview,
  listMatchFilters: async () => options,
  listMatchTimeline: async () => {
    matchListCalls += 1;
    return matches;
  },
});

assert.equal(matchListCalls, 1, "startup preload does not reload an already cached default list");

clearMatchesPageCache();
let season21LoaderFilters: MatchFilters | undefined;
const season21Preloaded = await preloadMatchesPageCache(
  {
    getOverview: async () => overview,
    listMatchFilters: async () => options,
    listMatchTimeline: async (filters) => {
      season21LoaderFilters = filters;
      return matches;
    },
  },
  { season: 21 },
);

assert.deepEqual(season21LoaderFilters, { season: 21 }, "S21 preload passes the selected season to the match-list loader");
assert.equal(season21Preloaded.loadedFilterKey, matchFiltersKey({ season: 21 }), "S21 preload stores the S21 cache key");
assert.equal(getMatchesPageCache()?.loadedFilterKey, matchFiltersKey({ season: 21 }), "S21 preload caches its own loaded key");

clearMatchesPageCache();
const detachedPreload = await preloadMatchesPageCache(
  {
    getOverview: async () => overview,
    listMatchFilters: async () => options,
    listMatchTimeline: async () => matches,
  },
  {},
  { cacheResult: false },
);
assert.equal(detachedPreload.hasLoaded, true, "a detached preload still returns a complete page snapshot");
assert.equal(getMatchesPageCache(), null, "a detached preload leaves cache commit to the current page request");

clearMatchesPageCache();
const policyGate = createDeferred();
let detachedPolicyLoaderCalls = 0;
let cachedPolicyLoaderCalls = 0;
const detachedPolicyMatches = [{ ...matches[0], matchId: "detached-policy-match" }] satisfies MatchSummary[];
const cachedPolicyMatches = [{ ...matches[0], matchId: "cached-policy-match" }] satisfies MatchSummary[];
const detachedPolicyPromise = preloadMatchesPageCache(
  {
    getOverview: async () => {
      detachedPolicyLoaderCalls += 1;
      return overview;
    },
    listMatchFilters: async () => {
      detachedPolicyLoaderCalls += 1;
      return options;
    },
    listMatchTimeline: async () => {
      detachedPolicyLoaderCalls += 1;
      await policyGate.promise;
      return detachedPolicyMatches;
    },
  },
  {},
  { cacheResult: false },
);
const cachedPolicyPromise = preloadMatchesPageCache({
  getOverview: async () => {
    cachedPolicyLoaderCalls += 1;
    return overview;
  },
  listMatchFilters: async () => {
    cachedPolicyLoaderCalls += 1;
    return options;
  },
  listMatchTimeline: async () => {
    cachedPolicyLoaderCalls += 1;
    await policyGate.promise;
    return cachedPolicyMatches;
  },
});
policyGate.resolve();
const [detachedPolicyResult, cachedPolicyResult] = await Promise.all([detachedPolicyPromise, cachedPolicyPromise]);

assert.equal(detachedPolicyLoaderCalls, 3, "detached preload runs its own loader set");
assert.equal(cachedPolicyLoaderCalls, 3, "cache-writing preload does not inherit detached preload side effects");
assert.deepEqual(detachedPolicyResult.matches, detachedPolicyMatches, "detached preload returns its own result");
assert.deepEqual(cachedPolicyResult.matches, cachedPolicyMatches, "cache-writing preload returns its own result");
assert.deepEqual(getMatchesPageCache()?.matches, cachedPolicyMatches, "only the default preload policy updates the global cache");

clearMatchesPageCache();
const clearedPreloadFilters: MatchFilters = { season: 23 };
const oldPreloadGate = createDeferred();
const newPreloadGate = createDeferred();
const oldPreloadMatches = [{ ...matches[0], matchId: "old-cleared-preload", season: 23 }] satisfies MatchSummary[];
const newPreloadMatches = [{ ...matches[0], matchId: "new-after-clear-preload", season: 23 }] satisfies MatchSummary[];
const probeMatches = [{ ...matches[0], matchId: "same-key-probe", season: 23 }] satisfies MatchSummary[];
const oldPreloadPromise = preloadMatchesPageCache(
  {
    getOverview: async () => overview,
    listMatchFilters: async () => options,
    listMatchTimeline: async () => {
      await oldPreloadGate.promise;
      return oldPreloadMatches;
    },
  },
  clearedPreloadFilters,
);

clearMatchesPageCache();
let newPreloadLoaderCalls = 0;
const newPreloadPromise = preloadMatchesPageCache(
  {
    getOverview: async () => {
      newPreloadLoaderCalls += 1;
      return overview;
    },
    listMatchFilters: async () => {
      newPreloadLoaderCalls += 1;
      return options;
    },
    listMatchTimeline: async () => {
      newPreloadLoaderCalls += 1;
      await newPreloadGate.promise;
      return newPreloadMatches;
    },
  },
  clearedPreloadFilters,
);
assert.equal(newPreloadLoaderCalls, 3, "clear removes old pending dedupe so a same-key preload starts fresh loaders");

oldPreloadGate.resolve();
const oldPreloadResult = await oldPreloadPromise;
assert.deepEqual(oldPreloadResult.matches, oldPreloadMatches, "the cleared preload caller still receives its completed snapshot");
assert.equal(getMatchesPageCache(), null, "a preload started before clear cannot restore its old snapshot globally");

let sameKeyProbeLoaderCalls = 0;
const sameKeyProbeResult = await preloadMatchesPageCache(
  {
    getOverview: async () => {
      sameKeyProbeLoaderCalls += 1;
      return overview;
    },
    listMatchFilters: async () => {
      sameKeyProbeLoaderCalls += 1;
      return options;
    },
    listMatchTimeline: async () => {
      sameKeyProbeLoaderCalls += 1;
      return probeMatches;
    },
  },
  clearedPreloadFilters,
  { cacheResult: false },
);
assert.equal(sameKeyProbeLoaderCalls, 3, "the old result cannot shortcut a same-key detached probe after clear");
assert.deepEqual(sameKeyProbeResult.matches, probeMatches, "the same-key probe receives fresh loader data");
assert.equal(getMatchesPageCache(), null, "the detached probe also leaves the global cache empty");

let joinedNewLoaderCalls = 0;
const joinedNewPreloadPromise = preloadMatchesPageCache(
  {
    getOverview: async () => {
      joinedNewLoaderCalls += 1;
      return overview;
    },
    listMatchFilters: async () => {
      joinedNewLoaderCalls += 1;
      return options;
    },
    listMatchTimeline: async () => {
      joinedNewLoaderCalls += 1;
      return newPreloadMatches;
    },
  },
  clearedPreloadFilters,
);
assert.equal(joinedNewPreloadPromise, newPreloadPromise, "old promise cleanup cannot remove the new same-key pending preload");
assert.equal(joinedNewLoaderCalls, 0, "a caller still joins the new preload after the old promise settles");

newPreloadGate.resolve();
const [newPreloadResult, joinedNewPreloadResult] = await Promise.all([newPreloadPromise, joinedNewPreloadPromise]);
assert.deepEqual(newPreloadResult.matches, newPreloadMatches, "the post-clear preload returns its new snapshot");
assert.deepEqual(joinedNewPreloadResult.matches, newPreloadMatches, "the joined caller receives the post-clear snapshot");
assert.deepEqual(getMatchesPageCache()?.matches, newPreloadMatches, "the post-clear preload may update the global cache normally");

clearMatchesPageCache();
const olderFilterGate = createDeferred();
const newerFilterGate = createDeferred();
const olderFilterMatches = [{ ...matches[0], matchId: "older-filter-write", season: 21 }] satisfies MatchSummary[];
const newerFilterMatches = [{ ...matches[0], matchId: "newer-filter-write", season: 23 }] satisfies MatchSummary[];
const olderFilterPreload = preloadMatchesPageCache(
  {
    getOverview: async () => overview,
    listMatchFilters: async () => options,
    listMatchTimeline: async () => {
      await olderFilterGate.promise;
      return olderFilterMatches;
    },
  },
  { season: 21 },
);
const newerFilterPreload = preloadMatchesPageCache(
  {
    getOverview: async () => overview,
    listMatchFilters: async () => options,
    listMatchTimeline: async () => {
      await newerFilterGate.promise;
      return newerFilterMatches;
    },
  },
  { season: 23 },
);

newerFilterGate.resolve();
const newerFilterResult = await newerFilterPreload;
assert.deepEqual(newerFilterResult.matches, newerFilterMatches, "the later-started default preload returns its own snapshot");
assert.deepEqual(getMatchesPageCache()?.matches, newerFilterMatches, "the later-started default preload writes the global cache");

olderFilterGate.resolve();
const olderFilterResult = await olderFilterPreload;
assert.deepEqual(olderFilterResult.matches, olderFilterMatches, "the older default preload caller still receives its result");
assert.deepEqual(
  getMatchesPageCache()?.matches,
  newerFilterMatches,
  "a late older default preload cannot overwrite the later-started filter snapshot",
);

const matchesPageSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");
const matchDetailPageSource = readFileSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url), "utf8");

assert.equal(shouldInvalidateStandaloneFavoriteCache(null, { note: "" }), true);
assert.equal(shouldInvalidateStandaloneFavoriteCache({ note: "old" }, null), true);
assert.equal(
  shouldInvalidateStandaloneFavoriteCache({ note: "old" }, { note: "new" }),
  false,
  "a note-only update keeps the timeline favorite presence unchanged",
);
assert.equal(
  shouldInvalidateStandaloneFavoriteCache(undefined, { note: "" }),
  false,
  "favorite presence is not invalidated before standalone detail has loaded",
);

updateMatchesPageCache({
  filters: { favoriteOnly: true },
  loadedFilterKey: matchFiltersKey({ favoriteOnly: true }),
  hasLoaded: true,
});
invalidateStandaloneMatchFavoritePersistence();
assert.equal(
  getMatchesPageCache(),
  null,
  "standalone favorite persistence invalidates the hydrated timeline, including favorite-only results",
);
assert.match(
  matchDetailPageSource,
  /const favoriteRef = useRef<MatchDetailView\["favorite"\] \| undefined>\(undefined\);[\s\S]*const handleFavoritePersisted[\s\S]*shouldInvalidateStandaloneFavoriteCache\(favoriteRef\.current, favorite\)[\s\S]*invalidateStandaloneMatchFavoritePersistence\(\);[\s\S]*favoriteRef\.current = favorite;[\s\S]*setDetail/,
  "standalone persistence compares and updates a stable favorite ref before updating detail state",
);
assert.match(
  matchDetailPageSource,
  /const nextDetail = await getMatchDetails\(nextMatchId\);\s*favoriteRef\.current = nextDetail\.favorite;\s*setDetail\(nextDetail\);/,
  "loading standalone detail synchronizes the favorite-presence ref",
);

let sameFilterConfirmCount = 0;
assert.equal(
  guardMatchFilterChange(
    { season: 23, favoriteOnly: true },
    { favoriteOnly: true, season: 23 },
    () => {
      sameFilterConfirmCount += 1;
      return false;
    },
  ),
  true,
);
assert.equal(sameFilterConfirmCount, 0, "an equivalent filter key does not prompt");

let rejectedFilterConfirmCount = 0;
assert.equal(
  guardMatchFilterChange({ season: 22 }, { season: 23 }, () => {
    rejectedFilterConfirmCount += 1;
    return false;
  }),
  false,
);
assert.equal(rejectedFilterConfirmCount, 1);

let acceptedFilterConfirmCount = 0;
assert.equal(
  guardMatchFilterChange({ season: 22 }, { season: 23 }, () => {
    acceptedFilterConfirmCount += 1;
    return true;
  }),
  true,
);
assert.equal(acceptedFilterConfirmCount, 1);

let unchangedNavigationConfirmCount = 0;
assert.equal(
  guardMatchesPageNavigation(
    {
      currentFilters: { season: 23, favoriteOnly: true },
      nextFilters: { favoriteOnly: true, season: 23 },
      currentSelectedMatchId: "match-a",
      nextSelectedMatchId: "match-a",
    },
    () => {
      unchangedNavigationConfirmCount += 1;
      return false;
    },
  ),
  true,
);
assert.equal(unchangedNavigationConfirmCount, 0);

let matchOnlyNavigationConfirmCount = 0;
assert.equal(
  guardMatchesPageNavigation(
    {
      currentFilters: { season: 23 },
      nextFilters: { season: 23 },
      currentSelectedMatchId: "match-a",
      nextSelectedMatchId: "match-b",
    },
    () => {
      matchOnlyNavigationConfirmCount += 1;
      return false;
    },
  ),
  false,
);
assert.equal(matchOnlyNavigationConfirmCount, 1, "match-only URL transitions can be rejected while dirty or busy");

let filterOnlyNavigationConfirmCount = 0;
assert.equal(
  guardMatchesPageNavigation(
    {
      currentFilters: { season: 22 },
      nextFilters: { season: 23 },
      currentSelectedMatchId: "match-a",
      nextSelectedMatchId: "match-a",
    },
    () => {
      filterOnlyNavigationConfirmCount += 1;
      return false;
    },
  ),
  false,
);
assert.equal(filterOnlyNavigationConfirmCount, 1);

let combinedNavigationConfirmCount = 0;
assert.equal(
  guardMatchesPageNavigation(
    {
      currentFilters: { season: 22 },
      nextFilters: { season: 23 },
      currentSelectedMatchId: "match-a",
      nextSelectedMatchId: "match-b",
    },
    () => {
      combinedNavigationConfirmCount += 1;
      return true;
    },
  ),
  true,
);
assert.equal(combinedNavigationConfirmCount, 1, "a combined filter and match transition confirms exactly once");

const restoredFromQaAccepted = restoreRejectedMatchesSearchParams(
  "season=22&matchId=match-old&qa=1",
  { season: 23 },
  "match-current",
);
assert.equal(restoredFromQaAccepted.get("qa"), "1");
assert.equal(restoredFromQaAccepted.get("season"), "23");
assert.equal(restoredFromQaAccepted.get("matchId"), "match-current");

const restoredFromPlainAccepted = restoreRejectedMatchesSearchParams(
  "season=22&matchId=match-old",
  { season: 23 },
  "match-current",
);
assert.equal(
  restoredFromPlainAccepted.has("qa"),
  false,
  "an attempted qa=1 URL cannot add QA mode when the last accepted URL had none",
);

assert.match(
  matchesPageSource,
  /function setFilters\(nextFilters: MatchFilters\) \{\s*if \(!guardMatchFilterChange\(currentFiltersRef\.current, nextFilters, favoriteController\.confirmDiscard\)\) return false;\s*commitFilters\(nextFilters\);[\s\S]*?function commitFilters\(nextFilters: MatchFilters\) \{/,
  "UI filter entry points guard before using the internal filter commit path",
);
const favoritePersistedSource = matchesPageSource.slice(
  matchesPageSource.indexOf("const handleFavoritePersisted"),
  matchesPageSource.indexOf("const favoriteController"),
);
assert.equal(
  matchesPageSource.includes("const pageLifetimeRef = useRef(createMatchesPageLifetime());"),
  true,
  "MatchesPage should retain one tested lifetime gate across renders and StrictMode effect replay",
);
assert.match(
  favoritePersistedSource,
  /async \(favorite: MatchDetailView\["favorite"\]\) => \{\s*if \(!pageLifetimeRef\.current\.isActive\(\)\) return;\s*const selectedFavoriteMatchId = selectedMatchIdRef\.current;/,
  "favorite reconciliation checks the shared page lifetime before reading selection or causing any state/cache side effect",
);
assert.match(
  favoritePersistedSource,
  /const selectedFavoriteMatchId = selectedMatchIdRef\.current;/,
  "favorite reconciliation reads the current selected match id instead of a render-time id",
);
assert.match(
  favoritePersistedSource,
  /if \(currentFiltersRef\.current\.favoriteOnly && favorite === null\) \{\s*await loadMatchesRef\.current\(currentFiltersRef\.current\);\s*return;/,
  "unfavoriting under the favorite-only filter reloads the current timeline and returns",
);
assert.match(
  favoritePersistedSource,
  /if \(selectedFavoriteMatchId === null\) return;\s*setMatches/,
  "a persistence result that finishes after selection clears must not update an unrelated list row",
);
const favoriteStateUpdaterBody = favoritePersistedSource.match(
  /setMatches\(\(current\) => \{([\s\S]*?)\}\);/,
)?.[1] ?? "";
assert.notEqual(favoriteStateUpdaterBody, "", "favorite reconciliation should update list state through a functional updater");
assert.equal(
  favoriteStateUpdaterBody.includes("updateMatchesPageCache"),
  false,
  "the React state updater must not perform cache side effects",
);
assert.match(
  favoriteStateUpdaterBody,
  /return updateMatchFavoriteState\(current, selectedFavoriteMatchId, favorite !== null\);/,
  "the favorite state updater should only return the immutable helper result",
);
assert.equal(
  favoritePersistedSource.includes("favorite.note"),
  false,
  "favorite reconciliation never copies note text into list state or cache",
);
assert.match(
  matchesPageSource,
  /<div\s+ref=\{timelineRef\}\s+className="matchListPane phase6WorkbenchList"\s+onScroll=\{rememberScroll\}>/,
  "the cache and anchor ref should stay on the actual list-pane scroll owner",
);
const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const hydrateInitialMatchesSource = matchesPageSource.slice(
  matchesPageSource.indexOf("async function hydrateInitialMatches"),
  matchesPageSource.indexOf("async function loadMatches"),
);
const loadMatchesSource = matchesPageSource.slice(
  matchesPageSource.indexOf("async function loadMatches"),
  matchesPageSource.indexOf("function setSelectedMatchIdSafely"),
);
const commitLoadedMatchesSource = matchesPageSource.slice(
  matchesPageSource.indexOf("function commitLoadedMatches"),
  matchesPageSource.indexOf("function rememberScroll"),
);
assert.equal(matchesPageSource.includes("window.scrollY"), false, "MatchesPage should not save window scroll for the match list");
assert.equal(matchesPageSource.includes("window.scrollTo"), false, "MatchesPage should not restore window scroll for the match list");
assert.equal(matchesPageSource.includes("readMatchScrollAnchor"), true, "MatchesPage should save a match-id scroll anchor");
assert.equal(matchesPageSource.includes("scrollTop"), true, "MatchesPage should keep scrollTop as a fallback");
assert.equal(
  matchesPageSource.includes('window.addEventListener("scroll", rememberScroll'),
  true,
  "MatchesPage should listen to document scrolling before navigating away from the list",
);
assert.match(
  matchesPageSource,
  /const isCurrentFilterLoaded = loadedFilterKeyRef\.current === matchFiltersKey\(filters\);\s*const nextSelectedMatchId = chooseMatchIdForLoadedMatches\(\s*selectedMatchId,\s*matches,\s*isCurrentFilterLoaded,\s*\);/s,
  "selection reconciliation waits until the current filter list is loaded",
);
assert.equal(matchesPageSource.includes("const searchParamsKey = searchParams.toString();"), true, "MatchesPage tracks same-instance URL changes");
assert.equal(
  matchesPageSource.includes("const lastAcceptedSearchParamsKeyRef = useRef(searchParamsKey);"),
  true,
  "MatchesPage tracks the last accepted canonical URL independently from attempted navigation",
);
assert.match(
  matchesPageSource,
  /if \(!navigationDecision\.shouldApplyNavigation\) \{\s*lastAcceptedSearchParamsKeyRef\.current = searchParamsKey;\s*return;\s*\}[\s\S]*?if \(!guardMatchesPageNavigation\([\s\S]*?\)\) \{\s*const restoredSearchParams = restoreRejectedMatchesSearchParams\(\s*lastAcceptedSearchParamsKeyRef\.current,\s*filters,\s*selectedMatchId,\s*\);[\s\S]*?lastWrittenSearchParamsKeyRef\.current = restoredSearchParamsKey;[\s\S]*?setSearchParams\(restoredSearchParams, \{ replace: true \}\);\s*return;\s*\}\s*lastAcceptedSearchParamsKeyRef\.current = searchParamsKey;\s*if \(matchFiltersKey\(navigation\.filters\) !== matchFiltersKey\(filters\)\) commitFilters/,
  "URL navigation records accepted echoes, restores rejected transitions from the prior accepted key, and records accepted external navigation",
);
assert.match(
  matchesPageSource,
  /useEffect\(\(\) => \{\s*if \(!hasLoadedOptions\(options\)\) return;[\s\S]*?\}, \[\s*favoriteController\.busy,\s*favoriteController\.confirmDiscard,\s*favoriteController\.dirty,\s*filters,\s*options,\s*\]\);/,
  "filter validation retries after busy or dirty guard state changes",
);
assert.equal(
  matchesPageSource.includes("const lastWrittenSearchParamsKeyRef = useRef<string | null>(null);"),
  true,
  "MatchesPage tracks the canonical key of its last local URL write",
);
assert.match(
  matchesPageSource,
  /const nextSearchParams = preserveQaParam\(searchParams, paramsFromFilters\(filters, selectedMatchId\)\);\s*const nextSearchParamsKey = nextSearchParams\.toString\(\);\s*if \(nextSearchParamsKey === searchParamsKey\) return;\s*lastWrittenSearchParamsKeyRef\.current = nextSearchParamsKey;\s*setSearchParams\(nextSearchParams, \{ replace: true \}\);/,
  "state-to-URL sync records the same canonical key before writing only a changed URL",
);
assert.equal(matchesPageSource.includes("matchRequestIdRef"), false, "metadata and timeline no longer share one cancellation domain");
assert.equal(matchesPageSource.includes("const pageDataRequestIdRef = useRef(0);"), true, "MatchesPage tracks metadata hydration separately");
assert.equal(matchesPageSource.includes("const timelineRequestIdRef = useRef(0);"), true, "MatchesPage tracks timeline requests separately");
assert.equal(matchesPageSource.includes("const pageDataReadyRef = useRef(canHydrateCache);"), true, "cache hydration waits for real page metadata");
assert.equal(matchesPageSource.includes("const currentFiltersRef = useRef(initialFilters);"), true, "async timeline commits read the current filter key");
const setFiltersSource = matchesPageSource.match(
  /function setFilters\(nextFilters: MatchFilters\) \{([\s\S]*?)\r?\n\s*\}\r?\n\r?\n\s*function retryMatches/,
)?.[1] ?? "";
assert.notEqual(setFiltersSource, "", "MatchesPage keeps one filter state writer");
assert.ok(
  setFiltersSource.indexOf("currentFiltersRef.current = nextFilters;") < setFiltersSource.indexOf("setFiltersState(nextFilters);"),
  "every filter state write synchronizes the current-filter ref before React state",
);
assert.equal(
  matchesPageSource.includes("skipInitialFilterLoadRef"),
  false,
  "MatchesPage no longer uses a one-shot filter-effect skip that breaks StrictMode replay",
);
assert.equal(
  matchesPageSource.includes(
    "const initialHydrationFilterKeyRef = useRef(canHydrateCache ? null : matchFiltersKey(initialFilters));",
  ),
  true,
  "MatchesPage gives initial hydration durable ownership of its filter across StrictMode replay",
);
assert.match(
  matchesPageSource,
  /const currentFilterKey = matchFiltersKey\(filters\);\s*if \(currentFilterKey === loadedFilterKeyRef\.current\) \{\s*timelineRequestIdRef\.current \+= 1;\s*initialHydrationFilterKeyRef\.current = null;\s*setBusy\(false\);\s*return;\s*\}\s*if \(!shouldLoadMatchesForFilter\(\s*currentFilterKey,\s*loadedFilterKeyRef\.current,\s*initialHydrationFilterKeyRef\.current,\s*\)\) return;\s*initialHydrationFilterKeyRef\.current = null;\s*void loadMatches\(filters\);/,
  "filter effects preserve the initial hydration owner, but clear it before a genuinely different load",
);
assert.match(
  matchesPageSource,
  /useEffect\(\(\) => \{\s*pageLifetimeRef\.current\.activate\(\);\s*return \(\) => \{\s*pageLifetimeRef\.current\.deactivate\(\);\s*pageDataRequestIdRef\.current \+= 1;\s*timelineRequestIdRef\.current \+= 1;\s*\};\s*\}, \[\]\);/,
  "effect setup activates the shared lifetime while cleanup deactivates it and invalidates existing requests",
);
assert.equal(hydrateInitialMatchesSource.includes("const pageDataRequestId = ++pageDataRequestIdRef.current;"), true, "hydration owns a metadata request id");
assert.equal(hydrateInitialMatchesSource.includes("const timelineRequestId = ++timelineRequestIdRef.current;"), true, "hydration separately owns its A timeline id");
assert.equal(
  hydrateInitialMatchesSource.includes("if (!isLatestMatchRequest(pageDataRequestId, pageDataRequestIdRef.current)) return;"),
  true,
  "only an obsolete metadata request can discard hydrated page data",
);
assert.ok(
  hydrateInitialMatchesSource.indexOf("pageDataReadyRef.current = true;") <
    hydrateInitialMatchesSource.indexOf("canCommitMatchTimelineRequest("),
  "metadata becomes ready before the old hydration timeline is considered for commit",
);
assert.match(
  hydrateInitialMatchesSource,
  /pageDataRef\.current = pageData;\s*pageDataReadyRef\.current = true;\s*setOverview\(pageData\.overview\);\s*setSummary\(pageData\.summary\);\s*setOptions\(pageData\.options\);/,
  "metadata hydration commits overview, summary, and options independently of the current timeline filter",
);
assert.match(
  hydrateInitialMatchesSource,
  /if \(canCommitMatchTimelineRequest\(\s*timelineRequestId,\s*timelineRequestIdRef\.current,\s*hydrationFilterKey,\s*matchFiltersKey\(currentFiltersRef\.current\),\s*\)\) \{\s*commitLoadedMatches\(nextFilters, cache\.matches, pageData\);\s*\}/,
  "hydration timeline A cannot overwrite a newer current filter B",
);
assert.match(
  hydrateInitialMatchesSource,
  /refreshMatchEvaluations\(\)\s*\.then\(\(\) => \{\s*if \(!isLatestMatchRequest\(pageDataRequestId, pageDataRequestIdRef\.current\)\) return;\s*void loadMatches\(currentFiltersRef\.current\);\s*\}\)/,
  "evaluation refresh follows the current filter and remains scoped to the live metadata request",
);
assert.match(
  hydrateInitialMatchesSource,
  /finally \{\s*if \(isLatestMatchRequest\(pageDataRequestId, pageDataRequestIdRef\.current\)\) \{\s*if \(initialHydrationFilterKeyRef\.current === matchFiltersKey\(nextFilters\)\) \{\s*initialHydrationFilterKeyRef\.current = null;\s*\}\s*setBusy\(false\);\s*\}\s*\}/,
  "only the live metadata hydration releases the StrictMode filter owner",
);
assert.equal(loadMatchesSource.includes("pageDataRequestIdRef"), false, "loading timeline B never invalidates metadata hydration A");
assert.equal(loadMatchesSource.includes("const timelineRequestId = ++timelineRequestIdRef.current;"), true, "filter loading only advances the timeline domain");
assert.equal(loadMatchesSource.includes("canCommitMatchTimelineRequest("), true, "filter loading checks both request id and current filter before commit");
assert.match(
  commitLoadedMatchesSource,
  /const cachePageData = pageData \?\? \(pageDataReadyRef\.current \? pageDataRef\.current : \{\}\);[\s\S]*?\.\.\.cachePageData,[\s\S]*?hasLoaded: pageDataReadyRef\.current,/,
  "a B timeline caches as partial until metadata is ready and includes metadata when it is available",
);
assert.equal(
  (matchesPageSource.match(/hasLoaded: pageDataReadyRef\.current/g) ?? []).length >= 2,
  true,
  "metadata state promotes both an existing B snapshot and later B commits to a hydratable cache",
);
assert.match(
  matchesPageSource,
  /const cleaned = validateFilters\(filters, options\);[\s\S]*?\}, \[\s*favoriteController\.busy,\s*favoriteController\.confirmDiscard,\s*favoriteController\.dirty,\s*filters,\s*options,\s*\]\);/,
  "filter validation reruns after URL navigation, options load, or favorite guard state changes",
);
assert.equal(
  matchesPageSource.includes("if (loadedFilterKeyRef.current !== matchFiltersKey(filters)) return;"),
  true,
  "cache snapshots are only written for the list loaded under current filters",
);
const matchesCacheSyncStart = matchesPageSource.indexOf(
  "if (loadedFilterKeyRef.current !== matchFiltersKey(filters)) return;",
);
const matchesCacheSyncEnd = matchesPageSource.indexOf("\n  useEffect(", matchesCacheSyncStart);
const matchesCacheSyncEffect = matchesPageSource.slice(matchesCacheSyncStart, matchesCacheSyncEnd);
assert.notEqual(matchesCacheSyncStart, -1, "MatchesPage should keep one guarded post-commit cache sync effect");
assert.equal(
  matchesCacheSyncEffect.includes("updateMatchesPageCache({"),
  true,
  "the guarded post-commit effect should own cache synchronization",
);
assert.match(matchesCacheSyncEffect, /\bmatches,\s*/, "the post-commit cache sync effect should persist committed match rows");
assert.doesNotMatch(
  matchesPageSource,
  /updateMatchesPageCache\(\{\s*filters\s*\}\)/,
  "pending filters must not be mixed into the last completed cache snapshot",
);
assert.equal(matchesPageSource.includes("commitLoadedMatches(nextFilters, nextMatches"), true, "successful list requests use one atomic commit path");
assert.match(
  onboardingSource,
  /import\s*\{[^}]*clearMatchesPageCache[^}]*\}\s*from\s*["']\.\.\/shared\/matchesPageCache["']/s,
  "a completed season sync imports the match-page cache invalidator",
);
assert.match(
  onboardingSource,
  /const summary = await syncSeason\(season, mode\);\s*clearMatchesPageCache\(\);/s,
  "a completed season sync invalidates the cached match list before reloading dashboard data",
);
