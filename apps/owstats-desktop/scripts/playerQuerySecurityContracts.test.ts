import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { PlayerQueryCard, PlayerQueryCardImages, PlayerQueryMatchDetail, PlayerQuerySessionStart } from "../src/features/player-query/playerQueryTypes";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const inventory = JSON.parse(await readFile(path.join(appRoot, "runtime-config/assets-inventory.json"), "utf8")) as { files: string[] };
assert.ok(inventory.files.includes("maps/stadium.png"), "stadium preview must be copied by the tracked runtime asset inventory");

const dtoSource = await readFile(path.join(appRoot, "src/features/player-query/playerQueryTypes.ts"), "utf8");
for (const forbidden of ["customerToken", "bigdata", "authorization", "cookie", "rawJson", "remoteUrl"]) {
  assert.equal(dtoSource.toLowerCase().includes(forbidden.toLowerCase()), false, `query DTO source must not expose ${forbidden}`);
}

type WebViewBoundary = PlayerQuerySessionStart | PlayerQueryCard | PlayerQueryCardImages | PlayerQueryMatchDetail;
const samples: WebViewBoundary[] = [
  { sessionId: "pq-1", generation: 1, battleTag: "测试#1" },
  { battleTag: "测试#1", level: 1, gameTimeSeconds: 60, title: null },
  { icon: { mimeType: "image/png", dataBase64: "AQID" } },
];
for (const sample of samples) {
  assert.equal(/https?:\/\//i.test(JSON.stringify(sample)), false, "WebView DTO sample must use in-memory image bytes, never a remote URL");
}
console.log("player query security contract tests passed");
