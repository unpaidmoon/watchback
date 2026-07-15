# 对局列表参考式斜切横幅 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将对局卡片改为参考图同构的斜切地图横幅与单行数据区，并移除对局时间。

**Architecture:** `MatchListCard` 负责把地图名和模式放进横幅、移除旧的模式时间行；`matches.css` 仅在列表卡片作用域内实现地图全宽背景、炭黑斜切结果前景和四列数据区；现有 `MatchPerformanceBadges` 继续提供主标签和奖牌，不显示评分。

**Tech Stack:** React、TypeScript、CSS、Node assert 脚本测试。

---

### Task 1: 锁定参考式横幅契约

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [x] **Step 1: 写入失败断言**

```ts
assert.equal(listCardSource.includes("matchListCardBannerCopy"), true);
assert.equal(listCardSource.includes("match.beginLabel"), false);
assert.match(matchStyleSource, /\.matchListCardResultBlock::after[\s\S]*clip-path/);
```

- [x] **Step 2: 运行测试确认失败**

Run: `npx tsx scripts/matchWorkbench.test.ts`

Expected: 失败于 `matchListCardBannerCopy`，因为当前卡片仍显示独立模式/时间行且没有斜切边缘。

### Task 2: 实现横幅与单行数据区

**Files:**
- Modify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [x] **Step 1: 最小组件调整**

```tsx
<span className="matchListCardBannerCopy">
  <span>{match.mapName}</span>
  <span>{match.modeLabel}</span>
</span>
```

删除 `match.beginLabel`、独立模式时间行及分隔线；保留头像、K/A/D、伤害/治疗/抵挡、标签和奖牌。

- [x] **Step 2: 最小样式调整**

```css
.matchListCardResultBlock::after {
  clip-path: polygon(0 0, 34% 0, 100% 100%, 0 100%);
}
```

让地图预览覆盖整个横幅，以 `--action-primary` 的低透明炭黑块承载胜负和比分，并把地图名、模式定位到横幅右下；底部数据区在宽屏保持头像加三组事实的一行网格。

- [x] **Step 3: 运行目标测试确认通过**

Run: `npx tsx scripts/matchWorkbench.test.ts && npx tsx scripts/matchPerformanceBadges.test.ts`

Expected: 两个脚本退出码均为 0。

### Task 3: 验证

**Files:**
- Verify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
- Verify: `apps/owstats-desktop/src/styles/features/matches.css`

- [x] **Step 1: 执行前端验证**

Run: `npm run test:resources && npm run test:unit && npm run test:components && npm run test:legacy && npm run test:text && npx tsc && npx vite build && node scripts/verifyDistPaths.mjs && git diff --check`

Expected: 全部通过；如果 `public/assets` 因用户正在运行的开发进程被锁，只记录该环境阻塞，不停止用户进程。
