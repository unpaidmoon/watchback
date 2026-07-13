import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { parseFriendDetailState } from "../features/review/reviewNavigation";
import { getFriendDetails } from "../shared/api";
import { PageEmptyState, PageErrorState, PageLoadingState } from "../shared/page-shell";
import { GlassPanel, HeroAvatar, MapPreview } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import { formatWinRate } from "../shared/rankingUtils";
import type { FriendBreakdown, FriendDetail, FriendRecentMatch } from "../shared/types";

export function FriendDetailPage() {
  const location = useLocation();
  const navigate = useNavigate();
  const state = parseFriendDetailState(location.state);
  const [detail, setDetail] = useState<FriendDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function requestDetail() {
    if (!state) return;
    setLoading(true);
    setError(null);
    void getFriendDetails(state.bnetId, { season: state.season })
      .then(setDetail)
      .catch((err) => setError(productErrorMessage(err)))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    requestDetail();
  }, [state?.bnetId, state?.season]);

  function returnToReview() {
    if (!state) {
      navigate("/review");
      return;
    }
    sessionStorage.setItem(state.returnTo.scrollKey, String(state.returnTo.scrollY));
    navigate(state.returnTo.href);
  }

  if (!state) {
    return (
      <section className="detailPage phase6DetailPage">
        <PageEmptyState
          title="无法打开好友详情"
          description="详情入口已失效，请从战报的队友协作页重新进入。"
          actionLabel="返回战报"
          onAction={() => navigate("/review")}
        />
      </section>
    );
  }

  return (
    <section className="detailPage phase6DetailPage">
      <header className="detailHero phase6DetailHero">
        <button type="button" className="ghostButton" onClick={returnToReview}>
          <ArrowLeft size={18} />
          返回战报
        </button>
        <div>
          <span className="phase6Eyebrow">好友详情</span>
          <h2>{detail?.name ?? "好友详情"}</h2>
        </div>
      </header>

      {error && <PageErrorState message={error} onRetry={requestDetail} />}
      {loading && <PageLoadingState message="正在读取好友详情..." />}
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
