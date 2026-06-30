import { AlertCircle, Map, Users, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getSeasonAnalysis } from "../shared/api";
import { GlassPanel, HeroAvatar, MapPreview, MetricCard, StatusBadge } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import { formatWinRate } from "../shared/rankingUtils";
import type { FriendAnalytics, FriendBoard, FriendBoardEntry, MapAnalysisCard, SeasonAnalysis } from "../shared/types";

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
      .catch((err) => setError(productErrorMessage(err)))
      .finally(() => setLoading(false));
  }, []);

  const analytics = analysis?.friend ?? null;

  return (
    <>
      <section className="workspace analyticsWorkspace phase6AnalyticsWorkspace">
        <header className="phase6PageHeader">
          <div>
            <span className="phase6Eyebrow">分析模型</span>
            <h2>组排资源与地图洞察</h2>
            <p>查看高频队友、常用英雄组合和赛季地图表现，把组排经验变成可复盘的数据模型。</p>
          </div>
          <div className="phase6CommandStatus">
            <StatusBadge tone={loading ? "warn" : "good"}>
              <Users size={15} /> {loading ? "计算中" : "分析就绪"}
            </StatusBadge>
            <StatusBadge>
              <Map size={15} /> {analysis?.season ? `S${analysis.season}` : "本地赛季"}
            </StatusBadge>
          </div>
        </header>

        <section className="phase6MatchMetrics">
          <MetricCard label="预设 / 开放" value={String(analytics?.summary.presetMatches ?? 0)} meta="分析样本" />
          <MetricCard label="好友记录" value={String(analytics?.summary.friendRows ?? 0)} meta="同队记录数" />
          <MetricCard label="入榜好友" value={String(analytics?.summary.eligibleFriends ?? 0)} meta="达到样本门槛" />
          <MetricCard label="入榜门槛" value={`${analytics?.summary.minSample ?? 5} 场`} meta="避免小样本误导" />
        </section>

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
                <FriendBoardCard key={board.key} board={board} onOpen={setActiveFriend} />
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
      </section>

      {activeFriend && <FriendExplainDialog friend={activeFriend} onClose={() => setActiveFriend(null)} />}
    </>
  );
}

function FriendPhase5Summary({ analytics }: { analytics: FriendAnalytics }) {
  return (
    <div className="reviewGrid phase6ReviewGrid topGap">
      <GlassPanel className="phase6ReviewPanel" tone="solid">
        <header>
          <span className="phase6Eyebrow">好友常用英雄</span>
          <h3>组排识别</h3>
        </header>
        <div className="breakdownList">
          {analytics.commonFriendHeroes.map((friend) => (
            <div key={friend.bnetId}>
              <span className="phase6HeroCell">
                <HeroAvatar heroKey={friend.representativeHeroGuid || friend.representativeHeroName} name={friend.representativeHeroName} size="sm" />
                {friend.name} · {friend.representativeHeroName}
              </span>
              <strong>
                {friend.matches} 场 / {formatWinRate(friend.winRate, 0)}
              </strong>
            </div>
          ))}
          {analytics.commonFriendHeroes.length === 0 && <div className="emptyState compact">暂无 3 场以上组排好友。</div>}
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
  onOpen,
}: {
  board: FriendBoard;
  onOpen: (friend: FriendBoardEntry) => void;
}) {
  return (
    <GlassPanel className="friendBoard phase6FriendBoard" tone="solid">
      <header>
        <div>
          <h3>{board.label}</h3>
          <p>{board.description}</p>
        </div>
        <span>{board.friends.length}</span>
      </header>
      <div className="friendCardList">
        {board.friends.slice(0, 2).map((friend) => (
          <button type="button" className="friendCard phase6FriendCard" key={`${board.key}-${friend.bnetId}`} onClick={() => onOpen(friend)}>
            <strong>{friend.name}</strong>
            <span>{friend.boardLabel}</span>
            <MetricList friend={friend} compact />
          </button>
        ))}
        {board.friends.length === 0 && <div className="emptyState compact">暂无符合样本的好友。</div>}
      </div>
    </GlassPanel>
  );
}

function FriendExplainDialog({ friend, onClose }: { friend: FriendBoardEntry; onClose: () => void }) {
  return (
    <div className="modalOverlay" role="presentation" onClick={onClose}>
      <section className="friendModal" role="dialog" aria-modal="true" aria-label="好友解释数据" onClick={(event) => event.stopPropagation()}>
        <header>
          <div>
            <span className="phase6Eyebrow">好友详情</span>
            <h3>{friend.name}</h3>
          </div>
          <button type="button" className="iconOnlyButton" onClick={onClose} title="关闭">
            <X size={18} />
          </button>
        </header>

        <div className="friendModalBody">
          <GlassPanel className="phase6ReviewPanel" tone="solid">
            <h3>{friend.boardLabel}</h3>
            <p className="explainText">{friend.reasons.join(" ")}</p>
            <MetricList friend={friend} />
          </GlassPanel>
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
