import assert from "node:assert/strict";
import { playerHighlightMap, playerKeyFor } from "../src/shared/playerHighlights";
import type { PlayerRow } from "../src/shared/types";

const players = [
  player("ana", { kills: 8, assists: 2, damage: 6296, healing: 0, mitigation: 349 }),
  player("winston", { kills: 21, assists: 6, damage: 5923, healing: 1147, mitigation: 8965 }),
  player("genji", { kills: 28, assists: 1, damage: 8941, healing: 352, mitigation: 1250 }),
];

const highlights = playerHighlightMap(players);

assert.deepEqual([...highlights.get(playerKeyFor(players[0], 0)) ?? []], []);
assert.deepEqual([...highlights.get(playerKeyFor(players[1], 1)) ?? []].sort(), ["assists", "healing", "mitigation"]);
assert.deepEqual([...highlights.get(playerKeyFor(players[2], 2)) ?? []].sort(), ["damage", "kills"]);

const sourceFlagPlayers = [
  player("ana", { kills: 1, damage: 1, healing: 1, mitigation: 1, killMax: true, heroDamageMax: false, cureMax: false, resistDamageMax: true }),
  player("winston", { kills: 99, damage: 99, healing: 99, mitigation: 99, killMax: false, heroDamageMax: true, cureMax: true, resistDamageMax: false }),
];
const sourceFlagHighlights = playerHighlightMap(sourceFlagPlayers);

assert.deepEqual([...sourceFlagHighlights.get(playerKeyFor(sourceFlagPlayers[0], 0)) ?? []].sort(), ["kills", "mitigation"]);
assert.deepEqual([...sourceFlagHighlights.get(playerKeyFor(sourceFlagPlayers[1], 1)) ?? []].sort(), ["damage", "healing"]);

function player(heroName: string, stats: Partial<PlayerRow>): PlayerRow {
  return {
    side: "teammate",
    name: heroName,
    bnetId: heroName,
    heroGuid: heroName,
    heroName,
    kills: 0,
    assists: 0,
    deaths: 0,
    damage: 0,
    healing: 0,
    mitigation: 0,
    finalHits: 0,
    targetTimeSec: 0,
    healingTaken: 0,
    damageTaken: 0,
    ...stats,
  };
}
