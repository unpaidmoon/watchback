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
  const result = resultPresentation(match.result);
  return (
    <button
      type="button"
      className={`matchListCard matchRow phase6MatchRowReal ${match.result} ${selected ? "phase6MatchRowSelected" : ""}`}
      data-match-id={match.matchId}
      aria-current={selected ? "true" : undefined}
      onClick={() => onSelect(match.matchId)}
    >
      <span className="matchListCardSummary">
        <MapPreview mapKey={formatMatchPreviewKey(match)} name={match.mapName} className="phase6MatchMapThumb matchListCardMap" />
        <span className="matchListCardCopy">
          <span className="matchListCardResultScore">
            <span className={`matchListCardResult result-${result.tone}`}>
              <span className="matchListCardResultMarker" aria-hidden="true" />
              {result.label}
            </span>
            <strong className="matchListCardScore" data-numeric>{formatScoreLine(match)}</strong>
          </span>
          <strong className="matchListCardTitle">{formatMatchBannerTitle(match)}</strong>
          <span className="matchListCardMeta">
            <span>{match.beginLabel}</span>
            <span className="phase6MatchListHeroLine matchListCardHeroLine">
              <HeroAvatar heroKey={match.heroGuid || match.heroName} name={match.heroName} />
              <strong className="matchListCardHeroName">{match.heroName}</strong>
              {match.evaluation ? (
                <span className={`phase6VerticalPerformanceTag badgeTone-${listPerformanceTone(match.evaluation.primaryTag)}`}>{match.evaluation.primaryTag}</span>
              ) : (
                <span className="phase6VerticalPerformanceTag badgeTone-blue">常规</span>
              )}
            </span>
          </span>
        </span>
      </span>
      <span className="phase6MatchDataPanel matchListCardData">
        <span className="phase6MatchListStats matchListCardStats">
          <MetricBlock label="K/D/A" value={`${match.kills}/${match.deaths}/${match.assists}`} />
          <MetricBlock label="伤害" value={formatCompactStat(match.damage)} />
          <MetricBlock label="承伤" value={formatCompactStat(match.mitigation)} />
        </span>
        <span className="phase6MatchOpen">›</span>
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
