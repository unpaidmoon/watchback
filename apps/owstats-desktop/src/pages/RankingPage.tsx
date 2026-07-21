import { ArrowDown, ArrowUp, ArrowUpDown } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { getRankings } from "../shared/api";
import { resolveHeroDisplayName, resolveMapDisplayName } from "../shared/assets";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";
import { HeroAvatar, MapPreview } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import {
  defaultRankingSort,
  formatPlayTimeHours,
  formatWinRate,
  sortRankingRows,
  toggleRankingSort,
} from "../shared/rankingUtils";
import type { RankingSort, RankingSortKey } from "../shared/rankingUtils";
import type { RankingResponse, RankingRow } from "../shared/types";

type RankingTab = "heroes" | "maps";

const heroSortKeys: RankingSortKey[] = ["matches", "winRate", "playTimeHours", "kda"];
const mapSortKeys: RankingSortKey[] = ["matches", "winRate", "playTimeHours", "kda"];

export function createLatestRankingRequestGuard() {
  let latestRequest = 0;

  return {
    begin() {
      latestRequest += 1;
      return latestRequest;
    },
    isCurrent(request: number) {
      return latestRequest === request;
    },
    invalidate(request?: number) {
      if (request === undefined || latestRequest === request) latestRequest += 1;
    },
  };
}

const sortLabels: Record<RankingSortKey, string> = {
  bestHero: "最佳英雄",
  matches: "场数",
  winRate: "胜率",
  playTimeHours: "时长",
  kda: "K/D/A",
  rating: "英雄评分",
};

export function RankingPage() {
  const [params, setParams] = useSearchParams();
  const [tab, setTab] = useState<RankingTab>(params.get("tab") === "maps" ? "maps" : "heroes");
  const requestedSeason = parseSeasonParam(params.get("season"));
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>(requestedSeason);
  const [sort, setSort] = useState<RankingSort>(() => defaultRankingSort(tab));
  const [ranking, setRanking] = useState<RankingResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retryRequest, setRetryRequest] = useState(0);
  const rankingRequestGuard = useRef(createLatestRankingRequestGuard());

  useEffect(() => {
    const nextTab = params.get("tab") === "maps" ? "maps" : "heroes";
    setTab(nextTab);
    setSort(defaultRankingSort(nextTab));
    setSelectedSeason(requestedSeason);
  }, [params, requestedSeason]);

  useEffect(() => {
    const request = rankingRequestGuard.current.begin();
    setLoading(true);
    setError(null);
    void getRankings(selectedSeason)
      .then((nextRanking) => {
        if (!rankingRequestGuard.current.isCurrent(request)) return;
        setRanking(nextRanking);
        const nextSeason = nextRanking.season ?? undefined;
        if (nextSeason !== selectedSeason) {
          setSelectedSeason(nextSeason);
          const nextParams = new URLSearchParams(params);
          if (nextSeason === undefined) nextParams.delete("season");
          else nextParams.set("season", String(nextSeason));
          setParams(nextParams, { replace: true });
        }
      })
      .catch((err) => {
        if (rankingRequestGuard.current.isCurrent(request)) setError(productErrorMessage(err));
      })
      .finally(() => {
        if (rankingRequestGuard.current.isCurrent(request)) setLoading(false);
      });

    return () => rankingRequestGuard.current.invalidate(request);
  }, [selectedSeason, retryRequest]);

  const rows = useMemo(() => {
    const source = (tab === "maps" ? ranking?.maps ?? [] : ranking?.heroes ?? []).map((row) => ({
      ...row,
      name: tab === "maps" ? resolveMapDisplayName(row.key, row.name) : resolveHeroDisplayName(row.key, row.name),
      bestHeroName:
        tab === "maps" && row.bestHeroName
          ? resolveHeroDisplayName(row.bestHeroGuid, row.bestHeroName)
          : row.bestHeroName,
    }));
    return sortRankingRows(source, sort);
  }, [ranking, sort, tab]);

  const sourceCount = tab === "maps" ? ranking?.maps.length ?? 0 : ranking?.heroes.length ?? 0;
  const availableSeasons = ranking?.availableSeasons ?? [];

  function switchTab(next: RankingTab) {
    setTab(next);
    setSort(defaultRankingSort(next));
    const nextParams = new URLSearchParams(params);
    if (next === "maps") {
      nextParams.set("tab", "maps");
    } else {
      nextParams.delete("tab");
    }
    setParams(nextParams);
  }

  function changeSeason(nextSeason: number) {
    setSelectedSeason(nextSeason);
    const nextParams = new URLSearchParams(params);
    nextParams.set("season", String(nextSeason));
    setParams(nextParams);
  }

  function changeSort(key: RankingSortKey) {
    setSort((current) => toggleRankingSort(current, key));
  }

  return (
    <section className="workspace rankingWorkspace phase6RankingWorkspace">
      <PageHeader
        title="英雄与地图排行"
        description={tab === "heroes" ? "按本地赛季汇总英雄表现" : "按本地赛季汇总地图表现"}
        actions={(
          <PageSeasonSelect
            value={selectedSeason}
            seasons={availableSeasons}
            disabled={availableSeasons.length === 0}
            onChange={(season) => season !== undefined && changeSeason(season)}
          />
        )}
      />

      <div className="rankingToolbar phase6RankingToolbar">
        <div className="segmentedControl phase6SegmentedControl" role="group" aria-label="排行类型">
          <button
            type="button"
            className={tab === "heroes" ? "active" : ""}
            aria-pressed={tab === "heroes"}
            onClick={() => switchTab("heroes")}
          >
            英雄排行
          </button>
          <button
            type="button"
            className={tab === "maps" ? "active" : ""}
            aria-pressed={tab === "maps"}
            onClick={() => switchTab("maps")}
          >
            地图排行
          </button>
        </div>
        <span className="phase6RankingCount">{sourceCount} 项</span>
      </div>

      {error ? (
        <PageErrorState message={error} onRetry={() => setRetryRequest((value) => value + 1)} />
      ) : loading ? (
        <PageLoadingState message="正在计算排行..." />
      ) : rows.length === 0 ? (
        <PageEmptyState
          title="暂无排行"
          description="本地赛季数据不足，暂无排行。"
          actionLabel="重新加载"
          onAction={() => setRetryRequest((value) => value + 1)}
        />
      ) : (
        <RankingTable rows={rows} tab={tab} sort={sort} onSort={changeSort} />
      )}
    </section>
  );
}

export function parseSeasonParam(value: string | null) {
  if (!value) return undefined;
  const season = Number(value);
  return Number.isInteger(season) && season > 0 ? season : undefined;
}

function RankingTable({
  rows,
  tab,
  sort,
  onSort,
}: {
  rows: RankingRow[];
  tab: RankingTab;
  sort: RankingSort;
  onSort: (key: RankingSortKey) => void;
}) {
  const sortKeys = tab === "maps" ? mapSortKeys : heroSortKeys;
  return (
    <div
      className={`rankingTable phase6RankingTable ${tab}`}
      role="table"
      aria-label={tab === "maps" ? "地图排行" : "英雄排行"}
      aria-rowcount={rows.length + 1}
      aria-colcount={tab === "maps" ? 7 : 6}
    >
      <div className="rankingHead phase6RankingHead" role="row" aria-rowindex={1}>
        <span role="columnheader">{tab === "maps" ? "地图" : "英雄"}</span>
        {tab === "maps" && <span role="columnheader">最佳英雄</span>}
        {sortKeys.map((key) => (
          <SortButton key={key} sortKey={key} activeSort={sort} tab={tab} onSort={onSort} />
        ))}
        <span role="columnheader">{tab === "maps" ? "地图评分" : "英雄评分"}</span>
      </div>
      {rows.map((row, index) => (
        <div
          className="rankingLine phase6RankingLine"
          key={`${tab}-${row.key}`}
          role="row"
          aria-rowindex={index + 2}
        >
          <span className="phase6RankName" role="cell">
            <b>{index + 1}</b>
            {tab === "maps" ? (
              <MapPreview mapKey={row.key || row.name} name={row.name} className="phase6RankMapPreview" />
            ) : (
              <HeroAvatar heroKey={row.key || row.name} name={row.name} />
            )}
            <strong>{row.name}</strong>
          </span>
          {tab === "maps" && <BestHeroCell row={row} />}
          <span role="cell">{row.matches}</span>
          <span role="cell">{formatWinRate(row.winRate)}</span>
          <span role="cell">{formatPlayTimeHours(row.playTimeHours)}</span>
          <span role="cell">{formatKda(row.kda)}</span>
          <span role="cell">{formatRating(row.rating)}</span>
        </div>
      ))}
    </div>
  );
}

function SortButton({
  sortKey,
  activeSort,
  tab,
  onSort,
}: {
  sortKey: RankingSortKey;
  activeSort: RankingSort;
  tab: RankingTab;
  onSort: (key: RankingSortKey) => void;
}) {
  const label = sortKey === "rating" && tab === "maps" ? "地图评分" : sortLabels[sortKey];
  const active = activeSort.key === sortKey;
  const currentDirection = activeSort.direction === "desc" ? "降序" : "升序";
  const nextDirection = activeSort.direction === "desc" ? "升序" : "降序";
  return (
    <span
      className="rankingSortHeader"
      data-sort-key={sortKey}
      role="columnheader"
      aria-sort={active ? activeSort.direction === "desc" ? "descending" : "ascending" : undefined}
    >
      <button
        type="button"
        className={active ? "active" : ""}
        onClick={() => onSort(sortKey)}
        aria-label={active ? `${label}，当前${currentDirection}，点击切换为${nextDirection}` : `${label}，点击排序`}
        title={`按${label}排序`}
      >
        {label}
        {active ? activeSort.direction === "desc" ? <ArrowDown size={14} aria-hidden="true" /> : <ArrowUp size={14} aria-hidden="true" /> : <ArrowUpDown size={14} aria-hidden="true" />}
      </button>
    </span>
  );
}

function BestHeroCell({ row }: { row: RankingRow }) {
  if (!row.bestHeroName) {
    return <span className="phase6RankBestHero" role="cell">-</span>;
  }
  return (
    <span className="phase6RankBestHero" role="cell">
      <HeroAvatar heroKey={row.bestHeroGuid || row.bestHeroName} name={row.bestHeroName} />
      <strong>{row.bestHeroName}</strong>
    </span>
  );
}

function formatKda(value: number) {
  return value.toFixed(2);
}

function formatRating(value: string | null) {
  return value ?? "-";
}
