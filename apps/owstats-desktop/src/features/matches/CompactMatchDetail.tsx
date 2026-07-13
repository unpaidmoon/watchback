import { AlertCircle, CircleDollarSign } from "lucide-react";
import { useEffect, useState } from "react";
import { arenaHeaderTitle, formatArenaScore, formatArenaTabLabel } from "../../shared/arenaDetailFormat";
import { resolveArenaModTraitInfo } from "../../shared/assets";
import { MatchPerformanceBadges } from "../../shared/PerformanceBadgesView";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore } from "../../shared/matchDetailHeaderFormat";
import type { MatchDetailView, PlayerRow } from "../../shared/types";
import { MatchDetailHeader, type MatchDetailNavigationProps } from "./MatchDetailHeader";
import { MatchTeamTable } from "./MatchTeamTable";

export type CompactMatchDetailProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId: string;
} & MatchDetailNavigationProps;

export function CompactMatchDetail({ detail, loading, error, matchId, ...navigation }: CompactMatchDetailProps) {
  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }
  if (loading && !detail) return <div className="emptyState phase6WorkbenchEmpty">正在读取对局详情...</div>;
  if (!detail) return <div className="emptyState phase6WorkbenchEmpty">未找到对局详情。</div>;
  if (detail.arenaDetail) return <CompactArenaDetail detail={detail} loading={loading} {...navigation} />;

  const mapName = detail.mapName ?? "未知地图";
  return (
    <section className={`phase6CompactDetail ${detail.result ?? "pending"}`} aria-label="普通对局详情">
      <MatchDetailHeader
        resultLabel={detail.resultLabel ?? "未知结果"}
        result={detail.result}
        mapName={mapName}
        modeLabel={detail.modeLabel ?? "未知模式"}
        score={formatFinalScore(detail)}
        duration={formatDetailDuration(detail.durationSec)}
        startTime={formatDetailStartTime(detail.beginTs)}
        {...navigation}
      />

      <div className="phase6CompactDetailScroll">
        <section className="matchDetailPerformance" aria-labelledby="self-performance-title">
          <div>
            <span className="phase6Eyebrow">个人摘要</span>
            <h2 id="self-performance-title">本人表现</h2>
            <p>评分与同队奖牌</p>
          </div>
          <MatchPerformanceBadges evaluation={detail.evaluation} teamAwards={detail.teamAwards} />
        </section>

        <section className="matchDetailTeamComparison" aria-labelledby="team-comparison-title">
          <h2 id="team-comparison-title">队伍对比</h2>
          <div className="matchDetailTeamTables">
            <MatchTeamTable title="队友数据" tone="team" players={detail.teammates.slice(0, 6)} showPerks />
            <MatchTeamTable title="对手数据" tone="enemy" players={detail.enemies.slice(0, 6)} showPerks />
          </div>
        </section>
      </div>
    </section>
  );
}

function CompactArenaDetail({
  detail,
  loading,
  ...navigation
}: { detail: MatchDetailView; loading: boolean } & MatchDetailNavigationProps) {
  const arena = detail.arenaDetail;
  const [selectedKey, setSelectedKey] = useState("summary");
  const [expandedCompactArenaPlayerKeys, setExpandedCompactArenaPlayerKeys] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    setSelectedKey("summary");
    setExpandedCompactArenaPlayerKeys(new Set());
  }, [detail.matchId]);

  if (!arena) return null;

  function selectCompactArenaSlice(nextKey: string) {
    setSelectedKey(nextKey);
    setExpandedCompactArenaPlayerKeys(new Set());
  }

  function toggleCompactArenaPlayer(playerKey: string) {
    setExpandedCompactArenaPlayerKeys((current) => {
      const next = new Set(current);
      if (next.has(playerKey)) next.delete(playerKey);
      else next.add(playerKey);
      return next;
    });
  }

  const selected =
    selectedKey === "summary"
      ? arena.summary
      : arena.rounds.find((round) => round.key === selectedKey) ?? arena.summary;
  return (
    <section
      className={`phase6CompactDetail phase7CompactArenaDetail ${selected.result ?? detail.result ?? "pending"}`}
      aria-label="角斗对局详情"
    >
      <nav className="phase7ArenaSelector" aria-label="角斗回合切换">
        <button
          type="button"
          className={`phase7ArenaTab summary ${selected.key === arena.summary.key ? "active" : ""}`}
          onClick={() => selectCompactArenaSlice(arena.summary.key)}
        >
          <span>{arena.summary.label}</span>
        </button>
        {arena.rounds.map((round) => (
          <button
            type="button"
            className={`phase7ArenaTab ${round.result ?? "neutral"} ${selected.key === round.key ? "active" : ""}`}
            onClick={() => selectCompactArenaSlice(round.key)}
            key={round.key}
          >
            <span>{formatArenaTabLabel(round)}</span>
            <strong>{formatArenaScore(round)}</strong>
          </button>
        ))}
      </nav>

      <MatchDetailHeader
        resultLabel={selected.resultLabel ?? detail.resultLabel ?? "未知结果"}
        result={selected.result ?? detail.result}
        mapName={arenaHeaderTitle(selected)}
        modeLabel={detail.modeLabel ?? "角斗竞技"}
        score={formatArenaScore(selected)}
        duration={formatDetailDuration(selected.durationSec)}
        startTime={formatDetailStartTime(selected.beginTs)}
        {...navigation}
      />

      <div className="phase6CompactDetailScroll">
        {loading && <div className="emptyState">正在刷新角斗详情...</div>}
        <section className="matchDetailTeamComparison" aria-labelledby="arena-team-comparison-title" key={selected.key}>
          <h2 id="arena-team-comparison-title">队伍对比</h2>
          <div className="matchDetailTeamTables">
            <MatchTeamTable
              title="队友数据"
              tone="team"
              players={selected.teammates}
              showPerks={false}
              expandedPlayerKeys={expandedCompactArenaPlayerKeys}
              onTogglePlayer={toggleCompactArenaPlayer}
              renderExpansion={(player) => (
                <CompactArenaPlayerExpansion
                  player={player}
                  economyMode={selected.key === "summary" ? "final" : "round"}
                />
              )}
            />
            <MatchTeamTable
              title="对手数据"
              tone="enemy"
              players={selected.enemies}
              showPerks={false}
              expandedPlayerKeys={expandedCompactArenaPlayerKeys}
              onTogglePlayer={toggleCompactArenaPlayer}
              renderExpansion={(player) => (
                <CompactArenaPlayerExpansion
                  player={player}
                  economyMode={selected.key === "summary" ? "final" : "round"}
                />
              )}
            />
          </div>
        </section>
      </div>
    </section>
  );
}

function CompactArenaPlayerExpansion({ player, economyMode }: { player: PlayerRow; economyMode: "final" | "round" }) {
  return (
    <div className="phase7CompactArenaExpansion">
      <div className="phase7CompactArenaEconomy">
        <span className="phase7CompactArenaMoneyIcon" aria-hidden="true">
          <CircleDollarSign size={46} strokeWidth={1.8} />
        </span>
        <strong className={player.worthMax ? "phase6StatHighlight" : ""}>{formatCompactArenaEconomy(player, economyMode)}</strong>
        <span className="phase7CompactArenaMoneyLabel">本场资金</span>
      </div>
      <CompactArenaSlotGroup title="异能" guids={player.traitGuids} slots={4} />
      <CompactArenaSlotGroup title="装备" guids={player.modGuids} slots={6} />
    </div>
  );
}

function CompactArenaSlotGroup({ title, guids, slots }: { title: string; guids: string[]; slots: number }) {
  const variant = slots === 4 ? "trait" : "mod";
  return (
    <div className={`phase7ArenaSlotGroup ${variant}`}>
      <span>{title}</span>
      <div className="phase7ArenaSlots">
        {Array.from({ length: slots }).map((_, index) => {
          const guid = guids[index];
          const info = resolveArenaModTraitInfo(guid);
          const displayName = info?.name ?? guid;
          return (
            <span
              className={`phase7ArenaSlot ${variant} ${guid ? "filled" : ""}`}
              key={`${title}-${index}`}
              tabIndex={guid ? 0 : undefined}
              aria-label={guid ? displayName : undefined}
            >
              {guid && info?.iconUrl && <img src={info.iconUrl} alt={info.name} />}
              {guid && variant === "trait" && <span className="phase7ArenaSlotName">{displayName}</span>}
              {guid && (
                <span className="phase6PerkTooltipPanel" role="tooltip">
                  <strong>{displayName}</strong>
                  {info?.description && <small>{info.description}</small>}
                </span>
              )}
            </span>
          );
        })}
      </div>
    </div>
  );
}

function formatCompactArenaEconomy(player: PlayerRow, mode: "final" | "round"): string {
  if (mode === "final") {
    if (player.endWorth !== null) return String(player.endWorth);
    if (player.worth !== null) return String(player.worth);
    return "-";
  }
  if (player.worth !== null && player.endWorth !== null && player.worth !== player.endWorth) {
    return `${player.worth} -> ${player.endWorth}`;
  }
  if (player.endWorth !== null) return String(player.endWorth);
  if (player.worth !== null) return String(player.worth);
  return "-";
}
