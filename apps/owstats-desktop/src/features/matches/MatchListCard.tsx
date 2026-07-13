import { formatCompactStat, formatMatchBannerTitle, formatMatchPreviewKey, formatScoreLine } from "../../shared/matchListFormat";
import { primaryTagBadgeTone } from "../../shared/matchPerformanceBadges";
import { HeroAvatar, MapPreview } from "../../shared/phase6Components";
import type { MatchSummary } from "../../shared/types";

export type MatchListCardProps = {
  match: MatchSummary;
  selected: boolean;
  onSelect: (matchId: string) => void;
};

export function MatchListCard({ match, selected, onSelect }: MatchListCardProps) {
  return (
    <button
      type="button"
      className={`matchListCard matchRow phase6MatchRowReal ${match.result} ${selected ? "phase6MatchRowSelected" : ""}`}
      data-match-id={match.matchId}
      aria-current={selected ? "true" : undefined}
      onClick={() => onSelect(match.matchId)}
    >
      <span className="phase6MatchBanner">
        <MapPreview mapKey={formatMatchPreviewKey(match)} name={match.mapName} className="phase6MatchMapThumb" />
        <span className="phase6MatchScoreSlab">{formatScoreLine(match)}</span>
        <span className="phase6MatchMapTitle">{formatMatchBannerTitle(match)}</span>
      </span>
      <span className="phase6MatchDataPanel">
        <span className="phase6MatchListHeroLine">
          <span className="phase6HeroStackCell">
            <HeroAvatar heroKey={match.heroGuid || match.heroName} name={match.heroName} />
            {match.evaluation ? (
              <strong className={`phase6VerticalPerformanceTag badgeTone-${listPerformanceTone(match.evaluation.primaryTag)}`}>{match.evaluation.primaryTag}</strong>
            ) : (
              <strong className="phase6VerticalPerformanceTag badgeTone-blue">常规</strong>
            )}
          </span>
          <span className="phase6MatchListStats">
            <MetricBlock label="K/A/D" value={`${match.kills}/${match.assists}/${match.deaths}`} />
            <MetricBlock
              label="伤害 / 治疗 / 抵挡"
              value={`${formatCompactStat(match.damage)}/${formatCompactStat(match.healing)}/${formatCompactStat(match.mitigation)}`}
            />
          </span>
        </span>
        <span className="phase6MatchOpen">›</span>
      </span>
    </button>
  );
}

function listPerformanceTone(primaryTag: string) {
  if (primaryTagBadgeTone(primaryTag) === "gold") return "gold";
  if (primaryTag === "躺赢局" || primaryTag === "背锅局") return "danger";
  return "blue";
}

function MetricBlock({ label, value }: { label: string; value: string }) {
  return (
    <span className="phase6MatchMetricBlock">
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
  );
}
