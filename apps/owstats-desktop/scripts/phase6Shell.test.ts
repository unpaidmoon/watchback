import {
  commandCenterMetrics,
  credentialTone,
  hasDesktopBackend,
  phase6NavItems,
  productErrorMessage,
  readableSyncProgress,
} from "../src/shared/phase6Shell.ts";
import type { OnboardingStatus, SyncProgress } from "../src/shared/types.ts";

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

const navLabels = phase6NavItems.map((item) => item.label).join(",");
assertEqual(navLabels, "指挥台,对局,复盘,排行,分析,同步", "phase 6 navigation labels");

assertEqual(credentialTone("saved"), "good", "saved credential is good");
assertEqual(credentialTone("missing"), "warn", "missing credential needs attention");
assertEqual(credentialTone("failed"), "bad", "failed credential is bad");

const status: OnboardingStatus = {
  selectedSeason: 22,
  availableSeasons: [22],
  canEnterReview: true,
  credentialStatus: {
    state: "saved",
    message: "已保存登录凭据",
    exposesSecret: false,
    roleId: 123,
    savedAt: "2026-06-28T05:40:00Z",
  },
  localData: {
    dataSource: "sqlite",
    season: 22,
    presetMatches: 186,
    detailMatches: 142,
    lastSyncedOrImportedAt: "2026-06-28 05:40",
    freshnessRisk: "数据新鲜",
  },
  health: { readable: true, warnings: [] },
};

const metrics = commandCenterMetrics(status);
assertEqual(metrics[0].label, "登录状态", "first metric label");
assertEqual(metrics[0].value, "已登录", "credential metric value");
assertEqual(metrics[1].value, "S22", "season metric value");
assertEqual(metrics[2].value, "186", "match count value");
assertEqual(metrics[3].value, "142", "detail count value");

const progress: SyncProgress = {
  season: 22,
  phase: "details",
  processed: 8,
  total: 20,
  currentMatchId: null,
  failedDetails: 1,
  latestErrors: [],
};

assertEqual(readableSyncProgress(progress), "正在同步：8/20，失败详情 1", "sync progress text");
assertEqual(
  productErrorMessage("TypeError: Cannot read properties of undefined (reading 'invoke')"),
  "当前浏览器预览无法连接桌面后端，请在 Tauri 应用中使用登录和同步功能。",
  "tauri browser error becomes product copy",
);
assertEqual(hasDesktopBackend({}), false, "plain browser has no desktop backend");
assertEqual(hasDesktopBackend({ __TAURI_INTERNALS__: {} }), true, "tauri internals indicate desktop backend");
