import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const commands = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
const lib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const api = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const types = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");

for (const name of ["save_match_favorite", "delete_match_favorite"]) {
  assert.match(commands, new RegExp(`pub fn ${name}\\s*\\(`));
  assert.match(lib, new RegExp(`commands::${name}\\s*,`));
}

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
