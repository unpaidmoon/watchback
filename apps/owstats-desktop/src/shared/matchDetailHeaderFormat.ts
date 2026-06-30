import type { MatchDetailView } from "./types";

export function formatFinalScore(detail: MatchDetailView): string {
  if (detail.teamScore === null || detail.opponentScore === null) return "-";
  return `${detail.teamScore}-${detail.opponentScore}`;
}

export function formatDetailDuration(durationSec: number | null): string {
  if (durationSec === null) return "-";
  const minutes = Math.floor(durationSec / 60);
  const seconds = durationSec % 60;
  return `${minutes}分${seconds}秒`;
}

export function formatDetailStartTime(beginTs: number | null): string {
  if (beginTs === null) return "-";
  const date = new Date(beginTs * 1000);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(date.getFullYear() % 100)}/${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function resultEnglishLabel(result: string | null): string {
  if (result === "win") return "VICTORY";
  if (result === "loss") return "DEFEAT";
  return "DRAW";
}
