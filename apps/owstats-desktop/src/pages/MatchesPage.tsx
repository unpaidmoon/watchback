import { AlertCircle, CircleDollarSign } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { FilterBar } from "../features/matches/FilterBar";
import { MatchTimeline } from "../features/matches/MatchTimeline";
import { filtersFromParams, hasLoadedOptions, paramsFromFilters, validateFilters } from "../features/matches/matchFilters";
import { getMatchDetails, getOverview, listMatchFilters, listMatchTimeline, refreshMatchEvaluations } from "../shared/api";
import { arenaHeaderMapKey, arenaHeaderTitle, arenaResultSubLabel, formatArenaRankTierLabel, formatArenaScore, formatArenaTabLabel } from "../shared/arenaDetailFormat";
import { arenaRankAssetUrl, resolveArenaModTraitInfo, resolvePerkInfo } from "../shared/assets";
import { MatchPerformanceBadges } from "../shared/PerformanceBadgesView";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore, resultEnglishLabel } from "../shared/matchDetailHeaderFormat";
import { playerHighlightMap, playerKeyFor, type HighlightMetric } from "../shared/playerHighlights";
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
import type { ArenaDetailSlice, FilterOptions, ImportSummary, MatchDetailView, MatchFilters, MatchSummary, Overview, PlayerRow } from "../shared/types";

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
  if (detail.arenaDetail) return <CompactArenaDetail detail={detail} loading={loading} />;

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

function CompactArenaDetail({ detail, loading }: { detail: MatchDetailView; loading: boolean }) {
  const arena = detail.arenaDetail;
  const [selectedKey, setSelectedKey] = useState("summary");
  const [expandedCompactArenaPlayerKeys, setExpandedCompactArenaPlayerKeys] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    setSelectedKey("summary");
    setExpandedCompactArenaPlayerKeys(new Set());
  }, [detail.matchId]);

  if (!arena) return null;

  function selectCompactArenaSlice(nextKey: string) {
    setSelectedKey(nextKey);
    setExpandedCompactArenaPlayerKeys(new Set());
  }

  function toggleCompactArenaPlayer(playerKey: string) {
    setExpandedCompactArenaPlayerKeys((current) => {
      const next = new Set(current);
      if (next.has(playerKey)) next.delete(playerKey);
      else next.add(playerKey);
      return next;
    });
  }

  const selected =
    selectedKey === "summary"
      ? arena.summary
      : arena.rounds.find((round) => round.key === selectedKey) ?? arena.summary;
  return (
    <section className={`phase6CompactDetail phase7CompactArenaDetail ${selected.result ?? detail.result ?? "pending"}`}>
      <nav className="phase7ArenaSelector" aria-label="角斗回合切换">
        <button
          type="button"
          className={`phase7ArenaTab summary ${selected.key === arena.summary.key ? "active" : ""}`}
          onClick={() => selectCompactArenaSlice(arena.summary.key)}
        >
          <span>{arena.summary.label}</span>
        </button>
        {arena.rounds.map((round) => (
          <button
            type="button"
            className={`phase7ArenaTab ${round.result ?? "neutral"} ${selected.key === round.key ? "active" : ""}`}
            onClick={() => selectCompactArenaSlice(round.key)}
            key={round.key}
          >
            <span>{formatArenaTabLabel(round)}</span>
            <strong>{formatArenaScore(round)}</strong>
          </button>
        ))}
      </nav>

      <header className="phase6CompactHero">
        <MapPreview className="phase6CompactHeroMap" mapKey={arenaHeaderMapKey(selected)} name={selected.mapName ?? "未知地图"} />
        <div className="phase6CompactResult">
          <strong>{selected.resultLabel ?? "未知"}</strong>
          <span>{arenaResultSubLabel(selected.result ?? detail.result)}</span>
        </div>
        <div className="phase6CompactHeroText">
          <span>{arenaHeaderTitle(selected)}</span>
          <strong>{formatArenaScore(selected)}</strong>
          <small>{formatDetailDuration(selected.durationSec)} / {formatDetailStartTime(selected.beginTs)}</small>
        </div>
      </header>

      <div className="phase6CompactDetailScroll">
        {loading && <div className="emptyState">正在刷新角斗详情...</div>}
        <div className="phase6CompactFacts">
          <CompactFact label="比分" value={formatArenaScore(selected)} />
          <CompactFact label="游戏时长" value={formatDetailDuration(selected.durationSec)} />
          {selected.key !== "summary" && <CompactFact label="游戏模式" value={detail.modeLabel ?? "-"} />}
          <CompactFact label="开始时间" value={formatDetailStartTime(selected.beginTs)} />
        </div>

        <div className="phase6CompactPlayers" aria-label="角斗双方玩家数据" key={selected.key}>
          <CompactTeam
            title="队友"
            tone="team"
            players={selected.teammates}
            showPerks={false}
            economyMode={selected.key === "summary" ? "final" : "round"}
            expandedPlayerKeys={expandedCompactArenaPlayerKeys}
            onTogglePlayer={toggleCompactArenaPlayer}
          />
          <div className="phase6CompactVs">VS</div>
          <CompactTeam
            title="对手"
            tone="enemy"
            players={selected.enemies}
            showPerks={false}
            economyMode={selected.key === "summary" ? "final" : "round"}
            expandedPlayerKeys={expandedCompactArenaPlayerKeys}
            onTogglePlayer={toggleCompactArenaPlayer}
          />
        </div>
        {selected.teammates.length + selected.enemies.length === 0 && <div className="emptyState">该回合暂无玩家明细。</div>}
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
  showPerks = true,
  economyMode,
  expandedPlayerKeys,
  onTogglePlayer,
}: {
  title: string;
  players: PlayerRow[];
  tone: "team" | "enemy";
  showPerks?: boolean;
  economyMode?: "final" | "round";
  expandedPlayerKeys?: ReadonlySet<string>;
  onTogglePlayer?: (playerKey: string) => void;
}) {
  const highlights = playerHighlightMap(players);
  return (
    <section className={`phase6CompactTeam ${tone}`}>
      <h3>{title}</h3>
      <div className="phase6CompactPlayerRows">
        {players.map((player, index) => {
          const key = compactPlayerKey(player, index);
          const expanded = expandedPlayerKeys?.has(key) ?? false;
          return (
            <div className="phase6CompactPlayerBlock" key={key}>
              <CompactPlayerRow
                player={player}
                showPerks={showPerks}
                highlights={highlights.get(playerKeyFor(player, index)) ?? new Set()}
                expanded={expanded}
                onToggle={onTogglePlayer ? () => onTogglePlayer(key) : undefined}
              />
              {expanded && economyMode && <CompactArenaPlayerExpansion player={player} economyMode={economyMode} />}
            </div>
          );
        })}
      </div>
    </section>
  );
}

function CompactPlayerRow({
  player,
  showPerks = true,
  highlights,
  expanded = false,
  onToggle,
}: {
  player: PlayerRow;
  showPerks?: boolean;
  highlights: Set<HighlightMetric>;
  expanded?: boolean;
  onToggle?: () => void;
}) {
  const rankIcon = arenaRankAssetUrl(player.rankKey);
  const rankTierLabel = formatArenaRankTierLabel(player.rankLabel);
  const perks = showPerks
    ? player.perks.flatMap((perk) => {
        const info = resolvePerkInfo(perk.guid);
        return info ? [info] : [];
      })
    : [];
  const rowContent = (
    <>
      <span className="phase6CompactAvatarPlate">
        <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} size="sm" />
      </span>
      <span className="phase6CompactPlayerName">
        <span className="phase7CompactPlayerNameLine">
          <strong>{player.name}</strong>
          {(rankIcon || rankTierLabel) && (
            <small className="phase7CompactRankBadge">
              {rankIcon && <img src={rankIcon} alt="" />}
              {rankTierLabel}
            </small>
          )}
        </span>
        {showPerks && (
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
        )}
      </span>
      <span className="phase6CompactPlayerMetric">
        <small>K/A/D</small>
        <strong>
          <CompactHighlight value={player.kills} active={highlights.has("kills")} />/{player.assists}/{player.deaths}
        </strong>
      </span>
      <span className="phase6CompactPlayerMetric phase6CompactPlayerMetricWide">
        <small>伤/疗/挡</small>
        <strong>
          <CompactHighlight value={player.damage} active={highlights.has("damage")} />/
          <CompactHighlight value={player.healing} active={highlights.has("healing")} />/
          <CompactHighlight value={player.mitigation} active={highlights.has("mitigation")} />
        </strong>
      </span>
      <span className="phase6CompactPlayerMetric">
        <small>承伤/受疗</small>
        <strong>{player.damageTaken}/{player.healingTaken}</strong>
      </span>
      <span className="phase6CompactPlayerMetric phase6CompactPlayerMetricFinal">
        <small>最后一击</small>
        <strong>{player.finalHits}</strong>
      </span>
    </>
  );

  if (onToggle) {
    return (
      <button
        type="button"
        className="phase6CompactPlayerRow phase7CompactPlayerExpandable"
        onClick={onToggle}
        aria-expanded={expanded}
        aria-label={`${player.name}${expanded ? "收起角斗详情" : "展开角斗详情"}`}
      >
        {rowContent}
      </button>
    );
  }

  return <article className="phase6CompactPlayerRow">{rowContent}</article>;
}

function CompactArenaPlayerExpansion({ player, economyMode }: { player: PlayerRow; economyMode: "final" | "round" }) {
  return (
    <div className="phase7CompactArenaExpansion">
      <div className="phase7CompactArenaEconomy">
        <span className="phase7CompactArenaMoneyIcon" aria-hidden="true">
          <CircleDollarSign size={46} strokeWidth={1.8} />
        </span>
        <strong className={player.worthMax ? "phase6StatHighlight" : ""}>{formatCompactArenaEconomy(player, economyMode)}</strong>
        <span className="phase7CompactArenaMoneyLabel">本场资金</span>
      </div>
      <CompactArenaSlotGroup title="异能" guids={player.traitGuids} slots={4} fallbackName="待补充特质" />
      <CompactArenaSlotGroup title="装备" guids={player.modGuids} slots={6} fallbackName="待补充异能" />
    </div>
  );
}

function CompactArenaSlotGroup({ title, guids, slots, fallbackName }: { title: string; guids: string[]; slots: number; fallbackName: string }) {
  const variant = slots === 4 ? "trait" : "mod";
  return (
    <div className={`phase7ArenaSlotGroup ${variant}`}>
      <span>{title}</span>
      <div className="phase7ArenaSlots">
        {Array.from({ length: slots }).map((_, index) => {
          const guid = guids[index];
          const info = resolveArenaModTraitInfo(guid);
          return (
            <span
              className={`phase7ArenaSlot ${variant} ${guid ? "filled" : ""}`}
              key={`${title}-${index}`}
              tabIndex={guid ? 0 : undefined}
              aria-label={guid ? info?.name ?? fallbackName : undefined}
            >
              {guid && info?.iconUrl && <img src={info.iconUrl} alt={info.name} />}
              {guid && variant === "trait" && <span className="phase7ArenaSlotName">{info?.name ?? fallbackName}</span>}
              {guid && (
                <span className="phase6PerkTooltipPanel" role="tooltip">
                  <strong>{info?.name ?? fallbackName}</strong>
                  {info?.description && <small>{info.description}</small>}
                </span>
              )}
            </span>
          );
        })}
      </div>
    </div>
  );
}

function formatCompactArenaEconomy(player: PlayerRow, mode: "final" | "round"): string {
  if (mode === "final") {
    if (player.endWorth !== null) return String(player.endWorth);
    if (player.worth !== null) return String(player.worth);
    return "-";
  }
  if (player.worth !== null && player.endWorth !== null && player.worth !== player.endWorth) {
    return `${player.worth} -> ${player.endWorth}`;
  }
  if (player.endWorth !== null) return String(player.endWorth);
  if (player.worth !== null) return String(player.worth);
  return "-";
}

function CompactHighlight({ value, active }: { value: number; active: boolean }) {
  return <em className={active ? "phase6StatHighlight" : ""}>{value}</em>;
}

function compactPlayerKey(player: PlayerRow, index: number) {
  return `${player.side}-${player.bnetId || player.name}-${player.heroGuid || player.heroName}-${index}`;
}

function cssEscape(value: string) {
  return value.replace(/["\\]/g, "\\$&");
}

function isElementScrollable(element: HTMLElement) {
  return element.scrollHeight > element.clientHeight + 1;
}
