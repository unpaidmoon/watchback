import type { OnboardingStatus, SyncProgress } from "./types";

export type StatusTone = "neutral" | "good" | "warn" | "bad";

export type Phase6NavItem = {
  label: string;
  path: string;
  title: string;
};

export type CommandMetric = {
  label: string;
  value: string;
  meta: string;
};

export const phase6NavItems: Phase6NavItem[] = [
  { label: "对局", path: "/matches", title: "浏览和筛选对局" },
  { label: "战报", path: "/review", title: "个人赛季战报" },
  { label: "生涯 MMR", path: "/career-mmr", title: "本地估算的三职责生涯 MMR" },
  { label: "排行", path: "/ranking", title: "英雄与地图排行" },
  { label: "指挥台", path: "/onboarding", title: "登录、同步与数据状态" },
];

export function credentialTone(state: string | null | undefined): StatusTone {
  if (state === "ready") {
    return "good";
  }

  if (state === "failed" || state === "invalid") {
    return "bad";
  }

  if (state === "missing" || state === "not_found" || state === "incomplete" || state === "currentPlayerMissing") {
    return "warn";
  }

  return "neutral";
}

export function credentialLabel(state: string | null | undefined): string {
  if (state === "ready") {
    return "已登录";
  }

  if (state === "failed" || state === "invalid") {
    return "需重新登录";
  }

  if (state === "currentPlayerMissing") {
    return "待识别账号";
  }

  return "未登录";
}

export function readableSyncProgress(progress: SyncProgress | null): string {
  if (!progress) {
    return "等待同步";
  }

  const total = progress.total > 0 ? String(progress.total) : "-";
  return `正在同步：${progress.processed}/${total}，失败详情 ${progress.failedDetails}`;
}

export function commandCenterMetrics(status: OnboardingStatus | null): CommandMetric[] {
  return [
    {
      label: "登录状态",
      value: credentialLabel(status?.credentialStatus.state),
      meta: status?.credentialStatus.savedAt ? `保存于 ${status.credentialStatus.savedAt}` : "用于读取赛季报告",
    },
    {
      label: "本地赛季",
      value: status?.selectedSeason ? `S${status.selectedSeason}` : "-",
      meta: status?.availableSeasons.length ? `本地已有 ${status.availableSeasons.length} 个赛季` : "等待同步",
    },
    {
      label: "本地对局",
      value: String(status?.localData.presetMatches ?? 0),
      meta: "预设 / 开放模式",
    },
    {
      label: "可复盘详情",
      value: String(status?.localData.detailMatches ?? 0),
      meta: status?.localData.lastSyncedOrImportedAt ?? "暂无最近同步时间",
    },
  ];
}

export function productErrorMessage(error: unknown): string {
  const text = String(error);

  if (text.includes("invoke") || text.includes("__TAURI__") || text.includes("__TAURI_INTERNALS__")) {
    return "当前浏览器预览无法连接桌面后端，请在 Tauri 应用中使用登录和同步功能。";
  }

  const productText = text
    .replace(/^(?:(?:Error|TypeError|RangeError|ReferenceError|SyntaxError):\s*)+/i, "")
    .trim();
  return /[\u3400-\u9fff]/.test(productText)
    ? productText
    : "请求失败，请稍后重新加载。";
}

export function hasDesktopBackend(host: unknown = globalThis): boolean {
  return Boolean((host as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}
