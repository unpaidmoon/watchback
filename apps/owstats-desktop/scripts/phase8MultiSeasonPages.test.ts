import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  createLatestRequestGuard,
  hasReviewSeasonData,
  parseReviewSeasonParam,
  requestPersonalSeasonReviewOnce,
  ReviewEmptyState,
  shouldRequestReviewSeason,
} from "../src/pages/PersonalReviewPage";
import type { PersonalSeasonReview } from "../src/shared/types";
import { createLatestRankingRequestGuard, parseSeasonParam } from "../src/pages/RankingPage";
import {
  createLatestFriendAnalysisRequestGuard,
  FriendAnalysisEmptyState,
  hasFriendAnalysisSeasonData,
  normalizeFriendAnalysisRequestedSeason,
  parseFriendAnalysisSeasonParam,
} from "../src/pages/FriendAnalyticsPage";

const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");
const analyticsSource = readFileSync(new URL("../src/pages/FriendAnalyticsPage.tsx", import.meta.url), "utf8");
const friendDetailSource = readFileSync(new URL("../src/pages/FriendDetailPage.tsx", import.meta.url), "utf8");
const matchesSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");
const filterBarSource = readFileSync(new URL("../src/features/matches/FilterBar.tsx", import.meta.url), "utf8");
const matchFiltersSource = readFileSync(new URL("../src/features/matches/matchFilters.ts", import.meta.url), "utf8");

assert.equal(reviewSource.includes("params.get(\"season\") || 22"), false, "review page must not hard-code S22 as default season");
assert.equal(reviewSource.includes("selectableSeasons"), false, "review page must not keep a fixed season list");
assert.match(reviewSource, /review\?\.availableSeasons \?\? \[\]/, "review page should use backend-discovered seasons");
assert.match(reviewSource, /seasons=\{availableSeasons\}/, "review page should bind the selector to backend-discovered seasons");
assert.match(reviewSource, /requestPersonalSeasonReviewOnce\(selectedSeason/, "review page should request personal review with selectedSeason");
assert.match(reviewSource, /getSeasonAnalysis\(selectedSeason/, "review page should request season analysis with the same selectedSeason");
assert.match(reviewSource, /const titleSeason = selectedSeason \?\? review\.summary\.season/, "review title should use selectedSeason before backend summary season");
assert.equal(shouldRequestReviewSeason(undefined, null), true, "review should discover the latest season when the URL omits season");
assert.equal(shouldRequestReviewSeason(23, null), true, "review should request an explicit season that is not loaded");
assert.equal(shouldRequestReviewSeason(23, 23), false, "review should reuse the just-discovered season response");
assert.equal(shouldRequestReviewSeason(22, 23), true, "review should request after changing to a different season");
assert.match(reviewSource, /nextParams\.set\("season", String\(backendSeason\)\)/, "a discovered review season should be written to the URL");
assert.match(reviewSource, /setParams\(nextParams, \{ replace: true \}\)/, "discovered review season should replace the incomplete URL");
assert.match(reviewSource, /backendSeason !== selectedSeason/, "a backend season fallback should synchronize review state and URL");
assert.equal(parseReviewSeasonParam("15"), 15, "review page should accept an older locally available URL season");
assert.equal(parseReviewSeasonParam("20"), 20, "review page should accept a positive URL season");
assert.equal(parseReviewSeasonParam("0"), undefined, "review page should reject a zero URL season");
assert.equal(parseReviewSeasonParam("-1"), undefined, "review page should reject a negative URL season");
assert.equal(parseReviewSeasonParam("15.5"), undefined, "review page should reject a fractional URL season");

const coalescedReview = {} as PersonalSeasonReview;
let coalescedReviewLoads = 0;
const loadCoalescedReview = async () => {
  coalescedReviewLoads += 1;
  await Promise.resolve();
  return coalescedReview;
};
const firstCoalescedReview = requestPersonalSeasonReviewOnce(undefined, loadCoalescedReview);
const secondCoalescedReview = requestPersonalSeasonReviewOnce(undefined, loadCoalescedReview);
assert.equal(firstCoalescedReview, secondCoalescedReview, "simultaneous latest-season review requests should share one promise");
assert.equal(coalescedReviewLoads, 1, "StrictMode remount must not duplicate an in-flight review request");
await Promise.all([firstCoalescedReview, secondCoalescedReview]);
await requestPersonalSeasonReviewOnce(undefined, loadCoalescedReview);
assert.equal(coalescedReviewLoads, 2, "a settled review request must not become a stale permanent cache");

assert.match(rankingSource, /getRankings\(selectedSeason/, "ranking page should request rankings with selectedSeason");
assert.match(rankingSource, /<PageSeasonSelect/, "ranking page should render the shared page-level season selector");
assert.match(rankingSource, /value=\{selectedSeason\}/, "ranking page should bind the shared selector to selectedSeason");
assert.match(rankingSource, /seasons=\{availableSeasons\}/, "ranking page should use backend-discovered seasons");
assert.match(rankingSource, /\.set\("season", String\(nextSeason\)\)/, "ranking page selector should persist season in the URL");
assert.equal(rankingSource.includes("selectableSeasons"), false, "ranking page must not keep a fixed season list");
assert.match(rankingSource, /ranking\?\.availableSeasons/, "ranking page should render seasons returned by the backend");
assert.equal(parseSeasonParam("15"), 15, "ranking page should accept an older locally available URL season");
assert.equal(parseSeasonParam("999"), 999, "ranking page should let the backend validate a positive URL season");
assert.equal(parseSeasonParam("0"), undefined, "ranking page should reject a non-positive URL season");

assert.match(analyticsSource, /getSeasonAnalysis\(selectedSeason/, "friend analysis page should request analysis with selectedSeason");
assert.match(analyticsSource, /S\$\{selectedSeason\}/, "friend analysis page should title the selected season");
assert.match(analyticsSource, /<select[^>]+value=\{selectedSeason/, "friend analysis page should render a page-level season selector");
assert.equal(parseFriendAnalysisSeasonParam("21"), 21, "friend analysis should accept S21 URL state");
assert.equal(parseFriendAnalysisSeasonParam("23"), 23, "friend analysis should accept S23 URL state");
assert.equal(parseFriendAnalysisSeasonParam("999"), undefined, "friend analysis should ignore unsupported URL seasons");
assert.deepEqual(
  normalizeFriendAnalysisRequestedSeason("999"),
  { selectedSeason: undefined, shouldReplaceUrl: true },
  "an unsupported friend-analysis URL season must use the latest-season request and replace its URL",
);
assert.deepEqual(
  normalizeFriendAnalysisRequestedSeason(null),
  { selectedSeason: undefined, shouldReplaceUrl: false },
  "an absent friend-analysis URL season must use the latest-season request without replacing its URL",
);
assert.match(analyticsSource, /setAnalysis\(null\);[\s\S]*setError\(null\);[\s\S]*setLoading\(false\);/, "a URL season change must clear stale friend analysis state");
assert.match(analyticsSource, /<option value="">最新赛季<\/option>/, "friend analysis should render a valid initial latest-season selector option");
assert.match(analyticsSource, /正在加载好友分析/, "friend analysis should show a Chinese loading state before the latest season is known");
assert.match(analyticsSource, /buildReviewRankingHref\(season\)/);
assert.match(analyticsSource, /buildFriendDetailState\(friend\.bnetId, season, sourceView, window\.scrollY\)/);
assert.match(analyticsSource, /event\.preventDefault\(\)/, "好友详情点击应阻止使用渲染期旧 state 的默认导航");
assert.match(
  analyticsSource,
  /navigate\("\/analytics\/friends\/detail",\s*\{\s*state\s*\}\)/s,
  "好友详情应使用点击时的滚动位置显式导航",
);
assert.doesNotMatch(analyticsSource, /state=\{detailState\(\)\}/, "链接不应固化渲染期滚动位置");
assert.match(friendDetailSource, /getFriendDetails\(state\.bnetId, \{ season: state\.season \}\)/);
assert.match(friendDetailSource, /sessionStorage\.setItem\(state\.returnTo\.scrollKey/);
assert.match(friendDetailSource, /navigate\(state\.returnTo\.href\)/);
assert.doesNotMatch(friendDetailSource, /detail\?\.name \?\? bnetId/);

assert.match(matchesSource, /chooseInitialMatchFilters\(filtersFromParams\(searchParams\), cachedPage\?\.filters, hasExplicitFilters\)/, "matches page should restore URL season filters before considering cached filters");
assert.match(matchesSource, /<PageSeasonSelect/, "matches page should render its season selector in the page header");
assert.match(matchesSource, /seasons=\{options\.seasons\}/, "matches page should use locally available season options");
assert.match(matchesSource, /<FilterBar filters=\{filters\} options=\{options\} onChange=\{setFilters\} \/>/, "matches page should bind the filter bar to the page-local filter state");
assert.match(matchesSource, /paramsFromFilters\(filters, selectedMatchId\)/, "matches page should persist its current filter state, including season, to the URL");
assert.match(matchFiltersSource, /if \(next\.season !== undefined && !options\.seasons\.includes\(next\.season\)\) delete next\.season;/, "matches page should clear an unavailable URL season instead of querying a stale season");

assert.doesNotMatch(filterBarSource, />\s*赛季\s*</, "match filter bar should not duplicate the page-level season selector");
assert.equal(filterBarSource.includes("PageSeasonSelect"), false, "match filter bar should not own the page season selector");

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

async function settlePromises() {
  await new Promise<void>((resolve) => setImmediate(resolve));
}

const requestGuard = createLatestRequestGuard();
const firstRequest = requestGuard.begin();
const secondRequest = requestGuard.begin();
const firstResponse = deferred<string>();
const secondResponse = deferred<string>();
let displayedSeason: string | null = null;

void firstResponse.promise.then((value) => {
  if (requestGuard.isCurrent(firstRequest)) displayedSeason = value;
});
void secondResponse.promise.then((value) => {
  if (requestGuard.isCurrent(secondRequest)) displayedSeason = value;
});

secondResponse.resolve("S23");
await secondResponse.promise;
firstResponse.resolve("S22");
await firstResponse.promise;
assert.equal(displayedSeason, "S23", "a late response for an old season must not replace the active season");

const rankingRequestGuard = createLatestRankingRequestGuard();
const firstRankingRequest = rankingRequestGuard.begin();
const secondRankingRequest = rankingRequestGuard.begin();
const thirdRankingRequest = rankingRequestGuard.begin();
const firstRankingResponse = deferred<string>();
const secondRankingResponse = deferred<string>();
const thirdRankingResponse = deferred<string>();
let displayedRanking: string | null = null;
let rankingError: string | null = null;
let rankingLoading = true;

void firstRankingResponse.promise.then((value) => {
  if (rankingRequestGuard.isCurrent(firstRankingRequest)) displayedRanking = value;
}).catch((error: Error) => {
  if (rankingRequestGuard.isCurrent(firstRankingRequest)) rankingError = error.message;
}).finally(() => {
  if (rankingRequestGuard.isCurrent(firstRankingRequest)) rankingLoading = false;
});
void secondRankingResponse.promise.then((value) => {
  if (rankingRequestGuard.isCurrent(secondRankingRequest)) displayedRanking = value;
}).catch((error: Error) => {
  if (rankingRequestGuard.isCurrent(secondRankingRequest)) rankingError = error.message;
}).finally(() => {
  if (rankingRequestGuard.isCurrent(secondRankingRequest)) rankingLoading = false;
});
void thirdRankingResponse.promise.then((value) => {
  if (rankingRequestGuard.isCurrent(thirdRankingRequest)) displayedRanking = value;
}).catch((error: Error) => {
  if (rankingRequestGuard.isCurrent(thirdRankingRequest)) rankingError = error.message;
}).finally(() => {
  if (rankingRequestGuard.isCurrent(thirdRankingRequest)) rankingLoading = false;
});

firstRankingResponse.resolve("S22");
await firstRankingResponse.promise;
secondRankingResponse.reject(new Error("S23 请求失败"));
await secondRankingResponse.promise.catch(() => undefined);
await settlePromises();
assert.equal(displayedRanking, null, "a late ranking response for an old season must not replace the active ranking");
assert.equal(rankingError, null, "a late ranking error for an old season must not replace the active error");
assert.equal(rankingLoading, true, "a late ranking completion for an old season must not clear active loading");

thirdRankingResponse.resolve("S23");
await thirdRankingResponse.promise;
await settlePromises();
assert.equal(displayedRanking, "S23", "the latest ranking response must update the active ranking");
assert.equal(rankingLoading, false, "the latest ranking completion must clear active loading");

const friendRequestGuard = createLatestFriendAnalysisRequestGuard();
const firstFriendRequest = friendRequestGuard.begin();
const secondFriendRequest = friendRequestGuard.begin();
const firstFriendResponse = deferred<string>();
const secondFriendResponse = deferred<string>();
let displayedFriendAnalysis: string | null = null;
let friendAnalysisError: string | null = null;
let friendAnalysisLoading = true;

void firstFriendResponse.promise.then((value) => {
  if (friendRequestGuard.isCurrent(firstFriendRequest)) displayedFriendAnalysis = value;
}).catch((error: Error) => {
  if (friendRequestGuard.isCurrent(firstFriendRequest)) friendAnalysisError = error.message;
}).finally(() => {
  if (friendRequestGuard.isCurrent(firstFriendRequest)) friendAnalysisLoading = false;
});
void secondFriendResponse.promise.then((value) => {
  if (friendRequestGuard.isCurrent(secondFriendRequest)) displayedFriendAnalysis = value;
}).catch((error: Error) => {
  if (friendRequestGuard.isCurrent(secondFriendRequest)) friendAnalysisError = error.message;
}).finally(() => {
  if (friendRequestGuard.isCurrent(secondFriendRequest)) friendAnalysisLoading = false;
});

firstFriendResponse.resolve("S22");
await firstFriendResponse.promise;
await settlePromises();
assert.equal(displayedFriendAnalysis, null, "a late friend-analysis response must not replace the selected season");
assert.equal(friendAnalysisError, null, "a late friend-analysis error must not replace the selected season error");
assert.equal(friendAnalysisLoading, true, "a late friend-analysis completion must not clear active loading");

secondFriendResponse.reject(new Error("S23 请求失败"));
await secondFriendResponse.promise.catch(() => undefined);
await settlePromises();
assert.equal(friendAnalysisError, "S23 请求失败", "the current friend-analysis error must be displayed");
assert.equal(friendAnalysisLoading, false, "the current friend-analysis completion must clear loading");

const navigationFriendRequestGuard = createLatestFriendAnalysisRequestGuard();
const requestBeforeNavigation = navigationFriendRequestGuard.begin();
navigationFriendRequestGuard.invalidate();
assert.equal(navigationFriendRequestGuard.isCurrent(requestBeforeNavigation), false, "navigation must invalidate the previous friend-analysis request");

assert.equal(hasReviewSeasonData({ summary: { season: null } } as never), false, "a null backend season must show the empty review state");
assert.equal(hasReviewSeasonData({ summary: { season: 0 } } as never), false, "a zero backend season must show the empty review state");
assert.equal(hasReviewSeasonData({ summary: { season: 23 } } as never), true, "a valid backend season must render the review");

const emptyReviewMarkup = renderToStaticMarkup(createElement(ReviewEmptyState));
assert.match(emptyReviewMarkup, /暂无可用赛季战报/, "the empty review state must show a clear Chinese message");
assert.doesNotMatch(emptyReviewMarkup, /<select/, "the empty review state must not render an invalid season selector");

assert.equal(hasFriendAnalysisSeasonData({ season: null } as never), false, "a null friend-analysis season must show the empty state");
assert.equal(hasFriendAnalysisSeasonData({ season: 0 } as never), false, "a zero friend-analysis season must show the empty state");
assert.equal(hasFriendAnalysisSeasonData({ season: 23 } as never), true, "a valid friend-analysis season must render the analysis");

const emptyFriendAnalysisMarkup = renderToStaticMarkup(createElement(FriendAnalysisEmptyState));
assert.match(emptyFriendAnalysisMarkup, /暂无可用好友分析/, "the friend-analysis empty state must show a clear Chinese message");
assert.doesNotMatch(emptyFriendAnalysisMarkup, /<select/, "the friend-analysis empty state must not render an invalid season selector");
