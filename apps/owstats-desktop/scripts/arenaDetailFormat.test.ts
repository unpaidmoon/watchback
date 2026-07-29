import assert from "node:assert/strict";
import { arenaHeaderMapKey, arenaHeaderTitle, formatArenaRankTierLabel, formatArenaScore, formatArenaTabLabel } from "../src/shared/arenaDetailFormat";
import { competitiveRankAssetUrl } from "../src/shared/assets";
import type { ArenaDetailSlice } from "../src/shared/types";

const summary = slice({ key: "summary", label: "角斗汇总", teamScore: 4, opponentScore: 3, mapGuid: "kings-row", mapName: "国王大道" });
const round = slice({ key: "round-4", label: "回合 4", teamScore: 2, opponentScore: 2, mapName: "阿努比斯王座" });

assert.equal(formatArenaScore(summary), "4 - 3");
assert.equal(formatArenaScore(round), "2 - 2");
assert.equal(formatArenaTabLabel(round), "回合-4");
assert.equal(arenaHeaderMapKey(summary), "kings-row");
assert.equal(formatArenaRankTierLabel("大师 2"), "2");
assert.equal(formatArenaRankTierLabel("钻石 5"), "5");
assert.equal(formatArenaRankTierLabel("大师"), "");
assert.equal(competitiveRankAssetUrl("Diamond"), "/assets/ranks/diamond.png");
assert.equal(competitiveRankAssetUrl(null), null);
assert.equal(competitiveRankAssetUrl("UnknownTier"), null);
assert.equal(arenaHeaderTitle(summary), "国王大道");
assert.equal(arenaHeaderTitle(round), "地图：阿努比斯王座");

function slice(overrides: Partial<ArenaDetailSlice>): ArenaDetailSlice {
  return {
    key: "summary",
    label: "角斗汇总",
    result: "win",
    resultLabel: "胜利",
    teamScore: null,
    opponentScore: null,
    durationSec: null,
    beginTs: null,
    beginLabel: null,
    mapGuid: null,
    mapName: null,
    teammates: [],
    enemies: [],
    ...overrides,
  };
}
