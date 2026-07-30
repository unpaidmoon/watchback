import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { FilterBar } from "../features/matches/FilterBar";
import { MatchDetailPane } from "../features/matches/MatchDetailPane";
import { useMatchFavoriteEditor } from "../features/matches/useMatchFavoriteEditor";
import { MatchTimeline } from "../features/matches/MatchTimeline";
import { getLiveMatchTimelineHandshake, getLiveMatchTimelineRevisionToken, subscribeLiveMatchTimeline } from "../features/matches/liveMatchTimelineEventAdapter";
import { chooseLiveTimelineRefresh, mergeLiveTimelinePage, type SeasonDataRevisionEvent } from "../features/matches/liveMatchTimeline";
import {
  activeMatchFilterChips,
  clearVisibleMatchFilters,
  filtersFromParams,
  hasLoadedOptions,
  paramsFromFilters,
  validateFilters,
} from "../features/matches/matchFilters";
import { getMatchDetails, getOverview, listMatchFilters, listMatchTimelinePage, refreshMatchEvaluations } from "../shared/api";
import {
  canCommitMatchTimelineRequest,
  canHydrateMatchesPageCache,
  chooseInitialMatchId,
  chooseInitialMatchFilters,
  chooseMatchIdForLoadedMatches,
  chooseMatchesPageNavigationState,
  computeMatchAnchorScrollTop,
  consumeMatchesSearchParamsNavigation,
  createMatchesPageLifetime,
  getMatchesPageCache,
  guardMatchFilterChange,
  guardMatchesPageNavigation,
  hasExplicitMatchFilterParams,
  isLatestMatchRequest,
  matchFiltersKey,
  preloadMatchesPageCache,
  selectMatchScrollAnchor,
  shouldLoadMatchesForFilter,
  updateMatchFavoriteState,
  updateMatchesPageCache,
} from "../shared/matchesPageCache";
import type { MatchesPageCache } from "../shared/matchesPageCache";
import { useUnsavedChangesGuard } from "../shared/UnsavedChangesGuard";
import { productErrorMessage } from "../shared/phase6Shell";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";
import type { FilterOptions, ImportSummary, MatchCursor, MatchDetailView, MatchFilters, MatchPage, MatchSummary, Overview, } from "../shared/types";

const MATCH_PAGE_SIZE = 50;

const emptyOptions: FilterOptions = {
  seasons: [],
  heroes: [],
  maps: [],
  results: [],
  modes: []
};

export function MatchesPage({ onQueryPlayer }: { onQueryPlayer?: (battleTag: string) => void } = {}) {
  const [searchParams, setSearchParams] = useSearchParams();
  const searchParamsKey = searchParams.toString();
  const cachedPage = useMemo(() => getMatchesPageCache(), []);
  const hasExplicitFilters = useMemo(() => hasExplicitMatchFilterParams(searchParams), []);
  const initialFilters = useMemo(
    () => chooseInitialMatchFilters(filtersFromParams(searchParams), cachedPage?.filters, hasExplicitFilters),
    [],
  );
  const canHydrateCache = useMemo(() => canHydrateMatchesPageCache(cachedPage, initialFilters, hasExplicitFilters), []);
  const initialCacheRef = useRef(cachedPage);
  const timelineRef = useRef<HTMLDivElement | null>(null);
  const pageLifetimeRef = useRef(createMatchesPageLifetime());
  const lastWrittenSearchParamsKeyRef = useRef<string | null>(null);
  const lastAcceptedSearchParamsKeyRef = useRef(searchParamsKey);
  const initialHydrationFilterKeyRef = useRef(canHydrateCache ? null : matchFiltersKey(initialFilters));
  const loadedFilterKeyRef = useRef(canHydrateCache ? cachedPage?.loadedFilterKey ?? "" : "");
  const currentFiltersRef = useRef(initialFilters);
  const pageDataReadyRef = useRef(canHydrateCache);
  const pageDataRef = useRef<Pick<MatchesPageCache, "overview" | "summary" | "options">>({
    overview: canHydrateCache ? cachedPage?.overview ?? null : null,
    summary: canHydrateCache ? cachedPage?.summary ?? null : null,
    options: canHydrateCache ? cachedPage?.options ?? emptyOptions : emptyOptions,
  });
  const [overview, setOverview] = useState<Overview | null>(canHydrateCache ? cachedPage?.overview ?? null : null);
  const [summary, setSummary] = useState<ImportSummary | null>(canHydrateCache ? cachedPage?.summary ?? null : null);
  const [options, setOptions] = useState<FilterOptions>(canHydrateCache ? cachedPage?.options ?? emptyOptions : emptyOptions);
  const [filters, setFiltersState] = useState<MatchFilters>(initialFilters);
  const [matches, setMatches] = useState<MatchSummary[]>(canHydrateCache ? cachedPage?.matches ?? [] : []);
  const matchesRef = useRef(matches);
  const [nextCursor, setNextCursor] = useState<MatchCursor | null>(canHydrateCache ? cachedPage?.nextCursor ?? null : null);
  const nextCursorRef = useRef(nextCursor);
  const [totalMatches, setTotalMatches] = useState(canHydrateCache ? cachedPage?.totalMatches ?? matches.length : 0);
  const loadingMoreRef = useRef(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadMoreError, setLoadMoreError] = useState<string | null>(null);
  const [selectedMatchId, setSelectedMatchId] = useState<string | null>(() =>
    chooseInitialMatchId(searchParams.get("matchId"), cachedPage?.selectedMatchId, canHydrateCache),
  );
  const selectedMatchIdRef = useRef(selectedMatchId);
  const pageDataRequestIdRef = useRef(0);
  const timelineRequestIdRef = useRef(0);
  const [selectedDetail, setSelectedDetail] = useState<MatchDetailView | null>(null);
  const [detailStateMatchId, setDetailStateMatchId] = useState<string | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [detailRetryKey, setDetailRetryKey] = useState(0);
  const [busy, setBusy] = useState(!canHydrateCache);
  const [error, setError] = useState<string | null>(null);
  const [mobileDetailOpen, setMobileDetailOpen] = useState(false);
  const [pendingNewMatches, setPendingNewMatches] = useState(0);
  const accountScopeRef = useRef(cachedPage?.accountScope ?? null);
  const revisionTokenRef = useRef(cachedPage?.revisionToken ?? null);
  const pendingRevisionEventRef = useRef<SeasonDataRevisionEvent | null>(null);
  const atTimelineTopRef = useRef(true);
  const hasVisibleFilters = activeMatchFilterChips(filters, options).length > 0;
  const { register } = useUnsavedChangesGuard();
  const loadMatchesRef = useRef(loadMatches);
  loadMatchesRef.current = loadMatches;
  const handleFavoritePersisted = useCallback(
    async (favorite: MatchDetailView["favorite"]) => {
      if (!pageLifetimeRef.current.isActive()) return;
      const selectedFavoriteMatchId = selectedMatchIdRef.current;
      setSelectedDetail((current) =>
        current?.matchId === selectedFavoriteMatchId
          ? { ...current, favorite }
          : current,
      );
      if (currentFiltersRef.current.favoriteOnly && favorite === null) {
        await loadMatchesRef.current(currentFiltersRef.current);
        return;
      }
      if (selectedFavoriteMatchId === null) return;
      setMatches((current) => {
        const next = updateMatchFavoriteState(current, selectedFavoriteMatchId, favorite !== null);
        matchesRef.current = next;
        return next;
      });
    },
    [],
  );
  const favoriteController = useMatchFavoriteEditor(
    selectedDetail?.matchId ?? selectedMatchId,
    selectedDetail?.favorite,
    handleFavoritePersisted,
  );

  useEffect(() => {
    register(favoriteController.dirty || favoriteController.busy ? {
      isDirty: favoriteController.dirty,
      isBusy: favoriteController.busy,
      confirmDiscard: favoriteController.confirmDiscard,
    } : null);
    return () => register(null);
  }, [
    favoriteController.busy,
    favoriteController.confirmDiscard,
    favoriteController.dirty,
    register,
  ]);

  useEffect(() => {
    pageLifetimeRef.current.activate();
    return () => {
      pageLifetimeRef.current.deactivate();
      pageDataRequestIdRef.current += 1;
      timelineRequestIdRef.current += 1;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    const appliesToCurrentFilters = (event: SeasonDataRevisionEvent) =>
      chooseLiveTimelineRefresh(event, currentFiltersRef.current.season, atTimelineTopRef.current) !== "ignore";
    const accept = (event: SeasonDataRevisionEvent) => {
      if (disposed || !appliesToCurrentFilters(event)) return;
      const decision = chooseLiveTimelineRefresh(event, currentFiltersRef.current.season, atTimelineTopRef.current);
      if (decision === "queue") {
        pendingRevisionEventRef.current = event;
        setPendingNewMatches((current) => current + event.newMatches);
        return;
      }
      if (decision === "statusOnly") {
        accountScopeRef.current = event.accountScope;
        revisionTokenRef.current = getLiveMatchTimelineRevisionToken(event.accountScope, currentFiltersRef.current.season);
        updateMatchesPageCache({ accountScope: accountScopeRef.current, revisionToken: revisionTokenRef.current });
        return;
      }
      pendingRevisionEventRef.current = event;
      void loadMatchesRef.current(currentFiltersRef.current, { silent: true });
    };
    const unsubscribe = subscribeLiveMatchTimeline(accept);
    void getLiveMatchTimelineHandshake().then(({ accountScope, events }) => {
      if (disposed) return;
      accountScopeRef.current = accountScope;
      const relevant = events.filter(appliesToCurrentFilters).sort((left, right) => right.revision - left.revision)[0];
      if (!relevant) return;
      const token = getLiveMatchTimelineRevisionToken(accountScope ?? relevant.accountScope, currentFiltersRef.current.season);
      if (token !== revisionTokenRef.current) accept(relevant);
    }).catch(() => undefined);
    return () => { disposed = true; unsubscribe(); };
  }, []);

  useEffect(() => {
    if (canHydrateCache) return;
    void hydrateInitialMatches(filters);
  }, []);

  // Render a valid local snapshot immediately, then perform a database-backed
  // revision handshake so updates missed before the global listener existed
  // are still recovered without a route change.
  useEffect(() => {
    if (!canHydrateCache) return;
    void loadMatches(currentFiltersRef.current, { silent: true });
  }, []);

  useEffect(() => {
    if (!canHydrateCache || !initialCacheRef.current?.hasLoaded) return;
    restoreTimelineScroll(initialCacheRef.current.scrollTop, initialCacheRef.current.scrollAnchor);
  }, [matches.length]);

  useEffect(() => {
    const navigationDecision = consumeMatchesSearchParamsNavigation(
      searchParamsKey,
      lastWrittenSearchParamsKeyRef.current,
    );
    lastWrittenSearchParamsKeyRef.current = navigationDecision.nextLastWrittenSearchParamsKey;
    if (!navigationDecision.shouldApplyNavigation) {
      lastAcceptedSearchParamsKeyRef.current = searchParamsKey;
      return;
    }
    const navigation = chooseMatchesPageNavigationState(
      filtersFromParams(searchParams),
      searchParams.get("matchId"),
      getMatchesPageCache(),
      hasExplicitMatchFilterParams(searchParams),
    );
    if (!guardMatchesPageNavigation(
      {
        currentFilters: filters,
        nextFilters: navigation.filters,
        currentSelectedMatchId: selectedMatchId,
        nextSelectedMatchId: navigation.selectedMatchId,
      },
      favoriteController.confirmDiscard,
    )) {
      const restoredSearchParams = paramsFromFilters(filters, selectedMatchId);
      const restoredSearchParamsKey = restoredSearchParams.toString();
      lastWrittenSearchParamsKeyRef.current = restoredSearchParamsKey;
      setSearchParams(restoredSearchParams, { replace: true });
      return;
    }
    lastAcceptedSearchParamsKeyRef.current = searchParamsKey;
    if (matchFiltersKey(navigation.filters) !== matchFiltersKey(filters)) commitFilters(navigation.filters);
    if (navigation.selectedMatchId !== selectedMatchId) setSelectedMatchIdSafely(navigation.selectedMatchId);
  }, [searchParamsKey]);

  useEffect(() => {
    rememberScrollWithAnchor();
    const nextSearchParams = paramsFromFilters(filters, selectedMatchId);
    const nextSearchParamsKey = nextSearchParams.toString();
    if (nextSearchParamsKey === searchParamsKey) return;
    lastWrittenSearchParamsKeyRef.current = nextSearchParamsKey;
    setSearchParams(nextSearchParams, { replace: true });
  }, [filters, selectedMatchId]);

  useEffect(() => {
    const currentFilterKey = matchFiltersKey(filters);
    if (currentFilterKey === loadedFilterKeyRef.current) {
      timelineRequestIdRef.current += 1;
      initialHydrationFilterKeyRef.current = null;
      setBusy(false);
      return;
    }
    if (!shouldLoadMatchesForFilter(
      currentFilterKey,
      loadedFilterKeyRef.current,
      initialHydrationFilterKeyRef.current,
    )) return;
    initialHydrationFilterKeyRef.current = null;
    void loadMatches(filters);
  }, [filters]);

  useEffect(() => {
    const isCurrentFilterLoaded = loadedFilterKeyRef.current === matchFiltersKey(filters);
    if (selectedMatchId && nextCursorRef.current && !matches.some((match) => match.matchId === selectedMatchId)) return;
    const nextSelectedMatchId = chooseMatchIdForLoadedMatches(
      selectedMatchId,
      matches,
      isCurrentFilterLoaded,
    );
    if (nextSelectedMatchId !== selectedMatchId) setSelectedMatchIdSafely(nextSelectedMatchId);
  }, [filters, matches, selectedMatchId]);

  useEffect(() => {
    if (!selectedMatchId) {
      setDetailStateMatchId(null);
      setSelectedDetail(null);
      setDetailError(null);
      setDetailLoading(false);
      setMobileDetailOpen(false);
      return;
    }
    let cancelled = false;
    setDetailStateMatchId(selectedMatchId);
    setDetailLoading(true);
    setSelectedDetail(null);
    setDetailError(null);
    void getMatchDetails(selectedMatchId)
      .then((detail) => {
        if (!cancelled) setSelectedDetail(detail);
      })
      .catch((err) => {
        if (!cancelled) {
          setSelectedDetail(null);
          setDetailError(productErrorMessage(err));
        }
      })
      .finally(() => {
        if (!cancelled) setDetailLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedMatchId, detailRetryKey]);

  useEffect(() => {
    if (loadedFilterKeyRef.current !== matchFiltersKey(filters)) return;
    updateMatchesPageCache({
      overview,
      summary,
      options,
      filters,
      matches,
      nextCursor,
      totalMatches,
      selectedMatchId,
      loadedFilterKey: loadedFilterKeyRef.current,
      hasLoaded: pageDataReadyRef.current,
    });
  }, [overview, summary, options, filters, matches, nextCursor, totalMatches, selectedMatchId]);

  useEffect(() => {
    const onScroll = () => {
      rememberScroll();
      maybeLoadMore();
    };
    const onPageHide = () => rememberScrollWithAnchor();
    const onVisibilityChange = () => {
      if (document.visibilityState === "hidden") rememberScrollWithAnchor();
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("pagehide", onPageHide);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      rememberScrollWithAnchor();
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("pagehide", onPageHide);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, []);

  useEffect(() => {
    if (!hasLoadedOptions(options)) return;
    const cleaned = validateFilters(filters, options);
    if (JSON.stringify(cleaned) !== JSON.stringify(filters)) {
      setFilters(cleaned);
    }
  }, [
    favoriteController.busy,
    favoriteController.confirmDiscard,
    favoriteController.dirty,
    filters,
    options,
  ]);

  async function hydrateInitialMatches(nextFilters: MatchFilters) {
    const pageDataRequestId = ++pageDataRequestIdRef.current;
    const timelineRequestId = ++timelineRequestIdRef.current;
    const hydrationFilterKey = matchFiltersKey(nextFilters);
    setBusy(true);
    setError(null);
    try {
      const cache = await preloadMatchesPageCache(
        { getOverview, listMatchFilters, listMatchTimeline: (filters) => listMatchTimelinePage(filters, null, MATCH_PAGE_SIZE) },
        nextFilters,
        { cacheResult: false },
      );
      if (!isLatestMatchRequest(pageDataRequestId, pageDataRequestIdRef.current)) return;
      const pageData = {
        overview: cache.overview,
        summary: cache.summary,
        options: cache.options,
      };
      pageDataRef.current = pageData;
      pageDataReadyRef.current = true;
      setOverview(pageData.overview);
      setSummary(pageData.summary);
      setOptions(pageData.options);
      if (canCommitMatchTimelineRequest(
        timelineRequestId,
        timelineRequestIdRef.current,
        hydrationFilterKey,
        matchFiltersKey(currentFiltersRef.current),
      )) {
        commitLoadedMatches(nextFilters, {
          matches: cache.matches,
          nextCursor: cache.nextCursor,
          total: cache.totalMatches,
          revisionToken: cache.revisionToken ?? "",
          accountScope: cache.accountScope ?? "",
        }, pageData);
      }
      void refreshMatchEvaluations()
        .then(() => {
          if (!isLatestMatchRequest(pageDataRequestId, pageDataRequestIdRef.current)) return;
          void loadMatches(currentFiltersRef.current);
        })
        .catch(() => undefined);
    } catch (err) {
      if (isLatestMatchRequest(pageDataRequestId, pageDataRequestIdRef.current)) setError(productErrorMessage(err));
    } finally {
      if (isLatestMatchRequest(pageDataRequestId, pageDataRequestIdRef.current)) {
        if (initialHydrationFilterKeyRef.current === matchFiltersKey(nextFilters)) {
          initialHydrationFilterKeyRef.current = null;
        }
        setBusy(false);
      }
    }
  }

  async function loadMatches(nextFilters: MatchFilters, options: { silent?: boolean } = {}) {
    const timelineRequestId = ++timelineRequestIdRef.current;
    const timelineFilterKey = matchFiltersKey(nextFilters);
    if (!options.silent) setBusy(true);
    setError(null);
    try {
      const page = await listMatchTimelinePage(nextFilters, null, MATCH_PAGE_SIZE);
      if (!canCommitMatchTimelineRequest(
        timelineRequestId,
        timelineRequestIdRef.current,
        timelineFilterKey,
        matchFiltersKey(currentFiltersRef.current),
      )) return;
      commitLoadedMatches(nextFilters, page);
    } catch (err) {
      if (canCommitMatchTimelineRequest(
        timelineRequestId,
        timelineRequestIdRef.current,
        timelineFilterKey,
        matchFiltersKey(currentFiltersRef.current),
      )) setError(productErrorMessage(err));
    } finally {
      if (!options.silent && isLatestMatchRequest(timelineRequestId, timelineRequestIdRef.current)) setBusy(false);
    }
  }

  function setFilters(nextFilters: MatchFilters) {
    if (!guardMatchFilterChange(currentFiltersRef.current, nextFilters, favoriteController.confirmDiscard)) return false;
    commitFilters(nextFilters);
    return true;
  }

  function commitFilters(nextFilters: MatchFilters) {
    currentFiltersRef.current = nextFilters;
    pendingRevisionEventRef.current = null;
    setPendingNewMatches(0);
    if (matchFiltersKey(nextFilters) !== loadedFilterKeyRef.current) {
      setSelectedMatchIdSafely(null);
      nextCursorRef.current = null;
      setNextCursor(null);
      setBusy(true);
      setError(null);
    }
    setFiltersState(nextFilters);
  }

  function retryMatches() {
    if (pageDataReadyRef.current) {
      void loadMatches(currentFiltersRef.current);
      return;
    }
    void hydrateInitialMatches(currentFiltersRef.current);
  }

  function setSelectedMatchIdSafely(nextSelectedMatchId: string | null) {
    selectedMatchIdRef.current = nextSelectedMatchId;
    setSelectedMatchId(nextSelectedMatchId);
  }

  function commitLoadedMatches(
    nextFilters: MatchFilters,
    page: MatchPage,
    pageData?: Pick<MatchesPageCache, "overview" | "summary" | "options">,
  ) {
    const appliedRevision = pendingRevisionEventRef.current;
    if (appliedRevision && (nextFilters.season === undefined || nextFilters.season === appliedRevision.season)) {
      accountScopeRef.current = appliedRevision.accountScope;
      revisionTokenRef.current = getLiveMatchTimelineRevisionToken(appliedRevision.accountScope, nextFilters.season);
      pendingRevisionEventRef.current = null;
      setPendingNewMatches(0);
    }
    const preservesLoadedTail = Boolean(appliedRevision) && matchesRef.current.length > page.matches.length;
    const nextMatches = appliedRevision
      ? mergeLiveTimelinePage(matchesRef.current, page.matches, appliedRevision.phase)
      : page.matches;
    const nextPageCursor = preservesLoadedTail ? nextCursorRef.current : page.nextCursor;
    if (page.accountScope) accountScopeRef.current = page.accountScope;
    if (page.revisionToken) revisionTokenRef.current = page.revisionToken;
    const loadedFilterKey = matchFiltersKey(nextFilters);
    const cachePageData = pageData ?? (pageDataReadyRef.current ? pageDataRef.current : {});
    const selectedMatchId = selectedMatchIdRef.current;
    const nextSelectedMatchId = selectedMatchId && nextPageCursor && !nextMatches.some((match) => match.matchId === selectedMatchId)
      ? selectedMatchId
      : chooseMatchIdForLoadedMatches(selectedMatchId, nextMatches, true);
    loadedFilterKeyRef.current = loadedFilterKey;
    matchesRef.current = nextMatches;
    nextCursorRef.current = nextPageCursor;
    setMatches(nextMatches);
    setNextCursor(nextPageCursor);
    setTotalMatches(page.total ?? nextMatches.length);
    setSelectedMatchIdSafely(nextSelectedMatchId);
    updateMatchesPageCache({
      ...cachePageData,
      filters: nextFilters,
      matches: nextMatches,
      nextCursor: nextPageCursor,
      totalMatches: page.total ?? nextMatches.length,
      selectedMatchId: nextSelectedMatchId,
      accountScope: accountScopeRef.current,
      revisionToken: revisionTokenRef.current,
      loadedFilterKey,
      hasLoaded: pageDataReadyRef.current,
      scrollTop: readMatchListScrollTop(),
      scrollAnchor: readMatchScrollAnchor(),
    });
  }

  function rememberScroll() {
    atTimelineTopRef.current = readMatchListScrollTop() <= 24;
    updateMatchesPageCache({ scrollTop: readMatchListScrollTop() });
  }

  function rememberScrollWithAnchor() {
    updateMatchesPageCache({ scrollTop: readMatchListScrollTop(), scrollAnchor: readMatchScrollAnchor() });
  }

  function maybeLoadMore() {
    const container = timelineRef.current;
    if (!container || !nextCursorRef.current || loadingMoreRef.current) return;
    const scrollElement = isElementScrollable(container) ? container : document.scrollingElement;
    if (scrollElement && scrollElement.scrollHeight - scrollElement.scrollTop - scrollElement.clientHeight < 600) {
      void loadMoreMatches();
    }
  }

  async function loadMoreMatches() {
    const cursor = nextCursorRef.current;
    if (!cursor || loadingMoreRef.current) return;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    setLoadMoreError(null);
    const requestId = timelineRequestIdRef.current;
    const filterKey = matchFiltersKey(currentFiltersRef.current);
    try {
      const page = await listMatchTimelinePage(currentFiltersRef.current, cursor, MATCH_PAGE_SIZE);
      if (!canCommitMatchTimelineRequest(requestId, timelineRequestIdRef.current, filterKey, matchFiltersKey(currentFiltersRef.current))) return;
      const seen = new Set(matchesRef.current.map((match) => match.matchId));
      const nextMatches = [...matchesRef.current, ...page.matches.filter((match) => !seen.has(match.matchId))];
      matchesRef.current = nextMatches;
      nextCursorRef.current = page.nextCursor;
      setMatches(nextMatches);
      setNextCursor(page.nextCursor);
      updateMatchesPageCache({ matches: nextMatches, nextCursor: page.nextCursor });
    } catch (err) {
      if (canCommitMatchTimelineRequest(requestId, timelineRequestIdRef.current, filterKey, matchFiltersKey(currentFiltersRef.current))) {
        setLoadMoreError(productErrorMessage(err));
      }
    } finally {
      loadingMoreRef.current = false;
      setLoadingMore(false);
    }
  }

  function readMatchListScrollTop() {
    const container = timelineRef.current;
    if (container && isElementScrollable(container)) return container.scrollTop;
    return document.scrollingElement?.scrollTop ?? 0;
  }

  function readMatchScrollAnchor() {
    const container = timelineRef.current;
    if (!container) return getMatchesPageCache()?.scrollAnchor ?? null;
    const containerTop = isElementScrollable(container) ? container.getBoundingClientRect().top : 0;
    const candidates = Array.from(container.querySelectorAll<HTMLElement>("[data-match-id]")).map((item) => {
      const rect = item.getBoundingClientRect();
      return {
        matchId: item.dataset.matchId ?? "",
        top: rect.top,
        bottom: rect.bottom,
      };
    });
    return selectMatchScrollAnchor(candidates.filter((candidate) => candidate.matchId), containerTop);
  }

  function restoreTimelineScroll(scrollTop: number, scrollAnchor: { matchId: string; offsetTop: number } | null) {
    window.requestAnimationFrame(() => {
      restoreScrollOnce(scrollTop, scrollAnchor);
      window.requestAnimationFrame(() => {
        restoreScrollOnce(scrollTop, scrollAnchor);
      });
    });
  }

  function restoreScrollOnce(scrollTop: number, scrollAnchor: { matchId: string; offsetTop: number } | null) {
    const container = timelineRef.current;
    if (!container) return;
    const activeContainerTop = isElementScrollable(container) ? container.getBoundingClientRect().top : 0;
    const currentScrollTop = isElementScrollable(container) ? container.scrollTop : document.scrollingElement?.scrollTop ?? 0;
    if (scrollAnchor) {
      const target = container.querySelector<HTMLElement>(`[data-match-id="${CSS.escape(scrollAnchor.matchId)}"]`);
      if (target) {
        const itemTop = target.getBoundingClientRect().top;
        const nextScrollTop = computeMatchAnchorScrollTop({
          currentScrollTop,
          containerTop: activeContainerTop,
          itemTop,
          offsetTop: scrollAnchor.offsetTop,
        });
        if (isElementScrollable(container)) container.scrollTop = nextScrollTop;
        else if (document.scrollingElement) document.scrollingElement.scrollTop = nextScrollTop;
        return;
      }
    }
    if (isElementScrollable(container)) container.scrollTop = scrollTop;
    else if (document.scrollingElement) document.scrollingElement.scrollTop = scrollTop;
  }

  function selectMatch(matchId: string) {
    if (
      matchId !== selectedMatchIdRef.current &&
      !favoriteController.confirmDiscard()
    ) {
      return;
    }
    rememberScrollWithAnchor();
    setSelectedMatchIdSafely(matchId);
    setMobileDetailOpen(true);
  }

  return (
    <div className="matchesPage">
      <PageHeader
        title="对局记录"
        description={`${totalMatches} 场本地对局`}
        actions={(
          <PageSeasonSelect
            value={filters.season}
            seasons={options.seasons}
            includeAll
            allLabel="全部赛季"
            onChange={(season) => {
              const nextFilters = { ...filters };
              if (season === undefined) delete nextFilters.season;
              else nextFilters.season = season;
              setFilters(nextFilters);
            }}
          />
        )}
      />
      <div className="matchesPageControls">
        <FilterBar filters={filters} options={options} onChange={setFilters} />
      </div>

      <section className={`matchesWorkbench workspace listWorkspace phase6ListWorkspace phase6MatchWorkbench ${mobileDetailOpen ? "detailOpen" : ""}`}>
        <div ref={timelineRef} className="matchListPane phase6WorkbenchList" onScroll={() => { rememberScroll(); maybeLoadMore(); }}>
          <div>
            {pendingNewMatches > 0 ? (
              <button className="secondaryButton" type="button" onClick={() => {
                setPendingNewMatches(0);
                atTimelineTopRef.current = true;
                const container = timelineRef.current;
                if (container && isElementScrollable(container)) container.scrollTop = 0;
                else if (document.scrollingElement) document.scrollingElement.scrollTop = 0;
                void loadMatches(currentFiltersRef.current, { silent: true });
              }}>发现 {pendingNewMatches} 场新对局</button>
            ) : null}
            {error ? (
              <PageErrorState message={error} actionLabel="重新加载" onRetry={retryMatches} />
            ) : busy ? (
              <PageLoadingState message="正在加载对局列表..." />
            ) : matches.length === 0 ? (
              <PageEmptyState
                title="暂无符合条件的对局"
                description={hasVisibleFilters ? "调整或清除筛选条件后再试。" : "当前赛季没有可展示的本地对局。"}
                actionLabel={hasVisibleFilters ? "清除筛选" : "重新加载"}
                onAction={() => hasVisibleFilters ? setFilters(clearVisibleMatchFilters(filters)) : retryMatches()}
              />
            ) : (
              <MatchTimeline
                matches={matches}
                selectedMatchId={selectedMatchId}
                onSelect={selectMatch}
              />
            )}
            {nextCursor ? <button className="secondaryButton" type="button" disabled={loadingMore} onClick={() => void loadMoreMatches()}>{loadingMore ? "正在加载..." : "加载更多"}</button> : null}
            {loadMoreError ? <small role="status">{loadMoreError}</small> : null}
          </div>
        </div>
        <MatchDetailPane
          detail={selectedDetail?.matchId === selectedMatchId ? selectedDetail : null}
          loading={detailStateMatchId === selectedMatchId ? detailLoading : true}
          error={detailStateMatchId === selectedMatchId ? detailError : null}
          matchId={selectedMatchId}
          favoriteController={favoriteController}
          onRetry={() => setDetailRetryKey((value) => value + 1)}
          onQueryPlayer={onQueryPlayer ? (battleTag) => {
            if (favoriteController.confirmDiscard()) onQueryPlayer(battleTag);
          } : undefined}
          onBack={() => {
            if (!favoriteController.confirmDiscard()) return;
            setMobileDetailOpen(false);
          }}
        />
      </section>
    </div>
  );
}

function isElementScrollable(element: HTMLElement) {
  return element.scrollHeight > element.clientHeight + 1;
}
