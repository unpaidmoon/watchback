import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { FilterBar } from "../features/matches/FilterBar";
import { MatchDetailPane } from "../features/matches/MatchDetailPane";
import { useMatchFavoriteEditor } from "../features/matches/useMatchFavoriteEditor";
import { MatchTimeline } from "../features/matches/MatchTimeline";
import {
  activeMatchFilterChips,
  clearVisibleMatchFilters,
  filtersFromParams,
  hasLoadedOptions,
  paramsFromFilters,
  restoreRejectedMatchesSearchParams,
  validateFilters,
} from "../features/matches/matchFilters";
import { getMatchDetails, getOverview, listMatchFilters, listMatchTimeline, refreshMatchEvaluations } from "../shared/api";
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
import { QaRegion } from "../shared/QaFeedbackLayer";
import { useUnsavedChangesGuard } from "../shared/UnsavedChangesGuard";
import { productErrorMessage } from "../shared/phase6Shell";
import { preserveQaParam } from "../shared/qaFeedback";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";
import type { FilterOptions, ImportSummary, MatchDetailView, MatchFilters, MatchSummary, Overview, } from "../shared/types";

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
        return updateMatchFavoriteState(current, selectedFavoriteMatchId, favorite !== null);
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
    if (canHydrateCache) return;
    void hydrateInitialMatches(filters);
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
      const restoredSearchParams = restoreRejectedMatchesSearchParams(
        lastAcceptedSearchParamsKeyRef.current,
        filters,
        selectedMatchId,
      );
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
    rememberScroll();
    const nextSearchParams = preserveQaParam(searchParams, paramsFromFilters(filters, selectedMatchId));
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
      selectedMatchId,
      loadedFilterKey: loadedFilterKeyRef.current,
      hasLoaded: pageDataReadyRef.current,
    });
  }, [overview, summary, options, filters, matches, selectedMatchId]);

  useEffect(() => {
    window.addEventListener("scroll", rememberScroll, { passive: true });
    window.addEventListener("pagehide", rememberScroll);
    document.addEventListener("visibilitychange", rememberScroll);
    return () => {
      rememberScroll();
      window.removeEventListener("scroll", rememberScroll);
      window.removeEventListener("pagehide", rememberScroll);
      document.removeEventListener("visibilitychange", rememberScroll);
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
        { getOverview, listMatchFilters, listMatchTimeline },
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
        commitLoadedMatches(nextFilters, cache.matches, pageData);
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

  async function loadMatches(nextFilters: MatchFilters) {
    const timelineRequestId = ++timelineRequestIdRef.current;
    const timelineFilterKey = matchFiltersKey(nextFilters);
    setBusy(true);
    setError(null);
    try {
      const nextMatches = await listMatchTimeline(nextFilters);
      if (!canCommitMatchTimelineRequest(
        timelineRequestId,
        timelineRequestIdRef.current,
        timelineFilterKey,
        matchFiltersKey(currentFiltersRef.current),
      )) return;
      commitLoadedMatches(nextFilters, nextMatches);
    } catch (err) {
      if (canCommitMatchTimelineRequest(
        timelineRequestId,
        timelineRequestIdRef.current,
        timelineFilterKey,
        matchFiltersKey(currentFiltersRef.current),
      )) setError(productErrorMessage(err));
    } finally {
      if (isLatestMatchRequest(timelineRequestId, timelineRequestIdRef.current)) setBusy(false);
    }
  }

  function setFilters(nextFilters: MatchFilters) {
    if (!guardMatchFilterChange(currentFiltersRef.current, nextFilters, favoriteController.confirmDiscard)) return false;
    commitFilters(nextFilters);
    return true;
  }

  function commitFilters(nextFilters: MatchFilters) {
    currentFiltersRef.current = nextFilters;
    if (matchFiltersKey(nextFilters) !== loadedFilterKeyRef.current) {
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
    nextMatches: MatchSummary[],
    pageData?: Pick<MatchesPageCache, "overview" | "summary" | "options">,
  ) {
    const loadedFilterKey = matchFiltersKey(nextFilters);
    const cachePageData = pageData ?? (pageDataReadyRef.current ? pageDataRef.current : {});
    const nextSelectedMatchId = chooseMatchIdForLoadedMatches(selectedMatchIdRef.current, nextMatches, true);
    loadedFilterKeyRef.current = loadedFilterKey;
    setMatches(nextMatches);
    setSelectedMatchIdSafely(nextSelectedMatchId);
    updateMatchesPageCache({
      ...cachePageData,
      filters: nextFilters,
      matches: nextMatches,
      selectedMatchId: nextSelectedMatchId,
      loadedFilterKey,
      hasLoaded: pageDataReadyRef.current,
      scrollTop: readMatchListScrollTop(),
      scrollAnchor: readMatchScrollAnchor(),
    });
  }

  function rememberScroll() {
    updateMatchesPageCache({ scrollTop: readMatchListScrollTop(), scrollAnchor: readMatchScrollAnchor() });
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
    rememberScroll();
    setSelectedMatchIdSafely(matchId);
    setMobileDetailOpen(true);
  }

  return (
    <div className="matchesPage">
      <PageHeader
        title="对局记录"
        description={`${matches.length} 场本地对局`}
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
        <div ref={timelineRef} className="matchListPane phase6WorkbenchList" onScroll={rememberScroll}>
          <QaRegion id="MAT-LIST">
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
          </QaRegion>
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
