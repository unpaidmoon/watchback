import type { SeasonSyncResult, SyncProgress } from "./types";

export type Phase6NavItem = {
  label: string;
  path: string;
  title: string;
};

export const phase6NavItems: Phase6NavItem[] = [
  { label: "对局", path: "/matches", title: "浏览和筛选对局" },
  { label: "查询", path: "/player-query", title: "即时查询玩家数据" },
  { label: "战报", path: "/review", title: "个人赛季战报" },
  { label: "成就", path: "/achievements", title: "赛季成就与进度" },
  { label: "排行", path: "/ranking", title: "英雄与地图排行" },
  { label: "指挥台", path: "/onboarding", title: "登录、同步与数据状态" },
];

export function readableSyncProgress(progress: SyncProgress | null): string {
  if (!progress) {
    return "等待同步";
  }

  if (progress.total <= 0) {
    return progress.phase;
  }

  return `${progress.phase}：${progress.processed}/${progress.total}，失败详情 ${progress.failedDetails}`;
}

export function formatSyncNotice(season: number, result: SeasonSyncResult): string {
  if (result.kind === "quickList") {
    if (result.partialFailure) {
      return `S${season} 已保存成功获取的数据；部分模式检查失败，正在后台重试。`;
    }
    if (!result.dataChanged) {
      return `S${season} 已检查最新一页，没有发现新对局。`;
    }
    return `S${season} 已新增 ${result.newListMatches} 场对局，详情正在后台处理中。`;
  }
  const summary = result;
  const quickScanUnsafe = summary.diagnostics.pages.some((page) => page.stopReason === "unsafe_response");
  if (summary.mode === "quickUpdate" && quickScanUnsafe) {
    return `S${season} 快速更新遇到页面异常，无法确认已到达完整边界；已保留本地数据，如需完整校验请使用全量抓取。`;
  }

  if (summary.mode === "quickUpdate" && summary.quickScanLimited) {
    return `S${season} 已更新扫描范围内的数据；快速更新已达到扫描范围，如需完整校验请使用全量抓取。`;
  }

  if (summary.failedDetails > 0) {
    return `S${season} 对局列表已更新，${summary.failedDetails} 场详情获取失败，可再次操作重试。`;
  }

  if (summary.mode === "fullFetch") {
    return `S${season} 全量抓取完成：当前赛季共 ${summary.databaseMatches} 场。`;
  }

  if (summary.newMatches === 0 && summary.missingDetails === 0) {
    return `S${season} 已检查最新对局，没有发现更新。`;
  }

  return `S${season} 更新完成：新增 ${summary.newMatches} 场，当前赛季共 ${summary.databaseMatches} 场。`;
}

export function productErrorMessage(error: unknown): string {
  if (error === null || error === undefined) {
    return "请求失败，请稍后重新加载。";
  }
  const message = String(error);
  return message === "" ? "请求失败，请稍后重新加载。" : message;
}

export function hasDesktopBackend(host: unknown = globalThis): boolean {
  return Boolean((host as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}
