import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { explanationMetrics } from "../src/pages/FriendAnalyticsPage";
import type { FriendBoardEntry } from "../src/shared/types";

const metrics = explanationMetrics({
  boardKey: "healing_conversion",
  sameTeamMatches: 12,
  winRate: 0.75,
  avgDamageTaken: 1000,
  avgHealingTaken: 2000,
  avgMitigation: 300,
  avgDeaths: 4,
  avgKills: 18,
} as FriendBoardEntry);

assert.deepEqual(
  metrics.map((metric) => metric.label),
  ["同队", "受疗", "击杀"],
);

const friendPageSource = readFileSync(new URL("../src/pages/FriendAnalyticsPage.tsx", import.meta.url), "utf8");
assert.equal(
  friendPageSource.includes("好友亲密度"),
  true,
  "review friend summary should be renamed to friend intimacy",
);
assert.equal(
  friendPageSource.includes("commonFriendHeroes.slice(0, 5)"),
  true,
  "friend intimacy board should show the top five friends",
);
assert.equal(
  friendPageSource.includes("phase6FriendIntimacyBar"),
  true,
  "friend intimacy board should render a relative score bar",
);
assert.equal(
  friendPageSource.includes("组排关系热度"),
  false,
  "friend intimacy board should not show a secondary heat title",
);
assert.equal(
  friendPageSource.includes("识别 {topFriends.length} 位好友"),
  false,
  "friend intimacy board should not show the recognized friends summary",
);
assert.equal(
  friendPageSource.includes("最高亲密度"),
  false,
  "friend intimacy board should not show the max intimacy summary",
);
assert.equal(
  friendPageSource.includes("常用搭档英雄"),
  false,
  "friend intimacy cards should not show representative hero subtitles",
);
assert.equal(
  friendPageSource.includes("phase6FriendIntimacyRankBadge"),
  true,
  "friend intimacy rank should use the themed rank badge",
);
