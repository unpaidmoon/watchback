import type { PlayerRow } from "./types";

export type HighlightMetric = "kills" | "assists" | "damage" | "healing" | "mitigation";

const highlightMetrics: HighlightMetric[] = ["kills", "assists", "damage", "healing", "mitigation"];

export function playerHighlightMap(players: PlayerRow[]): Map<string, Set<HighlightMetric>> {
  if (players.some((player) => player.hasSourceMaxFlags || (player.hasSourceMaxFlags === undefined && hasExplicitSourceFlags(player)))) {
    return sourceFlagHighlightMap(players);
  }

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

function hasExplicitSourceFlags(player: PlayerRow): boolean {
  return "killMax" in player || "heroDamageMax" in player || "cureMax" in player || "resistDamageMax" in player;
}

function sourceFlagHighlightMap(players: PlayerRow[]): Map<string, Set<HighlightMetric>> {
  const highlights = new Map<string, Set<HighlightMetric>>();
  players.forEach((player, index) => {
    const playerHighlights = new Set<HighlightMetric>();
    if (player.killMax) playerHighlights.add("kills");
    if (player.heroDamageMax) playerHighlights.add("damage");
    if (player.cureMax) playerHighlights.add("healing");
    if (player.resistDamageMax) playerHighlights.add("mitigation");
    highlights.set(playerKeyFor(player, index), playerHighlights);
  });
  return highlights;
}

export function playerKeyFor(player: PlayerRow, index = 0): string {
  return `${player.side}-${player.bnetId || player.name}-${index}`;
}
