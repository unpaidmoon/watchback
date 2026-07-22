import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const component = readFileSync(new URL("../src/features/review/AchievementWall.tsx", import.meta.url), "utf8");
const source = `${component}\n${readFileSync(new URL("../src/shared/reviewAchievements.ts", import.meta.url), "utf8")}`;
const styles = readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");
const reviewStyles = readFileSync(new URL("../src/styles/features/review.css", import.meta.url), "utf8");
const cardSource = component.slice(component.indexOf("function AchievementCard"), component.indexOf("function AchievementDialog"));

for (const label of ["成就墙", "常胜将军", "胜率机器", "高光制造者", "战局核心", "招牌打法", "巅峰一战", "本命专精", "战术变阵"]) {
  assert.equal(source.includes(label), true, `review page should render achievement label: ${label}`);
}

assert.equal(source.includes("赛季成就墙"), false, "review page should rename the achievement wall title");
assert.equal(source.includes("把胜率、高光、英雄池和连胜记录转成可追溯的赛季奖章。"), false, "review page should remove the achievement wall explainer");
assert.equal(source.includes("buildReviewAchievements"), true, "review page should derive achievements from review data");
assert.equal(source.includes("achievementBadgeUrl"), true, "review page should use generated achievement badge assets");
assert.equal(source.includes("AchievementDialog"), true, "review page should include click-through achievement detail dialog");
assert.equal(source.includes("romanLevels"), true, "review page should model fixed I / II / III achievement levels");
assert.equal(source.includes("phase6AchievementWall"), true, "review page should include achievement wall markup");
assert.equal(styles.includes(".phase6AchievementWall"), true, "achievement wall styles should be defined");
assert.equal(
  styles.includes(".phase6GlassPanelShared.solid.phase6AchievementPanel"),
  true,
  "achievement panel background should override the shared solid glass panel background",
);
assert.equal(
  styles.includes("linear-gradient(180deg, rgba(42, 48, 58, 0.9), rgba(63, 70, 81, 0.8))"),
  true,
  "achievement panel should use the same dark base as achievement cards",
);
assert.equal(styles.includes(".phase6AchievementHeader p"), false, "achievement header should not keep explainer-only styles");
assert.equal(styles.includes(".phase6AchievementMedalImage"), true, "achievement badge image styles should be defined");
assert.equal(styles.includes(".phase6AchievementBadgeTrack"), true, "achievement detail badge track styles should be defined");
assert.equal(styles.includes("grayscale(1) saturate(0.35) brightness(0.55)"), true, "locked achievement badges should be dimmed with CSS");

for (const required of ["phase6AchievementMedalImage", "achievement.title", "achievement.detail", "onClick={onOpen}"]) {
  assert.equal(cardSource.includes(required), true, `achievement card should keep ${required}`);
}
for (const removed of ["achievement.metric", "phase6AchievementProgress", "phase6AchievementTierRail", "phase6AchievementNext", "<Evidence"]) {
  assert.equal(cardSource.includes(removed), false, `achievement card should remove ${removed}`);
}
assert.equal(component.includes("function AchievementDialog"), true, "achievement detail dialog should remain available");
assert.doesNotMatch(
  reviewStyles,
  /\.reviewWorkspace\s+\.phase6AchievementCard:focus-within\s*\{[^}]*var\(--data-primary\)/s,
  "achievement card focus should not use the purple data accent",
);
assert.match(
  reviewStyles,
  /\.phase6AchievementCard:hover\s*\{[^}]*background:\s*color-mix/s,
  "achievement cards should keep their temporary mouse hover background",
);
assert.match(
  reviewStyles,
  /\.phase6AchievementCard:focus-within\s*\{[^}]*background:\s*transparent/s,
  "achievement cards should not keep a selected-looking background after click",
);
assert.match(
  reviewStyles,
  /\.phase6AchievementCardButton:focus-visible\s*\{[^}]*outline:/s,
  "achievement card keyboard focus should use an outline instead of a selected background",
);
assert.doesNotMatch(
  reviewStyles,
  /\.phase6AchievementCard:hover\s+\.phase6AchievementMedalImage,\s*\n\.reviewWorkspace\s+\.phase6AchievementCard:focus-within/s,
  "achievement badge lift should only respond to mouse hover",
);
assert.match(
  reviewStyles,
  /\.phase6AchievementBadgeStage\.active\s*\{[^}]*background:\s*transparent/s,
  "achievement detail current tier should not use a selected-looking background",
);

assert.equal(cardSource.includes("tier-${achievement.currentLevel}"), true, "achievement cards should expose their current tier");
assert.equal(component.includes("tier-${tier}"), true, "achievement detail stages should expose their fixed tier");
assert.match(
  reviewStyles,
  /\.phase6AchievementCard\.tier-2\s+\.phase6AchievementGlow/,
  "unlocked level II achievement cards should keep their dynamic glow visible without hover",
);
assert.match(
  reviewStyles,
  /\.phase6AchievementCard\.tier-3\s+\.phase6AchievementGlow/,
  "unlocked level III achievement cards should keep their dynamic glow visible without hover",
);
assert.doesNotMatch(
  reviewStyles,
  /\.phase6AchievementCard\.tier-(?:0|1)\s+\.phase6AchievementGlow/,
  "inactive and level I achievement cards should not receive dynamic tier glow",
);
assert.doesNotMatch(
  reviewStyles,
  /\.phase6AchievementCard\.tier-(?:2|3):is\(:hover, :focus-within\)\s*\{[^}]*radial-gradient/s,
  "achievement cards should not use tier-colored background gradients",
);
assert.doesNotMatch(
  reviewStyles,
  /\.phase6AchievementBadgeStage\.tier-(?:2|3)\.unlocked\s*\{[^}]*radial-gradient/s,
  "achievement detail stages should not use tier-colored background gradients",
);

assert.equal(component.includes("phase6AchievementGlow"), true, "achievement badges should render a decorative glow layer");
assert.equal(component.includes("phase6AchievementGlowParticles"), true, "achievement badges should render a separately animated particle layer");
assert.match(reviewStyles, /\.phase6AchievementGlow\s*\{[^}]*pointer-events:\s*none/s, "achievement glow should not intercept interaction");
assert.match(reviewStyles, /achievementGlowBreathe/, "achievement glow should define a breathing animation");
assert.match(reviewStyles, /achievementGlowRotate/, "achievement glow should define a slow ray rotation");
assert.match(reviewStyles, /achievementGlowSparkle/, "level III particles should define a restrained sparkle animation");
assert.match(reviewStyles, /achievement-glow-tier-2\.webp/, "level II glow should use its packaged WebP asset");
assert.match(reviewStyles, /achievement-glow-tier-3\.webp/, "level III glow should use its packaged WebP asset");
assert.match(
  reviewStyles,
  /@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?\.phase6AchievementGlow[\s\S]*?animation:\s*none/,
  "reduced motion should keep the glow static",
);
assert.match(
  reviewStyles,
  /\.phase6AchievementGlow\s*\{[^}]*width:\s*230px;[^}]*height:\s*230px;/s,
  "wall glow canvas should fit the transparent achievement badge",
);
assert.match(
  reviewStyles,
  /\.phase6AchievementBadgeStage\s+\.phase6AchievementGlow\s*\{[^}]*top:\s*-48px;[^}]*width:\s*320px;[^}]*height:\s*320px;/s,
  "detail glow canvas should extend beyond the transparent achievement badge",
);
