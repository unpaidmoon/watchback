import { Search, X } from "lucide-react";
import type { FilterOptions, MatchFilters, PlayerSearchOption } from "../../shared/types";
import { filterModeLabel } from "./matchFilters";

export type MatchFilterToolbarProps = {
  filters: MatchFilters;
  options: FilterOptions;
  playerQuery: string;
  playerOptions: PlayerSearchOption[];
  playerSearchOpen: boolean;
  playerSearchLoading: boolean;
  advancedCount: number;
  advancedOpen: boolean;
  hasVisibleFilters: boolean;
  onPatch: (patch: Partial<MatchFilters>) => void;
  onPlayerQueryChange: (value: string) => void;
  onPlayerFocus: () => void;
  onPlayerBlur: () => void;
  onPlayerKeyDown: (key: string) => void;
  onPlayerSelect: (playerName: string) => void;
  onPlayerClear: () => void;
  onToggleAdvanced: () => void;
  onClear: () => void;
};

export function MatchFilterToolbar(props: MatchFilterToolbarProps) {
  return (
    <div className="matchFilterToolbar phase6FilterRow phase6FilterRowPrimary">
      <label>
        模式
        <select value={props.filters.mode ?? ""} onChange={(event) => props.onPatch({ mode: emptyOrValue(event.target.value) })}>
          <option value="">全部</option>
          {props.options.modes.map((mode) => (
            <option key={mode} value={mode}>{filterModeLabel(mode)}</option>
          ))}
        </select>
      </label>
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
      <label className="phase6PlayerSearchLabel">
        玩家名称
        <span className="phase6PlayerSearchBox">
          <Search size={15} aria-hidden="true" />
          <input
            type="search"
            value={props.playerQuery}
            placeholder="输入玩家展示名"
            onChange={(event) => props.onPlayerQueryChange(event.target.value)}
            onFocus={props.onPlayerFocus}
            onBlur={props.onPlayerBlur}
            onKeyDown={(event) => props.onPlayerKeyDown(event.key)}
          />
          {props.filters.playerName ? (
            <button type="button" className="phase6PlayerSearchClear" onClick={props.onPlayerClear} aria-label="清除玩家名称筛选">
              <X size={14} aria-hidden="true" />
            </button>
          ) : null}
        </span>
        {props.playerSearchOpen && props.playerQuery.trim().length >= 2 && !props.filters.playerName ? (
          <span className="phase6PlayerSearchMenu">
            {props.playerSearchLoading ? <span className="phase6PlayerSearchState">搜索中...</span> : null}
            {!props.playerSearchLoading
              ? props.playerOptions.map((item) => (
                  <button type="button" key={item.playerName} onMouseDown={(event) => event.preventDefault()} onClick={() => props.onPlayerSelect(item.playerName)}>
                    <strong>{item.playerName}</strong>
                    <small>{item.matches} 场{item.latestBeginTs ? ` / ${formatLatestBegin(item.latestBeginTs)}` : ""}</small>
                  </button>
                ))
              : null}
            {!props.playerSearchLoading && props.playerOptions.length === 0 ? <span className="phase6PlayerSearchState">未找到玩家展示名</span> : null}
          </span>
        ) : null}
      </label>
      <button
        type="button"
        className="secondaryButton matchAdvancedToggle"
        aria-expanded={props.advancedOpen}
        aria-controls="advanced-match-filters"
        onClick={props.onToggleAdvanced}
      >
        更多筛选 {props.advancedCount}
      </button>
      <button type="button" className="ghostButton matchClearFilters" disabled={!props.hasVisibleFilters} onClick={props.onClear}>
        清除
      </button>
    </div>
  );
}

function emptyOrValue(value: string) {
  return value.trim() ? value : undefined;
}

function formatLatestBegin(value: number) {
  const seconds = value > 10_000_000_000 ? Math.floor(value / 1000) : value;
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) return "";
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${month}-${day}`;
}
