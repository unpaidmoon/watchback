import { formatCompactStat, formatMatchPreviewKey, formatScoreLine } from "../../shared/matchListFormat";
import { MatchPerformanceBadges } from "../../shared/PerformanceBadgesView";
import { HeroAvatar, MapPreview } from "../../shared/phase6Components";
import type { MatchSummary } from "../../shared/types";

export type MatchListCardProps = {
  match: MatchSummary;
  selected: boolean;
  onSelect: (matchId: string) => void;
};

export function MatchListCard({ match, selected, onSelect }: MatchListCardProps) {
  const result = resultPresentation(match.result);
  return (
    <button
      type="button"
      className={`matchListCard matchRow phase6MatchRowReal ${match.result} ${selected ? "phase6MatchRowSelected" : ""}`}
      data-match-id={match.matchId}
      aria-current={selected ? "true" : undefined}
      onClick={() => onSelect(match.matchId)}
    >
      <span className="matchListCardHeader">
        <MapPreview
          mapKey={formatMatchPreviewKey(match)}
          name={match.mapName}
          className="matchListCardHeaderMap"
          decorative
        />
        <span className={`matchListCardResultBlock result-${result.tone}`}>
          <span>{result.label}</span>
          <strong data-numeric>{formatScoreLine(match)}</strong>
        </span>
        <span className="matchListCardBannerCopy" title={`${match.mapName} · ${match.modeLabel}`}>
          <span>{match.mapName}</span>
          <span>{match.modeLabel}</span>
        </span>
      </span>
      <span className="matchListCardData">
        <span className="matchListCardAvatar">
          <HeroAvatar heroKey={match.heroGuid || match.heroName} name={match.heroName} />
        </span>
        <MetricBlock label="K/A/D" value={`${match.kills}/${match.assists}/${match.deaths}`} />
        <MetricBlock
          className="matchListCardCombinedStats"
          label="伤害/治疗/抵挡"
          value={[match.damage, match.healing, match.mitigation].map(formatCompactStat).join("/")}
        />
        <span className="matchListCardPerformance">
          <span>对局表现</span>
          <MatchPerformanceBadges evaluation={match.evaluation} teamAwards={match.teamAwards} compact showGrade={false} />
        </span>
      </span>
    </button>
  );
}

function resultPresentation(result: string) {
  if (result === "win") return { tone: "win", label: "胜利" };
  if (result === "loss") return { tone: "loss", label: "失败" };
  if (result === "draw") return { tone: "draw", label: "平局" };
  return { tone: "neutral", label: "未知结果" };
}

function MetricBlock({ label, value, className = "" }: { label: string; value: string; className?: string }) {
  return (
    <span className={`phase6MatchMetricBlock ${className}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
  );
}
