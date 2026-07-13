# Career Role MMR Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the privacy-safe, dynamically replayable three-role baseline MMR feature, including input audits, outcome-based rating, Stadium last-round handling, APIs, and the first product page, while keeping learned performance weights in shadow-only follow-up work.

**Architecture:** Add a focused `career_mmr` Rust module with independent input extraction, rating math, persistence/replay, and query units. SQLite stores only non-sensitive derived inputs and rating events; all seasons are discovered from stored matches and replayed by timestamp. The frontend reads backend-owned overview/history DTOs and never recomputes MMR.

**Tech Stack:** Rust, rusqlite, serde/serde_json, Tauri 2 commands, SQLite migrations, React 18, TypeScript, Vite, existing `tsx` source-contract tests.

---

## Scope and follow-up boundary

This plan delivers the independently testable baseline feature:

- dynamic-season role MMR;
- ordinary-mode role extraction and documented fallback quality;
- Stadium role extraction from the last valid round only;
- outcome/opponent-strength rating with `mu/sigma`;
- per-role `±30` competitive/Stadium and `±10` quick-play limits;
- deterministic replay and materialized summaries;
- overview/history/mode/season/explanation APIs;
- the first “生涯 MMR” page.

It also produces the aggregate field-audit evidence needed for learned weights. It does **not** enable final-blow, resource-debt, damage, healing, or mitigation modifiers. Those require a separate plan after the audit, rolling time validation, 1,000 match-cluster bootstraps, and shadow-model gates pass.

Repository authorization rule: this checkout is already dirty. Every commit checkpoint below is conditional. The executor must first obtain explicit user authorization to stage and commit; without it, run the listed tests plus `git diff --check` and `git status --short`, report the checkpoint, and continue without staging.

## File structure

Backend files:

- Create `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`: public module boundary and orchestration exports.
- Create `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs`: ordinary and Stadium input extraction, role/mode normalization, quality flags.
- Create `apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs`: pure `mu/sigma`, expected-win, confidence, and delta-limit math.
- Create `apps/owstats-desktop/src-tauri/src/career_mmr/replay.rs`: deterministic event replay, invalidation, checkpoints, and materialized summaries.
- Create `apps/owstats-desktop/src-tauri/src/career_mmr/query.rs`: overview/history/mode/season/explanation DTO queries.
- Create `apps/owstats-desktop/src-tauri/src/bin/audit_career_mmr_inputs.rs`: aggregate-only audit command; it must not print IDs, names, raw JSON, or credentials.
- Modify `apps/owstats-desktop/src-tauri/src/database/migrations.rs`: idempotent MMR schema migration.
- Modify `apps/owstats-desktop/src-tauri/src/lib.rs`: module exports and command registration.
- Modify `apps/owstats-desktop/src-tauri/src/commands.rs`: Tauri command adapters.
- Modify `apps/owstats-desktop/src-tauri/src/models.rs`: public MMR DTOs.
- Modify `apps/owstats-desktop/src-tauri/src/importer.rs`: invalidate/rebuild derived MMR after successful season import without changing raw import semantics.
- Create `apps/owstats-desktop/src-tauri/tests/career_mmr_migration.rs`.
- Create `apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs`.
- Create `apps/owstats-desktop/src-tauri/tests/career_mmr_rating.rs`.
- Create `apps/owstats-desktop/src-tauri/tests/career_mmr_replay.rs`.
- Create `apps/owstats-desktop/src-tauri/tests/career_mmr_queries.rs`.

Frontend files:

- Create `apps/owstats-desktop/src/pages/CareerMmrPage.tsx`: product page and page-owned display filters.
- Create `apps/owstats-desktop/src/shared/careerMmrFormat.ts`: Chinese labels and chart-point formatting.
- Create `apps/owstats-desktop/src/styles/features/career-mmr.css`: feature-local styles.
- Create `apps/owstats-desktop/scripts/careerMmrUi.test.ts`: source-level route, copy, and contract checks.
- Modify `apps/owstats-desktop/src/shared/types.ts`: MMR DTO mirrors.
- Modify `apps/owstats-desktop/src/shared/api.ts`: invoke wrappers.
- Modify `apps/owstats-desktop/src/shared/phase6Shell.ts`: one navigation entry.
- Modify `apps/owstats-desktop/src/App.tsx`: one route.
- Modify `apps/owstats-desktop/src/styles.css`: feature stylesheet import.
- Modify `apps/owstats-desktop/package.json`: add the new source-contract test to `test:unit`.

### Task 1: Add idempotent MMR schema migration

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/database/migrations.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_migration.rs`

- [ ] **Step 1: Write the failing empty-database migration test**

```rust
#[test]
fn creates_career_mmr_tables_and_indexes() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let db = owstats_desktop::OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    for table in [
        "mmr_model_versions",
        "mmr_match_inputs",
        "career_mmr_events",
        "career_role_mmr",
        "career_role_mode_summary",
        "career_role_season_summary",
    ] {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1, "missing {table}");
    }
    Ok(())
}
```

- [ ] **Step 2: Run the migration test and verify it fails**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test career_mmr_migration creates_career_mmr_tables_and_indexes
```

Expected: FAIL because the six tables do not exist.

- [ ] **Step 3: Add the migration with exact privacy-safe columns and constraints**

Add an idempotent migration containing these core constraints:

```sql
CREATE TABLE IF NOT EXISTS mmr_model_versions (
    version INTEGER PRIMARY KEY,
    algorithm TEXT NOT NULL,
    config_json TEXT NOT NULL,
    is_active INTEGER NOT NULL CHECK(is_active IN (0, 1)),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mmr_match_inputs (
    match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('tank', 'damage', 'support')),
    season INTEGER NOT NULL,
    begin_ts INTEGER NOT NULL,
    mode_family TEXT NOT NULL,
    result REAL NOT NULL CHECK(result BETWEEN 0.0 AND 1.0),
    duration_sec INTEGER NOT NULL,
    role_share REAL NOT NULL CHECK(role_share > 0.0 AND role_share <= 1.0),
    role_source TEXT NOT NULL,
    data_quality REAL NOT NULL CHECK(data_quality BETWEEN 0.0 AND 1.0),
    team_strength REAL,
    opponent_strength REAL,
    rank_coverage REAL NOT NULL CHECK(rank_coverage BETWEEN 0.0 AND 1.0),
    performance_features_json TEXT NOT NULL DEFAULT '{}',
    input_hash TEXT NOT NULL,
    PRIMARY KEY (match_id, role)
);

CREATE TABLE IF NOT EXISTS career_mmr_events (
    model_version INTEGER NOT NULL REFERENCES mmr_model_versions(version),
    match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('tank', 'damage', 'support')),
    season INTEGER NOT NULL,
    begin_ts INTEGER NOT NULL,
    mode_family TEXT NOT NULL,
    role_share REAL NOT NULL,
    result REAL NOT NULL,
    expected_win_rate REAL NOT NULL CHECK(expected_win_rate BETWEEN 0.0 AND 1.0),
    data_quality REAL NOT NULL,
    performance_adjustment REAL NOT NULL DEFAULT 0.0,
    mu_before REAL NOT NULL,
    sigma_before REAL NOT NULL,
    base_delta REAL NOT NULL,
    final_delta REAL NOT NULL,
    mu_after REAL NOT NULL,
    sigma_after REAL NOT NULL,
    confidence_after REAL NOT NULL CHECK(confidence_after BETWEEN 0.0 AND 1.0),
    input_hash TEXT NOT NULL,
    computed_at TEXT NOT NULL,
    PRIMARY KEY (model_version, match_id, role)
);
```

Create the three summary tables named by the test with `(model_version, role)`, `(model_version, role, mode_family)`, and `(model_version, season, role)` primary keys. Add indexes on `mmr_match_inputs(begin_ts, match_id)`, `career_mmr_events(model_version, role, begin_ts)`, and `career_role_season_summary(model_version, season, role)`.

- [ ] **Step 4: Add upgrade and repeat-open tests**

Create a legacy database with only the existing core tables, open it twice, and assert that both opens succeed and the MMR tables remain single instances. Also assert `PRAGMA foreign_key_check` returns no rows.

- [ ] **Step 5: Run the migration test file**

Run:

```powershell
cargo test --test career_mmr_migration
```

Expected: all migration, upgrade, repeat-open, and foreign-key tests PASS.

- [ ] **Step 6: Record the isolated schema checkpoint**

```powershell
git diff --check
git status --short
```

### Task 2: Build role mapping and aggregate input audit

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs`
- Create: `apps/owstats-desktop/src-tauri/src/bin/audit_career_mmr_inputs.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing role normalization tests**

```rust
#[test]
fn normalizes_upstream_roles_without_guessing_from_stats() {
    assert_eq!(normalize_role(Some("tank")), Some(Role::Tank));
    assert_eq!(normalize_role(Some("dps")), Some(Role::Damage));
    assert_eq!(normalize_role(Some("healer")), Some(Role::Support));
    assert_eq!(normalize_role(Some("open")), None);
    assert_eq!(normalize_role(None), None);
}
```

Add a fixture where high healing has no role or hero mapping and assert extraction returns no role instead of guessing support.

- [ ] **Step 2: Run the focused input test and verify it fails**

Run:

```powershell
cargo test --test career_mmr_inputs normalizes_upstream_roles_without_guessing_from_stats
```

Expected: FAIL because `career_mmr::input` does not exist.

- [ ] **Step 3: Add focused domain types and normalization**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role { Tank, Damage, Support }

impl Role {
    pub fn as_str(self) -> &'static str {
        match self { Self::Tank => "tank", Self::Damage => "damage", Self::Support => "support" }
    }
}

pub fn normalize_role(value: Option<&str>) -> Option<Role> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "tank" => Some(Role::Tank),
        "dps" | "damage" => Some(Role::Damage),
        "healer" | "support" => Some(Role::Support),
        _ => None,
    }
}
```

Keep role mapping in `input.rs`; do not add it to UI code or infer it from combat totals.

- [ ] **Step 4: Add a privacy-safe audit result**

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrInputAudit {
    pub matches: i64,
    pub ordinary_matches: i64,
    pub stadium_matches: i64,
    pub player_rows: i64,
    pub mapped_player_rows: i64,
    pub unmapped_player_rows: i64,
    pub final_hit_rows: i64,
    pub healing_taken_rows: i64,
    pub discovered_seasons: Vec<i64>,
}
```

The audit command may print only this aggregate DTO as JSON. Add a source assertion that output fields do not include `bnet`, `playerName`, `guid`, `rawJson`, `token`, `cookie`, or `credential`.

- [ ] **Step 5: Run audit tests and build the explicit binary**

Run:

```powershell
cargo test --test career_mmr_inputs
cargo build --bin audit_career_mmr_inputs
```

Expected: tests PASS; binary builds without changing `default-run`.

- [ ] **Step 6: Record the role-mapping and audit checkpoint**

```powershell
git diff --check
git status --short
```

### Task 3: Extract ordinary-mode MMR inputs

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs`

- [ ] **Step 1: Write failing exact-role and fallback tests**

Seed four ordinary matches and assert:

```rust
assert_eq!(exact.role, Role::Tank);
assert_eq!(exact.role_share, 1.0);
assert_eq!(exact.role_source, "entry_role");
assert_eq!(exact.data_quality, 1.0);

assert_eq!(final_hero.role, Role::Damage);
assert_eq!(final_hero.role_source, "final_hero");
assert_eq!(final_hero.data_quality, 0.55);
assert!(final_hero.performance_features.is_empty());

assert!(unknown_role.is_none());
```

Add mode assertions for lowercase and uppercase upstream values: role competitive, open competitive, and quick play must normalize consistently.

- [ ] **Step 2: Verify the tests fail**

Run:

```powershell
cargo test --test career_mmr_inputs ordinary_
```

Expected: FAIL because ordinary extraction is absent.

- [ ] **Step 3: Implement the normalized input struct and mode mapping**

```rust
pub struct MmrMatchInput {
    pub match_id: String,
    pub season: i64,
    pub begin_ts: i64,
    pub mode_family: ModeFamily,
    pub result: f64,
    pub duration_sec: i64,
    pub role: Role,
    pub role_share: f64,
    pub role_source: &'static str,
    pub data_quality: f64,
    pub team_strength: Option<f64>,
    pub opponent_strength: Option<f64>,
    pub rank_coverage: f64,
    pub performance_features: serde_json::Value,
}
```

Map supported ordinary modes using existing product semantics, not a new global season or mode state. Unknown modes return `None` and are counted in the audit.

- [ ] **Step 4: Implement strength fallback without persistent opponent identity**

Parse already-redacted `rankInfo` values only long enough to aggregate team/opponent strength and coverage. Persist only the numeric aggregates. If coverage is below the configured threshold, set both strengths to `None`, `rank_coverage` to the measured ratio, and let rating math use 50%.

- [ ] **Step 5: Run input tests**

Run:

```powershell
cargo test --test career_mmr_inputs
```

Expected: ordinary extraction, fallback quality, unknown mode, and privacy tests PASS.

- [ ] **Step 6: Record the ordinary-input checkpoint**

```powershell
git diff --check
git status --short
```

### Task 4: Extract Stadium input from the last valid round only

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs`

- [ ] **Step 1: Write failing last-valid-round tests**

Build a three-round redacted fixture where the current player changes roles and the final array item is malformed. Assert:

```rust
assert_eq!(input.role, Role::Support); // role in the last valid round
assert_eq!(input.role_share, 1.0);
assert_eq!(input.role_source, "stadium_last_round");
assert_eq!(input.performance_features["finalHit"], 7);
assert_ne!(input.performance_features["finalHit"], 2 + 4 + 7);
```

Add a missing-last-round fixture and assert it never borrows performance data from the previous round.

- [ ] **Step 2: Verify the Stadium tests fail**

Run:

```powershell
cargo test --test career_mmr_inputs stadium_
```

Expected: FAIL because Stadium extraction is absent.

- [ ] **Step 3: Implement deterministic last-valid-round selection**

```rust
fn last_valid_round(rounds: &[Value], current_player_key: &str) -> Option<&Value> {
    rounds.iter()
        .filter(|round| round_has_current_player_and_hero(round, current_player_key))
        .max_by_key(|round| (
            optional_i64(round, "roundIndex").unwrap_or(i64::MIN),
            optional_i64(round, "startTime").unwrap_or(i64::MIN),
        ))
}
```

If neither ordering field exists, preserve original array order and choose the last valid element. Do not print or persist `current_player_key`.

- [ ] **Step 4: Add local-versus-cumulative audit classification**

Return an aggregate enum count:

```rust
pub enum StadiumStatShape { RoundLocal, Cumulative, Ambiguous }
```

Classification compares monotonic numeric patterns across rounds but exposes only aggregate counts. `Ambiguous` disables Stadium performance features while preserving outcome-based rating.

- [ ] **Step 5: Run Stadium tests**

Run:

```powershell
cargo test --test career_mmr_inputs stadium_
```

Expected: last-valid-round, malformed-last-item, no-previous-round-fallback, and aggregate-shape tests PASS.

- [ ] **Step 6: Record the Stadium-input checkpoint**

```powershell
git diff --check
git status --short
```

### Task 5: Implement pure baseline rating math

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_rating.rs`

- [ ] **Step 1: Write failing expected-win and direction tests**

```rust
#[test]
fn stronger_opponents_produce_more_gain_for_a_win() {
    let even = update(state(), input(1.0, Some(2500.0), Some(2500.0), ModeFamily::RoleCompetitive));
    let upset = update(state(), input(1.0, Some(2300.0), Some(2800.0), ModeFamily::RoleCompetitive));
    assert!(upset.delta > even.delta);
}

#[test]
fn result_direction_never_reverses() {
    assert!(update(state(), win_input()).delta >= 0.0);
    assert!(update(state(), loss_input()).delta <= 0.0);
}
```

- [ ] **Step 2: Write failing per-role cap and share tests**

```rust
assert_eq!(update(uncertain_state(), competitive_input()).delta.abs(), 30.0);
assert_eq!(update(uncertain_state(), quick_input()).delta.abs(), 10.0);
assert!(update(state(), input_with_share(0.4)).delta.abs() < update(state(), input_with_share(1.0)).delta.abs());
```

- [ ] **Step 3: Run and verify failure**

Run:

```powershell
cargo test --test career_mmr_rating
```

Expected: FAIL because rating math does not exist.

- [ ] **Step 4: Implement pure rating functions**

```rust
pub const INITIAL_MU: f64 = 2500.0;
pub const INITIAL_SIGMA: f64 = 833.0;

pub fn expected_win(team: Option<f64>, opponent: Option<f64>) -> f64 {
    match (team, opponent) {
        (Some(team), Some(opponent)) => 1.0 / (1.0 + 10_f64.powf((opponent - team) / 400.0)),
        _ => 0.5,
    }
}

fn mode_cap(mode: ModeFamily) -> f64 {
    match mode {
        ModeFamily::QuickPlay => 10.0,
        ModeFamily::RoleCompetitive | ModeFamily::OpenCompetitive | ModeFamily::StadiumCompetitive => 30.0,
    }
}
```

Use `sigma`, role share, data quality, and configured mode weight to derive the raw step; clamp only after all multipliers. Keep `performance_adjustment` exactly `0.0` in this foundation plan.

- [ ] **Step 5: Add confidence and inactivity tests**

Assert repeated informative matches reduce `sigma`, inactivity/season transition expands `sigma` without reducing `mu`, and confidence remains in `[0, 1]`.

- [ ] **Step 6: Run rating tests**

Run:

```powershell
cargo test --test career_mmr_rating
```

Expected: all pure math, cap, direction, confidence, and inactivity tests PASS.

- [ ] **Step 7: Record the rating-math checkpoint**

```powershell
git diff --check
git status --short
```

### Task 6: Persist inputs and deterministically replay all dynamic seasons

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/replay.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_replay.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/importer.rs`

- [ ] **Step 1: Write failing dynamic-season replay test**

Seed non-consecutive seasons `15`, `20`, and `23` with timestamps that do not match numeric season order. Assert events are ordered by `(begin_ts, season, stable match hash)` and summaries contain exactly those discovered seasons.

- [ ] **Step 2: Write failing idempotence and historical-backfill tests**

```rust
let first = rebuild_career_mmr(&db)?;
let second = rebuild_career_mmr(&db)?;
assert_eq!(first.event_digest, second.event_digest);

insert_older_match(&db)?;
let after_backfill = rebuild_career_mmr(&db)?;
assert_ne!(first.event_digest, after_backfill.event_digest);
assert_eq!(unaffected_prefix(&first), unaffected_prefix(&after_backfill));
```

- [ ] **Step 3: Verify replay tests fail**

Run:

```powershell
cargo test --test career_mmr_replay
```

Expected: FAIL because replay/persistence is absent.

- [ ] **Step 4: Implement versioned replay in one transaction**

```rust
pub const BASELINE_MODEL_VERSION: i64 = 1;

pub fn rebuild_career_mmr(db: &OwDatabase) -> Result<CareerMmrBuildSummary> {
    let mut conn = db.connect()?;
    let tx = conn.transaction()?;
    ensure_model_version(&tx, BASELINE_MODEL_VERSION, "role_bayesian_outcome_v1")?;
    let inputs = extract_all_inputs(&tx)?;
    replace_inputs(&tx, &inputs)?;
    replay_events(&tx, BASELINE_MODEL_VERSION, &inputs)?;
    rebuild_summaries(&tx, BASELINE_MODEL_VERSION)?;
    tx.commit()?;
    Ok(build_summary(&inputs))
}
```

Do not delete the previous complete active version before the replacement transaction commits. Use normalized non-sensitive fields for `input_hash` and event digest.

- [ ] **Step 5: Integrate rebuild after a successful import**

Call the replay orchestration only after core match/detail/player projections have successfully committed. A failed MMR rebuild must preserve the previous MMR version and return a Chinese non-sensitive warning; it must not roll back already-valid synced match data or expose raw JSON.

- [ ] **Step 6: Run replay and import regression tests**

Run:

```powershell
cargo test --test career_mmr_replay
cargo test --test import_baseline
```

Expected: dynamic-season, deterministic-order, idempotence, backfill, transaction rollback, and existing import tests PASS.

- [ ] **Step 7: Record the replay checkpoint**

```powershell
git diff --check
git status --short
```

### Task 7: Expose privacy-safe MMR queries and commands

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/query.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_queries.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing overview and dynamic-season query tests**

Assert the overview returns exactly three role cards, the discovered season list is not fixed, and a display filter never changes the stored current MMR.

```rust
assert_eq!(overview.roles.len(), 3);
assert_eq!(overview.discovered_seasons, vec![15, 20, 23]);
assert!(overview.disclaimer.contains("并非暴雪官方隐藏分"));
```

- [ ] **Step 2: Define DTOs once in Rust**

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerMmrOverview {
    pub model_version: i64,
    pub composite_mmr: Option<i64>,
    pub roles: Vec<RoleMmrSummary>,
    pub discovered_seasons: Vec<i64>,
    pub disclaimer: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleMmrSummary {
    pub role: String,
    pub current_mmr: i64,
    pub confidence: f64,
    pub confidence_label: String,
    pub peak_mmr: Option<i64>,
    pub peak_at: Option<i64>,
    pub effective_matches: i64,
}
```

Add history, mode, season, and single-event explanation DTOs using only numeric aggregates and Chinese labels.

- [ ] **Step 3: Implement query functions**

Implement:

```rust
get_career_mmr_overview(db)
get_role_mmr_history(db, request)
get_role_mmr_mode_breakdown(db, role)
get_role_mmr_season_breakdown(db, role)
get_mmr_match_explanation(db, match_id, role)
```

History filters are page-local display filters. They never rebuild the model from only the selected season.

- [ ] **Step 4: Add Tauri adapters and register handlers**

Use the existing `app_database(&app)?` and `map_err(to_message)` pattern. Register all five read commands plus explicit `rebuild_career_mmr` and `get_career_mmr_build_status` maintenance commands.

- [ ] **Step 5: Add recursive privacy assertions**

Serialize every DTO and assert the lowercase result contains none of:

```text
token, customertoken, cookie, authorization, credential, bnetid, playerid, rawjson
```

- [ ] **Step 6: Run query tests**

Run:

```powershell
cargo test --test career_mmr_queries
```

Expected: overview, history filters, dynamic seasons, explanation, missing-data Chinese fallback, and privacy tests PASS.

- [ ] **Step 7: Record the query-surface checkpoint**

```powershell
git diff --check
git status --short
```

### Task 8: Add the Career MMR product page

**Files:**
- Create: `apps/owstats-desktop/src/pages/CareerMmrPage.tsx`
- Create: `apps/owstats-desktop/src/shared/careerMmrFormat.ts`
- Create: `apps/owstats-desktop/src/styles/features/career-mmr.css`
- Create: `apps/owstats-desktop/scripts/careerMmrUi.test.ts`
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Modify: `apps/owstats-desktop/src/shared/phase6Shell.ts`
- Modify: `apps/owstats-desktop/src/App.tsx`
- Modify: `apps/owstats-desktop/src/styles.css`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Write the failing source-contract test**

```ts
assert.match(appSource, /path="\/career-mmr"/);
assert.match(shellSource, /生涯 MMR/);
assert.match(pageSource, /并非暴雪官方隐藏分/);
assert.match(pageSource, /重装/);
assert.match(pageSource, /输出/);
assert.match(pageSource, /支援/);
assert.doesNotMatch(pageSource, /S21.*S22.*S23/);
```

Also assert `getCareerMmrOverview` and `getRoleMmrHistory` are invoked through `shared/api.ts`, not calculated in the component.

- [ ] **Step 2: Run and verify the UI test fails**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/careerMmrUi.test.ts
```

Expected: FAIL because the route/page do not exist.

- [ ] **Step 3: Add TypeScript DTO mirrors and invoke wrappers**

```ts
export type CareerRole = "tank" | "damage" | "support";

export interface CareerMmrOverview {
  modelVersion: number;
  compositeMmr: number | null;
  roles: RoleMmrSummary[];
  discoveredSeasons: number[];
  disclaimer: string;
}

export function getCareerMmrOverview() {
  return invoke<CareerMmrOverview>("get_career_mmr_overview");
}
```

Mirror Rust field names exactly in camelCase. Add request/response types for history and mode/season breakdowns.

- [ ] **Step 4: Implement the page with page-owned filters**

Render:

- three primary role cards;
- optional secondary composite MMR;
- confidence label and percentage;
- peak MMR/date;
- dynamic season selector from `discoveredSeasons`;
- history list/line visualization with confidence band data;
- mode contribution cards;
- recent event explanations;
- loading, empty, and Chinese error states.

Do not add a global season context or sync-all action.

- [ ] **Step 5: Register route, nav, and styles**

Add one `phase6NavItems` entry for `/career-mmr`, one route in `App.tsx`, and import `./styles/features/career-mmr.css` from `styles.css`.

- [ ] **Step 6: Add the test to the unit suite and run frontend checks**

Run:

```powershell
npx tsx scripts/careerMmrUi.test.ts
npm test
npm run build
```

Expected: new source-contract test PASS; full frontend suite PASS; TypeScript/Vite build PASS.

- [ ] **Step 7: Record the UI checkpoint**

```powershell
git diff --check
git status --short
```

### Task 9: Run full validation and capture the weight-study handoff

**Files:**
- Modify: `docs/superpowers/specs/2026-07-13-career-role-mmr-design.md`
- Create: `docs/superpowers/plans/2026-07-13-career-role-mmr-weight-study.md`

- [ ] **Step 1: Run the aggregate audit against the real local database**

Run the explicit binary with the app-data database path resolved by the same app path helper used in production. Expected output contains only counts, coverage ratios, season numbers, and Stadium shape totals. Confirm no raw IDs, names, GUIDs, raw JSON, credentials, or token-like fields appear.

- [ ] **Step 2: Run backend formatting and tests**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: both commands exit 0.

- [ ] **Step 3: Run frontend tests and build**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: all test groups and build exit 0.

- [ ] **Step 4: Run release build and smoke checks**

```powershell
npm run tauri build
```

Expected: installer/bundle build exits 0. Install or launch the produced build against copied non-sensitive fixture/app-data, open “生涯 MMR”, and verify three roles, dynamic seasons, Chinese states, and no development absolute path dependency.

- [ ] **Step 5: Run repository hygiene checks**

```powershell
git diff --check
git status --short
```

Expected: no whitespace errors; status contains only task-owned files plus the user's pre-existing changes. Do not stage or alter unrelated files.

- [ ] **Step 6: Write the separate weight-study plan from measured audit results**

The follow-up plan must include:

- self-healing semantics for `healingTaken`;
- ordinary exact hero-time availability;
- Stadium local-versus-cumulative last-round classification;
- final-blow multiplier grid `1.00–2.00`;
- excess-healing/resource-debt alternatives;
- five rolling time folds;
- season and hero holdouts;
- 1,000 match-cluster bootstraps;
- shadow-model storage and promotion gates;
- no production performance modifier until all gates pass.

- [ ] **Step 7: Record the validation-document checkpoint after all checks pass**

```powershell
git diff --check
git status --short
```

## Plan self-review result

- Spec coverage: baseline rating, dynamic seasons, ordinary fallback, Stadium last-round-only rule, per-role caps, confidence, replay, API, UI, privacy, migration, and release checks each have an owning task.
- Deliberate follow-up: learned role weights, final-blow production modifiers, resource-debt production modifiers, and shadow promotion are excluded until Task 9 produces the measured audit handoff.
- Type consistency: Rust roles are `tank|damage|support`; upstream `dps|healer` exist only at normalization boundaries; TypeScript mirrors the normalized values.
- Placeholder scan: the plan contains no unresolved placeholder tokens or unspecified test steps.
