import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ActiveFilterChips } from "../src/features/matches/ActiveFilterChips.tsx";
import { AdvancedMatchFilters } from "../src/features/matches/AdvancedMatchFilters.tsx";
import { FilterBar } from "../src/features/matches/FilterBar.tsx";
import { MatchFilterToolbar } from "../src/features/matches/MatchFilterToolbar.tsx";
import {
  activeMatchFilterChips,
  clearVisibleMatchFilters,
  countAdvancedMatchFilters,
  filtersFromParams,
  paramsFromFilters,
  removeMatchFilter,
} from "../src/features/matches/matchFilters.ts";
import type { FilterOptions } from "../src/shared/types.ts";

const options: FilterOptions = {
  seasons: [23],
  heroes: [{ id: "ana", label: "安娜" }],
  maps: [{ id: "ilios", label: "伊利奥斯" }],
  results: ["win"],
  modes: ["QuickPlay"],
};
const noop = () => undefined;
const filterBarMarkup = renderToStaticMarkup(createElement(FilterBar, {
  filters: {},
  options,
  onChange: noop,
}));
assert.ok(filterBarMarkup.includes('aria-label="对局筛选"'));
assert.ok(!filterBarMarkup.includes("开始日期"), "advanced filters stay collapsed initially");

const toolbarMarkup = renderToStaticMarkup(createElement(MatchFilterToolbar, {
  filters: {},
  options,
  playerQuery: "",
  playerOptions: [],
  playerSearchOpen: false,
  playerSearchLoading: false,
  advancedCount: 0,
  advancedOpen: false,
  hasVisibleFilters: false,
  onPatch: noop,
  onPlayerQueryChange: noop,
  onPlayerFocus: noop,
  onPlayerBlur: noop,
  onPlayerKeyDown: noop,
  onPlayerSelect: noop,
  onPlayerClear: noop,
  onToggleAdvanced: noop,
  onClear: noop,
}));
for (const label of ["模式", "英雄", "地图", "玩家名称", "更多筛选", "清除"]) {
  assert.ok(toolbarMarkup.includes(label), `toolbar renders ${label}`);
}
assert.ok(toolbarMarkup.indexOf("英雄") < toolbarMarkup.indexOf("地图"));
assert.ok(toolbarMarkup.indexOf("地图") < toolbarMarkup.indexOf("玩家名称"));
assert.ok(!toolbarMarkup.includes("胜负"));
assert.ok(toolbarMarkup.includes('aria-expanded="false"'));
assert.ok(toolbarMarkup.includes('disabled=""'), "clear is disabled without visible filters");

const advancedMarkup = renderToStaticMarkup(createElement(AdvancedMatchFilters, {
  filters: { favoriteOnly: true },
  options,
  dateFrom: "2026-07-01",
  dateTo: "2026-07-16",
  onPatch: noop,
  onDateFromChange: noop,
  onDateToChange: noop,
  onCommitDates: noop,
}));
for (const label of ["胜负", "复盘标签", "收藏状态", "仅看收藏", "开始日期", "结束日期"]) {
  assert.ok(advancedMarkup.includes(label), `advanced filters render ${label}`);
}
assert.ok(!advancedMarkup.includes("英雄"));
assert.ok(!advancedMarkup.includes("地图"));

const chipsMarkup = renderToStaticMarkup(createElement(ActiveFilterChips, {
  chips: [{ key: "hero", label: "英雄：安娜" }],
  onRemove: noop,
}));
assert.ok(chipsMarkup.includes('aria-label="有效筛选"'));
assert.ok(chipsMarkup.includes('aria-label="移除英雄：安娜"'));

assert.equal(countAdvancedMatchFilters({ hero: "ana" }), 0);
assert.equal(countAdvancedMatchFilters({ result: "win" }), 1);
assert.equal(countAdvancedMatchFilters({ favoriteOnly: true }), 1);
assert.equal(countAdvancedMatchFilters({ hero: "ana", map: "ilios", result: "win" }), 1);
assert.deepEqual(clearVisibleMatchFilters({ season: 23, hero: "ana", result: "win" }), { season: 23 });
assert.deepEqual(clearVisibleMatchFilters({ season: 23, favoriteOnly: true }), { season: 23 });
assert.deepEqual(removeMatchFilter({ season: 23, hero: "ana", result: "win" }, "hero"), { season: 23, result: "win" });
assert.deepEqual(
  removeMatchFilter({ season: 23, hero: "ana", favoriteOnly: true }, "favoriteOnly"),
  { season: 23, hero: "ana" },
);
assert.deepEqual(
  activeMatchFilterChips(
    { favoriteOnly: true },
    { seasons: [], heroes: [], maps: [], results: [], modes: [] },
  ).map((chip) => chip.label),
  ["收藏：仅看收藏"],
);
assert.deepEqual(filtersFromParams(new URLSearchParams("season=23&favorite=1")), { season: 23, favoriteOnly: true });
assert.equal(paramsFromFilters({ season: 23, favoriteOnly: true }, "match-1").toString(), "matchId=match-1&season=23&favorite=1");
assert.deepEqual(filtersFromParams(paramsFromFilters({ season: 23, favoriteOnly: true })), { season: 23, favoriteOnly: true });
assert.equal(filtersFromParams(new URLSearchParams("favorite=true")).favoriteOnly, undefined);
const allFilters = {
  season: 23,
  hero: "ana",
  map: "ilios",
  result: "win",
  mode: "QuickPlay",
  primaryTag: "尽力局",
  playerName: "测试玩家",
  dateFrom: "2026-07-01",
  dateTo: "2026-07-16",
  favoriteOnly: true,
};
const allFilterParams = paramsFromFilters(allFilters, "selected-match");
assert.equal(allFilterParams.get("matchId"), "selected-match");
assert.deepEqual(filtersFromParams(allFilterParams), allFilters);
assert.deepEqual(
  activeMatchFilterChips(
    { hero: "ana", dateFrom: "2026-07-01" },
    { seasons: [], heroes: [{ id: "ana", label: "安娜" }], maps: [], results: [], modes: [] },
  ).map((chip) => chip.label),
  ["英雄：安娜", "开始日期：2026-07-01"],
);
assert.deepEqual(
  activeMatchFilterChips(
    { mode: "raw-mode-guid", hero: "raw-hero-guid", map: "raw-map-guid", result: "raw-result" },
    { seasons: [], heroes: [], maps: [], results: [], modes: [] },
  ).map((chip) => chip.label),
  ["模式：未知模式", "胜负：未知结果", "英雄：未知英雄", "地图：未知地图"],
  "filter chips never expose unmapped raw identifiers",
);
