import { formatPlayTimeHours, formatWinRate, modeLabel, sortRankingRows } from "../src/shared/rankingUtils.ts";
import type { RankingRow } from "../src/shared/types.ts";

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

function assertOrder(rows: RankingRow[], names: string[], message: string) {
  assertEqual(
    rows.map((row) => row.name).join(","),
    names.join(","),
    message,
  );
}

const rows: RankingRow[] = [
  { key: "a", name: "A", matches: 2, wins: 2, losses: 0, winRate: 1, playTimeHours: 0.5, adaptabilityLabel: null },
  { key: "b", name: "B", matches: 5, wins: 3, losses: 2, winRate: 0.6, playTimeHours: 2, adaptabilityLabel: null },
  { key: "c", name: "C", matches: 5, wins: 4, losses: 1, winRate: 0.8, playTimeHours: 1, adaptabilityLabel: null },
];

assertEqual(modeLabel("preset"), "预设 / 开放", "preset mode label");
assertEqual(modeLabel("stadium"), "体育场", "stadium mode label");
assertEqual(modeLabel("LeisurePreset"), "快速预设", "LeisurePreset mode label");
assertEqual(modeLabel("Leisure6v6"), "快速 6v6", "Leisure6v6 mode label");
assertEqual(modeLabel("SportPreset"), "竞技预设", "SportPreset mode label");
assertEqual(modeLabel("SportOpen"), "竞技开放", "SportOpen mode label");
assertEqual(modeLabel("custom"), "custom", "unknown mode label falls back");
assertEqual(formatWinRate(0.624), "62.4%", "win rate uses one decimal place");
assertEqual(formatWinRate(0.624, 0), "62%", "win rate can use integer display");
assertEqual(formatPlayTimeHours(3.25), "3.3 小时", "play time uses one decimal place");

assertOrder(sortRankingRows(rows, { key: "matches", direction: "desc" }), ["C", "B", "A"], "matches desc uses win rate tiebreak");
assertOrder(sortRankingRows(rows, { key: "wins", direction: "asc" }), ["A", "B", "C"], "wins asc");
assertOrder(sortRankingRows(rows, { key: "winRate", direction: "desc" }), ["A", "C", "B"], "win rate desc");
assertOrder(sortRankingRows(rows, { key: "playTimeHours", direction: "asc" }), ["A", "C", "B"], "play time asc");
