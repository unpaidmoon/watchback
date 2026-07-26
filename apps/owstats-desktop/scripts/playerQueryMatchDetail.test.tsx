import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MatchDetailContent } from "../src/features/matches/MatchDetailContent";
import { displayPlayerQueryDetail } from "../src/features/player-query/PlayerQueryMatchesWorkbench";
import { fixtureArenaDetail, fixtureNormalDetail } from "./uiVisualAcceptance.fixture";

const renderQueryDetail = (detail: typeof fixtureNormalDetail | typeof fixtureArenaDetail) => renderToStaticMarkup(
  createElement(MatchDetailContent, { detail, loading: false, error: null, onRetry: () => undefined }),
);

const normalMarkup = renderQueryDetail(fixtureNormalDetail);
assert.match(normalMarkup, /aria-label="普通对局详情"/);
assert.match(normalMarkup, /香巴里寺院/);
assert.match(normalMarkup, /队友数据/);
assert.match(normalMarkup, /对手数据/);
assert.match(normalMarkup, /安娜/);
assert.doesNotMatch(normalMarkup, /matchFavoriteEditor|AI 锐评|重新评价|本地徽章|标签/);

const arenaMarkup = renderQueryDetail(fixtureArenaDetail);
assert.match(arenaMarkup, /aria-label="角斗对局详情"/);
assert.match(arenaMarkup, /全场汇总/);
assert.match(arenaMarkup, /回合-1/);
assert.match(arenaMarkup, /队友数据/);
assert.match(arenaMarkup, /对手数据/);
assert.match(arenaMarkup, /phase7CompactPlayerExpandable/, "arena players retain expandable equipment and power details");
assert.doesNotMatch(arenaMarkup, /matchFavoriteEditor|AI 锐评|重新评价|本地徽章|标签/);

const loadingMarkup = renderToStaticMarkup(createElement(MatchDetailContent, {
  detail: fixtureNormalDetail,
  loading: true,
  error: null,
  onRetry: () => undefined,
}));
assert.match(loadingMarkup, /正在读取对局详情/);
assert.doesNotMatch(loadingMarkup, /香巴里寺院|队友数据|对手数据/, "loading must not retain the previous match");

let retried = false;
const errorMarkup = renderToStaticMarkup(createElement(MatchDetailContent, {
  detail: null,
  loading: false,
  error: "读取失败",
  onRetry: () => { retried = true; },
}));
assert.match(errorMarkup, /读取失败/);
assert.equal(retried, false);

const defensive = displayPlayerQueryDetail({
  ...fixtureArenaDetail,
  teammates: null,
  enemies: undefined,
  teamAwards: undefined,
  arenaDetail: { ...fixtureArenaDetail.arenaDetail!, rounds: undefined },
} as unknown as Parameters<typeof displayPlayerQueryDetail>[0]);
assert.deepEqual(defensive.teammates, []);
assert.deepEqual(defensive.enemies, []);
assert.deepEqual(defensive.teamAwards, []);
assert.deepEqual(defensive.arenaDetail?.rounds, []);

const enriched = displayPlayerQueryDetail({
  ...fixtureNormalDetail,
  gameMode: null,
  modeLabel: null,
  mapGuid: null,
  mapName: null,
}, {
  matchId: fixtureNormalDetail.matchId,
  matchKind: "normal",
  result: "win",
  score: "2-0",
  mapKey: "suravasa",
  mapName: "苏拉瓦萨",
  heroKey: null,
  heroName: null,
  kills: null,
  assists: null,
  deaths: null,
  playedAt: null,
}, "SportOpen");
assert.equal(enriched.gameMode, "SportOpen");
assert.equal(enriched.modeLabel, "开放竞技");
assert.equal(enriched.mapGuid, "suravasa");
assert.equal(enriched.mapName, "苏拉瓦萨");

const exactArenaMode = displayPlayerQueryDetail({
  ...fixtureArenaDetail,
  gameMode: null,
  modeLabel: "arena",
}, undefined, "SportFight");
assert.equal(exactArenaMode.gameMode, "SportFight");
assert.equal(exactArenaMode.modeLabel, "角斗竞技");

console.log("player query match detail tests passed");
