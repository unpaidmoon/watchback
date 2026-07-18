import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const reviewCss = readFileSync(new URL("../src/styles/features/review.css", import.meta.url), "utf8");
const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const achievementSource = readFileSync(new URL("../src/pages/AchievementsPage.tsx", import.meta.url), "utf8");

assert.match(reviewCss, /\.reviewReportFlow\s*\{[^}]*display:\s*grid;/s);
assert.doesNotMatch(reviewCss, /reviewTabs|reviewViewGrid|data-view/);
assert.match(reviewCss, /\.reviewRoleHeroPanel\s*\{[^}]*border:\s*1px solid var\(--glass-border\);/s);
assert.match(reviewSource, /className="reviewReportFlow"/);
assert.doesNotMatch(reviewSource, /AchievementWall/);
assert.match(achievementSource, /AchievementWall/);
