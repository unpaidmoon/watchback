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
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementModal\s*\{[^}]*background:\s*var\(--action-primary\);[^}]*color:\s*var\(--action-on-primary\);/s,
  "成就详情弹窗应使用统一深炭黑陈列面",
);
assert.match(reviewCss, /\.reviewWorkspace\s+\.phase6AchievementBadgeStage\s*\{[^}]*background:\s*transparent;/s, "成就等级区应在深色弹窗内压平");
assert.doesNotMatch(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementBadgeStage\.active\s*\{[^}]*var\(--data-primary\)/s,
  "当前等级不应使用紫色边线或文字",
);
assert.doesNotMatch(reviewCss, /phase6AchievementModalStats|phase6AchievementModalProgress/, "成就详情不应保留摘要或进度区域");
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6IconButton:focus-visible\s*\{[^}]*outline:\s*none\s*!important;[^}]*background:\s*color-mix\(in srgb, var\(--action-on-primary\) 12%, transparent\);/s,
  "详情关闭按钮应使用中性明度焦点而非彩色外环",
);
