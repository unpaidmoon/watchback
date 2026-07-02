import type { ArenaDetailSlice } from "./types";

export function formatArenaScore(slice: ArenaDetailSlice): string {
  if (slice.teamScore === null || slice.opponentScore === null) return "-";
  return `${slice.teamScore} - ${slice.opponentScore}`;
}

export function formatArenaTabLabel(slice: ArenaDetailSlice): string {
  if (slice.key === "summary") return slice.label;
  const match = slice.key.match(/^round-(\d+)$/);
  return match ? `回合-${match[1]}` : slice.label.replace(/\s+/g, "-");
}

export function formatArenaRankTierLabel(rankLabel: string | null | undefined): string {
  const match = rankLabel?.trim().match(/(\d+)$/);
  return match?.[1] ?? "";
}

export function arenaHeaderTitle(slice: ArenaDetailSlice): string {
  return slice.key === "summary" ? slice.mapName ?? "未知地图" : `地图：${slice.mapName ?? "未知地图"}`;
}

export function arenaHeaderMapKey(slice: ArenaDetailSlice): string | null {
  return slice.mapGuid ?? slice.mapName;
}

export function arenaResultSubLabel(result: string | null): string {
  if (result === "win") return "VICTORY";
  if (result === "loss") return "DEFEAT";
  if (result === "draw") return "DRAW";
  return "";
}
