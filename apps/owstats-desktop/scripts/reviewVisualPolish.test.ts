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
  /\.reviewWorkspace\s+\.phase6Eyebrow\s*\{[^}]*color:\s*var\(--text-secondary\);[^}]*font-weight:\s*400;/s,
  "战报区块眉题应使用中性次级文字和常规字重",
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
  /\.reviewWorkspace\s+\.phase6GlassPanelShared\.solid\.phase6AchievementPanel\s*\{[^}]*background:\s*var\(--glass-primary\);/s,
  "成就墙应使用一级玻璃表面",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementCard\s*\{[^}]*background:\s*var\(--glass-secondary\);/s,
  "成就卡片应使用次级玻璃表面",
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
  /\.friendDetailPage\s+\.phase6Eyebrow\s*\{[^}]*color:\s*var\(--text-secondary\);[^}]*font-weight:\s*400;/s,
  "好友详情眉题应使用中性次级文字和常规字重",
);
assert.match(
  friendCss,
  /\.friendDetailPage\s+\.phase6ReviewPanel\s*\{[^}]*border-color:\s*var\(--glass-border\);[^}]*background:\s*var\(--glass-primary\);[^}]*box-shadow:\s*none;/s,
  "好友详情面板应使用低边界玻璃表面而非叠层卡片",
);
assert.match(
  reviewCss,
  /\.reviewRoleHeroPanel\s*\{[^}]*border:\s*1px solid var\(--glass-border\);[^}]*background:\s*var\(--glass-primary\);[^}]*box-shadow:\s*none;/s,
  "战报职责与英雄面板应使用低边界玻璃表面且不叠加阴影",
);
assert.match(
  reviewCss,
  /\.reviewHeroSummary\s*\{[^}]*grid-template-columns:\s*repeat\(3, minmax\(0, 1fr\)\);[^}]*margin-bottom:\s*var\(--space-5\);[^}]*border:\s*1px solid var\(--glass-border\);/s,
  "战报赛季事实带应置于顶部并使用三列低层级表面",
);
assert.match(
  reviewCss,
  /\.reviewHeroSummary\s*>\s*\*\s*\+\s*\*\s*\{[^}]*border-left:\s*1px solid var\(--glass-border\);/s,
  "战报赛季事实带应使用细分隔线而非独立小卡",
);
assert.match(
  reviewCss,
  /\.reviewHeroAvatarList\s*\{[^}]*display:\s*flex;[^}]*gap:\s*var\(--space-2\);/s,
  "战报常用英雄头像应保持紧凑间距",
);
assert.match(
  reviewCss,
  /\.reviewHeroSummary\s+\.metric\s+strong\s*\{[^}]*font-size:\s*32px;/s,
  "战报赛季事实的核心数值应提升为主视觉",
);
assert.match(reviewSource, /closeButtonRef\.current\?\.focus\(\)/, "成就弹窗打开时应把焦点移入弹窗");
assert.match(reviewSource, /event\.key === "Escape"/, "成就弹窗应支持 Escape 关闭");
assert.match(reviewSource, /event\.key === "Tab"/, "成就弹窗应把键盘焦点保留在弹窗内");
