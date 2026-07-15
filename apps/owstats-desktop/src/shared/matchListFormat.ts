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
  if (value <= 10000) {
    return groupedNumber.format(value);
  }

  const roundedToThousand = Math.round(value / 1000) * 1000;
  return `${(roundedToThousand / 10000).toFixed(1)}w`;
}
