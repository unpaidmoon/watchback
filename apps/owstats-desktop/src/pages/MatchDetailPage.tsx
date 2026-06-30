import { AlertCircle, ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getMatchDetails } from "../shared/api";
import { MatchPerformanceBadges } from "../shared/PerformanceBadgesView";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore, resultEnglishLabel } from "../shared/matchDetailHeaderFormat";
import { GlassPanel, HeroAvatar, MapPreview } from "../shared/phase6Components";
import { playerHighlightMap, playerKeyFor, type HighlightMetric } from "../shared/playerHighlights";
import { productErrorMessage } from "../shared/phase6Shell";
import type { MatchDetailView, PlayerRow } from "../shared/types";

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

  const detailMapName = detail?.mapName ?? "未知地图";

  return (
    <section className="detailPage phase6DetailPage">
      <header className={`detailHero phase6DetailHero ${detail?.result ?? "pending"}`}>
        {detail ? (
          <>
            <button type="button" className="phase6DetailBackButton" onClick={() => navigate("/matches")}>
              <ArrowLeft size={18} />
              返回列表
            </button>
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
            <button type="button" className="ghostButton" onClick={() => navigate("/matches")}>
              <ArrowLeft size={18} />
              返回列表
            </button>
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
        {players.map((player, index) => (
          <PlayerCard player={player} index={index} highlights={highlights.get(playerKeyFor(player, index)) ?? new Set()} key={playerKeyFor(player, index)} />
        ))}
      </div>
    </GlassPanel>
  );
}

function PlayerCard({ player, highlights }: { player: PlayerRow; index: number; highlights: Set<HighlightMetric> }) {
  return (
    <article className="phase6PlayerCard">
      <div className="phase6PlayerPortrait">
        <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} />
      </div>
      <div className="phase6PlayerStats">
        <header>
          <strong>{player.name}</strong>
          <span>{player.heroName}</span>
        </header>
        <div className="phase6PlayerMetricGrid">
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
        </div>
      </div>
    </article>
  );
}

function Highlight({ value, active }: { value: number; active: boolean }) {
  return <em className={active ? "phase6StatHighlight" : ""}>{value}</em>;
}
