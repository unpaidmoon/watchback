import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const matchStyles = readFileSync(new URL("../src/styles/features/matches.css", import.meta.url), "utf8");

assert.match(
  matchStyles,
  /\.phase7CompactArenaDetail \.phase7ArenaTab\.win:not\(\.active\),\s*\.phase6DetailPage > \.phase7ArenaSelector \.phase7ArenaTab\.win:not\(\.active\)\s*\{[^}]*background:\s*var\(--status-success-bg\)/s,
  "未选中的胜利回合应使用浅绿色成功状态背景",
);

console.log("arena round result style tests passed");
