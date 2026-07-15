# Command Center Dynamic Season Sync Ledger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the fixed S21/S22/S23 command-center selector and selected-season card with an S15-to-current-season dropdown on the left and a real-data-only synced-season ledger on the right.

**Architecture:** Rust owns the single season catalog through `MIN_SYNC_SEASON` and `CURRENT_SEASON`, serializes `currentSeason` plus `syncableSeasons`, and returns `seasonStatuses` only for seasons present in SQLite. React keeps selection local to `OnboardingPage`，不引入全局赛季状态；页面 delegates the left control and right ledger to a focused command-center component, and reuses the existing `syncSeason` command, account gate, cache invalidation, and status refresh flow.

**Tech Stack:** Tauri 2, React, TypeScript, Rust, SQLite, Node/tsx source-contract tests, Cargo integration tests, CSS.

---

## File Structure

- Modify `apps/owstats-desktop/src-tauri/src/models.rs`: add serialized season catalog fields to `OnboardingStatus`.
- Modify `apps/owstats-desktop/src-tauri/src/review/command_center.rs`: define the S15/current-season catalog and return descending real-data season rows.
- Modify `apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs`: lock the dynamic catalog, empty database, descending order, and no fabricated rows.
- Modify `apps/owstats-desktop/src/shared/types.ts`: mirror `currentSeason` and `syncableSeasons`.
- Create `apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx`: own the left dropdown/control and right ledger rendering.
- Modify `apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx`: retain only shared timestamp/data helpers still consumed by the command center.
- Modify `apps/owstats-desktop/src/pages/OnboardingPage.tsx`: initialize and preserve dynamic selection, remove the fixed array and duplicate header sync action, and compose the new workbench.
- Modify `apps/owstats-desktop/src/styles/features/command-center.css`: implement the 1:2 workbench, ledger rows, focus/selected/syncing/empty states, and stacked responsive layout.
- Modify `apps/owstats-desktop/scripts/phase6Shell.test.ts`: replace fixed-segment/card assertions with dynamic dropdown/ledger and rendering-helper contracts.
- Modify `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`: update the command-center product surface contract without weakening account/target/privacy assertions.
- Modify `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`: add new DTO fields and preserve anonymous multi-season rows.

### Task 1: Add the Backend Season Catalog Contract

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/review/command_center.rs`

- [ ] **Step 1: Write failing Rust integration assertions**

Add assertions that an empty database still exposes the complete catalog while keeping the ledger empty:

```rust
assert_eq!(status.current_season, 23);
assert_eq!(status.syncable_seasons, (15..=23).rev().collect::<Vec<_>>());
assert!(status.season_statuses.is_empty());
```

Extend the multi-season test with S15, S20, and S23 rows and assert descending ledger order:

```rust
let seasons = status
    .season_statuses
    .iter()
    .map(|item| item.season)
    .collect::<Vec<_>>();
assert_eq!(seasons, vec![23, 20, 15]);
```

- [ ] **Step 2: Run the focused Rust test and verify RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test phase8_command_center
```

Expected: compilation fails because `OnboardingStatus` does not yet contain `current_season` or `syncable_seasons`, or the ordering assertion fails against ascending SQL output.

- [ ] **Step 3: Add the serialized model fields**

Update `OnboardingStatus`:

```rust
pub struct OnboardingStatus {
    pub selected_season: Option<i64>,
    pub current_season: i64,
    pub syncable_seasons: Vec<i64>,
    pub available_seasons: Vec<i64>,
    // existing fields unchanged
}
```

- [ ] **Step 4: Implement the single catalog source and real-data-only ledger**

In `review/command_center.rs`, add:

```rust
pub(super) const MIN_SYNC_SEASON: i64 = 15;
pub(super) const CURRENT_SEASON: i64 = 23;

fn syncable_seasons() -> Vec<i64> {
    (MIN_SYNC_SEASON..=CURRENT_SEASON).rev().collect()
}
```

Query available seasons in descending order, remove the existing selected-season fabrication block, and construct the DTO with:

```rust
let selected = request
    .season
    .filter(|season| (MIN_SYNC_SEASON..=CURRENT_SEASON).contains(season))
    .or(Some(CURRENT_SEASON));
let season_statuses = command_center_season_statuses(db, &available_seasons)?;

Ok(OnboardingStatus {
    selected_season: selected,
    current_season: CURRENT_SEASON,
    syncable_seasons: syncable_seasons(),
    available_seasons,
    // existing fields unchanged
})
```

- [ ] **Step 5: Run focused and model contract tests and verify GREEN**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test phase8_command_center
cargo test --test stab006_contracts
```

Expected: both test binaries pass with zero failures.

- [ ] **Step 6: Commit the backend contract slice**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/review/command_center.rs apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs apps/owstats-desktop/src-tauri/tests/stab006_contracts.rs
git commit -m "feat: expose dynamic command center seasons"
```

### Task 2: Build the Dynamic Sync Control and Ledger Component

**Files:**
- Create: `apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx`
- Modify: `apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx`
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] **Step 1: Write failing source and helper contracts**

Load `SeasonSyncWorkbench.tsx` in `phase6Shell.test.ts` and assert:

```ts
assertEqual(workbenchSource.includes('aria-label="选择同步赛季"'), true, "dynamic season select has a Chinese accessible name");
assertEqual(workbenchSource.includes("syncableSeasons.map"), true, "season options come from the backend catalog");
assertEqual(workbenchSource.includes("已同步数据"), true, "ledger has the confirmed Chinese heading");
assertEqual(workbenchSource.includes("详情覆盖度"), true, "ledger shows coverage");
assertEqual(workbenchSource.includes("本地对局"), true, "ledger shows local matches");
assertEqual(workbenchSource.includes("最近同步"), true, "ledger shows compact time");
assertEqual(workbenchSource.includes("普通对局"), false, "ledger omits non-approved columns");
assertEqual(workbenchSource.includes("角斗对局"), false, "ledger omits non-approved columns");
```

Add pure helper assertions for stable selection:

```ts
assertEqual(resolveCommandCenterSeason(21, [23, 22, 21, 20, 19, 18, 17, 16, 15], 23), 21, "valid user selection survives refresh");
assertEqual(resolveCommandCenterSeason(24, [23, 22, 21, 20, 19, 18, 17, 16, 15], 23), 23, "invalid selection falls back to current season");
```

- [ ] **Step 2: Run the focused frontend test and verify RED**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
```

Expected: module/file resolution or new assertions fail because the workbench and resolver do not exist.

- [ ] **Step 3: Create the focused component**

Define props that keep network and account coordination in the page:

```ts
type SeasonSyncWorkbenchProps = {
  selectedSeason: number;
  currentSeason: number;
  syncableSeasons: number[];
  seasonStatuses: SeasonDataStatus[];
  credentialReady: boolean;
  targetSyncAllowed: boolean;
  accountOperationBusy: boolean;
  syncingSeason: number | null;
  progress: SyncProgress | null;
  onSelectSeason: (season: number) => void;
  onSync: (season: number) => void;
};
```

Export the selection resolver:

```ts
export function resolveCommandCenterSeason(
  requested: number,
  syncableSeasons: number[],
  currentSeason: number,
) {
  return syncableSeasons.includes(requested) ? requested : currentSeason;
}
```

Render a labeled native `<select>`, one primary `同步 S{n}` button, local progress/error hint slot, and a semantic ledger with rows sorted by descending `season`. Each ledger row must be a real keyboard-focusable button with `aria-pressed={selectedSeason === seasonStatus.season}`; activation calls `onSelectSeason(seasonStatus.season)` and never calls `onSync`. When `seasonStatuses.length === 0`, render the confirmed 空状态 copy exactly.

- [ ] **Step 4: Preserve only shared helpers in `SeasonHealthPanel.tsx`**

Remove the obsolete selected-season `SeasonStatusCard` export after all call sites move. Keep `DataFact`, `formatTimestamp`, `formatCompactTimestamp`, and a gate-copy helper used by the new control. The gate helper must preserve existing credential/target/account-operation precedence.

- [ ] **Step 5: Run the focused frontend test and verify GREEN**

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
```

Expected: all command-center helper and source contracts pass.

- [ ] **Step 6: Commit the component slice**

```powershell
git add -- apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx apps/owstats-desktop/scripts/phase6Shell.test.ts
git commit -m "feat: add synced season ledger"
```

### Task 3: Integrate Dynamic Selection Into the Command Center

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`

- [ ] **Step 1: Update failing product-surface assertions**

Replace fixed segment assertions with:

```ts
assert.doesNotMatch(onboarding, /COMMAND_CENTER_SEASONS/, "command center removes the fixed season array");
assert.match(onboarding, /status\?\.syncableSeasons/, "command center consumes backend syncable seasons");
assert.match(onboarding, /<SeasonSyncWorkbench/, "command center renders the dynamic sync workbench");
assert.doesNotMatch(onboarding, /commandCenterSeasonSegments/, "fixed season segments are removed");
assert.doesNotMatch(onboarding, /同步全部/, "command center keeps single-season sync only");
```

- [ ] **Step 2: Run the product-surface test and verify RED**

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase8RealFetch.test.ts
```

Expected: fixed-array and old component assertions fail against the current page.

- [ ] **Step 3: Mirror the backend DTO**

Add to `OnboardingStatus` in `shared/types.ts`:

```ts
currentSeason: number;
syncableSeasons: number[];
```

- [ ] **Step 4: Replace page-local fixed season state**

Initialize `seasonInput` without a hard-coded S23 assumption, then reconcile it after status loads:

```ts
const [selectedSeason, setSelectedSeason] = useState<number | null>(null);

useEffect(() => {
  if (!status) return;
  setSelectedSeason((current) => resolveCommandCenterSeason(
    current ?? status.currentSeason,
    status.syncableSeasons,
    status.currentSeason,
  ));
}, [status]);
```

Remove `COMMAND_CENTER_SEASONS`, the synthetic selected `seasonStatuses` array, the header-level sync button, and the inline old picker/card markup. Render `SeasonSyncWorkbench` with the existing real credential, target, lock, progress, and `runSeasonSync` values.

- [ ] **Step 5: Run focused frontend tests and TypeScript build**

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/phase8RealFetch.test.ts
npm run build
```

Expected: both focused tests pass and `tsc && vite build` exits zero. If `tsc` reports fixture object shape errors, update only those `OnboardingStatus` fixtures with `currentSeason: 23` and `syncableSeasons: [23, 22, 21, 20, 19, 18, 17, 16, 15]`.

- [ ] **Step 6: Commit the page integration slice**

```powershell
git add -- apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/scripts/phase8RealFetch.test.ts apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts
git commit -m "feat: integrate dynamic season sync workbench"
```

### Task 4: Implement the Confirmed Ledger Visual System

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] **Step 1: Add failing visual contracts**

Assert the confirmed layout tokens and responsive behavior:

```ts
assertEqual(/\.seasonSyncWorkbench\s*\{[\s\S]*?grid-template-columns:\s*minmax\(220px, 1fr\) minmax\(0, 2fr\)/.test(commandCenterStyles), true, "sync controls and ledger use the confirmed 1:2 layout");
assertEqual(/\.syncedSeasonLedgerRow\s*\{[\s\S]*?min-height:\s*44px/.test(commandCenterStyles), true, "ledger rows meet the interaction target");
assertEqual(/\.syncedSeasonCoverageTrack[^}]*background:\s*var\(--data-primary\)/.test(commandCenterStyles), true, "coverage uses the approved data color");
assertEqual(/@media \(max-width: 920px\)[\s\S]*?\.seasonSyncWorkbench\s*\{[\s\S]*?grid-template-columns:\s*1fr/.test(commandCenterStyles), true, "narrow layout stacks control above ledger");
```

- [ ] **Step 2: Run the focused visual contract and verify RED**

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
```

Expected: the new layout selectors do not exist.

- [ ] **Step 3: Implement the command-center CSS**

Add a one-surface workbench with a left divider, a four-column ledger header/row, 44px minimum interactive rows, tabular figures, purple coverage track, subtle selected row indicator, visible 键盘 focus ring, and no nested scrolling. Remove obsolete `.commandCenterSeasonSegments` and selected-season-card overrides.

Use the established tokens only:

```css
.seasonSyncWorkbench {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) minmax(0, 2fr);
  background: rgba(250, 251, 247, 0.38);
}

@media (max-width: 920px) {
  .seasonSyncWorkbench { grid-template-columns: 1fr; }
}
```

- [ ] **Step 4: Run focused tests and verify GREEN**

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/phase8RealFetch.test.ts
```

Expected: both scripts exit zero.

- [ ] **Step 5: Commit the visual slice**

```powershell
git add -- apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/phase6Shell.test.ts
git commit -m "style: lay out synced season ledger"
```

### Task 5: Full Verification and Privacy Review

**Files:**
- No planned modifications; verification corrections are restricted to the exact implementation and test files listed in Tasks 1-4.

- [ ] **Step 1: Run frontend verification**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: all frontend resource, unit, component, legacy, and text tests pass; build exits zero. Record any existing non-fatal chunk warning separately.

- [ ] **Step 2: Run backend verification**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: formatting passes and all non-ignored integration tests pass.

- [ ] **Step 3: Run repository hygiene checks**

```powershell
cd ..\..\..
git diff --check
git status --short
rg -n "COMMAND_CENTER_SEASONS|\[21, 22, 23\]" apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/features/command-center
rg -n -i "customerToken|authorization|OW_DASHEN_TOKEN" apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx
```

Expected: diff check passes; status contains only intended task files; fixed season scan and sensitive-field scan return no product-surface matches.

- [ ] **Step 4: Perform visual acceptance**

Use anonymous fixture data at `2540x1344`, `1920x1080`, and `1440x900`. Verify left-only controls, right-only local data, descending S23/S20/S15 rows, empty state, existing-season resync state, keyboard focus, and the stacked narrow layout. 隐私验收不得使用 real credentials, browser profiles, player privacy IDs, or real payload dumps.

- [ ] **Step 5: Commit any direct verification corrections**

If verification required scoped fixes, stage the exact implementation/test paths that Tasks 1-4 authorize and commit:

```powershell
git add -- apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/review/command_center.rs apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/scripts/phase8RealFetch.test.ts apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts
git commit -m "fix: complete dynamic season ledger verification"
```

If no corrections were needed, do not create an empty commit.
