import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MatchFavoriteEditor } from "../src/features/matches/MatchFavoriteEditor";
import type { MatchFavoriteController } from "../src/features/matches/useMatchFavoriteEditor";

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
const detailPageSource = readFileSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url), "utf8");
const compactDetailSource = readFileSync(new URL("../src/features/matches/CompactMatchDetail.tsx", import.meta.url), "utf8");
const detailPaneSource = readFileSync(new URL("../src/features/matches/MatchDetailPane.tsx", import.meta.url), "utf8");

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
assert.match(matchesPageSource, /register\(favoriteController\.dirty\s*\?\s*\{/);
assert.match(matchesPageSource, /favoriteController\.confirmDiscard\(\)/);
assert.match(detailPageSource, /useMatchFavoriteEditor/);
assert.match(detailPageSource, /register\(favoriteController\.dirty\s*\?\s*\{/);
assert.match(detailPageSource, /favoriteController\.confirmDiscard\(\)/);
assert.match(compactDetailSource, /favoriteController:\s*MatchFavoriteController/);
assert.match(detailPaneSource, /favoriteController/);
assert.doesNotMatch(compactDetailSource, /useMatchFavoriteEditor\s*\(/);
assert.doesNotMatch(detailPaneSource, /useMatchFavoriteEditor\s*\(/);

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
