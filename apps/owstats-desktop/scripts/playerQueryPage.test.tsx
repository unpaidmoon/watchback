import assert from "node:assert/strict";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { PlayerQueryForm } from "../src/features/player-query/PlayerQueryForm";
import { formatQueryDuration, PlayerProfileCard } from "../src/features/player-query/PlayerProfileCard";
import { PlayerQueryModeTabs } from "../src/features/player-query/PlayerQueryModeTabs";
import { normalizeModeOrder, reorderModesForDrop } from "../src/features/player-query/modeOrder";
import { changePlayerQuerySeason, initialPlayerQueryState } from "../src/features/player-query/playerQueryState";

const form = renderToStaticMarkup(<PlayerQueryForm disabled={false} searching={true} onSubmit={() => undefined} />); assert.match(form, /正在查询/); assert.doesNotMatch(form, /取消/);
assert.doesNotMatch(form, /支持半角|全角/);
const textWhileImagesPending = renderToStaticMarkup(<PlayerProfileCard card={{ battleTag: "Player#123", level: 123, gameTimeSeconds: 90, title: "先锋", gameAction: null }} images={null} battleTag="Player#123" />);
assert.match(textWhileImagesPending, /Player#123/);
assert.match(textWhileImagesPending, /123/);
assert.match(textWhileImagesPending, /先锋/);
assert.doesNotMatch(textWhileImagesPending, /data:image\//, "card text renders while the independent image resource is pending");
const cardWithImages = renderToStaticMarkup(
  <PlayerProfileCard
    card={{ battleTag: "Player#123", level: 123, gameTimeSeconds: 90, title: "先锋", gameAction: null }}
    images={{
      icon: { mimeType: "image/png", dataBase64: "AQID" },
      titleIcon: { mimeType: "image/png", dataBase64: "BAUG" },
    }}
    battleTag="Player#123"
  />,
);
assert.match(cardWithImages, /src="data:image\/png;base64,AQID"/, "query profile renders the icon returned by cardImages");
assert.doesNotMatch(cardWithImages, /src="data:image\/png;base64,BAUG"/, "query profile intentionally hides titleIcon");
assert.equal(formatQueryDuration(3_756_060), "1043小时");
assert.match(form, /aria-label="BattleTag"/, "the input keeps a stable accessible name without a hidden label");

const normalizedOrder = normalizeModeOrder(["SportFight", "invalid", "SportFight", "LeisurePreset"]);
assert.equal(normalizedOrder[0], "SportFight", "persisted leftmost valid mode remains first");
assert.equal(new Set(normalizedOrder).size, normalizedOrder.length, "mode order removes duplicates");
assert.deepEqual(reorderModesForDrop(normalizedOrder, "LeisurePreset", "SportFight").slice(0, 2), ["LeisurePreset", "SportFight"]);
const tabs = renderToStaticMarkup(<PlayerQueryModeTabs
  order={normalizedOrder}
  selected="SportFight"
  disabled={false}
  onSelect={() => undefined}
  onDropMode={() => undefined}
/>);
assert.equal(tabs.includes("role=\"tablist\""), true);
assert.equal(tabs.includes("aria-label=\"对局模式顺序\""), true);
assert.equal(tabs.indexOf("角斗竞技") < tabs.indexOf("快速预设"), true, "rendered tabs preserve normalized mode order");
assert.equal(tabs.includes("aria-selected=\"true\""), true);

const idleState = initialPlayerQueryState("SportPreset");
const changedSeason = changePlayerQuerySeason(idleState, 22);
assert.equal(changedSeason.season, 22);
assert.equal(changedSeason.generation, idleState.generation + 1);
assert.equal(changedSeason.matches.status, "loading");
assert.equal(changedSeason.heroes.status, "loading");
assert.equal(changedSeason.matchesEndReached, false);
console.log("player-query page tests passed");
