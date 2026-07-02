import type { MatchSummary } from "./types";

export function formatScoreLine(match: MatchSummary): string {
  return `${match.resultLabel}：${match.teamScore}-${match.opponentScore}`;
}

export function formatMatchBannerTitle(match: MatchSummary): string {
  if (isFightMatch(match)) {
    return match.modeLabel;
  }
  return `${match.mapName} \\ ${match.modeLabel}`;
}

export function formatMatchPreviewKey(match: MatchSummary): string {
  return match.mapPreviewGuid || match.mapGuid || match.mapName;
}

function isFightMatch(match: MatchSummary): boolean {
  return match.matchKind === "fight" || match.gameMode === "LeisureFight" || match.gameMode === "SportFight";
}

export function formatCompactStat(value: number): string {
  if (value >= 10000) {
    return `${(value / 10000).toFixed(1).replace(".0", "")}w`;
  }
  return String(value);
}
