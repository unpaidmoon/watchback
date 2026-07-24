# Authorized Career MMR Training Dataset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a development-only, resumable batch collector that turns 20 authorized `customerToken` values across S15/S20/S21/S22/S23 into isolated per-participant SQLite training sources and an anonymous coverage report.

**Architecture:** Run all implementation in a dedicated Git worktree, while keeping credentials, participant manifests, raw payloads, SQLite files, checkpoints, and outputs in an operator-selected directory outside every Git worktree. A focused `training_dataset` module validates private inputs, provisions anonymous account sources, delegates season work to the existing sync/import pipeline, records atomic secret-free checkpoints, and audits the resulting sources through the existing Career MMR projection.

**Tech Stack:** Rust 2021, Tokio, Reqwest, Serde/serde_json, Rusqlite, Tauri path/account abstractions, existing Watchback season sync/import and Career MMR training projection.

---

## Scope boundary

This plan implements phase one of
`docs/superpowers/specs/2026-07-24-career-mmr-v4-authorized-training-dataset-design.md`.
It ends after the anonymous coverage gate.

Do not implement v4 rating mathematics, artifact activation, migrations, UI, or release packaging in this
plan. Write the v4 experiment implementation plan only after the real coverage report passes. This split is
the early-stop mechanism agreed in the design.

## File map

**Create**

- `apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs` — public types and orchestration boundary.
- `apps/owstats-desktop/src-tauri/src/training_dataset/manifest.rs` — strict private manifest parsing and validation.
- `apps/owstats-desktop/src-tauri/src/training_dataset/paths.rs` — safe private-root path validation.
- `apps/owstats-desktop/src-tauri/src/training_dataset/checkpoint.rs` — atomic anonymous checkpoint state.
- `apps/owstats-desktop/src-tauri/src/training_dataset/provision.rs` — identity verification and isolated account provisioning.
- `apps/owstats-desktop/src-tauri/src/training_dataset/runner.rs` — sequential 100-unit batch execution.
- `apps/owstats-desktop/src-tauri/src/training_dataset/report.rs` — anonymous collection and coverage reports.
- `apps/owstats-desktop/src-tauri/src/bin/collect_career_mmr_training_data.rs` — development-only collector CLI.
- `apps/owstats-desktop/src-tauri/src/bin/audit_career_mmr_training_dataset.rs` — read-only coverage gate CLI.
- `apps/owstats-desktop/src-tauri/tests/training_dataset_manifest.rs` — manifest and path contract tests.
- `apps/owstats-desktop/src-tauri/tests/training_dataset_checkpoint.rs` — state transition and atomicity tests.
- `apps/owstats-desktop/src-tauri/tests/training_dataset_provision.rs` — synthetic account isolation tests.
- `apps/owstats-desktop/src-tauri/tests/training_dataset_batch.rs` — synthetic HTTP, retry, resume and privacy tests.
- `apps/owstats-desktop/src-tauri/tests/training_dataset_audit.rs` — synthetic multi-account coverage tests.

**Modify**

- `apps/owstats-desktop/src-tauri/src/lib.rs` — export the development training dataset module.
- `apps/owstats-desktop/src-tauri/src/dashen_client.rs` — expose an injected shared request gate if required by tests and batch orchestration; do not change production defaults.
- `apps/owstats-desktop/src-tauri/src/player_target.rs` — add an atomic writer variant used by private training targets.
- `apps/owstats-desktop/src-tauri/src/sync/facade.rs` — expose a structured batch result only if the existing `SyncSummary` lacks required anonymous counts.
- `apps/owstats-desktop/src-tauri/src/career_mmr/training.rs` — expose read-only anonymous coverage data already present in `TrainingProjection`; do not expose rows or identities.

## Task 1: Create the isolated implementation worktree

**Files:**

- No repository file changes.

- [ ] **Step 1: Recheck the source worktree**

Run:

```powershell
git status --short
git branch --show-current
git rev-parse --short HEAD
```

Expected: empty status, branch `codex/code-review`, and HEAD containing design commit `18a4b36`.

- [ ] **Step 2: Use the required worktree skill**

Invoke `superpowers:using-git-worktrees`. Resolve and validate these exact targets before creating anything:

```text
branch: codex/career-mmr-v4-training
path: D:\Codex\OW_data\owstats-career-mmr-v4-training
```

Expected: the resolved worktree target is outside
`D:\Codex\OW_data\owstats-local`, does not already contain unrelated files, and starts at the approved HEAD.

- [ ] **Step 3: Verify worktree isolation**

Run in the new worktree:

```powershell
git branch --show-current
git status --short
git merge-base --is-ancestor 18a4b36 HEAD
```

Expected: branch `codex/career-mmr-v4-training`, empty status, exit code 0 for ancestry.

## Task 2: Add strict manifest and private-root validation

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs`
- Create: `apps/owstats-desktop/src-tauri/src/training_dataset/manifest.rs`
- Create: `apps/owstats-desktop/src-tauri/src/training_dataset/paths.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/training_dataset_manifest.rs`

- [ ] **Step 1: Write failing manifest tests**

Add tests that deserialize only the approved schema and reject duplicate IDs, duplicate customer tokens,
wrong season sets, unknown fields, non-canonical IDs, empty values, and a private root inside the repository.

```rust
#[test]
fn accepts_exact_authorized_manifest() {
    let manifest = parse_manifest(
        br#"{
          "schemaVersion":1,
          "seasons":[15,20,21,22,23],
          "participants":[
            {"datasetId":"dataset-001","customerToken":"synthetic-customer-001"},
            {"datasetId":"dataset-002","customerToken":"synthetic-customer-002"}
          ]
        }"#,
    )
    .unwrap();
    assert_eq!(manifest.seasons, [15, 20, 21, 22, 23]);
    assert_eq!(manifest.participants.len(), 2);
}

#[test]
fn rejects_private_root_inside_repository() {
    let error = TrainingDatasetPaths::new(repo_root(), repo_root().join("private")).unwrap_err();
    assert_eq!(error.to_string(), "训练数据目录必须位于代码工作树之外");
}
```

- [ ] **Step 2: Run tests and verify red**

Run:

```powershell
cargo test --test training_dataset_manifest
```

Expected: compilation fails because `training_dataset` does not exist.

- [ ] **Step 3: Implement strict types**

Define:

```rust
pub const TRAINING_SEASONS: [i64; 5] = [15, 20, 21, 22, 23];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingDatasetManifest {
    pub schema_version: u32,
    pub seasons: Vec<i64>,
    pub participants: Vec<TrainingParticipant>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingParticipant {
    pub dataset_id: String,
    pub customer_token: String,
}
```

`parse_manifest` must trim only for validation, preserve the original secret string for requests, require
exact schema version 1, exact normalized season set, exactly 20 participants for the real CLI, canonical
`dataset-\d{3}`, and uniqueness of both fields. Provide an internal test constructor that permits 1–20
synthetic participants so integration tests stay small.

`TrainingDatasetPaths::new(repo_root, private_root)` must canonicalize existing ancestors, reject equality or
containment in either direction, reject links/reparse points for existing components, and return fixed Chinese
errors without echoing either path.

- [ ] **Step 4: Export the module**

Add to `src/lib.rs`:

```rust
#[doc(hidden)]
pub mod training_dataset;
```

Do not export it through Tauri commands or frontend bindings.

- [ ] **Step 5: Run focused tests**

Run:

```powershell
cargo test --test training_dataset_manifest
```

Expected: all manifest and path tests pass.

- [ ] **Step 6: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs apps/owstats-desktop/src-tauri/src/training_dataset/manifest.rs apps/owstats-desktop/src-tauri/src/training_dataset/paths.rs apps/owstats-desktop/src-tauri/tests/training_dataset_manifest.rs
git commit -m "feat: validate private Career MMR training manifests"
```

## Task 3: Implement anonymous atomic checkpoints

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/training_dataset/checkpoint.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/training_dataset_checkpoint.rs`

- [ ] **Step 1: Write failing state tests**

Cover allowed transitions, completed skip, partial retry, failed retry, corrupt checkpoint rejection, atomic
replacement, and forbidden secret keys.

```rust
#[test]
fn completed_unit_is_skipped_without_refresh() {
    let checkpoint = UnitCheckpoint::completed("dataset-001", 15, 420, 417, 3, 1);
    assert_eq!(checkpoint.next_action(false), UnitAction::Skip);
    assert_eq!(checkpoint.next_action(true), UnitAction::FullRefresh);
}

#[test]
fn serialized_checkpoint_contains_no_secret_fields() {
    let encoded = serde_json::to_string(&UnitCheckpoint::pending("dataset-001", 15)).unwrap();
    for forbidden in ["customerToken", "token", "battleTag", "authorization", "cookie"] {
        assert!(!encoded.to_ascii_lowercase().contains(&forbidden.to_ascii_lowercase()));
    }
}
```

- [ ] **Step 2: Verify red**

Run:

```powershell
cargo test --test training_dataset_checkpoint
```

Expected: compilation fails because checkpoint types are missing.

- [ ] **Step 3: Implement checkpoint types and transition validation**

Use:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitState {
    Pending,
    FetchingList,
    FetchingDetails,
    Importing,
    Validating,
    Completed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnitCheckpoint {
    pub schema_version: u32,
    pub dataset_id: String,
    pub season: i64,
    pub state: UnitState,
    pub entries: usize,
    pub details: usize,
    pub failed_details: usize,
    pub attempts: u32,
}
```

Store one file per unit under `.training-operations/checkpoints/<dataset-id>/s<season>.json`.
Write with same-directory temporary file, `flush`, `sync_all`, and atomic rename. Validate the exact expected
dataset ID and season on every read.

- [ ] **Step 4: Run tests**

Run:

```powershell
cargo test --test training_dataset_checkpoint
```

Expected: all checkpoint tests pass.

- [ ] **Step 5: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs apps/owstats-desktop/src-tauri/src/training_dataset/checkpoint.rs apps/owstats-desktop/src-tauri/tests/training_dataset_checkpoint.rs
git commit -m "feat: add resumable anonymous training checkpoints"
```

## Task 4: Provision isolated participant accounts

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/training_dataset/provision.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/player_target.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/training_dataset_provision.rs`

- [ ] **Step 1: Write failing provisioning tests**

Use only synthetic card payloads. Verify stable resume mapping, separate databases, exact identity
verification, no credential copy, and atomic target writes.

```rust
#[test]
fn provisioning_keeps_participants_in_distinct_account_roots() {
    let first = provision_synthetic("dataset-001", "Alpha#1001");
    let second = provision_synthetic("dataset-002", "Bravo#1002");
    assert_ne!(first.account_id, second.account_id);
    assert_ne!(first.database_path, second.database_path);
    assert!(!first.account_root.join("credentials.json").exists());
}

#[test]
fn resume_reuses_dataset_account_mapping() {
    let first = provision_synthetic("dataset-001", "Alpha#1001");
    let second = provision_synthetic("dataset-001", "Alpha#1001");
    assert_eq!(first.account_id, second.account_id);
}
```

- [ ] **Step 2: Verify red**

Run:

```powershell
cargo test --test training_dataset_provision
```

Expected: compilation fails because provisioning does not exist.

- [ ] **Step 3: Add a private dataset index**

Create `.training-operations/dataset-index.json` with:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DatasetIndex {
    schema_version: u32,
    participants: BTreeMap<String, AccountId>,
}
```

The map contains only anonymous dataset IDs and random account UUIDs. It must never contain BattleTag or
credentials. Use `account_store::write_registry_atomic` and `write_manifest_atomic` for official account
structures.

- [ ] **Step 4: Verify each target through `queryCard`**

Construct `DashenClient` from the shared credential file and call `query_card(customer_token)`. Parse the
returned identity from the card response using a narrowly scoped function:

```rust
fn verified_battle_tag(card: &Value) -> Result<String> {
    let value = card
        .pointer("/data/bnetId")
        .or_else(|| card.pointer("/data/battleTag"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("参与者身份验证失败"))?;
    Ok(value.to_owned())
}
```

Confirm the upstream business success code before accepting identity. Never include the response or identity
in errors.

- [ ] **Step 5: Add atomic player-target writing**

Add `save_player_target_atomic_for_path` next to the existing writer. Use same-directory temporary write,
flush, sync and rename. Do not change existing runtime semantics.

The private target contains the actual identity and `customerToken` because the existing sync path needs both:

```rust
PlayerTarget {
    input_bnet: verified_identity.clone(),
    resolved_bnet: Some(verified_identity),
    customer_token: participant.customer_token.clone(),
    bound_at: now,
    source_role_id: Some(credentials.role_id),
}
```

- [ ] **Step 6: Run tests**

Run:

```powershell
cargo test --test training_dataset_provision
cargo test --test login_target
```

Expected: provisioning tests and existing target tests pass.

- [ ] **Step 7: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/player_target.rs apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs apps/owstats-desktop/src-tauri/src/training_dataset/provision.rs apps/owstats-desktop/src-tauri/tests/training_dataset_provision.rs
git commit -m "feat: provision isolated authorized training accounts"
```

## Task 5: Build the sequential resumable batch runner

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/training_dataset/runner.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_client.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/facade.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/training_dataset_batch.rs`

- [ ] **Step 1: Write failing orchestration tests**

Use a synthetic local HTTP server and two participants × two seasons through the internal test constructor.
Assert maximum one active unit, detail concurrency at most two, global request rate gate reuse, completion skip,
partial retry, 429 backoff, and auth failure batch stop.

```rust
#[tokio::test]
async fn auth_failure_stops_before_next_unit() {
    let server = SyntheticDashen::auth_failure_after_requests(1);
    let result = run_synthetic_batch(server.url()).await.unwrap_err();
    assert_eq!(result.to_string(), "登录凭据已失效，批量采集已停止");
    assert_eq!(server.requested_units(), vec![("dataset-001", 15)]);
}

#[tokio::test]
async fn completed_units_are_not_requested_again() {
    let first = run_synthetic_batch(server.url()).await.unwrap();
    let second = run_synthetic_batch(server.url()).await.unwrap();
    assert_eq!(first.completed_units, 4);
    assert_eq!(second.skipped_units, 4);
    assert_eq!(server.request_count_after(second), 0);
}
```

- [ ] **Step 2: Verify red**

Run:

```powershell
cargo test --test training_dataset_batch
```

Expected: compilation fails because the runner is missing.

- [ ] **Step 3: Implement runner request types**

```rust
pub struct BatchRunRequest {
    pub repo_root: PathBuf,
    pub private_root: PathBuf,
    pub credential_path: PathBuf,
    pub manifest_path: PathBuf,
    pub refresh: bool,
    pub customer_base_url: String,
}

pub struct BatchRunSummary {
    pub planned_units: usize,
    pub completed_units: usize,
    pub partial_units: usize,
    pub failed_units: usize,
    pub skipped_units: usize,
}
```

Do not include identities, account IDs, paths, tokens or upstream messages in `BatchRunSummary`.

- [ ] **Step 4: Delegate each unit to the existing sync pipeline**

For each participant in manifest order and each canonical season:

```rust
sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url(
    &OwDatabase::for_account(&account_paths),
    &account_paths.seasons(),
    &request.credential_path,
    &account_paths.player_target(),
    season,
    mode,
    &request.customer_base_url,
    |progress| anonymous_progress(dataset_id, &progress),
)
.await
```

Use `SyncMode::FullFetch` for pending/failed/refresh and the existing safe incremental behavior for partial
units. If current incremental behavior cannot target failed details, extend the sync layer with a
`RetryFailedDetails` mode rather than duplicating HTTP/import code in the runner.

- [ ] **Step 5: Share one request gate across all units**

Refactor `DashenClient` construction only as needed so batch clients share one
`Arc<DashenRequestGate>`. Preserve `DashenClient::new` and all production defaults. Add a constructor usable by
the runner:

```rust
pub fn with_shared_gate(
    credentials: DashenCredentials,
    base_url: String,
    request_gate: Arc<DashenRequestGate>,
) -> Result<Self>
```

Keep the gate type or constructor visibility at `pub(crate)` unless integration-test access requires a
`#[doc(hidden)]` wrapper.

- [ ] **Step 6: Classify fatal authentication without leaking upstream text**

Map only confirmed HTTP/business authentication failures to:

```text
登录凭据已失效，批量采集已停止
```

Other failures update the unit to `failed`, preserve the raw error in the existing private sync error surface,
and continue only when doing so cannot create repeated credential failures.

- [ ] **Step 7: Run focused tests**

Run:

```powershell
cargo test --test training_dataset_batch
cargo test --test phase8_real_fetch
cargo test --test season_sync_state
```

Expected: batch behavior and existing sync regressions pass.

- [ ] **Step 8: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/dashen_client.rs apps/owstats-desktop/src-tauri/src/sync/facade.rs apps/owstats-desktop/src-tauri/src/training_dataset/mod.rs apps/owstats-desktop/src-tauri/src/training_dataset/runner.rs apps/owstats-desktop/src-tauri/tests/training_dataset_batch.rs
git commit -m "feat: collect authorized training seasons with resume"
```

## Task 6: Add secret-free collection reporting

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/training_dataset/report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/training_dataset/runner.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/training_dataset_batch.rs`

- [ ] **Step 1: Add failing privacy contract tests**

Capture stdout/stderr and serialized reports from synthetic values deliberately shaped like secrets and
identities:

```rust
#[test]
fn public_batch_outputs_are_anonymous() {
    let output = synthetic_batch_output();
    for secret in [
        "synthetic-login-secret",
        "synthetic-customer-001",
        "Alpha#1001",
        "authorization",
        "cookie",
    ] {
        assert!(!output.contains(secret));
    }
    assert!(output.contains("dataset-001"));
    assert!(output.contains("planned_units"));
}
```

- [ ] **Step 2: Verify red**

Run:

```powershell
cargo test --test training_dataset_batch public_batch_outputs_are_anonymous -- --exact
```

Expected: test fails because report output is missing or leaks synthetic source errors.

- [ ] **Step 3: Implement anonymous reports**

Define report fields:

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionReport {
    pub schema_version: u32,
    pub participants_planned: usize,
    pub participants_verified: usize,
    pub planned_units: usize,
    pub completed_units: usize,
    pub partial_units: usize,
    pub failed_units: usize,
    pub skipped_units: usize,
    pub entries: usize,
    pub details: usize,
    pub failed_details: usize,
    pub elapsed_ms: u64,
}
```

Print progress as one line containing stage, `dataset-###`, season, anonymous counts and elapsed time. Do not
print source paths, match IDs, current payload lines or upstream messages.

- [ ] **Step 4: Run tests**

Run:

```powershell
cargo test --test training_dataset_batch
```

Expected: all batch and privacy tests pass.

- [ ] **Step 5: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/training_dataset/report.rs apps/owstats-desktop/src-tauri/src/training_dataset/runner.rs apps/owstats-desktop/src-tauri/tests/training_dataset_batch.rs
git commit -m "feat: report anonymous training collection progress"
```

## Task 7: Add the development-only collection CLI

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/bin/collect_career_mmr_training_data.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/training_dataset_batch.rs`

- [ ] **Step 1: Write failing CLI parser tests**

Require exactly these options:

```text
--repo-root PATH
--private-root PATH
--credentials PATH
--manifest PATH
[--refresh]
```

Reject positional credentials, direct `--token`, direct `--role-id`, direct `--customer-token`, unknown options,
duplicates, missing values and output paths inside the repository.

- [ ] **Step 2: Verify red**

Run:

```powershell
cargo test --test training_dataset_batch cli_rejects_direct_secret_arguments -- --exact
```

Expected: fails because the CLI parser does not exist.

- [ ] **Step 3: Implement the CLI**

The CLI entry point should remain thin:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let request = CliArgs::parse(std::env::args_os().skip(1))?.into_request()?;
    let summary = run_batch(request, |event| eprintln!("{}", event.render())).await?;
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}
```

Use the production base URL internally. Permit base URL injection only through test helpers, never a normal
CLI flag.

- [ ] **Step 4: Run parser and batch tests**

Run:

```powershell
cargo test --test training_dataset_batch
cargo run --bin collect_career_mmr_training_data -- --help
```

Expected: tests pass; help exits without reading files or displaying any secret-bearing example.

- [ ] **Step 5: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/bin/collect_career_mmr_training_data.rs apps/owstats-desktop/src-tauri/tests/training_dataset_batch.rs
git commit -m "feat: add authorized Career MMR collection CLI"
```

## Task 8: Add the read-only anonymous coverage gate

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/bin/audit_career_mmr_training_dataset.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/training_dataset/report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/training_dataset_audit.rs`

- [ ] **Step 1: Write failing audit tests**

Build two synthetic account databases with multiple seasons and modes. Verify:

- all registered databases are opened query-only;
- missing schema fails the whole audit;
- an entirely empty participant fails completeness;
- ordinary and stadium projection counts are nonzero;
- duplicate import does not change row counts;
- output contains only anonymous aggregate groups.

```rust
#[test]
fn audit_rejects_zero_stadium_projection() {
    let error = audit_fixture_without_stadium().unwrap_err();
    assert_eq!(error.to_string(), "角斗训练投影为空，停止 v4 实验");
}

#[test]
fn audit_reports_only_anonymous_groups() {
    let report = audit_complete_fixture().unwrap();
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(encoded.contains("dataset-001"));
    assert!(!encoded.contains("Alpha#1001"));
}
```

- [ ] **Step 2: Verify red**

Run:

```powershell
cargo test --test training_dataset_audit
```

Expected: compilation fails because the audit API is missing.

- [ ] **Step 3: Expose aggregate projection coverage**

Add a read-only method or companion struct that consumes `TrainingProjection` internally and returns only:

```rust
pub struct AnonymousProjectionCoverage {
    pub source_accounts: usize,
    pub discovered_seasons: BTreeMap<i64, usize>,
    pub rows_by_role: BTreeMap<String, usize>,
    pub rows_by_mode: BTreeMap<String, usize>,
    pub trainable_rows: usize,
    pub excluded_rows: usize,
}
```

Do not expose training rows, match IDs, BattleTags, account IDs, file paths or payloads.

- [ ] **Step 4: Implement the audit CLI**

Accept:

```text
--private-root PATH
--report PATH
```

Require the report path under the selected private root's `outputs` directory, open sources using
`discover_training_sources`/`open_query_only`, build the current multi-account projection, evaluate the
collection gates, and atomically write the anonymous JSON report.

- [ ] **Step 5: Run audit tests**

Run:

```powershell
cargo test --test training_dataset_audit
cargo test --test career_mmr_training_projection
```

Expected: audit and existing projection tests pass.

- [ ] **Step 6: Commit**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/bin/audit_career_mmr_training_dataset.rs apps/owstats-desktop/src-tauri/src/training_dataset/report.rs apps/owstats-desktop/src-tauri/src/career_mmr/training.rs apps/owstats-desktop/src-tauri/tests/training_dataset_audit.rs
git commit -m "feat: gate v4 experiments on anonymous dataset coverage"
```

## Task 9: Run complete synthetic verification

**Files:**

- Modify only files required by failures directly caused by Tasks 2–8.

- [ ] **Step 1: Format**

Run:

```powershell
cargo fmt --check
```

Expected: pass. If formatting differs, run `cargo fmt`, inspect the exact diff, then rerun the check.

- [ ] **Step 2: Run focused suites**

Run:

```powershell
cargo test --test training_dataset_manifest
cargo test --test training_dataset_checkpoint
cargo test --test training_dataset_provision
cargo test --test training_dataset_batch
cargo test --test training_dataset_audit
cargo test --test phase8_real_fetch
cargo test --test season_sync_state
cargo test --test career_mmr_training_projection
cargo test --test login_target
```

Expected: every suite passes using only synthetic data.

- [ ] **Step 3: Run the broader backend gate**

Before starting, confirm there is no other Cargo build sharing the target directory:

```powershell
Get-Process cargo,rustc -ErrorAction SilentlyContinue
cargo test --tests
```

Expected: all non-ignored backend tests pass. If the outer tool times out, inspect Cargo/Rustc state before
starting any second build, per `AGENTS.md`.

- [ ] **Step 4: Run repository checks**

Run:

```powershell
git diff --check
git status --short
```

Expected: no whitespace errors and only expected implementation files.

- [ ] **Step 5: Commit verification-only fixes if any**

If Steps 1–4 required changes, return to the task that owns the affected file, repeat that task's exact
test and staging steps, and create its listed task-specific commit. If no file changed, continue without an
empty verification commit.

## Task 10: Execute the authorized private batch

**Files:**

- No repository file changes.
- Private operator files only.

- [ ] **Step 1: Validate private inputs without reading secret contents into logs**

Confirm only existence, regular-file type, size and containment:

```powershell
$trainingPrivateRoot = 'D:\Codex\OW_data-private\career-mmr-v4-dataset'
$credentialsPath = Join-Path $trainingPrivateRoot 'credentials.json'
$manifestPath = Join-Path $trainingPrivateRoot 'participants.json'
Get-Item -LiteralPath $credentialsPath,$manifestPath |
  Select-Object Name,Length,Attributes
```

Expected: two regular files under the approved private root. Do not print their contents.

- [ ] **Step 2: Run the release collector**

Run from `apps\owstats-desktop\src-tauri`:

```powershell
cargo run --release --bin collect_career_mmr_training_data -- `
  --repo-root 'D:\Codex\OW_data\owstats-career-mmr-v4-training' `
  --private-root $trainingPrivateRoot `
  --credentials $credentialsPath `
  --manifest $manifestPath
```

Expected: anonymous progress for exactly 100 planned units. Do not redirect output into the repository.

- [ ] **Step 3: Monitor without exposing private state**

Monitor only process state and anonymous stdout:

```powershell
Get-Process collect_career_mmr_training_data,cargo,rustc -ErrorAction SilentlyContinue |
  Select-Object ProcessName,CPU,WorkingSet,StartTime
```

Expected: no credential, identity, path, match ID or payload output. Do not kill a progressing process merely
because an outer tool timed out.

- [ ] **Step 4: Run the release audit**

```powershell
$coveragePath = Join-Path $trainingPrivateRoot 'outputs\coverage.json'
cargo run --release --bin audit_career_mmr_training_dataset -- `
  --private-root $trainingPrivateRoot `
  --report $coveragePath
```

Expected: the report either passes all collection gates or exits with a fixed anonymous failure reason.

- [ ] **Step 5: Apply the early-stop decision**

If any of these occurs, stop and do not write the v4 experiment plan:

```text
verified participants < 20
planned units != 100
any participant has zero usable seasons
any source database fails schema/read-only validation
ordinary projection == 0
stadium projection == 0
idempotency check changes counts
```

Report actual elapsed time, anonymous counts, failed gate, retry recommendation and whether v2 remains active.

- [ ] **Step 6: Hand off to v4 planning only after success**

If the coverage gate passes, write a separate implementation plan against:

```text
docs/superpowers/specs/2026-07-21-career-mmr-v4-evidence-rating-design.md
```

That next plan must begin with the unified production/training projector and retain the 30-minute release
experiment hard limit. It must not assume that Glicko-2 or TrueSkill beats v2.

## Final delivery checklist

- [ ] Isolated branch and worktree identified.
- [ ] Exact commits from Tasks 2–9 listed.
- [ ] Synthetic verification commands and outcomes listed.
- [ ] Real run reports only anonymous participant/unit/match/detail counts.
- [ ] Partial and failed units distinguished from completed units.
- [ ] Coverage gate result stated without qualification.
- [ ] Unverified items listed.
- [ ] Real privacy-data contact stated: the collector processed private credentials and authorized raw data;
      the agent did not read, echo, or commit their contents.
- [ ] `git diff --check` and `git status --short` results reported.
