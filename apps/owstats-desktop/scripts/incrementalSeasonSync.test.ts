import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { formatSyncNotice } from "../src/shared/phase6Shell.ts";
import type { SyncSummary } from "../src/shared/types.ts";

const baseSummary: SyncSummary = {
  season: 23,
  mode: "fullFetch",
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
        stopReason: null,
        watermarkReached: false,
      },
    ],
    quickScanLimited: false,
  },
  quickScanLimited: false,
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
    name: "full fetch completion takes precedence over unchanged quick copy",
    summary: baseSummary,
    expected: "S23 全量抓取完成：当前赛季共 120 场。",
  },
  {
    name: "quick update with no changes",
    summary: { ...baseSummary, mode: "quickUpdate" },
    expected: "S23 已检查最新对局，没有发现更新。",
  },
  {
    name: "quick update success",
    summary: { ...baseSummary, mode: "quickUpdate", newMatches: 4, missingDetails: 2, detailsRequested: 6, repairedDetails: 2, databaseMatches: 124 },
    expected: "S23 更新完成：新增 4 场，当前赛季共 124 场。",
  },
  {
    name: "limited quick update does not claim latest even when details fail",
    summary: {
      ...baseSummary,
      mode: "quickUpdate",
      quickScanLimited: true,
      diagnostics: { ...baseSummary.diagnostics, quickScanLimited: true },
      newMatches: 3,
      detailsRequested: 3,
      failedDetails: 2,
      databaseMatches: 123,
    },
    expected: "S23 已更新扫描范围内的数据；快速更新已达到扫描范围，如需完整校验请使用全量抓取。",
  },
  {
    name: "unsafe quick update uses the range-limited copy",
    summary: {
      ...baseSummary,
      mode: "quickUpdate",
      diagnostics: {
        ...baseSummary.diagnostics,
        pages: [{ ...baseSummary.diagnostics.pages[0], stopReason: "unsafe_response" }],
      },
    },
    expected: "S23 已更新扫描范围内的数据；快速更新已达到扫描范围，如需完整校验请使用全量抓取。",
  },
  {
    name: "new match detail failures",
    summary: { ...baseSummary, mode: "quickUpdate", newMatches: 3, detailsRequested: 3, failedDetails: 2, databaseMatches: 123 },
    expected: "S23 对局列表已更新，2 场详情获取失败，可再次操作重试。",
  },
  {
    name: "old missing detail repair failure",
    summary: { ...baseSummary, mode: "quickUpdate", missingDetails: 2, detailsRequested: 2, repairedDetails: 1, failedDetails: 1 },
    expected: "S23 对局列表已更新，1 场详情获取失败，可再次操作重试。",
  },
];

for (const testCase of cases) {
  assert.equal(formatSyncNotice(23, testCase.summary), testCase.expected, testCase.name);
}

const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const apiSource = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const typesSource = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");

assert.match(
  typesSource,
  /export\s+type\s+SyncMode\s*=\s*"quickUpdate"\s*\|\s*"fullFetch"\s*;/,
  "shared types expose the explicit season sync modes",
);
assert.match(
  apiSource,
  /export\s+function\s+syncSeason\s*\(\s*season\s*:\s*number\s*,\s*mode\s*:\s*SyncMode\s*\)/,
  "syncSeason requires an explicit sync mode",
);
assert.match(
  apiSource,
  /invoke\s*<\s*SyncSummary\s*>\s*\(\s*"sync_season"\s*,\s*\{\s*request\s*:\s*\{\s*season\s*,\s*mode\s*\}\s*\}\s*\)/,
  "syncSeason forwards season and mode in the invoke request",
);
assert.match(
  onboardingSource,
  /const summary = await syncSeason\(season, "fullFetch"\);\s*clearMatchesPageCache\(\);\s*await loadDashboard\(\);/s,
  "successful sync preserves sync, cache clear, dashboard reload order",
);
assert.match(
  onboardingSource,
  /setSyncNotice\(formatSyncNotice\(season, summary\)\);/,
  "Onboarding delegates incremental result copy to formatSyncNotice",
);
