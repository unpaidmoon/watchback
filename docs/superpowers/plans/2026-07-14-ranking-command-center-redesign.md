# Ranking and Command Center Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不改变排行功能合同和当前账号同步语义的前提下，统一排行 PageHeader 与紧凑表格视觉，并把指挥台重组为导航末位、同步优先、账号健康与工具分层清晰的页面。

**Architecture:** 复用应用外壳切片已经提供的轻量 `PageHeader`、`PageSeasonSelect` 和页面状态组件，排行只移动现有控件和覆盖 feature CSS，不改请求、搜索、排序或聚合。指挥台继续复用 `getOnboardingStatus`、`SeasonStatusCard`、登录和单赛季同步命令，将现有内容拆成顶部指标、同步主栏、账号健康侧栏和底部工具区。BattleTag 自动绑定属于独立授权边界：执行前只读比较目标分支；任何合并类操作先获得用户明确许可，检测未形成端到端绑定时保留手工入口和全部身份同步语义。

**Tech Stack:** React 18、TypeScript、React Router 6、Tauri 2、Node/tsx 源码契约测试、CSS、Lucide React。

---

## 硬授权 Gate

本 Gate 必须在 Task 1 之前执行，也必须在任何删除 BattleTag 手工入口的建议之前重新执行。

- [ ] **Gate 1: 确认工作树和分支引用**

Run:

```powershell
git status --short
git rev-parse HEAD
git rev-parse codex/login-battletag-auto-binding
$base = git merge-base HEAD codex/login-battletag-auto-binding
git diff --name-status "$base..codex/login-battletag-auto-binding"
```

Expected at plan authoring baseline: 当前工作树只包含本轮计划文档；目标分支只修改 `credentials.rs`、`dashen_auth.rs`、`phase4_login_redesign.rs`。

- [ ] **Gate 2: 只读核对自动绑定是否端到端完成**

Run:

```powershell
git grep -n "start_dashen_login_capture\|detected_bnet\|bind_player_target\|auto.*bind" codex/login-battletag-auto-binding -- apps/owstats-desktop
git diff --unified=20 HEAD..codex/login-battletag-auto-binding -- apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/player_target.rs apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/shared/api.ts apps/owstats-desktop/src/shared/types.ts
```

Expected at plan authoring baseline: `credentials.rs` 产生 `BrowserLoginOutcome.detected_bnet`，但公开 `start_dashen_login` 仍只返回 credential status；目标分支没有调用 `bind_player_target`，没有更新 `PlayerTargetStatus`，也没有修改 Onboarding 前端。

- [ ] **Gate 3: 按授权矩阵决定范围**

| 只读结果 | 本计划允许动作 |
|---|---|
| 仍是“只检测、不绑定” | 保留 `battleTag`、`runBindTarget`、`runClearTarget`、`bindPlayerTarget`、`clearPlayerTarget` 和目标状态；只重排视觉位置。 |
| 目标分支新增端到端自动绑定，但当前分支尚未包含 | 停止执行并询问用户是否允许 merge、rebase 或 cherry-pick；未获许可不得运行任何一种操作。 |
| 用户已在外部完成集成 | 重新运行相关 Rust 与前端测试，以当前已集成合同为准；本计划仍不自行改变身份同步语义。 |
| 分支引用不存在或差异无法解释 | 停止执行并向用户报告精确命令输出，不猜测绑定状态。 |

禁止在本计划执行中自行运行 `git merge`、`git rebase`、`git cherry-pick` 或 `git switch`。没有端到端绑定证据时，不得删除手工入口，不得把 `credentialReady` 当作 `targetReady`，不得改变 `syncSeason` 的目标来源。

## 文件职责

- `apps/owstats-desktop/src/shared/page-shell/PageHeader.tsx`、`PageSeasonSelect.tsx`、`PageState.tsx`：复用应用外壳切片的页面框架和状态组件，不在本计划重复定义。
- `apps/owstats-desktop/src/styles/page-shell.css`：复用应用外壳切片样式，不把 feature 规则回填到 `shared.css`。
- `apps/owstats-desktop/src/pages/RankingPage.tsx`：只重排标题、赛季、tab、搜索和计数；排行逻辑保持原样。
- `apps/owstats-desktop/src/styles/features/ranking.css`：排行紧凑表格、浅灰绿色表头和深炭分段控件覆盖。
- `apps/owstats-desktop/src/pages/OnboardingPage.tsx`：指挥台页面协调器；不接管或重写登录/绑定/同步后端语义。
- `apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx`：同步按钮 disabled/busy/progress 无障碍状态。
- `apps/owstats-desktop/src/styles/features/command-center.css`：指挥台 2/3 + 1/3 布局、指标、工具和危险操作区。
- `apps/owstats-desktop/src/shared/phase6Shell.ts`、`src/App.tsx`：只作为导航顺序和默认路由的守护合同，不在本计划再次修改。
- `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`：新增本轮结构合同。
- `apps/owstats-desktop/scripts/phase6Shell.test.ts`、`phase8RealFetch.test.ts`、`phase8MultiSeasonPages.test.ts`、`rankingUtils.test.ts`、`matchesPageCache.test.ts`：保留并更新既有功能合同。

### Task 1: 固定 PageHeader 复用和排行不可变合同

**Files:**
- Create: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`
- Modify: `apps/owstats-desktop/package.json:18-20`

- [ ] **Step 1: 写导航和 PageHeader 失败测试**

```ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { phase6NavItems } from "../src/shared/phase6Shell";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const rankingSource = readFileSync(new URL("../src/pages/RankingPage.tsx", import.meta.url), "utf8");

assert.equal(phase6NavItems.map((item) => item.label).join(","), "对局,战报,生涯 MMR,排行,指挥台");
assert.match(appSource, /path="\/" element=\{<Navigate to="\/matches" replace \/>\}/);
assert.match(rankingSource, /<PageHeader/);
assert.match(rankingSource, /<PageSeasonSelect/);

for (const contract of [
  /getRankings\(selectedSeason\)/,
  /const heroSortKeys: RankingSortKey\[\] = \["matches", "winRate", "playTimeHours", "kda"\]/,
  /const mapSortKeys: RankingSortKey\[\] = \["matches", "winRate", "playTimeHours", "kda"\]/,
  /sortRankingRows\(filtered, sort\)/,
  /ranking\?\.availableSeasons/,
  /nextParams\.set\("season", String\(nextSeason\)\)/,
]) assert.match(rankingSource, contract);
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts`

Expected: FAIL，报告排行尚未复用统一 `PageHeader` 或 `PageSeasonSelect`；应用外壳的导航与默认路由守护断言保持通过。

- [ ] **Step 3: 只登记测试入口，不重复实现应用外壳**

将 `rankingCommandCenterRedesign.test.ts` 加入 `package.json` 的 `test:legacy`。此时保留 RED：不得为了让测试提前通过而在排行切片之外修改 `RankingPage`，也不得重写已经由应用外壳切片提供的 `PageHeader`、导航或 `/` 路由。

- [ ] **Step 4: 重跑并确认 RED 已收敛**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts`

Expected: FAIL，且只剩排行尚未接入 `PageHeader` / `PageSeasonSelect` 的预期失败；不可变排行合同与应用外壳守护合同通过。

- [ ] **Step 5: 不提交 RED 测试**

Task 1 只建立 TDD 红灯；测试和 `package.json` 与 Task 2 的最小实现一起提交，避免提交一个故意失败的独立状态。

### Task 2: 只统一排行页面框架和表格视觉

**Files:**
- Modify: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts:129-131`
- Modify: `apps/owstats-desktop/package.json:18-20`
- Modify: `apps/owstats-desktop/src/pages/RankingPage.tsx:140-185`
- Modify: `apps/owstats-desktop/src/styles/features/ranking.css`

- [ ] **Step 1: 写排行结构和样式失败测试**

```ts
const rankingCss = readFileSync(new URL("../src/styles/features/ranking.css", import.meta.url), "utf8");
assert.match(rankingSource, /title="英雄与地图排行"/);
assert.match(rankingSource, /<PageHeader[\s\S]*actions=\{\([\s\S]*<PageSeasonSelect/);
assert.match(rankingSource, /className="rankingToolbar phase6RankingToolbar"[\s\S]*phase6SegmentedControl[\s\S]*phase6RankingSearch[\s\S]*phase6RankingCount/);
assert.doesNotMatch(rankingSource, /className="rankingToolbar phase6RankingToolbar"[\s\S]*phase8SeasonSelect[\s\S]*<\/div>/);
assert.match(rankingCss, /\.phase6RankingHead\s*\{[^}]*background:\s*var\(--surface-primary\)/s);
assert.match(rankingCss, /\.phase6RankingLine\s*\{[^}]*min-height:\s*52px/s);
assert.match(rankingCss, /\.phase6RankingLine\s*\{[^}]*box-shadow:\s*none/s);
assert.match(rankingCss, /\.phase6SegmentedControl button\.active\s*\{[^}]*background:\s*var\(--action-primary\)/s);
assert.match(rankingCss, /\.phase6RankingHead button\.active\s*\{[^}]*color:\s*var\(--data-primary\)/s);
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts`

Expected: FAIL，报告排行缺少统一 PageHeader 或 `ranking.css` 没有浅灰绿色表头覆盖。

- [ ] **Step 3: 重排现有控件并写 feature CSS 覆盖**

只调整 JSX 位置：

```tsx
<PageHeader
  title="英雄与地图排行"
  description={tab === "heroes" ? "按本地赛季汇总英雄表现" : "按本地赛季汇总地图表现"}
  actions={(
    <PageSeasonSelect
      value={selectedSeason}
      seasons={availableSeasons}
      disabled={availableSeasons.length === 0}
      onChange={(season) => season !== undefined && changeSeason(season)}
    />
  )}
/>

<div className="rankingToolbar phase6RankingToolbar">
  <div className="segmentedControl phase6SegmentedControl">{/* 保持两个现有 tab button */}</div>
  <label className="searchBox phase6RankingSearch">{/* 保持现有搜索 input */}</label>
  <span className="phase6RankingCount">{sourceCount} 项 · 当前 {rows.length} 项</span>
</div>
```

不得修改 `getRankings`、`useMemo` 搜索、`switchTab`、`changeSeason`、`changeSort`、`RankingTable` 数据字段或排序键。

在 `ranking.css` 写覆盖，不迁移 `shared.css` 旧声明：

```css
.phase6RankingToolbar { align-items: center; padding: 10px 0; border: 0; border-radius: 0; background: transparent; box-shadow: none; backdrop-filter: none; }
.phase6SegmentedControl button.active { background: var(--action-primary); color: var(--action-on-primary); }
.phase6RankingTable { gap: 0; overflow: hidden; border: 1px solid var(--border-subtle); border-radius: var(--radius-control); background: var(--surface-secondary); }
.phase6RankingHead, .phase6RankingLine { border-width: 0 0 1px; border-radius: 0; }
.phase6RankingHead { min-height: 44px; background: var(--surface-primary); color: var(--text-primary); }
.phase6RankingHead button.active { color: var(--data-primary); }
.phase6RankingLine { min-height: 52px; background: var(--surface-secondary); box-shadow: none; }
.phase6RankingLine:last-child { border-bottom: 0; }
```

将 `phase6Shell.test.ts` 的“排行移除 header card”断言改为“排行使用轻量 `PageHeader`，且不恢复旧厚卡类”。

- [ ] **Step 4: 运行功能不变验证**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts; npx tsx scripts/rankingUtils.test.ts; npx tsx scripts/rankingDisplayNames.test.ts; npx tsx scripts/phase8MultiSeasonPages.test.ts`

Expected: 四个脚本全部 PASS；英雄/地图默认排序、搜索、评分、动态赛季和 URL 合同无变化。

- [ ] **Step 5: 提交排行视觉切片**

```powershell
git add -- apps/owstats-desktop/src/pages/RankingPage.tsx apps/owstats-desktop/src/styles/features/ranking.css apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/package.json
git commit -m "style: unify ranking page layout"
```

### Task 3: 把指挥台重组为同步主栏和账号健康侧栏

**Files:**
- Modify: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts:64-132`
- Modify: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx:41-395`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`

- [ ] **Step 1: 写指挥台分区失败测试**

```ts
const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
const commandCss = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");

assert.match(onboardingSource, /title="指挥台"/);
assert.match(onboardingSource, /className="commandCenterMetrics"/);
assert.match(onboardingSource, /className="commandCenterMain"/);
assert.match(onboardingSource, /className="commandCenterSyncColumn"/);
assert.match(onboardingSource, /className="commandCenterAccountColumn"/);
assert.match(onboardingSource, /className="commandCenterTools"/);
assert.match(onboardingSource, /className="commandCenterDangerZone"/);
assert.match(onboardingSource, /id="local-data"/);
assert.doesNotMatch(onboardingSource, /getPersonalSeasonReview/);
assert.doesNotMatch(onboardingSource, /常用英雄|成就展示|phase6ProfileShowcase/);
assert.match(onboardingSource, /const commandCenterSeasons = \[21, 22, 23\] as const/);
assert.match(commandCss, /grid-template-columns:\s*minmax\(0, 2fr\) minmax\(280px, 1fr\)/);
```

若硬授权 Gate 的结果仍为“只检测、不绑定”，同时保留：

```ts
for (const contract of [/runBindTarget/, /runClearTarget/, /bindPlayerTarget/, /clearPlayerTarget/, /targetReady/]) {
  assert.match(onboardingSource, contract);
}
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts`

Expected: FAIL，报告缺少 `commandCenterMain`，并命中当前首屏英雄/成就代码。

- [ ] **Step 3: 删除首屏英雄成就请求并重组 JSX**

从 `OnboardingPage.tsx` 删除 `getPersonalSeasonReview`、achievement asset、`HeroAvatar`、review achievement helper、`PROFILE_REVIEW_SEASON`、`review` state 和展示组件；`DashboardSnapshot` 只保留 `status`、`profile`、`targetStatus`。

固定支持赛季并从现有状态生成三张卡：

```ts
const commandCenterSeasons = [21, 22, 23] as const;
const seasonStatuses = commandCenterSeasons.map((season) =>
  status?.seasonStatuses.find((item) => item.season === season) ?? {
    season,
    matchCount: 0,
    detailCount: 0,
    normalMatchCount: 0,
    arenaMatchCount: 0,
    failedDetailCount: 0,
    lastSyncedOrImportedAt: null,
    coveragePercent: 0,
  },
);
```

页面结构改为：

```tsx
<PageHeader
  title="指挥台"
  description="管理当前账号、本地赛季数据和桌面工具"
  actions={(
    <>
      <button className="ghostButton" type="button" onClick={runUpdateCheck} disabled={checkingUpdate}>检查更新</button>
      <button className="primaryButton" type="button" onClick={() => syncSectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" })}>同步数据</button>
    </>
  )}
/>

<section className="commandCenterMetrics" aria-label="指挥台摘要">{/* 登录状态、本地对局、覆盖率、最近同步 */}</section>

<div className="commandCenterMain">
  <main id="local-data" tabIndex={-1} ref={syncSectionRef} className="commandCenterSyncColumn">
    <section className="syncTargetStrip">{/* Gate 要求保留时，原样保留手工目标入口 */}</section>
    <section className="phase8SeasonStatusGrid">{/* 渲染 S21/S22/S23 */}</section>
  </main>
  <aside className="commandCenterAccountColumn">
    <AccountSummary profile={profile} credentialReady={credentialReady} />
    <DataHealthSummary status={status} coveragePercent={coveragePercent} />
    <div className="commandCenterAccountActions">{/* 登录/重新登录；取消登录 */}</div>
    <div className="commandCenterDangerZone">{/* 仅退出登录 */}</div>
  </aside>
</div>

<footer className="commandCenterTools" aria-label="桌面工具">{/* 日志、数据路径、版本、GitHub */}</footer>
```

手工 BattleTag 区在 Gate 要求保留时只能移动，不得改变 `credentialReady && targetReady` 同步门槛。每张 `SeasonStatusCard` 继续调用 `runSeasonSync(seasonStatus.season)`，不得新增“同步全部”。

在 `command-center.css` 增加：

```css
.commandCenterMetrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); border-block: 1px solid var(--border-subtle); }
.commandCenterMain { display: grid; grid-template-columns: minmax(0, 2fr) minmax(280px, 1fr); gap: 18px; align-items: start; }
.commandCenterSyncColumn, .commandCenterAccountColumn { display: grid; gap: 14px; }
.commandCenterAccountColumn { position: sticky; top: 16px; }
.commandCenterTools { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; padding-top: 16px; border-top: 1px solid var(--border-subtle); }
.commandCenterDangerZone { padding-top: 14px; border-top: 1px solid color-mix(in srgb, var(--status-danger) 28%, var(--border-subtle)); }
@media (max-width: 920px) {
  .commandCenterMain { grid-template-columns: 1fr; }
  .commandCenterAccountColumn { position: static; }
  .commandCenterMetrics, .commandCenterTools { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
```

更新 `phase6Shell.test.ts`，移除要求英雄、成就、任意 `seasonInput` 和单体 `phase6UnifiedCommandPanel` 的旧断言。更新 `phase8RealFetch.test.ts` 为固定 S21/S22/S23、逐卡 `syncSeason` 和无“同步全部”的合同。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts; npx tsx scripts/phase6Shell.test.ts; npx tsx scripts/phase8RealFetch.test.ts; npx tsx scripts/matchesPageCache.test.ts`

Expected: 四个脚本全部 PASS；同步完成后仍调用 `clearMatchesPageCache()`，登录/目标门槛和单赛季同步不回归。

- [ ] **Step 5: 提交指挥台结构切片**

```powershell
git add -- apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/scripts/phase8RealFetch.test.ts
git commit -m "feat: reorganize command center workspace"
```

### Task 4: 统一指挥台加载、成功、错误、禁用和焦点状态

**Files:**
- Modify: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx:17-66`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`

- [ ] **Step 1: 写 aria 状态失败测试**

```ts
const seasonHealthSource = readFileSync(new URL("../src/features/command-center/SeasonHealthPanel.tsx", import.meta.url), "utf8");
assert.match(onboardingSource, /<PageErrorState/);
assert.match(onboardingSource, /<PageLoadingState/);
assert.match(onboardingSource, /<StatusPill/);
assert.match(onboardingSource, /aria-busy=\{checkingUpdate\}/);
assert.match(seasonHealthSource, /aria-busy=\{isSyncing\}/);
assert.match(seasonHealthSource, /aria-live="polite"/);
assert.match(seasonHealthSource, /disabled=\{!credentialReady \|\| isSyncBlocked \|\| isSyncing\}/);
assert.match(onboardingSource, /className="commandCenterDangerZone"[\s\S]*dangerButton/);
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts`

Expected: FAIL，报告状态横幅缺少动态 `role`/`aria-live` 或更新按钮缺少 `aria-busy`。

- [ ] **Step 3: 实现无障碍状态，不改变业务调用**

错误、首次加载和简短账号/同步状态分别复用应用外壳切片的 `PageErrorState`、`PageLoadingState` 和 `StatusPill`。保留现有可恢复错误的重试动作，例如：

```tsx
{error ? <PageErrorState message={error} onRetry={() => void refreshDashboard()} /> : null}
{loading && !status ? <PageLoadingState message="正在读取本地账号与赛季状态…" /> : null}
<StatusPill tone={credentialReady ? "success" : "neutral"}>
  {credentialReady ? "账号已登录" : "等待登录"}
</StatusPill>
```

检查更新按钮增加 `aria-busy={checkingUpdate}`；登录按钮增加 `aria-busy={loggingIn}`。`SeasonStatusCard` 的 footer 增加 `aria-live="polite"`，同步按钮增加 `aria-busy={isSyncing}`，保留现有 disabled 表达式。所有焦点轮廓使用 `:focus-visible`，不得在 CSS 中设置 `outline: 0` 覆盖键盘焦点。

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts; npx tsx scripts/phase6Shell.test.ts; npx tsx scripts/matchesPageCache.test.ts`

Expected: 三个脚本全部 PASS；状态文本为中文，异步按钮禁用逻辑不变。

- [ ] **Step 5: 提交状态反馈切片**

```powershell
git add -- apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts
git commit -m "fix: expose command center async states"
```

### Task 5: 完整验证排行与指挥台切片

**Files:**
- Verify: 本计划列出的全部文件

- [ ] **Step 1: 再次执行硬授权 Gate**

Run: 重复本计划“硬授权 Gate”的 Gate 1 与 Gate 2 命令。

Expected: 若分支差异仍是“只检测、不绑定”，最终差异必须仍保留手工目标入口；若结果发生变化，停止验证并先请求用户授权，不执行合并类操作。

- [ ] **Step 2: 运行聚焦测试**

Run: `cd apps/owstats-desktop; npx tsx scripts/rankingCommandCenterRedesign.test.ts; npx tsx scripts/rankingUtils.test.ts; npx tsx scripts/rankingDisplayNames.test.ts; npx tsx scripts/phase8MultiSeasonPages.test.ts; npx tsx scripts/phase6Shell.test.ts; npx tsx scripts/phase8RealFetch.test.ts; npx tsx scripts/matchesPageCache.test.ts`

Expected: 七个脚本全部 PASS。

- [ ] **Step 3: 运行前端完整验证**

Run: `cd apps/owstats-desktop; npm test; npm run build`

Expected: 两条命令退出码均为 0；资源准备、TypeScript、Vite 和构建产物路径扫描通过。

- [ ] **Step 4: 运行仓库验证和浏览器烟测**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -ChangedOnly`

Expected: 敏感信息扫描、测试、构建和 `git diff --check` 通过。随后用脱敏 Tauri mock 检查 `/ranking?season=23&tab=maps` 和 `/onboarding` 的 1440px、920px、760px 布局；确认表格紧凑、指挥台主栏/侧栏降级正确、键盘焦点可见、同步按钮 disabled/busy 清晰。不得连接真实桌面后端。

- [ ] **Step 5: 审查最终差异，不创建空泛验证提交**

Run: `git diff --check; git status --short`

Expected: `git diff --check` 无错误；状态只包含本计划列出的实现与测试文件，且没有 BattleTag 自动绑定分支的未授权提交。验证本身不制造空提交，也不宽泛暂存 `apps/owstats-desktop`。
