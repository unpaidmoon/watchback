import type { PlayerRow } from "../../shared/types";

export type PlayerPartyBlock = {
  partyGroup: number | null;
  players: PlayerRow[];
};

export function playerPartyBlocks(players: PlayerRow[]): PlayerPartyBlock[] {
  const blocks: PlayerPartyBlock[] = [];
  for (const player of players) {
    const partyGroup = player.partyGroup ?? null;
    const previous = blocks[blocks.length - 1];
    if (partyGroup !== null && previous?.partyGroup === partyGroup) {
      previous.players.push(player);
    } else {
      blocks.push({ partyGroup, players: [player] });
    }
  }
  return blocks;
}
