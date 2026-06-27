import type { MatchSummary } from "../../shared/types";

type Props = {
  matches: MatchSummary[];
  onSelect: (matchId: string) => void;
};

export function MatchTimeline({ matches, onSelect }: Props) {
  if (matches.length === 0) {
    return <div className="emptyState">没有符合条件的预设/开放模式对局。</div>;
  }

  return (
    <div className="timeline" aria-label="对局时间线">
      {matches.map((match) => (
        <button type="button" key={match.matchId} className={`matchRow ${match.result}`} onClick={() => onSelect(match.matchId)}>
          <span className="resultPill">{match.resultLabel}</span>
          <span className="matchMain">
            <strong>{match.mapName}</strong>
            <span>
              {match.heroName} · {match.modeLabel}
            </span>
          </span>
          <span className="score">
            {match.teamScore}-{match.opponentScore}
          </span>
          <span className="statLine">
            消灭/助攻/死亡 {match.kills}/{match.assists}/{match.deaths}
          </span>
          <span className="statLine">
            伤害 {formatNumber(match.damage)} · 治疗 {formatNumber(match.healing)} · 格挡 {formatNumber(match.mitigation)}
          </span>
          <time>{match.beginLabel}</time>
        </button>
      ))}
    </div>
  );
}

function formatNumber(value: number) {
  if (value >= 10000) {
    return `${(value / 10000).toFixed(1).replace(".0", "")}万`;
  }
  return String(value);
}
