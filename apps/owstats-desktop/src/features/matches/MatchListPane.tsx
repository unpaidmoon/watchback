import { forwardRef } from "react";
import type { MatchSummary } from "../../shared/types";
import { MatchTimeline } from "./MatchTimeline";

export type MatchListPaneProps = {
  matches: MatchSummary[];
  selectedMatchId: string | null;
  onSelect: (matchId: string) => void;
  onScroll: () => void;
};

export const MatchListPane = forwardRef<HTMLDivElement, MatchListPaneProps>(
  function MatchListPane(props, ref) {
    return <MatchTimeline ref={ref} {...props} />;
  },
);
