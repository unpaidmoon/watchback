import { AlertCircle, ArrowDown, ArrowUp, ArrowUpDown, Map, Search, Trophy } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { getRankings } from "../shared/api";
import { HeroAvatar, MapPreview, MetricCard, StatusBadge } from "../shared/phase6Components";
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

const sortLabels: Record<RankingSortKey, string> = {
  matches: "场数",
  wins: "胜场",
  losses: "负场",
  winRate: "胜率",
  playTimeHours: "时长",
};

export function RankingPage() {
  const [params, setParams] = useSearchParams();
  const [tab, setTab] = useState<RankingTab>(params.get("tab") === "maps" ? "maps" : "heroes");
  const [sort, setSort] = useState<RankingSort>(() => defaultRankingSort(tab));
  const [search, setSearch] = useState("");
  const [ranking, setRanking] = useState<RankingResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const nextTab = params.get("tab") === "maps" ? "maps" : "heroes";
    setTab(nextTab);
    setSort(defaultRankingSort(nextTab));
  }, [params]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    void getRankings()
      .then(setRanking)
      .catch((err) => setError(productErrorMessage(err)))
      .finally(() => setLoading(false));
  }, []);

  const rows = useMemo(() => {
    const source = tab === "maps" ? ranking?.maps ?? [] : ranking?.heroes ?? [];
    const keyword = search.trim().toLowerCase();
    const filtered = keyword ? source.filter((row) => row.name.toLowerCase().includes(keyword)) : source;
    return sortRankingRows(filtered, sort);
  }, [ranking, search, sort, tab]);

  const sourceCount = tab === "maps" ? ranking?.maps.length ?? 0 : ranking?.heroes.length ?? 0;
  const topRow = rows[0] ?? null;

  function switchTab(next: RankingTab) {
    setTab(next);
    setSort(defaultRankingSort(next));
    setParams(next === "maps" ? { tab: "maps" } : {});
  }

  function changeSort(key: RankingSortKey) {
    setSort((current) => toggleRankingSort(current, key));
  }

  return (
    <section className="workspace rankingWorkspace phase6RankingWorkspace">
      <header className="phase6PageHeader phase6RankingHeader">
        <div>
          <span className="phase6Eyebrow">本地赛季排行</span>
          <h2>{tab === "maps" ? "地图表现排行" : "英雄表现排行"}</h2>
          <p>基于本地赛季数据聚合胜率、场数、胜负和游玩时长，用于定位稳定优势和薄弱地图。</p>
        </div>
        <div className="phase6CommandStatus">
          <StatusBadge tone={loading ? "warn" : "good"}>
            <Trophy size={15} /> {loading ? "计算中" : "排行就绪"}
          </StatusBadge>
          <StatusBadge>
            <Map size={15} /> {ranking?.season ? `S${ranking.season}` : "本地赛季"}
          </StatusBadge>
        </div>
      </header>

      <section className="phase6MatchMetrics">
        <MetricCard label={tab === "maps" ? "地图数量" : "英雄数量"} value={String(sourceCount)} meta="当前赛季样本" />
        <MetricCard label="当前列表" value={String(rows.length)} meta={search ? "搜索结果" : "完整排行"} />
        <MetricCard label="榜首胜率" value={topRow ? formatWinRate(topRow.winRate) : "0.0%"} meta={topRow?.name ?? "暂无样本"} />
        <MetricCard label="榜首场数" value={String(topRow?.matches ?? 0)} meta={topRow ? formatPlayTimeHours(topRow.playTimeHours) : "暂无时长"} />
      </section>

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
          <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索名称" />
        </label>
      </div>

      {loading && <div className="emptyState">正在计算排行...</div>}
      {!loading && rows.length === 0 && <div className="emptyState">本地赛季数据不足，暂无排行。</div>}
      {!loading && rows.length > 0 && <RankingTable rows={rows} tab={tab} sort={sort} onSort={changeSort} />}
    </section>
  );
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
  return (
    <div className="rankingTable phase6RankingTable">
      <div className="rankingHead phase6RankingHead">
        <span>{tab === "maps" ? "地图" : "英雄"}</span>
        {(Object.keys(sortLabels) as RankingSortKey[]).map((key) => (
          <button
            type="button"
            className={sort.key === key ? "active" : ""}
            key={key}
            onClick={() => onSort(key)}
            title={`按${sortLabels[key]}排序`}
          >
            {sortLabels[key]}
            {sort.key === key ? sort.direction === "desc" ? <ArrowDown size={14} /> : <ArrowUp size={14} /> : <ArrowUpDown size={14} />}
          </button>
        ))}
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
          <span>{row.matches}</span>
          <span>{row.wins}</span>
          <span>{row.losses}</span>
          <span>{formatWinRate(row.winRate)}</span>
          <span>{formatPlayTimeHours(row.playTimeHours)}</span>
          {tab === "maps" && row.adaptabilityLabel && <em>{row.adaptabilityLabel}</em>}
        </div>
      ))}
    </div>
  );
}
