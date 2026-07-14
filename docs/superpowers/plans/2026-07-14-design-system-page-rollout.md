# Design System Page Rollout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将经用户验收的指挥台玻璃画布、低层级卡片和中性文字规范，推广到战报、排行、生涯 MMR、对局及好友详情页面，同时保持所有既有数据、筛选和可访问性行为。

**Architecture:** `design.md` 是本轮全部视觉决策的唯一规范。每个页面仅在自身 feature CSS 作用域内覆盖旧的高饱和文案、厚边框、叠层卡片和过强阴影；React 组件的数据请求、路由、赛季 URL、缓存和交互不变。每个页面先扩展已有源代码契约测试以表达新的 CSS 规则，再做最小样式改动并独立提交。

**Tech Stack:** React 18、TypeScript、CSS、Node `tsx` 源码契约测试、Vite、Tauri。

---

## File map

- `design.md`：全项目视觉令牌、排版、间距和组件层级的控制规范；只读取，不在本轮重写。
- `apps/owstats-desktop/src/styles/features/review.css`：战报页及页内标签的视觉作用域。
- `apps/owstats-desktop/src/styles/features/friends.css`：好友详情和战报跳转落点的视觉作用域。
- `apps/owstats-desktop/src/styles/features/ranking.css`：排行工具栏、表头、表格行的视觉作用域。
- `apps/owstats-desktop/src/styles/features/career-mmr.css`：MMR 摘要、图表、分解模块的视觉作用域；图表数据线例外保留紫/冷蓝绿。
- `apps/owstats-desktop/src/styles/features/matches.css`：对局筛选、列表、主从详情和角斗嵌入详情的视觉作用域。
- `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`：战报/好友视觉契约。
- `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`：排行结构和视觉契约。
- `apps/owstats-desktop/scripts/careerMmrUi.test.ts`：MMR 图表语义、可访问性和视觉令牌契约。
- `apps/owstats-desktop/scripts/matchWorkbench.test.ts`：对局工作台、普通详情和角斗详情契约。

### Task 1: 战报与好友详情采用低层级玻璃表面

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/review.css`
- Modify: `apps/owstats-desktop/src/styles/features/friends.css`

- [ ] **Step 1: 写入失败的视觉契约**

在 `reviewVisualPolish.test.ts` 添加断言，要求 `.reviewWorkspace` 和 `.friendDetailPage` 的眉题使用 `var(--text-secondary)` 而非数据紫；要求战报的一级面板使用 `var(--glass-primary)`、低对比 `var(--glass-border)` 且无盒阴影；要求成就/好友内部卡片使用透明或 `var(--glass-secondary)`，不再额外投影。

- [ ] **Step 2: 运行测试确认失败**

运行：`cd apps/owstats-desktop && npx tsx scripts/reviewVisualPolish.test.ts`

预期：失败，指出旧的 `--data-primary` 或表面层级不符合新增断言。

- [ ] **Step 3: 做最小 CSS 实现**

在两个页面的既有作用域末尾添加：

```css
.reviewWorkspace .phase6Eyebrow,
.friendDetailPage .phase6Eyebrow { color: var(--text-secondary); font-weight: 400; }

.reviewWorkspace .phase6GlassPanelShared,
.friendDetailPage .phase6ReviewPanel {
  background: var(--glass-primary);
  border-color: var(--glass-border);
  box-shadow: none;
}
```

把内部成就、协作和好友统计块调整为 `var(--glass-secondary)` 或透明分隔，保留进度条和可视化数据本身的语义色。

- [ ] **Step 4: 运行测试确认通过**

运行：`cd apps/owstats-desktop && npx tsx scripts/reviewVisualPolish.test.ts`

预期：`review visual polish contracts passed`。

- [ ] **Step 5: 提交独立切片**

```powershell
git add apps/owstats-desktop/scripts/reviewVisualPolish.test.ts apps/owstats-desktop/src/styles/features/review.css apps/owstats-desktop/src/styles/features/friends.css
git commit -m "style: align review surfaces with design system"
```

### Task 2: 排行采用中性工具栏与表格层级

**Files:**
- Modify: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/ranking.css`

- [ ] **Step 1: 写入失败的视觉契约**

在排行测试新增断言：`.phase6RankingHead` 使用 `var(--glass-primary)`、`var(--glass-border)` 和 `box-shadow: none`；非图表的数据文本不使用 `--data-primary`；活动排序保留中性深色背景并使用 `var(--text-on-action)`。

- [ ] **Step 2: 运行测试确认失败**

运行：`cd apps/owstats-desktop && npx tsx scripts/rankingCommandCenterRedesign.test.ts`

预期：失败并显示旧排行头部或排序颜色规则。

- [ ] **Step 3: 做最小 CSS 实现**

在 `ranking.css` 的 `.rankingPage` 作用域中，把工具栏、表头和行分隔统一为玻璃一级面和低对比边框；正文、计数和排序文字使用 `--text-primary`/`--text-secondary`、`font-weight: 400`；仅保留活动分段按钮的 `--action-primary` 深色填充。

- [ ] **Step 4: 运行测试确认通过**

运行：`cd apps/owstats-desktop && npx tsx scripts/rankingCommandCenterRedesign.test.ts`

预期：排行契约脚本退出码为 0。

- [ ] **Step 5: 提交独立切片**

```powershell
git add apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts apps/owstats-desktop/src/styles/features/ranking.css
git commit -m "style: align ranking with design system"
```

### Task 3: 生涯 MMR 区分中性文案与数据可视化

**Files:**
- Modify: `apps/owstats-desktop/scripts/careerMmrUi.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/career-mmr.css`

- [ ] **Step 1: 写入失败的视觉契约**

新增断言，要求 `.careerMmrPanel` 使用玻璃一级面、低对比边框、无重阴影；模块标题和说明使用中性文字且常规字重；已有 `.careerMmrConfidenceBound` 的冷蓝绿和 MMR 折线的数据颜色契约必须继续成立。

- [ ] **Step 2: 运行测试确认失败**

运行：`cd apps/owstats-desktop && npx tsx scripts/careerMmrUi.test.ts`

预期：失败于新加的面板/文案层级断言，不影响已有 MMR 计算断言。

- [ ] **Step 3: 做最小 CSS 实现**

在 `career-mmr.css` 中将摘要、趋势、模式贡献与赛季分解面板改为 `--glass-primary`/`--glass-secondary` 的两层结构，清除内部叠层阴影；保持图表线、置信区间和正负数等数据编码使用现有语义色。

- [ ] **Step 4: 运行测试确认通过**

运行：`cd apps/owstats-desktop && npx tsx scripts/careerMmrUi.test.ts`

预期：`career MMR UI contracts passed`。

- [ ] **Step 5: 提交独立切片**

```powershell
git add apps/owstats-desktop/scripts/careerMmrUi.test.ts apps/owstats-desktop/src/styles/features/career-mmr.css
git commit -m "style: align career mmr with design system"
```

### Task 4: 对局工作台降低筛选、列表和详情的表面噪声

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [ ] **Step 1: 写入失败的视觉契约**

新增断言，要求 `.filterBar.phase6FilterBar`、`.matchListPane`、`.matchDetailPane` 使用玻璃面、低对比边框和无重阴影；结果状态仍由细标记表达；选中项使用深色或细边线，不使用紫色正文；角斗标签的激活态保留可辨识深色操作面。

- [ ] **Step 2: 运行测试确认失败**

运行：`cd apps/owstats-desktop && npx tsx scripts/matchWorkbench.test.ts`

预期：失败于新的表面层级断言，既有普通对局、角斗详情、缓存和滚动契约仍执行。

- [ ] **Step 3: 做最小 CSS 实现**

在 `matches.css` 为筛选栏、两栏工作台和详情模块添加页面作用域覆盖：外层工作台保留唯一轻阴影，内部表格、行和分段只用边线/背景差异；结果的胜负标记维持语义色；不修改滚动高度、窄屏切换和 `:has()` 工作台布局规则。

- [ ] **Step 4: 运行测试确认通过**

运行：`cd apps/owstats-desktop && npx tsx scripts/matchWorkbench.test.ts && npx tsx scripts/filterBarUi.test.ts`

预期：两个脚本退出码为 0。

- [ ] **Step 5: 提交独立切片**

```powershell
git add apps/owstats-desktop/scripts/matchWorkbench.test.ts apps/owstats-desktop/src/styles/features/matches.css
git commit -m "style: align match workbench with design system"
```

### Task 5: 全量构建和视觉验收

**Files:**
- Modify: no source changes expected

- [ ] **Step 1: 运行完整前端与后端验证**

依次运行：

```powershell
cd apps/owstats-desktop; npm test; npm run build
cd apps/owstats-desktop/src-tauri; cargo fmt --check; cargo test --tests
git diff --check
git status --short
```

预期：命令全部通过；状态中除已知 `Cargo.toml` 时间戳假变更外，不包含未提交的页面改动。

- [ ] **Step 2: 视觉检查三种视口**

使用匿名 fixture 或安全空态，检查 2540×1344、1920×1080、1440×900 下的导航、页面标题、筛选、滚动、选中、错误/空态、键盘焦点与 reduced-motion。确认卡片无白色硬边、非数据文字为中性黑灰、仅页面主标题加粗、紫/冷蓝绿只用于图表/进度等数据表达。

- [ ] **Step 3: 交付验收记录**

报告每个切片提交、实际运行命令、视觉检查结果、已知的 `Cargo.toml` 假变更，以及未接触真实隐私数据的结论。
