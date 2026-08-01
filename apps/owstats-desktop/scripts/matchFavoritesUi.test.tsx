import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MatchFavoriteEditor } from "../src/features/matches/MatchFavoriteEditor";
import {
  createMatchFavoriteDraft,
  createMatchFavoriteMutationGate,
  matchFavoriteDraftReducer,
} from "../src/features/matches/matchFavoriteState";
import type { MatchFavoriteController } from "../src/features/matches/useMatchFavoriteEditor";
import {
  guardUnsavedBeforeUnload,
  guardUnsavedNavigation,
  useUnsavedChangesGuard,
} from "../src/shared/UnsavedChangesGuard";

let cleanConfirmCount = 0;
let cleanPreventDefaultCount = 0;
assert.equal(
  guardUnsavedNavigation(
    { isDirty: false, confirmDiscard: () => {
      cleanConfirmCount += 1;
      return false;
    } },
    { preventDefault: () => {
      cleanPreventDefaultCount += 1;
    } },
  ),
  true,
);
assert.equal(cleanConfirmCount, 0);
assert.equal(cleanPreventDefaultCount, 0);
assert.equal(guardUnsavedNavigation(null), true);

let acceptedConfirmCount = 0;
assert.equal(
  guardUnsavedNavigation({
    isDirty: true,
    confirmDiscard: () => {
      acceptedConfirmCount += 1;
      return true;
    },
  }),
  true,
);
assert.equal(acceptedConfirmCount, 1);

let rejectedConfirmCount = 0;
let rejectedPreventDefaultCount = 0;
assert.equal(
  guardUnsavedNavigation(
    {
      isDirty: true,
      confirmDiscard: () => {
        rejectedConfirmCount += 1;
        return false;
      },
    },
    { preventDefault: () => {
      rejectedPreventDefaultCount += 1;
    } },
  ),
  false,
);
assert.equal(rejectedConfirmCount, 1);
assert.equal(rejectedPreventDefaultCount, 1);

let busyConfirmCount = 0;
let busyPreventDefaultCount = 0;
assert.equal(
  guardUnsavedNavigation(
    {
      isDirty: false,
      isBusy: true,
      confirmDiscard: () => {
        busyConfirmCount += 1;
        return true;
      },
    },
    {
      preventDefault: () => {
        busyPreventDefaultCount += 1;
      },
    },
  ),
  false,
);
assert.equal(busyConfirmCount, 0);
assert.equal(busyPreventDefaultCount, 1);

const cleanUnloadEvent = {
  preventDefaultCount: 0,
  returnValue: "unchanged",
  preventDefault() {
    this.preventDefaultCount += 1;
  },
};
guardUnsavedBeforeUnload(
  { isDirty: false, confirmDiscard: () => true },
  cleanUnloadEvent,
);
assert.equal(cleanUnloadEvent.preventDefaultCount, 0);
assert.equal(cleanUnloadEvent.returnValue, "unchanged");

const dirtyUnloadEvent = {
  preventDefaultCount: 0,
  returnValue: "unchanged",
  preventDefault() {
    this.preventDefaultCount += 1;
  },
};
guardUnsavedBeforeUnload(
  { isDirty: true, confirmDiscard: () => true },
  dirtyUnloadEvent,
);
assert.equal(dirtyUnloadEvent.preventDefaultCount, 1);
assert.equal(dirtyUnloadEvent.returnValue, "");

const busyUnloadEvent = {
  preventDefaultCount: 0,
  returnValue: "unchanged",
  preventDefault() {
    this.preventDefaultCount += 1;
  },
};
guardUnsavedBeforeUnload(
  { isDirty: false, isBusy: true, confirmDiscard: () => true },
  busyUnloadEvent,
);
assert.equal(busyUnloadEvent.preventDefaultCount, 1);
assert.equal(busyUnloadEvent.returnValue, "");

function MissingProviderProbe() {
  useUnsavedChangesGuard();
  return null;
}
assert.throws(
  () => renderToStaticMarkup(createElement(MissingProviderProbe)),
  /UnsavedChangesGuardProvider is missing/,
);

const emptyController: MatchFavoriteController = {
  savedNote: "",
  draftNote: "",
  isFavorite: false,
  busy: false,
  error: null,
  saved: false,
  dirty: false,
  count: 0,
  canSave: false,
  edit: () => undefined,
  favoriteMatch: async () => undefined,
  saveNote: async () => undefined,
  unfavoriteMatch: async () => undefined,
  confirmDiscard: () => true,
};
const emptyEditor = renderToStaticMarkup(
  createElement(MatchFavoriteEditor, { controller: emptyController }),
);
assert.match(emptyEditor, /aria-label="对局收藏"/);
assert.match(emptyEditor, /收藏此对局/);
assert.doesNotMatch(emptyEditor, /对局备注/);
const failedEmptyEditor = renderToStaticMarkup(
  createElement(MatchFavoriteEditor, {
    controller: { ...emptyController, error: "收藏失败" },
  }),
);
assert.match(failedEmptyEditor, /class="matchFavoriteError" role="alert"/);

const favoriteController: MatchFavoriteController = {
  ...emptyController,
  savedNote: "保持高台",
  draftNote: "保持高台",
  isFavorite: true,
  count: 4,
};
const favoriteEditor = renderToStaticMarkup(
  createElement(MatchFavoriteEditor, { controller: favoriteController }),
);
assert.match(favoriteEditor, /aria-label="对局收藏与备注"/);
assert.match(favoriteEditor, /对局备注/);
assert.match(favoriteEditor, /保存备注/);
assert.match(favoriteEditor, /已收藏/);
assert.match(favoriteEditor, /class="matchFavoriteNoteField"/);
assert.match(favoriteEditor, /class="matchFavoriteActions"/);
assert.match(favoriteEditor, /class="secondaryButton"/);
assert.match(favoriteEditor, /class="primaryButton"/);

const unicodeNote = "复".repeat(500);
const unicodeErrorController: MatchFavoriteController = {
  ...favoriteController,
  savedNote: "已保存的匿名短备注。",
  draftNote: unicodeNote,
  dirty: true,
  count: 500,
  canSave: true,
  saved: false,
  error: "匿名保存错误，请稍后重试。",
};
const unicodeEditor = renderToStaticMarkup(
  createElement(MatchFavoriteEditor, {
    controller: unicodeErrorController,
  }),
);
assert.match(unicodeEditor, new RegExp(`>${unicodeNote}</textarea>`), "500 Unicode characters should render without truncation");
assert.match(unicodeEditor, />500\/500</);
assert.doesNotMatch(unicodeEditor, /role="status">已保存</);
assert.match(unicodeEditor, /class="matchFavoriteError" role="alert">匿名保存错误，请稍后重试。/);
assert.doesNotMatch(unicodeEditor, /is-over-limit/);

const savedEditor = renderToStaticMarkup(
  createElement(MatchFavoriteEditor, {
    controller: {
      ...favoriteController,
      savedNote: unicodeNote,
      draftNote: unicodeNote,
      dirty: false,
      count: 500,
      canSave: false,
      saved: true,
      error: null,
    },
  }),
);
assert.match(savedEditor, /role="status">已保存</);
assert.doesNotMatch(savedEditor, /role="alert"/);
assert.match(savedEditor, /<button[^>]*class="primaryButton" disabled=""/);

const overLimitEditor = renderToStaticMarkup(
  createElement(MatchFavoriteEditor, {
    controller: {
      ...favoriteController,
      savedNote: "已保存的匿名短备注。",
      draftNote: `${unicodeNote}复`,
      dirty: true,
      count: 501,
      canSave: false,
      saved: false,
      error: null,
    },
  }),
);
assert.match(overLimitEditor, /class="is-over-limit">501\/500</);
assert.match(overLimitEditor, /<button[^>]*class="primaryButton" disabled=""/);

const mutationGate = createMatchFavoriteMutationGate("match-a");
const saveToken = mutationGate.begin("match-a");
assert.ok(saveToken);
assert.equal(
  mutationGate.begin("match-a"),
  null,
  "save and delete actions must not overlap",
);

assert.equal(mutationGate.commitMatch("match-b"), true);
const deleteToken = mutationGate.begin("match-b");
assert.ok(deleteToken);
assert.equal(
  mutationGate.isCurrent(saveToken),
  false,
  "a completed request from the previous match must not update the selected match",
);
assert.equal(
  mutationGate.end(saveToken),
  false,
  "a stale request must not release the active match mutation",
);
assert.equal(mutationGate.begin("match-b"), null);
assert.equal(mutationGate.end(deleteToken), true);

const savingDraft = matchFavoriteDraftReducer(
  createMatchFavoriteDraft({ note: "saved note" }),
  { type: "edit", note: "new note" },
);
const saveFailed = matchFavoriteDraftReducer(
  matchFavoriteDraftReducer(savingDraft, { type: "start" }),
  { type: "failure", message: "save failed" },
);
assert.equal(saveFailed.isFavorite, true);
assert.equal(saveFailed.savedNote, "saved note");
assert.equal(saveFailed.draftNote, "new note");
assert.equal(saveFailed.error, "save failed");
assert.equal(saveFailed.busy, false);

const savedAfterRetry = matchFavoriteDraftReducer(saveFailed, {
  type: "saved",
  favorite: { note: "new note" },
});
assert.equal(savedAfterRetry.savedNote, "new note");
assert.equal(savedAfterRetry.draftNote, "new note");
assert.equal(savedAfterRetry.saved, true);
assert.equal(savedAfterRetry.error, null);

const deleted = matchFavoriteDraftReducer(savedAfterRetry, { type: "deleted" });
assert.deepEqual(deleted, createMatchFavoriteDraft(null));

console.log("match favorite UI behavior passed");
