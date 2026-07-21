import assert from "node:assert/strict";
import { summarizeCommandCenterSeasons } from "../src/features/command-center/commandCenterTotals";
import type { SeasonDataStatus } from "../src/shared/types";

function seasonStatus(season: number, matchCount: number, detailCount: number): SeasonDataStatus {
  return {
    season,
    matchCount,
    detailCount,
    normalMatchCount: matchCount,
    arenaMatchCount: 0,
    failedDetailCount: matchCount - detailCount,
    lastSyncedOrImportedAt: null,
    coveragePercent: matchCount > 0 ? Math.round((detailCount / matchCount) * 100) : 0,
  };
}

const totals = summarizeCommandCenterSeasons([
  seasonStatus(23, 259, 259),
  seasonStatus(22, 941, 900),
  seasonStatus(21, 564, 500),
  seasonStatus(20, 329, 300),
  seasonStatus(15, 295, 200),
]);

assert.deepEqual(totals, {
  matchCount: 2388,
  detailCount: 2159,
  coveragePercent: 90,
});

assert.deepEqual(summarizeCommandCenterSeasons([]), {
  matchCount: 0,
  detailCount: 0,
  coveragePercent: 0,
});
