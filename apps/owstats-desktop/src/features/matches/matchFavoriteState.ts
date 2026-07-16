import type { MatchFavoriteView } from "../../shared/types";

export type MatchFavoriteDraft = {
  savedNote: string;
  draftNote: string;
  isFavorite: boolean;
  busy: boolean;
  error: string | null;
  saved: boolean;
};

export type MatchFavoriteDraftAction =
  | { type: "reset"; favorite: MatchFavoriteView | null }
  | { type: "edit"; note: string }
  | { type: "start" }
  | { type: "saved"; favorite: MatchFavoriteView }
  | { type: "deleted" }
  | { type: "failure"; message: string };

export function normalizeMatchFavorite(
  favorite: MatchFavoriteView | null | undefined,
): MatchFavoriteView | null {
  return favorite ?? null;
}

export function createMatchFavoriteDraft(
  favorite: MatchFavoriteView | null,
): MatchFavoriteDraft {
  const note = favorite?.note ?? "";
  return {
    savedNote: note,
    draftNote: note,
    isFavorite: favorite !== null,
    busy: false,
    error: null,
    saved: false,
  };
}

export function favoriteCharacterCount(value: string): number {
  return Array.from(value).length;
}

export function matchFavoriteDraftReducer(
  state: MatchFavoriteDraft,
  action: MatchFavoriteDraftAction,
): MatchFavoriteDraft {
  switch (action.type) {
    case "reset":
      return createMatchFavoriteDraft(action.favorite);
    case "edit":
      return { ...state, draftNote: action.note, error: null, saved: false };
    case "start":
      return { ...state, busy: true, error: null, saved: false };
    case "saved":
      return {
        ...createMatchFavoriteDraft(action.favorite),
        isFavorite: true,
        saved: true,
      };
    case "deleted":
      return createMatchFavoriteDraft(null);
    case "failure":
      return { ...state, busy: false, error: action.message, saved: false };
  }
}

export function shouldResetMatchFavoriteDraft({
  previousMatchId,
  matchId,
  favorite,
  state,
}: {
  previousMatchId: string | null | undefined;
  matchId: string | null | undefined;
  favorite: MatchFavoriteView | null | undefined;
  state: MatchFavoriteDraft;
}): boolean {
  if (previousMatchId !== matchId) return true;
  const normalizedFavorite = normalizeMatchFavorite(favorite);
  const dirty = state.isFavorite && state.draftNote !== state.savedNote;
  if (dirty) return false;
  return (
    (normalizedFavorite !== null) !== state.isFavorite ||
    (normalizedFavorite?.note ?? "") !== state.savedNote
  );
}

export type MatchFavoriteMutationGeneration = {
  matchId: string | null;
  generation: number;
};

export type MatchFavoriteMutationToken = MatchFavoriteMutationGeneration & {
  requestId: number;
};

export function advanceMatchFavoriteMutationGeneration(
  current: MatchFavoriteMutationGeneration,
  matchId: string | null | undefined,
): MatchFavoriteMutationGeneration {
  const normalizedMatchId = matchId ?? null;
  if (current.matchId === normalizedMatchId) return current;
  return {
    matchId: normalizedMatchId,
    generation: current.generation + 1,
  };
}

export function createMatchFavoriteMutationToken(
  current: MatchFavoriteMutationGeneration,
  requestId: number,
): MatchFavoriteMutationToken {
  return { ...current, requestId };
}

export function isCurrentMatchFavoriteMutationToken(
  token: MatchFavoriteMutationToken,
  current: MatchFavoriteMutationGeneration,
  currentRequestId: number,
): boolean {
  return (
    token.matchId === current.matchId &&
    token.generation === current.generation &&
    token.requestId === currentRequestId
  );
}
