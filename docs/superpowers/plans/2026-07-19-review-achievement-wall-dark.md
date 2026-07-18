# 战报成就墙深色陈列面 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将战报“成就墙”改成统一深炭灰陈列面，压平八个内部成就项，让黑底徽章自然融入，同时保持详情弹窗、数据和交互不变。

**Architecture:** 仅在战报页面样式作用域 `.reviewWorkspace` 内覆盖共享 Phase 6 成就样式；`PersonalReviewPage.tsx` 的 DOM、成就计算和弹窗逻辑不变。用 `reviewVisualPolish.test.ts` 锁定深色外层、透明成就项、细分隔线、深色可读文本、冷蓝绿色进度和浅色弹窗，避免影响其他复用共享样式的页面。

**Tech Stack:** React 18、TypeScript、CSS、Node `assert` 静态视觉契约、Vite

---

## 文件结构

- Modify: `apps/owstats-desktop/src/styles/features/review.css` — 战报页面专属的深色成就墙、内部压平、响应式分隔线和深色状态样式。
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts` — 深色成就墙的静态视觉回归契约。
- Preserve: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx` — 不改 DOM、数据、交互和弹窗逻辑。
- Preserve: `apps/owstats-desktop/src/styles/shared.css` — 不改变其他页面仍可能使用的共享 Phase 6 成就样式。

### Task 1: 锁定并实现深色外层与压平成就项

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts:27-42`
- Modify: `apps/owstats-desktop/src/styles/features/review.css:167-229`

- [ ] **Step 1: 将浅色成就墙断言改成 C 方案断言**

把 `reviewVisualPolish.test.ts` 中“一级玻璃表面”和“次级玻璃表面”两条断言替换为：

```ts
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6GlassPanelShared\.solid\.phase6AchievementPanel\s*\{[^}]*background:\s*var\(--action-primary\);[^}]*color:\s*var\(--action-on-primary\);/s,
  "成就墙应使用统一深炭黑陈列面",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementCard\s*\{[^}]*border:\s*0;[^}]*border-radius:\s*0;[^}]*background:\s*transparent;[^}]*box-shadow:\s*none;/s,
  "成就项应在深色墙内压平而不是继续使用独立卡片",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementWall\s*\{[^}]*gap:\s*0;[^}]*border-top:\s*1px solid color-mix\([^;]+;[^}]*border-bottom:\s*1px solid color-mix\([^;]+;/s,
  "成就墙应使用连续细分隔线组织成就项",
);
```

- [ ] **Step 2: 运行定向测试并确认先失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewVisualPolish.test.ts
```

Expected: FAIL，错误消息至少包含“成就墙应使用统一深炭黑陈列面”。

- [ ] **Step 3: 实现深色外层、连续网格和透明成就项**

在 `review.css` 的现有成就墙覆盖区中，将外层、标题和成就项规则改为：

```css
.reviewWorkspace .phase6GlassPanelShared.solid.phase6AchievementPanel {
  border-color: color-mix(in srgb, var(--action-on-primary) 10%, transparent);
  background: var(--action-primary);
  box-shadow: 0 10px 28px rgba(45, 54, 48, 0.045);
  color: var(--action-on-primary);
}

.reviewWorkspace .phase6AchievementHeader h3 {
  color: var(--action-on-primary);
  font-weight: 400;
  text-shadow: none;
}

.reviewWorkspace .phase6AchievementWall {
  gap: 0;
  border-top: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
  border-bottom: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
}

.reviewWorkspace .phase6AchievementCard {
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.reviewWorkspace .phase6AchievementCard:nth-child(4n + 2),
.reviewWorkspace .phase6AchievementCard:nth-child(4n + 3),
.reviewWorkspace .phase6AchievementCard:nth-child(4n + 4) {
  border-left: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
}

.reviewWorkspace .phase6AchievementCard:nth-child(n + 5) {
  border-top: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
}
```

- [ ] **Step 4: 运行定向测试并确认通过**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewVisualPolish.test.ts
```

Expected: PASS，无输出且退出码为 `0`。

- [ ] **Step 5: 提交外层与压平网格**

```powershell
git add -- apps/owstats-desktop/scripts/reviewVisualPolish.test.ts apps/owstats-desktop/src/styles/features/review.css
git commit -m "style: darken review achievement wall"
```

### Task 2: 锁定并实现深色文字、状态与两列分隔线

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts:27-55`
- Modify: `apps/owstats-desktop/src/styles/features/review.css:185-288`

- [ ] **Step 1: 添加文字、链接、焦点和响应式分隔线断言**

在成就墙断言之后添加：

```ts
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementCopy\s+strong\s*\{[^}]*color:\s*var\(--action-on-primary\);/s,
  "成就名称应在深色墙上使用高对比浅色文字",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementCard\s+\.phase6EvidenceLink\s*\{[^}]*background:\s*color-mix\([^;]+;[^}]*color:\s*var\(--action-on-primary\);/s,
  "成就证据链接应适配深色背景",
);
assert.match(
  reviewCss,
  /\.reviewWorkspace\s+\.phase6AchievementCard:focus-within\s*\{[^}]*outline:\s*2px solid var\(--data-primary\);/s,
  "成就项应保留可见键盘焦点",
);
assert.match(
  reviewCss,
  /@media\s*\(max-width:\s*1080px\)[\s\S]*?\.reviewWorkspace\s+\.phase6AchievementCard:nth-child\(2n\)\s*\{[^}]*border-left:\s*1px solid color-mix\(/s,
  "两列成就墙应重新计算纵向分隔线",
);
```

- [ ] **Step 2: 运行定向测试并确认新增断言失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewVisualPolish.test.ts
```

Expected: FAIL，首先缺失的深色文字、证据链接、焦点或两列分隔线断言报告明确失败消息。

- [ ] **Step 3: 实现深色内容层级和克制交互**

将 `review.css` 中现有成就文字、徽章悬停、进度和等级覆盖改为以下规则；成就详情弹窗规则保持原样：

```css
.reviewWorkspace .phase6AchievementCard:hover {
  border-color: transparent;
  background: color-mix(in srgb, var(--action-on-primary) 4%, transparent);
  box-shadow: none;
  transform: translateY(-1px);
}

.reviewWorkspace .phase6AchievementCard:focus-within {
  border-color: transparent;
  background: color-mix(in srgb, var(--action-on-primary) 4%, transparent);
  box-shadow: none;
  outline: 2px solid var(--data-primary);
  outline-offset: -2px;
}

.reviewWorkspace .phase6AchievementMedalImage {
  filter: drop-shadow(0 12px 18px rgba(0, 0, 0, 0.28));
}

.reviewWorkspace .phase6AchievementCard:hover .phase6AchievementMedalImage,
.reviewWorkspace .phase6AchievementCard:focus-within .phase6AchievementMedalImage {
  filter: drop-shadow(0 14px 20px rgba(0, 0, 0, 0.34));
}

.reviewWorkspace .phase6AchievementCopy strong {
  color: var(--action-on-primary);
}

.reviewWorkspace .phase6AchievementCopy strong span {
  border-color: color-mix(in srgb, var(--action-on-primary) 16%, transparent);
  color: color-mix(in srgb, var(--action-on-primary) 72%, transparent);
}

.reviewWorkspace .phase6AchievementCopy > span {
  color: color-mix(in srgb, var(--action-on-primary) 82%, transparent);
}

.reviewWorkspace .phase6AchievementCopy small,
.reviewWorkspace .phase6AchievementNext {
  color: color-mix(in srgb, var(--action-on-primary) 62%, transparent);
}

.reviewWorkspace .phase6AchievementProgress {
  background: color-mix(in srgb, var(--action-on-primary) 12%, transparent);
}

.reviewWorkspace .phase6AchievementProgress span,
.reviewWorkspace .phase6AchievementModalProgress span {
  background: var(--data-secondary);
}

.reviewWorkspace .phase6AchievementTierRail span {
  border-color: color-mix(in srgb, var(--action-on-primary) 14%, transparent);
  color: color-mix(in srgb, var(--action-on-primary) 54%, transparent);
}

.reviewWorkspace .phase6AchievementTierRail span.unlocked {
  border-color: color-mix(in srgb, var(--action-on-primary) 24%, transparent);
  color: color-mix(in srgb, var(--action-on-primary) 78%, transparent);
}

.reviewWorkspace .phase6AchievementCard .phase6EvidenceLink {
  border-color: color-mix(in srgb, var(--action-on-primary) 16%, transparent);
  background: color-mix(in srgb, var(--action-on-primary) 8%, transparent);
  color: var(--action-on-primary);
}
```

- [ ] **Step 4: 在 `1080px` 断点重排分隔线**

在 `review.css` 的 `@media (max-width: 899px)` 之前新增：

```css
@media (max-width: 1080px) {
  .reviewWorkspace .phase6AchievementCard:nth-child(n) {
    border-left: 0;
    border-top: 0;
  }

  .reviewWorkspace .phase6AchievementCard:nth-child(2n) {
    border-left: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
  }

  .reviewWorkspace .phase6AchievementCard:nth-child(n + 3) {
    border-top: 1px solid color-mix(in srgb, var(--action-on-primary) 10%, transparent);
  }
}
```

- [ ] **Step 5: 运行成就墙相关测试并确认通过**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewVisualPolish.test.ts
npx tsx scripts/reviewAchievementWall.test.ts
npx tsx scripts/reviewTrimmedSections.test.ts
```

Expected: 三条命令均退出码 `0`；详情弹窗浅色断言、锁定徽章滤镜断言和成就墙结构断言继续通过。

- [ ] **Step 6: 提交深色内容与响应式细节**

```powershell
git add -- apps/owstats-desktop/scripts/reviewVisualPolish.test.ts apps/owstats-desktop/src/styles/features/review.css
git commit -m "style: refine dark achievement display"
```

### Task 3: 完整验证与视觉验收

**Files:**
- Verify: `apps/owstats-desktop/src/styles/features/review.css`
- Verify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`

- [ ] **Step 1: 运行完整前端测试**

Run:

```powershell
cd apps\owstats-desktop
npm test
```

Expected: `test:resources`、`test:unit`、`test:components`、`test:legacy`、`test:text` 全部通过。

- [ ] **Step 2: 运行生产构建**

Run:

```powershell
cd apps\owstats-desktop
npm run build
```

Expected: TypeScript、Vite 构建和 `verifyDistPaths.mjs` 全部通过，无资源路径错误。

- [ ] **Step 3: 使用安全数据进行三档视口视觉验收**

启动现有开发应用或安全视觉 fixture，仅使用匿名 fixture / 安全本地数据，依次检查：

```text
2540×1344：四列两行；第 5 项无左边线；第二行有连续顶边线。
1920×1080：四列两行；徽章黑底融入深炭灰墙面；文字和进度清晰。
1440×900：两列四行；偶数项有左边线；第 3 项起有顶边线；无横向滚动。
```

同时用键盘 Tab 检查每个成就项焦点可见，打开成就详情确认弹窗仍为浅色，并检查未激活徽章仍显示“未激活”及灰度状态。

- [ ] **Step 4: 检查差异和工作树边界**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` 无输出；本任务只涉及 `review.css` 与 `reviewVisualPolish.test.ts`，其他已有修改保持原样且未被暂存或提交。交付时报告实际命令、结果、未验证项，并明确说明未接触真实隐私数据。
