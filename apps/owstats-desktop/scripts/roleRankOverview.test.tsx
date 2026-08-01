import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { RoleRankOverview, type RoleRankOverviewEntry } from "../src/features/review/RoleRankOverview";

const competitive: RoleRankOverviewEntry[] = [{
  roleType: "tank",
  roleLabel: "重装",
  matches: 12,
  winRate: 0.625,
  kda: 3.456,
  rankSubTier: 2,
  rankKey: "diamond",
}];
const stadium: RoleRankOverviewEntry[] = [{
  roleType: "damage",
  roleLabel: "输出",
  matches: null,
  winRate: null,
  kda: null,
  rankSubTier: null,
  rankKey: "diamond",
}];

const markup = renderToStaticMarkup(createElement(RoleRankOverview, { competitive, stadium }));
assert.ok(markup.includes(">预设竞技与开放竞技<"));
assert.ok(markup.includes(">角斗竞技<"));
assert.ok(markup.includes(">63%<"));
assert.ok(markup.includes(">12<"));
assert.ok(markup.includes(">3.46<"));
assert.ok(markup.includes("reviewStadiumRankGrid"));
assert.ok(markup.includes("assets/stadium/ranks/fight/5-diamond"));
assert.equal((markup.match(/<strong>-<\/strong>/g) ?? []).length, 3, "missing win rate, matches and KDA stay visible as dashes");

const loading = renderToStaticMarkup(createElement(RoleRankOverview, { competitive: [], stadium: [], loading: true }));
assert.ok(loading.includes('role="status"'));
assert.ok(loading.includes("正在加载段位总览"));

const error = renderToStaticMarkup(createElement(RoleRankOverview, { competitive: [], stadium: [], error: "段位请求失败" }));
assert.ok(error.includes('role="alert"'));
assert.ok(error.includes("段位请求失败"));

const empty = renderToStaticMarkup(createElement(RoleRankOverview, { competitive: [], stadium: [] }));
assert.ok(empty.includes("暂无段位数据"));

const placeholder = renderToStaticMarkup(createElement(RoleRankOverview, {
  competitive: [], stadium: [], showMissingRoles: true,
}));
assert.ok(placeholder.includes(">重装<"));
assert.ok(placeholder.includes(">输出<"));
assert.ok(placeholder.includes(">支援<"));
assert.equal((placeholder.match(/phase6RoleRankCard/g) ?? []).length, 6, "query placeholder keeps both three-role grids stable");

const threeGroups = renderToStaticMarkup(createElement(RoleRankOverview, {
  competitive, openCompetitive: [], stadium, showMissingRoles: true,
}));
assert.ok(threeGroups.includes(">预设竞技与开放竞技<"));
assert.ok(threeGroups.includes(">角斗竞技<"));
assert.doesNotMatch(threeGroups, />开放竞技</);
assert.ok(threeGroups.includes("reviewCompetitiveRankGrid"));
assert.ok(threeGroups.includes(">开放<"));
assert.equal((threeGroups.match(/reviewRoleRankGroup/g) ?? []).length, 2, "open rank remains in the first report-style group");
assert.equal((threeGroups.match(/phase6RoleRankCard/g) ?? []).length, 7, "query renders competitive row 4 + stadium row 3 cards");

const percentScale = renderToStaticMarkup(createElement(RoleRankOverview, {
  competitive: [{ ...competitive[0], winRate: 62.5 }], stadium: [],
}));
assert.ok(percentScale.includes(">63%<"), "percentage-scale win rates are not multiplied twice");

console.log("role rank overview tests passed");
