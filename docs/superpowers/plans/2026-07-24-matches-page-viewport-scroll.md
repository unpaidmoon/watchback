# 对局页视口滚动修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除对局页工作台下方的大面积空白和窗口最右侧的整页纵向滚动，同时保留左右面板独立滚动。

**Architecture:** 在现有对局页专属 CSS 作用域内补全画布的高度与溢出约束，不修改全局页面滚动策略。使用源代码布局契约测试锁定必要声明，再通过合成 fixture 做真实浏览器尺寸验证。

**Tech Stack:** React、TypeScript、CSS、Node.js `node:test`/`assert`、Vite

---

### Task 1: 建立对局页高度链回归契约

**Files:**
- Create: `apps/owstats-desktop/scripts/matchesViewportLayout.test.ts`
- Test: `apps/owstats-desktop/scripts/matchesViewportLayout.test.ts`

- [ ] **Step 1: 写入失败测试**

```ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const matchesCss = readFileSync(
  new URL("../src/styles/features/matches.css", import.meta.url),
  "utf8",
);

const canvasRule = matchesCss.match(
  /\.appCanvas:has\(>\s*\.matchesPage\)\s*\{([^}]*)\}/s,
)?.[1] ?? "";

assert.match(
  matchesCss,
  /html:has\(\.matchesPage\),\s*body:has\(\.matchesPage\)\s*\{[^}]*overflow:\s*hidden\s*;/s,
  "对局页应锁定根滚动，只保留工作台面板内部滚动",
);
assert.match(canvasRule, /(?:^|;)\s*height:\s*0\s*;/, "对局页画布应以 flex 剩余空间作为唯一高度来源");
assert.match(canvasRule, /overflow:\s*hidden\s*;/, "对局页画布不得把 body 撑出额外滚动范围");
assert.match(
  matchesCss,
  /\.matchesWorkbench\.phase6MatchWorkbench\s*\{[^}]*height:\s*100%\s*;/s,
  "对局工作台应占满页面网格分配的剩余高度",
);

console.log("matches viewport layout contracts passed");
```

- [ ] **Step 2: 运行测试并确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchesViewportLayout.test.ts
```

Expected: FAIL，指出 `.appCanvas:has(> .matchesPage)` 缺少 `height: 0` 或 `overflow: hidden`。

### Task 2: 补全对局页高度与溢出约束

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/matches.css:316-351`
- Test: `apps/owstats-desktop/scripts/matchesViewportLayout.test.ts`

- [ ] **Step 1: 添加最小 CSS 修复**

```css
html:has(.matchesPage),
body:has(.matchesPage) {
  overflow: hidden;
}

.appCanvas:has(> .matchesPage) {
  flex: 1;
  height: 0;
  min-height: 0;
  overflow: hidden;
}

.matchesWorkbench.phase6MatchWorkbench {
  width: 100%;
  max-width: none;
  height: 100%;
  min-height: 0;
  margin: 0;
  display: grid;
  grid-template-columns: minmax(300px, 32fr) minmax(0, 68fr);
  gap: var(--space-4);
  overflow: hidden;
}
```

- [ ] **Step 2: 运行精准测试并确认通过**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchesViewportLayout.test.ts
```

Expected: PASS，输出 `matches viewport layout contracts passed`。

### Task 3: 验证页面与工作树

**Files:**
- Verify: `apps/owstats-desktop/src/styles/features/matches.css`
- Verify: `apps/owstats-desktop/scripts/matchesViewportLayout.test.ts`

- [ ] **Step 1: 运行前端测试**

Run:

```powershell
cd apps\owstats-desktop
npm test
```

Expected: 所有前端测试通过。

- [ ] **Step 2: 运行前端构建**

Run:

```powershell
cd apps\owstats-desktop
npm run build
```

Expected: 命令退出码为 0。

- [ ] **Step 3: 使用合成 fixture 检查滚动**

在 `1920×1080` 视口打开现有对局页视觉 fixture，在页面最右侧滚动后确认：

```text
window.scrollY === 0
getComputedStyle(document.documentElement).overflow === "hidden"
getComputedStyle(document.body).overflow === "hidden"
左侧面板 scrollHeight >= clientHeight
右侧面板 scrollHeight >= clientHeight
```

Expected: 页面外层无额外纵向滚动范围，左右面板内容仍可滚动。

- [ ] **Step 4: 检查差异**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` 无输出；状态中仅新增本任务的规格、计划、测试文件以及 `matches.css` 修改，既有用户改动保持不变。
