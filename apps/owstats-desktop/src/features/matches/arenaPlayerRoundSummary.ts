import type { ArenaDetailSlice, PlayerRow } from "../../shared/types";

export type ArenaPlayerRoundSummaryRow = {
  key: string;
  label: string;
  startWorth: number | null;
  traitGuids: string[];
  modGuids: string[];
};

export function buildArenaPlayerRoundSummary(
  target: PlayerRow,
  rounds: ArenaDetailSlice[],
): ArenaPlayerRoundSummaryRow[] {
  return rounds.map((round, index) => {
    const current = findPlayer(round, target);
    const previous = index > 0 ? findPlayer(rounds[index - 1], target) : null;
    return {
      key: round.key,
      label: `R${index + 1}`,
      startWorth: index === 0 ? 3000 : previous?.endWorth ?? current?.worth ?? null,
      traitGuids: current?.traitGuids.slice(0, 4) ?? [],
      modGuids: current?.modGuids.slice(0, 6) ?? [],
    };
  });
}

function findPlayer(round: ArenaDetailSlice, target: PlayerRow): PlayerRow | null {
  const players = [...round.teammates, ...round.enemies];
  if (target.bnetId) {
    const byId = players.find((candidate) => candidate.bnetId === target.bnetId);
    if (byId) return byId;
  }
  return players.find((candidate) => candidate.name === target.name) ?? null;
}
