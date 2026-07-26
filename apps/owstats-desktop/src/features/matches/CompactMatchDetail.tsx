import type { MatchDetailView } from "../../shared/types";
import { AiMatchRoast } from "./AiMatchRoast";
import { MatchDetailContent } from "./MatchDetailContent";
import { MatchFavoriteEditor } from "./MatchFavoriteEditor";
import type { MatchFavoriteController } from "./useMatchFavoriteEditor";

export type CompactMatchDetailProps = {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId: string;
  onRetry: () => void;
  favoriteController: MatchFavoriteController;
  onQueryPlayer?: (battleTag: string) => void;
};

/** Local match-workbench adapter. Persistent capabilities deliberately live outside MatchDetailContent. */
export function CompactMatchDetail({ detail, loading, error, onRetry, favoriteController, onQueryPlayer }: CompactMatchDetailProps) {
  return MatchDetailContent({
    detail,
    loading,
    error,
    onRetry,
    onQueryPlayer,
    capabilities: {
      confirmDiscard: favoriteController.confirmDiscard,
      renderAi: (current) => (
        <AiMatchRoast matchId={current.matchId} initialRoast={current.aiRoast} detailReady={true} />
      ),
      renderFavorite: () => <MatchFavoriteEditor controller={favoriteController} />,
    },
  });
}
