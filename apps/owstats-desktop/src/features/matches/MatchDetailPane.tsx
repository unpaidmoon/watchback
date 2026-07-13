import type { MatchDetailView } from "../../shared/types";
import { CompactMatchDetail } from "./CompactMatchDetail";

export type MatchDetailPaneProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId: string | null;
};

export function MatchDetailPane(props: MatchDetailPaneProps) {
  return (
    <aside className="matchDetailPane phase6WorkbenchDetail" aria-label="对局详情">
      {props.matchId ? (
        <CompactMatchDetail {...props} matchId={props.matchId} />
      ) : (
        <div className="emptyState phase6WorkbenchEmpty">选择左侧对局查看详情。</div>
      )}
    </aside>
  );
}
