import { AlertCircle, Map } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import {
  buildFriendDetailState,
  buildReviewRankingHref,
} from "../features/review/reviewNavigation";
import { getSeasonAnalysis } from "../shared/api";
import { GlassPanel, HeroAvatar, MapPreview } from "../shared/phase6Components";
import { productErrorMessage } from "../shared/phase6Shell";
import { formatWinRate } from "../shared/rankingUtils";
import type { FriendAnalytics, FriendBoard, FriendBoardEntry, MapAnalysisCard, SeasonAnalysis } from "../shared/types";

const selectableSeasons = [23, 22, 21] as const;

export function createLatestFriendAnalysisRequestGuard() {
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

export function parseFriendAnalysisSeasonParam(value: string | null) {
  if (!value) return undefined;
  const season = Number(value);
  return selectableSeasons.includes(season as (typeof selectableSeasons)[number]) ? season : undefined;
}

export function normalizeFriendAnalysisRequestedSeason(value: string | null) {
  const selectedSeason = parseFriendAnalysisSeasonParam(value);
  return {
    selectedSeason,
    shouldReplaceUrl: value !== null && selectedSeason === undefined,
  };
}

export function hasFriendAnalysisSeasonData(analysis: Pick<SeasonAnalysis, "season">): analysis is { season: number } {
  return typeof analysis.season === "number" && analysis.season > 0;
}

export function FriendAnalysisEmptyState() {
  return (
    <section className="workspace analyticsWorkspace">
      <GlassPanel className="phase6RoleReport" tone="solid">
        <h2>暂无可用好友分析</h2>
        <p>当前本地数据库没有可用于好友分析的赛季数据。</p>
      </GlassPanel>
    </section>
  );
}

export function FriendAnalyticsPage() {
  const [params, setParams] = useSearchParams();
  const seasonParam = params.get("season");
  const { selectedSeason: requestedSeason, shouldReplaceUrl } = normalizeFriendAnalysisRequestedSeason(seasonParam);
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>(requestedSeason);
  const [analysis, setAnalysis] = useState<SeasonAnalysis | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const analysisRequestGuard = useRef(createLatestFriendAnalysisRequestGuard());

  useEffect(() => {
    if (!shouldReplaceUrl) return;
    const nextParams = new URLSearchParams(params);
    nextParams.delete("season");
    setParams(nextParams, { replace: true });
  }, [shouldReplaceUrl, params, setParams]);

  useEffect(() => {
    analysisRequestGuard.current.invalidate();
    setSelectedSeason(requestedSeason);
    setAnalysis(null);
    setError(null);
    setLoading(false);
  }, [requestedSeason]);

  useEffect(() => {
    const request = analysisRequestGuard.current.begin();
    setAnalysis(null);
    setLoading(true);
    setError(null);
    void getSeasonAnalysis(selectedSeason)
      .then((nextAnalysis) => {
        if (!analysisRequestGuard.current.isCurrent(request)) return;
        setAnalysis(nextAnalysis);
        if (selectedSeason === undefined && hasFriendAnalysisSeasonData(nextAnalysis)) {
          setSelectedSeason(nextAnalysis.season);
        }
      })
      .catch((err) => {
        if (analysisRequestGuard.current.isCurrent(request)) setError(productErrorMessage(err));
      })
      .finally(() => {
        if (analysisRequestGuard.current.isCurrent(request)) setLoading(false);
      });

    return () => analysisRequestGuard.current.invalidate(request);
  }, [selectedSeason]);

  function changeSeason(nextSeason: number | undefined) {
    setSelectedSeason(nextSeason);
    const nextParams = new URLSearchParams(params);
    if (nextSeason === undefined) {
      nextParams.delete("season");
    } else {
      nextParams.set("season", String(nextSeason));
    }
    setParams(nextParams);
  }

  const analytics = analysis?.friend ?? null;

  if (!loading && analysis && !hasFriendAnalysisSeasonData(analysis)) return <FriendAnalysisEmptyState />;

  return (
    <section className="workspace analyticsWorkspace">
      <header className="sectionHeader phase6InlineHeader">
        <div>
          <span className="phase6Eyebrow">好友分析</span>
          <h2>{selectedSeason ? `S${selectedSeason} 好友分析` : "最新赛季好友分析"}</h2>
        </div>
        <label className="seasonSelect phase8SeasonSelect">
          <span>赛季</span>
          <select value={selectedSeason ?? ""} onChange={(event) => changeSeason(event.target.value ? Number(event.target.value) : undefined)}>
            <option value="">最新赛季</option>
            {selectableSeasons.map((season) => (
              <option key={season} value={season}>
                S{season}
              </option>
            ))}
          </select>
        </label>
      </header>
      <SeasonAnalysisModules
        analysis={analysis}
        analytics={analytics}
        loading={loading}
        error={error}
        season={selectedSeason ?? analysis?.season ?? undefined}
      />
    </section>
  );
}

export function SeasonAnalysisModules({
  analysis,
  analytics,
  loading,
  error,
  season,
}: {
  analysis: SeasonAnalysis | null;
  analytics: FriendAnalytics | null;
  loading: boolean;
  error: string | null;
  season?: number;
}) {
  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }

  if (loading) return <div className="emptyState">正在加载好友分析...</div>;

  const resolvedSeason = season ?? analysis?.season;
  if (!resolvedSeason) return <div className="emptyState compact">暂无可用好友分析数据。</div>;

  return (
    <>
      <FriendAnalysisModules analytics={analytics} loading={false} error={null} season={resolvedSeason} />
      <MapAnalysisModules analysis={analysis} loading={false} error={null} season={resolvedSeason} />
    </>
  );
}

export function FriendAnalysisModules({
  analytics,
  loading,
  error,
  season,
}: {
  analytics: FriendAnalytics | null;
  loading: boolean;
  error: string | null;
  season: number;
}) {
  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }

  if (loading) return <div className="emptyState">正在加载队友协作分析...</div>;
  if (!analytics) return <div className="emptyState compact">暂无可用队友协作数据。</div>;

  return (
    <>
      <FriendPhase5Summary analytics={analytics} />
      <div className="boardGrid topGap phase6FriendBoardGrid">
        {analytics.boards.map((board) => (
          <FriendBoardCard key={board.key} board={board} season={season} />
        ))}
      </div>
    </>
  );
}

export function MapAnalysisModules({
  analysis,
  loading,
  error,
  season,
}: {
  analysis: SeasonAnalysis | null;
  loading: boolean;
  error: string | null;
  season: number;
}) {
  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }

  if (loading) return <div className="emptyState">正在加载地图分析...</div>;

  return (
    <>
      <header className="sectionHeader phase6InlineHeader">
        <div>
          <span className="phase6Eyebrow">地图分析</span>
          <h2>地图表现概览</h2>
        </div>
        <Map size={22} />
      </header>
      {analysis ? <MapCardGrid analysis={analysis} season={season} /> : <div className="emptyState compact">暂无可用地图数据。</div>}
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

function MapCardGrid({ analysis, season }: { analysis: SeasonAnalysis; season: number }) {
  const cards = [
    ["王牌地图", analysis.mapCards.aceMap],
    ["翻车路段", analysis.mapCards.throwZone],
    ["常驻战场", analysis.mapCards.homeField],
    ["稀有来客", analysis.mapCards.rareGuest],
  ] as const;
  return (
    <div className="mapAnalysisGrid phase6MapAnalysisGrid">
      {cards.map(([title, card]) => (
        <MapAnalysisCardView key={title} title={title} card={card} minSample={analysis.mapCards.minSample} season={season} />
      ))}
    </div>
  );
}

function MapAnalysisCardView({
  title,
  card,
  minSample,
  season,
}: {
  title: string;
  card: MapAnalysisCard | null;
  minSample: number;
  season: number;
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
    <Link className="mapAnalysisCard phase6MapAnalysisCard" to={buildReviewRankingHref(season)}>
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
  season,
}: {
  board: FriendBoard;
  season: number;
}) {
  const navigate = useNavigate();

  return (
    <GlassPanel className="friendBoard phase6FriendBoard" tone="solid">
      <header>
        <div>
          <h3>{board.label}</h3>
          <p>{board.description}</p>
        </div>
      </header>
      <div className="friendCardList">
        {board.friends.slice(0, 2).map((friend) => {
          function detailState() {
            if (typeof window === "undefined") return buildFriendDetailState(friend.bnetId, season, 0);
            return buildFriendDetailState(friend.bnetId, season, window.scrollY);
          }

          return (
            <Link
              className="friendCardLink"
              key={`${board.key}-${friend.bnetId}`}
              to="/analytics/friends/detail"
              onClick={(event) => {
                event.preventDefault();
                const state = detailState();
                sessionStorage.setItem(state.returnTo.scrollKey, String(state.returnTo.scrollY));
                navigate("/analytics/friends/detail", { state });
              }}
              aria-label={`查看${friend.name}详情`}
            >
              <article className="friendCard phase6FriendCard">
                <strong>{friend.name}</strong>
                <span>{friend.boardLabel}</span>
                <MetricList friend={friend} compact />
              </article>
            </Link>
          );
        })}
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
