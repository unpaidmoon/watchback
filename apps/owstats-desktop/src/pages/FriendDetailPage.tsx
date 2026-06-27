import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getFriendDetails } from "../shared/api";
import type { FriendBreakdown, FriendDetail, FriendRecentMatch } from "../shared/types";

export function FriendDetailPage() {
  const { bnetId } = useParams();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<FriendDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!bnetId) return;
    setLoading(true);
    setError(null);
    void getFriendDetails(decodeURIComponent(bnetId), {})
      .then(setDetail)
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, [bnetId]);

  return (
    <section className="detailPage">
      <header className="detailHero">
        <button type="button" className="ghostButton" onClick={() => navigate("/analytics/friends")}>
          <ArrowLeft size={18} />
          返回分析
        </button>
        <div>
          <span className="eyebrow">好友</span>
          <h2>{detail?.name ?? bnetId}</h2>
        </div>
      </header>

      {error && <div className="errorBanner">{error}</div>}
      {loading && <div className="emptyState">正在读取好友详情...</div>}
      {detail && (
        <div className="detailContent friendDetailContent">
          <section className="playerSection">
            <h3>命中的榜单</h3>
            <div className="boardHits">
              {detail.boards.map((board) => (
                <article key={board.boardKey}>
                  <strong>{board.boardLabel}</strong>
                  <p>{board.reasons.join(" ")}</p>
                  <span>
                    {board.sameTeamMatches} 场 · 胜率 {Math.round(board.winRate * 100)}% · 承伤{" "}
                    {Math.round(board.avgDamageTaken)} · 格挡 {Math.round(board.avgMitigation)} · 受疗{" "}
                    {Math.round(board.avgHealingTaken)} · 死亡{" "}
                    {board.avgDeaths.toFixed(1)}
                  </span>
                </article>
              ))}
            </div>
          </section>

          <div className="playerColumns">
            <BreakdownPanel title="常用英雄" items={detail.commonHeroes} />
            <BreakdownPanel title="常见地图" items={detail.commonMaps} />
          </div>

          <section className="playerSection">
            <h3>最近同场比赛</h3>
            <div className="recentMatchList">
              {detail.recentMatches.map((match) => (
                <RecentMatchRow key={match.matchId} match={match} />
              ))}
            </div>
          </section>
        </div>
      )}
    </section>
  );
}

function BreakdownPanel({ title, items }: { title: string; items: FriendBreakdown[] }) {
  return (
    <section className="playerSection">
      <h3>{title}</h3>
      <div className="breakdownList">
        {items.map((item) => (
          <div key={item.label}>
            <span>{item.label}</span>
            <strong>{item.count}</strong>
          </div>
        ))}
      </div>
    </section>
  );
}

function RecentMatchRow({ match }: { match: FriendRecentMatch }) {
  return (
    <div className="recentMatchRow">
      <span>{match.beginLabel}</span>
      <strong>{match.mapName}</strong>
      <span>{match.heroName}</span>
      <span>{match.result === "win" ? "胜利" : match.result === "loss" ? "失败" : "平局"}</span>
      <span>
        {match.kills}/{match.assists}/{match.deaths}
      </span>
      <span>
        {match.damage}/{match.healing}
      </span>
      <span>
        {match.damageTaken}/{match.healingTaken}
      </span>
    </div>
  );
}
