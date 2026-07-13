import type { FilterOptions, MatchFilters } from "../../shared/types";

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
  if (next.primaryTag && !["带飞局", "尽力局", "躺赢局", "背锅局"].includes(next.primaryTag)) delete next.primaryTag;
  return next;
}

function validDate(value: string) {
  return /^\d{4}-\d{2}-\d{2}$/.test(value);
}
