import assert from "node:assert/strict";
import { formatCompactStat, formatMatchBannerTitle, formatScoreLine } from "../src/shared/matchListFormat";
import type { MatchSummary } from "../src/shared/types";

const baseMatch: MatchSummary = {
  matchId: "match-1",
  season: 22,
  bucket: "preset",
  gameMode: "SportPreset",
  modeLabel: "闪点作战",
  result: "win",
  resultLabel: "胜",
  mapGuid: "suravasa",
  mapName: "苏拉瓦萨",
  heroGuid: "ana",
  heroName: "安娜",
  beginTs: 1800000000,
  beginLabel: "06-29 21:30",
  teamScore: 3,
  opponentScore: 0,
  kills: 13,
  assists: 7,
  deaths: 2,
  damage: 3047,
  healing: 15500,
  mitigation: 0,
  durationSec: 600,
  evaluation: null,
  teamAwards: [],
};

assert.equal(formatScoreLine(baseMatch), "3-0");
assert.equal(formatMatchBannerTitle(baseMatch), "苏拉瓦萨 · 闪点作战");
assert.equal(
  formatMatchBannerTitle({
    ...baseMatch,
    matchKind: "fight",
    gameMode: "SportFight",
    modeLabel: "角斗竞技",
    mapGuid: "576460752303428223",
    mapName: "绿洲城 - 花园",
  }),
  "绿洲城 - 花园 · 角斗竞技",
);
assert.equal(formatCompactStat(5960), "5,960");
assert.equal(formatCompactStat(10000), "10,000");
assert.equal(formatCompactStat(10001), "1.0w");
assert.equal(formatCompactStat(10499), "1.0w");
assert.equal(formatCompactStat(10500), "1.1w");
assert.equal(formatCompactStat(21499), "2.1w");
assert.equal(formatCompactStat(21500), "2.2w");
