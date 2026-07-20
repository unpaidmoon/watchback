# 成就详情深色陈列面 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将成就详情弹窗改为与成就墙一致的深炭灰陈列面，并用中性明度状态替代彩色边线与彩色焦点环。

**Architecture:** 仅修改 `reviewWorkspace` 作用域内的成就详情 CSS，不改变 `AchievementDialog` 的 DOM、数据或焦点管理逻辑。用现有静态视觉契约锁定深色弹窗、透明徽章/指标区、中性选中状态和无彩色焦点环。

**Tech Stack:** React 18、TypeScript、CSS、Node `assert` 静态视觉契约、Vite

---

### Task 1: 深色成就详情弹窗

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/review.css`

- [ ] **Step 1: 写入失败的视觉契约**

在 `reviewVisualPolish.test.ts` 中增加以下断言：

```ts
assert.match(reviewCss, /\.reviewWorkspace\s+\.phase6AchievementModal\s*\{[^}]*background:\s*var\(--action-primary\);[^}]*color:\s*var\(--action-on-primary\);/s);
assert.match(reviewCss, /\.reviewWorkspace\s+\.phase6AchievementBadgeStage\s*\{[^}]*border:\s*0;[^}]*border-radius:\s*0;[^}]*background:\s*transparent;/s);
assert.match(reviewCss, /\.reviewWorkspace\s+\.phase6AchievementBadgeStage\.active\s*\{[^}]*background:\s*color-mix\(in srgb, var\(--action-on-primary\) 6%, transparent\);[^}]*box-shadow:\s*none;/s);
assert.doesNotMatch(reviewCss, /\.reviewWorkspace\s+\.phase6AchievementBadgeStage\.active\s*\{[^}]*var\(--data-primary\)/s);
assert.match(reviewCss, /\.reviewWorkspace\s+\.phase6AchievementModalStats\s+\.metric\s*\{[^}]*border:\s*0;[^}]*border-radius:\s*0;[^}]*background:\s*transparent;/s);
assert.match(reviewCss, /\.reviewWorkspace\s+\.phase6IconButton:focus-visible\s*\{[^}]*outline:\s*none\s*!important;[^}]*background:\s*color-mix\(in srgb, var\(--action-on-primary\) 12%, transparent\);/s);
```

- [ ] **Step 2: 运行测试并确认失败原因正确**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewVisualPolish.test.ts
```

Expected: FAIL，原因是详情弹窗仍使用 `--surface-secondary` 浅色表面。

- [ ] **Step 3: 实现最小深色样式**

在 `review.css` 中将现有成就详情覆盖改为：

```css
.reviewWorkspace .phase6AchievementModal {
  border-color: color-mix(in srgb, var(--action-on-primary) 10%, transparent);
  background: var(--action-primary);
  box-shadow: 0 10px 28px rgba(45, 54, 48, 0.045);
  color: var(--action-on-primary);
}

.reviewWorkspace .phase6AchievementModal .phase6Eyebrow,
.reviewWorkspace .phase6AchievementModalHeader p,
.reviewWorkspace .phase6AchievementBadgeStage span,
.reviewWorkspace .phase6AchievementModalStats .metric span {
  color: color-mix(in srgb, var(--action-on-primary) 62%, transparent);
}

.reviewWorkspace .phase6AchievementModalHeader h3,
.reviewWorkspace .phase6AchievementBadgeStage strong,
.reviewWorkspace .phase6AchievementModalStats .metric strong {
  color: var(--action-on-primary);
}

.reviewWorkspace .phase6IconButton {
  width: 44px;
  height: 44px;
  border-color: color-mix(in srgb, var(--action-on-primary) 14%, transparent);
  background: transparent;
  color: var(--action-on-primary);
}

.reviewWorkspace .phase6IconButton:hover,
.reviewWorkspace .phase6IconButton:focus-visible {
  background: color-mix(in srgb, var(--action-on-primary) 12%, transparent);
}

.reviewWorkspace .phase6IconButton:focus-visible {
  outline: none !important;
  outline-offset: 0 !important;
}

.reviewWorkspace .phase6AchievementBadgeTrack,
.reviewWorkspace .phase6AchievementModalStats {
  gap: 0;
  border-top: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
  border-bottom: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
}

.reviewWorkspace .phase6AchievementBadgeStage,
.reviewWorkspace .phase6AchievementModalStats .metric {
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.reviewWorkspace .phase6AchievementBadgeStage + .phase6AchievementBadgeStage,
.reviewWorkspace .phase6AchievementModalStats .metric + .metric {
  border-left: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
}

.reviewWorkspace .phase6AchievementBadgeStage.active {
  border-color: transparent;
  background: color-mix(in srgb, var(--action-on-primary) 6%, transparent);
  box-shadow: none;
}

.reviewWorkspace .phase6AchievementModalProgress {
  background: color-mix(in srgb, var(--action-on-primary) 12%, transparent);
}
```

- [ ] **Step 4: 运行定向测试、完整测试和构建**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewVisualPolish.test.ts
npx tsx scripts/reviewAchievementWall.test.ts
npm test
npm run build
```

Expected: 全部退出码 `0`；构建路径扫描通过。

- [ ] **Step 5: 使用匿名 fixture 做浏览器验收**

检查深色弹窗、透明三级徽章区、透明指标区、中性当前等级背景、关闭按钮明度焦点、Escape 与焦点恢复；不得读取真实账号、SQLite 或凭据。

- [ ] **Step 6: 提交实现**

```powershell
git add -- apps/owstats-desktop/scripts/reviewVisualPolish.test.ts apps/owstats-desktop/src/styles/features/review.css
git commit -m "style: darken achievement details"
```
