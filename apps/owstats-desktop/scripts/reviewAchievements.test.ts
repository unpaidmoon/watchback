import assert from "node:assert/strict";
import { activeBadgeLevel, formatAchievementUnlockDate } from "../src/shared/reviewAchievements.ts";
import type { CareerAchievement } from "../src/shared/types.ts";

const achievement: CareerAchievement = { key: "win-streak", title: "常胜将军", detail: "连续表现奖章", currentLevel: 2, tiers: [] };
assert.equal(activeBadgeLevel(achievement), 2);
assert.equal(activeBadgeLevel({ ...achievement, currentLevel: 0 }), 1);
assert.equal(activeBadgeLevel({ ...achievement, currentLevel: 1 }), 1);
assert.equal(activeBadgeLevel({ ...achievement, currentLevel: 3 }), 3);
assert.equal(activeBadgeLevel({ ...achievement, currentLevel: 99 }), 3);
assert.equal(formatAchievementUnlockDate(null), null);
assert.equal(formatAchievementUnlockDate(Number.NaN), null);
const localUnlockTimestamp = new Date(2026, 0, 2, 12).getTime() / 1000;
assert.equal(formatAchievementUnlockDate(localUnlockTimestamp), "2026.01.02 解锁");
