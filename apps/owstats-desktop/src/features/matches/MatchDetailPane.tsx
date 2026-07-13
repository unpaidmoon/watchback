import type { MatchDetailView } from "../../shared/types";
import { CompactMatchDetail, type CompactMatchDetailProps } from "./CompactMatchDetail";

export type MatchDetailPaneProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId: string | null;
} & Pick<CompactMatchDetailProps, "hasPrevious" | "hasNext" | "onPrevious" | "onNext">;

export function MatchDetailPane(props: MatchDetailPaneProps) {
  const { matchId, ...detailProps } = props;
  return (
    <aside className="matchDetailPane phase6WorkbenchDetail" aria-label="对局详情">
      {matchId ? (
        <CompactMatchDetail {...detailProps} matchId={matchId} />
      ) : (
        <div className="emptyState phase6WorkbenchEmpty">选择左侧对局查看详情。</div>
      )}
    </aside>
  );
}
