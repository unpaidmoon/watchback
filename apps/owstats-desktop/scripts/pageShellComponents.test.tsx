import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import {
  PageEmptyState,
  PageErrorState,
  PageHeader,
  PageLoadingState,
  PageSeasonSelect,
  StatusPill,
} from "../src/shared/page-shell/index";

const header = renderToStaticMarkup(
  createElement(PageHeader, {
    title: "对局",
    description: "浏览本地对局",
    actions: createElement("button", null, "刷新"),
  }),
);
assert.match(header, /<h1>对局<\/h1>/);
assert.match(header, /浏览本地对局/);
assert.match(header, /刷新/);

const season = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: undefined,
    seasons: [21, 22, 23],
    includeAll: true,
    allLabel: "全部赛季",
    onChange: () => undefined,
  }),
);
assert.match(season, />赛季<\/label>/);
assert.match(season, /<option value="" selected="">全部赛季<\/option>/);
assert.match(season, /<option value="22">S22<\/option>/);
const seasonLabelTarget = season.match(/<label for="([^"]+)">赛季<\/label>/)?.[1];
const seasonSelectId = season.match(/<select id="([^"]+)"/)?.[1];
assert.ok(seasonLabelTarget);
assert.equal(seasonLabelTarget, seasonSelectId);

const seasonWithoutAll = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: undefined,
    seasons: [21, 22, 23],
    onChange: () => undefined,
  }),
);
assert.match(seasonWithoutAll, /<option value="" disabled="" selected="">请选择赛季<\/option>/);
assert.doesNotMatch(seasonWithoutAll, /<option value="21" selected="">S21<\/option>/);

const unavailableSeason = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: 24,
    seasons: [21, 22, 23],
    includeAll: true,
    onChange: () => undefined,
  }),
);
assert.match(unavailableSeason, /<option value="24" disabled="" selected="">S24 不可用<\/option>/);
assert.doesNotMatch(unavailableSeason, /<option value="" selected="">全部赛季<\/option>/);

const selectedSeason = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: 22,
    seasons: [21, 22, 23],
    onChange: () => undefined,
  }),
);
assert.match(selectedSeason, /<option value="22" selected="">S22<\/option>/);

const emptySeasonList = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: undefined,
    seasons: [],
    onChange: () => undefined,
  }),
);
assert.match(emptySeasonList, /<select[^>]*disabled=""/);

const loading = renderToStaticMarkup(createElement(PageLoadingState, { message: "正在加载" }));
assert.match(loading, /role="status"/);
assert.match(loading, /aria-live="polite"/);
assert.match(loading, /aria-busy="true"/);
assert.match(loading, /正在加载/);

const empty = renderToStaticMarkup(
  createElement(PageEmptyState, {
    title: "暂无数据",
    description: "请先同步",
    actionLabel: "去同步",
    onAction: () => undefined,
  }),
);
assert.match(empty, /暂无数据/);
assert.match(empty, /请先同步/);
assert.equal(empty.match(/<button\b/g)?.length, 1);
assert.match(empty, /<button[^>]*type="button"[^>]*>去同步<\/button>/);

const error = renderToStaticMarkup(
  createElement(PageErrorState, { message: "读取失败", onRetry: () => undefined }),
);
assert.match(error, /role="alert"/);
assert.match(error, /读取失败/);
assert.match(error, /<button[^>]*type="button"[^>]*>重试<\/button>/);

const status = renderToStaticMarkup(
  createElement(StatusPill, { tone: "success", children: "已完成" }),
);
assert.match(status, /class="[^"]*statusPill--success[^"]*"/);
assert.match(status, /data-tone="success"/);
assert.match(status, /已完成/);

const pageShellCss = readFileSync(new URL("../src/styles/page-shell.css", import.meta.url), "utf8");
assert.match(
  pageShellCss,
  /\.statusPill\s*\{[^}]*border-radius:\s*var\(--radius-control\)/s,
  "status pills reuse the semantic control radius",
);
for (const tone of ["success", "danger", "warning"]) {
  assert.match(
    pageShellCss,
    new RegExp(`\\.statusPill--${tone}\\s*\\{[^}]*color:\\s*var\\(--text-primary\\)`, "s"),
    `${tone} status text keeps readable contrast`,
  );
}
