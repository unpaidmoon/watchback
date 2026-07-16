import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const commands = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
const lib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const models = readFileSync(new URL("../src-tauri/src/models.rs", import.meta.url), "utf8");
const api = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const types = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");

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
