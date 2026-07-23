import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const component = readFileSync(new URL("../src/features/review/AchievementWall.tsx", import.meta.url), "utf8");
const styles = readFileSync(new URL("../src/styles/features/review.css", import.meta.url), "utf8");
for (const required of ["achievement.tiers.map", "achievementTier.requirement", "formatAchievementUnlockDate", "phase6AchievementUnlockDate", "achievementTier.level <= achievement.currentLevel"]) assert.equal(component.includes(required), true, required);
assert.equal(component.includes("unlockedAt !== null"), false, "解锁状态不得依赖解锁时间");
for (const removed of ["phase6Eyebrow", "phase6AchievementModalStats", "phase6AchievementModalProgress", "function Metric", "当前等级", "下一等级", "最高等级"]) assert.equal(component.includes(removed), false, removed);
assert.match(styles, /\.phase6AchievementGlow\s*\{[^}]*pointer-events:\s*none/s);
assert.match(styles, /\.phase6AchievementBadgeStage\s+\.phase6AchievementGlow\s*\{[^}]*width:\s*320px;[^}]*height:\s*320px;/s);
