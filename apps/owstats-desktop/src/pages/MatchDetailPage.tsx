import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getMatchDetails } from "../shared/api";
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
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className="detailPage">
      <header className="detailHero">
        <button type="button" className="ghostButton" onClick={() => navigate("/matches")}>
          <ArrowLeft size={18} />
          返回列表
        </button>
        <div>
          <span className="eyebrow">对局详情</span>
          <h2>{matchId}</h2>
        </div>
      </header>

      {error && <div className="errorBanner">{error}</div>}
      {loading && <div className="emptyState">正在读取对局详情...</div>}
      {!loading && !detail && !error && <div className="emptyState">未找到对局详情。</div>}
      {detail && (
        <div className="detailContent">
          <div className="playerColumns">
            <PlayerTable title="队友" players={detail.teammates} />
            <PlayerTable title="对手" players={detail.enemies} />
          </div>
          <SameRoleComparison detail={detail} />
          <section className="rawViewer">
            <h3>已脱敏原始 JSON</h3>
            <pre>{JSON.stringify(detail.rawJson, null, 2)}</pre>
          </section>
        </div>
      )}
    </section>
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
    <section className="roleComparison">
      <header>
        <span className="eyebrow">同职责本赛季均值对比</span>
        <h3>{comparison.roleLabel ?? comparison.rawRole ?? "未知职责"}</h3>
      </header>
      <div className="deltaGrid">
        {cards.map((card) => (
          <div className={`deltaCard ${card.tone}`} key={card.metric}>
            <span>{card.label}</span>
            <strong>{formatSigned(card.delta)}</strong>
          </div>
        ))}
      </div>
    </section>
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

function PlayerTable({ title, players }: { title: string; players: PlayerRow[] }) {
  return (
    <section className="playerSection">
      <h3>{title}</h3>
      <div className="playerTable">
        <div className="playerHead">
          <span>玩家</span>
          <span>英雄</span>
          <span>击杀/助攻/死亡</span>
          <span>伤害/治疗/格挡</span>
          <span>承伤/受疗</span>
        </div>
        {players.map((player, index) => (
          <div className="playerLine" key={`${player.side}-${player.bnetId || player.name}-${index}`}>
            <span>{player.name}</span>
            <span>{player.heroName}</span>
            <span>
              {player.kills}/{player.assists}/{player.deaths}
            </span>
            <span>
              {player.damage}/{player.healing}/{player.mitigation}
            </span>
            <span>
              {player.damageTaken}/{player.healingTaken}
            </span>
          </div>
        ))}
      </div>
    </section>
  );
}
