import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { qaRegionsForPath } from "../src/shared/qaFeedback";

const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const analyticsSource = readFileSync(new URL("../src/pages/FriendAnalyticsPage.tsx", import.meta.url), "utf8");
const reviewCss = readFileSync(new URL("../src/styles/features/review.css", import.meta.url), "utf8");
const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");

assert.equal(analyticsSource.includes("export function SeasonAnalysisModules"), true);
assert.match(appSource, /function FriendAnalyticsRoute\(\)[\s\S]*?<Navigate to=\{season === undefined \? "\/review" : buildReviewHref\(season\)\} replace \/>/);
assert.match(appSource, /<Route path="\/analytics\/friends" element=\{<FriendAnalyticsRoute \/>\} \/>/);
assert.equal(reviewSource.includes("getSeasonAnalysis"), true);
assert.equal(reviewSource.includes("usePersonalSeasonReview"), true);
assert.doesNotMatch(reviewSource, /ReviewView|ReviewTabButton|reviewTabs|AchievementWall|params\.get\("view"\)/);
assert.match(reviewSource, /<div className="reviewReportFlow">[\s\S]*?<RoleAndHeroView review=\{review\} \/>[\s\S]*?<FriendAnalysisModules[\s\S]*?<MapAnalysisModules/);
assert.match(reviewCss, /\.reviewReportFlow\s*\{/);
assert.doesNotMatch(reviewCss, /reviewTabs|reviewViewGrid|data-view/);
assert.deepEqual(qaRegionsForPath("/review").map((region) => region.id), ["NAV-CAPSULE", "REV-RANKS", "FRD-BOARDS", "FRD-LINEUP", "FRD-MAPS"]);
