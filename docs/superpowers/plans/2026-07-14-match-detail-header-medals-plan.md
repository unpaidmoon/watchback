# 对局详情头图与奖牌迁移 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在普通对局详情使用浅色斜切地图头图，并把评分与同队奖牌从详情摘要迁移至可扫描的对局列表行。

**Architecture:** 不改动 Tauri 或 SQLite：`MatchSummary` 已携带 `evaluation` 与 `teamAwards`，列表卡片直接复用 `MatchPerformanceBadges` 的紧凑模式。普通与角斗详情共用扩展后的 `MatchDetailHeader`；只有普通详情删除个人摘要，角斗回合结构保持不变。

**Tech Stack:** React、TypeScript、现有 Vite 资源组件 `MapPreview`、CSS 设计令牌、Node `assert` 前端合同测试。

---

## 文件职责

- `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`：在单场列表行中渲染完整紧凑表现徽章。
- `apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx`：渲染地图预览背景、结果斜切块与中文/英文结果文案。
- `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`：向头图传递地图 GUID，并移除普通对局个人摘要。
- `apps/owstats-desktop/src/styles/features/matches.css`：为列表徽章和头图提供令牌化布局、三种结果状态与窄屏规则。
- `apps/owstats-desktop/scripts/matchWorkbench.test.ts`：锁定普通/角斗详情、头图和列表卡片合同。
- `apps/owstats-desktop/scripts/matchPerformanceBadges.test.ts`：锁定列表中完整紧凑徽章的使用方式。

### Task 1: 先把页面合同改成目标行为

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchPerformanceBadges.test.ts`

- [ ] **Step 1: 写出普通详情与列表的失败断言**

  在 `matchWorkbench.test.ts` 将旧的“列表不渲染评分奖牌”和“普通详情含本人表现”的断言替换为：

  ```ts
  assert.equal(listCardSource.includes("<MatchPerformanceBadges"), true, "MatchListCard should render compact grade and award badges");
  assert.equal(listCardSource.includes("compact"), true, "list badges should use the compact layout");
  assert.equal(compactDetailSource.includes("<section className=\"matchDetailPerformance\""), false, "normal detail should remove the personal summary panel");
  assert.equal(compactDetailSource.includes("本人表现"), false, "normal detail should not retain the removed summary title");
  ```

  将普通详情静态标记断言更新为：

  ```ts
  assert.doesNotMatch(normalMarkup, />本人表现</);
  assert.match(normalMarkup, />队友数据</);
  assert.match(normalMarkup, />对手数据</);
  ```

- [ ] **Step 2: 为头图状态写出失败断言**

  在同一测试中加入：

  ```ts
  assert.equal(detailHeaderSource.includes("MapPreview"), true, "detail header should render the existing map preview asset");
  assert.equal(detailHeaderSource.includes("resultEnglishLabel"), true, "detail header should render an English result helper");
  assert.match(matchStyleSource, /\.matchDetailHeaderResultBlock\.result-win[\s\S]*?background:\s*var\(--status-success-bg\)/);
  assert.match(matchStyleSource, /\.matchDetailHeaderResultBlock\.result-loss[\s\S]*?background:\s*var\(--surface-control\)/);
  assert.match(matchStyleSource, /\.matchDetailHeaderResultBlock\.result-draw[\s\S]*?background:\s*var\(--surface-control\)/);
  assert.doesNotMatch(matchStyleSource, /#ff8[0-9a-f]{3}|orange/i, "match result header must not introduce orange");
  ```

  同时把 `matchPerformanceBadges.test.ts` 中 `MatchListCard should not render grade or award badges` 改为 `true`，并断言 `MatchPerformanceBadges` 的 `compact` 属性存在。

- [ ] **Step 3: 运行测试并确认失败原因**

  Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/matchPerformanceBadges.test.ts`  
  Expected: `matchWorkbench.test.ts` 与 `matchPerformanceBadges.test.ts` 因旧 UI 合同失败；其余失败不得被忽略。

- [ ] **Step 4: 提交仅测试合同**

  ```powershell
  git add apps/owstats-desktop/scripts/matchWorkbench.test.ts apps/owstats-desktop/scripts/matchPerformanceBadges.test.ts
  git commit -m "test: define match detail header and medal behavior"
  ```

### Task 2: 在列表迁移完整表现徽章并移除普通详情摘要

**Files:**
- Modify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Test: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Test: `apps/owstats-desktop/scripts/matchPerformanceBadges.test.ts`

- [ ] **Step 1: 在列表卡片导入并渲染紧凑徽章**

  用下列导入替换 `primaryTagBadgeTone` 导入，并在英雄名之后替换现有的条件标签分支：

  ```tsx
  import { MatchPerformanceBadges } from "../../shared/PerformanceBadgesView";

  <HeroAvatar heroKey={match.heroGuid || match.heroName} name={match.heroName} />
  <strong className="matchListCardHeroName">{match.heroName}</strong>
  <MatchPerformanceBadges evaluation={match.evaluation} teamAwards={match.teamAwards} compact />
  ```

  删除不再使用的 `listPerformanceTone` 函数，保留 K/D/A 与数据面板的顺序不变。

- [ ] **Step 2: 移除普通详情的个人摘要 JSX**

  在 `CompactMatchDetail` 的普通详情 `phase6CompactDetailScroll` 内，删除整个：

  ```tsx
  <section className="matchDetailPerformance" aria-labelledby="self-performance-title">
    ...
    <MatchPerformanceBadges evaluation={detail.evaluation} teamAwards={detail.teamAwards} />
  </section>
  ```

  并删除该文件中仅由此使用的 `MatchPerformanceBadges` 导入。`CompactArenaDetail`、其回合状态和 `MatchTeamTable` 调用不得改变。

- [ ] **Step 3: 运行目标测试并确认通过**

  Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/matchPerformanceBadges.test.ts`  
  Expected: 两个更新的合同测试通过，且没有角斗静态标记或加载/错误状态回归。

- [ ] **Step 4: 提交功能迁移**

  ```powershell
  git add apps/owstats-desktop/src/features/matches/MatchListCard.tsx apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx
  git commit -m "feat: move match medals into list rows"
  ```

### Task 3: 实现共享地图头图和令牌化响应式样式

**Files:**
- Modify: `apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Test: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: 扩展头图输入并渲染背景图片**

  在 `MatchDetailHeaderProps` 加入 `mapGuid: string | null`，从 `phase6Components` 导入 `MapPreview`，并将 header 内容组织为：

  ```tsx
  const tone = result === "win" || result === "loss" || result === "draw" ? result : "neutral";
  const resultEnglishLabel = tone === "win" ? "VICTORY" : tone === "loss" ? "DEFEAT" : "DRAW";

  <header className={`matchDetailHeader result-${tone}`}>
    <MapPreview mapKey={mapGuid ?? mapName} name={mapName} className="matchDetailHeaderMap" />
    <div className="matchDetailHeaderScrim" aria-hidden="true" />
    <div className={`matchDetailHeaderResultBlock result-${tone}`}>
      <strong>{resultLabel}</strong>
      <span>{resultEnglishLabel}</span>
    </div>
    <div className="matchDetailHeaderContent">
      {/* 保留地图名、比分、模式、时长、开始时间和现有上一场/下一场按钮 */}
    </div>
  </header>
  ```

  在普通分支传 `mapGuid={detail.mapGuid}`，在角斗分支传 `mapGuid={selected.mapGuid}`。不要复制或改写 `MapPreview` 的资源回退逻辑。

- [ ] **Step 2: 将样式替换为一层横幅而非嵌套卡片**

  在 `matches.css` 用以下关键规则替换现有 `.matchDetailHeader*` 视觉规则，并删除已废弃的 `.matchDetailPerformance*` 规则：

  ```css
  .matchDetailHeader { position:relative; isolation:isolate; overflow:hidden; min-height:156px; display:grid; grid-template-columns:minmax(180px, .37fr) minmax(0, 1fr); border:1px solid var(--border-subtle); border-radius:var(--radius-secondary); background:var(--surface-secondary); }
  .matchDetailHeaderMap { position:absolute; inset:0; z-index:-2; width:100%; height:100%; object-fit:cover; filter:saturate(.78) contrast(.92); }
  .matchDetailHeaderScrim { position:absolute; inset:0; z-index:-1; background:color-mix(in srgb, var(--text-primary) 42%, transparent); }
  .matchDetailHeaderResultBlock { z-index:1; display:grid; align-content:center; gap:2px; padding:var(--space-5); clip-path:polygon(0 0,86% 0,100% 100%,0 100%); }
  .matchDetailHeaderResultBlock.result-win { background:var(--status-success-bg); color:var(--text-primary); }
  .matchDetailHeaderResultBlock.result-loss, .matchDetailHeaderResultBlock.result-draw, .matchDetailHeaderResultBlock.result-neutral { background:var(--surface-control); color:var(--text-primary); }
  .matchDetailHeaderContent { z-index:1; min-width:0; display:grid; grid-template-columns:minmax(0,1fr) auto; gap:var(--space-4); align-items:center; padding:var(--space-4); color:var(--text-primary); }
  .matchListCardHeroLine .phase6PerformanceBadges.compact { min-width:0; margin-left:auto; }
  ```

  使用已有的 `--status-success-bg`，不能创建新的十六进制颜色。不得使用 CSS 渐变、橙色或新增阴影。

- [ ] **Step 3: 添加窄屏和图片回退规则**

  在已有 `@media (max-width: 1099px)` 内加入：

  ```css
  .matchDetailHeader { grid-template-columns:minmax(132px, .42fr) minmax(0, 1fr); }
  .matchDetailHeaderContent { grid-template-columns:minmax(0,1fr) auto; }
  .matchDetailHeaderNavigation { grid-column:1 / -1; justify-content:flex-end; }
  ```

  再在 `@media (max-width: 640px)` 加入：

  ```css
  .matchDetailHeader { grid-template-columns:1fr; }
  .matchDetailHeaderResultBlock { min-height:88px; clip-path:none; padding:var(--space-4); }
  .matchDetailHeaderContent { grid-template-columns:1fr; }
  .matchDetailHeaderNavigation { justify-content:stretch; }
  .matchDetailHeaderNavigation .ghostButton { flex:1; min-height:44px; }
  ```

  `MapPreview` 在资源耗尽时会替换为已有 fallback 元素；横幅自身的 `background: var(--surface-secondary)` 和遮罩必须保证此时文字仍可读。

- [ ] **Step 4: 运行合同测试与构建**

  Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/matchPerformanceBadges.test.ts; npm run build`  
  Expected: 测试全通过，Vite 生产构建完成且没有 TypeScript 错误。

- [ ] **Step 5: 提交头图实现**

  ```powershell
  git add apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx apps/owstats-desktop/src/styles/features/matches.css
  git commit -m "feat: add result map banners to match details"
  ```

### Task 4: 完整回归与交付检查

**Files:**
- Verify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Verify: `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
- Verify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`

- [ ] **Step 1: 用 fixture 手动验证两个入口与三种结果**

  启动既有视觉 fixture，依次打开普通胜利、失败和平局对局；检查内嵌 `/matches` 详情和独立 `MatchDetailPage` 都使用同一横幅，列表行都有标签、评级和奖牌，普通详情没有个人摘要，角斗详情仍有回合标签与队伍表。

- [ ] **Step 2: 运行完整前端验证**

  Run: `cd apps/owstats-desktop; npm test; npm run build`  
  Expected: 测试套件和生产构建均成功。

- [ ] **Step 3: 检查变更边界**

  Run: `git diff --check; git status --short`  
  Expected: 无空白错误；除本计划的前端文件、测试与设计/计划文档外，不暂存或修改已有用户改动。

- [ ] **Step 4: 提交最终验证使用的合同测试**

  ```powershell
  git add apps/owstats-desktop/scripts/matchWorkbench.test.ts apps/owstats-desktop/scripts/matchPerformanceBadges.test.ts
  git commit -m "test: cover match banner responsiveness"
  ```
