import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import { deleteMatchFavorite, saveMatchFavorite } from "../../shared/api";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { MatchFavoriteView } from "../../shared/types";
import {
  createMatchFavoriteDraft,
  createMatchFavoriteMutationGate,
  favoriteCharacterCount,
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
  const [mutationGate] = useState(() =>
    createMatchFavoriteMutationGate(matchId),
  );
  const previousFavorite = useRef(normalizeMatchFavorite(favorite));
  const pendingExternalFavorite = useRef(false);
  const dirty = state.isFavorite && state.draftNote !== state.savedNote;
  const count = useMemo(
    () => favoriteCharacterCount(state.draftNote),
    [state.draftNote],
  );

  useLayoutEffect(() => {
    if (!mutationGate.commitMatch(matchId)) return;
    const normalizedFavorite = normalizeMatchFavorite(favorite);
    pendingExternalFavorite.current = false;
    previousFavorite.current = normalizedFavorite;
    dispatch({ type: "reset", favorite: normalizedFavorite });
  }, [favorite, matchId, mutationGate]);

  useEffect(() => {
    const normalizedFavorite = normalizeMatchFavorite(favorite);
    const favoriteChanged =
      (previousFavorite.current !== null) !== (normalizedFavorite !== null) ||
      (previousFavorite.current?.note ?? "") !== (normalizedFavorite?.note ?? "");
    const shouldReset = shouldResetMatchFavoriteDraft({
      previousMatchId: matchId,
      matchId,
      favorite: normalizedFavorite,
      state,
    });
    previousFavorite.current = normalizedFavorite;

    const incomingDiffersFromState =
      (normalizedFavorite !== null) !== state.isFavorite ||
      (normalizedFavorite?.note ?? "") !== state.savedNote;
    if (favoriteChanged) {
      pendingExternalFavorite.current = incomingDiffersFromState;
    } else if (!incomingDiffersFromState) {
      pendingExternalFavorite.current = false;
    }
    if (mutationGate.hasInFlight()) return;
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

  const edit = useCallback((note: string) => {
    dispatch({ type: "edit", note });
  }, []);

  const favoriteMatch = useCallback(async () => {
    if (!matchId) return;
    const requestMatchId = matchId;
    const token = mutationGate.begin(requestMatchId);
    if (!token) return;
    dispatch({ type: "start" });
    try {
      let persisted: MatchFavoriteView;
      try {
        persisted = await saveMatchFavorite({
          matchId: requestMatchId,
          note: "",
        });
      } catch (error) {
        if (mutationGate.isCurrent(token)) {
          dispatch({ type: "failure", message: productErrorMessage(error) });
        }
        return;
      }
      if (!mutationGate.isCurrent(token)) return;
      try {
        await onPersisted(persisted);
      } catch {
        if (mutationGate.isCurrent(token)) {
          dispatch({ type: "saved", favorite: persisted });
          dispatch({
            type: "failure",
            message: "收藏已保存，但页面状态刷新失败，请稍后重试。",
          });
        }
        return;
      }
      if (mutationGate.isCurrent(token)) {
        dispatch({ type: "saved", favorite: persisted });
      }
    } finally {
      mutationGate.end(token);
    }
  }, [matchId, mutationGate, onPersisted]);

  const saveNote = useCallback(async () => {
    if (!matchId || favoriteCharacterCount(state.draftNote) > 500) return;
    const requestMatchId = matchId;
    const token = mutationGate.begin(requestMatchId);
    if (!token) return;
    const note = state.draftNote;
    dispatch({ type: "start" });
    try {
      let persisted: MatchFavoriteView;
      try {
        persisted = await saveMatchFavorite({ matchId: requestMatchId, note });
      } catch (error) {
        if (mutationGate.isCurrent(token)) {
          dispatch({ type: "failure", message: productErrorMessage(error) });
        }
        return;
      }
      if (!mutationGate.isCurrent(token)) return;
      try {
        await onPersisted(persisted);
      } catch {
        if (mutationGate.isCurrent(token)) {
          dispatch({ type: "saved", favorite: persisted });
          dispatch({
            type: "failure",
            message: "收藏已保存，但页面状态刷新失败，请稍后重试。",
          });
        }
        return;
      }
      if (mutationGate.isCurrent(token)) {
        dispatch({ type: "saved", favorite: persisted });
      }
    } finally {
      mutationGate.end(token);
    }
  }, [
    matchId,
    mutationGate,
    onPersisted,
    state.draftNote,
  ]);

  const unfavoriteMatch = useCallback(async () => {
    if (!matchId) return;
    const requestMatchId = matchId;
    const token = mutationGate.begin(requestMatchId);
    if (!token) return;
    dispatch({ type: "start" });
    try {
      try {
        await deleteMatchFavorite(requestMatchId);
      } catch (error) {
        if (mutationGate.isCurrent(token)) {
          dispatch({ type: "failure", message: productErrorMessage(error) });
        }
        return;
      }
      if (!mutationGate.isCurrent(token)) return;
      try {
        await onPersisted(null);
      } catch {
        if (mutationGate.isCurrent(token)) {
          dispatch({ type: "deleted" });
          dispatch({
            type: "failure",
            message: "已取消收藏，但页面状态刷新失败，请稍后重试。",
          });
        }
        return;
      }
      if (mutationGate.isCurrent(token)) {
        dispatch({ type: "deleted" });
      }
    } finally {
      mutationGate.end(token);
    }
  }, [matchId, mutationGate, onPersisted]);

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
