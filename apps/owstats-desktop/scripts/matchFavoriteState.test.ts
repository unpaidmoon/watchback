import assert from "node:assert/strict";
import {
  createMatchFavoriteDraft,
  favoriteCharacterCount,
  matchFavoriteReducer,
} from "../src/features/matches/matchFavoriteState";

const empty = createMatchFavoriteDraft(null);
assert.deepEqual(empty, {
  savedNote: "",
  draftNote: "",
  isFavorite: false,
  busy: false,
  error: null,
  saved: false,
});

assert.deepEqual(createMatchFavoriteDraft({ note: "复盘🙂" }), {
  savedNote: "复盘🙂",
  draftNote: "复盘🙂",
  isFavorite: true,
  busy: false,
  error: null,
  saved: false,
});
assert.equal(favoriteCharacterCount("中文🙂"), 3);

const edited = matchFavoriteReducer(
  { ...createMatchFavoriteDraft({ note: "原备注" }), error: "旧错误", saved: true },
  { type: "edit", note: "新草稿" },
);
assert.equal(edited.savedNote, "原备注");
assert.equal(edited.draftNote, "新草稿");
assert.equal(edited.error, null);
assert.equal(edited.saved, false);

const started = matchFavoriteReducer(edited, { type: "start" });
assert.equal(started.busy, true);
assert.equal(started.error, null);
assert.equal(started.saved, false);

const failed = matchFavoriteReducer(started, { type: "failure", error: "保存失败" });
assert.equal(failed.busy, false);
assert.equal(failed.draftNote, "新草稿");
assert.equal(failed.savedNote, "原备注");
assert.equal(failed.error, "保存失败");

const saved = matchFavoriteReducer(failed, { type: "saved", note: "新草稿" });
assert.deepEqual(saved, {
  savedNote: "新草稿",
  draftNote: "新草稿",
  isFavorite: true,
  busy: false,
  error: null,
  saved: true,
});

const reset = matchFavoriteReducer(saved, { type: "reset", favorite: { note: "服务端备注" } });
assert.deepEqual(reset, createMatchFavoriteDraft({ note: "服务端备注" }));

const deleted = matchFavoriteReducer(reset, { type: "deleted" });
assert.deepEqual(deleted, createMatchFavoriteDraft(null));
