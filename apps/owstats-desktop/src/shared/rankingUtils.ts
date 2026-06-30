import type { RankingRow } from "./types";

export type RankingSortKey = "matches" | "wins" | "losses" | "winRate" | "playTimeHours";
export type SortDirection = "asc" | "desc";

export type RankingSort = {
  key: RankingSortKey;
  direction: SortDirection;
};

const modeLabels: Record<string, string> = {
  preset: "预设 / 开放",
  open: "预设 / 开放",
  stadium: "体育场",
  fight: "战斗",
  LeisurePreset: "快速预设",
  Leisure6v6: "快速 6v6",
  SportPreset: "竞技预设",
  SportOpen: "竞技开放",
};

export function modeLabel(mode: string) {
  return modeLabels[mode] ?? mode;
}

export function formatWinRate(value: number, digits = 1) {
  return `${(value * 100).toFixed(digits)}%`;
}

export function formatPlayTimeHours(value: number) {
  return `${value.toFixed(1)} 小时`;
}

export function defaultRankingSort(tab: "heroes" | "maps"): RankingSort {
  return tab === "heroes" ? { key: "matches", direction: "desc" } : { key: "winRate", direction: "desc" };
}

export function toggleRankingSort(current: RankingSort, key: RankingSortKey): RankingSort {
  if (current.key !== key) {
    return { key, direction: "desc" };
  }
  return { key, direction: current.direction === "desc" ? "asc" : "desc" };
}

export function sortRankingRows(rows: RankingRow[], sort: RankingSort) {
  const direction = sort.direction === "desc" ? -1 : 1;
  return [...rows].sort((a, b) => {
    const valueCompare = compareNumber(a[sort.key], b[sort.key]) * direction;
    if (valueCompare !== 0) return valueCompare;
    const matchTiebreak = compareNumber(a.matches, b.matches) * -1;
    if (matchTiebreak !== 0) return matchTiebreak;
    const winRateTiebreak = compareNumber(a.winRate, b.winRate) * -1;
    if (winRateTiebreak !== 0) return winRateTiebreak;
    return a.name.localeCompare(b.name, "zh-Hans-CN");
  });
}

function compareNumber(a: number, b: number) {
  return a === b ? 0 : a > b ? 1 : -1;
}
