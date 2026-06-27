import { Map, Users, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getSeasonAnalysis } from "../shared/api";
import type { FriendBoard, FriendBoardEntry, MapAnalysisCard, SeasonAnalysis } from "../shared/types";

export function FriendAnalyticsPage() {
  const [analysis, setAnalysis] = useState<SeasonAnalysis | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeFriend, setActiveFriend] = useState<FriendBoardEntry | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);
    void getSeasonAnalysis()
      .then(setAnalysis)
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, []);

  const analytics = analysis?.friend ?? null;

  return (
    <>
      <section className="controlBand">
        <div className="metric">
          <span>分析赛季</span>
          <strong>{analysis?.season ?? "全部"}</strong>
        </div>
        <div className="metric">
          <span>预设/开放</span>
          <strong>{analytics?.summary.presetMatches ?? 0}</strong>
        </div>
        <div className="metric">
          <span>好友记录</span>
          <strong>{analytics?.summary.friendRows ?? 0}</strong>
        </div>
        <div className="metric">
          <span>入榜门槛</span>
          <strong>{analytics?.summary.minSample ?? 5} 场</strong>
        </div>
      </section>

      {error && <div className="errorBanner">{error}</div>}

      <section className="workspace analyticsWorkspace">
        <header className="sectionHeader">
          <div>
            <span className="eyebrow">好友分析</span>
            <h2>好友资源与压力分析</h2>
          </div>
          <Users size={22} />
        </header>
        {loading && <div className="emptyState">正在计算赛季分析...</div>}
        {!loading && analytics && (
          <div className="boardGrid">
            {analytics.boards.map((board) => (
              <FriendBoardCard key={board.key} board={board} onOpen={setActiveFriend} />
            ))}
          </div>
        )}

        <header className="sectionHeader topGap">
          <div>
            <span className="eyebrow">地图分析</span>
            <h2>地图表现概览</h2>
          </div>
          <Map size={22} />
        </header>
        {analysis && <MapCardGrid analysis={analysis} />}
      </section>

      {activeFriend && <FriendExplainDialog friend={activeFriend} onClose={() => setActiveFriend(null)} />}
    </>
  );
}

function MapCardGrid({ analysis }: { analysis: SeasonAnalysis }) {
  const cards = [
    ["王牌地图", analysis.mapCards.aceMap],
    ["翻车路段", analysis.mapCards.throwZone],
    ["常驻战场", analysis.mapCards.homeField],
    ["稀有来客", analysis.mapCards.rareGuest]
  ] as const;
  return (
    <div className="mapAnalysisGrid">
      {cards.map(([title, card]) => (
        <MapAnalysisCardView key={title} title={title} card={card} minSample={analysis.mapCards.minSample} />
      ))}
    </div>
  );
}

function MapAnalysisCardView({
  title,
  card,
  minSample
}: {
  title: string;
  card: MapAnalysisCard | null;
  minSample: number;
}) {
  if (!card) {
    return (
      <section className="mapAnalysisCard">
        <h3>{title}</h3>
        <p className="explainText">样本不足，至少需要 {minSample} 场。</p>
      </section>
    );
  }
  return (
    <Link className="mapAnalysisCard" to="/ranking?tab=maps">
      <h3>{title}</h3>
      <strong>{card.name}</strong>
      <span>{(card.winRate * 100).toFixed(1)}%</span>
      <small>{card.matches} 场</small>
    </Link>
  );
}

function FriendBoardCard({
  board,
  onOpen
}: {
  board: FriendBoard;
  onOpen: (friend: FriendBoardEntry) => void;
}) {
  return (
    <section className="friendBoard">
      <header>
        <div>
          <h3>{board.label}</h3>
          <p>{board.description}</p>
        </div>
        <span>{board.friends.length}</span>
      </header>
      <div className="friendCardList">
        {board.friends.slice(0, 3).map((friend) => (
          <button
            type="button"
            className="friendCard"
            key={`${board.key}-${friend.bnetId}`}
            onClick={() => onOpen(friend)}
          >
            <strong>{friend.name}</strong>
            <span>{friend.boardLabel}</span>
            <MetricList friend={friend} compact />
          </button>
        ))}
        {board.friends.length === 0 && <div className="emptyState compact">暂无符合样本的好友。</div>}
      </div>
    </section>
  );
}

function FriendExplainDialog({ friend, onClose }: { friend: FriendBoardEntry; onClose: () => void }) {
  return (
    <div className="modalOverlay" role="presentation" onClick={onClose}>
      <section
        className="friendModal"
        role="dialog"
        aria-modal="true"
        aria-label="好友解释数据"
        onClick={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <span className="eyebrow">好友详情</span>
            <h3>{friend.name}</h3>
          </div>
          <button type="button" className="iconOnlyButton" onClick={onClose} title="关闭">
            <X size={18} />
          </button>
        </header>

        <div className="friendModalBody">
          <section className="playerSection">
            <h3>{friend.boardLabel}</h3>
            <p className="explainText">{friend.reasons.join(" ")}</p>
            <MetricList friend={friend} />
          </section>
        </div>
      </section>
    </div>
  );
}

function MetricList({ friend, compact = false }: { friend: FriendBoardEntry; compact?: boolean }) {
  return (
    <dl className={compact ? "explainMetrics compact" : "explainMetrics"}>
      {explanationMetrics(friend).map((metric) => (
        <div key={metric.label}>
          <dt>{metric.label}</dt>
          <dd>{metric.value}</dd>
        </div>
      ))}
    </dl>
  );
}

function explanationMetrics(friend: FriendBoardEntry) {
  const base = [{ label: "同队", value: `${friend.sameTeamMatches} 场` }];
  const winRate = { label: "胜率", value: `${Math.round(friend.winRate * 100)}%` };
  const damageTaken = { label: "承伤", value: `${Math.round(friend.avgDamageTaken)}` };
  const healingTaken = { label: "受疗", value: `${Math.round(friend.avgHealingTaken)}` };
  const mitigation = { label: "格挡", value: `${Math.round(friend.avgMitigation)}` };
  const deaths = { label: "死亡", value: friend.avgDeaths.toFixed(1) };
  const kills = { label: "击杀", value: friend.avgKills.toFixed(1) };

  switch (friend.boardKey) {
    case "front_pressure":
      return [...base, damageTaken, mitigation];
    case "low_resource_partner":
      return [...base, healingTaken, deaths];
    case "priority_care":
      return [...base, healingTaken, deaths];
    case "healing_conversion":
      return [...base, healingTaken, kills, winRate];
    default:
      return [...base, damageTaken, mitigation, healingTaken, deaths];
  }
}
