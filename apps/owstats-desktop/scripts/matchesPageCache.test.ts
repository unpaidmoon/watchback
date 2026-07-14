import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { filtersFromParams, validateFilters } from "../src/features/matches/matchFilters.ts";
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
  hasExplicitMatchFilterParams,
  isLatestMatchRequest,
  matchFiltersKey,
  preloadMatchesPageCache,
  selectMatchScrollAnchor,
  shouldLoadMatchesForFilter,
  updateMatchesPageCache,
} from "../src/shared/matchesPageCache.ts";
import type { FilterOptions, MatchFilters, MatchSummary, Overview } from "../src/shared/types.ts";

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

assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, false), false, "cache does not hydrate before first successful load");
assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, true), false, "cache without loaded key does not hydrate explicit filters");

updateMatchesPageCache({ loadedFilterKey: matchFiltersKey(cachedFilters), hasLoaded: true });
assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, false), true, "cache hydrates normal route re-entry");
assert.equal(canHydrateMatchesPageCache(getMatchesPageCache(), cachedFilters, true), true, "cache hydrates explicit filters when loaded key matches");

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
  },
] satisfies MatchSummary[];

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
assert.match(
  matchesPageSource,
  /useEffect\(\(\) => \{\s*const navigationDecision = consumeMatchesSearchParamsNavigation\(\s*searchParamsKey,\s*lastWrittenSearchParamsKeyRef\.current,\s*\);\s*lastWrittenSearchParamsKeyRef\.current = navigationDecision\.nextLastWrittenSearchParamsKey;\s*if \(!navigationDecision\.shouldApplyNavigation\) return;\s*const navigation = chooseMatchesPageNavigationState\([\s\S]*?if \(matchFiltersKey\(navigation\.filters\) !== matchFiltersKey\(filters\)\) setFilters\(navigation\.filters\);[\s\S]*?if \(navigation\.selectedMatchId !== selectedMatchId\) setSelectedMatchIdSafely\(navigation\.selectedMatchId\);[\s\S]*?\}, \[searchParamsKey\]\);/,
  "URL navigation consumes local echoes while external navigation still updates filters and selection",
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
  /function setFilters\(nextFilters: MatchFilters\) \{([\s\S]*?)\n\s*\}\n\n\s*function retryMatches/,
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
  /useEffect\(\(\) => \(\) => \{\s*pageDataRequestIdRef\.current \+= 1;\s*timelineRequestIdRef\.current \+= 1;\s*\}, \[\]\);/,
  "leaving MatchesPage invalidates both metadata and timeline requests before a later re-entry",
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
  /const cleaned = validateFilters\(filters, options\);[\s\S]*?\}, \[filters, options\]\);/,
  "filter validation reruns after URL navigation changes filters as well as after options load",
);
assert.equal(
  matchesPageSource.includes("if (loadedFilterKeyRef.current !== matchFiltersKey(filters)) return;"),
  true,
  "cache snapshots are only written for the list loaded under current filters",
);
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
  /const summary = await syncSeason\(season\);\s*clearMatchesPageCache\(\);/s,
  "a completed season sync invalidates the cached match list before reloading dashboard data",
);
