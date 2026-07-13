# UI Visual and Release Acceptance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Browser verification must also use the `playwright` skill.

**Goal:** Prove the redesigned OWStatsLocal shell and all five primary pages satisfy the controlling visual, interaction, accessibility, privacy, build, and packaging contracts at the required desktop viewports.

**Architecture:** Automated source contracts cover semantic tokens, focus, reduced motion, page states, Chinese copy, and forbidden runtime dependencies. A sanitized browser acceptance harness supplies synthetic Tauri responses for deterministic page-state and screenshot checks, while a separate native Tauri build/smoke pass proves packaging without reading real app data or credentials.

**Tech Stack:** TypeScript, React 18, Vite 6, Tauri 2, Rust, source-contract tests via `tsx`, Playwright CLI, PowerShell.

---

### Task 1: Lock cross-page accessibility and visual-system contracts

**Files:**
- Create: `apps/owstats-desktop/scripts/uiVisualContracts.test.ts`
- Modify: `apps/owstats-desktop/package.json`
- Read: `apps/owstats-desktop/src/styles/tokens.css`
- Read: `apps/owstats-desktop/src/styles/base.css`
- Read: `apps/owstats-desktop/src/styles/shell.css`
- Read: `apps/owstats-desktop/src/styles/page-shell.css`
- Read: `apps/owstats-desktop/src/styles/features/*.css`

- [ ] **Step 1: Write the failing visual-contract test**

Add a source-contract test that reads the style entry points and page sources, then asserts the exact controlling tokens, system-only font stack, global focus ring, reduced-motion media query, semantic state markup, and the absence of remote font imports or development-machine URLs. The central assertions must include:

```ts
assert.match(styles, /--app-canvas:\s*#E3E8E2/i);
assert.match(styles, /--action-primary:\s*#303030/i);
assert.match(styles, /--data-primary:\s*#4B22D1/i);
assert.match(styles, /--data-secondary:\s*#2F8C95/i);
assert.match(styles, /:focus-visible[\s\S]*2px[\s\S]*var\(--data-primary\)/);
assert.match(styles, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
assert.doesNotMatch(styles, /fonts\.googleapis\.com|https?:\/\/[^)'"\s]+\.(?:woff2?|ttf|otf)/i);
assert.doesNotMatch(styles, /\/@fs\/|[A-Z]:\\(?:Codex|Users)\\/i);
assert.doesNotMatch(styles, /linear-gradient\([^)]*(?:orange|#f59|#fb9)[^)]*(?:blue|#2f6|#3b8)/i);
```

For each primary page source, assert a shared `PageHeader` is present; for loading/error/success status sources, assert `aria-busy`, `role="alert"`, or `aria-live` is present as appropriate.

- [ ] **Step 2: Run the test and verify RED**

Run:

```powershell
cd apps/owstats-desktop
npx tsx scripts/uiVisualContracts.test.ts
```

Expected: FAIL because at least the legacy orange/blue shell declarations, shared page-state integration, or reduced-motion contract is still missing.

- [ ] **Step 3: Finish the smallest remaining cross-page fixes**

Only fix gaps left by the feature plans. Do not redesign page content here. Typical minimal fixes are semantic attributes or replacing a remaining raw legacy color with an existing token:

```tsx
<div className="pageStatusMessage" role="status" aria-live="polite">
  {message}
</div>
```

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    scroll-behavior: auto !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

- [ ] **Step 4: Add the contract test to the unit test chain and verify GREEN**

Add `tsx scripts/uiVisualContracts.test.ts` to `test:unit`, then run:

```powershell
npm run test:unit
npm run test:text
```

Expected: both commands PASS with no corrupted Chinese copy and no contract failure.

- [ ] **Step 5: Commit the cross-page contract slice**

```powershell
git status --short
git diff --check
git add apps/owstats-desktop/scripts/uiVisualContracts.test.ts apps/owstats-desktop/package.json apps/owstats-desktop/src
git commit -m "test(ui): lock visual accessibility contracts"
```

Before committing, inspect the staged diff and remove any file not required for this task.

### Task 2: Build a deterministic sanitized browser acceptance harness

**Files:**
- Create: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`
- Create: `apps/owstats-desktop/scripts/uiVisualAcceptance.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Write the failing fixture-privacy test**

The fixture module must export only synthetic, non-identifying DTOs for shell, matches, normal detail, arena detail, review, MMR, ranking, and command-center states. The test serializes every fixture and rejects sensitive field names and raw-looking player identifiers:

```ts
const serialized = JSON.stringify(allVisualFixtures);
assert.doesNotMatch(serialized, /token|customerToken|cookie|authorization|credential|browser[_-]?profile/i);
assert.doesNotMatch(serialized, /\b\d{16,}\b/);
assert.match(serialized, /示例玩家/);
assert.match(serialized, /普通对局/);
assert.match(serialized, /角斗竞技/);
```

Run:

```powershell
cd apps/owstats-desktop
npx tsx scripts/uiVisualAcceptance.test.ts
```

Expected: FAIL because the fixture module does not exist.

- [ ] **Step 2: Implement the minimal synthetic fixture module**

Use stable visible values such as `示例玩家`, `训练队友`, `训练对手`, `S23`, `香巴里寺院`, and `花冈`. Use clearly synthetic match IDs such as `fixture-normal-1` and `fixture-arena-1`; never copy values from local SQLite, app data, browser storage, or a real data dump.

The module should expose a command-to-response map usable by a Playwright init script:

```ts
export const visualInvokeResponses: Record<string, unknown> = {
  get_overview: fixtureOverview,
  list_match_filters: fixtureFilters,
  list_matches: fixtureMatches,
  get_match_details: fixtureNormalDetail,
  get_personal_season_review: fixtureReview,
  get_season_analysis: fixtureAnalysis,
  get_career_mmr_overview: fixtureCareerOverview,
  get_rankings: fixtureRanking,
  get_onboarding_status: fixtureOnboarding,
};
```

- [ ] **Step 3: Verify fixture GREEN and add it to `test:unit`**

```powershell
npx tsx scripts/uiVisualAcceptance.test.ts
npm run test:unit
```

Expected: PASS; serialized fixture output contains no forbidden key or raw-looking long numeric identifier.

- [ ] **Step 4: Commit the sanitized acceptance fixture**

```powershell
git status --short
git diff --check
git add apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts apps/owstats-desktop/scripts/uiVisualAcceptance.test.ts apps/owstats-desktop/package.json
git commit -m "test(ui): add sanitized visual fixtures"
```

### Task 3: Run focused functional regression suites

**Files:**
- Verify only; no production files should change.

- [ ] **Step 1: Run the shell and page-state suites**

```powershell
cd apps/owstats-desktop
npx tsx scripts/uiArchitectureShell.test.ts
npx tsx scripts/pageShellComponents.test.tsx
npx tsx scripts/uiArchitectureTokens.test.ts
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/phase8MultiSeasonPages.test.ts
```

Expected: all PASS.

- [ ] **Step 2: Run the match workbench suites**

```powershell
npx tsx scripts/matchesPageCache.test.ts
npx tsx scripts/filterBarUi.test.ts
npx tsx scripts/matchListFormat.test.ts
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/arenaDetailFormat.test.ts
```

Expected: all PASS, including normal-detail and arena-detail embedded branch contracts.

- [ ] **Step 3: Run review, MMR, ranking, and command-center suites**

```powershell
npx tsx scripts/analysisMovedToReview.test.ts
npx tsx scripts/friendBoardsReviewUi.test.ts
npx tsx scripts/careerMmrUi.test.ts
npx tsx scripts/rankingUtils.test.ts
npx tsx scripts/rankingDisplayNames.test.ts
npx tsx scripts/phase8RealFetch.test.ts
```

Expected: all PASS. `careerMmrUi.test.ts` must prove the recent single-match request and UI are absent; ranking tests must prove sorting, search, scoring, and aggregation contracts are unchanged.

- [ ] **Step 4: Run the full frontend suite**

```powershell
npm test
npm run build
```

Expected: PASS. The build path scan must report no `/@fs/`, developer drive path, or `docs/reference` dependency.

### Task 4: Perform browser visual and interaction acceptance at required viewports

**Files:**
- Create outside the repository: `C:\Users\cyb\.codex\visualizations\2026\07\14\ui-architecture-visual-redesign\`
- Do not commit screenshots, `.playwright-cli`, `test-results`, traces, or browser profiles.

- [ ] **Step 1: Read and activate the Playwright skill, then start the Vite app**

Run the development server from the product directory without accessing Tauri app data:

```powershell
cd apps/owstats-desktop
npm run dev -- --host 127.0.0.1
```

Use a hidden/background process and record the actual localhost URL. Inject the synthetic command responses before app code executes; do not point the browser at a native user profile.

- [ ] **Step 2: Verify navigation and every primary page at `2540x1344`**

Capture sanitized screenshots for `/matches`, `/review?season=23&view=overview`, `/career-mmr`, `/ranking?season=23&tab=heroes`, and `/onboarding`. Check:

- root redirects to `/matches`;
- navigation order is exact and command center is last;
- page title and page season controls align consistently;
- gray-green canvas, charcoal actions, purple/cool-teal data accents, card radius, border, shadow, and spacing match the control design;
- no orange primary color or blue-orange gradient remains;
- content is centered and does not stretch beyond the intended wide layout.

- [ ] **Step 3: Verify the same pages at `1920x1080` and `1440x900`**

At each viewport, check no horizontal page scroll, no clipped top navigation, no hidden page actions, and no card collision. Save one screenshot per primary page per viewport plus focused screenshots for the match filters, normal embedded detail, arena embedded detail, MMR chart, ranking table, and command-center sync/health split.

- [ ] **Step 4: Verify match interaction and restoration**

Using the synthetic normal and arena rows:

1. Apply a frequent filter, open advanced filters, add hero/map/date filters, remove one chip, then clear all.
2. Select a normal match, scroll the list and detail independently, move to the next match, leave the page, and return.
3. Confirm URL filters, selected `matchId`, list anchor, and scroll position restore.
4. Select the arena row and verify summary/round switch, fixed slots, row expansion, raw unmapped mod/trait GUID exception, and no normal-only perk strip.
5. At a viewport below `1100px`, verify the explicit list-to-detail flow and visible return action instead of two compressed columns or stacked full panes.

- [ ] **Step 5: Verify review, MMR, ranking, and command-center interaction**

1. Change each of the five review tabs and confirm `view` plus `season` stay in the URL without duplicate same-season requests.
2. Follow the map link to ranking and the friend link to friend detail, then go back and confirm source tab, season, and scroll restore.
3. Confirm MMR has three role cards, composite summary, trend with axis/unit/hover value/text summary, confidence band, mode contribution, and full-width season breakdown; verify no recent single-match panel or request.
4. Confirm ranking search/sort/tab behavior is unchanged and the selected column remains visibly indicated.
5. Confirm command-center login/account, season sync, data health, tools, and destructive logout are visually separated; all disabled/syncing states are explicit.

- [ ] **Step 6: Verify keyboard focus and reduced motion**

Use Tab/Shift+Tab/Enter/Escape without a mouse. Confirm logical focus order, visible purple focus rings, no keyboard trap, advanced-filter escape route, and accessible names on icon-only controls. Emulate `prefers-reduced-motion: reduce` and confirm transitions collapse without hiding state changes or blocking interaction.

- [ ] **Step 7: Review screenshots against the controlling design and iterate**

Compare typography hierarchy, canvas, navigation capsule, whitespace, metric bands, card density, control styling, and data tables with the control design and the available local reference image. Any obvious mismatch returns to the owning feature plan with a new failing regression assertion before a code change. Do not accept a visually wrong page merely because tests pass.

### Task 5: Run native, Rust, packaging, and repository verification

**Files:**
- Verify only unless a failure reveals a tested product defect.

- [ ] **Step 1: Run formatting and backend tests**

```powershell
cd apps/owstats-desktop/src-tauri
cargo fmt --check
cargo test --tests
```

Expected: PASS. Report the real-credential/upstream integration test as ignored; do not force-run it.

- [ ] **Step 2: Build the Windows Tauri bundles**

```powershell
cd ..
npm run tauri build
```

Expected: PASS with installers/bundles under `src-tauri/target/release/bundle`. Do not stage or commit build output.

- [ ] **Step 3: Run the repository verification entry point**

From the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: PASS for privacy/path scans, asset preparation, frontend layers, build artifact scan, Rust format/Clippy/tests, STAB contracts, `git diff --check`, and tracked-only checkout acceptance.

- [ ] **Step 4: Run final diff and status checks**

```powershell
git diff --check
git status --short
git log --oneline --decorate -12
```

Expected: no unstaged implementation changes and only the intentional sequence of focused commits on `codex/ui-architecture-visual-redesign`. Confirm no screenshot, browser profile, local database, generated `public/assets`, `dist`, `target`, or test-results directory is tracked.

- [ ] **Step 5: Produce the final evidence report**

List each implementation slice, commit hash, changed files, exact commands and results, three-viewport visual evidence, interaction/accessibility results, ignored real-credential test, warnings, remaining unverified items, BattleTag integration status, and the statement: `未读取或接触真实凭据、浏览器 profile、真实数据 dump 或原始玩家隐私 ID。`
