import { AlertCircle } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { CompactMatchDetail } from "../features/matches/CompactMatchDetail";
import { FilterBar } from "../features/matches/FilterBar";
import { MatchTimeline } from "../features/matches/MatchTimeline";
import { filtersFromParams, hasLoadedOptions, paramsFromFilters, validateFilters } from "../features/matches/matchFilters";
import { getMatchDetails, getOverview, listMatchFilters, listMatchTimeline, refreshMatchEvaluations } from "../shared/api";
import {
  canHydrateMatchesPageCache,
  chooseInitialMatchFilters,
  computeMatchAnchorScrollTop,
  getMatchesPageCache,
  hasExplicitMatchFilterParams,
  matchFiltersKey,
  preloadMatchesPageCache,
  selectMatchScrollAnchor,
  updateMatchesPageCache,
} from "../shared/matchesPageCache";
import { QaRegion } from "../shared/QaFeedbackLayer";
import { productErrorMessage } from "../shared/phase6Shell";
import { preserveQaParam } from "../shared/qaFeedback";
import type { FilterOptions, ImportSummary, MatchDetailView, MatchFilters, MatchSummary, Overview, } from "../shared/types";

const emptyOptions: FilterOptions = {
  seasons: [],
  heroes: [],
  maps: [],
  results: [],
  modes: []
};

export function MatchesPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const cachedPage = useMemo(() => getMatchesPageCache(), []);
  const hasExplicitFilters = useMemo(() => hasExplicitMatchFilterParams(searchParams), []);
  const initialFilters = useMemo(
    () => chooseInitialMatchFilters(filtersFromParams(searchParams), cachedPage?.filters, hasExplicitFilters),
    [],
  );
  const canHydrateCache = useMemo(() => canHydrateMatchesPageCache(cachedPage, initialFilters, hasExplicitFilters), []);
  const initialCacheRef = useRef(cachedPage);
  const timelineRef = useRef<HTMLDivElement | null>(null);
  const skipInitialFilterLoadRef = useRef(true);
  const loadedFilterKeyRef = useRef(canHydrateCache ? cachedPage?.loadedFilterKey ?? "" : "");
  const [overview, setOverview] = useState<Overview | null>(canHydrateCache ? cachedPage?.overview ?? null : null);
  const [summary, setSummary] = useState<ImportSummary | null>(canHydrateCache ? cachedPage?.summary ?? null : null);
  const [options, setOptions] = useState<FilterOptions>(canHydrateCache ? cachedPage?.options ?? emptyOptions : emptyOptions);
  const [filters, setFilters] = useState<MatchFilters>(initialFilters);
  const [matches, setMatches] = useState<MatchSummary[]>(canHydrateCache ? cachedPage?.matches ?? [] : []);
  const [selectedMatchId, setSelectedMatchId] = useState<string | null>(() => searchParams.get("matchId"));
  const [selectedDetail, setSelectedDetail] = useState<MatchDetailView | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (canHydrateCache) return;
    void hydrateInitialMatches(filters);
  }, []);

  useEffect(() => {
    if (!canHydrateCache || !initialCacheRef.current?.hasLoaded) return;
    restoreTimelineScroll(initialCacheRef.current.scrollTop, initialCacheRef.current.scrollAnchor);
  }, [matches.length]);

  useEffect(() => {
    rememberScroll();
    updateMatchesPageCache({ filters });
    setSearchParams(preserveQaParam(searchParams, paramsFromFilters(filters, selectedMatchId)), { replace: true });
    if (skipInitialFilterLoadRef.current) {
      skipInitialFilterLoadRef.current = false;
      return;
    }
    if (matchFiltersKey(filters) === loadedFilterKeyRef.current) return;
    void loadMatches(filters);
  }, [filters, selectedMatchId]);

  useEffect(() => {
    if (matches.length === 0) {
      if (selectedMatchId !== null) setSelectedMatchId(null);
      return;
    }
    if (!selectedMatchId || !matches.some((match) => match.matchId === selectedMatchId)) {
      setSelectedMatchId(matches[0].matchId);
    }
  }, [matches, selectedMatchId]);

  useEffect(() => {
    if (!selectedMatchId) {
      setSelectedDetail(null);
      setDetailError(null);
      setDetailLoading(false);
      return;
    }
    let cancelled = false;
    setDetailLoading(true);
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
  }, [selectedMatchId]);

  useEffect(() => {
    updateMatchesPageCache({ overview, summary, options, filters, matches, loadedFilterKey: loadedFilterKeyRef.current });
  }, [overview, summary, options, filters, matches]);

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
  }, [options]);

  async function hydrateInitialMatches(nextFilters: MatchFilters) {
    setBusy(true);
    setError(null);
    try {
      const cache = await preloadMatchesPageCache({ getOverview, listMatchFilters, listMatchTimeline }, nextFilters);
      loadedFilterKeyRef.current = cache.loadedFilterKey;
      setOverview(cache.overview);
      setSummary(cache.summary);
      setOptions(cache.options);
      setMatches(cache.matches);
      void refreshMatchEvaluations()
        .then(() => loadMatches(nextFilters))
        .catch(() => undefined);
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function loadMatches(nextFilters: MatchFilters) {
    try {
      const nextMatches = await listMatchTimeline(nextFilters);
      const loadedFilterKey = matchFiltersKey(nextFilters);
      loadedFilterKeyRef.current = loadedFilterKey;
      setMatches(nextMatches);
      updateMatchesPageCache({
        filters: nextFilters,
        matches: nextMatches,
        loadedFilterKey,
        hasLoaded: true,
        scrollTop: readMatchListScrollTop(),
        scrollAnchor: readMatchScrollAnchor(),
      });
    } catch (err) {
      setError(productErrorMessage(err));
    }
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
      const target = container.querySelector<HTMLElement>(`[data-match-id="${cssEscape(scrollAnchor.matchId)}"]`);
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

  return (
    <>
      {error && (
        <div className="errorBanner phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      <FilterBar filters={filters} options={options} onChange={setFilters} />

      <section className="workspace listWorkspace phase6ListWorkspace phase6MatchWorkbench">
        <div className="phase6WorkbenchList">
          <QaRegion id="MAT-LIST">
            <MatchTimeline
              ref={timelineRef}
              matches={matches}
              selectedMatchId={selectedMatchId}
              onScroll={rememberScroll}
              onSelect={(matchId) => {
                rememberScroll();
                setSelectedMatchId(matchId);
              }}
            />
          </QaRegion>
        </div>
        <aside className="phase6WorkbenchDetail" aria-label="对局详情">
          {selectedMatchId ? (
            <CompactMatchDetail detail={selectedDetail} loading={detailLoading} error={detailError} matchId={selectedMatchId} />
          ) : (
            <div className="emptyState phase6WorkbenchEmpty">选择左侧对局查看详情。</div>
          )}
        </aside>
      </section>
    </>
  );
}

function cssEscape(value: string) {
  return value.replace(/["\\]/g, "\\$&");
}

function isElementScrollable(element: HTMLElement) {
  return element.scrollHeight > element.clientHeight + 1;
}
