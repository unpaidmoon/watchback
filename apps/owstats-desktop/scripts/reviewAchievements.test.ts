import assert from "node:assert/strict";
import { activeTopAchievements, buildReviewAchievements } from "../src/shared/reviewAchievements.ts";
import type { EvidenceLink, PersonalSeasonReview } from "../src/shared/types.ts";

const evidence: EvidenceLink = {
  kind: "match",
  label: "sample",
  route: "/matches/sample",
  matchId: "sample",
  hero: "Ana",
  map: "Ilios",
};

const review: PersonalSeasonReview = {
  availableSeasons: [23, 20, 15],
  summary: {
    season: 22,
    totalMatches: 60,
    baselineMatches: 60,
    wins: 36,
    losses: 24,
    winRate: 0.6,
    avgKills: 10,
    avgDeaths: 5,
    avgDamage: 8000,
    avgHealing: 9000,
  },
  baseline: {
    matchCount: 60,
    excludedTotal: 0,
    shortMatchThresholdSec: 180,
    exclusionReasons: [],
    avgKills: 10,
    avgDeaths: 5,
    avgDamage: 8000,
    avgHealing: 9000,
  },
  roleSummaries: [],
  phase5Highlights: {
    carryCount: 12,
    tryHardCount: 0,
    sGradeCount: 16,
    mostCommonHighlight: "高光制造者",
    tagCounts: [],
  },
  topConclusions: [],
  personalBests: [
    {
      metric: "damage",
      label: "伤害",
      value: 100,
      matchId: "peak",
      result: "win",
      resultLabel: "胜利",
      heroName: "Ana",
      mapName: "Ilios",
      evidence,
    },
  ],
  streaks: { longestWinStreak: 9, longestLossStreak: 1 },
  heroPool: {
    label: "stable",
    totalHeroes: 8,
    top1Share: 0.58,
    top3Share: 0.82,
    avgHeroesPerMatch: 1.2,
    switchMatchRate: 0.2,
    mainHeroes: [
      { hero: "Ana", matches: 30, winRate: 0.66, evidence },
      { hero: "Kiriko", matches: 18, winRate: 0.61, evidence },
      { hero: "Mercy", matches: 10, winRate: 0.58, evidence },
      { hero: "Moira", matches: 7, winRate: 0.55, evidence },
      { hero: "Lucio", matches: 4, winRate: 0.5, evidence },
    ],
    clues: [],
  },
  maps: {
    minSample: 3,
    bestMap: null,
    maps: [],
    conclusions: [],
  },
};

const achievements = buildReviewAchievements(review);
assert.equal(achievements.length, 8, "shared builder keeps the review achievement set");
assert.equal(achievements.find((item) => item.key === "win-streak")?.currentLevel, 3, "win streak reaches level III");
assert.equal(achievements.find((item) => item.key === "hero-focus")?.currentLevel, 3, "hero focus reaches level III");

const top = activeTopAchievements(review, 3);
assert.deepEqual(
  top.map((item) => item.key),
  ["win-streak", "win-rate", "signature"],
  "top active achievements sort by level, then progress, then stable achievement order",
);
assert.equal(top.every((item) => item.currentLevel > 0), true, "top achievements only include active achievements");
