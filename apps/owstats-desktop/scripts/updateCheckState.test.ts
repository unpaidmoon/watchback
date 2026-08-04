import assert from "node:assert/strict";
import { announcementFromUpdateResult } from "../src/features/update/useStartupUpdateCheck";
import type { UpdateCheckResult } from "../src/shared/types";

const base: Omit<UpdateCheckResult, "status" | "announcement"> = {
  currentVersion: "0.5.2",
};

const announcement = {
  latestVersion: "0.5.3",
  publishedAt: "2026-08-10",
  title: "Watchback v0.5.3 已发布",
  notes: ["新增程序内更新公告"],
};

assert.deepEqual(
  announcementFromUpdateResult({ ...base, status: "updateAvailable", announcement }),
  announcement,
);

for (const status of ["upToDate", "disabled", "unavailable"] as const) {
  assert.equal(
    announcementFromUpdateResult({ ...base, status, announcement: null }),
    null,
    `${status} must keep the startup dialog closed`,
  );
}

assert.equal(
  announcementFromUpdateResult({ ...base, status: "updateAvailable", announcement: null }),
  null,
  "malformed updateAvailable responses must stay closed",
);

console.log("update check state tests passed");
