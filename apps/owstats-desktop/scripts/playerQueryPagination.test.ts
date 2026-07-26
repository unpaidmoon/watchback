import assert from "node:assert/strict";
import { failedPaginationPage, hasNewDownwardScrollIntent, initialPaginationEndReached, isPlayerQuerySentinelNear, mergePlayerQueryMatches, nextPaginationPage, PLAYER_QUERY_PAGE_SIZE } from "../src/features/player-query/usePlayerQueryPagination";

const row = (matchId: string) => ({ matchId, matchKind:"normal" as const, result:"win" as const, score:"1:0", mapName:"地图", heroKey:null, heroName:null, kills:null, assists:null, deaths:null, playedAt:null });
assert.equal(PLAYER_QUERY_PAGE_SIZE, 12);
assert.equal(hasNewDownwardScrollIntent(0, 0), false, "initial/layout scroll event has no user downward intent");
assert.equal(hasNewDownwardScrollIntent(12, 11), false);
assert.equal(hasNewDownwardScrollIntent(11, 12), true);
assert.equal(isPlayerQuerySentinelNear(0, 500, 1000), false);
assert.equal(isPlayerQuerySentinelNear(405, 500, 1000), true);
assert.deepEqual(mergePlayerQueryMatches([row("a"),row("b")],[row("b"),row("c"),row("c")]).map((item)=>item.matchId),["a","b","c"]);
assert.deepEqual(nextPaginationPage(2, { page: 2, matches: [row("c")], endReached: false }), { nextPage: 3, endReached: false, retryPage: null }, "short filtered page must not override backend pagination marker");
assert.deepEqual(nextPaginationPage(2, { page: 2, matches: Array.from({length:12},(_,index)=>row(String(index))), endReached: true }), { nextPage: 3, endReached: true, retryPage: null });
assert.deepEqual(failedPaginationPage(2), { nextPage: 2, retryPage: 2 }, "failure retries the same page without advancing");
assert.equal(initialPaginationEndReached(undefined), false, "initial end state never infers from rendered row count");
console.log("player query pagination tests passed");
