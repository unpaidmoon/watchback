import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { phase6NavItems } from "../src/shared/phase6Shell";
import { qaRegionsForPath } from "../src/shared/qaFeedback";

const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const analyticsSource = readFileSync(new URL("../src/pages/FriendAnalyticsPage.tsx", import.meta.url), "utf8");

assert.equal(phase6NavItems.some((item) => item.label === "分析" || item.path === "/analytics/friends"), false, "top capsule nav should not include analysis");
assert.equal(analyticsSource.includes("phase6PageHeader"), false, "analysis page should remove analysis model overview header");
assert.equal(analyticsSource.includes("MetricCard"), false, "analysis page should remove analysis model overview metric cards");
assert.equal(analyticsSource.includes("export function SeasonAnalysisModules"), true, "analysis modules should be reusable outside the analysis page");
assert.equal(reviewSource.includes("getSeasonAnalysis"), true, "review page should load season analysis data");
assert.equal(reviewSource.includes("SeasonAnalysisModules"), true, "review page should render moved analysis modules");

const reviewRegions = qaRegionsForPath("/review").map((region) => region.id);
assert.deepEqual(reviewRegions, ["NAV-CAPSULE", "REV-RANKS", "FRD-BOARDS", "FRD-LINEUP", "FRD-MAPS"], "review QA regions should include moved analysis modules");
