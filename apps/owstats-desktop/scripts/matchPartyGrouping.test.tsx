import assert from "node:assert/strict";
import type { ReactElement, ReactNode } from "react";
import type { PlayerRow } from "../src/shared/types";
import { MatchTeamTable } from "../src/features/matches/MatchTeamTable";

type PartyModule = typeof import("../src/features/matches/partyGroups");

let partyModule: PartyModule | null = null;
try {
  partyModule = await import("../src/features/matches/partyGroups");
} catch {
  // The first RED run proves the production helper does not exist yet.
}
assert.notEqual(partyModule, null, "party grouping helper should exist");

function player(name: string, partyGroup: number | null): PlayerRow {
  return {
    side: "teammate",
    name,
    bnetId: name,
    rankLabel: null,
    rankKey: null,
    heroGuid: `hero-${name}`,
    heroName: "测试英雄",
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
    worth: null,
    endWorth: null,
    worthMax: false,
    modGuids: [],
    traitGuids: [],
    hasSourceMaxFlags: false,
    killMax: false,
    heroDamageMax: false,
    cureMax: false,
    resistDamageMax: false,
    perks: [],
    heroStats: [],
    partyGroup,
  } as PlayerRow;
}

const players = [
  player("组队甲", 0),
  player("组队乙", 0),
  player("单排", null),
  player("组队丙", 1),
  player("组队丁", 1),
];
const blocks = partyModule.playerPartyBlocks(players);
assert.deepEqual(
  blocks.map((block) => [block.partyGroup, block.players.map((item) => item.name)]),
  [
    [0, ["组队甲", "组队乙"]],
    [null, ["单排"]],
    [1, ["组队丙", "组队丁"]],
  ],
);

const tree = MatchTeamTable({ title: "己方", tone: "team", players });
assert.equal(findByClass(tree, "matchTeamPartyGroup").length, 2);
assert.equal(findByClass(tree, "matchTeamPartyRail").length, 2);
assert.equal(findByClass(tree, "matchTeamSoloPlayer").length, 1);

assert.deepEqual(partyModule.playerPartyBlocks([]), []);
assert.deepEqual(
  partyModule.playerPartyBlocks([player("甲", 7), player("单排", null), player("乙", 7)])
    .map((block) => [block.partyGroup, block.players.map((item) => item.name)]),
  [[7, ["甲"]], [null, ["单排"]], [7, ["乙"]]],
  "only adjacent players in the same party form one visual block",
);

function findByClass(node: ReactNode, className: string): ReactElement[] {
  const matches: ReactElement[] = [];
  visit(node, (element) => {
    const value = element.props.className;
    if (
      typeof value === "string" &&
      value.split(/\s+/).includes(className)
    ) {
      matches.push(element);
    }
  });
  return matches;
}

function visit(node: ReactNode, onElement: (element: ReactElement) => void) {
  if (Array.isArray(node)) {
    node.forEach((child) => visit(child, onElement));
    return;
  }
  if (!node || typeof node !== "object" || !("type" in node)) return;
  const element = node as ReactElement;
  onElement(element);
  visit(element.props.children, onElement);
}
