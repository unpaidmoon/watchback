import { CircleDollarSign } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { arenaHeaderTitle, formatArenaScore, formatArenaTabLabel } from "../../shared/arenaDetailFormat";
import { resolveArenaModTraitInfo } from "../../shared/stadiumAssets";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore } from "../../shared/matchDetailHeaderFormat";
import { PageErrorState, PageLoadingState } from "../../shared/page-shell";
import type { MatchDetailView, PlayerRow } from "../../shared/types";
import { ArenaPlayerRoundSummaryDialog } from "./ArenaPlayerRoundSummaryDialog";
import { MatchDetailHeader } from "./MatchDetailHeader";
import { MatchTeamTable } from "./MatchTeamTable";
import { buildArenaPlayerRoundSummary } from "./arenaPlayerRoundSummary";

export type MatchDetailCapabilities = {
  confirmDiscard?: () => boolean;
  renderAi?: (detail: MatchDetailView) => ReactNode;
  renderFavorite?: (detail: MatchDetailView) => ReactNode;
};

export type MatchDetailContentProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
  capabilities?: MatchDetailCapabilities;
  onQueryPlayer?: (battleTag: string) => void;
};

/** Pure remote-compatible match presentation. Persistent/local features are opt-in capability slots. */
export function MatchDetailContent({ detail, loading, error, onRetry, capabilities, onQueryPlayer }: MatchDetailContentProps) {
  if (error) return <PageErrorState message={error} actionLabel="重新加载" onRetry={onRetry} />;
  // Loading always wins so a selection change can never expose the previous match.
  if (loading) return <PageLoadingState message="正在读取对局详情..." />;
  if (!detail) return <div className="emptyState phase6WorkbenchEmpty">未找到对局详情。</div>;
  if (detail.arenaDetail) return <ArenaMatchDetailContent detail={detail} capabilities={capabilities} onQueryPlayer={onQueryPlayer} />;

  const rankVariant = detail.gameMode === "SportPreset" || detail.gameMode === "SportOpen" ? "competitive" : undefined;
  return (
    <section className={`phase6CompactDetail ${detail.result ?? "pending"}`} aria-label="普通对局详情">
      <MatchDetailHeader
        resultLabel={detail.resultLabel ?? "未知结果"}
        result={detail.result}
        mapGuid={detail.mapGuid}
        mapName={detail.mapName ?? "未知地图"}
        modeLabel={detail.modeLabel ?? "未知模式"}
        score={formatFinalScore(detail)}
        duration={formatDetailDuration(detail.durationSec)}
        startTime={formatDetailStartTime(detail.beginTs)}
      />
      <div className="phase6CompactDetailScroll">
        <section className="matchDetailTeamComparison" aria-labelledby="team-comparison-title">
          <h2 id="team-comparison-title">队伍对比</h2>
          <div className="matchDetailTeamTables">
            <MatchTeamTable title="队友数据" tone="team" players={detail.teammates.slice(0, 6)} showPerks rankVariant={rankVariant} onQueryPlayer={onQueryPlayer} />
            <MatchTeamTable title="对手数据" tone="enemy" players={detail.enemies.slice(0, 6)} showPerks rankVariant={rankVariant} onQueryPlayer={onQueryPlayer} />
          </div>
        </section>
        {capabilities?.renderAi?.(detail)}
        {capabilities?.renderFavorite?.(detail)}
      </div>
    </section>
  );
}

function ArenaMatchDetailContent({ detail, capabilities, onQueryPlayer }: { detail: MatchDetailView; capabilities?: MatchDetailCapabilities; onQueryPlayer?: (battleTag: string) => void }) {
  const arena = detail.arenaDetail;
  const [selectedKey, setSelectedKey] = useState("summary");
  const [expandedCompactArenaPlayerKeys, setExpandedCompactArenaPlayerKeys] = useState<Set<string>>(() => new Set());
  const [summaryTarget, setSummaryTarget] = useState<{ player: PlayerRow; trigger: HTMLButtonElement } | null>(null);

  useEffect(() => {
    setSelectedKey("summary");
    setExpandedCompactArenaPlayerKeys(new Set());
    setSummaryTarget(null);
  }, [detail.matchId]);
  if (!arena) return null;

  function selectCompactArenaSlice(nextKey: string) {
    if (selectedKey === arena!.summary.key && nextKey !== arena!.summary.key && capabilities?.confirmDiscard && !capabilities.confirmDiscard()) return;
    setSelectedKey(nextKey);
    setExpandedCompactArenaPlayerKeys(new Set());
    setSummaryTarget(null);
  }
  function toggleCompactArenaPlayer(playerKey: string) {
    setExpandedCompactArenaPlayerKeys((current) => {
      const next = new Set(current);
      if (next.has(playerKey)) next.delete(playerKey); else next.add(playerKey);
      return next;
    });
  }
  function closeArenaPlayerSummary() {
    const trigger = summaryTarget?.trigger;
    setSummaryTarget(null);
    requestAnimationFrame(() => trigger?.focus());
  }

  const selected = selectedKey === "summary" ? arena.summary : arena.rounds.find((round) => round.key === selectedKey) ?? arena.summary;
  return (
    <section className={`phase6CompactDetail phase7CompactArenaDetail ${selected.result ?? detail.result ?? "pending"}`} aria-label="角斗对局详情">
      <nav className="phase7ArenaSelector" aria-label="角斗回合切换">
        <button type="button" className={`phase7ArenaTab summary ${selected.key === arena.summary.key ? "active" : ""}`} onClick={() => selectCompactArenaSlice(arena.summary.key)}><span>{arena.summary.label}</span></button>
        {arena.rounds.map((round) => (
          <button type="button" className={`phase7ArenaTab ${round.result ?? "neutral"} ${selected.key === round.key ? "active" : ""}`} onClick={() => selectCompactArenaSlice(round.key)} key={round.key}>
            <span>{formatArenaTabLabel(round)}</span><strong>{formatArenaScore(round)}</strong>
          </button>
        ))}
      </nav>
      <MatchDetailHeader
        resultLabel={selected.resultLabel ?? detail.resultLabel ?? "未知结果"}
        result={selected.result ?? detail.result}
        mapGuid={selected.mapGuid}
        mapName={arenaHeaderTitle(selected)}
        modeLabel={detail.modeLabel ?? "角斗竞技"}
        score={formatArenaScore(selected)}
        duration={formatDetailDuration(selected.durationSec)}
        startTime={formatDetailStartTime(selected.beginTs)}
      />
      <div className="phase6CompactDetailScroll">
        <section className="matchDetailTeamComparison" aria-labelledby="arena-team-comparison-title" key={selected.key}>
          <h2 id="arena-team-comparison-title">队伍对比</h2>
          <div className="matchDetailTeamTables">
            {(["team", "enemy"] as const).map((tone) => (
              <MatchTeamTable
                key={tone}
                rankVariant="arena"
                title={tone === "team" ? "队友数据" : "对手数据"}
                tone={tone}
                players={tone === "team" ? selected.teammates : selected.enemies}
                showPerks={false}
                expandedPlayerKeys={expandedCompactArenaPlayerKeys}
                onTogglePlayer={toggleCompactArenaPlayer}
                onQueryPlayer={onQueryPlayer}
                onSelectPlayerSummary={selected.key === "summary" ? (player, trigger) => setSummaryTarget({ player, trigger }) : undefined}
                renderExpansion={(player) => <ArenaPlayerExpansion player={player} economyMode={selected.key === "summary" ? "final" : "round"} />}
              />
            ))}
          </div>
        </section>
        {selected.key === "summary" && capabilities?.renderAi?.(detail)}
        {selected.key === "summary" && capabilities?.renderFavorite?.(detail)}
      </div>
      {summaryTarget && <ArenaPlayerRoundSummaryDialog player={summaryTarget.player} rows={buildArenaPlayerRoundSummary(summaryTarget.player, arena.rounds)} onClose={closeArenaPlayerSummary} />}
    </section>
  );
}

function ArenaPlayerExpansion({ player, economyMode }: { player: PlayerRow; economyMode: "final" | "round" }) {
  return (
    <div className="phase7CompactArenaExpansion">
      <div className="phase7CompactArenaEconomy">
        <span className="phase7CompactArenaMoneyIcon" aria-hidden="true"><CircleDollarSign size={46} strokeWidth={1.8} /></span>
        <strong className={player.worthMax ? "phase6StatHighlight" : ""}>{formatArenaEconomy(player, economyMode)}</strong>
        <span className="phase7CompactArenaMoneyLabel">本场资金</span>
      </div>
      <ArenaSlotGroup title="异能" guids={player.traitGuids} slots={4} />
      <ArenaSlotGroup title="装备" guids={player.modGuids} slots={6} />
    </div>
  );
}

function ArenaSlotGroup({ title, guids, slots }: { title: string; guids: string[]; slots: number }) {
  const variant = slots === 4 ? "trait" : "mod";
  return <div className={`phase7ArenaSlotGroup ${variant}`}><span>{title}</span><div className="phase7ArenaSlots">
    {Array.from({ length: slots }).map((_, index) => {
      const guid = guids[index];
      const info = resolveArenaModTraitInfo(guid);
      const displayName = info?.name ?? guid;
      return <span className={`phase7ArenaSlot ${variant} ${guid ? "filled" : ""}`} key={`${title}-${index}`} tabIndex={guid ? 0 : undefined} aria-label={guid ? displayName : undefined}>
        {guid && info?.iconUrl && <img src={info.iconUrl} alt={info.name} />}
        {guid && variant === "trait" && <span className="phase7ArenaSlotName">{displayName}</span>}
        {guid && <span className="phase6PerkTooltipPanel" role="tooltip"><strong>{displayName}</strong>{info?.description && <small>{info.description}</small>}</span>}
      </span>;
    })}
  </div></div>;
}

function formatArenaEconomy(player: PlayerRow, mode: "final" | "round"): string {
  if (mode === "final") {
    if (player.endWorth !== null) return String(player.endWorth);
    if (player.worth !== null) return String(player.worth);
    return "-";
  }
  if (mode === "round") {
    if (player.endWorth !== null) return String(player.endWorth);
    if (player.worth !== null) return String(player.worth);
  }
  return "-";
}
