import { AlertCircle } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { FilterBar } from "../features/matches/FilterBar";
import { MatchTimeline } from "../features/matches/MatchTimeline";
import { getMatchDetails, getOverview, listMatchFilters, listMatchTimeline, refreshMatchEvaluations } from "../shared/api";
import { resolvePerkInfo } from "../shared/assets";
import { MatchPerformanceBadges } from "../shared/PerformanceBadgesView";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore, resultEnglishLabel } from "../shared/matchDetailHeaderFormat";
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
import { HeroAvatar, MapPreview } from "../shared/phase6Components";
import { QaRegion } from "../shared/QaFeedbackLayer";
import { productErrorMessage } from "../shared/phase6Shell";
import { preserveQaParam } from "../shared/qaFeedback";
import type { FilterOptions, ImportSummary, MatchDetailView, MatchFilters, MatchSummary, Overview, PlayerRow } from "../shared/types";

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

function CompactMatchDetail({ detail, loading, error, matchId }: { detail: MatchDetailView | null; loading: boolean; error: string | null; matchId: string }) {
  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }
  if (loading && !detail) return <div className="emptyState phase6WorkbenchEmpty">正在读取对局详情...</div>;
  if (!detail) return <div className="emptyState phase6WorkbenchEmpty">未找到对局详情：{matchId}</div>;

  const mapName = detail.mapName ?? "未知地图";
  const compactPlayers = [...detail.teammates.slice(0, 6), ...detail.enemies.slice(0, 6)];
  return (
    <section className={`phase6CompactDetail ${detail.result ?? "pending"}`}>
      <header className="phase6CompactHero">
        <MapPreview className="phase6CompactHeroMap" mapKey={detail.mapGuid ?? detail.mapName} name={mapName} />
        <div className="phase6CompactResult">
          <strong>{detail.resultLabel ?? "未知"}</strong>
          <span>{resultEnglishLabel(detail.result)}</span>
        </div>
        <div className="phase6CompactHeroText">
          <span>{mapName}</span>
          <strong>{formatFinalScore(detail)}</strong>
          <small>{detail.modeLabel ?? "-"} · {formatDetailDuration(detail.durationSec)} · {formatDetailStartTime(detail.beginTs)}</small>
        </div>
      </header>

      <div className="phase6CompactDetailScroll">
        <div className="phase6CompactFacts">
          <CompactFact label="最终比分" value={formatFinalScore(detail)} />
          <CompactFact label="游戏模式" value={detail.modeLabel ?? "-"} />
          <CompactFact label="游戏时长" value={formatDetailDuration(detail.durationSec)} />
          <CompactFact label="开始时间" value={formatDetailStartTime(detail.beginTs)} />
        </div>

        <div className="phase6CompactPerformance">
          <div>
            <span className="phase6Eyebrow">对局表现</span>
            <strong>评分与同队奖牌</strong>
          </div>
          <MatchPerformanceBadges evaluation={detail.evaluation} teamAwards={detail.teamAwards} />
        </div>

        <div className="phase6CompactPlayers" aria-label="双方玩家数据">
          <CompactTeam
            title="队友"
            tone="team"
            players={detail.teammates.slice(0, 6)}
          />
          <div className="phase6CompactVs">VS</div>
          <CompactTeam
            title="对手"
            tone="enemy"
            players={detail.enemies.slice(0, 6)}
          />
        </div>
        {compactPlayers.length === 0 && <div className="emptyState">该对局暂无玩家明细。</div>}
      </div>
    </section>
  );
}

function CompactFact({ label, value }: { label: string; value: string }) {
  return (
    <span>
      <small>{label}</small>
      <strong>{value}</strong>
    </span>
  );
}

function CompactTeam({
  title,
  players,
  tone,
}: {
  title: string;
  players: PlayerRow[];
  tone: "team" | "enemy";
}) {
  return (
    <section className={`phase6CompactTeam ${tone}`}>
      <h3>{title}</h3>
      <div className="phase6CompactPlayerRows">
        {players.map((player, index) => {
          const key = compactPlayerKey(player, index);
          return (
            <div className="phase6CompactPlayerBlock" key={key}>
              <CompactPlayerRow player={player} />
            </div>
          );
        })}
      </div>
    </section>
  );
}

function CompactPlayerRow({ player }: { player: PlayerRow }) {
  const perks = player.perks.flatMap((perk) => {
    const info = resolvePerkInfo(perk.guid);
    return info ? [info] : [];
  });
  return (
    <article className="phase6CompactPlayerRow">
      <span className="phase6CompactAvatarPlate">
        <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} size="sm" />
      </span>
      <span className="phase6CompactPlayerName">
        <strong>{player.name}</strong>
        <span className="phase6CompactPerkStrip" aria-label="威能">
          {perks.length > 0 ? (
            perks.slice(0, 2).map((perk) => (
              <span className="phase6PerkTooltip" key={perk.id}>
                <img className="phase6CompactPerkIcon" src={perk.iconUrl ?? undefined} alt={perk.name} />
                <span className="phase6PerkTooltipPanel" role="tooltip">
                  <strong>{perk.name}</strong>
                  <small>{perk.description ?? "暂无释义"}</small>
                </span>
              </span>
            ))
          ) : (
            <span className="phase6CompactPerkEmpty">未记录威能</span>
          )}
        </span>
      </span>
      <span className="phase6CompactPlayerMetric">
        <small>K/A/D</small>
        <strong>{player.kills}/{player.assists}/{player.deaths}</strong>
      </span>
      <span className="phase6CompactPlayerMetric phase6CompactPlayerMetricWide">
        <small>伤/疗/挡</small>
        <strong>{player.damage}/{player.healing}/{player.mitigation}</strong>
      </span>
      <span className="phase6CompactPlayerMetric">
        <small>承伤/受疗</small>
        <strong>{player.damageTaken}/{player.healingTaken}</strong>
      </span>
      <span className="phase6CompactPlayerMetric phase6CompactPlayerMetricFinal">
        <small>最后一击</small>
        <strong>{player.finalHits}</strong>
      </span>
    </article>
  );
}

function compactPlayerKey(player: PlayerRow, index: number) {
  return `${player.side}-${player.bnetId || player.name}-${player.heroGuid || player.heroName}-${index}`;
}

function filtersFromParams(params: URLSearchParams): MatchFilters {
  const filters: MatchFilters = {};
  const season = params.get("season");
  if (season && Number.isFinite(Number(season))) filters.season = Number(season);
  for (const key of ["hero", "map", "result", "mode", "primaryTag", "playerName", "startDate", "endDate"] as const) {
    const value = params.get(key);
    if (!value) continue;
    if (key === "startDate") filters.dateFrom = validDate(value) ? value : undefined;
    else if (key === "endDate") filters.dateTo = validDate(value) ? value : undefined;
    else filters[key] = value;
  }
  return filters;
}

function paramsFromFilters(filters: MatchFilters, selectedMatchId?: string | null) {
  const params = new URLSearchParams();
  if (selectedMatchId) params.set("matchId", selectedMatchId);
  if (filters.season) params.set("season", String(filters.season));
  if (filters.hero) params.set("hero", filters.hero);
  if (filters.map) params.set("map", filters.map);
  if (filters.result) params.set("result", filters.result);
  if (filters.mode) params.set("mode", filters.mode);
  if (filters.primaryTag) params.set("primaryTag", filters.primaryTag);
  if (filters.playerName) params.set("playerName", filters.playerName);
  if (filters.dateFrom) params.set("startDate", filters.dateFrom);
  if (filters.dateTo) params.set("endDate", filters.dateTo);
  return params;
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
