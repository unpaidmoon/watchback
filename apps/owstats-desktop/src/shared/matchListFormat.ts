import type { MatchSummary } from "./types";

const groupedNumber = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 0 });

export function formatScoreLine(match: MatchSummary): string {
  return `${match.teamScore}-${match.opponentScore}`;
}

export function formatPlayerQueryScoreLine(score: string | null): string {
  return score?.trim() || "-";
}

export function formatPlayerQueryDate(playedAt: string | null): string {
  const value = playedAt?.trim();
  if (!value) return "-";
  const calendarDate = value.match(/^\d{4}-(\d{2})-(\d{2})/);
  if (calendarDate) return `${calendarDate[1]}-${calendarDate[2]}`;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "-";
  return `${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
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
