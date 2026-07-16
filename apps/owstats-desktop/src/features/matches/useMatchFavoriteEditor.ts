import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { deleteMatchFavorite, saveMatchFavorite } from "../../shared/api";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { MatchFavoriteView } from "../../shared/types";
import {
  createMatchFavoriteDraft,
  favoriteCharacterCount,
  isCurrentMatchFavoriteMutation,
  matchFavoriteDraftReducer,
  normalizeMatchFavorite,
  shouldResetMatchFavoriteDraft,
} from "./matchFavoriteState";

type PersistFavorite = (
  favorite: MatchFavoriteView | null,
) => void | Promise<void>;

export function useMatchFavoriteEditor(
  matchId: string | null | undefined,
  favorite: MatchFavoriteView | null | undefined,
  onPersisted: PersistFavorite,
) {
  const [state, dispatch] = useReducer(
    matchFavoriteDraftReducer,
    normalizeMatchFavorite(favorite),
    createMatchFavoriteDraft,
  );
  const previousMatchId = useRef(matchId);
  const currentMatchId = useRef(matchId);
  currentMatchId.current = matchId;
  const dirty = state.isFavorite && state.draftNote !== state.savedNote;
  const count = useMemo(
    () => favoriteCharacterCount(state.draftNote),
    [state.draftNote],
  );

  useEffect(() => {
    const shouldReset = shouldResetMatchFavoriteDraft({
      previousMatchId: previousMatchId.current,
      matchId,
      favorite,
      state,
    });
    previousMatchId.current = matchId;
    if (shouldReset) {
      dispatch({ type: "reset", favorite: normalizeMatchFavorite(favorite) });
    }
  }, [favorite, matchId]);

  const edit = useCallback((note: string) => {
    dispatch({ type: "edit", note });
  }, []);

  const favoriteMatch = useCallback(async () => {
    if (!matchId) return;
    const requestMatchId = matchId;
    dispatch({ type: "start" });
    try {
      const persisted = await saveMatchFavorite({
        matchId: requestMatchId,
        note: "",
      });
      if (
        !isCurrentMatchFavoriteMutation(requestMatchId, currentMatchId.current)
      ) return;
      dispatch({ type: "saved", favorite: persisted });
      await onPersisted(persisted);
    } catch (error) {
      if (!isCurrentMatchFavoriteMutation(requestMatchId, currentMatchId.current)) return;
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }, [matchId, onPersisted]);

  const saveNote = useCallback(async () => {
    if (!matchId || favoriteCharacterCount(state.draftNote) > 500) return;
    const requestMatchId = matchId;
    const note = state.draftNote;
    dispatch({ type: "start" });
    try {
      const persisted = await saveMatchFavorite({ matchId: requestMatchId, note });
      if (
        !isCurrentMatchFavoriteMutation(requestMatchId, currentMatchId.current)
      ) return;
      dispatch({ type: "saved", favorite: persisted });
      await onPersisted(persisted);
    } catch (error) {
      if (!isCurrentMatchFavoriteMutation(requestMatchId, currentMatchId.current)) return;
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }, [matchId, onPersisted, state.draftNote]);

  const unfavoriteMatch = useCallback(async () => {
    if (!matchId) return;
    const requestMatchId = matchId;
    dispatch({ type: "start" });
    try {
      await deleteMatchFavorite(requestMatchId);
      if (
        !isCurrentMatchFavoriteMutation(requestMatchId, currentMatchId.current)
      ) return;
      dispatch({ type: "deleted" });
      await onPersisted(null);
    } catch (error) {
      if (!isCurrentMatchFavoriteMutation(requestMatchId, currentMatchId.current)) return;
      dispatch({ type: "failure", message: productErrorMessage(error) });
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
    ...state,
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

export type MatchFavoriteController = ReturnType<
  typeof useMatchFavoriteEditor
>;
