import type { ReactNode } from "react";
import { formatArenaRankTierLabel } from "../../shared/arenaDetailFormat";
import { arenaRankAssetUrl, competitiveRankAssetUrl, resolvePerkInfo } from "../../shared/assets";
import { playerHighlightMap, playerKeyFor, type HighlightMetric } from "../../shared/playerHighlights";
import { HeroAvatar } from "../../shared/phase6Components";
import type { PlayerRow } from "../../shared/types";
import { isValidBattleTag, normalizeBattleTag } from "../player-query/playerQueryState";
import { playerPartyBlocks } from "./partyGroups";

export type MatchTeamTableProps = {
  title: string;
  tone: "team" | "enemy";
  players: PlayerRow[];
  showPerks?: boolean;
  rankVariant?: "competitive" | "arena";
  expandedPlayerKeys?: ReadonlySet<string>;
  onTogglePlayer?: (playerKey: string) => void;
  onSelectPlayerSummary?: (player: PlayerRow, trigger: HTMLButtonElement) => void;
  renderExpansion?: (player: PlayerRow) => ReactNode;
  onQueryPlayer?: (battleTag: string) => void;
};

export function MatchTeamTable({
  title,
  players,
  tone,
  showPerks = true,
  rankVariant,
  expandedPlayerKeys,
  onTogglePlayer,
  onSelectPlayerSummary,
  renderExpansion,
  onQueryPlayer,
}: MatchTeamTableProps) {
  const highlights = playerHighlightMap(players);
  return (
    <section className={`matchTeamTable phase6CompactTeam ${tone}`}>
      <h3>{title}</h3>
      <div className="phase6CompactPlayerRows">
        {playerPartyBlocks(players).map((block, blockIndex) => {
          const rows = block.players.map((player) => {
            const index = players.indexOf(player);
            const key = compactPlayerKey(player, index);
            const expanded = expandedPlayerKeys?.has(key) ?? false;
            return (
              <div className="phase6CompactPlayerBlock" key={key}>
                <MatchTeamPlayerRow
                  player={player}
                  showPerks={showPerks}
                  rankVariant={rankVariant}
                  highlights={highlights.get(playerKeyFor(player, index)) ?? new Set()}
                  expanded={expanded}
                  onToggle={onTogglePlayer ? () => onTogglePlayer(key) : undefined}
                  onSelectPlayerSummary={onSelectPlayerSummary}
                  onQueryPlayer={onQueryPlayer}
                />
                {expanded && renderExpansion ? renderExpansion(player) : null}
              </div>
            );
          });
          if (block.partyGroup !== null) {
            return (
              <div
                className="matchTeamPartyGroup"
                aria-label={`${block.players.length} 人小队`}
                key={`party-${block.partyGroup}`}
              >
                <span className="matchTeamPartyRail" aria-hidden="true" />
                {rows}
              </div>
            );
          }
          return (
            <div className="matchTeamSoloPlayer" key={`solo-${blockIndex}`}>
              {rows}
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
  rankVariant,
  highlights,
  expanded = false,
  onToggle,
  onSelectPlayerSummary,
  onQueryPlayer,
}: {
  player: PlayerRow;
  showPerks?: boolean;
  rankVariant?: "competitive" | "arena";
  highlights: Set<HighlightMetric>;
  expanded?: boolean;
  onToggle?: () => void;
  onSelectPlayerSummary?: (player: PlayerRow, trigger: HTMLButtonElement) => void;
  onQueryPlayer?: (battleTag: string) => void;
}) {
  const rankIcon =
    rankVariant === "arena"
      ? arenaRankAssetUrl(player.rankKey)
      : rankVariant === "competitive"
        ? competitiveRankAssetUrl(player.rankKey)
        : null;
  const rankTierLabel = formatArenaRankTierLabel(player.rankLabel);
  const perks = showPerks
    ? player.perks.flatMap((perk) => {
        const info = resolvePerkInfo(perk.guid);
        return info ? [info] : [];
      })
    : [];
  const avatar = (
    <HeroAvatar heroKey={player.heroGuid || player.heroName} name={player.heroName} size="sm" />
  );
  const battleTag = normalizeBattleTag(player.name);
  const queryPlayerName = isValidBattleTag(battleTag) && onQueryPlayer
    ? <button type="button" className="matchTeamPlayerQueryButton" onClick={(event) => { event.stopPropagation(); onQueryPlayer(battleTag); }}>{player.name}</button>
    : <strong>{player.name}</strong>;
  const playerContent = (
    <>
      <span className="phase6CompactPlayerName">
        <span className="phase7CompactPlayerNameLine">
          {queryPlayerName}
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
        <small>K/D/A</small>
        <strong>
          <MatchTeamHighlight value={player.kills} active={highlights.has("kills")} />/{player.deaths}/{player.assists}
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
      <div className="phase6CompactPlayerRow phase7CompactPlayerExpandable" aria-expanded={expanded}>
        {onSelectPlayerSummary ? (
          <button
            type="button"
            className="phase6CompactAvatarPlate phase7CompactAvatarSummaryButton"
            onClick={(event) => onSelectPlayerSummary(player, event.currentTarget)}
            aria-label={`查看${player.name}的汇总卡片`}
          >
            {avatar}
          </button>
        ) : (
          <span className="phase6CompactAvatarPlate">{avatar}</span>
        )}
        <div
          role="button"
          tabIndex={0}
          className="phase7CompactPlayerRowToggle"
          onClick={onToggle}
          onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); onToggle(); } }}
          aria-expanded={expanded}
          aria-label={`${player.name}${expanded ? "收起角斗详情" : "展开角斗详情"}`}
        >
          {playerContent}
        </div>
      </div>
    );
  }

  return (
    <article className="phase6CompactPlayerRow">
      <span className="phase6CompactAvatarPlate">{avatar}</span>
      {playerContent}
    </article>
  );
}

function MatchTeamHighlight({ value, active }: { value: number; active: boolean }) {
  return <em className={active ? "phase6StatHighlight" : ""}>{value}</em>;
}

function compactPlayerKey(player: PlayerRow, index: number) {
  return `${player.side}-${player.bnetId || player.name}-${player.heroGuid || player.heroName}-${index}`;
}
