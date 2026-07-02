import { formatWinRate } from "./rankingUtils";
import type { EvidenceLink, PersonalSeasonReview } from "./types";

export type AchievementKey = "win-streak" | "win-rate" | "s-grade" | "carry" | "signature" | "peak" | "hero-focus" | "switching";

export type ReviewAchievement = {
  key: AchievementKey;
  title: string;
  metric: string;
  detail: string;
  value: number;
  thresholds: readonly [number, number, number];
  unit: string;
  currentLevel: 0 | 1 | 2 | 3;
  progress: number;
  evidence?: EvidenceLink;
};

export const romanLevels = ["I", "II", "III"] as const;

const achievementOrder: AchievementKey[] = ["win-streak", "win-rate", "s-grade", "carry", "signature", "peak", "hero-focus", "switching"];

export function buildReviewAchievements(review: PersonalSeasonReview): ReviewAchievement[] {
  const peakBest = review.personalBests[0];
  const mainHero = review.heroPool.mainHeroes[0];
  const winRatePercent = review.summary.winRate * 100;
  const mainHeroSharePercent = review.heroPool.top1Share * 100;
  const signatureCount = review.phase5Highlights.mostCommonHighlight ? Math.max(1, review.phase5Highlights.sGradeCount) : 0;
  const peakValue = peakBest?.value ?? 0;

  return [
    buildAchievement({
      key: "win-streak",
      title: "常胜将军",
      metric: `最长连胜 ${review.streaks.longestWinStreak} 局`,
      detail: "连续表现奖章",
      value: review.streaks.longestWinStreak,
      thresholds: [3, 5, 8],
      unit: "局",
    }),
    buildAchievement({
      key: "win-rate",
      title: "胜率机器",
      metric: `${formatWinRate(review.summary.winRate, 0)} 胜率`,
      detail: `${review.summary.wins} 胜 / ${review.summary.losses} 负`,
      value: winRatePercent,
      thresholds: [50, 55, 60],
      unit: "%",
    }),
    buildAchievement({
      key: "s-grade",
      title: "高光制造者",
      metric: `S 级表现 ${review.phase5Highlights.sGradeCount} 场`,
      detail: `${review.summary.totalMatches} 场样本中的峰值表现`,
      value: review.phase5Highlights.sGradeCount,
      thresholds: [5, 15, 30],
      unit: "场 S",
    }),
    buildAchievement({
      key: "carry",
      title: "战局核心",
      metric: `带飞局 ${review.phase5Highlights.carryCount} 场`,
      detail: "高价值正向表现",
      value: review.phase5Highlights.carryCount,
      thresholds: [5, 12, 25],
      unit: "场",
    }),
    buildAchievement({
      key: "signature",
      title: "招牌打法",
      metric: review.phase5Highlights.mostCommonHighlight ?? "暂无高频高光",
      detail: "出现最多的个人高光标签",
      value: signatureCount,
      thresholds: [3, 8, 15],
      unit: "次",
    }),
    buildAchievement({
      key: "peak",
      title: "巅峰一战",
      metric: peakBest ? `${peakBest.label} ${peakBest.value}` : "暂无单场峰值",
      detail: peakBest ? `${peakBest.resultLabel} / ${peakBest.heroName} / ${peakBest.mapName}` : "需要更多对局样本",
      value: peakValue,
      thresholds: buildPeakThresholds(peakValue),
      unit: "",
      evidence: peakBest?.evidence,
    }),
    buildAchievement({
      key: "hero-focus",
      title: "本命专精",
      metric: `第一英雄占比 ${formatWinRate(review.heroPool.top1Share, 0)}`,
      detail: mainHero ? `${mainHero.hero} ${mainHero.matches} 场` : "暂无主力英雄",
      value: mainHeroSharePercent,
      thresholds: [25, 40, 55],
      unit: "%",
      evidence: mainHero?.evidence,
    }),
    buildAchievement({
      key: "switching",
      title: "战术变阵",
      metric: `使用 ${review.heroPool.totalHeroes} 名英雄`,
      detail: `切换率 ${formatWinRate(review.heroPool.switchMatchRate, 0)}`,
      value: review.heroPool.totalHeroes,
      thresholds: [5, 10, 15],
      unit: "名英雄",
    }),
  ];
}

export function activeTopAchievements(review: PersonalSeasonReview, limit = 3): ReviewAchievement[] {
  return buildReviewAchievements(review)
    .filter((achievement) => achievement.currentLevel > 0)
    .sort((left, right) => {
      const byLevel = right.currentLevel - left.currentLevel;
      if (byLevel !== 0) return byLevel;
      const byProgress = right.progress - left.progress;
      if (byProgress !== 0) return byProgress;
      return achievementOrder.indexOf(left.key) - achievementOrder.indexOf(right.key);
    })
    .slice(0, limit);
}

export function activeBadgeLevel(achievement: ReviewAchievement): 1 | 2 | 3 {
  if (achievement.currentLevel === 2) return 2;
  if (achievement.currentLevel === 3) return 3;
  return 1;
}

export function formatThreshold(value: number, unit: string) {
  if (unit === "%") return `${value}%`;
  if (!unit) return `${value}`;
  return `${value} ${unit}`;
}

function buildAchievement(input: Omit<ReviewAchievement, "currentLevel" | "progress">): ReviewAchievement {
  return {
    ...input,
    currentLevel: achievementLevel(input.value, input.thresholds),
    progress: achievementProgress(input.value, input.thresholds),
  };
}

function achievementLevel(value: number, thresholds: readonly [number, number, number]): 0 | 1 | 2 | 3 {
  if (value >= thresholds[2]) return 3;
  if (value >= thresholds[1]) return 2;
  if (value >= thresholds[0]) return 1;
  return 0;
}

function achievementProgress(value: number, thresholds: readonly [number, number, number]) {
  if (value >= thresholds[2]) return 100;
  const nextThreshold = thresholds.find((threshold) => value < threshold) ?? thresholds[2];
  return Math.max(0, Math.min(100, (value / nextThreshold) * 100));
}

function buildPeakThresholds(value: number): [number, number, number] {
  const base = Math.max(1, value);
  return [1, Math.max(2, Math.ceil(base * 0.8)), Math.max(3, Math.ceil(base * 1.05))];
}
