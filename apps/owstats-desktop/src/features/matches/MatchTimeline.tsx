import { forwardRef, Fragment } from "react";
import type { MatchSummary } from "../../shared/types";
import { MatchListCard } from "./MatchListCard";

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
          <MatchListCard
            match={match}
            selected={selectedMatchId === match.matchId}
            onSelect={onSelect}
          />
        </Fragment>
      ))}
    </div>
  );
});

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
