import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { arenaHeaderMapKey, arenaHeaderTitle, formatArenaRankTierLabel, formatArenaScore, formatArenaTabLabel } from "../src/shared/arenaDetailFormat";
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
assert.equal(arenaHeaderTitle(summary), "国王大道");
assert.equal(arenaHeaderTitle(round), "地图：阿努比斯王座");

const styleSource = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const detailSource = readFileSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url), "utf8");
const matchesSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");

assert.equal(styleSource.includes(".phase7ArenaTab + .phase7ArenaTab"), true, "arena tabs should overlap adjacent slants");
assert.equal(styleSource.includes("gap: 0"), true, "arena selector should not leave gaps between tabs");
assert.equal(detailSource.includes("phase7PlayerNameLine"), true, "rank should sit in the player id line");
assert.match(
  styleSource,
  /\.phase7CompactArenaDetail\s*\{[^}]*grid-template-rows:\s*auto\s+124px\s+minmax\(0,\s*1fr\)/s,
  "compact arena detail should reserve separate rows for tabs, hero banner, and scroll content",
);
assert.equal(matchesSource.includes("phase7CompactPlayerNameLine"), true, "compact rank should sit in the player id line");
assert.equal(matchesSource.includes("playerHighlightMap"), true, "compact arena rows should use source max highlights");
const highlightRule = styleSource.match(/\.phase6StatHighlight\s*\{(?<body>[^}]*)\}/)?.groups?.body ?? "";
assert.equal(highlightRule.includes("background:"), false, "stat highlight should use slight text color only, without a background block");
assert.equal(highlightRule.includes("text-shadow:"), false, "stat highlight should avoid glow");
assert.equal(highlightRule.includes("padding:"), false, "stat highlight should not change metric spacing");
assert.match(styleSource, /\.phase7ArenaRankBadge,\s*\.phase7CompactRankBadge\s*\{[^}]*color:\s*#fff/s, "arena rank tier text should be white");
assert.match(
  styleSource,
  /\.phase6PlayerStats header \.phase7ArenaRankBadge,\s*\.phase6CompactPlayerRow \.phase7CompactRankBadge\s*\{[^}]*color:\s*#fff/s,
  "arena rank tier text should stay white against parent small/span color rules",
);
assert.match(styleSource, /\.phase7ArenaRankBadge img,\s*\.phase7CompactRankBadge img\s*\{[^}]*width:\s*24px[^}]*height:\s*24px/s, "arena rank icons should be larger");

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
