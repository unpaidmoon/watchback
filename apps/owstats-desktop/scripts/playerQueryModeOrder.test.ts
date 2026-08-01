import assert from "node:assert/strict";
import {
  PLAYER_QUERY_MODES,
  normalizeModeOrder,
  reorderModesForDrop,
  saveModeOrderAfterDrop,
} from "../src/features/player-query/modeOrder";

assert.deepEqual(normalizeModeOrder(undefined), [...PLAYER_QUERY_MODES]);
assert.deepEqual(
  normalizeModeOrder(["SportOpen", "unknown", "SportOpen", "LeisurePreset"]),
  ["SportOpen", "LeisurePreset", "Leisure6v6", "SportPreset", "SportFight", "LeisureFight"],
);

assert.deepEqual(
  reorderModesForDrop(PLAYER_QUERY_MODES, "LeisurePreset", "SportOpen"),
  ["Leisure6v6", "SportPreset", "SportOpen", "LeisurePreset", "SportFight", "LeisureFight"],
);
assert.deepEqual(reorderModesForDrop(PLAYER_QUERY_MODES, "unknown", "SportOpen"), [...PLAYER_QUERY_MODES]);

let saveCalls = 0;
const savedOrders: readonly string[][] = [];
const changed = await saveModeOrderAfterDrop(
  PLAYER_QUERY_MODES,
  "SportFight",
  "LeisurePreset",
  async (order) => {
    saveCalls += 1;
    savedOrders.push([...order]);
  },
);
assert.equal(changed.changed, true);
assert.equal(changed.saveError, null);
assert.deepEqual(changed.order, ["SportFight", "LeisurePreset", "Leisure6v6", "SportPreset", "SportOpen", "LeisureFight"]);
assert.deepEqual(savedOrders, [changed.order]);

const unchanged = await saveModeOrderAfterDrop(
  changed.order,
  "SportFight",
  "SportFight",
  async () => { saveCalls += 1; },
);
assert.equal(unchanged.changed, false);
assert.equal(saveCalls, 1, "self-drop must not save");

const saveFailure = new Error("disk unavailable");
const failed = await saveModeOrderAfterDrop(
  PLAYER_QUERY_MODES,
  "LeisureFight",
  "LeisurePreset",
  async () => { throw saveFailure; },
);
assert.equal(failed.changed, true);
assert.equal(failed.saveError, saveFailure);
assert.deepEqual(failed.order, ["LeisureFight", "LeisurePreset", "Leisure6v6", "SportPreset", "SportOpen", "SportFight"]);

console.log("player-query mode-order tests passed");
