import type { MatchDetailView } from "../../shared/types";
import { AiMatchRoast } from "./AiMatchRoast";
import { MatchDetailContent } from "./MatchDetailContent";
import { MatchFavoriteEditor } from "./MatchFavoriteEditor";
import type { MatchFavoriteController } from "./useMatchFavoriteEditor";

export type MatchDetailPaneProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId: string | null;
  onBack?: () => void;
  onRetry: () => void;
  favoriteController: MatchFavoriteController;
  onQueryPlayer?: (battleTag: string) => void;
};

export function MatchDetailPane(props: MatchDetailPaneProps) {
  const { detail, loading, error, matchId, onBack = () => undefined, onRetry, favoriteController, onQueryPlayer } = props;
  return (
    <aside className="matchDetailPane phase6WorkbenchDetail" aria-label="对局详情">
      <button type="button" className="ghostButton matchDetailBackButton" onClick={onBack}>返回对局列表</button>
      {matchId ? (
        <MatchDetailContent
          detail={detail}
          loading={loading}
          error={error}
          onRetry={onRetry}
          onQueryPlayer={onQueryPlayer}
          capabilities={{
            confirmDiscard: favoriteController.confirmDiscard,
            renderAi: (current) => <AiMatchRoast matchId={current.matchId} initialRoast={current.aiRoast} detailReady={true} />,
            renderFavorite: () => <MatchFavoriteEditor controller={favoriteController} />,
          }}
        />
      ) : (
        <div className="emptyState phase6WorkbenchEmpty">选择左侧对局查看详情。</div>
      )}
    </aside>
  );
}
