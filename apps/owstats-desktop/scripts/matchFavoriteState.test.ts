import assert from "node:assert/strict";
import {
  canDiscardMatchFavoriteChanges,
  createMatchFavoriteDraft,
  createMatchFavoriteMutationGate,
  favoriteCharacterCount,
  matchFavoriteDraftReducer,
  normalizeMatchFavorite,
  shouldResetMatchFavoriteDraft,
} from "../src/features/matches/matchFavoriteState";

let busyConfirmCount = 0;
assert.equal(
  canDiscardMatchFavoriteChanges(
    { busy: true, dirty: true },
    () => {
      busyConfirmCount += 1;
      return true;
    },
  ),
  false,
);
assert.equal(busyConfirmCount, 0, "busy favorite mutations block transitions without prompting");

let cleanConfirmCount = 0;
assert.equal(
  canDiscardMatchFavoriteChanges(
    { busy: false, dirty: false },
    () => {
      cleanConfirmCount += 1;
      return false;
    },
  ),
  true,
);
assert.equal(cleanConfirmCount, 0);

let dirtyConfirmCount = 0;
assert.equal(
  canDiscardMatchFavoriteChanges(
    { busy: false, dirty: true },
    () => {
      dirtyConfirmCount += 1;
      return false;
    },
  ),
  false,
);
assert.equal(dirtyConfirmCount, 1);

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
const failedReconciliationAfterSave = matchFavoriteDraftReducer(
  matchFavoriteDraftReducer(edited, {
    type: "saved",
    favorite: { note: "后端已保存" },
  }),
  {
    type: "failure",
    message: "收藏已保存，但页面状态刷新失败，请稍后重试。",
  },
);
assert.equal(failedReconciliationAfterSave.isFavorite, true);
assert.equal(failedReconciliationAfterSave.savedNote, "后端已保存");
assert.equal(failedReconciliationAfterSave.draftNote, "后端已保存");
assert.equal(
  failedReconciliationAfterSave.error,
  "收藏已保存，但页面状态刷新失败，请稍后重试。",
);
const failedReconciliationAfterDelete = matchFavoriteDraftReducer(
  matchFavoriteDraftReducer(reset, { type: "deleted" }),
  {
    type: "failure",
    message: "已取消收藏，但页面状态刷新失败，请稍后重试。",
  },
);
assert.equal(failedReconciliationAfterDelete.isFavorite, false);
assert.equal(failedReconciliationAfterDelete.draftNote, "");
assert.equal(
  failedReconciliationAfterDelete.error,
  "已取消收藏，但页面状态刷新失败，请稍后重试。",
);

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

const gate = createMatchFavoriteMutationGate("match-a");
const oldMatchAToken = gate.begin("match-a");
assert.ok(oldMatchAToken);
assert.equal(gate.hasInFlight(), true);
assert.equal(
  gate.begin("match-a"),
  null,
  "a second mutation on the committed match is rejected while one is in flight",
);
const abortedCandidate = "match-b";
assert.equal(abortedCandidate, "match-b");
assert.equal(
  gate.isCurrent(oldMatchAToken),
  true,
  "an uncommitted render candidate does not advance the generation",
);
assert.equal(gate.end(oldMatchAToken), true);
assert.equal(gate.hasInFlight(), false);
const nextMatchAToken = gate.begin("match-a");
assert.ok(nextMatchAToken, "ending the current operation allows the next mutation");
assert.equal(gate.end(nextMatchAToken), true);

const firstGenerationAToken = gate.begin("match-a");
assert.ok(firstGenerationAToken);
assert.equal(gate.commitMatch("match-b"), true);
const matchBToken = gate.begin("match-b");
assert.ok(matchBToken);
assert.equal(
  gate.end(firstGenerationAToken),
  false,
  "an old A finally does not release the current B operation",
);
assert.equal(gate.begin("match-b"), null);
assert.equal(gate.commitMatch("match-a"), true);
const currentMatchAToken = gate.begin("match-a");
assert.ok(currentMatchAToken);
assert.equal(
  gate.isCurrent(firstGenerationAToken),
  false,
  "A -> B -> A invalidates the request from the first A generation",
);
assert.equal(
  gate.end(firstGenerationAToken),
  false,
  "ending an old A token does not release the current A operation",
);
assert.equal(
  gate.end(matchBToken),
  false,
  "ending an old B token does not release the current A operation",
);
assert.equal(
  gate.begin("match-a"),
  null,
  "the current A operation remains guarded after stale tokens end",
);
assert.equal(gate.end(currentMatchAToken), true);

const pendingSyncGate = createMatchFavoriteMutationGate("match-pending");
const pendingSyncToken = pendingSyncGate.begin("match-pending");
assert.ok(pendingSyncToken);
const cleanBusyState = {
  ...createMatchFavoriteDraft({ note: "旧值" }),
  busy: true,
};
assert.equal(
  shouldResetMatchFavoriteDraft({
    previousMatchId: "match-pending",
    matchId: "match-pending",
    favorite: { note: "外部新值" },
    state: cleanBusyState,
  }),
  true,
);
assert.equal(
  pendingSyncGate.hasInFlight(),
  true,
  "the effect defers the otherwise eligible external reset while mutation reconciliation is active",
);
const failedPendingState = matchFavoriteDraftReducer(cleanBusyState, {
  type: "failure",
  message: "请求失败",
});
assert.equal(pendingSyncGate.end(pendingSyncToken), true);
assert.equal(failedPendingState.busy, false);
assert.equal(
  shouldResetMatchFavoriteDraft({
    previousMatchId: "match-pending",
    matchId: "match-pending",
    favorite: { note: "外部新值" },
    state: failedPendingState,
  }),
  true,
  "after failure releases the gate, the pending clean external value remains eligible to reset",
);
