import { useEffect, useState } from "react";
import { modeLabel } from "../../shared/rankingUtils";
import type { FilterOptions, MatchFilters } from "../../shared/types";

type Props = {
  filters: MatchFilters;
  options: FilterOptions;
  onChange: (filters: MatchFilters) => void;
};

const resultLabels: Record<string, string> = {
  win: "胜利",
  loss: "失败",
  draw: "平局"
};

const primaryTags = ["带飞局", "尽力局", "躺赢局", "背锅局"];

export function FilterBar({ filters, options, onChange }: Props) {
  const [dateFrom, setDateFrom] = useState(filters.dateFrom ?? "");
  const [dateTo, setDateTo] = useState(filters.dateTo ?? "");
  const patch = (next: Partial<MatchFilters>) => onChange(cleanFilters({ ...filters, ...next }));

  useEffect(() => {
    setDateFrom(filters.dateFrom ?? "");
    setDateTo(filters.dateTo ?? "");
  }, [filters.dateFrom, filters.dateTo]);

  function commitDates(nextFrom = dateFrom, nextTo = dateTo) {
    patch({ dateFrom: emptyOrValue(nextFrom), dateTo: emptyOrValue(nextTo) });
  }

  return (
    <section className="filterBar phase6FilterBar" aria-label="对局筛选">
      <label>
        赛季
        <select value={filters.season ?? ""} onChange={(event) => patch({ season: numberOrUndefined(event.target.value) })}>
          <option value="">全部</option>
          {options.seasons.map((season) => (
            <option key={season} value={season}>
              S{season}
            </option>
          ))}
        </select>
      </label>
      <label>
        英雄
        <select value={filters.hero ?? ""} onChange={(event) => patch({ hero: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {options.heroes.map((hero) => (
            <option key={hero.id} value={hero.id}>
              {hero.label}
            </option>
          ))}
        </select>
      </label>
      <label>
        地图
        <select value={filters.map ?? ""} onChange={(event) => patch({ map: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {options.maps.map((map) => (
            <option key={map.id} value={map.id}>
              {map.label}
            </option>
          ))}
        </select>
      </label>
      <label>
        胜负
        <select value={filters.result ?? ""} onChange={(event) => patch({ result: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {options.results.map((result) => (
            <option key={result} value={result}>
              {resultLabels[result] ?? result}
            </option>
          ))}
        </select>
      </label>
      <label>
        模式
        <select value={filters.mode ?? ""} onChange={(event) => patch({ mode: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {options.modes.map((mode) => (
            <option key={mode} value={mode}>
              {modeLabel(mode)}
            </option>
          ))}
        </select>
      </label>
      <label>
        复盘标签
        <select value={filters.primaryTag ?? ""} onChange={(event) => patch({ primaryTag: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {primaryTags.map((tag) => (
            <option key={tag} value={tag}>
              {tag}
            </option>
          ))}
        </select>
      </label>
      <label>
        开始日期
        <input
          type="date"
          value={dateFrom}
          onChange={(event) => setDateFrom(event.target.value)}
          onBlur={() => commitDates()}
          onKeyDown={(event) => {
            if (event.key === "Enter") commitDates();
          }}
        />
      </label>
      <label>
        结束日期
        <input
          type="date"
          value={dateTo}
          onChange={(event) => setDateTo(event.target.value)}
          onBlur={() => commitDates()}
          onKeyDown={(event) => {
            if (event.key === "Enter") commitDates();
          }}
        />
      </label>
    </section>
  );
}

function cleanFilters(filters: MatchFilters): MatchFilters {
  return Object.fromEntries(Object.entries(filters).filter(([, value]) => value !== undefined && value !== "")) as MatchFilters;
}

function emptyOrValue(value: string) {
  return value.trim() ? value : undefined;
}

function numberOrUndefined(value: string) {
  return value ? Number(value) : undefined;
}
