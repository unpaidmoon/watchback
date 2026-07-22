# Career Achievements Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace season-scoped frontend achievement derivation with an account-scoped career achievement query, add ten achievements and per-tier unlock dates, and simplify the achievement detail dialog.

**Architecture:** Add a focused Rust `review::achievements` module that loads normalized career match facts, evaluates all achievement definitions deterministically, and returns display-ready tier requirements plus first-unlock timestamps. Expose it through a dedicated Tauri command and a small frontend hook; the achievements page no longer consumes `PersonalSeasonReview` or a season query. Keep unlock state derived from existing match evidence instead of adding a mutable achievement ledger.

**Tech Stack:** Rust, rusqlite, serde/serde_json, Tauri 2, React, TypeScript, CSS, Node/tsx contract tests, PNG/WebP packaged assets.

---

## File map

- Create `apps/owstats-desktop/src-tauri/src/review/achievements.rs`: career fact loading, rule evaluation, requirement text, unlock timestamps.
- Modify `apps/owstats-desktop/src-tauri/src/review/mod.rs`: expose the career achievement query inside the review module.
- Modify `apps/owstats-desktop/src-tauri/src/models.rs`: add `CareerAchievement` and `CareerAchievementTier` DTOs.
- Modify `apps/owstats-desktop/src-tauri/src/commands.rs`: add the account-scoped Tauri command.
- Modify `apps/owstats-desktop/src-tauri/src/lib.rs`: export and register the command.
- Create `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`: synthetic cross-season rule, timestamp, idempotency, missing-field, and account-database tests.
- Modify `apps/owstats-desktop/src/shared/types.ts`: mirror the career achievement DTO.
- Modify `apps/owstats-desktop/src/shared/api.ts`: invoke `get_career_achievements` without a season argument.
- Create `apps/owstats-desktop/src/features/review/useCareerAchievements.ts`: independent loading, retry, stale-request guard, and error state.
- Modify `apps/owstats-desktop/src/pages/AchievementsPage.tsx`: remove season state and render career achievements.
- Modify `apps/owstats-desktop/src/features/review/AchievementWall.tsx`: consume backend achievement DTOs and show unlock dates per tier.
- Modify `apps/owstats-desktop/src/features/review/reviewNavigation.ts`: make the achievements URL season-independent.
- Retire `apps/owstats-desktop/src/shared/reviewAchievements.ts` after removing non-achievement consumers, or reduce it to presentation helpers only.
- Modify `apps/owstats-desktop/src/styles/features/review.css`: remove obsolete detail summary/progress styles and style unlock dates.
- Modify frontend tests in `apps/owstats-desktop/scripts/reviewAchievements.test.ts`, `reviewAchievementWall.test.ts`, `reviewNavigation.test.ts`, `phase8MultiSeasonPages.test.ts`, `reviewVisualPolish.test.ts`, and visual fixtures.
- Add 30 transparent PNGs for the ten new keys, remove the three `peak-token-tier-*.png` files, and update `assets/manifest.json`, `apps/owstats-desktop/runtime-config/assets-inventory.json`, and resource tests.

### Task 1: Define the backend career achievement contract

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/review/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Create: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`

- [ ] **Step 1: Add a failing command-contract test**

Add a test that creates an empty synthetic `OwDatabase`, calls `get_career_achievements`, and expects seventeen stable achievement keys in this order: the seven retained existing keys `win-streak`, `win-rate`, `s-grade`, `carry`, `signature`, `hero-focus`, `switching`, followed by `glass-cannon`, `damage-machine`, `bloodbath`, `comeback`, `godlike-run`, `iron-wall`, `rank-proof`, `battle-hardened`, `brothers-in-arms`, and `best-teammate`. Assert every result has exactly three tiers, every empty-database tier has `unlocked_at == None`, and `peak` is absent.

- [ ] **Step 2: Run the new test and verify RED**

Run:

```powershell
cd apps/owstats-desktop/src-tauri
cargo test --test career_achievements career_achievement_contract_is_stable
```

Expected: compilation fails because the DTO and query do not exist.

- [ ] **Step 3: Add DTOs and an empty evaluator shell**

Add these serializable DTO shapes to `models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerAchievementTier {
    pub level: i64,
    pub requirement: String,
    pub unlocked_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerAchievement {
    pub key: String,
    pub title: String,
    pub detail: String,
    pub current_level: i64,
    pub tiers: Vec<CareerAchievementTier>,
}
```

Expose `review::get_career_achievements(&OwDatabase)`, add `commands::get_career_achievements(State<AccountContextManager>)`, export it from `lib.rs`, and register it in `tauri::generate_handler!`. The initial evaluator must return all seventeen definitions with stable Chinese labels and three locked tiers.

- [ ] **Step 4: Run the contract test and verify GREEN**

Run the Task 1 test command again. Expected: one passing test.

### Task 2: Build a normalized, cross-season career fact stream

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`

- [ ] **Step 1: Add failing projection tests**

Seed synthetic matches in two seasons with deliberately duplicated sync input, current-player rows, friend rows, durations, `damage_taken`, rank JSON, and arena detail JSON. Assert the projection:

- orders facts by `(begin_ts, match_id)`;
- emits each `match_id` once;
- reads current-player `damage_taken` from `match_players`;
- counts a match with two friends as one friend match;
- exposes normalized rank order, role, season, result, duration, kills, assists, deaths and damage;
- exposes `arena_was_three_points_behind` only when a round-by-round score shows a deficit of at least three.

- [ ] **Step 2: Run projection tests and verify RED**

```powershell
cd apps/owstats-desktop/src-tauri
cargo test --test career_achievements career_fact
```

Expected: failure because `CareerMatchFact` and loaders are absent.

- [ ] **Step 3: Implement the fact loader**

Create a private `CareerMatchFact` containing only normalized values needed by achievements. Query `matches` once, left join the uniquely selected current-player projection, and use `EXISTS` for friend presence so friend counts cannot multiply rows. Read arena round history from `match_details.raw_detail_json`; reuse the established JSON field fallbacks for `teamScore`/`opponentScore`, and require final `result == "win"` for comeback qualification.

Use `BTreeMap<String, CareerMatchFact>` or SQL uniqueness keyed by `match_id` and finish with a deterministic `(begin_ts, match_id)` sort. Do not log match IDs, paths or raw JSON.

- [ ] **Step 4: Run projection tests and verify GREEN**

Run the Task 2 command again. Expected: all `career_fact` tests pass.

### Task 3: Implement the ten new achievement rules

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`

- [ ] **Step 1: Add table-driven failing tests for all thresholds**

For each new achievement, seed one fact immediately below and one fact exactly at each threshold. Assert exact boundary behavior and first qualifying timestamp:

```text
glass-cannon: 12000/8000, 18000/7000, 25000/6000
damage-machine per 10: 8000, 11000, 14000
bloodbath kills: 20, 35, 50
comeback qualifying wins: 1, 5, 15
godlike-run: 0 deaths plus 10, 20, 30 kills
iron-wall damage taken per 10: 10000, 14000, 18000
rank-proof: platinum, master, champion
battle-hardened wins: 50, 250, 1000
brothers-in-arms friend matches: 20, 100, 300
best-teammate assists: 15, 30, 45
```

Also assert that a 299-second match never qualifies for either per-ten-minute achievement and that glass-cannon/godlike conditions cannot be combined across different matches.

- [ ] **Step 2: Run new-rule tests and verify RED**

```powershell
cd apps/owstats-desktop/src-tauri
cargo test --test career_achievements new_achievement
```

Expected: threshold assertions fail against the locked shell definitions.

- [ ] **Step 3: Implement reusable evaluators**

Add small helpers with explicit responsibilities:

```rust
fn per_ten_minutes(value: i64, duration_sec: i64) -> Option<f64>;
fn first_match_unlock<F>(facts: &[CareerMatchFact], predicate: F) -> Option<i64>;
fn cumulative_unlock_dates<I>(dated_hits: I, thresholds: [i64; 3]) -> [Option<i64>; 3];
fn highest_rank_unlock_dates(facts: &[CareerMatchFact]) -> [Option<i64>; 3];
```

Each achievement definition supplies three complete Chinese `requirement` strings and uses the helpers to produce tier dates. Compute `current_level` as the highest tier with a non-null date. Keep thresholds as named constants beside each definition.

- [ ] **Step 4: Run new-rule tests and verify GREEN**

Run the Task 3 command again. Expected: all new achievement boundary tests pass.

### Task 4: Move the seven retained existing achievements to career scope

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`

- [ ] **Step 1: Add failing cross-season tests for existing achievements**

Cover the existing thresholds exactly:

- longest win streak `[3, 5, 8]`, calculated within a season and never concatenated across season boundaries;
- career win rate `[50, 55, 60]` percent;
- S-grade count `[5, 15, 30]`;
- carry count `[5, 12, 25]`;
- signature count `[3, 8, 15]` using the most frequent stable highlight tag;
- top-one hero share `[25, 40, 55]` percent;
- distinct heroes `[5, 10, 15]`.

Assert each tier receives the date of the historical event that first crossed it. For career win rate and hero share, replay facts in time order and take the first date at which the ratio reaches the threshold with at least the same minimum sample used by the production rule.

- [ ] **Step 2: Add a regression test that removes `peak`**

Assert the backend result, frontend key union, achievement order, wall copy, asset inventory and resource tests contain no `peak` or “巅峰一战”. Assert `buildPeakThresholds` is removed with the old frontend-derived builder.

- [ ] **Step 3: Run existing-rule tests and verify RED**

```powershell
cd apps/owstats-desktop/src-tauri
cargo test --test career_achievements existing_achievement
```

- [ ] **Step 4: Implement career evaluators for the retained existing set**

Reuse match evaluation rows for S-grade/carry/signature and the normalized fact stream for results, heroes and personal bests. Keep all queries account-local and cross-season. Generate full Chinese requirement strings in the backend.

- [ ] **Step 5: Run all career achievement tests and verify GREEN**

```powershell
cd apps/owstats-desktop/src-tauri
cargo test --test career_achievements
```

Expected: every career achievement test passes.

### Task 5: Add the frontend career query and remove season routing

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Create: `apps/owstats-desktop/src/features/review/useCareerAchievements.ts`
- Modify: `apps/owstats-desktop/src/pages/AchievementsPage.tsx`
- Modify: `apps/owstats-desktop/src/features/review/reviewNavigation.ts`
- Modify: `apps/owstats-desktop/src/shared/reviewAchievements.ts`
- Test: `apps/owstats-desktop/scripts/reviewAchievements.test.ts`
- Test: `apps/owstats-desktop/scripts/reviewNavigation.test.ts`
- Test: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`

- [ ] **Step 1: Change frontend tests to the career contract**

Assert:

- `getCareerAchievements()` invokes `get_career_achievements` with no season payload;
- `buildAchievementsHref()` returns `/achievements`;
- `AchievementsPage` does not import `PageSeasonSelect`, `parseReviewSeasonParam`, or `usePersonalSeasonReview`;
- the page header description is exactly `生涯成就与进度`;
- loading, error and empty states use career-achievement Chinese copy.

- [ ] **Step 2: Run targeted frontend tests and verify RED**

```powershell
cd apps/owstats-desktop
npx tsx scripts/reviewAchievements.test.ts
npx tsx scripts/reviewNavigation.test.ts
npx tsx scripts/phase8MultiSeasonPages.test.ts
```

- [ ] **Step 3: Implement the TypeScript contract and hook**

Add exact frontend DTOs matching Rust camelCase serialization. `useCareerAchievements` owns `achievements`, `error`, `loading`, `requestAchievements`, and a latest-request guard. It must not accept URL parameters or season state.

Update `AchievementsPage` to render:

```tsx
<PageHeader title="成就" description="生涯成就与进度" />
```

Render the existing empty-state action only when the returned list is empty; a valid list of locked achievements is not an empty state.

- [ ] **Step 4: Remove obsolete season-derived achievement code**

After confirming no remaining consumers, remove `buildReviewAchievements` and its `PersonalSeasonReview` coupling. Preserve only presentation helpers such as `activeBadgeLevel` and `romanLevels`, updated to accept `CareerAchievement`.

- [ ] **Step 5: Run targeted frontend tests and verify GREEN**

Run the Task 5 commands again. Expected: all three scripts pass.

### Task 6: Simplify the detail dialog and display unlock dates

**Files:**
- Modify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/review.css`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`
- Test: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`

- [ ] **Step 1: Add failing markup and style assertions**

Assert the dialog:

- renders only `achievement.title` in the header plus the close button;
- contains no `phase6Eyebrow`, header detail paragraph, `phase6AchievementModalStats`, `phase6AchievementModalProgress`, `Metric`, current, current level, next level or highest level copy;
- renders each tier's `requirement`;
- renders `formatAchievementUnlockDate(tier.unlockedAt)` only for non-null dates;
- keeps tier-2/tier-3 unlocked glow and locked dimming;
- uses a 12px tertiary unlock-date style with no colored pill or new surface.

- [ ] **Step 2: Run dialog tests and verify RED**

```powershell
cd apps/owstats-desktop
npx tsx scripts/reviewAchievementWall.test.ts
npx tsx scripts/reviewVisualPolish.test.ts
```

- [ ] **Step 3: Implement the simplified dialog**

Use `achievement.tiers.map`. The tier stage class is unlocked when `tier.unlockedAt !== null`. Format timestamps using a deterministic local-date helper that produces `YYYY.MM.DD 解锁` and never substitutes the current date for invalid input.

Remove obsolete summary/progress markup and styles. Keep dialog semantics, close-button focus, Escape handling, backdrop behavior, the 320×320 detail glow canvas, reduced motion, and the existing card-only hover state.

- [ ] **Step 4: Update anonymous visual fixtures**

Provide synthetic timestamps for unlocked tiers and null for locked tiers. Update fixtures so visual acceptance covers one level-I, one level-II, one level-III and one fully locked achievement without real account data.

- [ ] **Step 5: Run dialog tests and verify GREEN**

Run the Task 6 commands again. Expected: both scripts pass.

### Task 7: Produce and package the ten transparent badge sets

**Files:**
- Create: `assets/achievements/token-style/{key}-token-tier-{1,2,3}.png` for all ten new keys
- Delete: `assets/achievements/token-style/peak-token-tier-1.png`
- Delete: `assets/achievements/token-style/peak-token-tier-2.png`
- Delete: `assets/achievements/token-style/peak-token-tier-3.png`
- Modify: `assets/manifest.json`
- Modify: `apps/owstats-desktop/runtime-config/assets-inventory.json`
- Modify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`
- Reuse: `scripts/process_achievement_token_transparency.py`

- [ ] **Step 1: Generate one approved three-tier design preview per achievement**

Follow `docs/season-achievement-badge-production-workflow.md` and the visual directions in the approved design spec. Present each group for user approval before producing formal tier files. The group preview is never used as a production crop.

- [ ] **Step 2: Generate three isolated chroma-key sources per approved group**

For each key, create `tmp/achievement-transparent/<key>/tier-{1,2,3}-key.png` with a uniform `#ff00ff` background and open interior gaps. Do not bake outer glow, particles, shadow or a dark disc into the badge.

- [ ] **Step 3: Convert all approved sources to transparent PNGs**

Run for each of the ten exact keys:

```powershell
python scripts/process_achievement_token_transparency.py --group <key> --input-dir tmp/achievement-transparent/<key>
```

Expected per group: three 512×512 RGBA PNGs with transparent corners and a fully opaque subject.

- [ ] **Step 4: Update asset manifests and resource assertions**

Add all 30 new paths to the root manifest and runtime inventory. Remove the three `peak` paths and delete their files. Extend `phase6Assets.test.ts` to assert each retained/new key has tier 1, 2 and 3 files and that no `peak` asset remains packaged.

- [ ] **Step 5: Verify resources**

```powershell
cd apps/owstats-desktop
npm run prepare:assets
npm run test:resources
```

Expected: resources are copied to packaged runtime paths and all resource tests pass.

### Task 8: Full regression and release-boundary verification

**Files:**
- Verify all files changed by Tasks 1–7

- [ ] **Step 1: Run frontend tests and build**

```powershell
cd apps/owstats-desktop
npm test
npm run build
```

Expected: zero test failures and a successful Vite/TypeScript build.

- [ ] **Step 2: Run backend formatting and tests**

```powershell
cd apps/owstats-desktop/src-tauri
cargo fmt --check
cargo test --tests
```

Expected: formatting is clean and all Rust test binaries pass.

- [ ] **Step 3: Run visual acceptance with synthetic data**

Check 2540×1344, 1920×1080 and 1440×900. Verify the career header, no season selector, four achievement levels/states, per-tier dates, simplified header, removed bottom summary/progress, hover-only wall background, and tier glows. Do not use real account data in screenshots.

- [ ] **Step 4: Run the packaged application boundary check**

```powershell
cd apps/owstats-desktop
npm run tauri build
```

Expected: the installer build succeeds and every new badge resolves from packaged resources.

- [ ] **Step 5: Inspect scope and whitespace**

```powershell
cd ../../
git diff --check
git status --short
```

Expected: no whitespace errors; only the intended achievement, asset, test and documentation files are newly changed by this implementation. Preserve all unrelated pre-existing worktree changes.

## Execution notes

- Do not add an achievement persistence table unless deterministic replay proves impossible for a specific rule; the approved architecture derives state from existing evidence.
- Do not read or print local real-player payloads while implementing or testing. Use synthetic SQLite fixtures only.
- Do not stage or commit unrelated dirty-worktree files. Commits are optional and require explicit user authorization under this repository's collaboration rules.
- Asset approval is a user checkpoint; backend and frontend tests may use synthetic asset-path strings, but completion cannot be claimed until formal transparent assets are packaged.
