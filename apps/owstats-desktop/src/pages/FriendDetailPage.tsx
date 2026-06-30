import { AlertCircle, ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getFriendDetails } from "../shared/api";
import { GlassPanel, HeroAvatar, MapPreview } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import { formatWinRate } from "../shared/rankingUtils";
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
      .catch((err) => setError(productErrorMessage(err)))
      .finally(() => setLoading(false));
  }, [bnetId]);

  return (
    <section className="detailPage phase6DetailPage">
      <header className="detailHero phase6DetailHero">
        <button type="button" className="ghostButton" onClick={() => navigate("/analytics/friends")}>
          <ArrowLeft size={18} />
          返回分析
        </button>
        <div>
          <span className="phase6Eyebrow">好友详情</span>
          <h2>{detail?.name ?? bnetId}</h2>
        </div>
      </header>

      {error && (
        <div className="errorBanner phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}
      {loading && <div className="emptyState">正在读取好友详情...</div>}
      {detail && (
        <div className="detailContent friendDetailContent">
          <GlassPanel className="phase6ReviewPanel" tone="solid">
            <header>
              <span className="phase6Eyebrow">命中的榜单</span>
              <h3>队友关系说明</h3>
            </header>
            <div className="boardHits">
              {detail.boards.map((board) => (
                <article key={board.boardKey}>
                  <strong>{board.boardLabel}</strong>
                  <p>{board.reasons.join(" ")}</p>
                  <span>
                    {board.sameTeamMatches} 场 · 胜率 {formatWinRate(board.winRate, 0)} · 承伤 {Math.round(board.avgDamageTaken)} · 格挡{" "}
                    {Math.round(board.avgMitigation)} · 受疗 {Math.round(board.avgHealingTaken)} · 死亡 {board.avgDeaths.toFixed(1)}
                  </span>
                </article>
              ))}
            </div>
          </GlassPanel>

          <div className="playerColumns">
            <BreakdownPanel title="常用英雄" items={detail.commonHeroes} kind="hero" />
            <BreakdownPanel title="常见地图" items={detail.commonMaps} kind="map" />
          </div>

          <GlassPanel className="phase6ReviewPanel" tone="solid">
            <header>
              <span className="phase6Eyebrow">最近同场比赛</span>
              <h3>共同出战记录</h3>
            </header>
            <div className="recentMatchList">
              {detail.recentMatches.map((match) => (
                <RecentMatchRow key={match.matchId} match={match} />
              ))}
            </div>
          </GlassPanel>
        </div>
      )}
    </section>
  );
}

function BreakdownPanel({ title, items, kind }: { title: string; items: FriendBreakdown[]; kind: "hero" | "map" }) {
  return (
    <GlassPanel className="phase6ReviewPanel" tone="solid">
      <h3>{title}</h3>
      <div className="breakdownList">
        {items.map((item) => (
          <div key={item.label}>
            <span className="phase6HeroCell">
              {kind === "hero" ? (
                <HeroAvatar heroKey={item.label} name={item.label} size="sm" />
              ) : (
                <MapPreview mapKey={item.label} name={item.label} className="phase6MiniMapPreview" />
              )}
              {item.label}
            </span>
            <strong>{item.count}</strong>
          </div>
        ))}
      </div>
    </GlassPanel>
  );
}

function RecentMatchRow({ match }: { match: FriendRecentMatch }) {
  return (
    <div className="recentMatchRow phase6RecentMatchRow">
      <span>{match.beginLabel}</span>
      <span className="phase6HeroCell">
        <MapPreview mapKey={match.mapName} name={match.mapName} className="phase6MiniMapPreview" />
        <strong>{match.mapName}</strong>
      </span>
      <span className="phase6HeroCell">
        <HeroAvatar heroKey={match.heroName} name={match.heroName} size="sm" />
        {match.heroName}
      </span>
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
