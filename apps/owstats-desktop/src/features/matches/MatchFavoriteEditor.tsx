import { Star } from "lucide-react";
import type { MatchFavoriteEditorController } from "./useMatchFavoriteEditor";

type MatchFavoriteEditorProps = {
  controller: MatchFavoriteEditorController;
};

export function MatchFavoriteEditor({
  controller,
}: MatchFavoriteEditorProps) {
  const {
    state,
    count,
    canSave,
    edit,
    favoriteMatch,
    saveNote,
    unfavoriteMatch,
  } = controller;

  if (!state.isFavorite) {
    return (
      <section className="matchFavoriteEditor is-empty" aria-label="对局收藏">
        {state.error ? <p role="alert">{state.error}</p> : null}
        <button
          type="button"
          className="secondary"
          disabled={state.busy}
          onClick={() => void favoriteMatch()}
        >
          <Star aria-hidden="true" />
          {state.busy ? "正在收藏..." : "收藏此对局"}
        </button>
      </section>
    );
  }

  const overLimit = count > 500;
  return (
    <section
      className="matchFavoriteEditor is-favorite"
      aria-label="对局收藏与备注"
    >
      <label htmlFor="match-favorite-note">对局备注</label>
      <textarea
        id="match-favorite-note"
        value={state.draftNote}
        disabled={state.busy}
        onChange={(event) => edit(event.currentTarget.value)}
      />
      <p className={overLimit ? "is-over-limit" : undefined}>
        {count}/500
      </p>
      <div className="matchFavoriteEditor__actions">
        <button
          type="button"
          className="secondary"
          disabled={state.busy}
          onClick={() => void unfavoriteMatch()}
        >
          <Star aria-hidden="true" fill="currentColor" />
          已收藏
        </button>
        <button
          type="button"
          disabled={!canSave}
          onClick={() => void saveNote()}
        >
          {state.busy ? "正在保存..." : "保存备注"}
        </button>
      </div>
      {state.saved ? <p role="status">已保存</p> : null}
      {state.error ? <p role="alert">{state.error}</p> : null}
    </section>
  );
}
