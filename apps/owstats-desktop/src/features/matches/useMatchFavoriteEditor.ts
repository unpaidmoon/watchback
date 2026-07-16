import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { deleteMatchFavorite, saveMatchFavorite } from "../../shared/api";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { MatchFavoriteView } from "../../shared/types";
import {
  createMatchFavoriteDraft,
  favoriteCharacterCount,
  matchFavoriteReducer,
  type MatchFavoriteDraft,
} from "./matchFavoriteState";

type PersistFavorite = (
  favorite: MatchFavoriteView | null,
) => void | Promise<void>;

export type MatchFavoriteEditorController = {
  state: MatchFavoriteDraft;
  dirty: boolean;
  count: number;
  canSave: boolean;
  edit: (note: string) => void;
  favoriteMatch: () => Promise<void>;
  saveNote: () => Promise<void>;
  unfavoriteMatch: () => Promise<void>;
  confirmDiscard: () => boolean;
};

export function useMatchFavoriteEditor(
  matchId: string | null | undefined,
  favorite: MatchFavoriteView | null,
  onPersisted: PersistFavorite,
): MatchFavoriteEditorController {
  const [state, dispatch] = useReducer(
    matchFavoriteReducer,
    favorite,
    createMatchFavoriteDraft,
  );
  const previousMatchId = useRef(matchId);
  const dirty = state.isFavorite && state.draftNote !== state.savedNote;
  const count = useMemo(
    () => favoriteCharacterCount(state.draftNote),
    [state.draftNote],
  );

  useEffect(() => {
    const matchChanged = previousMatchId.current !== matchId;
    previousMatchId.current = matchId;
    const incomingNote = favorite?.note ?? "";
    const incomingIsFavorite = favorite !== null;
    const incomingChanged =
      incomingIsFavorite !== state.isFavorite || incomingNote !== state.savedNote;

    if (matchChanged || (!dirty && incomingChanged)) {
      dispatch({ type: "reset", favorite });
    }
  }, [dirty, favorite, matchId, state.isFavorite, state.savedNote]);

  const edit = useCallback((note: string) => {
    dispatch({ type: "edit", note });
  }, []);

  const favoriteMatch = useCallback(async () => {
    if (!matchId) return;
    dispatch({ type: "start" });
    try {
      const persisted = await saveMatchFavorite({ matchId, note: "" });
      dispatch({ type: "saved", note: persisted.note });
      await onPersisted(persisted);
    } catch (error) {
      dispatch({ type: "failure", error: productErrorMessage(error) });
    }
  }, [matchId, onPersisted]);

  const saveNote = useCallback(async () => {
    if (!matchId || favoriteCharacterCount(state.draftNote) > 500) return;
    const note = state.draftNote;
    dispatch({ type: "start" });
    try {
      const persisted = await saveMatchFavorite({ matchId, note });
      dispatch({ type: "saved", note: persisted.note });
      await onPersisted(persisted);
    } catch (error) {
      dispatch({ type: "failure", error: productErrorMessage(error) });
    }
  }, [matchId, onPersisted, state.draftNote]);

  const unfavoriteMatch = useCallback(async () => {
    if (!matchId) return;
    dispatch({ type: "start" });
    try {
      await deleteMatchFavorite(matchId);
      dispatch({ type: "deleted" });
      await onPersisted(null);
    } catch (error) {
      dispatch({ type: "failure", error: productErrorMessage(error) });
    }
  }, [matchId, onPersisted]);

  const confirmDiscard = useCallback(() => {
    if (!dirty) return true;
    if (!window.confirm("备注尚未保存，是否放弃修改？")) return false;
    dispatch({
      type: "reset",
      favorite: { note: state.savedNote },
    });
    return true;
  }, [dirty, state.savedNote]);

  return {
    state,
    dirty,
    count,
    canSave: dirty && count <= 500 && !state.busy,
    edit,
    favoriteMatch,
    saveNote,
    unfavoriteMatch,
    confirmDiscard,
  };
}
