# CMD-DATA 数据操作紧凑布局 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将指挥台 CMD-DATA 的六项赛季数据事实压缩为宽屏单行标签，同时保持同步与数据逻辑不变。

**Architecture:** `SeasonHealthPanel.tsx` 继续提供相同的六项 `DataFact`；仅将最近同步值格式化为适合紧凑标签的月日时分。`shared.css` 将事实区从双列卡片改为六列标签，并让该行跨越同步工作区整宽；窄屏安全地切换为三列。源码级回归测试覆盖格式化、宽屏列数、整宽跨列和窄屏降级。

**Tech Stack:** React、TypeScript、CSS Grid、tsx 测试脚本。

---

### Task 1: 先锁定紧凑布局行为

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [x] **Step 1: 添加失败断言和格式化导入**

在现有导入后加入：

```ts
import { formatCompactTimestamp } from "../src/features/command-center/SeasonHealthPanel.tsx";
```

在读取 `styles` 后加入：

```ts
assertEqual(
  /\.phase8SeasonStatusStats\s*\{\s*display: grid;\s*grid-template-columns: repeat\(6, minmax\(0, 1fr\)\);/.test(styles),
  true,
  "command center data facts use one compact six-column row on wide screens",
);
assertEqual(
  /@media \(max-width: 920px\)\s*\{[\s\S]*?\.phase8SeasonStatusStats\s*\{\s*grid-template-columns: repeat\(3, minmax\(0, 1fr\)\);/.test(styles),
  true,
  "command center data facts fall back to three columns on narrow screens",
);
assertEqual(
  formatCompactTimestamp("2026-07-12T02:56:30Z"),
  "07-12 10:56",
  "compact timestamp removes only the year while preserving local month day and minute",
);
```

- [x] **Step 2: 运行断言并确认失败**

Run: `npx tsx scripts/phase6Shell.test.ts`

Expected: FAIL，因为 `formatCompactTimestamp` 尚未导出，且样式仍为双列。

### Task 2: 最小化压缩实现

**Files:**
- Modify: `apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx`
- Modify: `apps/owstats-desktop/src/styles/shared.css:2318-2354`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [x] **Step 1: 添加紧凑时间格式化函数并替换唯一调用点**

将最后一个 `DataFact` 的 `value` 改为：

```tsx
value={formatCompactTimestamp(seasonStatus.lastSyncedOrImportedAt)}
```

在 `formatTimestamp` 后新增：

```ts
export function formatCompactTimestamp(value: string | null | undefined) {
  const formatted = formatTimestamp(value);
  return /^\d{4}-\d{2}-\d{2}\s\d{2}:\d{2}$/.test(formatted) ? formatted.slice(5) : formatted;
}
```

- [x] **Step 2: 将事实区压缩为六列标签**

替换 `.phase8SeasonStatusStats` 与其三个子选择器为：

```css
.phase8SeasonStatusStats {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 7px;
}

.phase8SeasonStatusStats > span {
  min-width: 0;
  display: grid;
  align-content: center;
  gap: 3px;
  padding: 8px 7px;
  border: 1px solid rgb(219 229 240 / 0.85);
  border-radius: 9px;
  background: rgb(255 255 255 / 0.74);
}

.phase8SeasonStatusStats > span > span {
  overflow: hidden;
  color: var(--phase6-muted);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.phase8SeasonStatusStats > span > strong {
  overflow: hidden;
  color: var(--phase6-ink);
  font-size: 13px;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

在同一文件新增窄屏规则：

```css
@media (max-width: 920px) {
  .phase8SeasonStatusStats {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
```

- [x] **Step 3: 运行断言并确认通过**

Run: `npx tsx scripts/phase6Shell.test.ts`

Expected: exit code 0，且无断言错误。

### Task 3: 前端验证与工作区审计

**Files:**
- Verify only: `apps/owstats-desktop`

- [x] **Step 1: 运行前端测试**

Run: `npm test`

Expected: exit code 0。

- [x] **Step 2: 构建前端**

Run: `npm run build`

Expected: exit code 0。

- [x] **Step 3: 检查补丁与工作区**

Run: `git diff --check; git diff -- apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx apps/owstats-desktop/src/styles/shared.css apps/owstats-desktop/scripts/phase6Shell.test.ts; git status --short`

Expected: `git diff --check` 无输出；仅报告本任务的三个前端文件，以及已确认由用户或既有工作产生的其它改动。
