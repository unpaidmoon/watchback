import { AlertCircle, CircleDollarSign } from "lucide-react";
import { useEffect, useState } from "react";
import { arenaHeaderMapKey, arenaHeaderTitle, arenaResultSubLabel, formatArenaRankTierLabel, formatArenaScore, formatArenaTabLabel } from "../../shared/arenaDetailFormat";
import { arenaRankAssetUrl, resolveArenaModTraitInfo, resolvePerkInfo } from "../../shared/assets";
import { MatchPerformanceBadges } from "../../shared/PerformanceBadgesView";
import { formatDetailDuration, formatDetailStartTime, formatFinalScore, resultEnglishLabel } from "../../shared/matchDetailHeaderFormat";
import { playerHighlightMap, playerKeyFor, type HighlightMetric } from "../../shared/playerHighlights";
import { HeroAvatar, MapPreview } from "../../shared/phase6Components";
import type { MatchDetailView, PlayerRow } from "../../shared/types";
export function CompactMatchDetail({ detail, loading, error, matchId }: { detail: MatchDetailView | null; loading: boolean; error: string | null; matchId: string }) {
  if (error) {
    return (
      <div className="errorBanner phase6RecoverableError">
        <AlertCircle size={18} />
        <span>{error}</span>
      </div>
    );
  }
  if (loading && !detail) return <div className="emptyState phase6WorkbenchEmpty">正在读取对局详情...</div>;
  if (!detail) return <div className="emptyState phase6WorkbenchEmpty">未找到对局详情：{matchId}</div>;
  if (detail.arenaDetail) return <CompactArenaDetail detail={detail} loading={loading} />;

  const mapName = detail.mapName ?? "未知地图";
  const compactPlayers = [...detail.teammates.slice(0, 6), ...detail.enemies.slice(0, 6)];
  return (
    <section className={`phase6CompactDetail ${detail.result ?? "pending"}`}>
      <header className="phase6CompactHero">
        <MapPreview className="phase6CompactHeroMap" mapKey={detail.mapGuid ?? detail.mapName} name={mapName} />
        <div className="phase6CompactResult">
          <strong>{detail.resultLabel ?? "未知"}</strong>
          <span>{resultEnglishLabel(detail.result)}</span>
        </div>
        <div className="phase6CompactHeroText">
          <span>{mapName}</span>
          <strong>{formatFinalScore(detail)}</strong>
          <small>{detail.modeLabel ?? "-"} · {formatDetailDuration(detail.durationSec)} · {formatDetailStartTime(detail.beginTs)}</small>
        </div>
      </header>

      <div className="phase6CompactDetailScroll">
        <div className="phase6CompactFacts">
          <CompactFact label="最终比分" value={formatFinalScore(detail)} />
          <CompactFact label="游戏模式" value={detail.modeLabel ?? "-"} />
          <CompactFact label="游戏时长" value={formatDetailDuration(detail.durationSec)} />
          <CompactFact label="开始时间" value={formatDetailStartTime(detail.beginTs)} />
        </div>

        <div className="phase6CompactPerformance">
          <div>
            <span className="phase6Eyebrow">对局表现</span>
            <strong>评分与同队奖牌</strong>
          </div>
          <MatchPerformanceBadges evaluation={detail.evaluation} teamAwards={detail.teamAwards} />
        </div>

        <div className="phase6CompactPlayers" aria-label="双方玩家数据">
          <CompactTeam
            title="队友"
            tone="team"
            players={detail.teammates.slice(0, 6)}
          />
          <div className="phase6CompactVs">VS</div>
          <CompactTeam
            title="对手"
            tone="enemy"
            players={detail.enemies.slice(0, 6)}
          />
        </div>
        {compactPlayers.length === 0 && <div className="emptyState">该对局暂无玩家明细。</div>}
      </div>
    </section>
  );
}

function CompactArenaDetail({ detail, loading }: { detail: MatchDetailView; loading: boolean }) {
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
    <section className={`phase6CompactDetail phase7CompactArenaDetail ${selected.result ?? detail.result ?? "pending"}`}>
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

      <header className="phase6CompactHero">
        <MapPreview className="phase6CompactHeroMap" mapKey={arenaHeaderMapKey(selected)} name={selected.mapName ?? "未知地图"} />
        <div className="phase6CompactResult">
          <strong>{selected.resultLabel ?? "未知"}</strong>
          <span>{arenaResultSubLabel(selected.result ?? detail.result)}</span>
        </div>
        <div className="phase6CompactHeroText">
          <span>{arenaHeaderTitle(selected)}</span>
          <strong>{formatArenaScore(selected)}</strong>
          <small>{formatDetailDuration(selected.durationSec)} / {formatDetailStartTime(selected.beginTs)}</small>
        </div>
      </header>

      <div className="phase6CompactDetailScroll">
        {loading && <div className="emptyState">正在刷新角斗详情...</div>}
        <div className="phase6CompactFacts">
          <CompactFact label="比分" value={formatArenaScore(selected)} />
          <CompactFact label="游戏时长" value={formatDetailDuration(selected.durationSec)} />
          {selected.key !== "summary" && <CompactFact label="游戏模式" value={detail.modeLabel ?? "-"} />}
          <CompactFact label="开始时间" value={formatDetailStartTime(selected.beginTs)} />
        </div>

        <div className="phase6CompactPlayers" aria-label="角斗双方玩家数据" key={selected.key}>
          <CompactTeam
            title="队友"
            tone="team"
            players={selected.teammates}
            showPerks={false}
            economyMode={selected.key === "summary" ? "final" : "round"}
            expandedPlayerKeys={expandedCompactArenaPlayerKeys}
            onTogglePlayer={toggleCompactArenaPlayer}
          />
          <div className="phase6CompactVs">VS</div>
          <CompactTeam
            title="对手"
            tone="enemy"
            players={selected.enemies}
            showPerks={false}
            economyMode={selected.key === "summary" ? "final" : "round"}
            expandedPlayerKeys={expandedCompactArenaPlayerKeys}
            onTogglePlayer={toggleCompactArenaPlayer}
          />
        </div>
        {selected.teammates.length + selected.enemies.length === 0 && <div className="emptyState">该回合暂无玩家明细。</div>}
      </div>
    </section>
  );
}

function CompactFact({ label, value }: { label: string; value: string }) {
  return (
    <span>
      <small>{label}</small>
      <strong>{value}</strong>
    </span>
  );
}

function CompactTeam({
  title,
  players,
  tone,
  showPerks = true,
  economyMode,
  expandedPlayerKeys,
  onTogglePlayer,
}: {
  title: string;
  players: PlayerRow[];
  tone: "team" | "enemy";
  showPerks?: boolean;
  economyMode?: "final" | "round";
  expandedPlayerKeys?: ReadonlySet<string>;
  onTogglePlayer?: (playerKey: string) => void;
}) {
  const highlights = playerHighlightMap(players);
  return (
    <section className={`phase6CompactTeam ${tone}`}>
      <h3>{title}</h3>
      <div className="phase6CompactPlayerRows">
        {players.map((player, index) => {
          const key = compactPlayerKey(player, index);
          const expanded = expandedPlayerKeys?.has(key) ?? false;
          return (
            <div className="phase6CompactPlayerBlock" key={key}>
              <CompactPlayerRow
                player={player}
                showPerks={showPerks}
                highlights={highlights.get(playerKeyFor(player, index)) ?? new Set()}
                expanded={expanded}
                onToggle={onTogglePlayer ? () => onTogglePlayer(key) : undefined}
              />
              {expanded && economyMode && <CompactArenaPlayerExpansion player={player} economyMode={economyMode} />}
            </div>
          );
        })}
      </div>
    </section>
  );
}

function CompactPlayerRow({
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
          <CompactHighlight value={player.kills} active={highlights.has("kills")} />/{player.assists}/{player.deaths}
        </strong>
      </span>
      <span className="phase6CompactPlayerMetric phase6CompactPlayerMetricWide">
        <small>伤/疗/挡</small>
        <strong>
          <CompactHighlight value={player.damage} active={highlights.has("damage")} />/
          <CompactHighlight value={player.healing} active={highlights.has("healing")} />/
          <CompactHighlight value={player.mitigation} active={highlights.has("mitigation")} />
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

function CompactHighlight({ value, active }: { value: number; active: boolean }) {
  return <em className={active ? "phase6StatHighlight" : ""}>{value}</em>;
}

function compactPlayerKey(player: PlayerRow, index: number) {
  return `${player.side}-${player.bnetId || player.name}-${player.heroGuid || player.heroName}-${index}`;
}
