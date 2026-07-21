# Remove Ranking Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 移除英雄排行与地图排行中的搜索功能，并始终展示当前赛季的完整排行结果。

**Architecture:** 保留排行页现有数据加载、赛季选择、分页切换与排序流程。删除页面级搜索状态、过滤分支、搜索控件及搜索专属空状态，让解析显示名后的数据直接进入排序函数。

**Tech Stack:** React 18、TypeScript、Node.js 契约测试

---

### Task 1: Remove ranking search behavior

**Files:**
- Modify: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`
- Modify: `apps/owstats-desktop/src/pages/RankingPage.tsx`

- [ ] **Step 1: Write the failing contract test**

更新工具栏断言，使分段切换后直接出现排行计数；并断言源码不再包含 `phase6RankingSearch`、搜索状态、搜索图标和过滤变量。

- [ ] **Step 2: Run test to verify it fails**

Run: `npx tsx scripts/rankingCommandCenterRedesign.test.ts`

Expected: FAIL，因为当前排行页仍渲染搜索框并保留搜索状态。

- [ ] **Step 3: Write minimal implementation**

从 `RankingPage.tsx` 删除 `Search` 导入、`search` 状态、关键字过滤和搜索框。将 `rows` 直接设为 `sortRankingRows(source, sort)`，依赖项删除 `search`；空状态固定使用赛季数据不足文案和重新加载动作；计数只显示总条目数。

- [ ] **Step 4: Run focused tests**

Run: `npx tsx scripts/rankingCommandCenterRedesign.test.ts && npx tsx scripts/phase6Shell.test.ts && npx tsx scripts/phase8MultiSeasonPages.test.ts`

Expected: PASS。

- [ ] **Step 5: Run frontend verification**

Run: `npm test`, `npm run build`, and repository-level `git diff --check`.

Expected: 全部成功，且 `git status --short` 仅新增本计划及本任务预期的两个修改文件（外加开始工作前已有的用户改动）。

不创建提交：当前工作树包含大量用户未提交改动，且用户未授权暂存或提交。
