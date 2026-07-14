# 战报概览合并实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将职责与英雄、地图的完整分析并入战报概览，并保留 URL 与赛季数据流的向后兼容。

**Architecture:** 视图导航只保留 `overview`、`friends`、`achievements`。概览组件按职责与英雄、赛季摘要、地图分析的顺序组合现有展示组件；地图继续读取已存在的同赛季 `SeasonAnalysis` 状态。历史 `roles` 与 `maps` URL 值由解析函数回退至 `overview`。

**Tech Stack:** React、TypeScript、React Router、CSS Grid、Node `assert`、tsx。

---

### Task 1: 锁定合并后的导航兼容性

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewNavigation.test.ts`
- Modify: `apps/owstats-desktop/src/features/review/reviewNavigation.ts`

- [x] 先将 `roles`、`maps` 断言为解析到 `overview`，并将好友返回上下文改为 `friends`。
- [x] 运行 `npx tsx scripts/reviewNavigation.test.ts`，确认旧视图集合导致预期失败。
- [x] 将 `reviewViews` 缩减为 `overview`、`friends`、`achievements`。
- [x] 重跑导航测试，确认通过。

### Task 2: 锁定概览的内容与顺序

**Files:**
- Modify: `apps/owstats-desktop/scripts/analysisMovedToReview.test.ts`
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/review.css`

- [x] 添加源码测试，锁定三项导航、职责与英雄→赛季摘要→地图的概览顺序，以及地图跨列样式。
- [x] 运行 `npx tsx scripts/analysisMovedToReview.test.ts`，确认旧独立视图导致预期失败。
- [x] 移除独立职责/地图分支，将既有组件组合进 `ReviewOverview`；用无视觉表面的 `reviewMapAnalysis` 布局容器承载地图片段。
- [x] 重跑页面结构测试，确认通过。

### Task 3: 回归验证

**Files:**
- Verify: `apps/owstats-desktop/scripts/reviewNavigation.test.ts`
- Verify: `apps/owstats-desktop/scripts/analysisMovedToReview.test.ts`
- Verify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- Verify: `apps/owstats-desktop/src/styles/features/review.css`

- [x] 已运行目标回归测试与 `npm run build`。
- [ ] 运行完整 `npm test`、`git diff --check`，检查最终工作树范围。
