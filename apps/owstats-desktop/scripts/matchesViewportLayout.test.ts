import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const matchesCss = readFileSync(
  new URL("../src/styles/features/matches.css", import.meta.url),
  "utf8",
);

const canvasRule = matchesCss.match(
  /\.appCanvas:has\(>\s*\.matchesPage\)\s*\{([^}]*)\}/s,
)?.[1] ?? "";

assert.match(
  matchesCss,
  /html:has\(\.matchesPage\),\s*body:has\(\.matchesPage\)\s*\{[^}]*overflow:\s*hidden\s*;/s,
  "对局页应锁定根滚动，只保留工作台面板内部滚动",
);
assert.match(canvasRule, /(?:^|;)\s*height:\s*0\s*;/, "对局页画布应以 flex 剩余空间作为唯一高度来源");
assert.match(canvasRule, /overflow:\s*hidden\s*;/, "对局页画布不得把 body 撑出额外滚动范围");
assert.match(
  matchesCss,
  /\.matchesWorkbench\.phase6MatchWorkbench\s*\{[^}]*height:\s*100%\s*;/s,
  "对局工作台应占满页面网格分配的剩余高度",
);

console.log("matches viewport layout contracts passed");
