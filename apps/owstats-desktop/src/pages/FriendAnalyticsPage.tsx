import { AlertCircle, Map } from "lucide-react";
import { Link, Navigate } from "react-router-dom";
import { GlassPanel, HeroAvatar, MapPreview } from "../shared/phase6Components";
import { formatWinRate } from "../shared/rankingUtils";
import type { FriendAnalytics, FriendBoard, FriendBoardEntry, MapAnalysisCard, SeasonAnalysis } from "../shared/types";

export function FriendAnalyticsPage() {
  return <Navigate to="/review" replace />;
}

export function SeasonAnalysisModules({
  analysis,
  analytics,
  loading,
  error,
}: {
  analysis: SeasonAnalysis | null;
  analytics: FriendAnalytics | null;
  loading: boolean;
  error: string | null;
}) {
  return (
    <>
      {error && (
        <div className="errorBanner phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}

      {loading && <div className="emptyState">正在计算赛季分析...</div>}
      {!loading && analytics && (
        <>
          <FriendPhase5Summary analytics={analytics} />
          <div className="boardGrid topGap phase6FriendBoardGrid">
            {analytics.boards.map((board) => (
              <FriendBoardCard key={board.key} board={board} />
            ))}
          </div>
        </>
      )}

      <header className="sectionHeader topGap phase6InlineHeader">
        <div>
          <span className="phase6Eyebrow">地图分析</span>
          <h2>地图表现概览</h2>
        </div>
        <Map size={22} />
      </header>
      {analysis && <MapCardGrid analysis={analysis} />}
    </>
  );
}

function FriendPhase5Summary({ analytics }: { analytics: FriendAnalytics }) {
  const topFriends = analytics.commonFriendHeroes.slice(0, 5);
  const maxIntimacyScore = Math.max(...topFriends.map((friend) => friend.intimacyScore), 1);

  return (
    <div className="reviewGrid phase6ReviewGrid topGap">
      <GlassPanel className="phase6ReviewPanel phase6FriendIntimacyPanel" tone="solid">
        <header>
          <h3>好友亲密度</h3>
        </header>
        <div className="phase6FriendIntimacyList">
          {topFriends.map((friend, index) => {
            const barWidth = Math.max(8, Math.round((friend.intimacyScore / maxIntimacyScore) * 100));
            return (
              <article className="phase6FriendIntimacyCard" key={friend.bnetId}>
                <span className="phase6FriendIntimacyRankBadge">#{index + 1}</span>
                <HeroAvatar heroKey={friend.representativeHeroGuid || friend.representativeHeroName} name={friend.representativeHeroName} size="md" />
                <div className="phase6FriendIntimacyMain">
                  <strong>{friend.name}</strong>
                  <div className="phase6FriendIntimacyBar" aria-label={`${friend.name} 亲密度 ${friend.intimacyScore}`}>
                    <span style={{ width: `${barWidth}%` }} />
                  </div>
                </div>
                <dl className="phase6FriendIntimacyMetrics">
                  <div>
                    <dt>亲密度</dt>
                    <dd>{friend.intimacyScore}</dd>
                  </div>
                  <div>
                    <dt>同队</dt>
                    <dd>{friend.matches} 场</dd>
                  </div>
                  <div>
                    <dt>胜率</dt>
                    <dd>{formatWinRate(friend.winRate, 0)}</dd>
                  </div>
                </dl>
              </article>
            );
          })}
          {topFriends.length === 0 && <div className="emptyState compact">暂无 3 场以上组排好友。</div>}
        </div>
      </GlassPanel>

      {analytics.lineupCombinations.length > 0 && (
        <GlassPanel className="phase6ReviewPanel" tone="solid">
          <header>
            <span className="phase6Eyebrow">阵容搭配</span>
            <h3>高频组合</h3>
          </header>
          <div className="breakdownList phase6LineupList">
            {analytics.lineupCombinations.slice(0, 5).map((combo) => (
              <div key={combo.key}>
                <span>
                  {combo.members.map((member) => (
                    <span className="phase6LineupMember" key={`${combo.key}-${member.name}-${member.heroName}`}>
                      <HeroAvatar heroKey={member.heroGuid || member.heroName} name={member.heroName} size="sm" />
                      {member.name}: {member.heroName}
                    </span>
                  ))}
                </span>
                <strong>
                  {combo.matches} 场 / {formatWinRate(combo.winRate, 0)}
                </strong>
              </div>
            ))}
          </div>
        </GlassPanel>
      )}
    </div>
  );
}

function MapCardGrid({ analysis }: { analysis: SeasonAnalysis }) {
  const cards = [
    ["王牌地图", analysis.mapCards.aceMap],
    ["翻车路段", analysis.mapCards.throwZone],
    ["常驻战场", analysis.mapCards.homeField],
    ["稀有来客", analysis.mapCards.rareGuest],
  ] as const;
  return (
    <div className="mapAnalysisGrid phase6MapAnalysisGrid">
      {cards.map(([title, card]) => (
        <MapAnalysisCardView key={title} title={title} card={card} minSample={analysis.mapCards.minSample} />
      ))}
    </div>
  );
}

function MapAnalysisCardView({
  title,
  card,
  minSample,
}: {
  title: string;
  card: MapAnalysisCard | null;
  minSample: number;
}) {
  if (!card) {
    return (
      <section className="mapAnalysisCard phase6MapAnalysisCard">
        <h3>{title}</h3>
        <p className="explainText">样本不足，至少需要 {minSample} 场。</p>
      </section>
    );
  }
  return (
    <Link className="mapAnalysisCard phase6MapAnalysisCard" to="/ranking?tab=maps">
      <MapPreview mapKey={card.key || card.name} name={card.name} className="phase6AnalysisMapPreview" />
      <div>
        <h3>{title}</h3>
        <strong>{card.name}</strong>
        <span>{formatWinRate(card.winRate)}</span>
        <small>{card.matches} 场</small>
      </div>
    </Link>
  );
}

function FriendBoardCard({
  board,
}: {
  board: FriendBoard;
}) {
  return (
    <GlassPanel className="friendBoard phase6FriendBoard" tone="solid">
      <header>
        <div>
          <h3>{board.label}</h3>
          <p>{board.description}</p>
        </div>
      </header>
      <div className="friendCardList">
        {board.friends.slice(0, 2).map((friend) => (
          <article className="friendCard phase6FriendCard" key={`${board.key}-${friend.bnetId}`}>
            <strong>{friend.name}</strong>
            <span>{friend.boardLabel}</span>
            <MetricList friend={friend} compact />
          </article>
        ))}
        {board.friends.length === 0 && <div className="emptyState compact">暂无符合样本的好友。</div>}
      </div>
    </GlassPanel>
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

export function explanationMetrics(friend: FriendBoardEntry) {
  const base = [{ label: "同队", value: `${friend.sameTeamMatches} 场` }];
  const winRate = { label: "胜率", value: formatWinRate(friend.winRate, 0) };
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
      return [...base, healingTaken, kills];
    default:
      return [...base, damageTaken, mitigation, healingTaken, deaths];
  }
}
