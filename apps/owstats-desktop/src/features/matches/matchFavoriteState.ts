import type { MatchFavoriteView } from "../../shared/types";

export type MatchFavoriteDraft = {
  savedNote: string;
  draftNote: string;
  isFavorite: boolean;
  busy: boolean;
  error: string | null;
  saved: boolean;
};

export type MatchFavoriteAction =
  | { type: "reset"; favorite: MatchFavoriteView | null }
  | { type: "edit"; note: string }
  | { type: "start" }
  | { type: "saved"; note: string }
  | { type: "deleted" }
  | { type: "failure"; error: string };

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

export function matchFavoriteReducer(
  state: MatchFavoriteDraft,
  action: MatchFavoriteAction,
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
        savedNote: action.note,
        draftNote: action.note,
        isFavorite: true,
        busy: false,
        error: null,
        saved: true,
      };
    case "deleted":
      return createMatchFavoriteDraft(null);
    case "failure":
      return { ...state, busy: false, error: action.error, saved: false };
  }
}
