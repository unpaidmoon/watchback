import type { ReactNode } from "react";
import { formatArenaRankTierLabel } from "../../shared/arenaDetailFormat";
import { arenaRankAssetUrl, resolvePerkInfo } from "../../shared/assets";
import { playerHighlightMap, playerKeyFor, type HighlightMetric } from "../../shared/playerHighlights";
import { HeroAvatar } from "../../shared/phase6Components";
import type { PlayerRow } from "../../shared/types";

export type MatchTeamTableProps = {
  title: string;
  tone: "team" | "enemy";
  players: PlayerRow[];
  showPerks?: boolean;
  expandedPlayerKeys?: ReadonlySet<string>;
  onTogglePlayer?: (playerKey: string) => void;
  renderExpansion?: (player: PlayerRow) => ReactNode;
};

export function MatchTeamTable({
  title,
  players,
  tone,
  showPerks = true,
  expandedPlayerKeys,
  onTogglePlayer,
  renderExpansion,
}: MatchTeamTableProps) {
  const highlights = playerHighlightMap(players);
  return (
    <section className={`matchTeamTable phase6CompactTeam ${tone}`}>
      <span className={`matchTeamTableMarker ${tone}`} aria-hidden="true" />
      <h3>{title}</h3>
      <div className="phase6CompactPlayerRows">
        {players.map((player, index) => {
          const key = compactPlayerKey(player, index);
          const expanded = expandedPlayerKeys?.has(key) ?? false;
          return (
            <div className="phase6CompactPlayerBlock" key={key}>
              <MatchTeamPlayerRow
                player={player}
                showPerks={showPerks}
                highlights={highlights.get(playerKeyFor(player, index)) ?? new Set()}
                expanded={expanded}
                onToggle={onTogglePlayer ? () => onTogglePlayer(key) : undefined}
              />
              {expanded && renderExpansion ? renderExpansion(player) : null}
            </div>
          );
        })}
      </div>
      {players.length === 0 && <div className="matchTeamTableEmpty">暂无玩家数据。</div>}
    </section>
  );
}

function MatchTeamPlayerRow({
  player,
  showPerks = true,
  highlights,
  expanded = false,
  onToggle,
}: {
  player: PlayerRow;
  showPerks?: boolean;
  highlights: Set<HighlightMetric>;
  expanded?: boolean;
  onToggle?: () => void;
}) {
  const rankIcon = arenaRankAssetUrl(player.rankKey);
  const rankTierLabel = formatArenaRankTierLabel(player.rankLabel);
  const perks = showPerks
    ? player.perks.flatMap((perk) => {
        const info = resolvePerkInfo(perk.guid);
        return info ? [info] : [];
      })
    : [];
  const rowContent = (
    <>
      <span className="phase6CompactAvatarPlate">
        <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} size="sm" />
      </span>
      <span className="phase6CompactPlayerName">
        <span className="phase7CompactPlayerNameLine">
          <strong>{player.name}</strong>
          {(rankIcon || rankTierLabel) && (
            <small className="phase7CompactRankBadge">
              {rankIcon && <img src={rankIcon} alt="" />}
              {rankTierLabel}
            </small>
          )}
        </span>
        {showPerks && (
          <span className="phase6CompactPerkStrip" aria-label="威能">
            {perks.length > 0 ? (
              perks.slice(0, 2).map((perk) => (
                <span className="phase6PerkTooltip" key={perk.id}>
                  <img className="phase6CompactPerkIcon" src={perk.iconUrl ?? undefined} alt={perk.name} />
                  <span className="phase6PerkTooltipPanel" role="tooltip">
                    <strong>{perk.name}</strong>
                    <small>{perk.description ?? "暂无释义"}</small>
                  </span>
                </span>
              ))
            ) : (
              <span className="phase6CompactPerkEmpty">未记录威能</span>
            )}
          </span>
        )}
      </span>
      <span className="phase6CompactPlayerMetric">
        <small>K/A/D</small>
        <strong>
          <MatchTeamHighlight value={player.kills} active={highlights.has("kills")} />/{player.assists}/{player.deaths}
        </strong>
      </span>
      <span className="phase6CompactPlayerMetric phase6CompactPlayerMetricWide">
        <small>伤/疗/挡</small>
        <strong>
          <MatchTeamHighlight value={player.damage} active={highlights.has("damage")} />/
          <MatchTeamHighlight value={player.healing} active={highlights.has("healing")} />/
          <MatchTeamHighlight value={player.mitigation} active={highlights.has("mitigation")} />
        </strong>
      </span>
      <span className="phase6CompactPlayerMetric">
        <small>承伤/受疗</small>
        <strong>{player.damageTaken}/{player.healingTaken}</strong>
      </span>
      <span className="phase6CompactPlayerMetric phase6CompactPlayerMetricFinal">
        <small>最后一击</small>
        <strong>{player.finalHits}</strong>
      </span>
    </>
  );

  if (onToggle) {
    return (
      <button
        type="button"
        className="phase6CompactPlayerRow phase7CompactPlayerExpandable"
        onClick={onToggle}
        aria-expanded={expanded}
        aria-label={`${player.name}${expanded ? "收起角斗详情" : "展开角斗详情"}`}
      >
        {rowContent}
      </button>
    );
  }

  return <article className="phase6CompactPlayerRow">{rowContent}</article>;
}

function MatchTeamHighlight({ value, active }: { value: number; active: boolean }) {
  return <em className={active ? "phase6StatHighlight" : ""}>{value}</em>;
}

function compactPlayerKey(player: PlayerRow, index: number) {
  return `${player.side}-${player.bnetId || player.name}-${player.heroGuid || player.heroName}-${index}`;
}
