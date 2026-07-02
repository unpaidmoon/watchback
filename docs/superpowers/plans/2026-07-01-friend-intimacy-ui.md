# Friend Intimacy UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the review page friend common hero module as a five-item friend intimacy board.

**Architecture:** Keep the existing `FriendAnalytics` DTO and `FriendPhase5Summary` entry point. Replace the simple breakdown list with static insight cards that show rank, friend identity, representative hero, intimacy score, match count, win rate, and a relative score bar.

**Tech Stack:** React, TypeScript, CSS, existing `HeroAvatar` and `formatWinRate` helpers.

---

### Task 1: Lock UI Contract With Text Test

**Files:**
- Modify: `apps/owstats-desktop/scripts/friendAnalyticsMetrics.test.ts`
- Modify: `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx`
- Modify: `apps/owstats-desktop/src/styles.css`

- [ ] **Step 1: Write the failing test**

Add checks that the review friend summary uses the new title, keeps five cards, and renders the score bar class.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/owstats-desktop; npx tsx scripts/friendAnalyticsMetrics.test.ts`
Expected: FAIL because the current source still contains `好友常用英雄` and no intimacy board classes.

- [ ] **Step 3: Write minimal implementation**

Update `FriendPhase5Summary` to render the five-card `好友亲密度` board and add CSS for the card layout.

- [ ] **Step 4: Run focused and full frontend tests**

Run: `cd apps/owstats-desktop; npx tsx scripts/friendAnalyticsMetrics.test.ts`
Expected: PASS.

Run: `cd apps/owstats-desktop; npm run test:text`
Expected: PASS.
