import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MatchFavoriteEditor } from "../src/features/matches/MatchFavoriteEditor";
import type { MatchFavoriteController } from "../src/features/matches/useMatchFavoriteEditor";
import {
  guardUnsavedBeforeUnload,
  guardUnsavedNavigation,
  useUnsavedChangesGuard,
} from "../src/shared/UnsavedChangesGuard";

const commands = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
const lib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const models = readFileSync(new URL("../src-tauri/src/models.rs", import.meta.url), "utf8");
const api = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const types = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");
const guardUrl = new URL("../src/shared/UnsavedChangesGuard.tsx", import.meta.url);
assert.equal(existsSync(guardUrl), true, "the shared unsaved-changes guard must exist");
const guardSource = readFileSync(guardUrl, "utf8");
const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const mainSource = readFileSync(new URL("../src/main.tsx", import.meta.url), "utf8");
const matchesPageSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");
const compactDetailSource = ["MatchDetailPane.tsx", "MatchDetailContent.tsx"]
  .map((file) => readFileSync(new URL(`../src/features/matches/${file}`, import.meta.url), "utf8"))
  .join("\n");
const detailPaneSource = readFileSync(new URL("../src/features/matches/MatchDetailPane.tsx", import.meta.url), "utf8");
const favoriteEditorHookSource = readFileSync(new URL("../src/features/matches/useMatchFavoriteEditor.ts", import.meta.url), "utf8");
const matchesCss = readFileSync(new URL("../src/styles/features/matches.css", import.meta.url), "utf8");

assert.match(guardSource, /register/);
assert.match(guardSource, /guardNavigation/);
assert.match(guardSource, /beforeunload/);
assert.match(guardSource, /event\.preventDefault\(\)/);
assert.match(guardSource, /event\.returnValue\s*=\s*""/);
assert.doesNotMatch(guardSource, /useBlocker/);
assert.match(mainSource, /<HashRouter>[\s\S]*<UnsavedChangesProvider>[\s\S]*<App\s*\/>[\s\S]*<\/UnsavedChangesProvider>[\s\S]*<\/HashRouter>/);
assert.match(appSource, /useUnsavedChangesGuard/);
assert.match(appSource, /onClick=\{guardNavigation\}/);
assert.match(matchesPageSource, /useMatchFavoriteEditor/);
assert.match(matchesPageSource, /register\(favoriteController\.dirty\s*\|\|\s*favoriteController\.busy\s*\?\s*\{/);
assert.match(matchesPageSource, /isBusy:\s*favoriteController\.busy/);
assert.match(matchesPageSource, /favoriteController\.confirmDiscard\(\)/);
assert.match(
  favoriteEditorHookSource,
  /const confirmDiscard = useCallback\(\(\) => \{\s*if \(!canDiscardMatchFavoriteChanges\(\s*\{ busy: state\.busy, dirty \},[\s\S]*?\)\) return false;\s*if \(!dirty\) return true;/,
  "favorite, save-note, and delete mutations share one busy-aware controller transition guard",
);
assert.match(compactDetailSource, /favoriteController:\s*MatchFavoriteController/);
assert.match(detailPaneSource, /favoriteController/);
assert.doesNotMatch(compactDetailSource, /useMatchFavoriteEditor\s*\(/);
assert.doesNotMatch(detailPaneSource, /useMatchFavoriteEditor\s*\(/);
assert.match(
  compactDetailSource,
  /confirmDiscard:\s*favoriteController\.confirmDiscard[\s\S]*renderFavorite:\s*\(\) => <MatchFavoriteEditor/,
);

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

const saveCommandPattern =
  /#\[tauri::command\]\s*pub fn save_match_favorite\s*\(\s*accounts\s*:\s*State\s*<\s*'_\s*,\s*AccountContextManager\s*>\s*,\s*request\s*:\s*MatchFavoriteRequest\s*,?\s*\)\s*->\s*Result\s*<\s*MatchFavoriteView\s*,\s*String\s*>\s*\{/;
const deleteCommandPattern =
  /#\[tauri::command\]\s*pub fn delete_match_favorite\s*\(\s*accounts\s*:\s*State\s*<\s*'_\s*,\s*AccountContextManager\s*>\s*,\s*match_id\s*:\s*String\s*,?\s*\)\s*->\s*Result\s*<\s*\(\s*\)\s*,\s*String\s*>\s*\{/;

const saveDeclaration = commands.match(saveCommandPattern)?.[0];
const deleteDeclaration = commands.match(deleteCommandPattern)?.[0];
assert.ok(saveDeclaration, "save_match_favorite must expose the expected Tauri command signature");
assert.ok(deleteDeclaration, "delete_match_favorite must expose the expected Tauri command signature");
assert.doesNotMatch(
  saveDeclaration.replace(/#\[tauri::command\]/, ""),
  saveCommandPattern,
);
assert.doesNotMatch(
  saveDeclaration.replace(/request\s*:\s*MatchFavoriteRequest/, "request: String"),
  saveCommandPattern,
);
assert.doesNotMatch(
  saveDeclaration.replace(
    /Result\s*<\s*MatchFavoriteView\s*,\s*String\s*>/,
    "Result<(), String>",
  ),
  saveCommandPattern,
);
assert.doesNotMatch(
  deleteDeclaration.replace(/#\[tauri::command\]/, ""),
  deleteCommandPattern,
);
assert.doesNotMatch(
  deleteDeclaration.replace(/match_id\s*:\s*String/, "match_id: u64"),
  deleteCommandPattern,
);
assert.doesNotMatch(
  deleteDeclaration.replace(/Result\s*<\s*\(\s*\)\s*,\s*String\s*>/, "Result<bool, String>"),
  deleteCommandPattern,
);

for (const name of ["save_match_favorite", "delete_match_favorite"]) {
  assert.match(lib, new RegExp(`commands::${name}\\s*,`));
}

function serializedStructPattern(name: string) {
  return new RegExp(
    `#\\[serde\\s*\\(\\s*rename_all\\s*=\\s*"camelCase"\\s*\\)\\]\\s*pub struct ${name}\\s*\\{[^}]*\\}`,
  );
}

function serializedStruct(source: string, name: string) {
  const pattern = serializedStructPattern(name);
  const match = source.match(pattern);
  assert.ok(match, `${name} must be a camelCase serde struct`);
  return match[0];
}

const favoriteRequest = serializedStruct(models, "MatchFavoriteRequest");
assert.match(favoriteRequest, /pub\s+match_id\s*:\s*String\s*,/);
assert.match(favoriteRequest, /pub\s+note\s*:\s*String\s*,?/);
assert.doesNotMatch(
  favoriteRequest.replace(
    /rename_all\s*=\s*"camelCase"/,
    'rename_all = "snake_case"',
  ),
  serializedStructPattern("MatchFavoriteRequest"),
);
assert.doesNotMatch(
  favoriteRequest.replace(/pub\s+match_id\s*:\s*String/, "pub match_id: u64"),
  /pub\s+match_id\s*:\s*String\s*,/,
);
assert.doesNotMatch(
  favoriteRequest.replace(/pub\s+note\s*:\s*String/, "pub note: Option<String>"),
  /pub\s+note\s*:\s*String\s*,?/,
);

assert.match(
  api,
  /invoke<MatchFavoriteView>\("save_match_favorite",\s*\{\s*request\s*\}\)/,
);
assert.match(
  api,
  /invoke<void>\("delete_match_favorite",\s*\{\s*matchId\s*\}\)/,
);
assert.match(types, /favoriteOnly\?:\s*boolean/);
assert.match(types, /isFavorite:\s*boolean/);
assert.match(types, /favorite:\s*MatchFavoriteView\s*\|\s*null/);

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

assert.match(matchesCss, /\.matchFavoriteEditor\s*\{/);
assert.match(matchesCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto/);
assert.match(matchesCss, /\.matchFavoriteEditor\.is-empty\s*\{[^}]*justify-content:\s*flex-end/s);
assert.match(
  matchesCss,
  /\.matchFavoriteActions\s*\{(?=[^}]*display:\s*grid)(?=[^}]*align-self:\s*start)(?=[^}]*margin-top:\s*calc\(21px\s*\+\s*var\(--space-2\)\))[^}]*\}/s,
);
assert.match(
  matchesCss,
  /@media \(max-width:\s*1099px\)\s*\{[\s\S]*?\.matchFavoriteActions\s*\{(?=[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\))(?=[^}]*margin-top:\s*0)[^}]*\}/s,
);
assert.match(matchesCss, /\.matchFavoriteError,\s*\.matchFavoriteNoteField \.is-over-limit\s*\{[^}]*color:\s*var\(--status-danger-strong\)/s);
assert.doesNotMatch(
  matchesCss,
  /\.matchFavoriteError,\s*\.matchFavoriteNoteField \.is-over-limit\s*\{[^}]*color:\s*var\(--status-danger\)\s*;/s,
);
assert.match(matchesCss, /\.matchFavoriteEditor button\s*\{[^}]*min-height:\s*44px/s);
assert.match(
  matchesCss,
  /\.matchFavoriteEditor \.primaryButton\s*\{(?=[^}]*background:\s*var\(--action-primary\))(?=[^}]*color:\s*var\(--action-on-primary\))[^}]*\}/s,
);
assert.match(
  matchesCss,
  /\.matchFavoriteEditor \.secondaryButton\s*\{(?=[^}]*background:\s*var\(--surface-control\))(?=[^}]*color:\s*var\(--text-primary\))[^}]*\}/s,
);
assert.match(matchesCss, /\.matchFavoriteNoteField textarea:disabled\s*\{[^}]*cursor:\s*not-allowed/s);

const editorSource = readFileSync(
  new URL("../src/features/matches/MatchFavoriteEditor.tsx", import.meta.url),
  "utf8",
);
assert.doesNotMatch(editorSource, /localStorage/);
assert.doesNotMatch(editorSource, /rawJson/);
assert.doesNotMatch(editorSource, /maxLength/);
assert.doesNotMatch(editorSource, /matchListCard|matchRow/);

const editorHookSource = readFileSync(
  new URL("../src/features/matches/useMatchFavoriteEditor.ts", import.meta.url),
  "utf8",
);
assert.match(editorHookSource, /useLayoutEffect/);
assert.match(editorHookSource, /createMatchFavoriteMutationGate/);
assert.match(editorHookSource, /mutationGate\.begin\(/);
assert.match(editorHookSource, /mutationGate\.commitMatch\(/);
assert.match(editorHookSource, /mutationGate\.end\(/);
const favoriteSyncEffectDependencies = editorHookSource.match(
  /useEffect\(\(\) => \{[\s\S]*?\}, \[([\s\S]*?)\]\);/,
)?.[1];
assert.ok(favoriteSyncEffectDependencies);
assert.match(
  favoriteSyncEffectDependencies,
  /state\.busy/,
  "pending external favorites must resume synchronization when a mutation clears busy",
);
assert.match(
  editorHookSource,
  /收藏已保存，但页面状态刷新失败，请稍后重试。/,
);
assert.match(
  editorHookSource,
  /已取消收藏，但页面状态刷新失败，请稍后重试。/,
);

const listCardSource = readFileSync(
  new URL("../src/features/matches/MatchListCard.tsx", import.meta.url),
  "utf8",
);
assert.doesNotMatch(
  listCardSource,
  /MatchFavoriteEditor/,
  "the note editor must not be nested inside the clickable match list card",
);
