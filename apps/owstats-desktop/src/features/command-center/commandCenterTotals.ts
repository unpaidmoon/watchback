import type { SeasonDataStatus } from "../../shared/types";

export type CommandCenterTotals = {
  matchCount: number;
  detailCount: number;
  coveragePercent: number;
};

export function summarizeCommandCenterSeasons(seasonStatuses: SeasonDataStatus[]): CommandCenterTotals {
  const totals = seasonStatuses.reduce(
    (summary, seasonStatus) => ({
      matchCount: summary.matchCount + seasonStatus.matchCount,
      detailCount: summary.detailCount + seasonStatus.detailCount,
    }),
    { matchCount: 0, detailCount: 0 },
  );

  return {
    ...totals,
    coveragePercent: totals.matchCount > 0
      ? Math.min(100, Math.round((totals.detailCount / totals.matchCount) * 100))
      : 0,
  };
}
