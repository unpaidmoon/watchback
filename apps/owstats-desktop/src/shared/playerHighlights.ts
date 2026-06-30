import type { PlayerRow } from "./types";

export type HighlightMetric = "kills" | "assists" | "damage" | "healing" | "mitigation";

const highlightMetrics: HighlightMetric[] = ["kills", "assists", "damage", "healing", "mitigation"];

export function playerHighlightMap(players: PlayerRow[]): Map<string, Set<HighlightMetric>> {
  const maxima = new Map<HighlightMetric, number>();
  for (const metric of highlightMetrics) {
    maxima.set(metric, Math.max(0, ...players.map((player) => player[metric])));
  }

  const highlights = new Map<string, Set<HighlightMetric>>();
  players.forEach((player, index) => {
    const playerKey = playerKeyFor(player, index);
    const playerHighlights = new Set<HighlightMetric>();
    for (const metric of highlightMetrics) {
      const maximum = maxima.get(metric) ?? 0;
      if (maximum > 0 && player[metric] === maximum) {
        playerHighlights.add(metric);
      }
    }
    highlights.set(playerKey, playerHighlights);
  });
  return highlights;
}

export function playerKeyFor(player: PlayerRow, index = 0): string {
  return `${player.side}-${player.bnetId || player.name}-${index}`;
}
