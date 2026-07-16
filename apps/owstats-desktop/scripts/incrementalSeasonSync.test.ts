import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { formatSyncNotice } from "../src/shared/phase6Shell.ts";
import type { SyncSummary } from "../src/shared/types.ts";

const baseSummary: SyncSummary = {
  season: 23,
  phase: "completed",
  processed: 120,
  total: 120,
  currentMatchId: null,
  listEntriesSeen: 120,
  newMatches: 0,
  missingDetails: 0,
  detailsRequested: 0,
  repairedDetails: 0,
  databaseMatches: 120,
  databaseDetails: 120,
  diagnostics: {
    listDurationMs: 110,
    detailDurationMs: 220,
    persistDurationMs: 30,
    pages: [
      {
        matchKind: "normal",
        gameMode: "SportPreset",
        page: 1,
        entries: 120,
        uniqueValidEntries: 120,
        newMatches: 0,
        missingDetails: 0,
        existingCompleteMatches: 120,
        earliestBeginTs: 1_700_000_000_000,
        latestBeginTs: 1_700_100_000_000,
      },
    ],
  },
  importedMatches: 120,
  importedDetails: 120,
  skippedDuplicates: 0,
  failedDetails: 0,
  latestErrors: [],
  importSummary: {
    season: 23,
    matchesSeen: 120,
    detailsSeen: 120,
    presetMatches: 100,
    stadiumMatches: 20,
    importedAt: "2026-07-16T00:00:00Z",
  },
};

const cases: Array<{ name: string; summary: SyncSummary; expected: string }> = [
  {
    name: "no changes",
    summary: baseSummary,
    expected: "S23 已是最新数据，共校验 120 场对局，无需重新获取详情。",
  },
  {
    name: "new matches and repaired details",
    summary: { ...baseSummary, newMatches: 4, missingDetails: 2, detailsRequested: 6, repairedDetails: 2, databaseMatches: 124 },
    expected: "S23 更新完成：新增 4 场，补全 2 场详情，当前赛季共 124 场。",
  },
  {
    name: "new match detail failures",
    summary: { ...baseSummary, newMatches: 3, detailsRequested: 3, failedDetails: 2, databaseMatches: 123 },
    expected: "S23 对局列表已更新：新增 3 场，补全 0 场详情，2 场详情获取失败，可再次同步重试；当前赛季共 123 场。",
  },
  {
    name: "old missing detail repair failure",
    summary: { ...baseSummary, missingDetails: 2, detailsRequested: 2, repairedDetails: 1, failedDetails: 1 },
    expected: "S23 对局列表已更新：新增 0 场，补全 1 场详情，1 场详情获取失败，可再次同步重试；当前赛季共 120 场。",
  },
];

for (const testCase of cases) {
  assert.equal(formatSyncNotice(23, testCase.summary), testCase.expected, testCase.name);
}

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
assert.match(
  onboardingSource,
  /const summary = await syncSeason\(season\);\s*clearMatchesPageCache\(\);\s*await loadDashboard\(\);/s,
  "successful sync preserves sync, cache clear, dashboard reload order",
);
assert.match(
  onboardingSource,
  /setSyncNotice\(formatSyncNotice\(season, summary\)\);/,
  "Onboarding delegates incremental result copy to formatSyncNotice",
);
