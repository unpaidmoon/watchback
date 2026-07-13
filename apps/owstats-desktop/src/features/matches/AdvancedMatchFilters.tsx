import type { FilterOptions, MatchFilters } from "../../shared/types";
import { matchPrimaryTags } from "./matchFilters";

export type AdvancedMatchFiltersProps = {
  filters: MatchFilters;
  options: FilterOptions;
  dateFrom: string;
  dateTo: string;
  onPatch: (patch: Partial<MatchFilters>) => void;
  onDateFromChange: (value: string) => void;
  onDateToChange: (value: string) => void;
  onCommitDates: () => void;
};

export function AdvancedMatchFilters(props: AdvancedMatchFiltersProps) {
  return (
    <div id="advanced-match-filters" className="advancedMatchFilters phase6FilterRow phase6FilterRowSecondary">
      <label>
        英雄
        <select value={props.filters.hero ?? ""} onChange={(event) => props.onPatch({ hero: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {props.options.heroes.map((hero) => <option key={hero.id} value={hero.id}>{hero.label}</option>)}
        </select>
      </label>
      <label>
        地图
        <select value={props.filters.map ?? ""} onChange={(event) => props.onPatch({ map: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {props.options.maps.map((map) => <option key={map.id} value={map.id}>{map.label}</option>)}
        </select>
      </label>
      <label>
        复盘标签
        <select value={props.filters.primaryTag ?? ""} onChange={(event) => props.onPatch({ primaryTag: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {matchPrimaryTags().map((tag) => <option key={tag} value={tag}>{tag}</option>)}
        </select>
      </label>
      <label>
        开始日期
        <input
          type="date"
          value={props.dateFrom}
          onChange={(event) => props.onDateFromChange(event.target.value)}
          onBlur={props.onCommitDates}
          onKeyDown={(event) => { if (event.key === "Enter") props.onCommitDates(); }}
        />
      </label>
      <label>
        结束日期
        <input
          type="date"
          value={props.dateTo}
          onChange={(event) => props.onDateToChange(event.target.value)}
          onBlur={props.onCommitDates}
          onKeyDown={(event) => { if (event.key === "Enter") props.onCommitDates(); }}
        />
      </label>
    </div>
  );
}

function emptyOrValue(value: string) {
  return value.trim() ? value : undefined;
}
