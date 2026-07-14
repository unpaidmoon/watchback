import type { MatchSummary } from "../../shared/types";
import { MatchTimeline } from "./MatchTimeline";

export type MatchListPaneProps = {
  matches: MatchSummary[];
  selectedMatchId: string | null;
  onSelect: (matchId: string) => void;
};

export function MatchListPane(props: MatchListPaneProps) {
  return <MatchTimeline {...props} />;
}
