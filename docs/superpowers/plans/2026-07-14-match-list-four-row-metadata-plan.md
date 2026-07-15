# 对局列表四行信息 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将对局列表卡片改为比赛摘要、模式时间、英雄与四项数据、标签奖牌四个稳定信息行。

**Architecture:** `MatchListCard` 只重组既有 `MatchSummary` 字段，不引入新状态或 API。`matches.css` 负责四行网格与紧凑回退；`MatchPerformanceBadges` 仍负责标签/奖牌数据和可访问名称，其列表上下文样式改为细描边。

**Tech Stack:** React、TypeScript、CSS、Node assert 脚本测试。

---

### Task 1: 固化列表卡的四行结构

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`

- [ ] **Step 1: 写入失败的结构断言**

```ts
assert.equal(listCardSource.includes("matchListCardModeTime"), true);
assert.equal(listCardSource.includes('label="伤"'), true);
assert.equal(listCardSource.includes('label="疗"'), true);
assert.equal(listCardSource.includes('label="挡"'), true);
assert.ok(listCardSource.indexOf("matchListCardHeroLine") < listCardSource.indexOf("matchListCardBadgeLine"));
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `npm test -- matchWorkbench.test.ts`

Expected: 失败，提示 `matchListCardModeTime` 尚不存在。

- [ ] **Step 3: 最小实现四行标记**

```tsx
<span className="matchListCardModeTime"><span>{match.modeLabel}</span><span>{match.beginLabel}</span></span>
<span className="matchListCardHeroLine">...</span>
<span className="matchListCardBadgeLine"><MatchPerformanceBadges ... compact /></span>
<MetricBlock label="伤" value={formatCompactStat(match.damage)} />
<MetricBlock label="疗" value={formatCompactStat(match.healing)} />
<MetricBlock label="挡" value={formatCompactStat(match.mitigation)} />
```

- [ ] **Step 4: 重跑测试并确认通过**

Run: `npm test -- matchWorkbench.test.ts`

Expected: PASS。

### Task 2: 将标签奖牌收束为细描边第四行

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [ ] **Step 1: 写入失败的样式断言**

```ts
assert.match(matchStyleSource, /\.matchListCardBadgeLine[\s\S]*?min-height:\s*28px/);
assert.match(matchStyleSource, /\.matchListCardBadgeLine \.primaryTag[\s\S]*?background:\s*transparent/);
assert.match(matchStyleSource, /\.matchListCardBadgeLine \.phase6AwardMedal[\s\S]*?border-radius:\s*50%/);
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `npm test -- matchWorkbench.test.ts`

Expected: 失败，提示第四行选择器尚不存在。

- [ ] **Step 3: 最小实现细描边样式和窄屏回退**

```css
.matchListCardBadgeLine { min-height: 28px; display: flex; align-items: center; }
.matchListCardBadgeLine .primaryTag { background: transparent; }
.matchListCardBadgeLine .phase6AwardMedal { border-radius: 50%; }
```

在 `@media (max-width: 640px)` 中让第 2、3 行可换行，第四行保持独占并保留 28px 高度。

- [ ] **Step 4: 重跑测试并确认通过**

Run: `npm test -- matchWorkbench.test.ts`

Expected: PASS。

### Task 3: 完整验证

**Files:**
- Verify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
- Verify: `apps/owstats-desktop/src/styles/features/matches.css`

- [ ] **Step 1: 运行前端检查**

Run: `npm test; npm run build; git diff --check`

Expected: 命令全部成功，且 `git diff --check` 无输出。

- [ ] **Step 2: 审核变更范围**

Run: `git status --short; git diff -- apps/owstats-desktop/src/features/matches/MatchListCard.tsx apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/matchWorkbench.test.ts`

Expected: 仅有计划中三处产品文件及本任务文档变更；不得包含凭据或真实用户数据。
