import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const reviewCss = readFileSync(new URL("../src/styles/features/review.css", import.meta.url), "utf8");
const friendCss = readFileSync(new URL("../src/styles/features/friends.css", import.meta.url), "utf8");
const friendDetailSource = readFileSync(new URL("../src/pages/FriendDetailPage.tsx", import.meta.url), "utf8");
const reviewSource = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");

assert.match(
  reviewCss,
  /\.reviewViewGrid\[data-view="overview"\]\s*\{[^}]*align-items:\s*start;/s,
  "战报概览的两列卡片应按内容高度顶部对齐",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6Eyebrow\s*\{[^}]*color:\s*var\(--data-primary\);/s,
  "战报区块眉题应使用紫色数据强调，不应沿用旧橙色",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6FriendIntimacyRankBadge\s*\{[^}]*background:\s*var\(--data-primary\);/s,
  "队友排名徽标应使用纯紫色强调",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6FriendIntimacyBar\s+span\s*\{[^}]*background:\s*var\(--data-secondary\);/s,
  "亲密度进度应使用纯冷蓝绿色强调",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6GlassPanelShared\.solid\.phase6AchievementPanel\s*\{[^}]*background:\s*var\(--surface-secondary\);/s,
  "成就墙应回到浅色页面表面",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementCard\s*\{[^}]*background:\s*var\(--surface-control\);/s,
  "成就卡片应使用浅色中性表面",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementProgress\s+span,\s*\.reviewWorkspace\s+\.phase6AchievementModalProgress\s+span\s*\{[^}]*background:\s*var\(--data-secondary\);/s,
  "成就进度应使用纯冷蓝绿色强调",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementModal\s*\{[^}]*background:\s*var\(--surface-secondary\);/s,
  "成就详情弹窗应使用浅色页面表面",
);
assert.match(friendDetailSource, /className="detailPage phase6DetailPage friendDetailPage"/, "好友详情应有独立视觉作用域");
assert.match(friendDetailSource, /<PageHeader[\s\S]*title=\{detail\?\.name \?\? "好友详情"\}/, "好友详情应复用统一页面标题");
assert.match(
  friendCss,
  /\.friendDetailPage\s+\.phase6Eyebrow\s*\{[^}]*color:\s*var\(--data-primary\);/s,
  "好友详情眉题不应沿用旧橙色",
);
assert.match(
  friendCss,
  /\.friendDetailPage\s+\.phase6ReviewPanel\s*\{[^}]*background:\s*var\(--surface-secondary\);/s,
  "好友详情面板应使用统一浅色表面",
);
assert.match(reviewSource, /closeButtonRef\.current\?\.focus\(\)/, "成就弹窗打开时应把焦点移入弹窗");
assert.match(reviewSource, /event\.key === "Escape"/, "成就弹窗应支持 Escape 关闭");
assert.match(reviewSource, /event\.key === "Tab"/, "成就弹窗应把键盘焦点保留在弹窗内");
