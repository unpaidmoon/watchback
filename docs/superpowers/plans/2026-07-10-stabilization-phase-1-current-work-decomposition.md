# STAB-002 Current Work Decomposition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconstruct the valid work preserved in snapshot `648531e08b0f528173308b6556597316d9799076` as small, independently verifiable commits on `codex/stabilization`, while explicitly deferring unsafe resource-path work to STAB-003.

**Architecture:** Treat the snapshot as evidence, not as a patch to cherry-pick. Rebuild one behavior at a time from base `c0a5fc0abe959cc4a0534f0459a6106b25288bc9`, add tests before behavior changes, and keep database evolution forward-only. Changes that require ignored `docs/reference` inputs or `/@fs/D:/Codex/...` URLs are recorded as deferred rather than reintroduced.

**Tech Stack:** Git, PowerShell, Markdown, React 18, TypeScript, Node/tsx, Tauri 2, Rust, rusqlite, SQLite

---

## Scope and immutable sources

- Formal branch: `codex/stabilization`
- Starting commit: `f544d2546adc2ba28fa7ef33c2b5ae88e109102b`
- Snapshot branch: `codex/stabilization-snapshot`
- Snapshot commit: `648531e08b0f528173308b6556597316d9799076`
- Recovery bundle: `D:\Codex\OW_data\owstats-local-backups\20260710-172542\snapshot.bundle`
- Product scope remains fixed to S21/S22/S23; no global season store and no sync-all action.
- This phase does not change login identity, credentials, persistent raw JSON redaction, installer packaging, or general resource-path architecture.

Do not cherry-pick the snapshot commit. Do not restore whole mixed-responsibility Rust files from it.

## Snapshot reconciliation map

| Snapshot paths | STAB-002 disposition |
| --- | --- |
| `AGENTS.md` | Rebuild as governance commit. |
| `package.json`, `phase8MultiSeasonPages.test.ts`, three season-aware pages, `styles.css` | Rebuild as three page commits plus one matches-contract test commit. |
| `phase8_multi_season_pages.rs` | Rebuild as backend characterization test commit. |
| Role-mode hunks in `review.rs`, `phase3_review.rs` | Rebuild as one behavior-fix commit. |
| Arena schema/import/query hunks in `db.rs`, `importer.rs`, `queries.rs`, `import_baseline.rs` | Rebuild as three commits; replace the snapshot's global row-count backfill check with schema v3 migration plus per-season completion markers. |
| Dictionary fallback hunks in `analytics.rs`, `db.rs`, `importer.rs`, `queries.rs`, `ranking.rs`, `review.rs`, `phase4_ranking_analysis.rs` | Defer to STAB-003 because the snapshot embeds or reads ignored `docs/reference/ow_config_cache.json`. |
| `assets.ts` stadium mapping and most `phase6Assets.test.ts` hunks | Defer to STAB-003 because runtime URLs still depend on `/@fs/D:/Codex/...`. |
| `assets/manifest.json` Shion localization | Rebuild with a focused regression test. |
| Five referenced previews: `esperanca2.png`, `ilios2.png`, `nepal2.png`, `oasis-university.png`, `samoa2.png` | Preserve as reviewed binary assets with blob checks. Consumer wiring remains STAB-003. |
| Seven unreferenced Chinese-named PNG candidates | Exclude: no accepted runtime mapping uses them. They remain recoverable from the snapshot. |
| `2026-07-09-s23-arena-map-preview-gaps.csv` | Exclude: it contains development-machine absolute paths and spreadsheet-rounded GUIDs; accepted mappings are preserved in `main-thread-log.md`. |
| `main-thread-log.md` mapping appendix | Rebuild with the five accepted assets. |
| `docs/current-development-status.md` | Rewrite at phase end; do not restore stale branch, dirty-worktree, installer, or verification claims. |
| `test-results/.last-run.json` | Delete and ignore as generated output. |

## Commit sequence

1. `docs: start STAB-002 decomposition`
2. `chore: stop tracking generated test results`
3. `fix: classify lowercase competitive modes`
4. `test: lock multi-season query isolation`
5. `feat: add review season selection`
6. `feat: add ranking season selection`
7. `feat: add friend analysis season selection`
8. `test: lock matches season URL contract`
9. `feat: migrate arena player index schema`
10. `feat: project arena players during import`
11. `fix: backfill and query arena players`
12. `fix: localize Shion asset alias`
13. `assets: preserve confirmed S23 arena previews`
14. `docs: complete STAB-002 decomposition`

### Task 1: Start STAB-002 and restore the collaboration contract

**Files:**
- Modify: `AGENTS.md`
- Modify: `docs/stabilization-progress.md`

- [ ] **Step 1: Verify the execution baseline**

```powershell
git branch --show-current
git rev-parse HEAD
git status --short
git rev-parse codex/stabilization-snapshot
```

Expected: formal branch at `f544d2546adc2ba28fa7ef33c2b5ae88e109102b`, snapshot at `648531e08b0f528173308b6556597316d9799076`, and no status entries.

- [ ] **Step 2: Restore only the reviewed collaboration contract**

```powershell
git restore --source=648531e08b0f528173308b6556597316d9799076 -- AGENTS.md
```

Use `apply_patch` to change only STAB-002 in `docs/stabilization-progress.md` from `待开始 / 未执行` to `进行中 / 正在按快照清单重建小提交`.

- [ ] **Step 3: Verify and commit**

```powershell
git diff --check
git diff --name-only
git add -- AGENTS.md docs/stabilization-progress.md
git commit -m "docs: start STAB-002 decomposition"
```

Expected: exactly the two documentation files are committed.

### Task 2: Remove generated test state from version control

**Files:**
- Modify: `.gitignore`
- Delete: `test-results/.last-run.json`

- [ ] **Step 1: Add the failing repository-hygiene check**

```powershell
git ls-files --error-unmatch test-results/.last-run.json
```

Expected before the change: exit 0, proving generated state is tracked.

- [ ] **Step 2: Apply the minimal cleanup**

Add this exact ignore rule under `# Test artifacts`:

```gitignore
/test-results/
```

Then run:

```powershell
git rm -- test-results/.last-run.json
git check-ignore -v --no-index test-results/.last-run.json
```

Expected: the root `.gitignore` rule matches the path.

- [ ] **Step 3: Commit**

```powershell
git diff --cached --check
git add -- .gitignore
git commit -m "chore: stop tracking generated test results"
```

### Task 3: Correct lowercase competitive mode classification

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/review.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase3_review.rs`

- [ ] **Step 1: Add the two snapshot regression tests**

Copy only these tests and their `seed_ranked_match_entry_with_instance_type` helper from the snapshot:

```rust
personal_review_counts_only_lowercase_competitive_roles_by_entry_role_type
personal_review_keeps_lowercase_sport_open_out_of_role_queue
```

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test phase3_review personal_review_counts_only_lowercase -- --exact
cargo test --test phase3_review personal_review_keeps_lowercase -- --exact
```

Expected: at least one assertion fails because lowercase `sport` is not split by `instanceType`.

- [ ] **Step 2: Implement one classification function**

Replace the classification body with this interface and rules, and make both entry and match-player aggregation call it:

```rust
fn role_type_from_match_player(
    game_mode: &str,
    raw_entry: &Value,
    fallback_role_type: &str,
) -> &'static str {
    let instance_type = raw_entry
        .get("instanceType")
        .and_then(Value::as_str)
        .unwrap_or("");
    if game_mode == "SportOpen" || (game_mode == "sport" && instance_type == "IT_RULESET") {
        return "open";
    }
    if game_mode == "SportPreset" || (game_mode == "sport" && instance_type == "IT_RANKED") {
        return normalize_role_type(
            raw_entry.get("roleType").and_then(Value::as_str).unwrap_or(fallback_role_type),
        );
    }
    ""
}
```

The `match_players` query must also select `m.game_mode` and `m.raw_entry_json`; do not apply the unrelated dictionary-name hunks from the snapshot.

- [ ] **Step 3: Verify and commit**

```powershell
cargo fmt --check
cargo test --test phase3_review
git diff --check
git add -- apps/owstats-desktop/src-tauri/src/review.rs apps/owstats-desktop/src-tauri/tests/phase3_review.rs
git commit -m "fix: classify lowercase competitive modes"
```

### Task 4: Lock backend multi-season isolation

**Files:**
- Create: `apps/owstats-desktop/src-tauri/tests/phase8_multi_season_pages.rs`

- [ ] **Step 1: Restore the exact characterization test**

```powershell
git restore --source=648531e08b0f528173308b6556597316d9799076 -- apps/owstats-desktop/src-tauri/tests/phase8_multi_season_pages.rs
```

This test must retain all three cases: page-query isolation, match-list isolation, and global `match_id` uniqueness.

- [ ] **Step 2: Verify current behavior without product changes**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --test phase8_multi_season_pages
```

Expected: 3 passed. If it fails, stop and report the exact query path; do not fold a fix into this test-only commit.

- [ ] **Step 3: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/tests/phase8_multi_season_pages.rs
git commit -m "test: lock multi-season query isolation"
```

### Task 5: Add personal-review season selection

**Files:**
- Create then extend: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- Modify: `apps/owstats-desktop/package.json`
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- Modify: `apps/owstats-desktop/src/styles.css`

- [ ] **Step 1: Create the review-only failing source contract**

Use the snapshot test's four `reviewSource` assertions only, add the script to `package.json`, and run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase8MultiSeasonPages.test.ts
```

Expected: failure on the hard-coded S22/default request contract.

- [ ] **Step 2: Restore the reviewed page and styles**

```powershell
git restore --source=648531e08b0f528173308b6556597316d9799076 -- apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/src/styles.css
```

Required behavior: URL season first; backend-selected latest season second; review and analysis use the same `selectedSeason`; visible text remains Chinese.

- [ ] **Step 3: Verify and commit**

```powershell
npx tsx scripts/phase8MultiSeasonPages.test.ts
npm run build
git diff --check
git add -- package.json scripts/phase8MultiSeasonPages.test.ts src/pages/PersonalReviewPage.tsx src/styles.css
git commit -m "feat: add review season selection"
```

### Task 6: Add ranking season selection

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- Modify: `apps/owstats-desktop/src/pages/RankingPage.tsx`

- [ ] **Step 1: Append the snapshot's ranking assertions and prove failure**

```powershell
npx tsx scripts/phase8MultiSeasonPages.test.ts
```

Expected: failure because `getRankings()` has no selected-season argument or selector.

- [ ] **Step 2: Restore only the ranking page**

```powershell
git restore --source=648531e08b0f528173308b6556597316d9799076 -- apps/owstats-desktop/src/pages/RankingPage.tsx
npx tsx scripts/phase8MultiSeasonPages.test.ts
npm run build
```

Expected: pass; changing tabs preserves the `season` query parameter.

- [ ] **Step 3: Commit**

```powershell
git add -- scripts/phase8MultiSeasonPages.test.ts src/pages/RankingPage.tsx
git commit -m "feat: add ranking season selection"
```

### Task 7: Add friend-analysis season selection

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- Modify: `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx`

- [ ] **Step 1: Append the snapshot's friend-analysis assertions and prove failure**

Expected failure: the current route redirects to `/review` and does not request selected-season analysis.

- [ ] **Step 2: Restore the reviewed page**

```powershell
git restore --source=648531e08b0f528173308b6556597316d9799076 -- apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx
npx tsx scripts/phase8MultiSeasonPages.test.ts
npm run build
```

Expected: pass with page-local URL state; no global season state is added.

- [ ] **Step 3: Commit**

```powershell
git add -- scripts/phase8MultiSeasonPages.test.ts src/pages/FriendAnalyticsPage.tsx
git commit -m "feat: add friend analysis season selection"
```

### Task 8: Lock the existing matches-page season contract

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`

- [ ] **Step 1: Append the snapshot assertions for `MatchesPage`, `FilterBar`, and `matchesPageCache`**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase8MultiSeasonPages.test.ts
npm test
npm run build
```

Expected: all pass without product code changes. If not, stop; do not hide a matches regression in a test commit.

- [ ] **Step 2: Commit the characterization only**

```powershell
git add -- scripts/phase8MultiSeasonPages.test.ts
git commit -m "test: lock matches season URL contract"
```

### Task 9: Add the forward-only arena index migration

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/db.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`

- [ ] **Step 1: Add a failing v2-upgrade test**

Create `database_migrates_v2_arena_player_index_idempotently`. It must seed a v2 database with one existing match, initialize twice, then assert:

```rust
assert_eq!(version, 3);
assert_eq!(arena_table_count, 1);
assert_eq!(bnet_index_count, 1);
assert_eq!(name_index_count, 1);
assert_eq!(existing_match_count, 1);
```

Run the exact test and expect failure because schema version is still 2.

- [ ] **Step 2: Implement migration v3 transactionally**

Set `CURRENT_SCHEMA_VERSION` to 3. In `run_migrations`, use `conn.unchecked_transaction()?` for the v3 block and create exactly:

```sql
CREATE TABLE IF NOT EXISTS arena_match_players (
    match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
    season INTEGER NOT NULL,
    side TEXT NOT NULL,
    bnet_id TEXT NOT NULL,
    player_name TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (match_id, side, bnet_id)
);
CREATE INDEX IF NOT EXISTS idx_arena_match_players_bnet
    ON arena_match_players(bnet_id, season);
CREATE INDEX IF NOT EXISTS idx_arena_match_players_name
    ON arena_match_players(player_name, season);
```

Update `PRAGMA user_version` to 3 inside the same transaction and commit. Do not add `include_str!("../../../../docs/reference/ow_config_cache.json")`.

- [ ] **Step 3: Verify and commit**

```powershell
cargo fmt --check
cargo test --test import_baseline database_migrates_v2_arena_player_index_idempotently -- --exact
cargo test --test import_baseline database_initialize_sets_schema_version_idempotently -- --exact
git add -- src/db.rs tests/import_baseline.rs
git commit -m "feat: migrate arena player index schema"
```

### Task 10: Project arena players during import

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/importer.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`

- [ ] **Step 1: Add failing projection/privacy tests**

Add tests proving that total and round lists are indexed, duplicate players upsert, numeric-only names become `未知玩家`, and rebuilding S22 does not delete S21 arena rows.

- [ ] **Step 2: Implement the snapshot projection with an explicit completion marker**

Use the snapshot helpers `insert_arena_player_index_rows`, `insert_arena_player_index_side`, `arena_name_map`, `arena_player_name`, and `looks_like_private_id`. At the start of each season rebuild delete only that season:

```rust
tx.execute("DELETE FROM arena_match_players WHERE season = ?", params![season])?;
```

After processing every detail, atomically upsert this marker even when a fight has zero valid players:

```rust
let key = format!("arena_player_index_v1:{season}");
tx.execute(
    "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, 'complete', ?2) \
     ON CONFLICT(key) DO UPDATE SET value='complete', updated_at=excluded.updated_at",
    params![key, now],
)?;
```

Do not copy the snapshot's `load_lookup` change that reads `docs/reference`.

- [ ] **Step 3: Verify and commit**

```powershell
cargo fmt --check
cargo test --test import_baseline arena_player
cargo test --test import_baseline import_is_idempotent
git add -- src/importer.rs tests/import_baseline.rs
git commit -m "feat: project arena players during import"
```

### Task 11: Backfill and query arena players safely

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/queries.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`

- [ ] **Step 1: Add failing search/filter and partial-upgrade tests**

Tests must prove name and bnet-id search return only the stable player name, filters find fight matches, and a database with S21 marked complete plus unmarked S22 fight details still backfills S22.

- [ ] **Step 2: Implement per-season backfill detection**

Call `ensure_arena_player_index(db, filters.season)` before `list_matches` and `ensure_arena_player_index(db, None)` before player search. Missing seasons are selected by absent completion marker, not by a global row count:

```sql
SELECT DISTINCT m.season
FROM matches m
JOIN match_details d ON d.match_id = m.match_id
WHERE m.bucket='stadium'
  AND (m.match_kind='fight' OR d.match_kind='fight')
  AND (?1 IS NULL OR m.season=?1)
  AND NOT EXISTS (
      SELECT 1 FROM app_settings s
      WHERE s.key = 'arena_player_index_v1:' || m.season
  )
```

If any season is missing, call the existing transactional rebuild once. Extend search and `append_filters` with `arena_match_players`, but return `player_name`, never raw `bnet_id`. Do not apply dictionary fallback joins from the snapshot.

- [ ] **Step 3: Verify and commit**

```powershell
cargo fmt --check
cargo test --test import_baseline arena_player
cargo test --tests
git add -- src/queries.rs tests/import_baseline.rs
git commit -m "fix: backfill and query arena players"
```

### Task 12: Localize the Shion asset alias

**Files:**
- Modify: `assets/manifest.json`
- Modify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`

- [ ] **Step 1: Add the failing assertion**

Read `assets/manifest.json` in the test and add this assertion without adding another absolute-path expectation:

```ts
const assetManifest = JSON.parse(
  readFileSync(new URL("../../../assets/manifest.json", import.meta.url), "utf8"),
) as { heroes: Array<{ key: string; cn_name?: string }> };
const shion = assetManifest.heroes.find((item) => item.key === "shion");
assertEqual(shion?.cn_name, "死怨", "Shion uses the accepted Chinese display alias");
```

The existing asset-root assertions remain untouched; STAB-003 will replace that pre-existing absolute root.

- [ ] **Step 2: Change only `shion.cn_name` from `Shion` to `死怨`**

Run the focused test, `npm test`, and commit only these two files.

```powershell
git commit -m "fix: localize Shion asset alias"
```

### Task 13: Preserve confirmed S23 arena preview assets

**Files:**
- Create: `assets/maps/esperanca2.png`
- Create: `assets/maps/ilios2.png`
- Create: `assets/maps/nepal2.png`
- Create: `assets/maps/oasis-university.png`
- Create: `assets/maps/samoa2.png`
- Modify: `docs/superpowers/specs/main-thread-log.md`

- [ ] **Step 1: Restore only accepted, referenced assets and the mapping record**

```powershell
git restore --source=648531e08b0f528173308b6556597316d9799076 -- `
  assets/maps/esperanca2.png assets/maps/ilios2.png assets/maps/nepal2.png `
  assets/maps/oasis-university.png assets/maps/samoa2.png `
  docs/superpowers/specs/main-thread-log.md
```

- [ ] **Step 2: Verify snapshot blob identity**

```powershell
git hash-object assets/maps/esperanca2.png       # 7133f873ef64384b5a5522802e3fe183b12bb20a
git hash-object assets/maps/ilios2.png           # fcf1ea0e11534d1c1f8b8bc4839bf9455315a5a0
git hash-object assets/maps/nepal2.png           # 4775c5d41c085a8c5281f7f7c8e883da043cbfb1
git hash-object assets/maps/oasis-university.png # fe6d8b87e27bf2e650462a1aafa2b23958662044
git hash-object assets/maps/samoa2.png           # ba4390bd5ce2d3cc6d5eac17942348fe578a5114
```

Expected: exact hashes above. Confirm the seven unreferenced Chinese-named PNGs and the gaps CSV are absent.

- [ ] **Step 3: Commit preservation only**

```powershell
git add -- assets/maps/esperanca2.png assets/maps/ilios2.png assets/maps/nepal2.png assets/maps/oasis-university.png assets/maps/samoa2.png docs/superpowers/specs/main-thread-log.md
git commit -m "assets: preserve confirmed S23 arena previews"
```

### Task 14: Reconcile the snapshot and complete STAB-002

**Files:**
- Create: `docs/current-development-status.md`
- Modify: `docs/stabilization-progress.md`

- [ ] **Step 1: Write current status from verified facts**

Create a concise Chinese document containing: product architecture; verified S21/S22/S23 page behavior; arena index schema v3 and backfill marker; preserved preview assets not yet wired; deferred STAB-003 dictionary/resource-path work; deferred STAB-004 privacy work; tests actually run; and no installer claim. Do not restore the snapshot's stale branch/ahead/dirty-worktree text.

- [ ] **Step 2: Prove every snapshot path is accounted for**

```powershell
$snapshotPaths = git diff --name-only c0a5fc0abe959cc4a0534f0459a6106b25288bc9 648531e08b0f528173308b6556597316d9799076
$formalPaths = git diff --name-only f544d2546adc2ba28fa7ef33c2b5ae88e109102b HEAD
$snapshotPaths.Count
$formalPaths.Count
```

Review the reconciliation map above against both lists. All 36 snapshot paths must be categorized as rebuilt, rewritten, deferred, or excluded; no unknown path is allowed.

- [ ] **Step 3: Run the phase verification gate**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
cd ..
npm test
npm run build
cd ..\..
git diff --check
git status --short
```

Expected: all commands pass; before the final documentation commit only the two documentation files are listed. No real credentials, database, browser profile, or player dump is read.

- [ ] **Step 4: Mark STAB-002 verified and commit**

Update its row to `已验证`, list the commands above as evidence, and put the exact commit range obtained from Git in the commit cell. STAB-003 remains `待开始`.

```powershell
git add -- docs/current-development-status.md docs/stabilization-progress.md
git commit -m "docs: complete STAB-002 decomposition"
git status --short
```

Expected: clean formal branch.

## Final acceptance

- Snapshot and external bundle remain unchanged and verifiable.
- Every formal commit has one rollback reason and passes its focused checks.
- Full Rust tests, frontend tests, and frontend build pass on the final formal branch.
- No `docs/reference` import, `D:\Codex\...` dependency, new `/@fs/` dependency, credential, local database, browser profile, or real player dump is introduced.
- Arena schema upgrade is forward-only, idempotent, transactionally versioned, and preserves old rows.
- Multi-season pages keep independent URL state and fixed S21/S22/S23 choices.
- Deferred dictionary/resource resolver work is explicitly left for STAB-003; STAB-004 and later stages are not started.

## Execution handoff

Use `superpowers:subagent-driven-development` for fresh implementation and two-stage review per task, or `superpowers:executing-plans` for inline batch execution with checkpoints. Do not execute this plan on the snapshot branch.
