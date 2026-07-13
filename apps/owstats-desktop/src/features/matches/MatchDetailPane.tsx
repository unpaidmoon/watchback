import type { MatchDetailView } from "../../shared/types";
import { CompactMatchDetail, type CompactMatchDetailProps } from "./CompactMatchDetail";

export type MatchDetailPaneProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId: string | null;
  onBack: () => void;
} & Pick<CompactMatchDetailProps, "hasPrevious" | "hasNext" | "onPrevious" | "onNext">;

export function MatchDetailPane(props: MatchDetailPaneProps) {
  const { matchId, onBack, ...detailProps } = props;
  return (
    <aside className="matchDetailPane phase6WorkbenchDetail" aria-label="对局详情">
      <button type="button" className="ghostButton matchDetailBackButton" onClick={onBack}>返回对局列表</button>
      {matchId ? (
        <CompactMatchDetail {...detailProps} matchId={matchId} />
      ) : (
        <div className="emptyState phase6WorkbenchEmpty">选择左侧对局查看详情。</div>
      )}
    </aside>
  );
}
