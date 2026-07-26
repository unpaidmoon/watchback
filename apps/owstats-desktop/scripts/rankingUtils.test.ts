import { defaultRankingSort, formatPlayTimeHours, formatWinRate, modeLabel, sortRankingRows } from "../src/shared/rankingUtils.ts";
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
  { key: "a", name: "A", matches: 2, winRate: 1, playTimeHours: 0.5, kda: 2, rating: "B", bestHeroGuid: null, bestHeroName: null, adaptabilityLabel: null },
  { key: "b", name: "B", matches: 5, winRate: 0.6, playTimeHours: 2, kda: 3, rating: "S", bestHeroGuid: "hero-b", bestHeroName: "Hero B", adaptabilityLabel: null },
  { key: "c", name: "C", matches: 5, winRate: 0.8, playTimeHours: 1, kda: 1.5, rating: "A", bestHeroGuid: "hero-c", bestHeroName: "Hero C", adaptabilityLabel: null },
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
assertEqual(formatWinRate(62.4), "62.4%", "percentage-scale win rate remains percentage-scale");
assertEqual(formatPlayTimeHours(3.25), "3.3 小时", "play time uses one decimal place");
assertEqual(defaultRankingSort("heroes").key, "matches", "hero ranking defaults to matches");
assertEqual(defaultRankingSort("heroes").direction, "desc", "hero ranking defaults to matches desc");
assertEqual(defaultRankingSort("maps").key, "winRate", "map ranking defaults to win rate");
assertEqual(defaultRankingSort("maps").direction, "desc", "map ranking defaults to win rate desc");

assertOrder(sortRankingRows(rows, { key: "matches", direction: "desc" }), ["C", "B", "A"], "matches desc uses win rate tiebreak");
assertOrder(sortRankingRows(rows, { key: "winRate", direction: "desc" }), ["A", "C", "B"], "win rate desc");
assertOrder(sortRankingRows(rows, { key: "playTimeHours", direction: "asc" }), ["A", "C", "B"], "play time asc");
assertOrder(sortRankingRows(rows, { key: "kda", direction: "desc" }), ["B", "A", "C"], "kda desc");
assertOrder(sortRankingRows(rows, { key: "rating", direction: "desc" }), ["B", "C", "A"], "rating desc uses grade order");
