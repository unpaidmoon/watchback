import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { FilterBar } from "../features/matches/FilterBar";
import { MatchTimeline } from "../features/matches/MatchTimeline";
import { getOverview, listMatchFilters, listMatchTimeline } from "../shared/api";
import type { FilterOptions, ImportSummary, MatchFilters, MatchSummary, Overview } from "../shared/types";

const emptyOptions: FilterOptions = {
  seasons: [],
  heroes: [],
  maps: [],
  results: [],
  modes: []
};

export function MatchesPage() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const [overview, setOverview] = useState<Overview | null>(null);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [options, setOptions] = useState<FilterOptions>(emptyOptions);
  const [filters, setFilters] = useState<MatchFilters>({});
  const [matches, setMatches] = useState<MatchSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const winRate = useMemo(() => {
    if (matches.length === 0) return "0%";
    const wins = matches.filter((match) => match.result === "win").length;
    return `${Math.round((wins / matches.length) * 100)}%`;
  }, [matches]);

  useEffect(() => {
    void refreshAll();
  }, []);

  useEffect(() => {
    setFilters(filtersFromParams(searchParams));
  }, []);

  useEffect(() => {
    void loadMatches(filters);
    setSearchParams(paramsFromFilters(filters), { replace: true });
  }, [filters]);

  useEffect(() => {
    if (!hasLoadedOptions(options)) return;
    const cleaned = validateFilters(filters, options);
    if (JSON.stringify(cleaned) !== JSON.stringify(filters)) {
      setFilters(cleaned);
    }
  }, [options]);

  async function refreshAll() {
    setBusy(true);
    setError(null);
    try {
      const nextOverview = await getOverview();
      setOverview(nextOverview);
      setSummary(nextOverview.importSummary);
      setOptions(await listMatchFilters());
      await loadMatches(filters);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function loadMatches(nextFilters: MatchFilters) {
    try {
      setMatches(await listMatchTimeline(nextFilters));
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <>
      <section className="controlBand">
        <div className="metric">
          <span>本地数据库</span>
          <strong>{overview?.databasePath ?? "未初始化"}</strong>
        </div>
        <div className="metric">
          <span>预设/开放</span>
          <strong>{summary?.presetMatches ?? matches.length}</strong>
        </div>
        <div className="metric">
          <span>当前列表</span>
          <strong>{matches.length}</strong>
        </div>
        <div className="metric">
          <span>当前胜率</span>
          <strong>{winRate}</strong>
        </div>
      </section>

      {error && <div className="errorBanner">{error}</div>}

      <FilterBar filters={filters} options={options} onChange={setFilters} />

      <section className="workspace listWorkspace">
        <MatchTimeline matches={matches} onSelect={(matchId) => navigate(`/matches/${matchId}`)} />
      </section>
    </>
  );
}

function filtersFromParams(params: URLSearchParams): MatchFilters {
  const filters: MatchFilters = {};
  const season = params.get("season");
  if (season && Number.isFinite(Number(season))) filters.season = Number(season);
  for (const key of ["hero", "map", "result", "mode", "startDate", "endDate"] as const) {
    const value = params.get(key);
    if (!value) continue;
    if (key === "startDate") filters.dateFrom = validDate(value) ? value : undefined;
    else if (key === "endDate") filters.dateTo = validDate(value) ? value : undefined;
    else filters[key] = value;
  }
  return filters;
}

function paramsFromFilters(filters: MatchFilters) {
  const params = new URLSearchParams();
  if (filters.season) params.set("season", String(filters.season));
  if (filters.hero) params.set("hero", filters.hero);
  if (filters.map) params.set("map", filters.map);
  if (filters.result) params.set("result", filters.result);
  if (filters.mode) params.set("mode", filters.mode);
  if (filters.dateFrom) params.set("startDate", filters.dateFrom);
  if (filters.dateTo) params.set("endDate", filters.dateTo);
  return params;
}

function validDate(value: string) {
  return /^\d{4}-\d{2}-\d{2}$/.test(value);
}

function hasLoadedOptions(options: FilterOptions) {
  return (
    options.seasons.length > 0 ||
    options.heroes.length > 0 ||
    options.maps.length > 0 ||
    options.results.length > 0 ||
    options.modes.length > 0
  );
}

function validateFilters(filters: MatchFilters, options: FilterOptions): MatchFilters {
  const next = { ...filters };
  if (next.season !== undefined && !options.seasons.includes(next.season)) delete next.season;
  if (next.hero && !options.heroes.some((hero) => hero.id === next.hero)) delete next.hero;
  if (next.map && !options.maps.some((map) => map.id === next.map)) delete next.map;
  if (next.result && !options.results.includes(next.result)) delete next.result;
  if (next.mode && !options.modes.includes(next.mode)) delete next.mode;
  return next;
}
