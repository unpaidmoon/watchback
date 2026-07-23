# Remove Redundant Page Labels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove three redundant UI elements and their layout footprint.

**Architecture:** Delete the owning React markup and any imports, state, or callbacks that become unreachable. Protect the result with source-contract assertions in the existing shell test, then run the focused test, full frontend test suite, build, and repository hygiene checks.

**Tech Stack:** React, TypeScript, TSX source-contract tests, Vite.

---

### Task 1: Add removal contracts

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] **Step 1: Replace the existing positive assertions for the local-data entry and update action with negative assertions, and add negative assertions for the map heading**

```ts
assertEqual(appSource.includes('to="/onboarding#local-data"'), false, "global shell removes the local data entry");
assertEqual(appSource.includes('aria-label="查看本地数据"'), false, "global shell removes the local data entry accessible node");
assertEqual(onboardingSource.includes("onClick={runUpdateCheck}"), false, "command center removes the header update action");
assertEqual(friendAnalyticsSource.includes("地图表现概览"), false, "map analysis removes its redundant title");
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `npx tsx scripts/phase6Shell.test.ts`

Expected: FAIL because all three elements still exist.

### Task 2: Remove the three UI elements

**Files:**
- Modify: `apps/owstats-desktop/src/App.tsx`
- Modify: `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`

- [ ] **Step 1: Delete the global local-data wrapper and remove the unused `CircleDot` import**

- [ ] **Step 2: Delete the map analysis header so `MapCardGrid` or its empty state renders first, and remove the unused `Map` import if no other usage remains**

- [ ] **Step 3: Remove the `PageHeader` actions prop, `Download` and `checkAppUpdate` imports, `checkingUpdate` state, and `runUpdateCheck` callback**

- [ ] **Step 4: Run the focused test and verify GREEN**

Run: `npx tsx scripts/phase6Shell.test.ts`

Expected: PASS.

### Task 3: Update fixtures and clean dead styles

**Files:**
- Modify if directly tied to removed UI: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`
- Modify if no longer referenced: `apps/owstats-desktop/src/styles/shell.css`
- Modify if required by removal contract: `apps/owstats-desktop/scripts/visualSystem.test.ts`

- [ ] **Step 1: Remove “检查更新” from the command-center visual fixture**

- [ ] **Step 2: Remove `.appShellStatus` and `.appDataEntry` styles only if no remaining production usage exists, updating the corresponding style contract**

- [ ] **Step 3: Re-run the focused test and affected visual-system test**

Run: `npx tsx scripts/phase6Shell.test.ts && npx tsx scripts/visualSystem.test.ts`

Expected: PASS.

### Task 4: Verify

**Files:**
- No production changes expected.

- [ ] **Step 1: Run all frontend tests**

Run: `npm test`

Expected: PASS.

- [ ] **Step 2: Build the frontend**

Run: `npm run build`

Expected: PASS.

- [ ] **Step 3: Check patch hygiene and status**

Run: `git diff --check` and `git status --short`

Expected: no whitespace errors; only the two pre-existing user modifications plus files belonging to this task.

No commit or staging step is included because the project instructions prohibit them unless explicitly requested.
