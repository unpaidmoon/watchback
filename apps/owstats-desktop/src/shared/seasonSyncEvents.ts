import type { SeasonBackgroundEvent, SeasonSyncResult } from "./types";

export type SeasonRefreshScope = {
  accountScope: string;
  season: number;
};

export function quickListRefreshScope(
  accountScope: string | null | undefined,
  result: SeasonSyncResult,
): SeasonRefreshScope | null {
  if (result.kind !== "quickList" || !result.dataChanged || !accountScope) {
    return null;
  }
  return { accountScope, season: result.season };
}

export function isCurrentSeasonEvent(
  event: SeasonBackgroundEvent,
  accountScope: string | null | undefined,
): boolean {
  return Boolean(accountScope) && event.accountScope === accountScope;
}

export function backgroundStatusLabel(event: Pick<SeasonBackgroundEvent, "phase" | "status" | "pendingDetails">) {
  if (event.status === "failed") return "后台处理失败，可重试";
  if (event.phase === "continuePages") return "正在继续检查后续页面";
  if (event.phase === "fetchDetails") return `详情处理中：${event.pendingDetails} 场`;
  if (event.phase === "rebuildSnapshot") return "正在重建本地快照";
  if (event.status === "completed") return "更新完成";
  return "后台处理中";
}
