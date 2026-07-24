# Install Data Final Security Closeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining install-data publication gaps without weakening the existing Windows DACL, fixed-handle, no-reparse, or raw-data contracts.

**Architecture:** Treat an existing `data` directory as published only when it contains a valid migration marker or the versioned startup root marker. Validate root and account SQLite databases through the same private fixed-handle snapshot path under a single injected validation budget, and reject hardlinks before either copy or deletion can affect an external inode.

**Tech Stack:** Rust, Tauri 2, Windows NT fixed-handle APIs, rusqlite/SQLite, integration tests.

---

### Task 1: Published-data proof and atomic initialization

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/install_data_migration.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`

- [x] **Step 1: Write failing existing-data conflict tests**

Update the Windows and non-Windows publish-race tests so a concurrently inserted empty `data` directory makes both the first publish and the retry fail, while source and `data.migrating` remain untouched. Add assertions that `Created` writes `.watchback-data-root-v1` with `v1`.

- [x] **Step 2: Verify the conflict tests fail**

Run:

```powershell
cargo test --test install_directory_data refuses_a_destination_created_immediately_before_handle_publish_and_retries_safely -- --exact
cargo test --test install_directory_data no_source_creates_an_empty_data_directory -- --exact
```

Expected: the retry incorrectly returns `ExistingData`, and the created directory lacks the root marker.

- [x] **Step 3: Implement explicit publication proof**

Define shared constants:

```rust
pub(crate) const INSTALL_DATA_ROOT_MARKER: &str = ".watchback-data-root-v1";
pub(crate) const INSTALL_DATA_ROOT_MARKER_CONTENTS: &[u8] = b"v1";
const INSTALL_DATA_MIGRATION_MARKER: &str = ".install-data-migration-v1.json";
```

On Windows, classify `data` through the pinned install-parent handle. Accept a regular bounded migration marker whose JSON is exactly version 1 with `sourceRetained: true`, or a regular root marker containing either `v1` or the empty interrupted-start state. Reject an existing directory with neither proof. For `Created`, atomically create a protected staging/data directory, write and sync the root marker before returning. For migration, write both markers before publication. Keep startup compatible with an empty interrupted root marker while sharing the same marker name and version.

- [x] **Step 4: Verify marker tests pass**

Run both commands from Step 2 and the normal startup marker tests. Expected: PASS.

### Task 2: Root SQLite validation

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/install_data_migration.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`

- [x] **Step 1: Write failing root database tests**

Add tests proving a corrupt root `owstats.sqlite3` is not published, root `-wal`/`-shm` without the main database is rejected, and a valid root database without `accounts.json` is published without `.install-data-validation-*` artifacts.

- [x] **Step 2: Verify the tests fail**

Run:

```powershell
cargo test --test install_directory_data root_
```

Expected: corrupt/orphan root inputs are currently published because validation returns when `accounts.json` is absent.

- [x] **Step 3: Reuse the private snapshot validator**

Call `validate_database_snapshot(staging, ...)` before opening optional `accounts.json`. When the main database is absent, probe both sidecars with exact NTSTATUS missing semantics and reject either existing sidecar. Keep all opened files guarded without DELETE sharing through marker write and final validation.

- [x] **Step 4: Verify root database tests pass**

Run the Step 2 command. Expected: PASS.

### Task 3: Validation resource budgets

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/Cargo.toml`
- Modify: `apps/owstats-desktop/src-tauri/src/install_data_migration.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`

- [x] **Step 1: Write failing injected-budget tests**

Expose a hidden Windows-only `WindowsValidationLimits` plus thread-local setter. Add small-limit tests for total JSON metadata bytes, account count, simultaneous handles, cumulative snapshot bytes across root/account databases, and quick-check progress callbacks.

- [x] **Step 2: Verify tests fail**

Run:

```powershell
cargo test --test install_directory_data validation_budget_
```

Expected: the API or limits do not yet exist.

- [x] **Step 3: Implement one validation budget**

Track bounded metadata reads before allocation, reject registries over the account limit, check retained plus temporary handles before every validation open/create, charge actual snapshot bytes cumulatively, and install a rusqlite progress handler with a fixed opcode interval and callback ceiling. Map all limit failures to stable Chinese errors containing no path, account ID, row, payload, or credential data.

- [x] **Step 4: Verify budget tests pass**

Run the Step 2 command. Expected: PASS.

### Task 4: Hardlink fail-closed regressions

**Files:**
- Test: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`

- [x] **Step 1: Add hardlink migration and deletion tests**

Create a synthetic external sentinel and a hardlink inside the legacy source; assert migration fails without publishing or changing the sentinel. Create a hardlink inside stale `data.migrating`; assert reset/delete fails without deleting either link or changing the sentinel.

- [x] **Step 2: Run the hardlink tests**

Run:

```powershell
cargo test --test install_directory_data hardlink_
```

Expected: PASS if the existing `nNumberOfLinks == 1` fixed-handle checks cover both paths; otherwise add only the missing fixed-handle check and rerun.

### Task 5: Final verification and commit

**Files:**
- Verify all modified files above.

- [x] **Step 1: Run required regressions**

```powershell
cargo test --test install_directory_data
cargo test --test phase9_multi_account
cargo test --test raw_runtime_data
cargo fmt --check
git diff --check
git status --short
```

Expected: all tests and checks pass; status contains only planned files.

- [x] **Step 2: Self-review and commit**

Confirm no real private data was accessed, errors contain no private identifiers, and the DACL/fixed-handle guard lifetime is unchanged. Then stage only planned files and commit:

```powershell
git commit -m "fix: close install data validation gaps"
```
