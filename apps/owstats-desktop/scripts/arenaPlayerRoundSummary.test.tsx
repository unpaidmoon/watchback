import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ArenaPlayerRoundSummaryDialog } from "../src/features/matches/ArenaPlayerRoundSummaryDialog";
import type { ArenaDetailSlice, PlayerRow } from "../src/shared/types";
import { buildArenaPlayerRoundSummary } from "../src/features/matches/arenaPlayerRoundSummary";

function player(overrides: Partial<PlayerRow> = {}): PlayerRow {
  return {
    side: "teammate",
    name: "测试玩家",
    bnetId: "synthetic-player",
    rankLabel: null,
    rankKey: null,
    heroGuid: "synthetic-hero",
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
    ...overrides,
  };
}

function round(key: string, teammates: PlayerRow[], enemies: PlayerRow[] = []): ArenaDetailSlice {
  return {
    key,
    label: key,
    result: null,
    resultLabel: null,
    teamScore: null,
    opponentScore: null,
    durationSec: null,
    beginTs: null,
    beginLabel: null,
    mapGuid: null,
    mapName: null,
    teammates,
    enemies,
  };
}

const target = player();
const rows = buildArenaPlayerRoundSummary(target, [
  round("round-1", [player({ worth: 3000, endWorth: 4200, traitGuids: ["trait-a"], modGuids: ["mod-a"] })]),
  round("round-2", [player({ worth: 4200, endWorth: 6100, traitGuids: ["trait-a", "trait-b"], modGuids: ["mod-a", "mod-b"] })]),
  round("round-3", [player({ worth: 6100, endWorth: 8000, traitGuids: ["trait-a", "trait-b", "trait-c"], modGuids: ["mod-a", "mod-b", "mod-c"] })]),
]);

assert.deepEqual(rows.map((row) => row.label), ["R1", "R2", "R3"]);
assert.deepEqual(rows.map((row) => row.startWorth), [3000, 4200, 6100]);
assert.deepEqual(rows[1]?.traitGuids, ["trait-a", "trait-b"]);
assert.deepEqual(rows[1]?.modGuids, ["mod-a", "mod-b"]);

const fallbackRows = buildArenaPlayerRoundSummary(target, [
  round("round-1", [player({ endWorth: null })]),
  round("round-2", [player({ worth: 4700, endWorth: 6000 })]),
  round("round-3", [player({ worth: null, endWorth: 7200 })]),
]);
assert.deepEqual(fallbackRows.map((row) => row.startWorth), [3000, 4700, 6000]);

const missingRows = buildArenaPlayerRoundSummary(target, [
  round("round-1", [player({ endWorth: 3900 })]),
  round("round-2", []),
  round("round-3", [player({ worth: null, endWorth: 6500 })]),
]);
assert.equal(missingRows.length, 3);
assert.equal(missingRows[1]?.startWorth, 3900);
assert.deepEqual(missingRows[1]?.traitGuids, []);
assert.equal(missingRows[2]?.startWorth, null);

const enemyTarget = player({ side: "enemy", name: "测试对手", bnetId: "synthetic-enemy" });
const enemyRows = buildArenaPlayerRoundSummary(enemyTarget, [
  round("round-1", [], [player({ ...enemyTarget, endWorth: 5100, traitGuids: ["trait-enemy"] })]),
  round("round-2", [], [player({ ...enemyTarget, worth: 5100, endWorth: 7300 })]),
]);
assert.deepEqual(enemyRows.map((row) => row.startWorth), [3000, 5100]);
assert.deepEqual(enemyRows[0]?.traitGuids, ["trait-enemy"]);

const sameNameEnemy = player({ side: "enemy", bnetId: "", name: "同名玩家" });
const sameNameRows = buildArenaPlayerRoundSummary(sameNameEnemy, [
  round(
    "round-1",
    [player({ bnetId: "", name: "同名玩家", traitGuids: ["trait-team"] })],
    [player({ ...sameNameEnemy, traitGuids: ["trait-enemy"] })],
  ),
]);
assert.deepEqual(sameNameRows[0]?.traitGuids, ["trait-enemy"]);

const dialogMarkup = renderToStaticMarkup(createElement(ArenaPlayerRoundSummaryDialog, {
  player: target,
  rows,
  onClose: () => undefined,
}));

assert.match(dialogMarkup, /role="dialog"/);
assert.match(dialogMarkup, /aria-modal="true"/);
assert.match(dialogMarkup, />汇总卡片</);
assert.match(dialogMarkup, />测试玩家</);
assert.match(dialogMarkup, />R1</);
assert.match(dialogMarkup, />3000</);
assert.equal((dialogMarkup.match(/phase7SummaryTraitSlot/g) ?? []).length, rows.length * 4);
assert.equal((dialogMarkup.match(/phase7SummaryModSlot/g) ?? []).length, rows.length * 6);
assert.doesNotMatch(dialogMarkup, /phase6PerkTooltipPanel/);

const compactSource = readFileSync(new URL("../src/features/matches/CompactMatchDetail.tsx", import.meta.url), "utf8");
const tableSource = readFileSync(new URL("../src/features/matches/MatchTeamTable.tsx", import.meta.url), "utf8");

assert.match(compactSource, /selected\.key === "summary" \? openArenaPlayerSummary : undefined/);
assert.match(compactSource, /buildArenaPlayerRoundSummary\(summaryTarget\.player, arena\.rounds\)/);
assert.match(tableSource, /onSelectPlayerSummary\?: \(player: PlayerRow, trigger: HTMLButtonElement\) => void/);
assert.match(tableSource, /aria-label=\{`查看\$\{player\.name\}的汇总卡片`\}/);
assert.match(tableSource, /className="phase7CompactPlayerRowToggle"/);
assert.match(tableSource, /className="phase6CompactAvatarPlate phase7CompactAvatarSummaryButton"/);

const styles = readFileSync(new URL("../src/styles/features/matches.css", import.meta.url), "utf8");
assert.match(styles, /\.phase7SummaryDialogBackdrop\s*\{[^}]*position:\s*fixed/s);
assert.match(styles, /\.phase7SummaryDialog\s*\{[^}]*max-height:\s*min\(760px,\s*calc\(100dvh - 48px\)\)/s);
assert.match(styles, /\.phase7SummaryRound\s*\{[^}]*grid-template-columns:/s);
assert.match(styles, /\.phase7SummarySlots\.trait\s*\{[^}]*grid-template-columns:\s*repeat\(4,/s);
assert.match(styles, /\.phase7SummarySlots\.mod\s*\{[^}]*grid-template-columns:\s*repeat\(6,/s);
assert.match(styles, /@media\s*\(max-width:\s*900px\)/s);
assert.doesNotMatch(styles, /\.phase7SummaryDialog[\s\S]*overflow-x:\s*(auto|scroll)/s);
