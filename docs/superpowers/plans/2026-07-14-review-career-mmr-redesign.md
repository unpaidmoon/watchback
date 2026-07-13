# Review and Career MMR Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将战报重组为共享赛季的五个 URL 标签页，补齐排行与好友详情的来源上下文，并按设计删除生涯 MMR 的近期单场解释、完善图表可读性和布局。

**Architecture:** 战报使用纯函数模块统一解析和生成 `season`、`view`、排行目标及好友详情返回上下文；页面请求只依赖赛季，标签切换只改变 URL 和展示内容。好友详情通过 React Router `state` 携带仅供后端请求的 `bnetId`，公开 URL 不新增原始玩家 ID，滚动位置只以页面 URL 为键保存在 `sessionStorage`。MMR 保留现有后端聚合与公开 API，只从页面请求集合和 UI 中移除近期解释，并用纯格式化函数生成坐标轴、悬停文本和屏幕阅读器摘要。

**Tech Stack:** React 18、TypeScript、React Router 6、Tauri 2、Node/tsx 源码契约测试、CSS、内联 SVG。

---

## 执行前提与文件职责

- 开始 Task 1 前运行 `git status --short`，保留所有非本计划改动；本计划不读取 app data、凭据、浏览器 profile 或真实玩家数据。
- `apps/owstats-desktop/src/features/review/reviewNavigation.ts`：新增；唯一负责战报 URL、跨页来源和无隐私 ID 的滚动恢复键。
- `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`：协调同赛季战报与分析请求，并按当前 `view` 选择内容。
- `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx`：拆分好友与地图展示组件，接收明确的赛季和来源上下文。
- `apps/owstats-desktop/src/pages/FriendDetailPage.tsx`：读取路由 state、按来源赛季请求并返回原战报 URL。
- `apps/owstats-desktop/src/pages/CareerMmrPage.tsx`：保留总览、历史、模式和赛季请求，删除近期解释请求与面板。
- `apps/owstats-desktop/src/shared/careerMmrFormat.ts`：生成图表轴刻度、精确悬停文本和无障碍摘要。
- `apps/owstats-desktop/src/styles/features/review.css`、`career-mmr.css`：只新增或调整本切片样式；不迁移无关的 `shared.css` 规则。
- `apps/owstats-desktop/scripts/reviewNavigation.test.ts`：新增纯 URL/返回上下文测试。
- `apps/owstats-desktop/scripts/analysisMovedToReview.test.ts`、`friendBoardsReviewUi.test.ts`、`phase8MultiSeasonPages.test.ts`、`careerMmrUi.test.ts`：更新旧源码契约。

### Task 1: 固定战报五标签 URL 合同

**Files:**
- Create: `apps/owstats-desktop/src/features/review/reviewNavigation.ts`
- Create: `apps/owstats-desktop/scripts/reviewNavigation.test.ts`
- Modify: `apps/owstats-desktop/package.json:18-20`

- [ ] **Step 1: 写 URL 和返回上下文失败测试**

```ts
import assert from "node:assert/strict";
import {
  buildFriendDetailState,
  buildReviewHref,
  buildReviewRankingHref,
  parseReviewView,
  reviewScrollKey,
} from "../src/features/review/reviewNavigation";

assert.equal(parseReviewView(null), "overview");
assert.equal(parseReviewView("roles"), "roles");
assert.equal(parseReviewView("friends"), "friends");
assert.equal(parseReviewView("maps"), "maps");
assert.equal(parseReviewView("achievements"), "achievements");
assert.equal(parseReviewView("invalid"), "overview");
assert.equal(buildReviewHref(23, "friends"), "/review?season=23&view=friends");
assert.equal(buildReviewRankingHref(22), "/ranking?tab=maps&season=22");

const detailState = buildFriendDetailState("private-id", 21, "maps", 640);
assert.equal(detailState.bnetId, "private-id");
assert.equal(detailState.season, 21);
assert.deepEqual(detailState.returnTo, {
  href: "/review?season=21&view=maps",
  scrollKey: "review-scroll:/review?season=21&view=maps",
  scrollY: 640,
});
assert.equal(reviewScrollKey("/review?season=21&view=maps"), detailState.returnTo.scrollKey);
assert.equal(detailState.returnTo.href.includes("private-id"), false);
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewNavigation.test.ts`

Expected: FAIL，提示无法解析 `../src/features/review/reviewNavigation`。

- [ ] **Step 3: 实现纯导航模块**

```ts
export const reviewViews = ["overview", "roles", "friends", "maps", "achievements"] as const;
export type ReviewView = (typeof reviewViews)[number];

export type ReviewReturnContext = {
  href: string;
  scrollKey: string;
  scrollY: number;
};

export type FriendDetailLocationState = {
  bnetId: string;
  season: number;
  returnTo: ReviewReturnContext;
};

export function parseReviewView(value: string | null): ReviewView {
  return reviewViews.includes(value as ReviewView) ? (value as ReviewView) : "overview";
}

export function buildReviewHref(season: number, view: ReviewView) {
  const params = new URLSearchParams({ season: String(season), view });
  return `/review?${params.toString()}`;
}

export function buildReviewRankingHref(season: number) {
  return `/ranking?tab=maps&season=${season}`;
}

export function reviewScrollKey(href: string) {
  return `review-scroll:${href}`;
}

export function buildFriendDetailState(
  bnetId: string,
  season: number,
  view: ReviewView,
  scrollY: number,
): FriendDetailLocationState {
  const href = buildReviewHref(season, view);
  return {
    bnetId,
    season,
    returnTo: { href, scrollKey: reviewScrollKey(href), scrollY },
  };
}
```

在 `package.json` 的 `test:unit` 命令中把 `tsx scripts/reviewNavigation.test.ts` 放在 `careerMmrUi.test.ts` 之前，使 URL 合同进入统一测试入口。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewNavigation.test.ts`

Expected: PASS，进程退出码为 0，且测试输出不包含 `private-id` 之外的任何运行时数据。

- [ ] **Step 5: 提交导航合同**

```powershell
git add -- apps/owstats-desktop/src/features/review/reviewNavigation.ts apps/owstats-desktop/scripts/reviewNavigation.test.ts apps/owstats-desktop/package.json
git commit -m "test: define review navigation context"
```

### Task 2: 把战报改为共享赛季的五个标签页

**Files:**
- Modify: `apps/owstats-desktop/scripts/analysisMovedToReview.test.ts`
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx:69-220`
- Modify: `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx:145-306`
- Modify: `apps/owstats-desktop/src/styles/features/review.css`

- [ ] **Step 1: 写五标签和请求复用失败测试**

在 `analysisMovedToReview.test.ts` 增加以下断言；保留现有“分析模块已进入战报”断言：

```ts
for (const view of ["overview", "roles", "friends", "maps", "achievements"]) {
  assert.match(reviewSource, new RegExp(`value="${view}"`), `review renders ${view} tab`);
}
assert.match(reviewSource, /parseReviewView\(params\.get\("view"\)\)/);
assert.match(reviewSource, /nextParams\.set\("view", nextView\)/);
assert.match(reviewSource, /<FriendAnalysisModules/);
assert.match(reviewSource, /<MapAnalysisModules/);
assert.doesNotMatch(reviewSource, /<SeasonAnalysisModules analysis=\{analysis\}/);
assert.match(reviewSource, /<PageHeader/);
assert.match(reviewSource, /<PageSeasonSelect/);
assert.match(reviewSource, /<PageLoadingState/);
assert.match(reviewSource, /<PageErrorState/);
assert.match(reviewSource, /<PageEmptyState/);
assert.match(reviewSource, /\}, \[selectedSeason\]\);/s, "review data requests depend on season, not view");
assert.doesNotMatch(reviewSource, /\[selectedSeason,\s*view\]/, "tab switches must reuse loaded season data");
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/analysisMovedToReview.test.ts`

Expected: FAIL，第一个新增断言报告缺少 `overview` 标签值。

- [ ] **Step 3: 实现标签状态和按需展示**

在 `PersonalReviewPage.tsx` 从 query 派生当前标签，不增加第二份 React 标签状态：

```tsx
const view = parseReviewView(params.get("view"));

function changeView(nextView: ReviewView) {
  const nextParams = new URLSearchParams(params);
  nextParams.set("view", nextView);
  setParams(nextParams);
}
```

赛季切换时保留当前标签；若 URL 没有 `view`，以 replace 写入 `overview`。两个现有请求 effect 的依赖继续只有 `[selectedSeason]`，不得把 `view` 加入依赖。

页面最外层先接入共享壳，替换旧的自建 `h2`、赛季 label 及裸加载/错误/空状态：

```tsx
<PageHeader
  title="战报"
  description={titleSeason ? `S${titleSeason} 赛季表现与协作复盘` : "查看赛季表现与协作复盘"}
  actions={
    <PageSeasonSelect
      value={selectedSeason}
      seasons={selectableSeasons}
      onChange={(season) => {
        if (season !== undefined) changeSeason(season);
      }}
    />
  }
/>
```

首次请求使用 `PageLoadingState`，失败使用带原请求重试回调的 `PageErrorState`，无赛季数据使用 `PageEmptyState` 并提供“前往指挥台”动作。不得让错误状态只剩颜色或一条不可恢复的横幅。

将页面主体改为以下明确分区：

```tsx
<nav className="reviewTabs" aria-label="战报内容">
  {reviewViews.map((item) => (
    <button
      key={item}
      type="button"
      value={item}
      aria-current={view === item ? "page" : undefined}
      className={view === item ? "active" : ""}
      onClick={() => changeView(item)}
    >
      {reviewViewLabels[item]}
    </button>
  ))}
</nav>

<div className="reviewViewGrid" data-view={view}>
  {view === "overview" && <ReviewOverview review={review} />}
  {view === "roles" && <RoleAndHeroView review={review} />}
  {view === "friends" && (
    <FriendAnalysisModules analytics={analytics} loading={analysisLoading} error={analysisError} />
  )}
  {view === "maps" && (
    <MapAnalysisModules analysis={analysis} loading={analysisLoading} error={analysisError} season={titleSeason} />
  )}
  {view === "achievements" && (
    <AchievementWall achievements={achievements} onSelect={setSelectedAchievement} />
  )}
</div>
```

`reviewViewLabels` 固定为：

```ts
const reviewViewLabels: Record<ReviewView, string> = {
  overview: "概览",
  roles: "职责与英雄",
  friends: "队友协作",
  maps: "地图",
  achievements: "成就",
};
```

把 `SeasonAnalysisModules` 拆成导出的 `FriendAnalysisModules` 与 `MapAnalysisModules`；`FriendAnalyticsPage` 仍可顺序组合两者，避免改变独立好友分析路由。`ReviewOverview` 只复用已有 `summary`、`roleSummaries` 和 `phase5Highlights`，不新增后端字段。

在 `review.css` 增加 `.reviewTabs`、`.reviewViewGrid`、`.reviewOverviewMetrics`，激活项使用深炭背景；宽屏主内容采用 `minmax(0, 2fr) minmax(260px, 1fr)`，小于 900px 改为单列。不要把新声明追加到 `shared.css`。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/analysisMovedToReview.test.ts; npx tsx scripts/reviewAchievementWall.test.ts; npx tsx scripts/reviewTrimmedSections.test.ts`

Expected: 三个脚本全部 PASS；成就墙、职责段位和既有分析模块仍存在。

- [ ] **Step 5: 提交战报标签布局**

```powershell
git add -- apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx apps/owstats-desktop/src/styles/features/review.css apps/owstats-desktop/scripts/analysisMovedToReview.test.ts
git commit -m "feat: organize review into URL tabs"
```

### Task 3: 保留排行和好友详情的来源、赛季与滚动位置

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- Modify: `apps/owstats-desktop/scripts/friendBoardsReviewUi.test.ts`
- Modify: `apps/owstats-desktop/src/App.tsx:67-68`
- Modify: `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx:189-331`
- Modify: `apps/owstats-desktop/src/pages/FriendDetailPage.tsx:1-88`
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/friends.css`

- [ ] **Step 1: 写跨页上下文失败测试**

在 `phase8MultiSeasonPages.test.ts` 增加：

```ts
const friendDetailSource = readFileSync(new URL("../src/pages/FriendDetailPage.tsx", import.meta.url), "utf8");

assert.match(analyticsSource, /buildReviewRankingHref\(season\)/);
assert.match(analyticsSource, /buildFriendDetailState\(friend\.bnetId, season, sourceView, window\.scrollY\)/);
assert.match(friendDetailSource, /getFriendDetails\(state\.bnetId, \{ season: state\.season \}\)/);
assert.match(friendDetailSource, /sessionStorage\.setItem\(state\.returnTo\.scrollKey/);
assert.match(friendDetailSource, /navigate\(state\.returnTo\.href\)/);
assert.doesNotMatch(friendDetailSource, /detail\?\.name \?\? bnetId/);
```

将 `friendBoardsReviewUi.test.ts` 的静态卡片旧断言替换为：

```ts
assert.match(source, /className="friendCardLink"/);
assert.match(source, /aria-label=\{`查看\$\{friend\.name\}详情`\}/);
assert.doesNotMatch(source, /to=\{`\/analytics\/friends\/\$\{friend\.bnetId\}`\}/);
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/phase8MultiSeasonPages.test.ts; npx tsx scripts/friendBoardsReviewUi.test.ts`

Expected: FAIL，报告地图链接未调用 `buildReviewRankingHref`，好友卡没有详情入口。

- [ ] **Step 3: 实现不暴露原始 ID 的详情路由和滚动恢复**

将详情路由改为固定路径：

```tsx
<Route path="/analytics/friends/detail" element={<FriendDetailPage />} />
```

`FriendAnalysisModules` 接收 `season: number` 与 `sourceView: ReviewView`。好友卡使用固定 URL，敏感 `bnetId` 只放在 router state：

```tsx
<Link
  className="friendCardLink"
  to="/analytics/friends/detail"
  state={buildFriendDetailState(friend.bnetId, season, sourceView, window.scrollY)}
  onClick={() => {
    const state = buildFriendDetailState(friend.bnetId, season, sourceView, window.scrollY);
    sessionStorage.setItem(state.returnTo.scrollKey, String(state.returnTo.scrollY));
  }}
  aria-label={`查看${friend.name}详情`}
>
  <article className="friendCard phase6FriendCard">{/* 保留现有中文内容 */}</article>
</Link>
```

地图卡改为：

```tsx
<Link className="mapAnalysisCard phase6MapAnalysisCard" to={buildReviewRankingHref(season)}>
```

`FriendDetailPage` 严格校验 location state；缺失时显示中文恢复动作，不显示原始 ID：

```tsx
const location = useLocation();
const state = location.state as FriendDetailLocationState | null;

useEffect(() => {
  if (!state) return;
  void getFriendDetails(state.bnetId, { season: state.season })
    .then(setDetail)
    .catch((err) => setError(productErrorMessage(err)))
    .finally(() => setLoading(false));
}, [state?.bnetId, state?.season]);

function returnToReview() {
  if (!state) {
    navigate("/review");
    return;
  }
  sessionStorage.setItem(state.returnTo.scrollKey, String(state.returnTo.scrollY));
  navigate(state.returnTo.href);
}
```

`PersonalReviewPage` 在 `season/view` 组合变化后恢复并删除滚动快照：

```tsx
useLayoutEffect(() => {
  if (!titleSeason) return;
  const key = reviewScrollKey(buildReviewHref(titleSeason, view));
  const stored = sessionStorage.getItem(key);
  if (stored === null) return;
  sessionStorage.removeItem(key);
  const scrollY = Number(stored);
  if (Number.isFinite(scrollY)) requestAnimationFrame(() => window.scrollTo({ top: scrollY }));
}, [titleSeason, view]);
```

在 `friends.css` 为 `.friendCardLink` 增加块级布局、可见 `:focus-visible` 轮廓和不改变卡片尺寸的 hover 状态。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewNavigation.test.ts; npx tsx scripts/phase8MultiSeasonPages.test.ts; npx tsx scripts/friendBoardsReviewUi.test.ts`

Expected: 三个脚本全部 PASS；测试确认排行 URL 含 `tab=maps&season`，好友详情公开 URL 不含 `bnetId`。

- [ ] **Step 5: 提交跨页上下文**

```powershell
git add -- apps/owstats-desktop/src/App.tsx apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx apps/owstats-desktop/src/pages/FriendDetailPage.tsx apps/owstats-desktop/src/styles/features/friends.css apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts apps/owstats-desktop/scripts/friendBoardsReviewUi.test.ts
git commit -m "fix: preserve review navigation context"
```

### Task 4: 删除 MMR 近期解释的页面请求和 UI

**Files:**
- Modify: `apps/owstats-desktop/scripts/careerMmrUi.test.ts`
- Modify: `apps/owstats-desktop/src/pages/CareerMmrPage.tsx:1-202`
- Modify: `apps/owstats-desktop/src/styles/features/career-mmr.css:23-41`

- [ ] **Step 1: 把旧测试改成删除合同**

用以下断言替换 `careerMmrUi.test.ts` 中要求 `getRecentMmrExplanations` 存在的断言：

```ts
assert.doesNotMatch(pageSource, /getRecentMmrExplanations/);
assert.doesNotMatch(pageSource, /recentExplanations/);
assert.doesNotMatch(pageSource, /近期单场解释/);
assert.doesNotMatch(pageSource, /CareerMmrRecentExplanations/);
assert.match(pageSource, /Promise\.all\(\[\s*getRoleMmrHistory[\s\S]*getRoleMmrModeBreakdown[\s\S]*getRoleMmrSeasonBreakdown[\s\S]*\]\)/);
assert.match(pageSource, /className="careerMmrPanel careerMmrSeasonPanel"/);
assert.doesNotMatch(pageSource, /careerMmrLowerGrid/);
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/careerMmrUi.test.ts`

Expected: FAIL，第一条删除断言命中当前 `getRecentMmrExplanations` 导入。

- [ ] **Step 3: 最小删除页面调用和面板**

从 `CareerMmrPage.tsx` 删除：

- `getRecentMmrExplanations` 导入。
- `CareerMmrRecentExplanations` 类型导入。
- `recentExplanations` state 及所有 `setRecentExplanations`。
- 详情 `Promise.all` 的第四个请求和第四个结果。
- “近期单场解释”整块 JSX 与 `formatExplanationFactor`。

把赛季分解直接渲染为全宽面板：

```tsx
<article className="careerMmrPanel careerMmrSeasonPanel">
  <div className="careerMmrPanelTitle">
    <div><Clock3 size={17} /><b>赛季分解</b></div>
  </div>
  <div className="careerMmrSeasonTable">{/* 保留现有赛季行 */}</div>
</article>
```

从 `career-mmr.css` 删除 `.careerMmrLowerGrid`、`.careerMmrEventList` 及其子规则，新增：

```css
.careerMmrSeasonPanel { width: 100%; }
```

不要删除 `shared/api.ts`、`shared/types.ts` 或 Rust 中的解释 API；本任务只停止页面请求，不扩大后端范围。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/careerMmrUi.test.ts`

Expected: PASS；总览、历史、模式、赛季请求断言继续通过。

- [ ] **Step 5: 提交删除切片**

```powershell
git add -- apps/owstats-desktop/src/pages/CareerMmrPage.tsx apps/owstats-desktop/src/styles/features/career-mmr.css apps/owstats-desktop/scripts/careerMmrUi.test.ts
git commit -m "refactor: remove recent MMR explanation panel"
```

### Task 5: 完成 MMR 图表、模式口径和页面层级

**Files:**
- Modify: `apps/owstats-desktop/scripts/careerMmrUi.test.ts`
- Modify: `apps/owstats-desktop/src/shared/careerMmrFormat.ts:29-67`
- Modify: `apps/owstats-desktop/src/pages/CareerMmrPage.tsx:114-219`
- Modify: `apps/owstats-desktop/src/styles/features/career-mmr.css`

- [ ] **Step 1: 写轴、悬停和摘要失败测试**

在 `careerMmrUi.test.ts` 导入并测试新纯函数：

```ts
import {
  buildCareerMmrChart,
  buildCareerMmrChartSummary,
  buildCareerMmrYAxisTicks,
  formatCareerMmrTooltip,
} from "../src/shared/careerMmrFormat";

const chartPoints = [
  { timestamp: 1_720_000_000, season: 22, mmr: 2480, confidence: 0.7, delta: -12.5 },
  { timestamp: 1_720_086_400, season: 22, mmr: 2525, confidence: 0.8, delta: 45.0 },
];
const chart = buildCareerMmrChart(chartPoints, 760, 210)!;
const ticks = buildCareerMmrYAxisTicks(chart.domainMin, chart.domainMax, 4);
assert.equal(ticks.length, 4);
assert.ok(ticks.every((value) => Number.isFinite(value)));
assert.match(formatCareerMmrTooltip(chartPoints[1]), /S22/);
assert.match(formatCareerMmrTooltip(chartPoints[1]), /2,525/);
assert.match(buildCareerMmrChartSummary("damage", chartPoints), /输出/);
assert.match(buildCareerMmrChartSummary("damage", chartPoints), /上升 45/);
assert.match(pageSource, /aria-label="MMR 纵轴"/);
assert.match(pageSource, /<title>\{formatCareerMmrTooltip\(point\)\}<\/title>/);
assert.match(pageSource, /className="careerMmrScreenReaderSummary"/);
assert.match(pageSource, /生涯累计贡献/);
assert.match(pageSource, />展示赛季</);
assert.match(pageSource, /<PageHeader/);
assert.match(pageSource, /<PageSeasonSelect/);
assert.match(pageSource, /<PageLoadingState/);
assert.match(pageSource, /<PageErrorState/);
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/careerMmrUi.test.ts`

Expected: FAIL，提示 `buildCareerMmrChartSummary` 等导出尚不存在。

- [ ] **Step 3: 实现纯格式化函数和可访问 SVG**

在 `careerMmrFormat.ts` 增加：

```ts
type CareerChartNarrativePoint = CareerChartInput & {
  timestamp: number;
  season: number;
  delta: number;
};

export function buildCareerMmrYAxisTicks(domainMin: number, domainMax: number, count: number) {
  if (count < 2) return [Math.round(domainMin)];
  const step = (domainMax - domainMin) / (count - 1);
  return Array.from({ length: count }, (_, index) => Math.round(domainMax - index * step));
}

export function formatCareerMmrTooltip(point: CareerChartNarrativePoint) {
  return `S${point.season} · ${formatCareerDate(point.timestamp)} · MMR ${formatCareerMmr(point.mmr)} · 变化 ${signedCareerDelta(point.delta)}`;
}

export function buildCareerMmrChartSummary(role: CareerRole, points: CareerChartNarrativePoint[]) {
  if (!points.length) return `${careerRoleLabels[role]}暂无历史记录。`;
  const first = points[0];
  const last = points[points.length - 1];
  const change = Math.round(last.mmr - first.mmr);
  const direction = change > 0 ? `上升 ${change}` : change < 0 ? `下降 ${Math.abs(change)}` : "保持不变";
  return `${careerRoleLabels[role]}共有 ${points.length} 个记录点，MMR 从 ${Math.round(first.mmr)} 到 ${Math.round(last.mmr)}，${direction}。`;
}
```

给 `buildCareerMmrChart` 的 line 点保留输入索引，使页面能够为每个点渲染透明但可聚焦的命中圆：

```tsx
{chart.line.map((coordinate, index) => {
  const point = points[index];
  return (
    <circle key={`${point.timestamp}-${index}`} cx={coordinate.x} cy={coordinate.y} r="8" tabIndex={0}>
      <title>{formatCareerMmrTooltip(point)}</title>
    </circle>
  );
})}
```

在 SVG 左侧渲染四个纵轴刻度和 `MMR` 单位，底部显示首、中、末日期；在图表后添加：

```tsx
<p className="careerMmrScreenReaderSummary">
  {buildCareerMmrChartSummary(history!.role, points)}
</p>
```

将赛季控件标签改为“展示赛季”，保留“只改变趋势和分解展示，不重新计算当前 MMR”。将模式项目值改为：

用共享页面壳替换旧 hero 头部和裸状态：

```tsx
<PageHeader
  title="生涯 MMR"
  description="本地模型推算，并非暴雪官方隐藏分"
  actions={
    <PageSeasonSelect
      label="展示赛季"
      value={selectedSeason}
      seasons={overview?.discoveredSeasons ?? []}
      includeAll
      allLabel="全部"
      onChange={setSelectedSeason}
    />
  }
/>
```

总览首次加载使用 `PageLoadingState`，失败使用 `PageErrorState` 并重新触发总览请求；详情请求失败在趋势区域使用同一错误组件，不移除已经可读的职责摘要。

模式项目值改为：

```tsx
<span>生涯累计贡献</span>
<strong className={item.cumulativeDelta >= 0 ? "positive" : "negative"}>
  {signedCareerDelta(item.cumulativeDelta)}
</strong>
```

`career-mmr.css` 为纵轴、日期刻度、可聚焦命中点和只对屏幕阅读器可见摘要增加样式；置信区间继续使用同色 10%-15% 透明填充。职责激活态只保留深色小标记或紫色圆点，不对整卡增加 glow。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/careerMmrUi.test.ts`

Expected: PASS；空历史、平坦历史、低置信区间、轴刻度、悬停文本和摘要全部通过。

- [ ] **Step 5: 提交 MMR 可读性切片**

```powershell
git add -- apps/owstats-desktop/src/shared/careerMmrFormat.ts apps/owstats-desktop/src/pages/CareerMmrPage.tsx apps/owstats-desktop/src/styles/features/career-mmr.css apps/owstats-desktop/scripts/careerMmrUi.test.ts
git commit -m "feat: improve career MMR chart readability"
```

### Task 6: 完整验证战报与 MMR 切片

**Files:**
- Verify: 本计划列出的全部文件

- [ ] **Step 1: 运行聚焦前端测试**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewNavigation.test.ts; npx tsx scripts/analysisMovedToReview.test.ts; npx tsx scripts/friendBoardsReviewUi.test.ts; npx tsx scripts/phase8MultiSeasonPages.test.ts; npx tsx scripts/careerMmrUi.test.ts`

Expected: 五个脚本全部 PASS。

- [ ] **Step 2: 运行统一前端验证**

Run: `cd apps/owstats-desktop; npm test; npm run build`

Expected: 两条命令退出码均为 0；TypeScript 无错误，Vite 构建完成，发布产物路径扫描通过。

- [ ] **Step 3: 运行隐私与路径检查**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -ChangedOnly`

Expected: 递归敏感字段扫描、前端测试、构建和 `git diff --check` 全部通过；输出不含 token-like 值、真实玩家 ID 或开发机绝对路径。

- [ ] **Step 4: 执行浏览器烟测**

Run: `cd apps/owstats-desktop; npm run dev -- --host 127.0.0.1`

Expected: Vite 在 `http://127.0.0.1:1420` 启动。使用脱敏 Tauri mock 验证 `/review?season=23&view=overview` 的五个标签、地图跳转 URL、好友详情返回滚动位置，以及 `/career-mmr` 无“近期单场解释”、图表点可键盘聚焦。不得连接真实桌面后端。

- [ ] **Step 5: 审查最终差异并确认工作树已由前五个聚焦提交收口**

Run: `git diff --check; git status --short`

Expected: `git diff --check` 无错误；`git status --short` 无输出。验证步骤不创建空提交，也不把其他切片文件打包进一个收口提交。
