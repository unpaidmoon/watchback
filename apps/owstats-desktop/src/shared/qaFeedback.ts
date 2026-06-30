export type QaRegionDefinition = {
  id: string;
  label: string;
};

export type QaPageDefinition = {
  key: string;
  label: string;
  regions: QaRegionDefinition[];
};

const NAV_REGION: QaRegionDefinition = { id: "NAV-CAPSULE", label: "顶部胶囊导航" };

export const phase6QaPages: QaPageDefinition[] = [
  {
    key: "/onboarding",
    label: "指挥台",
    regions: [
      NAV_REGION,
      { id: "CMD-HEADER", label: "指挥台总览" },
      { id: "CMD-STATUS", label: "登录与数据状态" },
      { id: "CMD-ACTIONS", label: "同步入口" },
    ],
  },
  {
    key: "/matches",
    label: "对局",
    regions: [
      NAV_REGION,
      { id: "MAT-HEADER", label: "对局工作台头部" },
      { id: "MAT-FILTERS", label: "筛选器" },
      { id: "MAT-LIST", label: "对局列表" },
    ],
  },
  {
    key: "/matches/",
    label: "对局详情",
    regions: [
      NAV_REGION,
      { id: "MTD-HEADER", label: "详情首屏" },
      { id: "MTD-SCORE", label: "评分与职责对比" },
      { id: "MTD-PLAYERS", label: "玩家数据表" },
    ],
  },
  {
    key: "/review",
    label: "复盘",
    regions: [
      NAV_REGION,
      { id: "REV-HEADER", label: "复盘总览" },
      { id: "REV-SUMMARY", label: "赛季摘要" },
      { id: "REV-EVIDENCE", label: "复盘证据" },
      { id: "REV-MAP-CARD", label: "地图分析卡片" },
    ],
  },
  {
    key: "/ranking",
    label: "排行",
    regions: [
      NAV_REGION,
      { id: "RANK-HEADER", label: "排行头部" },
      { id: "RANK-TOOLS", label: "排行工具栏" },
      { id: "RANK-TABLE", label: "排行表格" },
    ],
  },
  {
    key: "/analytics/friends",
    label: "分析模型",
    regions: [
      NAV_REGION,
      { id: "FRD-HEADER", label: "分析模型总览" },
      { id: "FRD-BOARDS", label: "模型榜单" },
      { id: "FRD-LINEUP", label: "常见阵容" },
      { id: "FRD-MAPS", label: "地图分析" },
    ],
  },
  {
    key: "/analytics/friends/",
    label: "队友详情",
    regions: [
      NAV_REGION,
      { id: "FDT-HEADER", label: "队友详情头部" },
      { id: "FDT-HEROES", label: "常用英雄" },
      { id: "FDT-MATCHES", label: "近期共同对局" },
    ],
  },
  {
    key: "/sync",
    label: "同步",
    regions: [
      NAV_REGION,
      { id: "SYNC-HEADER", label: "同步头部" },
      { id: "SYNC-CONTROL", label: "同步控制" },
      { id: "SYNC-STATUS", label: "同步状态" },
    ],
  },
];

export function isQaEnabledFromSearch(search: string): boolean {
  const params = new URLSearchParams(search.startsWith("?") ? search.slice(1) : search);
  const value = params.get("qa")?.toLowerCase();
  return value === "1" || value === "true" || value === "on";
}

export function preserveQaParam(current: URLSearchParams, next: URLSearchParams): URLSearchParams {
  const merged = new URLSearchParams(next);
  if (isQaEnabledFromSearch(current.toString())) {
    merged.set("qa", current.get("qa") ?? "1");
  }
  return merged;
}

export function qaPageForPath(pathname: string): QaPageDefinition {
  const dynamicPage = phase6QaPages
    .filter((page) => page.key.endsWith("/") && pathname.startsWith(page.key))
    .sort((a, b) => b.key.length - a.key.length)[0];
  if (dynamicPage) return dynamicPage;

  return phase6QaPages.find((page) => page.key === pathname) ?? phase6QaPages[0];
}

export function qaRegionsForPath(pathname: string): QaRegionDefinition[] {
  return qaPageForPath(pathname).regions;
}

export function formatQaFeedbackText(input: {
  pageLabel: string;
  regionId: string;
  regionLabel: string;
}): string {
  return [
    `页面：${input.pageLabel}`,
    `区域：${input.regionId}（${input.regionLabel}）`,
    "问题：",
    "期望：",
    "优先级：P1",
  ].join("\n");
}
