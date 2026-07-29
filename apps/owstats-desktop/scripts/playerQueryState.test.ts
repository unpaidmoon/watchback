import assert from "node:assert/strict";
import { alignIdlePlayerQueryMode, beginPlayerSearch, changePlayerQueryMode, changePlayerQuerySeason, initialPlayerQueryState, isAuthenticationError, isValidBattleTag, listScopeKeyForMode, normalizeBattleTag, PLAYER_QUERY_HOME_HERO_LIMIT, PLAYER_QUERY_HOME_MATCH_LIMIT, visibleQueryHeroes } from "../src/features/player-query/playerQueryState";
import { decodePlayerQueryError } from "../src/features/player-query/playerQueryApi";
import { requestPlayerQueryModeScope } from "../src/features/player-query/usePlayerQuerySession";

const row = (matchId: string) => ({ matchId, matchKind: "normal" as const, result: "win", score: "1:0", mapName: "地图", heroKey: null, heroName: null, kills: null, assists: null, deaths: null, playedAt: null });

assert.equal(normalizeBattleTag(" 西条阳＃5161 "), "西条阳#5161");
assert.equal(isValidBattleTag("西条阳＃5161"), true); assert.equal(isValidBattleTag("西条阳"), false);
const initial = initialPlayerQueryState("LeisurePreset");
const reorderedInitial = alignIdlePlayerQueryMode(initial, "SportFight");
assert.equal(reorderedInitial.mode, "SportFight");
assert.equal(reorderedInitial.listScopeKey, "arena:SportFight");
assert.equal(alignIdlePlayerQueryMode({ ...reorderedInitial, search: { status: "success", data: { sessionId: "session", battleTag: "Player#123" }, error: null } }, "LeisurePreset").mode, "SportFight", "loaded settings must not replace an active query mode");
const first = beginPlayerSearch(initial, "Player#123", "LeisurePreset"); assert.equal(first.season, 23);
assert.equal(beginPlayerSearch({ ...initial, season: 20 }, "Player#123", "LeisurePreset").season, 20, "the first query uses the season selected while idle");
const changed = changePlayerQueryMode(changePlayerQuerySeason({ ...first, battleTag: "Player#123" }, 20), "SportOpen");
assert.equal(changed.matchesEndReached, false, "a new season/mode must discard the previous raw-page end marker");
const refreshed = beginPlayerSearch(changed, "player#123", "LeisurePreset"); assert.equal(refreshed.season, 20); assert.equal(refreshed.mode, "SportOpen");
const replaced = beginPlayerSearch(refreshed, "Other#9", "Leisure6v6"); assert.equal(replaced.season, 23); assert.equal(replaced.mode, "Leisure6v6");
assert.equal(listScopeKeyForMode("LeisurePreset"), "normal:leisure");
assert.equal(listScopeKeyForMode("Leisure6v6"), "normal:leisure");
assert.equal(listScopeKeyForMode("SportPreset"), "normal:sport");
assert.equal(listScopeKeyForMode("SportOpen"), "normal:sport");
const populated = { ...initialPlayerQueryState("LeisurePreset"), matches: { status: "success" as const, data: [row("kept")], error: null }, matchesEndReached: true };
const sameListScope = changePlayerQueryMode(populated, "Leisure6v6");
assert.equal(sameListScope.matches, populated.matches, "same list scope preserves the loaded list");
assert.equal(sameListScope.matchesEndReached, true);
assert.equal(sameListScope.listGeneration, populated.listGeneration);
assert.equal(sameListScope.heroGeneration, populated.heroGeneration + 1);
const otherListScope = changePlayerQueryMode(populated, "SportPreset");
assert.equal(otherListScope.matches.status, "loading");
assert.equal(otherListScope.matchesEndReached, false);
assert.equal(otherListScope.listGeneration, populated.listGeneration + 1);
assert.deepEqual(decodePlayerQueryError({ kind: "upstream", message: "第二页失败", secret: "do-not-show" }), { kind: "upstream", message: "第二页失败" });
assert.equal(decodePlayerQueryError({ nope: true }).message, "请求失败，请稍后重试");
assert.equal(isAuthenticationError({ kind: "sessionInvalid", message: "查询会话已结束" }), true);
assert.equal(PLAYER_QUERY_HOME_MATCH_LIMIT, 12); assert.equal(PLAYER_QUERY_HOME_HERO_LIMIT, 9);
assert.deepEqual(visibleQueryHeroes([{ heroGuid:"a",heroName:"A",gameTimeSeconds:0,wins:null,games:null,winRate:null },{ heroGuid:"b",heroName:"B",gameTimeSeconds:1,wins:null,games:null,winRate:null }]).map((hero) => hero.heroName), ["B"]);

let listCalls = 0; let heroCalls = 0;
const modeApi = {
  modePage: async () => { listCalls += 1; return { page: 1, matches: [], endReached: true }; },
  modeHeroes: async () => { heroCalls += 1; return []; },
};
await Promise.all([
  requestPlayerQueryModeScope(modeApi, "matches", "session", 23, "LeisurePreset"),
  requestPlayerQueryModeScope(modeApi, "heroes", "session", 23, "LeisurePreset"),
]);
assert.deepEqual({ listCalls, heroCalls }, { listCalls: 1, heroCalls: 1 }, "initial mode resources are independent and concurrent");
await requestPlayerQueryModeScope(modeApi, "heroes", "session", 23, "LeisurePreset");
assert.deepEqual({ listCalls, heroCalls }, { listCalls: 1, heroCalls: 2 }, "hero retry must not request the list");
await requestPlayerQueryModeScope(modeApi, "matches", "session", 23, "LeisurePreset");
assert.deepEqual({ listCalls, heroCalls }, { listCalls: 2, heroCalls: 2 }, "list retry must not request heroes");
console.log("player-query state tests passed");
