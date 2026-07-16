# 对局筛选器英雄与胜负位置交换实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将英雄移入对局页主筛选栏，将胜负移入更多筛选，并让更多筛选计数与新位置一致。

**Architecture:** 保持 `MatchFilters`、URL 参数和后端查询不变，只在现有主筛选组件与高级筛选组件之间移动两个 `<select>`。同步更新 `advancedMatchFilterKeys`，使视觉归属、DOM 顺序、键盘顺序和计数语义一致。

**Tech Stack:** React、TypeScript、Node `assert` 合同测试。

---

### Task 1: 用失败测试锁定新顺序和计数

**Files:**
- Modify: `apps/owstats-desktop/scripts/filterBarUi.test.ts`
- Test: `apps/owstats-desktop/scripts/filterBarUi.test.ts`

- [ ] **Step 1: 更新组件区域断言**

将主筛选栏预期标签改为 `模式、英雄、玩家名称、更多筛选、清除`，更多筛选预期标签改为 `胜负、地图、复盘标签、开始日期、结束日期`；分别断言英雄不再出现在更多筛选、胜负不再出现在主筛选栏。

```ts
for (const label of ["模式", "英雄", "玩家名称", "更多筛选", "清除"]) assert.match(toolbarSource, new RegExp(label));
for (const label of ["胜负", "地图", "复盘标签", "开始日期", "结束日期"]) assert.match(advancedSource, new RegExp(label));
assert.doesNotMatch(toolbarSource, />\s*胜负\s*</);
assert.doesNotMatch(advancedSource, />\s*英雄\s*</);
```

- [ ] **Step 2: 更新更多筛选计数断言**

```ts
assert.equal(countAdvancedMatchFilters({ hero: "ana" }), 0);
assert.equal(countAdvancedMatchFilters({ result: "win" }), 1);
assert.equal(countAdvancedMatchFilters({ hero: "ana", map: "ilios", result: "win" }), 2);
```

- [ ] **Step 3: 运行测试并确认 RED**

Run: `npx tsx scripts/filterBarUi.test.ts`

Expected: FAIL，因为当前英雄仍在高级筛选、胜负仍在主筛选，且 `hero` 仍计入高级筛选数量。

### Task 2: 最小化交换两个筛选控件

**Files:**
- Modify: `apps/owstats-desktop/src/features/matches/MatchFilterToolbar.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/matchFilters.ts`
- Test: `apps/owstats-desktop/scripts/filterBarUi.test.ts`

- [ ] **Step 1: 将英雄选择器放入主筛选栏**

在 `MatchFilterToolbar` 中用英雄选择器替换胜负选择器，保留原位置和控件样式：

```tsx
<label>
  英雄
  <select value={props.filters.hero ?? ""} onChange={(event) => props.onPatch({ hero: emptyOrValue(event.target.value) })}>
    <option value="">全部</option>
    {props.options.heroes.map((hero) => <option key={hero.id} value={hero.id}>{hero.label}</option>)}
  </select>
</label>
```

- [ ] **Step 2: 将胜负选择器放入更多筛选**

在 `AdvancedMatchFilters` 第一项用胜负选择器替换英雄选择器，并从 `matchFilters` 导入 `filterResultLabel`：

```tsx
<label>
  胜负
  <select value={props.filters.result ?? ""} onChange={(event) => props.onPatch({ result: emptyOrValue(event.target.value) })}>
    <option value="">全部</option>
    {props.options.results.map((result) => (
      <option key={result} value={result}>{filterResultLabel(result)}</option>
    ))}
  </select>
</label>
```

- [ ] **Step 3: 调整更多筛选计数键**

```ts
export const advancedMatchFilterKeys = ["result", "map", "primaryTag", "dateFrom", "dateTo"] as const;
```

- [ ] **Step 4: 运行针对性测试并确认 GREEN**

Run: `npx tsx scripts/filterBarUi.test.ts`

Expected: PASS。

### Task 3: 完整验证

**Files:**
- Verify only; no additional production files.

- [ ] **Step 1: 运行完整前端测试**

Run: `npm test`

Expected: PASS，所有测试零失败。

- [ ] **Step 2: 运行生产构建**

Run: `npm run build`

Expected: PASS，TypeScript 与 Vite 构建成功，dist 路径扫描通过。

- [ ] **Step 3: 检查差异与工作树**

Run: `git diff --check`，随后运行 `git status --short`。

Expected: `git diff --check` 退出码为 0；本轮仅新增规格/计划并修改上述四个筛选器相关文件，其他原有改动保持不变。
