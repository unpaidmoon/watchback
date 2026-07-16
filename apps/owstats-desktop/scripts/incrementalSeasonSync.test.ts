import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const typesSource = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");
const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");

assert.match(
  typesSource,
  /export type SyncPageDiagnostic = \{\s*matchKind: string;\s*gameMode: string;\s*page: number;\s*entries: number;\s*uniqueValidEntries: number;\s*newMatches: number;\s*missingDetails: number;\s*existingCompleteMatches: number;\s*earliestBeginTs: number \| null;\s*latestBeginTs: number \| null;\s*\};/s,
  "SyncPageDiagnostic mirrors the Rust camelCase protocol",
);
assert.match(
  typesSource,
  /export type SyncDiagnostics = \{\s*listDurationMs: number;\s*detailDurationMs: number;\s*persistDurationMs: number;\s*pages: SyncPageDiagnostic\[\];\s*\};/s,
  "SyncDiagnostics mirrors the Rust camelCase protocol",
);

for (const field of [
  "listEntriesSeen",
  "newMatches",
  "missingDetails",
  "detailsRequested",
  "repairedDetails",
  "databaseMatches",
  "databaseDetails",
] as const) {
  assert.match(typesSource, new RegExp(`${field}: number;`), `SyncSummary exposes ${field}`);
}
assert.match(typesSource, /diagnostics: SyncDiagnostics;/, "SyncSummary exposes diagnostics");
assert.match(typesSource, /importedMatches: number;/, "SyncSummary keeps importedMatches for compatibility");
assert.match(typesSource, /importedDetails: number;/, "SyncSummary keeps importedDetails for compatibility");

assert.match(
  onboardingSource,
  /const summary = await syncSeason\(season\);\s*clearMatchesPageCache\(\);\s*await loadDashboard\(\);/s,
  "successful sync preserves sync, cache clear, dashboard reload order",
);
assert.match(
  onboardingSource,
  /summary\.newMatches === 0 && summary\.missingDetails === 0[\s\S]*?`S\$\{season\} 已是最新数据，共校验 \$\{summary\.listEntriesSeen\} 场对局，无需重新获取详情。`/,
  "already-current notice reports the verified list count",
);
assert.match(
  onboardingSource,
  /`S\$\{season\} 更新完成：新增 \$\{summary\.newMatches\} 场，补全 \$\{summary\.repairedDetails\} 场详情，当前赛季共 \$\{summary\.databaseMatches\} 场。/,
  "incremental notice reports new, repaired, and database match counts",
);
assert.doesNotMatch(onboardingSource, /summary\.importedMatches/, "incremental UI does not report importedMatches");
assert.doesNotMatch(onboardingSource, /本次导入/, "incremental UI does not claim a full import");
