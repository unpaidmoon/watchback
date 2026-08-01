import assert from "node:assert/strict";
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
assert.ok(header.includes("<h1>对局</h1>"));
assert.ok(header.includes("浏览本地对局"));
assert.ok(header.includes("刷新"));

const season = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: undefined,
    seasons: [21, 22, 23],
    includeAll: true,
    allLabel: "全部赛季",
    onChange: () => undefined,
  }),
);
assert.ok(season.includes(">赛季</label>"));
assert.ok(season.includes('<option value="" selected="">全部赛季</option>'));
assert.ok(season.includes('<option value="22">S22</option>'));
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
assert.ok(seasonWithoutAll.includes('<option value="" disabled="" selected="">请选择赛季</option>'));
assert.ok(!seasonWithoutAll.includes('<option value="21" selected="">S21</option>'));

const unavailableSeason = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: 24,
    seasons: [21, 22, 23],
    includeAll: true,
    onChange: () => undefined,
  }),
);
assert.ok(unavailableSeason.includes('<option value="24" disabled="" selected="">S24 不可用</option>'));
assert.ok(!unavailableSeason.includes('<option value="" selected="">全部赛季</option>'));

const selectedSeason = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: 22,
    seasons: [21, 22, 23],
    onChange: () => undefined,
  }),
);
assert.ok(selectedSeason.includes('<option value="22" selected="">S22</option>'));

const emptySeasonList = renderToStaticMarkup(
  createElement(PageSeasonSelect, {
    value: undefined,
    seasons: [],
    onChange: () => undefined,
  }),
);
assert.ok(emptySeasonList.includes("<select") && emptySeasonList.includes('disabled=""'));

const loading = renderToStaticMarkup(createElement(PageLoadingState, { message: "正在加载" }));
assert.ok(loading.includes('role="status"'));
assert.ok(loading.includes('aria-live="polite"'));
assert.ok(loading.includes('aria-busy="true"'));
assert.ok(loading.includes("正在加载"));

const empty = renderToStaticMarkup(
  createElement(PageEmptyState, {
    title: "暂无数据",
    description: "请先同步",
    actionLabel: "去同步",
    onAction: () => undefined,
  }),
);
assert.ok(empty.includes("暂无数据"));
assert.ok(empty.includes("请先同步"));
assert.equal(empty.match(/<button\b/g)?.length, 1);
assert.ok(empty.includes('type="button"') && empty.includes(">去同步</button>"));

const error = renderToStaticMarkup(
  createElement(PageErrorState, { message: "读取失败", onRetry: () => undefined }),
);
assert.ok(error.includes('role="alert"'));
assert.ok(error.includes("读取失败"));
assert.ok(error.includes('type="button"') && error.includes(">重新加载</button>"));

const reloadError = renderToStaticMarkup(
  createElement(PageErrorState, {
    message: "对局读取失败",
    actionLabel: "重新加载",
    onRetry: () => undefined,
  }),
);
assert.ok(reloadError.includes("对局读取失败"));
assert.ok(reloadError.includes('type="button"') && reloadError.includes(">重新加载</button>"));

const status = renderToStaticMarkup(
  createElement(StatusPill, { tone: "success", children: "已完成" }),
);
assert.ok(status.includes("statusPill--success"));
assert.ok(status.includes('data-tone="success"'));
assert.ok(status.includes("已完成"));
