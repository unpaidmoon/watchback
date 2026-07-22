# 成就等级高光 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为成就墙的二、三级成就增加分色悬停/焦点高光，并让详情中已解锁的二、三级等级常驻对应高光。

**Architecture:** 继续使用 `ReviewAchievement.currentLevel` 作为唯一等级来源，在现有 React 标记上输出稳定的 `tier-N` 类。视觉行为集中放入成就专用的 `review.css` 覆盖层，不修改数据计算、徽章资源或共享布局。

**Tech Stack:** React 18、TypeScript、CSS、Node `assert` 契约测试

---

### Task 1: 用契约测试固定等级高光行为

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 写入失败的结构与样式断言**

在现有测试末尾增加断言，要求卡片和详情等级区输出 `tier-${...}`，并要求 `review.css` 包含：

```ts
assert.equal(cardSource.includes("tier-${achievement.currentLevel}"), true);
assert.equal(component.includes("tier-${tier}"), true);
assert.match(reviewStyles, /\.phase6AchievementCard\.tier-2:is\(:hover, :focus-within\)/);
assert.match(reviewStyles, /\.phase6AchievementCard\.tier-3:is\(:hover, :focus-within\)/);
assert.match(reviewStyles, /\.phase6AchievementBadgeStage\.tier-2\.unlocked/);
assert.match(reviewStyles, /\.phase6AchievementBadgeStage\.tier-3\.unlocked/);
assert.doesNotMatch(reviewStyles, /\.phase6AchievementCard\.tier-(?:0|1):is\(:hover, :focus-within\)/);
assert.doesNotMatch(reviewStyles, /\.phase6AchievementBadgeStage\.tier-1\.unlocked/);
```

- [ ] **Step 2: 运行测试并确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: FAIL，指出 `tier-${achievement.currentLevel}` 或二三级高光选择器尚不存在。

### Task 2: 输出卡片与详情等级类

**Files:**
- Modify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 为成就墙卡片输出当前等级类**

将卡片根节点调整为：

```tsx
<article className={`phase6AchievementCard tier-${achievement.currentLevel}`}>
```

- [ ] **Step 2: 为详情等级区输出固定等级类**

将等级区类名调整为包含 `tier-${tier}`：

```tsx
className={`phase6AchievementBadgeStage tier-${tier} ${achievement.currentLevel >= tier ? "unlocked" : "locked"} ${achievement.currentLevel === tier || (achievement.currentLevel === 0 && tier === 1) ? "active" : ""}`}
```

- [ ] **Step 3: 运行测试并确认类名断言通过、样式断言仍失败**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: FAIL，仅剩二三级高光 CSS 选择器断言失败。

### Task 3: 实现二三级分色高光

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/review.css`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 增加成就墙二三级悬停与焦点高光**

在现有卡片悬停规则之后增加仅针对 `tier-2` 和 `tier-3` 的背景与阴影。二级采用紫蓝 `rgba(126, 117, 255, ...)`，三级采用金色 `rgba(238, 190, 84, ...)`；使用径向渐变和内外阴影，不改变尺寸或位移：

```css
.reviewWorkspace .phase6AchievementCard.tier-2:is(:hover, :focus-within) { /* 紫蓝径向高光 */ }
.reviewWorkspace .phase6AchievementCard.tier-3:is(:hover, :focus-within) { /* 金色径向高光 */ }
```

- [ ] **Step 2: 增加详情已解锁二三级常驻高光**

只匹配 `.tier-2.unlocked` 与 `.tier-3.unlocked`，以对应径向渐变、内发光和徽章 `drop-shadow` 覆盖通用 `.active` 视觉；不添加一级或 locked 选择器。

- [ ] **Step 3: 增加无障碍过渡规则**

为卡片和等级区设置 200ms 的背景、阴影过渡，并在 `@media (prefers-reduced-motion: reduce)` 中将这些过渡设为 `none`。

- [ ] **Step 4: 运行定向测试并确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: PASS。

### Task 4: 回归与交付检查

**Files:**
- Verify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Verify: `apps/owstats-desktop/src/styles/features/review.css`
- Verify: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 运行完整前端测试**

Run: `cd apps/owstats-desktop; npm test`

Expected: 所有测试通过。

- [ ] **Step 2: 运行前端构建**

Run: `cd apps/owstats-desktop; npm run build`

Expected: TypeScript、Vite 构建和路径验证通过。

- [ ] **Step 3: 检查补丁格式**

Run: `git diff --check`

Expected: 无空白错误。

- [ ] **Step 4: 核对工作树范围**

Run: `git status --short`

Expected: 保留用户原有的成就墙相关改动，并只新增本计划涉及的规格、计划、测试、组件和样式修改；不暂存或提交文件。

> 本计划按仓库协作约束省略提交步骤：用户未授权暂存或提交。
