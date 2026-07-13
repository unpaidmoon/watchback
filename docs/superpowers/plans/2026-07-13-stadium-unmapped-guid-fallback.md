# Stadium Unmapped GUID Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在角斗详情中将未映射的装备/异能显示为其原始 GUID，而非“待补充”文本。

**Architecture:** 前端组件保留已解析资源的名称、图标和描述；仅当 `resolveArenaModTraitInfo` 返回空值且槽位存在 GUID 时，把该 GUID 作为槽位名称、tooltip 标题和无障碍标签。协作约束明确允许这个仅限角斗素材的例外，其他玩家、英雄和地图 fallback 仍不得展示原始标识。

**Tech Stack:** React、TypeScript、Node `tsx` 资源测试。

---

### Task 1: 记录角斗素材 GUID 例外

**Files:**

- Modify: `AGENTS.md:32`
- Modify: `docs/superpowers/specs/2026-07-13-stadium-unmapped-guid-fallback-design.md`

- [ ] **Step 1: 写入协作约束例外**

将隐私条款中的 fallback 规则明确为：玩家、英雄、地图继续使用中文稳定兜底；角斗 `traitGuids` 与 `modGuids` 无法映射时必须显示原始 GUID，供资产校对使用。

- [ ] **Step 2: 同步设计文档**

在设计文档范围中明确该例外覆盖已映射失败的槽位名称、tooltip 标题和 `aria-label`，不适用于任何玩家身份或凭据字段。

### Task 2: 为 GUID 兜底写失败测试

**Files:**

- Modify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`
- Modify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`

- [ ] **Step 1: 增加组件行为断言**

在 `arenaDetailFormat.test.ts` 断言组件将槽位 GUID 用作资源未命中时的显示名，并且调用点不再传入“待补充特质”或“待补充异能”。

```ts
assert.equal(compactSource.includes("const displayName = info?.name ?? guid"), true);
assert.equal(compactSource.includes("fallbackName="), false);
```

- [ ] **Step 2: 增加解析器失败断言**

在 `phase6Assets.test.ts` 导入 `resolveArenaModTraitInfo`，断言未知 `0x0CC800000000FFFF` 返回 `null`，以说明显示 GUID 是组件兜底而不是伪造资源映射。

```ts
assertEqual(resolveArenaModTraitInfo("0x0CC800000000FFFF"), null, "unmapped arena id remains unresolved");
```

- [ ] **Step 3: 运行测试确认失败**

Run: `npx tsx scripts/arenaDetailFormat.test.ts`

Expected: FAIL，因为组件尚未声明 `displayName`。

### Task 3: 实现紧凑角斗详情 GUID 兜底

**Files:**

- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx:335-365`

- [ ] **Step 1: 简化调用点**

删除 `fallbackName` 参数；异能和装备组仅传 `title`、`guids` 和 `slots`。

- [ ] **Step 2: 实现槽位显示名**

在每个有 GUID 的槽位中声明：

```ts
const info = resolveArenaModTraitInfo(guid);
const displayName = info?.name ?? guid;
```

将 `displayName` 同时用于 `aria-label`、异能槽位文字和 tooltip 标题；描述仍只在 `info?.description` 存在时显示。

- [ ] **Step 3: 运行定向测试确认通过**

Run: `npx tsx scripts/arenaDetailFormat.test.ts && npx tsx scripts/phase6Assets.test.ts`

Expected: PASS。

### Task 4: 完整验证

**Files:**

- Verify: `AGENTS.md`
- Verify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Verify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`
- Verify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`

- [ ] **Step 1: 运行资源与构建验证**

Run: `npm run test:resources; npm run build`

Expected: PASS；构建可保留 Vite 大 bundle 警告。

- [ ] **Step 2: 检查工作树**

Run: `git diff --check; git status --short`

Expected: 无 whitespace 错误；仅报告本任务预期变更及既有用户改动，不暂存或提交。
