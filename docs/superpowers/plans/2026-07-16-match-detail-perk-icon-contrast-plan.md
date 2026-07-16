# 对局详情威能图标对比度修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让普通对局详情的威能图标使用白色背景与可辨认的浅灰色线稿。

**Architecture:** 保持 `MatchTeamTable` 的 DOM 和资源解析不变，由现有 tooltip 外层提供白色承托面，透明 PNG 仅负责经过灰度处理的线稿。使用源码契约测试锁定普通对局详情覆盖，不影响角斗详情的独立槽位样式。

**Tech Stack:** React 18、TypeScript、CSS、Node `assert`/`tsx`

---

### Task 1: 调整威能图标承托面与线稿

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [x] **Step 1: Write the failing test**

断言 `.phase6PerkTooltip` 使用 `--surface-primary`，并断言 `.phase6CompactPerkIcon` 使用透明背景、`brightness(0)` 与 `0.46` 透明度。

- [x] **Step 2: Run test to verify it fails**

Run: `npx tsx scripts\matchWorkbench.test.ts`

Expected: FAIL，因为旧实现仍使用 `--action-primary` 黑底。

- [x] **Step 3: Write minimal implementation**

外层使用白色语义表面和细边框，图像使用透明背景、`brightness(0)` 与 `opacity: 0.46`，并保持 `object-fit: contain`。

- [x] **Step 4: Run targeted test to verify it passes**

Run: `npx tsx scripts\matchWorkbench.test.ts`

Expected: PASS，进程退出码为 `0`。

- [x] **Step 5: Run frontend regression verification**

Run: `npm test` and `npm run build`

Expected: 全部测试与 TypeScript/Vite 构建通过。

- [x] **Step 6: Check patch hygiene**

Run: `git diff --check` and `git status --short`

Expected: 无空白错误；状态只包含规格、计划、测试与目标样式文件。依照仓库协作规则，不自行暂存或提交。

### Task 2: 防止左侧长描述被裁剪

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [x] **Step 1: Write the failing test**

```ts
assert.match(
  matchStyleSource,
  /\.matchTeamTable \.phase6PerkTooltipPanel\s*\{[^}]*left:\s*0[^}]*max-width:\s*min\(300px,\s*60vw\)[^}]*overflow-wrap:\s*anywhere[^}]*transform:\s*translate\(0,\s*6px\)/s,
  "match detail perk tooltips should open rightward and wrap long copy inside the pane",
);
```

同时断言 hover/focus 状态使用 `transform: translate(0, 0)`。

- [x] **Step 2: Run test to verify it fails**

Run: `npx tsx scripts\matchWorkbench.test.ts`

Expected: FAIL，因为共享提示框仍居中展开并使用 `translate(-50%, ...)`。

- [x] **Step 3: Write minimal implementation**

在 `.matchTeamTable` 范围内覆盖提示框的 `left`、`max-width`、换行与 transform，并同步 hover/focus transform；不修改共享提示框和角斗槽位。

- [x] **Step 4: Run targeted and full verification**

Run: `npx tsx scripts\matchWorkbench.test.ts`, `npm test`, `npm run build`, `git diff --check`。

Expected: 定向测试、完整前端测试、构建和差异检查通过。

### Task 3: 恢复提示框标题对比度

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [x] **Step 1: Write the failing test**

断言 `.matchTeamTable .phase6CompactPlayerRow .phase6PerkTooltipPanel strong` 使用 `color: var(--action-on-primary)`。

- [x] **Step 2: Run test to verify it fails**

Run: `npx tsx scripts\matchWorkbench.test.ts`

Expected: FAIL，因为玩家行通用 `strong` 规则仍会把标题覆盖为 `--text-primary`。

- [x] **Step 3: Write minimal implementation**

增加仅作用于普通对局详情 tooltip 标题的高优先级规则，恢复现有主操作反色文字令牌。

- [x] **Step 4: Run targeted and full verification**

Run: `npx tsx scripts\matchWorkbench.test.ts`, `npm test`, `npm run build`, `git diff --check`。

Expected: 定向测试、完整前端测试、构建和差异检查通过。
