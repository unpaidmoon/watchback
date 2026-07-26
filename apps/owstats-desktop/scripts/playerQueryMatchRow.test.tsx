import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MatchListCard, type PlayerQueryMatchRowView } from "../src/features/matches/MatchListCard";
import { formatPlayerQueryDate, formatPlayerQueryScoreLine } from "../src/shared/matchListFormat";

const normalRow: PlayerQueryMatchRowView = {
  matchId: "normal-1",
  matchKind: "normal",
  result: "win",
  score: "3-2",
  mapName: "示例地图",
  heroKey: "hero-a",
  heroName: "示例英雄A",
  kills: 20,
  assists: 8,
  deaths: 4,
  playedAt: "2026-01-01T00:00:00+08:00",
};

const normalMarkup = renderToStaticMarkup(
  createElement(MatchListCard, { variant: "playerQuery", match: normalRow, selected: true, onSelect: () => undefined }),
);
assert.match(normalMarkup, /胜利/);
assert.match(normalMarkup, />3-2</);
assert.match(normalMarkup, /示例地图/);
assert.match(normalMarkup, /示例英雄A/);
assert.match(normalMarkup, /20\/8\/4/);
assert.match(normalMarkup, /01-01/);
assert.doesNotMatch(normalMarkup, /已收藏|伤害\/治疗\/抵挡|对局表现|MatchPerformanceBadges/);
assert.doesNotMatch(normalMarkup, /\/assets\/maps\/stadium\.png/);

for (const upstreamLoss of ["loss", "defeat", "lose", "失败", "-1"]) {
  const lossMarkup = renderToStaticMarkup(createElement(MatchListCard, {
    variant: "playerQuery",
    match: { ...normalRow, matchId: `loss-${upstreamLoss}`, result: upstreamLoss },
    selected: false,
    onSelect: () => undefined,
  }));
  assert.match(lossMarkup, />失败</, `${upstreamLoss} is presented as a loss`);
  assert.doesNotMatch(lossMarkup, /未知结果/, `${upstreamLoss} must not degrade to unknown`);
}

const compactNormalMarkup = renderToStaticMarkup(
  createElement(MatchListCard, {
    variant: "playerQuery",
    compact: true,
    match: normalRow,
    selected: false,
    onSelect: () => undefined,
  }),
);
assert.match(compactNormalMarkup, /playerQueryCompactMatchRow/);
assert.match(compactNormalMarkup, /matchListCard phase6MatchRowReal/);
assert.match(compactNormalMarkup, /playerQueryCompactMap/);
assert.doesNotMatch(compactNormalMarkup, /phase6MapPreviewShared|matchListCardHeaderMap|stadium\.png/);

const realGuidMarkup = renderToStaticMarkup(
  createElement(MatchListCard, {
    variant: "playerQuery",
    compact: true,
    match: {
      ...normalRow,
      matchId: "real-guid-row",
      mapKey: "576460752303423876",
      mapName: null,
      heroKey: "207165582859042857",
      heroName: null,
    },
    selected: false,
    onSelect: () => undefined,
  }),
);
assert.match(realGuidMarkup, /监测站：直布罗陀/);
assert.match(realGuidMarkup, /源氏/);

const arenaMarkup = renderToStaticMarkup(
  createElement(MatchListCard, {
    variant: "playerQuery",
    match: { ...normalRow, matchId: "arena-1", matchKind: "arena", mapName: "不应显示的末回合地图" },
    selected: false,
    onSelect: () => undefined,
  }),
);
assert.match(arenaMarkup, /src="\/assets\/maps\/stadium\.png"/);
assert.doesNotMatch(arenaMarkup, /不应显示的末回合地图/);
assert.match(arenaMarkup, />角斗</);

const unknownMapMarkup = renderToStaticMarkup(
  createElement(MatchListCard, {
    variant: "playerQuery",
    match: { ...normalRow, matchId: "unknown-1", mapName: null },
    selected: false,
    onSelect: () => undefined,
  }),
);

const compactArenaMarkup = renderToStaticMarkup(
  createElement(MatchListCard, {
    variant: "playerQuery",
    compact: true,
    match: { ...normalRow, matchId: "arena-compact", matchKind: "arena", mapName: "arena-map-must-stay-hidden" },
    selected: false,
    onSelect: () => undefined,
  }),
);
assert.match(compactArenaMarkup, /playerQueryCompactMatchRow/);
assert.doesNotMatch(compactArenaMarkup, /arena-map-must-stay-hidden|playerQueryCompactMap|phase6MapPreviewShared|matchListCardHeaderMap|stadium\.png/);
assert.match(unknownMapMarkup, /未知地图/);
assert.doesNotMatch(unknownMapMarkup, /\/assets\/maps\/stadium\.png/);

assert.equal(formatPlayerQueryScoreLine(null), "-");
assert.equal(formatPlayerQueryScoreLine("0-0"), "0-0");
assert.equal(formatPlayerQueryDate("2026-01-01T00:00:00+08:00"), "01-01");
assert.equal(formatPlayerQueryDate(null), "-");

const source = readFileSync(new URL("../src/features/matches/MatchListCard.tsx", import.meta.url), "utf8");
assert.match(source, /props\.variant === "playerQuery"/);
assert.match(source, /src="\/assets\/maps\/stadium\.png"/);

const homeListSource = readFileSync(new URL("../src/features/player-query/PlayerQueryMatchList.tsx", import.meta.url), "utf8");
assert.match(homeListSource, /variant="playerQuery" compact/);
