# 对局详情头图右侧信息重排 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 删除相邻对局导航，将地图名、比分、模式、时长和开始时间按参考图统一放在头图右侧。

**Architecture:** 收缩 `MatchDetailHeader` 为纯展示组件，不再接收导航状态或回调。调用方删除相邻对局派生状态，CSS 将地图侧内容改为单列右对齐三层结构，并用合同测试锁定信息顺序和响应式行为。

**Tech Stack:** React、TypeScript、CSS 设计令牌、Node `assert` 合同测试。

---

### Task 1: 用失败测试定义无导航的右侧信息结构

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`

- [ ] **Step 1: 替换旧导航合同**

在 `matchWorkbench.test.ts` 删除要求 `hasPrevious`、`hasNext`、`onPrevious`、`onNext`、禁用按钮和相邻对局计算存在的断言，加入：

```ts
assert.equal(detailHeaderSource.includes("hasPrevious"), false, "detail header should remove previous navigation state");
assert.equal(detailHeaderSource.includes("hasNext"), false, "detail header should remove next navigation state");
assert.equal(detailHeaderSource.includes("上一场"), false, "detail header should remove the previous button");
assert.equal(detailHeaderSource.includes("下一场"), false, "detail header should remove the next button");
assert.equal(matchesPageSource.includes("previousMatchId"), false, "matches page should stop deriving previous match state");
assert.equal(matchesPageSource.includes("nextMatchId"), false, "matches page should stop deriving next match state");
assert.equal(detailHeaderSource.includes("matchDetailHeaderInfo"), true, "header should own one right-aligned information group");
assert.ok(
  detailHeaderSource.indexOf("<h2>{mapName}</h2>") < detailHeaderSource.indexOf("matchDetailHeaderScore") &&
    detailHeaderSource.indexOf("matchDetailHeaderScore") < detailHeaderSource.indexOf("matchDetailHeaderMeta"),
  "header information order should be map, score, then metadata",
);
```

- [ ] **Step 2: 加入右对齐视觉合同**

```ts
assert.match(matchStyleSource, /\.matchDetailHeaderInfo\s*\{[^}]*justify-items:\s*end;[^}]*text-align:\s*right;/s);
assert.match(matchStyleSource, /\.matchDetailHeaderCopy h2\s*\{[^}]*font-size:\s*22px;/s);
assert.match(matchStyleSource, /\.matchDetailHeaderScore\s*\{[^}]*font-size:\s*30px;/s);
assert.match(matchStyleSource, /\.matchDetailHeaderMeta\s*\{[^}]*font-size:\s*13px;/s);
assert.doesNotMatch(matchStyleSource, /\.matchDetailHeaderNavigation\s*\{/);
```

同步删除 `matchWorkbenchVisualFixture.tsx` 向 `CompactMatchDetail` 传递的四个导航 props。

- [ ] **Step 3: 运行测试并确认红灯**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts`

Expected: 因 `MatchDetailHeader` 仍包含导航 props/按钮而失败，不得是语法或模块加载错误。

### Task 2: 删除导航状态并实现参考图信息层级

**Files:**
- Modify: `apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx`
- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Test: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: 收缩共享 Header 类型与 JSX**

将 `MatchDetailHeaderProps` 保留为：

```ts
export type MatchDetailHeaderProps = {
  resultLabel: string;
  result: string | null;
  mapGuid: string | null;
  mapName: string;
  modeLabel: string;
  score: string;
  duration: string;
  startTime: string;
};
```

删除 `MatchDetailNavigationProps`，把地图侧 JSX 改为：

```tsx
<div className="matchDetailHeaderContent">
  <div className="matchDetailHeaderInfo">
    <h2>{mapName}</h2>
    <strong className="matchDetailHeaderScore" data-numeric>{score}</strong>
    <span className="matchDetailHeaderMeta">
      <span>{modeLabel}</span>
      <span>时长 {duration}</span>
      <span>开始 {startTime}</span>
    </span>
  </div>
</div>
```

- [ ] **Step 2: 删除所有调用方导航状态**

- `CompactMatchDetailProps` 不再交叉 `MatchDetailNavigationProps`，普通与角斗分支只传展示数据。
- `MatchDetailPaneProps` 只保留 `onRetry` 相关选择，不再选择四个导航字段。
- `MatchesPage.tsx` 删除 `selectedMatchIndex`、`previousMatchId`、`nextMatchId`、`selectAdjacentMatch` 以及四个传参。
- `MatchDetailPage.tsx` 删除四个固定假值传参。

- [ ] **Step 3: 替换地图侧 CSS**

```css
.matchDetailHeaderContent {
  z-index: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: var(--space-4) var(--space-6);
  color: var(--action-on-primary);
}

.matchDetailHeaderInfo {
  min-width: 0;
  display: grid;
  justify-items: end;
  gap: var(--space-1);
  text-align: right;
}

.matchDetailHeaderInfo h2 {
  margin: 0;
  max-width: 100%;
  overflow: hidden;
  color: var(--action-on-primary);
  font-size: 22px;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.matchDetailHeaderScore {
  color: var(--action-on-primary);
  font-size: 30px;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}

.matchDetailHeaderMeta {
  justify-content: flex-end;
  gap: var(--space-2);
  color: color-mix(in srgb, var(--action-on-primary) 78%, transparent);
  font-size: 13px;
}
```

删除全部 `.matchDetailHeaderNavigation` 规则。在 `max-width: 640px` 中保持 `.matchDetailHeaderInfo { justify-items: end; text-align: right; }`，并允许 `.matchDetailHeaderMeta` 换行。

- [ ] **Step 4: 运行目标测试与完整验证**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/matchPerformanceBadges.test.ts; npm test; npm run build`

Expected: 全部命令退出码为 0；生产构建仅允许既有 chunk 大小警告。

- [ ] **Step 5: 检查边界并提交**

Run: `git diff --check; git status --short`

Expected: 无空白错误；无关评审页、运行时配置和后端文件仍保持用户原有未暂存状态。

```powershell
git add apps/owstats-desktop/scripts/matchWorkbench.test.ts apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx apps/owstats-desktop/src/pages/MatchesPage.tsx apps/owstats-desktop/src/pages/MatchDetailPage.tsx apps/owstats-desktop/src/styles/features/matches.css
git commit -m "feat: right-align match header details"
```
