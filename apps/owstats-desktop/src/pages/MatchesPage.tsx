import { AlertCircle, ListFilter, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { FilterBar } from "../features/matches/FilterBar";
import { MatchTimeline } from "../features/matches/MatchTimeline";
import { getOverview, listMatchFilters, listMatchTimeline, refreshMatchEvaluations } from "../shared/api";
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
import { MetricCard, StatusBadge } from "../shared/phase6Components";
import { QaRegion } from "../shared/QaFeedbackLayer";
import { productErrorMessage } from "../shared/phase6Shell";
import { preserveQaParam } from "../shared/qaFeedback";
import type { FilterOptions, ImportSummary, MatchFilters, MatchSummary, Overview } from "../shared/types";

const emptyOptions: FilterOptions = {
  seasons: [],
  heroes: [],
  maps: [],
  results: [],
  modes: []
};

export function MatchesPage() {
  const navigate = useNavigate();
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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const winRate = useMemo(() => {
    if (matches.length === 0) return "0%";
    const wins = matches.filter((match) => match.result === "win").length;
    return `${Math.round((wins / matches.length) * 100)}%`;
  }, [matches]);

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
    setSearchParams(preserveQaParam(searchParams, paramsFromFilters(filters)), { replace: true });
    if (skipInitialFilterLoadRef.current) {
      skipInitialFilterLoadRef.current = false;
      return;
    }
    if (matchFiltersKey(filters) === loadedFilterKeyRef.current) return;
    void loadMatches(filters);
  }, [filters]);

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
      <section className="phase6PageHeader phase6MatchesHeader">
        <div>
          <span className="phase6Eyebrow">对局</span>
          <h2>赛季对局工作台</h2>
          <p>在这里筛选比赛、查看评分标签，并进入单场复盘报告。</p>
        </div>
        <div className="phase6CommandStatus">
          <StatusBadge>
            <ListFilter size={15} /> {activeFilterCount(filters)} 个筛选
          </StatusBadge>
          <StatusBadge tone={busy ? "warn" : "good"}>
            <RefreshCw size={15} /> {busy ? "刷新中" : "列表就绪"}
          </StatusBadge>
        </div>
      </section>

      <section className="phase6MatchMetrics">
        <MetricCard label="本地赛季库" value={summary ? `S${summary.season}` : "-"} meta={overview?.databasePath ? "本地数据已连接" : "等待桌面后端"} />
        <MetricCard label="预设 / 开放" value={String(summary?.presetMatches ?? matches.length)} meta="用于复盘与分析" />
        <MetricCard label="当前列表" value={String(matches.length)} meta="筛选后的对局数" />
        <MetricCard label="当前胜率" value={winRate} meta="按当前列表计算" />
      </section>

      {error && (
        <div className="errorBanner phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      <FilterBar filters={filters} options={options} onChange={setFilters} />

      <section className="workspace listWorkspace phase6ListWorkspace">
        <QaRegion id="MAT-LIST">
          <MatchTimeline
            ref={timelineRef}
            matches={matches}
            onScroll={rememberScroll}
            onSelect={(matchId) => {
              rememberScroll();
              navigate(`/matches/${matchId}`);
            }}
          />
        </QaRegion>
      </section>
    </>
  );
}

function filtersFromParams(params: URLSearchParams): MatchFilters {
  const filters: MatchFilters = {};
  const season = params.get("season");
  if (season && Number.isFinite(Number(season))) filters.season = Number(season);
  for (const key of ["hero", "map", "result", "mode", "primaryTag", "startDate", "endDate"] as const) {
    const value = params.get(key);
    if (!value) continue;
    if (key === "startDate") filters.dateFrom = validDate(value) ? value : undefined;
    else if (key === "endDate") filters.dateTo = validDate(value) ? value : undefined;
    else filters[key] = value;
  }
  return filters;
}

function paramsFromFilters(filters: MatchFilters) {
  const params = new URLSearchParams();
  if (filters.season) params.set("season", String(filters.season));
  if (filters.hero) params.set("hero", filters.hero);
  if (filters.map) params.set("map", filters.map);
  if (filters.result) params.set("result", filters.result);
  if (filters.mode) params.set("mode", filters.mode);
  if (filters.primaryTag) params.set("primaryTag", filters.primaryTag);
  if (filters.dateFrom) params.set("startDate", filters.dateFrom);
  if (filters.dateTo) params.set("endDate", filters.dateTo);
  return params;
}

function activeFilterCount(filters: MatchFilters) {
  return Object.values(filters).filter(Boolean).length;
}

function validDate(value: string) {
  return /^\d{4}-\d{2}-\d{2}$/.test(value);
}

function hasLoadedOptions(options: FilterOptions) {
  return (
    options.seasons.length > 0 ||
    options.heroes.length > 0 ||
    options.maps.length > 0 ||
    options.results.length > 0 ||
    options.modes.length > 0
  );
}

function validateFilters(filters: MatchFilters, options: FilterOptions): MatchFilters {
  const next = { ...filters };
  if (next.season !== undefined && !options.seasons.includes(next.season)) delete next.season;
  if (next.hero && !options.heroes.some((hero) => hero.id === next.hero)) delete next.hero;
  if (next.map && !options.maps.some((map) => map.id === next.map)) delete next.map;
  if (next.result && !options.results.includes(next.result)) delete next.result;
  if (next.mode && !options.modes.includes(next.mode)) delete next.mode;
  if (next.primaryTag && !["带飞局", "尽力局", "躺赢局", "背锅局"].includes(next.primaryTag)) {
    delete next.primaryTag;
  }
  return next;
}

function cssEscape(value: string) {
  return value.replace(/["\\]/g, "\\$&");
}

function isElementScrollable(element: HTMLElement) {
  return element.scrollHeight > element.clientHeight + 1;
}
