import assert from "node:assert/strict";
import {
  advanceMatchFavoriteMutationGeneration,
  createMatchFavoriteMutationToken,
  createMatchFavoriteDraft,
  favoriteCharacterCount,
  isCurrentMatchFavoriteMutationToken,
  matchFavoriteDraftReducer,
  normalizeMatchFavorite,
  shouldResetMatchFavoriteDraft,
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

assert.equal(normalizeMatchFavorite(undefined), null);

const edited = matchFavoriteDraftReducer(
  { ...createMatchFavoriteDraft({ note: "原备注" }), error: "旧错误", saved: true },
  { type: "edit", note: "新草稿" },
);
assert.equal(edited.savedNote, "原备注");
assert.equal(edited.draftNote, "新草稿");
assert.equal(edited.error, null);
assert.equal(edited.saved, false);

const started = matchFavoriteDraftReducer(edited, { type: "start" });
assert.equal(started.busy, true);
assert.equal(started.error, null);
assert.equal(started.saved, false);

const failed = matchFavoriteDraftReducer(started, {
  type: "failure",
  message: "保存失败",
});
assert.equal(failed.busy, false);
assert.equal(failed.draftNote, "新草稿");
assert.equal(failed.savedNote, "原备注");
assert.equal(failed.error, "保存失败");

const saved = matchFavoriteDraftReducer(failed, {
  type: "saved",
  favorite: { note: "新草稿" },
});
assert.deepEqual(saved, {
  savedNote: "新草稿",
  draftNote: "新草稿",
  isFavorite: true,
  busy: false,
  error: null,
  saved: true,
});

const reset = matchFavoriteDraftReducer(saved, {
  type: "reset",
  favorite: { note: "服务端备注" },
});
assert.deepEqual(reset, createMatchFavoriteDraft({ note: "服务端备注" }));

const deleted = matchFavoriteDraftReducer(reset, { type: "deleted" });
assert.deepEqual(deleted, createMatchFavoriteDraft(null));

assert.equal(
  shouldResetMatchFavoriteDraft({
    previousMatchId: "match-a",
    matchId: "match-b",
    favorite: { note: "B 备注" },
    state: edited,
  }),
  true,
  "switching matches resets even when the previous match has a dirty draft",
);
assert.equal(
  shouldResetMatchFavoriteDraft({
    previousMatchId: "match-a",
    matchId: "match-a",
    favorite: { note: "外部更新" },
    state: edited,
  }),
  false,
  "same-match external updates do not overwrite a dirty draft",
);
assert.equal(
  shouldResetMatchFavoriteDraft({
    previousMatchId: "match-a",
    matchId: "match-a",
    favorite: { note: "外部更新" },
    state: { ...edited, draftNote: edited.savedNote },
  }),
  true,
  "same-match external updates synchronize when the draft is clean",
);

const cleanedAfterExternalUpdate = matchFavoriteDraftReducer(edited, {
  type: "edit",
  note: "原备注",
});
assert.equal(
  shouldResetMatchFavoriteDraft({
    previousMatchId: "match-a",
    matchId: "match-a",
    favorite: { note: "外部更新" },
    state: cleanedAfterExternalUpdate,
  }),
  true,
  "a pending external favorite synchronizes after the dirty draft becomes clean",
);

let generation = { matchId: "match-a" as string | null, generation: 0 };
const oldMatchAToken = createMatchFavoriteMutationToken(generation, 1);
generation = advanceMatchFavoriteMutationGeneration(generation, "match-b");
generation = advanceMatchFavoriteMutationGeneration(generation, "match-a");
assert.equal(generation.generation, 2);
assert.equal(
  isCurrentMatchFavoriteMutationToken(oldMatchAToken, generation, 1),
  false,
  "A -> B -> A invalidates the request from the first A generation",
);

const firstSameMatchToken = createMatchFavoriteMutationToken(generation, 2);
const secondSameMatchToken = createMatchFavoriteMutationToken(generation, 3);
assert.equal(
  isCurrentMatchFavoriteMutationToken(firstSameMatchToken, generation, 3),
  false,
  "a newer request on the same match invalidates the older request",
);
assert.equal(
  isCurrentMatchFavoriteMutationToken(secondSameMatchToken, generation, 3),
  true,
);
