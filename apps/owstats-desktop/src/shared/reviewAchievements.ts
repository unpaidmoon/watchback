import type { CareerAchievement } from "./types";

export const romanLevels = ["I", "II", "III"] as const;

export function activeBadgeLevel(achievement: CareerAchievement): 1 | 2 | 3 {
  if (achievement.currentLevel === 2) return 2;
  if (achievement.currentLevel >= 3) return 3;
  return 1;
}

export function formatAchievementUnlockDate(timestamp: number | null) {
  if (timestamp === null || !Number.isFinite(timestamp)) return null;
  const date = new Date(timestamp * 1000);
  if (Number.isNaN(date.getTime())) return null;
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}.${month}.${day} 解锁`;
}
