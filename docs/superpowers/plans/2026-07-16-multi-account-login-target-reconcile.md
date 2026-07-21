# Multi-Account Login Target Reconcile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure successful account addition and relogin immediately reconcile the captured BattleTag into the account-scoped season-sync target.

**Architecture:** Keep account publication responsible only for durable account/credential state. In the Tauri command layer, after publication succeeds, resolve the published account snapshot and pass its saved credentials, captured BattleTag, account-scoped `player_target` path, and `LoginTargetCoordinator` to the existing `reconcile_detected_target` workflow before returning to the frontend.

**Tech Stack:** Rust, Tauri 2, Tokio, existing `AccountContextManager` and `LoginTargetCoordinator` modules.

---

### Task 1: Add the missing command contract

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`

- [ ] **Step 1: Write the failing test**

Add a source contract test that isolates both `add_account` and `relogin_account` command bodies and asserts each calls a shared post-publication target reconciliation helper.

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-login-target-tdd'
cargo test --test phase4_login_redesign published_login_commands_reconcile_the_detected_sync_target
```

Expected: FAIL because neither command currently calls the reconciliation helper.

### Task 2: Wire published accounts into the target coordinator

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`

- [ ] **Step 1: Add the minimal helper**

Implement an async helper that:

1. Parses `AccountLoginResult.account_id`.
2. Loads that exact account with `snapshot_for`.
3. Reads the just-published credentials.
4. Calls `login_target::reconcile_detected_target` with `result.battle_tag`, the account-scoped `player_target` path, and the existing Dashen root base URL.
5. Leaves binding failures represented by `LoginTargetCoordinator` instead of converting a successful login into a credential failure.

- [ ] **Step 2: Call the helper from both commands**

Inject `LoginTargetCoordinator` into `add_account` and `relogin_account`, await account publication, then await reconciliation before returning the unchanged `AccountLoginResult`.

- [ ] **Step 3: Run the focused test**

Run the Task 1 command. Expected: PASS.

### Task 3: Verify behavior and runtime

**Files:**
- Verify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Verify: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`

- [ ] **Step 1: Run focused backend suites**

```powershell
cargo fmt --check
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-login-target-tdd'
cargo test --test phase4_login_redesign
cargo test --test login_target
cargo test --test phase9_multi_account
```

Expected: all tests pass.

- [ ] **Step 2: Run frontend verification**

```powershell
npm test
npm run build
```

Expected: tests and build pass.

- [ ] **Step 3: Check the patch**

```powershell
git diff --check
git status --short
```

Expected: only the existing login repair files, this command wiring, its regression test, and this plan are modified; the pre-existing `Cargo.toml` change remains untouched.

- [ ] **Step 4: Restart development runtime**

Start with `npm run tauri:dev`, verify the desktop process exists, and verify `http://localhost:1420` returns HTTP 200.
