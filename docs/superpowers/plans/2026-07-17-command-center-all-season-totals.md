# Command Center All-Season Totals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make command-center summary metrics aggregate all synced seasons and all six match modes.

**Architecture:** Add a small frontend pure function that reduces the existing `seasonStatuses` DTO rows. Keep current-season `localData` and backend review-entry semantics unchanged, then make `OnboardingPage` render the aggregate.

**Tech Stack:** React, TypeScript, Vitest, Tauri DTOs

---

### Task 1: Aggregate command-center season totals

**Files:**
- Create: `apps/owstats-desktop/src/features/command-center/commandCenterTotals.ts`
- Create: `apps/owstats-desktop/scripts/commandCenterTotals.test.ts`
- Modify: `apps/owstats-desktop/package.json`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`

- [ ] **Step 1: Write the failing test**

Test five season rows whose match counts are `259`, `941`, `564`, `329`, and `295`. Assert total matches are `2388`, total details are the sum of the corresponding detail counts, and coverage is calculated from the two totals rather than averaging season percentages. Add an empty-list assertion for zero totals and `0%`.

- [ ] **Step 2: Run the test to verify RED**

Run: `npx tsx scripts/commandCenterTotals.test.ts`

Expected: FAIL because `summarizeCommandCenterSeasons` does not exist.

- [ ] **Step 3: Write the minimal implementation**

Export `summarizeCommandCenterSeasons(seasonStatuses)` from `commandCenterTotals.ts`. Reduce `matchCount` and `detailCount`, then return `coveragePercent = totalMatches > 0 ? Math.min(100, Math.round(totalDetails / totalMatches * 100)) : 0`.

In `OnboardingPage.tsx`, replace the display-only `presetMatches`, `detailMatches`, and `coveragePercent` derivation with the pure-function result. Preserve `status.localData` for existing backend health and review-entry behavior.

- [ ] **Step 4: Run targeted and full verification**

Add `tsx scripts/commandCenterTotals.test.ts` to `test:unit`. Run `npx tsx scripts/commandCenterTotals.test.ts`, then `npm test`, `npm run build`, `git diff --check`, and `git status --short`.

Expected: all test commands and build exit successfully; the status contains only the three implementation files, these two documentation files, and the user's pre-existing changes.
