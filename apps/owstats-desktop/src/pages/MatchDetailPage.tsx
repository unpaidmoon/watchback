import { AlertCircle, ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getMatchDetails } from "../shared/api";
import { MatchPerformanceBadges } from "../shared/PerformanceBadgesView";
import { arenaHeaderMapKey, arenaHeaderTitle, arenaResultSubLabel, formatArenaRankTierLabel, formatArenaScore, formatArenaTabLabel } from "../shared/arenaDetailFormat";
import { arenaRankAssetUrl } from "../shared/assets";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore, resultEnglishLabel } from "../shared/matchDetailHeaderFormat";
import { GlassPanel, HeroAvatar, MapPreview } from "../shared/phase6Components";
import { playerHighlightMap, playerKeyFor, type HighlightMetric } from "../shared/playerHighlights";
import { productErrorMessage } from "../shared/phase6Shell";
import type { ArenaDetailSlice, MatchDetailView, PlayerRow } from "../shared/types";

export function MatchDetailPage() {
  const { matchId } = useParams();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<MatchDetailView | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!matchId) return;
    void loadDetail(matchId);
  }, [matchId]);

  async function loadDetail(nextMatchId: string) {
    setLoading(true);
    setError(null);
    try {
      setDetail(await getMatchDetails(nextMatchId));
    } catch (err) {
      setError(productErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <MatchDetailReport
      detail={detail}
      loading={loading}
      error={error}
      matchId={matchId}
      onBack={() => navigate("/matches")}
    />
  );
}

type MatchDetailReportProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId?: string | null;
  onBack?: () => void;
  embedded?: boolean;
};

export function MatchDetailReport({ detail, loading, error, matchId, onBack, embedded = false }: MatchDetailReportProps) {
  const [selectedArenaKey, setSelectedArenaKey] = useState("summary");
  const arenaDetail = detail?.arenaDetail ?? null;

  useEffect(() => {
    setSelectedArenaKey("summary");
  }, [detail?.matchId]);

  if (detail && arenaDetail) {
    const selectedArena =
      selectedArenaKey === "summary"
        ? arenaDetail.summary
        : arenaDetail.rounds.find((round) => round.key === selectedArenaKey) ?? arenaDetail.summary;
    return (
      <section className={`detailPage phase6DetailPage ${embedded ? "phase6EmbeddedDetailPage" : ""}`}>
        <ArenaSelector
          summary={arenaDetail.summary}
          rounds={arenaDetail.rounds}
          selectedKey={selectedArena.key}
          onSelect={setSelectedArenaKey}
        />
        <header className={`detailHero phase6DetailHero phase7ArenaHero ${selectedArena.result ?? detail.result ?? "pending"}`}>
          {onBack && (
            <button type="button" className="phase6DetailBackButton" onClick={onBack}>
              <ArrowLeft size={18} />
              返回列表
            </button>
          )}
          <div className="phase6DetailBanner">
            <MapPreview className="phase6DetailBannerMap" mapKey={arenaHeaderMapKey(selectedArena)} name={selectedArena.mapName ?? "未知地图"} />
            <div className="phase6DetailResultPanel">
              <strong>{selectedArena.resultLabel ?? "未知"}</strong>
              <span>{arenaResultSubLabel(selectedArena.result ?? detail.result)}</span>
            </div>
            <div className="phase6DetailCornerAccent" />
          </div>
          <div className="phase6DetailSummary">
            <h2>{arenaHeaderTitle(selectedArena)}</h2>
            <div className="phase6DetailFacts">
              <span>
                <b>比分：</b>
                {formatArenaScore(selectedArena)}
              </span>
              <span>
                <b>游戏时长：</b>
                {formatDetailDuration(selectedArena.durationSec)}
              </span>
              {selectedArena.key !== "summary" && (
                <span>
                  <b>游戏模式：</b>
                  {detail.modeLabel ?? "-"}
                </span>
              )}
              <span>
                <b>开始时间：</b>
                {formatDetailStartTime(selectedArena.beginTs)}
              </span>
            </div>
          </div>
        </header>

        {error && (
          <div className="errorBanner phase6RecoverableError">
            <AlertCircle size={18} />
            <span>{error}</span>
          </div>
        )}
        {loading && <div className="emptyState">正在读取对局详情...</div>}
        <div className="detailContent">
          <div className="playerColumns phase6PlayerColumns" key={selectedArena.key}>
            <PlayerTable title="队友" players={selectedArena.teammates} tone="team" />
            <div className="phase6VsDivider" aria-hidden="true">
              <span>VS</span>
            </div>
            <PlayerTable title="对手" players={selectedArena.enemies} tone="enemy" />
          </div>
        </div>
      </section>
    );
  }
  const detailMapName = detail?.mapName ?? "未知地图";

  return (
    <section className={`detailPage phase6DetailPage ${embedded ? "phase6EmbeddedDetailPage" : ""}`}>
      <header className={`detailHero phase6DetailHero ${detail?.result ?? "pending"}`}>
        {detail ? (
          <>
            {onBack && (
              <button type="button" className="phase6DetailBackButton" onClick={onBack}>
                <ArrowLeft size={18} />
                返回列表
              </button>
            )}
            <div className="phase6DetailBanner">
              <MapPreview className="phase6DetailBannerMap" mapKey={detail.mapGuid ?? detail.mapName} name={detailMapName} />
              <div className="phase6DetailResultPanel">
                <strong>{detail.resultLabel ?? "未知"}</strong>
                <span>{resultEnglishLabel(detail.result)}</span>
              </div>
              <div className="phase6DetailCornerAccent" />
            </div>
            <div className="phase6DetailSummary">
              <h2>地图：{detailMapName}</h2>
              <div className="phase6DetailFacts">
                <span>
                  <b>最终比分：</b>
                  {formatFinalScore(detail)}
                </span>
                <span>
                  <b>游戏时长：</b>
                  {formatDetailDuration(detail.durationSec)}
                </span>
                <span>
                  <b>游戏模式：</b>
                  {detail.modeLabel ?? "-"}
                </span>
                <span>
                  <b>对局开始时间：</b>
                  {formatDetailStartTime(detail.beginTs)}
                </span>
              </div>
            </div>
          </>
        ) : (
          <div className="phase6DetailLoadingHeader">
            {onBack && (
              <button type="button" className="ghostButton" onClick={onBack}>
                <ArrowLeft size={18} />
                返回列表
              </button>
            )}
            <div>
              <span className="phase6Eyebrow">对局报告</span>
              <h2>{matchId}</h2>
            </div>
          </div>
        )}
      </header>

      {error && (
        <div className="errorBanner phase6RecoverableError">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      )}
      {loading && <div className="emptyState">正在读取对局详情...</div>}
      {!loading && !detail && !error && <div className="emptyState">未找到对局详情。</div>}
      {detail && (
        <div className="detailContent">
          <DetailEvaluation detail={detail} />
          <SameRoleComparison detail={detail} />
          <div className="playerColumns phase6PlayerColumns">
            <PlayerTable title="队友" players={detail.teammates} tone="team" />
            <div className="phase6VsDivider" aria-hidden="true">
              <span>VS</span>
            </div>
            <PlayerTable title="对手" players={detail.enemies} tone="enemy" />
          </div>
        </div>
      )}
    </section>
  );
}

function ArenaSelector({
  summary,
  rounds,
  selectedKey,
  onSelect,
}: {
  summary: ArenaDetailSlice;
  rounds: ArenaDetailSlice[];
  selectedKey: string;
  onSelect: (key: string) => void;
}) {
  return (
    <nav className="phase7ArenaSelector" aria-label="角斗回合切换">
      <button
        type="button"
        className={`phase7ArenaTab summary ${selectedKey === summary.key ? "active" : ""}`}
        onClick={() => onSelect(summary.key)}
      >
        <span>{summary.label}</span>
      </button>
      {rounds.map((round) => (
        <button
          type="button"
          className={`phase7ArenaTab ${round.result ?? "neutral"} ${selectedKey === round.key ? "active" : ""}`}
          onClick={() => onSelect(round.key)}
          key={round.key}
        >
          <span>{formatArenaTabLabel(round)}</span>
          <strong>{formatArenaScore(round)}</strong>
        </button>
      ))}
    </nav>
  );
}

function DetailEvaluation({ detail }: { detail: MatchDetailView }) {
  if (!detail.evaluation && detail.teamAwards.length === 0) return null;
  return (
    <GlassPanel className="roleComparison phase6ReportPanel" tone="solid">
      <header>
        <div>
          <span className="phase6Eyebrow">对局表现</span>
          <h3>评分与同队奖牌</h3>
        </div>
      </header>
      <MatchPerformanceBadges evaluation={detail.evaluation} teamAwards={detail.teamAwards} />
    </GlassPanel>
  );
}

function SameRoleComparison({ detail }: { detail: MatchDetailView }) {
  const comparison = detail.sameRoleComparison;
  if (!comparison) return null;
  const cards = [
    deltaCard("击杀", "kills", comparison.current.kills, comparison.average.kills),
    deltaCard("死亡", "deaths", comparison.current.deaths, comparison.average.deaths),
    deltaCard("伤害", "damage", comparison.current.damage, comparison.average.damage),
    deltaCard("治疗", "healing", comparison.current.healing, comparison.average.healing)
  ].filter((item): item is DeltaCard => item !== null);
  if (cards.length === 0) return null;
  return (
    <GlassPanel className="roleComparison phase6ReportPanel" tone="solid">
      <header>
        <div>
          <span className="phase6Eyebrow">同职责赛季均值对比</span>
          <h3>{comparison.roleLabel ?? "未知职责"}</h3>
        </div>
      </header>
      <div className="deltaGrid">
        {cards.map((card) => (
          <div className={`deltaCard ${card.tone}`} key={card.metric}>
            <span>{card.label}</span>
            <strong>{formatSigned(card.delta)}</strong>
          </div>
        ))}
      </div>
    </GlassPanel>
  );
}

type DeltaCard = {
  label: string;
  metric: "kills" | "deaths" | "damage" | "healing";
  delta: number;
  tone: "good" | "bad" | "neutral";
};

function deltaCard(
  label: string,
  metric: DeltaCard["metric"],
  current: number | null,
  average: number | null
): DeltaCard | null {
  if (current === null || average === null) return null;
  const delta = current - average;
  return {
    label,
    metric,
    delta,
    tone: deltaTone(metric, delta)
  };
}

function deltaTone(metric: DeltaCard["metric"], delta: number): DeltaCard["tone"] {
  if (delta === 0) return "neutral";
  if (metric === "deaths") return delta < 0 ? "good" : "bad";
  return delta > 0 ? "good" : "bad";
}

function formatSigned(value: number) {
  const rounded = Math.abs(value) >= 100 ? Math.round(value) : Number(value.toFixed(1));
  return `${value > 0 ? "+" : ""}${rounded}`;
}

function PlayerTable({ title, players, tone }: { title: string; players: PlayerRow[]; tone: "team" | "enemy" }) {
  const highlights = playerHighlightMap(players);
  return (
    <GlassPanel className={`playerSection phase6PlayerSection ${tone}`} tone="solid">
      <h3>{title}</h3>
      <div className="playerTable phase6PlayerTable" aria-label={`${title}玩家数据`}>
        {players.map((player, index) => {
          const playerKey = playerKeyFor(player, index);
          return (
            <PlayerCard
              player={player}
              highlights={highlights.get(playerKey) ?? new Set()}
              key={playerKey}
            />
          );
        })}
      </div>
    </GlassPanel>
  );
}

function PlayerCard({ player, highlights }: { player: PlayerRow; highlights: Set<HighlightMetric> }) {
  const rankIcon = arenaRankAssetUrl(player.rankKey);
  const rankTierLabel = formatArenaRankTierLabel(player.rankLabel);
  return (
    <article className="phase6PlayerCard">
      <div className="phase7PlayerCardMain">
        <span className="phase6PlayerPortrait">
          <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} />
        </span>
        <span className="phase6PlayerStats">
          <header>
            <span className="phase7PlayerIdentity">
              <span className="phase7PlayerNameLine">
                <strong>{player.name}</strong>
                {(rankIcon || rankTierLabel) && (
                  <span className="phase7ArenaRankBadge">
                    {rankIcon && <img src={rankIcon} alt="" />}
                    {rankTierLabel}
                  </span>
                )}
              </span>
              <span className="phase7PlayerHeroName">{player.heroName}</span>
            </span>
          </header>
          <span className="phase6PlayerMetricGrid">
            <span>
              <b>K/A/D</b>
              <strong>
                <Highlight value={player.kills} active={highlights.has("kills")} />/{player.assists}/{player.deaths}
              </strong>
            </span>
            <span>
              <b>伤害/治疗/抵挡</b>
              <strong>
                <Highlight value={player.damage} active={highlights.has("damage")} />/
                <Highlight value={player.healing} active={highlights.has("healing")} />/
                <Highlight value={player.mitigation} active={highlights.has("mitigation")} />
              </strong>
            </span>
            <span>
              <b>承伤/受疗</b>
              <strong>
                {player.damageTaken}/{player.healingTaken}
              </strong>
            </span>
          </span>
        </span>
      </div>
    </article>
  );
}

function Highlight({ value, active }: { value: number; active: boolean }) {
  return <em className={active ? "phase6StatHighlight" : ""}>{value}</em>;
}
