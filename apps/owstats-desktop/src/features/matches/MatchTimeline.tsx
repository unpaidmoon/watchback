import { forwardRef, Fragment } from "react";
import { formatCompactStat, formatMatchBannerTitle, formatMatchPreviewKey, formatScoreLine } from "../../shared/matchListFormat";
import { primaryTagBadgeTone } from "../../shared/matchPerformanceBadges";
import { HeroAvatar, MapPreview } from "../../shared/phase6Components";
import type { MatchSummary } from "../../shared/types";

type Props = {
  matches: MatchSummary[];
  onSelect: (matchId: string) => void;
  onScroll?: () => void;
  selectedMatchId?: string | null;
};

export const MatchTimeline = forwardRef<HTMLDivElement, Props>(function MatchTimeline({ matches, onSelect, onScroll, selectedMatchId }, ref) {
  if (matches.length === 0) {
    return <div className="emptyState">没有符合条件的预设 / 开放模式对局。</div>;
  }

  const streakLabels = streakSegmentLabels(matches);

  return (
    <div ref={ref} className="timeline phase6Timeline" aria-label="对局时间线" onScroll={onScroll}>
      {matches.map((match) => (
        <Fragment key={match.matchId}>
          {streakLabels.get(match.matchId) && <div className="streakSeparator">{streakLabels.get(match.matchId)}</div>}
          <button
            type="button"
            className={`matchRow phase6MatchRowReal ${match.result} ${selectedMatchId === match.matchId ? "phase6MatchRowSelected" : ""}`}
            data-match-id={match.matchId}
            aria-current={selectedMatchId === match.matchId ? "true" : undefined}
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
        </Fragment>
      ))}
    </div>
  );
});

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

function streakSegmentLabels(matches: MatchSummary[]) {
  const labels = new Map<string, string>();
  let index = 0;
  while (index < matches.length) {
    const result = matches[index].result;
    if (result !== "win" && result !== "loss") {
      index += 1;
      continue;
    }
    let end = index + 1;
    while (end < matches.length && matches[end].result === result) end += 1;
    const length = end - index;
    if (length >= 3) {
      labels.set(matches[index].matchId, `${length}${result === "win" ? "连胜" : "连败"}`);
    }
    index = end;
  }
  return labels;
}
