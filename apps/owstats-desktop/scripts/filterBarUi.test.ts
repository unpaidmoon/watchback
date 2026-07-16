import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  activeMatchFilterChips,
  clearVisibleMatchFilters,
  countAdvancedMatchFilters,
  filtersFromParams,
  paramsFromFilters,
  removeMatchFilter,
} from "../src/features/matches/matchFilters.ts";

const source = readFileSync(new URL("../src/features/matches/FilterBar.tsx", import.meta.url), "utf8");
const toolbarSource = readFileSync(new URL("../src/features/matches/MatchFilterToolbar.tsx", import.meta.url), "utf8");
const advancedSource = readFileSync(new URL("../src/features/matches/AdvancedMatchFilters.tsx", import.meta.url), "utf8");
const chipsSource = readFileSync(new URL("../src/features/matches/ActiveFilterChips.tsx", import.meta.url), "utf8");

assert.equal(source.includes("filterActions"), false, "FilterBar should not render date preset action buttons");
assert.equal(source.includes("shortcut("), false, "FilterBar should not keep date shortcut handlers");
assert.match(source, /<MatchFilterToolbar/);
assert.match(source, /<AdvancedMatchFilters/);
assert.match(source, /<ActiveFilterChips/);
for (const label of ["模式", "英雄", "玩家名称", "更多筛选", "清除"]) assert.match(toolbarSource, new RegExp(label));
for (const label of ["胜负", "地图", "复盘标签", "开始日期", "结束日期"]) assert.match(advancedSource, new RegExp(label));
for (const label of ["收藏状态", "仅看收藏"]) assert.match(advancedSource, new RegExp(label));
assert.doesNotMatch(toolbarSource, />\s*(胜负|地图|复盘标签|开始日期|结束日期)\s*</);
assert.doesNotMatch(advancedSource, />\s*英雄\s*</);
assert.match(chipsSource, /aria-label="有效筛选"/);
assert.match(chipsSource, /aria-label=\{`移除\$\{chip\.label\}`\}/);

assert.equal(countAdvancedMatchFilters({ hero: "ana" }), 0);
assert.equal(countAdvancedMatchFilters({ result: "win" }), 1);
assert.equal(countAdvancedMatchFilters({ favoriteOnly: true }), 1);
assert.equal(countAdvancedMatchFilters({ hero: "ana", map: "ilios", result: "win" }), 2);
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
