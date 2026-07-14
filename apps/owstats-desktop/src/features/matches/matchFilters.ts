import type { FilterOptions, MatchFilters } from "../../shared/types";
import { modeLabel } from "../../shared/rankingUtils";

export const advancedMatchFilterKeys = ["hero", "map", "primaryTag", "dateFrom", "dateTo"] as const;
export type VisibleMatchFilterKey = Exclude<keyof MatchFilters, "season">;

export type ActiveMatchFilterChip = {
  key: VisibleMatchFilterKey;
  label: string;
};

const resultLabels: Record<string, string> = {
  win: "胜利",
  loss: "失败",
  draw: "平局",
};

const primaryTags = ["带飞局", "尽力局", "躺赢局", "背锅局"];

export function filtersFromParams(params: URLSearchParams): MatchFilters {
  const filters: MatchFilters = {};
  const season = params.get("season");
  if (season && Number.isFinite(Number(season))) filters.season = Number(season);
  for (const key of ["hero", "map", "result", "mode", "primaryTag", "playerName", "startDate", "endDate"] as const) {
    const value = params.get(key);
    if (!value) continue;
    if (key === "startDate") filters.dateFrom = validDate(value) ? value : undefined;
    else if (key === "endDate") filters.dateTo = validDate(value) ? value : undefined;
    else filters[key] = value;
  }
  return filters;
}

export function paramsFromFilters(filters: MatchFilters, selectedMatchId?: string | null) {
  const params = new URLSearchParams();
  if (selectedMatchId) params.set("matchId", selectedMatchId);
  if (filters.season) params.set("season", String(filters.season));
  if (filters.hero) params.set("hero", filters.hero);
  if (filters.map) params.set("map", filters.map);
  if (filters.result) params.set("result", filters.result);
  if (filters.mode) params.set("mode", filters.mode);
  if (filters.primaryTag) params.set("primaryTag", filters.primaryTag);
  if (filters.playerName) params.set("playerName", filters.playerName);
  if (filters.dateFrom) params.set("startDate", filters.dateFrom);
  if (filters.dateTo) params.set("endDate", filters.dateTo);
  return params;
}

export function hasLoadedOptions(options: FilterOptions) {
  return options.seasons.length > 0 || options.heroes.length > 0 || options.maps.length > 0 || options.results.length > 0 || options.modes.length > 0;
}

export function validateFilters(filters: MatchFilters, options: FilterOptions): MatchFilters {
  const next = { ...filters };
  if (next.season !== undefined && !options.seasons.includes(next.season)) delete next.season;
  if (next.hero && !options.heroes.some((hero) => hero.id === next.hero)) delete next.hero;
  if (next.map && !options.maps.some((map) => map.id === next.map)) delete next.map;
  if (next.result && !options.results.includes(next.result)) delete next.result;
  if (next.mode && !options.modes.includes(next.mode)) delete next.mode;
  if (next.primaryTag && !primaryTags.includes(next.primaryTag)) delete next.primaryTag;
  return next;
}

export function countAdvancedMatchFilters(filters: MatchFilters) {
  return advancedMatchFilterKeys.filter((key) => Boolean(filters[key])).length;
}

export function clearVisibleMatchFilters(filters: MatchFilters): MatchFilters {
  return filters.season === undefined ? {} : { season: filters.season };
}

export function removeMatchFilter(filters: MatchFilters, key: VisibleMatchFilterKey): MatchFilters {
  const next = { ...filters };
  delete next[key];
  return next;
}

export function activeMatchFilterChips(filters: MatchFilters, options: FilterOptions): ActiveMatchFilterChip[] {
  const chips: ActiveMatchFilterChip[] = [];
  if (filters.mode) chips.push({ key: "mode", label: `模式：${safeModeLabel(filters.mode)}` });
  if (filters.result) chips.push({ key: "result", label: `胜负：${resultLabels[filters.result] ?? "未知结果"}` });
  if (filters.playerName) chips.push({ key: "playerName", label: `玩家：${filters.playerName}` });
  if (filters.hero) chips.push({ key: "hero", label: `英雄：${optionLabel(options.heroes, filters.hero, "未知英雄")}` });
  if (filters.map) chips.push({ key: "map", label: `地图：${optionLabel(options.maps, filters.map, "未知地图")}` });
  if (filters.primaryTag) chips.push({ key: "primaryTag", label: `复盘标签：${primaryTags.includes(filters.primaryTag) ? filters.primaryTag : "未知标签"}` });
  if (filters.dateFrom) chips.push({ key: "dateFrom", label: `开始日期：${filters.dateFrom}` });
  if (filters.dateTo) chips.push({ key: "dateTo", label: `结束日期：${filters.dateTo}` });
  return chips;
}

export function filterResultLabel(result: string) {
  return resultLabels[result] ?? "未知结果";
}

export function filterModeLabel(mode: string) {
  return safeModeLabel(mode);
}

export function matchPrimaryTags() {
  return [...primaryTags];
}

function safeModeLabel(mode: string) {
  const label = modeLabel(mode);
  return label === mode ? "未知模式" : label;
}

function optionLabel(options: Array<{ id: string; label: string }>, id: string, fallback: string) {
  return options.find((option) => option.id === id)?.label || fallback;
}

function validDate(value: string) {
  return /^\d{4}-\d{2}-\d{2}$/.test(value);
}
