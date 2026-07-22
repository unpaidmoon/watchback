import type { FilterOptions, MatchFilters } from "../../shared/types";
import { filterResultLabel, matchPrimaryTags } from "./matchFilters";

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
        胜负
        <select value={props.filters.result ?? ""} onChange={(event) => props.onPatch({ result: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {props.options.results.map((result) => (
            <option key={result} value={result}>{filterResultLabel(result)}</option>
          ))}
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
        收藏状态
        <select
          value={props.filters.favoriteOnly ? "favorite" : ""}
          onChange={(event) => props.onPatch({ favoriteOnly: event.target.value === "favorite" ? true : undefined })}
        >
          <option value="">全部</option>
          <option value="favorite">仅看收藏</option>
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
