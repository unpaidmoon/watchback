import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const commands = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");

for (const command of [
  "get_friend_details",
  "get_personal_season_review",
  "get_season_analysis",
]) {
  assert.match(
    commands,
    new RegExp(`#\\[tauri::command\\(async\\)\\]\\s*pub fn ${command}\\b`),
    `${command} performs blocking SQLite aggregation and must not run on Tauri's main thread`,
  );
}

console.log("review command dispatch tests passed");
