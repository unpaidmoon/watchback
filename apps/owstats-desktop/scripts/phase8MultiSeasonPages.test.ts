import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createLatestRequestGuard, hasReviewSeasonData, ReviewEmptyState } from "../src/pages/PersonalReviewPage";

const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");

assert.equal(reviewSource.includes("params.get(\"season\") || 22"), false, "review page must not hard-code S22 as default season");
assert.match(reviewSource, /getPersonalSeasonReview\(selectedSeason/, "review page should request personal review with selectedSeason");
assert.match(reviewSource, /getSeasonAnalysis\(selectedSeason/, "review page should request season analysis with the same selectedSeason");
assert.match(reviewSource, /const titleSeason = selectedSeason \?\? review\.summary\.season/, "review title should use selectedSeason before backend summary season");

assert.match(rankingSource, /getRankings\(selectedSeason/, "ranking page should request rankings with selectedSeason");
assert.match(rankingSource, /<select[^>]+value=\{selectedSeason/, "ranking page should render a page-level season selector");
assert.match(rankingSource, /\.set\("season", String\(nextSeason\)\)/, "ranking page selector should persist season in the URL");

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
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

assert.equal(hasReviewSeasonData({ summary: { season: null } } as never), false, "a null backend season must show the empty review state");
assert.equal(hasReviewSeasonData({ summary: { season: 0 } } as never), false, "a zero backend season must show the empty review state");
assert.equal(hasReviewSeasonData({ summary: { season: 23 } } as never), true, "a valid backend season must render the review");

const emptyReviewMarkup = renderToStaticMarkup(createElement(ReviewEmptyState));
assert.match(emptyReviewMarkup, /暂无可用赛季战报/, "the empty review state must show a clear Chinese message");
assert.doesNotMatch(emptyReviewMarkup, /<select/, "the empty review state must not render an invalid season selector");
