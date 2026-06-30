import type { MatchEvaluationSummary, MatchPerformanceAward } from "./types";

export type PerformanceBadgeTone = "gold" | "blue" | "muted";

const awardByEvidenceTag: Record<string, MatchPerformanceAward> = {
  击杀最多: { code: "kill", label: "击杀最多", tone: "gold" },
  伤害最高: { code: "damage", label: "伤害最高", tone: "gold" },
  治疗最高: { code: "healing", label: "治疗最高", tone: "gold" },
  格挡最多: { code: "mitigation", label: "格挡最多", tone: "gold" },
  死亡最多: { code: "death", label: "死亡最多", tone: "danger" },
};

const medalText: Record<MatchPerformanceAward["code"], string> = {
  kill: "杀",
  damage: "伤",
  healing: "疗",
  mitigation: "挡",
  death: "死",
};

export function awardsFromEvidenceTags(tags: string[]): MatchPerformanceAward[] {
  return tags.flatMap((tag) => (awardByEvidenceTag[tag] ? [awardByEvidenceTag[tag]] : []));
}

export function normalizeMatchAwards(evaluation: MatchEvaluationSummary | null, teamAwards: MatchPerformanceAward[] = []) {
  if (teamAwards.length > 0) return teamAwards;
  return awardsFromEvidenceTags(evaluation?.evidenceTags ?? []);
}

export function awardMedalText(award: MatchPerformanceAward) {
  return medalText[award.code];
}

export function gradeBadgeTone(grade: string): PerformanceBadgeTone {
  if (grade === "S") return "gold";
  if (grade === "A") return "blue";
  return "muted";
}

export function primaryTagBadgeTone(primaryTag: string): PerformanceBadgeTone {
  if (primaryTag === "带飞局" || primaryTag === "尽力局") return "gold";
  if (primaryTag === "躺赢局" || primaryTag === "背锅局") return "blue";
  return "muted";
}
