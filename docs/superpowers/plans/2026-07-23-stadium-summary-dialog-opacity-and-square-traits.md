# 角斗汇总卡片不透明表面与正方形异能图标 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让角斗汇总卡片完全遮挡底层内容，并将四个异能槽固定为 `44px × 44px` 正方形。

**Architecture:** 保持现有 React 结构和数据流不变，只收紧 `matches.css` 的视觉契约。通过现有源码样式测试验证不透明弹窗背景、固定宽度异能列和既有圆形装备槽。

**Tech Stack:** React、TypeScript、CSS、Node.js `assert` 测试

---

### Task 1: 锁定并实现汇总卡片视觉契约

**Files:**
- Modify: `apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Verify: `docs/superpowers/specs/2026-07-23-stadium-summary-dialog-opacity-and-square-traits-design.md`

- [ ] **Step 1: 编写失败的样式契约测试**

在既有 `styles` 断言后增加：

```ts
assert.match(styles, /\.phase7SummaryDialog\s*\{[^}]*background:\s*#f4f6f0/s);
assert.match(styles, /\.phase7SummarySlots\.trait\s*\{[^}]*grid-template-columns:\s*repeat\(4,\s*44px\)/s);
assert.match(styles, /\.phase7SummaryTraitSlot,[\s\S]*?height:\s*44px/s);
assert.match(styles, /\.phase7SummaryModSlot\s*\{[^}]*border-radius:\s*999px/s);
```

- [ ] **Step 2: 运行专项测试并确认失败**

Run: `npm run test:arena-player-round-summary`

Expected: FAIL，提示 `.phase7SummaryDialog` 未使用 `#f4f6f0`，或异能列仍为 `minmax(40px, 1fr)`。

- [ ] **Step 3: 实施最小样式修改**

将弹窗背景和异能列改为：

```css
.phase7SummaryDialog {
  background: #f4f6f0;
}

.phase7SummarySlots.trait {
  grid-template-columns: repeat(4, 44px);
}
```

保留共享槽位 `height: 44px`、异能槽控制圆角和装备槽 `border-radius: 999px`。

- [ ] **Step 4: 运行专项测试并确认通过**

Run: `npm run test:arena-player-round-summary`

Expected: PASS，进程退出码为 0。

- [ ] **Step 5: 运行完整前端验证**

Run:

```powershell
npm test
npm run build
git diff --check
git status --short
```

Expected: 测试与构建退出码为 0；差异无空白错误；工作树除用户原有文档改动外，仅包含本任务测试、样式、规格与计划。

- [ ] **Step 6: 提交本任务文件**

Run:

```powershell
git add apps/owstats-desktop/scripts/arenaPlayerRoundSummary.test.tsx apps/owstats-desktop/src/styles/features/matches.css docs/superpowers/specs/2026-07-23-stadium-summary-dialog-opacity-and-square-traits-design.md docs/superpowers/plans/2026-07-23-stadium-summary-dialog-opacity-and-square-traits.md
git commit -m "fix: refine stadium summary dialog visuals"
```

Expected: 提交成功，且不包含 `docs/superpowers/specs/2026-07-23-command-center-season-update-button-design.md`。
