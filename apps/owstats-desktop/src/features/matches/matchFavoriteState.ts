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

export function canDiscardMatchFavoriteChanges(
  state: Pick<MatchFavoriteDraft, "busy"> & { dirty: boolean },
  confirmDiscard: () => boolean,
) {
  if (state.busy) return false;
  if (!state.dirty) return true;
  return confirmDiscard();
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

export type MatchFavoriteMutationToken = {
  matchId: string | null;
  generation: number;
  requestId: number;
};

export type MatchFavoriteMutationGate = {
  begin: (matchId: string) => MatchFavoriteMutationToken | null;
  commitMatch: (matchId: string | null | undefined) => boolean;
  hasInFlight: () => boolean;
  isCurrent: (token: MatchFavoriteMutationToken) => boolean;
  end: (token: MatchFavoriteMutationToken) => boolean;
};

export function createMatchFavoriteMutationGate(
  initialMatchId: string | null | undefined,
): MatchFavoriteMutationGate {
  let matchId = initialMatchId ?? null;
  let generation = 0;
  let nextRequestId = 0;
  let inFlight: MatchFavoriteMutationToken | null = null;

  function isCurrent(token: MatchFavoriteMutationToken): boolean {
    return (
      inFlight !== null &&
      token.matchId === matchId &&
      token.generation === generation &&
      token.requestId === inFlight.requestId
    );
  }

  return {
    begin(candidateMatchId) {
      if (inFlight !== null || candidateMatchId !== matchId) return null;
      nextRequestId += 1;
      inFlight = { matchId, generation, requestId: nextRequestId };
      return inFlight;
    },
    commitMatch(candidateMatchId) {
      const normalizedMatchId = candidateMatchId ?? null;
      if (normalizedMatchId === matchId) return false;
      matchId = normalizedMatchId;
      generation += 1;
      inFlight = null;
      return true;
    },
    hasInFlight() {
      return inFlight !== null;
    },
    isCurrent,
    end(token) {
      if (!isCurrent(token)) return false;
      inFlight = null;
      return true;
    },
  };
}
