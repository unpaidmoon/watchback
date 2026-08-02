import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  filtersFromParams,
  validateFilters,
} from "../src/features/matches/matchFilters.ts";
import * as matchesPageCacheModule from "../src/shared/matchesPageCache.ts";
import {
  canCommitMatchTimelineRequest,
  canAcknowledgeMatchTimelineRevision,
  canHydrateMatchesPageCache,
  classifyMatchTimelineRevisionPage,
  chooseInitialMatchId,
  chooseInitialMatchFilters,
  chooseMatchIdForLoadedMatches,
  chooseMatchIdForTimelineCommit,
  chooseMatchesPageNavigationState,
  clearMatchesPageCache,
  consumeMatchesSearchParamsNavigation,
  computeMatchAnchorScrollTop,
  getMatchesPageCache,
  guardMatchFilterChange,
  guardMatchesPageNavigation,
  hasExplicitMatchFilterParams,
  isLatestMatchRequest,
  matchFiltersKey,
  preloadMatchesPageCache,
  selectMatchScrollAnchor,
  shouldLoadMatchesForFilter,
  shouldAcceptMatchTimelineEvent,
  shouldProbeMatchTimelineRevision,
  summarizeMatchTimelineInvalidations,
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

updateMatchesPageCache({
  accountScope: "account-a",
  revisionToken: "23:4",
  filters: { season: 23 },
  loadedFilterKey: matchFiltersKey({ season: 23 }),
  hasLoaded: true,
});
assert.equal(
  canHydrateMatchesPageCache(getMatchesPageCache(), { season: 23 }, false, { accountScope: "account-a", revisionToken: "23:4" }),
  true,
  "cache hydrates only when account and revision contracts match",
);
assert.equal(
  canHydrateMatchesPageCache(getMatchesPageCache(), { season: 23 }, false, { accountScope: "account-b", revisionToken: "23:4" }),
  false,
  "cache from another account cannot hydrate",
);
assert.equal(
  canHydrateMatchesPageCache(getMatchesPageCache(), { season: 23 }, false, { accountScope: "account-a", revisionToken: "23:5" }),
  false,
  "an older revision snapshot cannot hydrate",
);
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
  chooseMatchIdForTimelineCommit(
    "selected-old-match",
    [{ matchId: "fresh-head-match" }],
    { beginTs: 100, matchId: "fresh-head-match" },
  ),
  "selected-old-match",
  "a fresh first-page reset keeps the selected detail reachable through the remaining pagination cursor",
);
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

assert.equal(
  canAcknowledgeMatchTimelineRevision({
    requestIsCurrent: true,
    pageIsActive: true,
    requestedRevisionGeneration: 4,
    pendingRevisionGeneration: 4,
  }),
  true,
  "a successful current request may acknowledge the exact pending revision generation it loaded",
);
for (const blocked of [
  { requestIsCurrent: false, pageIsActive: true, requestedRevisionGeneration: 4, pendingRevisionGeneration: 4 },
  { requestIsCurrent: true, pageIsActive: false, requestedRevisionGeneration: 4, pendingRevisionGeneration: 4 },
  { requestIsCurrent: true, pageIsActive: true, requestedRevisionGeneration: 4, pendingRevisionGeneration: 5 },
]) {
  assert.equal(
    canAcknowledgeMatchTimelineRevision(blocked),
    false,
    "obsolete, unmounted, and superseded requests must leave pending revisions queued",
  );
}

assert.deepEqual(
  classifyMatchTimelineRevisionPage(
    [{ matchId: "loaded-newest" }, { matchId: "loaded-tail" }],
    [{ matchId: "database-new" }, { matchId: "loaded-newest" }],
  ),
  { requiresListMerge: true, newMatches: 1 },
  "a revision probe detects unseen database rows as a list invalidation",
);
assert.deepEqual(
  classifyMatchTimelineRevisionPage(
    [{ matchId: "loaded-newest" }, { matchId: "loaded-tail" }],
    [{ matchId: "loaded-newest" }],
  ),
  { requiresListMerge: false, newMatches: 0 },
  "a revision probe with only known rows is a detail invalidation and must preserve the loaded tail",
);

assert.equal(
  shouldProbeMatchTimelineRevision(
    { accountScope: "account-a", revisionToken: "23:4" },
    { accountScope: "account-a", revisionToken: "23:5" },
  ),
  true,
  "a changed database revision token reloads the visible first page",
);
assert.equal(
  shouldProbeMatchTimelineRevision(
    { accountScope: "account-a", revisionToken: "23:4" },
    { accountScope: "account-b", revisionToken: "23:4" },
  ),
  true,
  "a different account scope invalidates the cached list even when the token text matches",
);
assert.equal(
  shouldProbeMatchTimelineRevision(
    { accountScope: "account-a", revisionToken: "23:4" },
    { accountScope: "account-a", revisionToken: "23:4" },
  ),
  false,
  "an unchanged account-scoped database revision avoids a redundant first-page load",
);
assert.deepEqual(
  summarizeMatchTimelineInvalidations([
    { requiresListMerge: true, newMatches: 2 },
    { requiresListMerge: false, newMatches: 0 },
    { requiresListMerge: true, newMatches: 1 },
  ]),
  { requiresListMerge: true, newMatches: 3 },
  "all-season mode preserves list semantics and exact counts across multiple season queues",
);
assert.equal(shouldAcceptMatchTimelineEvent(null, "account-a"), true, "the first account-scoped event may establish page data");
assert.equal(shouldAcceptMatchTimelineEvent("account-a", "account-a"), true, "the loaded account accepts its own event");
assert.equal(
  shouldAcceptMatchTimelineEvent("account-a", "account-b"),
  false,
  "an event from a different account cannot clear or mutate the loaded account pending queue",
);

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

clearMatchesPageCache();
const pagedPreload = await preloadMatchesPageCache({
  getOverview: async () => overview,
  listMatchFilters: async () => options,
  listMatchTimeline: async () => ({
    matches,
    nextCursor: { beginTs: matches[0].beginTs, matchId: matches[0].matchId },
    total: 3,
  }),
});
assert.equal(pagedPreload.totalMatches, 3, "paged preload preserves the filtered total");
assert.equal(pagedPreload.nextCursor?.matchId, matches[0].matchId, "paged preload preserves its continuation cursor");

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
