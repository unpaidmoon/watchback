import assert from "node:assert/strict";
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
