import { AlertCircle, ArrowDown, ArrowUp, ArrowUpDown, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { getRankings } from "../shared/api";
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
const selectableSeasons = [23, 22, 21] as const;

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
  const [search, setSearch] = useState("");
  const [ranking, setRanking] = useState<RankingResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
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
        if (selectedSeason === undefined && nextRanking.season) {
          setSelectedSeason(nextRanking.season);
        }
      })
      .catch((err) => {
        if (rankingRequestGuard.current.isCurrent(request)) setError(productErrorMessage(err));
      })
      .finally(() => {
        if (rankingRequestGuard.current.isCurrent(request)) setLoading(false);
      });

    return () => rankingRequestGuard.current.invalidate(request);
  }, [selectedSeason]);

  const rows = useMemo(() => {
    const source = tab === "maps" ? ranking?.maps ?? [] : ranking?.heroes ?? [];
    const keyword = search.trim().toLowerCase();
    const filtered = keyword
      ? source.filter((row) => {
          const haystack = tab === "maps" ? `${row.name} ${row.bestHeroName ?? ""}` : row.name;
          return haystack.toLowerCase().includes(keyword);
        })
      : source;
    return sortRankingRows(filtered, sort);
  }, [ranking, search, sort, tab]);

  const sourceCount = tab === "maps" ? ranking?.maps.length ?? 0 : ranking?.heroes.length ?? 0;

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
      {error && (
        <div className="errorBanner phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      <div className="rankingToolbar phase6RankingToolbar">
        <div className="segmentedControl phase6SegmentedControl">
          <button type="button" className={tab === "heroes" ? "active" : ""} onClick={() => switchTab("heroes")}>
            英雄排行
          </button>
          <button type="button" className={tab === "maps" ? "active" : ""} onClick={() => switchTab("maps")}>
            地图排行
          </button>
        </div>
        <label className="searchBox phase6RankingSearch">
          <Search size={16} />
          <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder={tab === "maps" ? "搜索地图或英雄" : "搜索英雄"} />
        </label>
        <label className="seasonSelect phase8SeasonSelect">
          <span>赛季</span>
          <select value={selectedSeason ?? ""} onChange={(event) => changeSeason(Number(event.target.value))}>
            {selectableSeasons.map((season) => (
              <option key={season} value={season}>
                S{season}
              </option>
            ))}
          </select>
        </label>
        <span className="phase6RankingCount">
          {selectedSeason ? `S${selectedSeason}` : "本地赛季"} · {sourceCount} 项 · 当前 {rows.length} 项
        </span>
      </div>

      {loading && <div className="emptyState">正在计算排行...</div>}
      {!loading && rows.length === 0 && <div className="emptyState">本地赛季数据不足，暂无排行。</div>}
      {!loading && rows.length > 0 && <RankingTable rows={rows} tab={tab} sort={sort} onSort={changeSort} />}
    </section>
  );
}

export function parseSeasonParam(value: string | null) {
  if (!value) return undefined;
  const season = Number(value);
  return selectableSeasons.includes(season as (typeof selectableSeasons)[number]) ? season : undefined;
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
    <div className={`rankingTable phase6RankingTable ${tab}`}>
      <div className="rankingHead phase6RankingHead">
        <span>{tab === "maps" ? "地图" : "英雄"}</span>
        {tab === "maps" && <span>最佳英雄</span>}
        {sortKeys.map((key) => (
          <SortButton key={key} sortKey={key} activeSort={sort} tab={tab} onSort={onSort} />
        ))}
        <span>{tab === "maps" ? "地图评分" : "英雄评分"}</span>
      </div>
      {rows.map((row, index) => (
        <div className="rankingLine phase6RankingLine" key={`${tab}-${row.key}`}>
          <span className="phase6RankName">
            <b>{index + 1}</b>
            {tab === "maps" ? (
              <MapPreview mapKey={row.key || row.name} name={row.name} className="phase6RankMapPreview" />
            ) : (
              <HeroAvatar heroKey={row.key || row.name} name={row.name} />
            )}
            <strong>{row.name}</strong>
          </span>
          {tab === "maps" && <BestHeroCell row={row} />}
          <span>{row.matches}</span>
          <span>{formatWinRate(row.winRate)}</span>
          <span>{formatPlayTimeHours(row.playTimeHours)}</span>
          <span>{formatKda(row.kda)}</span>
          <span>{formatRating(row.rating)}</span>
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
  return (
    <button
      type="button"
      className={activeSort.key === sortKey ? "active" : ""}
      onClick={() => onSort(sortKey)}
      title={`按${label}排序`}
    >
      {label}
      {activeSort.key === sortKey ? activeSort.direction === "desc" ? <ArrowDown size={14} /> : <ArrowUp size={14} /> : <ArrowUpDown size={14} />}
    </button>
  );
}

function BestHeroCell({ row }: { row: RankingRow }) {
  if (!row.bestHeroName) {
    return <span className="phase6RankBestHero">-</span>;
  }
  return (
    <span className="phase6RankBestHero">
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
