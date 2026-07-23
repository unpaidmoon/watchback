# 角斗单回合累积资金显示 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让角斗对局的单回合详情只显示回合结束后的累积资金，不显示资金变化箭头。

**Architecture:** 保留现有 `CompactArenaPlayerExpansion` 数据流和 `economyMode` 区分，仅收窄 `formatCompactArenaEconomy` 的回合格式化规则。汇总模式、后端数据和展示结构均不变。

**Tech Stack:** React、TypeScript、Node.js assert 测试

---

### Task 1: 收窄回合资金格式化规则

**Files:**
- Modify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`

- [ ] **Step 1: 写入失败测试**

将原先要求资金变化箭头的源码断言改为以下断言：

```ts
assert.match(
  compactSource,
  /if \(mode === "round"\) \{[\s\S]*if \(player\.endWorth !== null\) return String\(player\.endWorth\);[\s\S]*if \(player\.worth !== null\) return String\(player\.worth\);/,
  "compact arena round economy should show cumulative end worth without a transition",
);
assert.doesNotMatch(compactSource, /return `\$\{player\.worth\} -> \$\{player\.endWorth\}`;/);
```

- [ ] **Step 2: 运行测试并确认旧逻辑失败**

Run: `npm test -- scripts/arenaDetailFormat.test.ts`

Expected: FAIL，失败信息指出旧实现仍返回 `` `${player.worth} -> ${player.endWorth}` ``。

- [ ] **Step 3: 实现最小修改**

将回合模式改为优先返回累积资金：

```ts
if (mode === "round") {
  if (player.endWorth !== null) return String(player.endWorth);
  if (player.worth !== null) return String(player.worth);
  return "暂无";
}
```

汇总模式继续优先使用 `endWorth`，然后回退到 `worth`。

- [ ] **Step 4: 运行针对性测试**

Run: `npm test -- scripts/arenaDetailFormat.test.ts`

Expected: PASS。

- [ ] **Step 5: 运行前端验证**

Run: `npm test`

Expected: 所有前端测试通过。

Run: `npm run build`

Expected: TypeScript 与 Vite 构建成功。

Run: `git diff --check`

Expected: 无空白错误。

- [ ] **Step 6: 提交本任务文件**

```powershell
git add -- docs/superpowers/specs/2026-07-23-stadium-round-cumulative-money-design.md docs/superpowers/plans/2026-07-23-stadium-round-cumulative-money.md apps/owstats-desktop/scripts/arenaDetailFormat.test.ts apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx
git commit -m "fix: 角斗回合仅显示累积资金"
```
