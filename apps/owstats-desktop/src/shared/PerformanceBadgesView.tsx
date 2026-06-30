import { awardMedalText, gradeBadgeTone, normalizeMatchAwards, primaryTagBadgeTone } from "./matchPerformanceBadges";
import type { MatchEvaluationSummary, MatchPerformanceAward } from "./types";

type Props = {
  evaluation: MatchEvaluationSummary | null;
  teamAwards?: MatchPerformanceAward[];
  compact?: boolean;
};

export function MatchPerformanceBadges({ evaluation, teamAwards = [], compact = false }: Props) {
  const awards = normalizeMatchAwards(evaluation, teamAwards);
  if (!evaluation && awards.length === 0) {
    return (
      <span className="phase6PerformanceBadges empty" aria-label="对局表现">
        <strong className="gradePill gradePending">-</strong>
      </span>
    );
  }

  return (
    <span className={`phase6PerformanceBadges ${compact ? "compact" : ""}`} aria-label="对局表现">
      {evaluation && <span className={`primaryTag badgeTone-${primaryTagBadgeTone(evaluation.primaryTag)}`}>{evaluation.primaryTag}</span>}
      {evaluation && <strong className={`gradePill grade${evaluation.grade} badgeTone-${gradeBadgeTone(evaluation.grade)}`}>{evaluation.grade}</strong>}
      {awards.map((award) => (
        <span className={`phase6AwardMedal ${award.tone}`} title={award.label} aria-label={award.label} key={award.code}>
          {awardMedalText(award)}
        </span>
      ))}
    </span>
  );
}
