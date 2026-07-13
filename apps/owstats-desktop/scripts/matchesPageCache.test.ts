import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { filtersFromParams, validateFilters } from "../src/features/matches/matchFilters.ts";
import {
  canHydrateMatchesPageCache,
  chooseInitialMatchId,
  chooseInitialMatchFilters,
  chooseMatchIdForLoadedMatches,
  chooseMatchesPageNavigationState,
  clearMatchesPageCache,
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

const matchesPageSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");
const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
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
  /useEffect\(\(\) => \{\s*const navigation = chooseMatchesPageNavigationState\([\s\S]*?if \(matchFiltersKey\(navigation\.filters\) !== matchFiltersKey\(filters\)\) setFilters\(navigation\.filters\);[\s\S]*?if \(navigation\.selectedMatchId !== selectedMatchId\) setSelectedMatchIdSafely\(navigation\.selectedMatchId\);[\s\S]*?\}, \[searchParamsKey\]\);/,
  "same-instance URL navigation updates filters and selection without redundant state writes",
);
assert.equal(matchesPageSource.includes("const matchRequestIdRef = useRef(0);"), true, "MatchesPage tracks the latest timeline request");
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
  /const currentFilterKey = matchFiltersKey\(filters\);\s*if \(currentFilterKey === loadedFilterKeyRef\.current\) \{\s*matchRequestIdRef\.current \+= 1;\s*initialHydrationFilterKeyRef\.current = null;\s*setBusy\(false\);\s*return;\s*\}\s*if \(!shouldLoadMatchesForFilter\(\s*currentFilterKey,\s*loadedFilterKeyRef\.current,\s*initialHydrationFilterKeyRef\.current,\s*\)\) return;\s*initialHydrationFilterKeyRef\.current = null;\s*void loadMatches\(filters\);/,
  "filter effects preserve the initial hydration owner, but clear it before a genuinely different load",
);
assert.match(
  matchesPageSource,
  /useEffect\(\(\) => \(\) => \{\s*matchRequestIdRef\.current \+= 1;\s*\}, \[\]\);/,
  "leaving MatchesPage invalidates pending timeline requests before a later re-entry",
);
assert.match(
  matchesPageSource,
  /const cache = await preloadMatchesPageCache\([\s\S]*?\{ cacheResult: false \},\s*\);\s*if \(!isLatestMatchRequest\(requestId, matchRequestIdRef\.current\)\) return;/,
  "initial hydration checks request currency before applying its detached preload",
);
assert.match(
  matchesPageSource,
  /const nextMatches = await listMatchTimeline\(nextFilters\);\s*if \(!isLatestMatchRequest\(requestId, matchRequestIdRef\.current\)\) return;/,
  "filter loading checks request currency before applying list results",
);
assert.match(
  matchesPageSource,
  /refreshMatchEvaluations\(\)\s*\.then\(\(\) => \{\s*if \(!isLatestMatchRequest\(requestId, matchRequestIdRef\.current\)\) return;\s*void loadMatches\(nextFilters\);\s*\}\)/,
  "evaluation refresh cannot restart an obsolete initial filter request",
);
assert.match(
  matchesPageSource,
  /finally \{\s*if \(isLatestMatchRequest\(requestId, matchRequestIdRef\.current\)\) \{\s*if \(initialHydrationFilterKeyRef\.current === matchFiltersKey\(nextFilters\)\) \{\s*initialHydrationFilterKeyRef\.current = null;\s*\}\s*setBusy\(false\);\s*\}\s*\}/,
  "the current hydration request releases filter ownership on success or failure without stale StrictMode cleanup doing so",
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
