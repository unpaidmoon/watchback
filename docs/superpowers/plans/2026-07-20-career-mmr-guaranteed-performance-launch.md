# Career MMR Guaranteed Performance Launch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship career MMR v3 with a non-zero personal-performance correction selected per role from learned, strongly-shrunk, or robust-percentile models, using all valid ordinary and stadium data.

**Architecture:** Repair stadium identity projection at the training-source boundary, train two shared parameter families while retaining per-mode baselines and caps, and compile every role into a validated runtime artifact with a guaranteed fallback tier. Activate v3 transactionally while retaining v2 for rollback; runtime replay consumes only a validated packaged artifact and never trains from user data.

**Tech Stack:** Rust, rusqlite/SQLite, serde/serde_json, existing career_mmr training and replay modules, Tauri 2, React/TypeScript.

**Repository constraint:** The working tree contains user changes. Do not stage or commit during execution unless the user explicitly authorizes it; the commit checkpoints below are review boundaries, not automatic authorization.

---

## File Structure

- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs`: define the two shared training families and per-mode runtime correction eligibility.
- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/training_sources.rs`: carry account BattleTag only inside the read-only training source.
- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/training.rs`: resolve stadium identity through `arena_match_players` with an exact uniqueness rule.
- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/performance_stability.rs`: encode balanced tier-one gates without manufacturing eligibility.
- Create `apps/owstats-desktop/src-tauri/src/career_mmr/performance_selection.rs`: select learned, shrunk, or percentile artifacts per role and validate universal invariants.
- Create `apps/owstats-desktop/src-tauri/src/career_mmr/performance_runtime.rs`: evaluate a selected artifact deterministically for one normalized row.
- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/training_report.rs`: serialize tiered artifacts and anonymous selection evidence.
- Modify `apps/owstats-desktop/src-tauri/src/bin/train_career_mmr.rs`: emit the v3 artifact only after all roles select a valid tier.
- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs`: apply v3 tier/mode caps without reversing the outcome delta.
- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/replay.rs`: transactionally build and activate v3 while retaining v2.
- Modify `apps/owstats-desktop/src-tauri/src/career_mmr/query.rs` and `apps/owstats-desktop/src-tauri/src/lib.rs`: expose active version and Chinese explanation factors.
- Modify focused `apps/owstats-desktop/src-tauri/tests/career_mmr_*.rs` integration tests listed below.

### Task 1: Lock the Stadium Identity and Shared-Family Contract

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_training_projection.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_multi_account_training.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training_sources.rs`

- [ ] **Step 1: Write failing mode-family tests**

Add assertions that the four ordinary modes return `TrainingCohort::RoleQueue`, both stadium modes return `TrainingCohort::StadiumCompetitive`, and `trains_performance()` is true for all six modes. Add a separate runtime-policy assertion that stadium quick has a `0.10` absolute tier-two cap and `0.05` tier-three cap instead of treating training eligibility as runtime eligibility.

- [ ] **Step 2: Run the mode test and verify RED**

Run:

```powershell
cargo test --test career_mmr_inputs modes_have_stable_names_training_cohorts_and_training_flags
```

Expected: FAIL because the existing test still expects `OutcomeOnly` or because the new runtime cap API does not exist.

- [ ] **Step 3: Write failing stadium uniqueness tests**

In `career_mmr_training_projection.rs`, retain the existing zero-`match_players` success fixture and add two cases:

```rust
#[test]
fn stadium_identity_rejects_duplicate_battle_tag_rows() -> Result<()> {
    // Insert two arena_match_players rows with the account BattleTag.
    // build_training_projection_for_account(..., Some(tag)) must emit no stadium row.
    // stadium_ambiguous_excluded must increase by one.
}

#[test]
fn stadium_identity_does_not_fall_back_to_another_player() -> Result<()> {
    // Insert only a different BattleTag.
    // The stadium match must be excluded rather than using the first arena row.
}
```

- [ ] **Step 4: Run the projection tests and verify RED**

Run:

```powershell
cargo test --test career_mmr_training_projection stadium_identity
```

Expected: at least the duplicate/mismatch case FAILS if the SQL does not enforce exact uniqueness.

- [ ] **Step 5: Implement the minimal input and projection contract**

Keep a single ordinary shared cohort and a single stadium shared cohort. Preserve the existing parameterized SQL shape:

```sql
SELECT a.bnet_id
FROM arena_match_players a
WHERE a.match_id=m.match_id AND a.player_name=?1
  AND (SELECT COUNT(*) FROM arena_match_players x
       WHERE x.match_id=m.match_id AND x.player_name=?1)=1
LIMIT 1
```

Pass the manifest BattleTag through `TrainingSourceSpec`, never through report DTOs. Recompute audit counters from projected rows so stadium quick is counted as performance-capable training data.

- [ ] **Step 6: Run focused tests and verify GREEN**

Run:

```powershell
cargo test --test career_mmr_inputs --test career_mmr_training_projection --test career_mmr_multi_account_training
```

Expected: all tests PASS, including the prior `stadium_quick_keeps_outcome_row_but_never_trains_performance` test after it is replaced by the approved shared-training behavior.

- [ ] **Step 7: Review checkpoint**

Inspect `git diff --check` and `git diff` for only the six files above. Do not stage or commit without explicit authorization.

### Task 2: Encode the Balanced Tier-One Gates

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_performance_forward.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_training_report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_stability.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training_report.rs`

- [ ] **Step 1: Write a failing configuration snapshot test**

Assert `StabilityConfig::for_stage(Screen100)` contains:

```rust
assert_eq!(config.minimum_improved_folds, 3);
assert_eq!(config.minimum_season_groups, 2);
assert_eq!(config.minimum_hero_groups, 3);
assert_eq!(config.minimum_bootstrap_completion, 0.80);
assert_eq!(config.minimum_direction_stability, 0.75);
assert_eq!(config.maximum_loo_worsened_fraction, 0.40);
assert_eq!(config.max_ece_worsening, 0.02);
assert_eq!(config.maximum_single_feature_share, 0.50);
assert_eq!(config.maximum_brier_ci_upper, 0.01);
```

- [ ] **Step 2: Run and verify RED**

Run:

```powershell
cargo test --test career_mmr_performance_forward stability_config -- --nocapture
```

Expected: FAIL on fields not yet represented or values still using old gates.

- [ ] **Step 3: Write failing eligibility boundary tests**

Create synthetic reports immediately below and above each changed boundary. Assert that `+0.010000` CI upper is accepted, `+0.010001` is rejected, 3/5 folds is accepted, 2/5 is rejected, and 40% worsened groups is accepted while a larger fraction is rejected.

- [ ] **Step 4: Implement explicit configuration fields and comparisons**

Replace embedded literals with serialized `StabilityConfig` fields. Keep worker errors, non-finite values, missing account coverage, sign violations, and shadow direction reversal as unconditional rejection reasons.

- [ ] **Step 5: Run report and stability tests**

Run:

```powershell
cargo test --test career_mmr_performance_forward --test career_mmr_training_report
```

Expected: PASS with deterministic config snapshots and exact boundary behavior.

- [ ] **Step 6: Review checkpoint**

Run `git diff --check`. Do not stage or commit without explicit authorization.

### Task 3: Add Guaranteed Per-Role Model Selection

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_selection.rs`
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_runtime.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_performance_selection.rs`

- [ ] **Step 1: Write failing tier-selection tests**

Define the desired public contract in the test:

```rust
let selected = select_role_model(role, &rows, &tier_one_evidence)?;
assert_eq!(selected.tier(), PerformanceTier::Learned);

let selected = select_role_model(role, &rows, &failed_tier_one)?;
assert_eq!(selected.tier(), PerformanceTier::Shrunk);

let selected = select_role_model(role, &unstable_rows, &failed_tier_one)?;
assert_eq!(selected.tier(), PerformanceTier::Percentile);
```

Also assert every non-empty role dataset returns one tier and empty role data returns an explicit Chinese error rather than a fabricated artifact.

- [ ] **Step 2: Run and verify RED**

Run:

```powershell
cargo test --test career_mmr_performance_selection
```

Expected: compilation FAIL because the selection module and types do not exist.

- [ ] **Step 3: Implement tier-one selection**

Wrap the existing promoted `PerformanceModelArtifact` as `SelectedRoleModel::Learned` only when the balanced gate evidence is eligible and all universal invariants pass.

- [ ] **Step 4: Implement the strong-shrink tier**

Retain coefficients with at least 60% bootstrap direction stability and valid business signs; set all others to zero. Binary-search a single shrink factor in `[0,1]` until mean absolute correction is at most `0.05`, then validate Brier degradation `<=0.01`, cap-hit rate `<=5%`, finite output, and no outcome-direction reversal.

- [ ] **Step 5: Implement the percentile tier**

Build deterministic median/MAD or empirical-percentile baselines keyed by role, environment, mode, and hero with the design fallback order. Recenter each role/mode training distribution to weighted mean zero and store only aggregate baseline statistics.

- [ ] **Step 6: Add runtime invariant tests**

For all tiers and modes, assert:

```rust
assert!(adjustment.is_finite());
assert!(adjustment.abs() <= model.cap_for(mode));
assert_eq!((base_delta + adjustment).signum(), base_delta.signum());
```

Include stadium quick `0.10` tier-two and `0.05` tier-three caps.

- [ ] **Step 7: Run and verify GREEN**

Run:

```powershell
cargo test --test career_mmr_performance_selection
```

Expected: all tier selection, fallback, centering, and cap tests PASS.

- [ ] **Step 8: Review checkpoint**

Run `cargo fmt --check` and `git diff --check`. Do not stage or commit without explicit authorization.

### Task 4: Serialize and Validate the v3 Training Artifact

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training_report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/bin/train_career_mmr.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_training_report.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_performance_cli.rs`

- [ ] **Step 1: Write failing artifact schema tests**

Require artifact schema v3 to contain exactly one selected model for tank, damage, and support, plus `ordinary-shared-v3`, `stadium-shared-v3`, mode scales, config hash, and anonymous aggregate evidence. Recursively reject keys matching `battleTag`, `account`, `matchId`, `path`, `payload`, `token`, `cookie`, or `credential`.

- [ ] **Step 2: Run and verify RED**

Run:

```powershell
cargo test --test career_mmr_training_report v3_artifact -- --nocapture
```

Expected: FAIL because the existing artifact has no tiered per-role selection.

- [ ] **Step 3: Implement schema v3 and validation**

Add tagged serde variants for learned, shrunk, and percentile artifacts. Validation must reject missing/duplicate roles, unsupported modes, non-finite numbers, invalid signs, excessive caps, mismatched config hashes, and privacy-forbidden fields before publication.

- [ ] **Step 4: Update the CLI publication sequence**

After stability evaluation, call `select_role_model` for all three roles. Atomically publish report, v3 artifact, and stage marker only after all models validate. Keep screen/candidate/final diagnostics, but allow fallback tiers at final publication rather than emitting `no_promoted_candidate`.

- [ ] **Step 5: Run CLI and report tests**

Run:

```powershell
cargo test --test career_mmr_training_report --test career_mmr_performance_cli
```

Expected: PASS; synthetic CLI output contains three selected roles and no forbidden fields.

- [ ] **Step 6: Review checkpoint**

Run `git diff --check`. Do not stage or commit without explicit authorization.

### Task 5: Activate Runtime v3 Transactionally

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/replay.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/database/migrations.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_migration.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_rating.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_replay.rs`

- [ ] **Step 1: Write failing forward-compatible migration tests**

Require a nullable `performance_tier` column on `career_mmr_events` and a model-artifact table keyed by model version, with idempotent initialization on pre-MMR, v1, and v2 databases. Existing v2 rows must remain unchanged.

- [ ] **Step 2: Run and verify RED**

Run:

```powershell
cargo test --test career_mmr_migration
```

Expected: FAIL because the v3 artifact storage and tier column do not exist.

- [ ] **Step 3: Write failing v3 replay tests**

Seed a synthetic validated artifact containing one role at each tier. Assert v3 events contain non-zero bounded corrections, repeat rebuilds are byte-for-byte identical, v2 remains queryable, and an injected mid-transaction failure leaves v2 active.

- [ ] **Step 4: Run and verify RED**

Run:

```powershell
cargo test --test career_mmr_replay v3
```

Expected: FAIL because replay still writes `performance_adjustment=0.0` and activates model 2.

- [ ] **Step 5: Implement runtime artifact loading and rating application**

Set `PERFORMANCE_MODEL_VERSION=3`, validate the artifact before opening the write transaction, evaluate normalized performance for each input, apply tier/mode caps, then pass the correction into `update_rating_for_expected`. Preserve the existing final mode cap and sign-protection logic.

- [ ] **Step 6: Implement transactional activation and rollback**

Within one transaction, write v3 model definition/artifact, rebuild v3 inputs/events/summaries, verify three role summaries and event counts, then switch the single active flag to v3. Never delete v2 rows.

- [ ] **Step 7: Run migration, rating, and replay tests**

Run:

```powershell
cargo test --test career_mmr_migration --test career_mmr_rating --test career_mmr_replay
```

Expected: PASS, including v2 preservation, v3 non-zero corrections, idempotency, and failure rollback.

- [ ] **Step 8: Review checkpoint**

Run `cargo fmt --check` and `git diff --check`. Do not stage or commit without explicit authorization.

### Task 6: Update Queries, Explanation, and UI Contracts

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/query.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_queries.rs`
- Modify: `apps/owstats-desktop/scripts/careerMmrUi.test.ts`
- Modify: `apps/owstats-desktop/src/pages/CareerMmrPage.tsx`

- [ ] **Step 1: Write failing active-v3 query tests**

Seed v2 and v3 simultaneously with only v3 active. Assert overview, history, breakdowns, and recent explanations return v3 values. Assert explanation factors include Chinese `胜负基础变化` and `个人表现修正`, with the selected tier label but no raw coefficient or identity fields.

- [ ] **Step 2: Run and verify RED**

Run:

```powershell
cargo test --test career_mmr_queries
```

Expected: FAIL where build status still targets v2 or tier explanation is absent.

- [ ] **Step 3: Implement active-version query behavior**

Change build status target to v3, keep all data queries keyed to the single active model, and map tiers to Chinese labels: `约束学习`, `强收缩`, `稳健百分位`.

- [ ] **Step 4: Add and run the frontend contract test**

Require the page to show model version and retain `本地模型推算，并非暴雪官方隐藏分`; do not add training diagnostics to the product UI.

Run:

```powershell
npx tsx scripts/careerMmrUi.test.ts
```

Expected: PASS after minimal page/API updates.

- [ ] **Step 5: Run query and UI tests**

Run:

```powershell
cargo test --test career_mmr_queries
npx tsx scripts/careerMmrUi.test.ts
```

Expected: both PASS.

- [ ] **Step 6: Review checkpoint**

Run `git diff --check`. Do not stage or commit without explicit authorization.

### Task 7: Full Verification and Controlled Training Run

**Files:**
- Modify after generated aggregate evidence only: `docs/reports/<new-career-mmr-v3-report>.md`
- Do not add: real databases, credentials, raw diagnostics, model files containing private data.

- [ ] **Step 1: Run all focused career MMR tests**

Run:

```powershell
cargo test --test career_mmr_migration --test career_mmr_inputs --test career_mmr_training_projection --test career_mmr_multi_account_training --test career_mmr_performance_forward --test career_mmr_training_report --test career_mmr_performance_selection --test career_mmr_performance_cli --test career_mmr_shadow --test career_mmr_rating --test career_mmr_replay --test career_mmr_queries
```

Expected: all PASS.

- [ ] **Step 2: Run the complete backend and frontend suites**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
cd ..
npm test
npm run build
```

Expected: all commands exit 0.

- [ ] **Step 3: Execute the controlled read-only training stages**

Use the documented `train_career_mmr` CLI against registered account sources without opening credentials or raw databases manually. Run screen, candidate, and final stages in order. Diagnostics may contain only anonymous counts, timing, sizes, selected tiers, and aggregate metrics.

Expected: projection reports non-zero stadium competitive rows; all three roles select learned, shrunk, or percentile tiers; final artifact validation succeeds.

- [ ] **Step 4: Verify shadow replay and activation artifact**

Run the existing evaluation CLI against the final artifact. Expected: no non-finite output, no sign reversal, caps respected, deterministic digest, and no forbidden privacy keys.

- [ ] **Step 5: Write the aggregate v3 report**

Document mode counts, selected tier per role, aggregate validation metrics, shadow replay totals, cap hits, and activation recommendation. Do not include season labels when they cannot be safely published, account identity, match identity, paths, or payloads.

- [ ] **Step 6: Run release verification required by repository policy**

Run:

```powershell
cd apps\owstats-desktop
npm run tauri build
git diff --check
git status --short
```

Expected: Tauri build exits 0; diff check is clean; status contains only expected files and pre-existing user changes. Record that clean-checkout/install smoke verification remains required if it cannot be performed locally.

- [ ] **Step 7: Final review checkpoint**

Report every command and result, unverified items, selected tiers, and whether any real private data was accessed. Do not stage or commit unless the user explicitly requests it.
