import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { phase6NavItems } from "../src/shared/phase6Shell";
import { qaRegionsForPath } from "../src/shared/qaFeedback";

const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const analyticsSource = readFileSync(new URL("../src/pages/FriendAnalyticsPage.tsx", import.meta.url), "utf8");
const reviewCss = readFileSync(new URL("../src/styles/features/review.css", import.meta.url), "utf8");

assert.equal(phase6NavItems.some((item) => item.label === "分析" || item.path === "/analytics/friends"), false, "top capsule nav should not include analysis");
assert.equal(analyticsSource.includes("phase6PageHeader"), false, "analysis page should remove analysis model overview header");
assert.equal(analyticsSource.includes("MetricCard"), false, "analysis page should remove analysis model overview metric cards");
assert.equal(analyticsSource.includes("export function SeasonAnalysisModules"), true, "analysis modules should be reusable outside the analysis page");
assert.equal(reviewSource.includes("getSeasonAnalysis"), true, "review page should load season analysis data");
assert.equal(reviewSource.includes("FriendAnalysisModules") && reviewSource.includes("MapAnalysisModules"), true, "review page should render moved analysis modules");

for (const view of ["overview", "friends", "achievements"]) {
  assert.match(reviewSource, new RegExp(`value="${view}"`), `review renders ${view} tab`);
}
assert.doesNotMatch(reviewSource, /value="roles"|value="maps"/, "职责与英雄和地图不应再作为独立标签");
assert.match(reviewSource, /parseReviewView\(params\.get\("view"\)\)/);
assert.match(reviewSource, /nextParams\.set\("view", nextView\)/);
assert.match(reviewSource, /<FriendAnalysisModules/);
assert.match(reviewSource, /<MapAnalysisModules/);
assert.match(
  reviewSource,
  /function ReviewOverview[\s\S]*?<RoleAndHeroView review=\{review\}[\s\S]*?<section className="reviewMapAnalysis">[\s\S]*?<MapAnalysisModules/,
  "概览应按职责与英雄、地图的顺序组合完整分析",
);
assert.doesNotMatch(reviewSource, /reviewOverviewPrimary|reviewOverviewSecondary/);
assert.match(reviewCss, /\.reviewMapAnalysis\s*\{[\s\S]*?grid-column:\s*1\s*\/\s*-1;/, "概览地图分析应横跨宽屏两列");
assert.doesNotMatch(reviewSource, /<SeasonAnalysisModules analysis=\{analysis\}/);
assert.match(reviewSource, /<PageHeader/);
assert.match(reviewSource, /<PageSeasonSelect/);
assert.match(reviewSource, /<PageLoadingState/);
assert.match(reviewSource, /<PageErrorState/);
assert.match(reviewSource, /<PageEmptyState/);
assert.match(reviewSource, /\}, \[selectedSeason\]\);/s, "review data requests depend on season, not view");
assert.doesNotMatch(reviewSource, /\[selectedSeason,\s*view\]/, "tab switches must reuse loaded season data");
assert.match(reviewSource, /<nav className="reviewTabs" aria-label="战报内容">/, "战报页内导航应有可访问名称");
assert.match(reviewSource, /aria-current=\{active \? "page" : undefined\}/, "战报当前标签应暴露当前页面语义");
assert.doesNotMatch(
  reviewSource,
  /role="tablist"|role="tab"|aria-selected=/,
  "战报应保持完整的页内导航模式，不混用不完整的 tab 语义",
);

const reviewRegions = qaRegionsForPath("/review").map((region) => region.id);
assert.deepEqual(reviewRegions, ["NAV-CAPSULE", "REV-RANKS", "FRD-BOARDS", "FRD-LINEUP", "FRD-MAPS"], "review QA regions should include moved analysis modules");
