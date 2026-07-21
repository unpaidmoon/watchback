# Match Favorite Actions Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align the two favorite action buttons with the top edge of the match-note textarea on wide layouts without changing control sizes or narrow-layout behavior.

**Architecture:** Keep the existing component markup and grid layout. Add a token-based top offset to `.matchFavoriteActions` for wide layouts, then reset that offset inside the existing `max-width: 1099px` media query.

**Tech Stack:** React, TypeScript, CSS, Node `assert` static UI contract tests

---

### Task 1: Lock the alignment contract

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`
- Test: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`

- [ ] **Step 1: Write the failing wide-layout assertion**

Extend the existing `.matchFavoriteActions` CSS assertion so it requires both `align-self: start` and `margin-top: calc(21px + var(--space-2))`:

```ts
assert.match(
  matchesCss,
  /\.matchFavoriteActions\s*\{(?=[^}]*display:\s*grid)(?=[^}]*align-self:\s*start)(?=[^}]*margin-top:\s*calc\(21px\s*\+\s*var\(--space-2\)\))[^}]*\}/s,
);
```

- [ ] **Step 2: Write the failing narrow-layout reset assertion**

Require the existing `max-width: 1099px` action rule to clear the offset:

```ts
assert.match(
  matchesCss,
  /@media \(max-width:\s*1099px\)\s*\{[\s\S]*?\.matchFavoriteActions\s*\{(?=[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\))(?=[^}]*margin-top:\s*0)[^}]*\}/s,
);
```

- [ ] **Step 3: Run the focused test and confirm RED**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchFavoritesUi.test.tsx
```

Expected: FAIL because the wide alignment and narrow reset declarations are absent.

### Task 2: Apply the minimal CSS fix

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Test: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`

- [ ] **Step 1: Align the wide action group to the textarea**

Add these declarations to the base `.matchFavoriteActions` rule:

```css
align-self: start;
margin-top: calc(21px + var(--space-2));
```

The `21px` value matches the existing 14px label at `line-height: 1.5`; `var(--space-2)` matches the existing label-to-textarea gap.

- [ ] **Step 2: Preserve the narrow stacked layout**

Add this declaration to `.matchFavoriteActions` inside `@media (max-width: 1099px)`:

```css
margin-top: 0;
```

- [ ] **Step 3: Run the focused test and confirm GREEN**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchFavoritesUi.test.tsx
```

Expected: PASS with no assertion error.

### Task 3: Verify the frontend and workspace

**Files:**
- Verify: `apps/owstats-desktop/src/styles/features/matches.css`
- Verify: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`

- [ ] **Step 1: Run the complete frontend test suite**

```powershell
cd apps\owstats-desktop
npm test
```

Expected: all resource, unit, component, legacy, and text-integrity tests pass.

- [ ] **Step 2: Build the frontend**

```powershell
cd apps\owstats-desktop
npm run build
```

Expected: TypeScript and Vite build successfully and distribution-path verification passes.

- [ ] **Step 3: Check patch integrity and scope**

```powershell
git diff --check
git status --short
```

Expected: no whitespace errors; only the two implementation files and the two task documents are new/modified by this task, while the pre-existing login backend changes remain untouched.

No staging or commit step is included because the repository instructions require preserving the user's dirty worktree unless explicitly requested.
