import { Star } from "lucide-react";
import type { MatchFavoriteController } from "./useMatchFavoriteEditor";

type MatchFavoriteEditorProps = {
  controller: MatchFavoriteController;
};

export function MatchFavoriteEditor({
  controller,
}: MatchFavoriteEditorProps) {
  const {
    isFavorite,
    draftNote,
    busy,
    error,
    saved,
    count,
    canSave,
    edit,
    favoriteMatch,
    saveNote,
    unfavoriteMatch,
  } = controller;

  if (!isFavorite) {
    return (
      <section className="matchFavoriteEditor is-empty" aria-label="对局收藏">
        {error ? <p className="matchFavoriteError" role="alert">{error}</p> : null}
        <button
          type="button"
          className="secondaryButton"
          disabled={busy}
          onClick={() => void favoriteMatch()}
        >
          <Star aria-hidden="true" />
          {busy ? "正在收藏..." : "收藏此对局"}
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
      <div className="matchFavoriteNoteField">
        <label htmlFor="match-favorite-note">对局备注</label>
        <textarea
          id="match-favorite-note"
          value={draftNote}
          disabled={busy}
          onChange={(event) => edit(event.currentTarget.value)}
        />
        <p className={overLimit ? "is-over-limit" : undefined}>
          {count}/500
        </p>
      </div>
      <div className="matchFavoriteActions">
        <button
          type="button"
          className="secondaryButton"
          disabled={busy}
          onClick={() => void unfavoriteMatch()}
        >
          <Star aria-hidden="true" fill="currentColor" />
          已收藏
        </button>
        <button
          type="button"
          className="primaryButton"
          disabled={!canSave}
          onClick={() => void saveNote()}
        >
          {busy ? "正在保存..." : "保存备注"}
        </button>
      </div>
      {saved ? <p role="status">已保存</p> : null}
      {error ? <p className="matchFavoriteError" role="alert">{error}</p> : null}
    </section>
  );
}
