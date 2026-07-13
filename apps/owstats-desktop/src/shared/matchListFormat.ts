import type { MatchSummary } from "./types";

const groupedNumber = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 0 });

export function formatScoreLine(match: MatchSummary): string {
  return `${match.teamScore}-${match.opponentScore}`;
}

export function formatMatchBannerTitle(match: MatchSummary): string {
  return `${match.mapName} · ${match.modeLabel}`;
}

export function formatMatchPreviewKey(match: MatchSummary): string {
  return match.mapPreviewGuid || match.mapGuid || match.mapName;
}

export function formatCompactStat(value: number): string {
  return groupedNumber.format(value);
}
