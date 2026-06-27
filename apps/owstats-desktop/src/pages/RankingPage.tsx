import { Search, Trophy } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { getRankings } from "../shared/api";
import type { RankingResponse, RankingRow } from "../shared/types";

type RankingTab = "heroes" | "maps";

export function RankingPage() {
  const [params, setParams] = useSearchParams();
  const [tab, setTab] = useState<RankingTab>(params.get("tab") === "maps" ? "maps" : "heroes");
  const [search, setSearch] = useState("");
  const [ranking, setRanking] = useState<RankingResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setTab(params.get("tab") === "maps" ? "maps" : "heroes");
  }, [params]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    void getRankings()
      .then(setRanking)
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, []);

  const rows = useMemo(() => {
    const source = tab === "maps" ? ranking?.maps ?? [] : ranking?.heroes ?? [];
    const keyword = search.trim().toLowerCase();
    if (!keyword) return source;
    return source.filter((row) => row.name.toLowerCase().includes(keyword));
  }, [ranking, search, tab]);

  function switchTab(next: RankingTab) {
    setTab(next);
    setParams(next === "maps" ? { tab: "maps" } : {});
  }

  return (
    <section className="workspace rankingWorkspace">
      <header className="sectionHeader">
        <div>
          <span className="eyebrow">本地赛季排行</span>
          <h2>{tab === "maps" ? "地图胜率" : "英雄胜率"}</h2>
        </div>
        <Trophy size={22} />
      </header>

      {error && <div className="errorBanner inline">{error}</div>}

      <div className="rankingToolbar">
        <div className="segmentedControl">
          <button type="button" className={tab === "heroes" ? "active" : ""} onClick={() => switchTab("heroes")}>
            英雄胜率
          </button>
          <button type="button" className={tab === "maps" ? "active" : ""} onClick={() => switchTab("maps")}>
            地图胜率
          </button>
        </div>
        <label className="searchBox">
          <Search size={16} />
          <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索名称" />
        </label>
      </div>

      {loading && <div className="emptyState">正在计算排行...</div>}
      {!loading && rows.length === 0 && <div className="emptyState">本地赛季数据不足，暂无排行。</div>}
      {!loading && rows.length > 0 && <RankingTable rows={rows} tab={tab} />}
    </section>
  );
}

function RankingTable({ rows, tab }: { rows: RankingRow[]; tab: RankingTab }) {
  return (
    <div className="rankingTable">
      <div className="rankingHead">
        <span>{tab === "maps" ? "地图名" : "英雄名"}</span>
        <span>场数</span>
        <span>胜场</span>
        <span>负场</span>
        <span>胜率</span>
        <span>游玩时长</span>
      </div>
      {rows.map((row) => (
        <div className="rankingLine" key={`${tab}-${row.key}`}>
          <strong>{row.name}</strong>
          <span>{row.matches}</span>
          <span>{row.wins}</span>
          <span>{row.losses}</span>
          <span>{(row.winRate * 100).toFixed(1)}%</span>
          <span>{row.playTimeHours.toFixed(1)} 小时</span>
          {tab === "maps" && row.adaptabilityLabel && <em>{row.adaptabilityLabel}</em>}
        </div>
      ))}
    </div>
  );
}
