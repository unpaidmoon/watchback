# S21 Match Hero Avatar Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve legacy S21 numeric hero identifiers to bundled hero avatars without changing stored match data.

**Architecture:** Extend the existing frontend runtime lookup shape with its hero dictionary. During manifest lookup creation, map each numeric lookup key through its Chinese hero name, then rely on the existing manifest index to resolve the stable asset key. Existing name/key/alias behavior remains unchanged.

**Tech Stack:** React, TypeScript, Vite, Node assertion scripts.

---

### Task 1: Lock the legacy-ID behavior with a regression test

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`

- [x] **Step 1: Add the failing assertion**

```ts
assertEqual(
  heroAssetUrl("207165582859043131"),
  "/assets/heroes/ana.png",
  "legacy numeric hero id maps through the bundled runtime lookup",
);
```

- [x] **Step 2: Run the focused test and confirm it fails**

Run: `npx tsx scripts/phase6Assets.test.ts`

Expected: assertion failure because the current resolver produces `/assets/heroes/207165582859043131.png`.

### Task 2: Resolve runtime dictionary IDs through the manifest

**Files:**
- Modify: `apps/owstats-desktop/src/shared/assets.ts`

- [x] **Step 1: Extend the runtime lookup type**

```ts
type QueryToolConfig = {
  heroes?: Record<string, { name?: string }>;
  heroPerkList?: Record<string, QueryToolPerk[]>;
  perkList?: QueryToolPerk[];
  heroAttrList?: QueryToolHeroAttr[];
};
```

- [x] **Step 2: Add numeric hero IDs to the existing hero lookup**

```ts
for (const [guid, hero] of Object.entries(queryTool.heroes ?? {})) {
  const key = heroLookup.get(hero.name?.trim().toLowerCase() ?? "");
  if (key) heroLookup.set(guid, key);
}
```

- [x] **Step 3: Run the focused regression test and confirm it passes**

Run: `npx tsx scripts/phase6Assets.test.ts`

Expected: exit code 0.

### Task 3: Run affected frontend verification

**Files:**
- Verify only: `apps/owstats-desktop/scripts/phase6Assets.test.ts`
- Verify only: `apps/owstats-desktop`

- [x] **Step 1: Run frontend tests**

Run: `npm test`

Expected: exit code 0.

- [x] **Step 2: Run the production build**

Run: `npm run build`

Expected: exit code 0 and no absolute-path scan failure.

- [x] **Step 3: Inspect the patch**

Run: `git diff --check` and `git status --short`

Expected: no whitespace errors; only the intentionally changed source, test, and design/plan files are new relative to the starting worktree.
