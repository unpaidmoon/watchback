import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const source = `${readFileSync(new URL("../src/features/review/AchievementWall.tsx", import.meta.url), "utf8")}\n${readFileSync(new URL("../src/shared/reviewAchievements.ts", import.meta.url), "utf8")}`;
const styles = readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");

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
