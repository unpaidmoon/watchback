import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import { deleteMatchFavorite, saveMatchFavorite } from "../../shared/api";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { MatchFavoriteView } from "../../shared/types";
import {
  advanceMatchFavoriteMutationGeneration,
  createMatchFavoriteMutationToken,
  createMatchFavoriteDraft,
  favoriteCharacterCount,
  isCurrentMatchFavoriteMutationToken,
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
  const previousFavorite = useRef(normalizeMatchFavorite(favorite));
  const pendingExternalFavorite = useRef(false);
  const mutationGeneration = useRef({
    matchId: matchId ?? null,
    generation: 0,
  });
  mutationGeneration.current = advanceMatchFavoriteMutationGeneration(
    mutationGeneration.current,
    matchId,
  );
  const mutationRequestId = useRef(0);
  const dirty = state.isFavorite && state.draftNote !== state.savedNote;
  const count = useMemo(
    () => favoriteCharacterCount(state.draftNote),
    [state.draftNote],
  );

  useEffect(() => {
    const normalizedFavorite = normalizeMatchFavorite(favorite);
    const matchChanged = previousMatchId.current !== matchId;
    const favoriteChanged =
      (previousFavorite.current !== null) !== (normalizedFavorite !== null) ||
      (previousFavorite.current?.note ?? "") !== (normalizedFavorite?.note ?? "");
    const shouldReset = shouldResetMatchFavoriteDraft({
      previousMatchId: previousMatchId.current,
      matchId,
      favorite: normalizedFavorite,
      state,
    });
    previousMatchId.current = matchId;
    previousFavorite.current = normalizedFavorite;

    if (matchChanged) {
      pendingExternalFavorite.current = false;
      dispatch({ type: "reset", favorite: normalizedFavorite });
      return;
    }

    const incomingDiffersFromState =
      (normalizedFavorite !== null) !== state.isFavorite ||
      (normalizedFavorite?.note ?? "") !== state.savedNote;
    if (favoriteChanged) {
      pendingExternalFavorite.current = incomingDiffersFromState;
    }
    if (
      shouldReset &&
      (favoriteChanged || pendingExternalFavorite.current)
    ) {
      pendingExternalFavorite.current = false;
      dispatch({ type: "reset", favorite: normalizedFavorite });
    }
  }, [
    favorite,
    matchId,
    state.draftNote,
    state.isFavorite,
    state.savedNote,
  ]);

  const beginMutation = useCallback(() => {
    mutationRequestId.current += 1;
    return createMatchFavoriteMutationToken(
      mutationGeneration.current,
      mutationRequestId.current,
    );
  }, []);

  const isCurrentMutation = useCallback(
    (token: ReturnType<typeof createMatchFavoriteMutationToken>) =>
      isCurrentMatchFavoriteMutationToken(
        token,
        mutationGeneration.current,
        mutationRequestId.current,
      ),
    [],
  );

  const edit = useCallback((note: string) => {
    dispatch({ type: "edit", note });
  }, []);

  const favoriteMatch = useCallback(async () => {
    if (!matchId) return;
    const requestMatchId = matchId;
    const token = beginMutation();
    dispatch({ type: "start" });
    try {
      const persisted = await saveMatchFavorite({
        matchId: requestMatchId,
        note: "",
      });
      if (!isCurrentMutation(token)) return;
      dispatch({ type: "saved", favorite: persisted });
      if (!isCurrentMutation(token)) return;
      await onPersisted(persisted);
    } catch (error) {
      if (!isCurrentMutation(token)) return;
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }, [beginMutation, isCurrentMutation, matchId, onPersisted]);

  const saveNote = useCallback(async () => {
    if (!matchId || favoriteCharacterCount(state.draftNote) > 500) return;
    const requestMatchId = matchId;
    const token = beginMutation();
    const note = state.draftNote;
    dispatch({ type: "start" });
    try {
      const persisted = await saveMatchFavorite({ matchId: requestMatchId, note });
      if (!isCurrentMutation(token)) return;
      dispatch({ type: "saved", favorite: persisted });
      if (!isCurrentMutation(token)) return;
      await onPersisted(persisted);
    } catch (error) {
      if (!isCurrentMutation(token)) return;
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }, [
    beginMutation,
    isCurrentMutation,
    matchId,
    onPersisted,
    state.draftNote,
  ]);

  const unfavoriteMatch = useCallback(async () => {
    if (!matchId) return;
    const requestMatchId = matchId;
    const token = beginMutation();
    dispatch({ type: "start" });
    try {
      await deleteMatchFavorite(requestMatchId);
      if (!isCurrentMutation(token)) return;
      dispatch({ type: "deleted" });
      if (!isCurrentMutation(token)) return;
      await onPersisted(null);
    } catch (error) {
      if (!isCurrentMutation(token)) return;
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }, [beginMutation, isCurrentMutation, matchId, onPersisted]);

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
