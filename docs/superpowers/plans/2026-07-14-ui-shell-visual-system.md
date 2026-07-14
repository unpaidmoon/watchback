# UI Shell and Visual System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将默认入口、一级导航、全局视觉令牌和共享 page-shell 组件落实为控制设计规定的中文桌面应用外壳，同时保留各业务页面独立赛季与请求状态。

**Architecture:** `App.tsx` 只组装 HashRouter 路由、全局外壳和路由焦点管理；一级导航继续由 `phase6Shell.ts` 的集中配置驱动。新视觉令牌、基础规则、shell 规则和 page-shell 规则分别进入独立 CSS 文件，旧业务样式暂留 `styles/shared.css`；业务页面接入由后续对局、战报/MMR、排行/指挥台切片负责。“本地数据”入口先使用最终目标 hash，实际指挥台锚点与 Onboarding 页面结构改动严格留到最后的指挥台切片。

**Tech Stack:** React 18、TypeScript、React Router 6、Lucide React、CSS、Node/tsx 源码契约测试、Vite、Tauri 2

---

## 执行前提

- 开始前运行 `git status --short`，保留所有执行前已有改动。
- 控制设计 `docs/superpowers/specs/2026-07-14-ui-architecture-visual-redesign-design.md` 的第 5、6、7、10、11、12、13 节优先于旧 Phase 6 测试断言。
- 不读取 app data、凭据文件、浏览器 profile 或真实玩家数据；测试只读源码和静态渲染组件。
- 本计划不修改同步协议、后端、SQLite、对局缓存或页面请求参数。

## 文件职责与接入边界

- `apps/owstats-desktop/src/App.tsx`：仅路由和全局外壳；移除对局预加载和无行为铃铛。
- `apps/owstats-desktop/src/shared/phase6Shell.ts`：一级导航配置和现有通用文案。
- `apps/owstats-desktop/src/shared/page-shell/RouteFocusManager.tsx`：路由 hash 目标的滚动与焦点恢复。
- `apps/owstats-desktop/src/shared/page-shell/PageHeader.tsx`：页面 `h1`、用途说明和右侧 action 槽。
- `apps/owstats-desktop/src/shared/page-shell/PageSeasonSelect.tsx`：受控页面级赛季选择器。
- `apps/owstats-desktop/src/shared/page-shell/PageState.tsx`：加载、空和错误状态。
- `apps/owstats-desktop/src/shared/page-shell/StatusPill.tsx`：中性、成功、失败、警告状态。
- `apps/owstats-desktop/src/styles/tokens.css`：控制设计规定的语义令牌。
- `apps/owstats-desktop/src/styles/base.css`：系统字体、基础控件、焦点和 reduced-motion。
- `apps/owstats-desktop/src/styles/shell.css`：窗口外沿、画布、顶栏、导航和本地数据入口。
- `apps/owstats-desktop/src/styles/page-shell.css`：共享页面头部、赛季控件和状态组件。
- `apps/owstats-desktop/src/styles/shared.css`：旧业务声明保留，不在本计划做声明级搬迁。

后续页面接入所有权固定如下，本计划不得把这些业务页面全部接入同一提交：

| 后续切片 | 负责文件 | 必须保留的行为 |
|---|---|---|
| 对局工作台 | `pages/MatchesPage.tsx`、`features/matches/FilterBar.tsx` | URL、缓存、选中项和滚动恢复；赛季从筛选栏移至 `PageHeader` |
| 战报与 MMR | `pages/PersonalReviewPage.tsx`、`pages/CareerMmrPage.tsx` | 页面独立 season query、请求守卫；删除单场解释由该切片负责 |
| 排行与指挥台 | `pages/RankingPage.tsx`、`pages/OnboardingPage.tsx` | 排行聚合/排序不变；指挥台同步赛季不使用 `PageSeasonSelect` |

### Task 1: 修正根路由和全局导航行为

**Files:**
- Modify: `apps/owstats-desktop/src/App.tsx:1-79`
- Modify: `apps/owstats-desktop/src/shared/phase6Shell.ts:3-21`
- Create: `apps/owstats-desktop/src/shared/page-shell/RouteFocusManager.tsx`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] **Step 1: 先写失败契约**

在 `phase6Shell.test.ts` 读取 `App.tsx`，把旧导航断言替换并增加：

```ts
const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");

assertEqual(
  phase6NavItems.map((item) => item.label).join(","),
  "对局,战报,生涯 MMR,排行,指挥台",
  "primary navigation follows the approved order",
);
assertEqual(appSource.includes('<Navigate to="/matches" replace />'), true, "root route opens matches");
assertEqual(appSource.includes('to="/onboarding#local-data"'), true, "local data is a link to the data region");
assertEqual(appSource.includes('aria-label="查看本地数据"'), true, "local data link has an accessible name");
assertEqual(appSource.includes("Bell"), false, "unused notification bell is removed");
assertEqual(appSource.includes("preloadDefaultMatchesPage"), false, "App contains no match business preload");
assertEqual(appSource.includes('<RouteFocusManager />'), true, "shell manages deep-route focus");
```

- [ ] **Step 2: 运行并确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/phase6Shell.test.ts`

Expected: FAIL；实际仍为旧导航顺序、`/onboarding` 根重定向，且 App 仍包含 `Bell` 与 `preloadDefaultMatchesPage`。

- [ ] **Step 3: 实现最小导航配置和路由焦点管理**

将 `phase6NavItems` 改为：

```ts
export const phase6NavItems: Phase6NavItem[] = [
  { label: "对局", path: "/matches", title: "浏览和筛选对局" },
  { label: "战报", path: "/review", title: "个人赛季战报" },
  { label: "生涯 MMR", path: "/career-mmr", title: "本地估算的三职责生涯 MMR" },
  { label: "排行", path: "/ranking", title: "英雄与地图排行" },
  { label: "指挥台", path: "/onboarding", title: "登录、同步与数据状态" },
];
```

新增 `RouteFocusManager.tsx`：

```tsx
import { useEffect } from "react";
import { useLocation } from "react-router-dom";

export function RouteFocusManager() {
  const location = useLocation();

  useEffect(() => {
    if (!location.hash) return;
    const frame = window.requestAnimationFrame(() => {
      const target = document.getElementById(location.hash.slice(1));
      if (!target) return;
      target.scrollIntoView({ block: "start" });
      target.focus({ preventScroll: true });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [location.pathname, location.hash]);

  return null;
}
```

- [ ] **Step 4: 收紧 App 外壳**

从 `App.tsx` 删除 `Bell`、`useEffect`、`preloadDefaultMatchesPage` 和通知按钮；品牌名改为非 heading 元素；在 `QaFeedbackProvider` 内加入 `<RouteFocusManager />`；根路由改为：

```tsx
<Route path="/" element={<Navigate to="/matches" replace />} />
```

右侧入口使用：

```tsx
<NavLink className="appDataEntry" to="/onboarding#local-data" aria-label="查看本地数据">
  <CircleDot size={14} aria-hidden="true" />
  本地数据
</NavLink>
```

本任务不修改 `OnboardingPage.tsx`。`/onboarding#local-data` 的稳定目标由最后执行的指挥台计划创建，以避免抢先占用 BattleTag 分支可能触及的页面文件。

- [ ] **Step 5: 运行定向 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/phase6Shell.test.ts`

Expected: PASS；根路由、顺序、可点击入口、焦点目标、无铃铛和 App 纯外壳断言全部通过。

- [ ] **Step 6: 运行相关全量测试和构建**

Run: `cd apps/owstats-desktop; npm run test:legacy; npm run build`

Expected: 两条命令退出码 0；现有深路由、QA 层和页面编译不回归。

- [ ] **Step 7: 检查差异与状态**

Run: `git diff --check; git status --short`

Expected: diff 检查无错误；只列出本任务实现/测试路径及执行前已有改动。

- [ ] **Step 8: 组织提交**

```powershell
git add -- apps/owstats-desktop/src/App.tsx apps/owstats-desktop/src/shared/phase6Shell.ts apps/owstats-desktop/src/shared/page-shell/RouteFocusManager.tsx apps/owstats-desktop/scripts/phase6Shell.test.ts
git commit -m "refactor: align global shell navigation"
```

### Task 2: 建立语义视觉令牌、系统字体和新 shell 样式

**Files:**
- Modify: `apps/owstats-desktop/src/styles.css:1-10`
- Modify: `apps/owstats-desktop/src/styles/tokens.css`
- Modify: `apps/owstats-desktop/src/styles/base.css`
- Modify: `apps/owstats-desktop/src/styles/shell.css`
- Create: `apps/owstats-desktop/scripts/visualSystem.test.ts`
- Modify: `apps/owstats-desktop/package.json:16`
- Modify: `apps/owstats-desktop/src/App.tsx`

- [ ] **Step 1: 写令牌和样式入口失败测试**

```ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const tokens = readFileSync(new URL("../src/styles/tokens.css", import.meta.url), "utf8");
const base = readFileSync(new URL("../src/styles/base.css", import.meta.url), "utf8");
const shell = readFileSync(new URL("../src/styles/shell.css", import.meta.url), "utf8");
const entry = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");

for (const [name, value] of Object.entries({
  "--app-outside": "#F5F5F3",
  "--app-canvas": "#E3E8E2",
  "--surface-primary": "#F0F1EA",
  "--surface-secondary": "#F6F7F2",
  "--surface-control": "#ECEEE9",
  "--text-primary": "#202220",
  "--text-secondary": "#60645F",
  "--text-tertiary": "#8A8F89",
  "--border-subtle": "#D8DED8",
  "--action-primary": "#303030",
  "--action-on-primary": "#FFFFFF",
  "--data-primary": "#4B22D1",
  "--data-secondary": "#2F8C95",
  "--data-neutral": "#A5AAA4",
  "--status-success": "#21A366",
  "--status-success-bg": "#E2F3E7",
  "--status-danger": "#E54848",
  "--status-danger-bg": "#F8E7E5",
  "--status-warning": "#B58A16",
  "--status-warning-bg": "#F4EFD8",
})) {
  assert.match(tokens, new RegExp(`${name}:\\s*${value}`, "i"), `${name} matches the approved value`);
}
assert.match(base, /"Microsoft YaHei UI", "PingFang SC", "Noto Sans CJK SC", "Segoe UI", sans-serif/);
assert.ok(entry.indexOf('styles/shared.css') < entry.indexOf('styles/base.css'), "new base rules override legacy globals");
assert.doesNotMatch(shell, /#ff8a24|#d45f14|blue.*orange|orange.*blue/i);
```

- [ ] **Step 2: 运行并确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/visualSystem.test.ts`

Expected: FAIL；`tokens.css`、`base.css`、`shell.css` 仍是占位注释。

- [ ] **Step 3: 写最小令牌和基础规则**

`tokens.css` 必须声明上述全部颜色，并补充：

```css
:root {
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --radius-shell: 30px;
  --radius-primary: 20px;
  --radius-secondary: 16px;
  --radius-control: 12px;
  --shadow-card: 0 8px 24px rgba(35, 40, 35, 0.035);
  --motion-fast: 180ms;
}
```

`base.css` 写入：

```css
html { background: var(--app-outside); }
body {
  margin: 0;
  min-width: 1024px;
  min-height: 100vh;
  padding: 8px;
  color: var(--text-primary);
  background: var(--app-outside);
  font-family: "Microsoft YaHei UI", "PingFang SC", "Noto Sans CJK SC", "Segoe UI", sans-serif;
  font-size: 14px;
  line-height: 1.5;
}
button, input, select { font: inherit; color: inherit; }
[data-numeric] { font-variant-numeric: tabular-nums; }
```

- [ ] **Step 4: 写最小 shell 样式并调整入口顺序**

`shell.css` 建立 `.appFrame`、`.appTopBar`、`.appBrand`、`.appBrandName`、`.appNav`、`.appNavLink`、`.appShellStatus`、`.appDataEntry`、`.appCanvas`；关键声明为：

```css
.appFrame {
  width: min(100%, 1920px);
  min-height: calc(100vh - 16px);
  margin-inline: auto;
  overflow: clip;
  border-radius: var(--radius-shell);
  color: var(--text-primary);
  background: var(--app-canvas);
}
.appNavLink.active { color: var(--action-on-primary); background: var(--action-primary); }
.appCanvas { min-height: 0; padding: clamp(24px, 2vw, 32px); }
```

`styles.css` 顺序固定为 `tokens.css`、旧 `shared.css`、新 `base.css`、新 `shell.css`、各 feature 文件；App 改用新的 shell class，不删除业务页仍引用的旧 class。

- [ ] **Step 5: 把视觉测试加入组件测试入口并运行定向 GREEN**

把 `test:components` 改为：

```json
"test:components": "tsx scripts/stab006ModuleContracts.test.ts && tsx scripts/visualSystem.test.ts"
```

Run: `cd apps/owstats-desktop; npx tsx scripts/visualSystem.test.ts`

Expected: PASS；令牌、字体、入口顺序和 shell 禁用橙色断言通过。

- [ ] **Step 6: 运行相关全量测试和构建**

Run: `cd apps/owstats-desktop; npm run test:components; npm run build`

Expected: 退出码 0；CSS 聚合和 TypeScript/Vite 构建成功。

- [ ] **Step 7: 检查差异与状态**

Run: `git diff --check; git status --short`

Expected: 无空白错误；只包含本任务样式、App、测试和 package 脚本路径及执行前已有改动。

- [ ] **Step 8: 组织提交**

```powershell
git add -- apps/owstats-desktop/src/styles.css apps/owstats-desktop/src/styles/tokens.css apps/owstats-desktop/src/styles/base.css apps/owstats-desktop/src/styles/shell.css apps/owstats-desktop/src/App.tsx apps/owstats-desktop/scripts/visualSystem.test.ts apps/owstats-desktop/package.json
git commit -m "style: establish semantic shell visual system"
```

### Task 3: 建立 shared/page-shell 组件边界

**Files:**
- Create: `apps/owstats-desktop/src/shared/page-shell/PageHeader.tsx`
- Create: `apps/owstats-desktop/src/shared/page-shell/PageSeasonSelect.tsx`
- Create: `apps/owstats-desktop/src/shared/page-shell/PageState.tsx`
- Create: `apps/owstats-desktop/src/shared/page-shell/StatusPill.tsx`
- Create: `apps/owstats-desktop/src/shared/page-shell/index.ts`
- Create: `apps/owstats-desktop/src/styles/page-shell.css`
- Create: `apps/owstats-desktop/scripts/pageShellComponents.test.tsx`
- Modify: `apps/owstats-desktop/src/styles.css`
- Modify: `apps/owstats-desktop/package.json:16`

- [ ] **Step 1: 写共享组件失败测试**

```tsx
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect, StatusPill } from "../src/shared/page-shell/index";

const header = renderToStaticMarkup(createElement(PageHeader, {
  title: "对局", description: "浏览本地对局", actions: createElement("button", null, "刷新"),
}));
assert.match(header, /<h1>对局<\/h1>/);
assert.match(header, /浏览本地对局/);
assert.match(header, /刷新/);

const season = renderToStaticMarkup(createElement(PageSeasonSelect, {
  value: undefined, seasons: [21, 22, 23], includeAll: true, allLabel: "全部赛季", onChange: () => undefined,
}));
assert.match(season, />赛季</);
assert.match(season, /<option value="" selected="">全部赛季<\/option>/);
assert.match(season, /<option value="22">S22<\/option>/);

assert.match(renderToStaticMarkup(createElement(PageLoadingState, { message: "正在加载" })), /aria-busy="true"/);
assert.match(renderToStaticMarkup(createElement(PageEmptyState, { title: "暂无数据", description: "请先同步", actionLabel: "去同步", onAction: () => undefined })), /<button[^>]*>去同步<\/button>/);
assert.match(renderToStaticMarkup(createElement(PageErrorState, { message: "读取失败", onRetry: () => undefined })), /role="alert"/);
assert.match(renderToStaticMarkup(createElement(StatusPill, { tone: "success", children: "已完成" })), /已完成/);
```

- [ ] **Step 2: 运行并确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/pageShellComponents.test.tsx`

Expected: FAIL，模块 `src/shared/page-shell/index` 尚不存在。

- [ ] **Step 3: 写最小组件实现**

组件接口固定为：

```tsx
export type PageHeaderProps = { title: string; description: string; actions?: ReactNode; className?: string };
export type PageSeasonSelectProps = { value: number | undefined; seasons: readonly number[]; onChange: (season: number | undefined) => void; disabled?: boolean; label?: string; includeAll?: boolean; allLabel?: string };
export type PageLoadingStateProps = { message: string };
export type PageEmptyStateProps = { title: string; description: string; actionLabel: string; onAction: () => void };
export type PageErrorStateProps = { message: string; onRetry: () => void };
export type StatusPillProps = { tone?: "neutral" | "success" | "danger" | "warning"; children: ReactNode };
```

`PageHeader` 输出一个 `header`、一个 `h1`、说明段落和可选 action；`PageSeasonSelect` 输出可见“赛季”label、动态 `Sxx` options，并可由页面显式启用“全部赛季”空值选项，空列表时禁用；`PageLoadingState` 使用 `role="status" aria-live="polite" aria-busy="true"`；`PageEmptyState` 只有一个恢复按钮；`PageErrorState` 使用 `role="alert"` 并提供“重试”；`StatusPill` 始终包含文字内容。

- [ ] **Step 4: 写 page-shell 样式和导出入口**

`page-shell.css` 使用语义令牌实现 `.pageHeader`、`.pageSeasonSelect`、`.pageState`、`.pageStateAction` 和 `.statusPill`，标题字号 `32px`/字重 `500`，正文 `14px`/行高 `1.5`，主卡圆角 `20px`。在 `styles.css` 中把该文件放在 `shell.css` 之后、feature CSS 之前；`index.ts` 显式导出六个组件。

- [ ] **Step 5: 加入测试入口并运行定向 GREEN**

把 `test:components` 扩展为：

```json
"test:components": "tsx scripts/stab006ModuleContracts.test.ts && tsx scripts/visualSystem.test.ts && tsx scripts/pageShellComponents.test.tsx"
```

Run: `cd apps/owstats-desktop; npx tsx scripts/pageShellComponents.test.tsx`

Expected: PASS；标题语义、赛季选项、状态可访问性和恢复动作断言通过。

- [ ] **Step 6: 运行相关全量测试和构建**

Run: `cd apps/owstats-desktop; npm run test:components; npm run build`

Expected: 退出码 0；共享组件可独立编译，未要求业务页面立即接入。

- [ ] **Step 7: 检查差异与状态**

Run: `git diff --check; git status --short`

Expected: 无空白错误；只包含本任务新组件、样式、测试、入口与 package 脚本及执行前已有改动。

- [ ] **Step 8: 组织提交**

```powershell
git add -- apps/owstats-desktop/src/shared/page-shell/PageHeader.tsx apps/owstats-desktop/src/shared/page-shell/PageSeasonSelect.tsx apps/owstats-desktop/src/shared/page-shell/PageState.tsx apps/owstats-desktop/src/shared/page-shell/StatusPill.tsx apps/owstats-desktop/src/shared/page-shell/index.ts apps/owstats-desktop/src/styles/page-shell.css apps/owstats-desktop/src/styles.css apps/owstats-desktop/scripts/pageShellComponents.test.tsx apps/owstats-desktop/package.json
git commit -m "feat: add shared page shell primitives"
```

### Task 4: 建立焦点、减少动态效果和 44px 命中区

**Files:**
- Modify: `apps/owstats-desktop/scripts/visualSystem.test.ts`
- Modify: `apps/owstats-desktop/src/styles/base.css`
- Modify: `apps/owstats-desktop/src/styles/shell.css`
- Modify: `apps/owstats-desktop/src/styles/page-shell.css`

- [ ] **Step 1: 写可访问性失败测试**

```ts
assert.match(base, /:focus-visible[\s\S]*outline:\s*2px solid var\(--data-primary\)/, "focus ring uses the purple data token");
assert.match(base, /@media\s*\(prefers-reduced-motion:\s*reduce\)/, "reduced motion is supported");
assert.match(base, /button,\s*input,\s*select[\s\S]*min-height:\s*44px/, "base form controls meet the hit target");
assert.match(shell, /\.appNavLink,[\s\S]*\.appDataEntry[\s\S]*min-height:\s*44px/, "shell links meet the hit target");
assert.match(pageShell, /\.pageStateAction[\s\S]*min-height:\s*44px/, "page recovery action meets the hit target");
```

- [ ] **Step 2: 运行并确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/visualSystem.test.ts`

Expected: FAIL；全局焦点环、reduced-motion 和统一 44px 规则尚未定义。

- [ ] **Step 3: 写最小可访问性规则**

```css
button, input, select { min-height: 44px; }
:where(a, button, input, select, [tabindex]):focus-visible {
  outline: 2px solid var(--data-primary);
  outline-offset: 2px;
}
.appNavLink, .appDataEntry, .pageStateAction { min-height: 44px; }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    scroll-behavior: auto !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

新 shell 与 page-shell 的常规交互仅使用 `opacity` 和 `transform` 的 `180ms` transition；不得动画宽高、定位或滚动布局。

- [ ] **Step 4: 运行定向 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/visualSystem.test.ts; npx tsx scripts/pageShellComponents.test.tsx`

Expected: 两项 PASS；焦点、动效、命中区和组件语义同时通过。

- [ ] **Step 5: 运行相关全量测试和构建**

Run: `cd apps/owstats-desktop; npm run test:components; npm run test:legacy; npm run build`

Expected: 全部退出码 0；旧页面控件在新的最小高度下仍可编译。

- [ ] **Step 6: 检查差异与状态**

Run: `git diff --check; git status --short`

Expected: 无空白错误；只包含三个样式文件和视觉测试及执行前已有改动。

- [ ] **Step 7: 组织提交**

```powershell
git add -- apps/owstats-desktop/scripts/visualSystem.test.ts apps/owstats-desktop/src/styles/base.css apps/owstats-desktop/src/styles/shell.css apps/owstats-desktop/src/styles/page-shell.css
git commit -m "style: add accessible shell interactions"
```

### Task 5: 综合验证并确认页面接入没有越界

**Files:**
- Verify: all files changed in Tasks 1-4
- Verify unchanged: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Verify unchanged: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- Verify unchanged: `apps/owstats-desktop/src/pages/CareerMmrPage.tsx`
- Verify unchanged: `apps/owstats-desktop/src/pages/RankingPage.tsx`

- [ ] **Step 1: 运行外壳定向验证**

Run:

```powershell
cd apps/owstats-desktop
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/visualSystem.test.ts
npx tsx scripts/pageShellComponents.test.tsx
npx tsx scripts/matchesPageCache.test.ts
npx tsx scripts/phase8MultiSeasonPages.test.ts
```

Expected: 全部退出码 0；对局 URL/缓存/滚动恢复和页面独立赛季测试不回归。

- [ ] **Step 2: 运行前端全量测试和构建**

Run: `cd apps/owstats-desktop; npm test; npm run build`

Expected: 全部退出码 0；Vite 构建成功，构建产物不引用在线字体或开发机绝对路径。

- [ ] **Step 3: 检查业务页面越界**

Run:

```powershell
git diff --name-only HEAD~4..HEAD -- apps/owstats-desktop/src/pages/MatchesPage.tsx apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/src/pages/CareerMmrPage.tsx apps/owstats-desktop/src/pages/RankingPage.tsx
```

Expected: 无输出；本计划没有抢占后续三个业务页面接入切片，`OnboardingPage.tsx` 也保持不变。

- [ ] **Step 4: 运行仓库差异检查**

Run: `git diff --check; git status --short`

Expected: `git diff --check` 无错误；状态只包含执行前已有改动，Tasks 1-4 已形成四个聚焦提交。

- [ ] **Step 5: 审查提交组织**

Run: `git log -4 --oneline`

Expected: 依次看到导航行为、视觉系统、page-shell 组件、可访问性交互四个独立提交；任何一项均可单独回滚，不混入业务页面请求、后端或数据库改动。

- [ ] **Step 6: 记录未包含的后续验收**

执行报告必须明确：对局、战报、MMR、排行尚未在本计划接入 `PageHeader`/`PageSeasonSelect`/页面状态组件，`OnboardingPage.tsx` 的 `local-data` 锚点也留给最后的指挥台切片；这些工作按上方所有权表进入后续切片。若未运行 `npm run tauri build` 和安装后烟测，报告写明“未验证安装版”，不得把 `npm run build` 表述为发布验证完成。
