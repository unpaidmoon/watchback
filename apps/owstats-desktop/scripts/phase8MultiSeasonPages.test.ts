import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");

assert.equal(reviewSource.includes("params.get(\"season\") || 22"), false, "review page must not hard-code S22 as default season");
assert.match(reviewSource, /getPersonalSeasonReview\(selectedSeason/, "review page should request personal review with selectedSeason");
assert.match(reviewSource, /getSeasonAnalysis\(selectedSeason/, "review page should request season analysis with the same selectedSeason");
assert.match(reviewSource, /const titleSeason = selectedSeason \?\? review\.summary\.season/, "review title should use selectedSeason before backend summary season");
