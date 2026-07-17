# Remove Runtime Redaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove every runtime redaction or sanitization transform so naturally occurring upstream fields and errors are preserved verbatim, while keeping build/resource guards and avoiding active credential propagation.

**Architecture:** Replace the current sanitize-before-store and redact-before-return pipeline with direct ownership transfer of the original `serde_json::Value` and original error strings. Keep existing DTO schemas, account registries, background event schemas, atomic snapshot replacement, revision recovery, and release-resource validation unchanged. Stop invoking the legacy privacy migration, then delete the now-unused runtime redaction module.

**Tech Stack:** Rust, Tauri 2, serde/serde_json, rusqlite/SQLite, TypeScript test scripts, PowerShell, Cargo, npm.

---

## File responsibility map

- `src/importer.rs`: persist upstream entry/detail/player/profile/error JSON without transforming it.
- `src/sync/payload.rs`, `src/season_fetch.rs`: preserve fetched payloads and detail errors before import.
- `src/sync/quick_list.rs`, `src/sync/persist.rs`: write quick-list rows and season files directly.
- `src/sync/snapshot.rs`: assemble and serialize complete SQLite history without recursive passes.
- `src/dashen_client.rs`, `src/sync/client.rs`: propagate original HTTP/request errors.
- `src/sync/background_worker.rs`, `src/sync/facade.rs`, `src/commands.rs`: stop masking errors at orchestration boundaries.
- `src/browser_session.rs`, `src/credential_store.rs`, `src/credentials.rs`, `src/login_flow.rs`, `src/login_target.rs`: stop masking naturally occurring login errors without adding new credential fields.
- `src/settings.rs`, `src/queries/normal_detail.rs`, `src/queries/mod.rs`: return stored raw JSON unchanged.
- `src/db.rs`, `src/privacy_migration.rs`: stop modifying existing persistence during initialization.
- `src/redaction.rs`, `src/lib.rs`, `Cargo.toml`: remove the obsolete runtime redaction module and its exclusive regex dependency.
- `scripts/prepareAssets.mjs`, `scripts/runtimeConfig.test.ts`, `scripts/prepareAssets.test.ts`: remain unchanged and continue guarding release resources.
- `AGENTS.md`, `README.md`, `docs/maintenance.md`: encode the new raw-runtime-data contract and the retained release boundary.
- `tests/raw_runtime_data.rs`: central end-to-end contract for raw persistence, snapshot, query, and error preservation.
- `src/bin/benchmark_season_snapshot.rs`: anonymous stage-timing utility for a copied local database.

## Execution precondition

The main checkout contains unrelated user work. Before implementation, create an isolated worktree from commit `8b0cd4f` with `superpowers:using-git-worktrees`. Do not copy, stash, stage, or modify the main checkout's unrelated files. Merge only after all tests pass, then restore the combined dirty checkout exactly as found.

### Task 1: Add the raw-runtime-data contract

**Files:**
- Create: `apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/snapshot.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Create: `apps/owstats-desktop/src-tauri/src/bin/benchmark_season_snapshot.rs`

- [ ] **Step 1: Write a failing importer and season-file preservation test**

Create `tests/raw_runtime_data.rs` with synthetic values only:

```rust
use anyhow::Result;
use owstats_desktop_lib::{
    import_payload_value_embedded, rebuild_complete_season_payload, write_season_payload,
    OwDatabase,
};
use serde_json::json;

const ENTRY_SECRET: &str = "synthetic-entry-token";
const DETAIL_SECRET: &str = "synthetic-detail-cookie";
const ERROR_SECRET: &str = "synthetic-error-authorization";

fn raw_payload() -> serde_json::Value {
    json!({
        "season": 22,
        "profile_card": {"data": {
            "bnetId": "synthetic-player",
            "name": "匿名账号",
            "customerToken": "synthetic-profile-token"
        }},
        "matches": {
            "entries": [{
                "matchId": "raw-contract-match",
                "gameMode": "sport",
                "beginTs": 1720000000,
                "token": ENTRY_SECRET
            }],
            "details": [{
                "match_id": "raw-contract-match",
                "match_kind": "normal",
                "payload": {"code": 0, "data": {
                    "cookie": DETAIL_SECRET,
                    "teammateList": [],
                    "enemyList": []
                }}
            }],
            "detail_errors": [{
                "match_id": "raw-error-match",
                "error_type": "DetailFetchError",
                "message": format!("Authorization: Bearer {ERROR_SECRET}"),
                "authorization": ERROR_SECRET
            }]
        }
    })
}

#[test]
fn importer_snapshot_and_season_file_preserve_natural_fields() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("raw.sqlite3"));
    import_payload_value_embedded(&db, &raw_payload())?;

    let conn = db.connect()?;
    let entry: String = conn.query_row(
        "SELECT raw_entry_json FROM matches WHERE match_id='raw-contract-match'",
        [],
        |row| row.get(0),
    )?;
    let detail: String = conn.query_row(
        "SELECT raw_detail_json FROM match_details WHERE match_id='raw-contract-match'",
        [],
        |row| row.get(0),
    )?;
    let error: String = conn.query_row(
        "SELECT raw_json FROM sync_errors WHERE season=22",
        [],
        |row| row.get(0),
    )?;
    assert!(entry.contains(ENTRY_SECRET));
    assert!(detail.contains(DETAIL_SECRET));
    assert!(error.contains(ERROR_SECRET));
    drop(conn);

    let rebuilt = rebuild_complete_season_payload(
        &db,
        22,
        &json!({"season": 22, "matches": {}}),
    )?;
    let output = temp.path().join("seasons");
    let path = write_season_payload(&output, 22, &rebuilt)?;
    let written = std::fs::read_to_string(path)?;
    for secret in [ENTRY_SECRET, DETAIL_SECRET, ERROR_SECRET] {
        assert!(written.contains(secret));
    }
    Ok(())
}
```

- [ ] **Step 2: Run the contract and verify current behavior fails**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
$env:CARGO_TARGET_DIR='target-codex-raw-runtime'
cargo test --test raw_runtime_data importer_snapshot_and_season_file_preserve_natural_fields -- --exact
```

Expected: FAIL because at least `synthetic-entry-token`, `synthetic-detail-cookie`, and `synthetic-error-authorization` are absent.

- [ ] **Step 3: Reverse the existing snapshot tests from redaction to preservation**

In `tests/incremental_sync.rs`, rename `rebuilt_snapshot_contains_complete_sanitized_sqlite_history` to `rebuilt_snapshot_contains_complete_unmodified_sqlite_history`. Keep its existing fixture, database mutations, completeness assertions, and local variables; replace only the final sensitive-value absence loop with:

```rust
for expected in [
    "db-secret",
    "db-cookie",
    "db-authorization",
    "synthetic-bearer-secret",
    "current-secret",
    "current-credential",
] {
    assert!(serialized.contains(expected), "missing raw value {expected}");
}
```

Retain the existing completeness, season-scope, malformed-JSON, atomic replace, and revision-race assertions.

- [ ] **Step 4: Add the anonymous benchmark before changing runtime behavior**

Add to `sync/snapshot.rs` without changing `rebuild_complete_season_payload` yet. Extend its imports with `std::io::Write` and `std::path::Path` if they are not already present:

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotBenchmark {
    pub season: i64,
    pub entries: usize,
    pub details: usize,
    pub errors: usize,
    pub assemble_ms: u64,
    pub serialize_ms: u64,
    pub write_and_sync_ms: u64,
    pub total_ms: u64,
    pub output_bytes: u64,
}

pub fn benchmark_snapshot(
    db: &OwDatabase,
    season: i64,
    output: &Path,
) -> Result<SnapshotBenchmark> {
    db.initialize()?;
    let total_started = std::time::Instant::now();
    let assemble_started = std::time::Instant::now();
    let payload = rebuild_complete_season_payload(
        db,
        season,
        &json!({"season": season, "matches": {}}),
    )?;
    let assemble_ms = assemble_started.elapsed().as_millis() as u64;
    let entries = payload.pointer("/matches/entries").and_then(Value::as_array).map_or(0, Vec::len);
    let details = payload.pointer("/matches/details").and_then(Value::as_array).map_or(0, Vec::len);
    let errors = payload.pointer("/matches/detail_errors").and_then(Value::as_array).map_or(0, Vec::len);

    let serialize_started = std::time::Instant::now();
    let bytes = serde_json::to_vec_pretty(&payload)?;
    let serialize_ms = serialize_started.elapsed().as_millis() as u64;

    let write_started = std::time::Instant::now();
    let parent = output.parent().ok_or_else(|| anyhow!("benchmark output has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.as_file_mut().write_all(&bytes)?;
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(output).map_err(|error| error.error)?;
    let write_and_sync_ms = write_started.elapsed().as_millis() as u64;

    Ok(SnapshotBenchmark {
        season,
        entries,
        details,
        errors,
        assemble_ms,
        serialize_ms,
        write_and_sync_ms,
        total_ms: total_started.elapsed().as_millis() as u64,
        output_bytes: bytes.len() as u64,
    })
}
```

Export `benchmark_snapshot` and `SnapshotBenchmark` through `sync/mod.rs` and `lib.rs` with `#[doc(hidden)]`. Create `src/bin/benchmark_season_snapshot.rs`:

```rust
use anyhow::{anyhow, Result};
use owstats_desktop_lib::{benchmark_snapshot, OwDatabase};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let database = PathBuf::from(args.next().ok_or_else(|| anyhow!("missing database path"))?);
    let season = args
        .next()
        .ok_or_else(|| anyhow!("missing season"))?
        .parse::<i64>()?;
    let output = PathBuf::from(args.next().ok_or_else(|| anyhow!("missing output path"))?);
    if args.next().is_some() {
        return Err(anyhow!("unexpected extra argument"));
    }
    let result = benchmark_snapshot(&OwDatabase::new(database), season, &output)?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
```

The CLI must print only counts, timings, and output size.

- [ ] **Step 5: Capture the pre-change S22 baseline on a copied database**

Close the desktop app first. Run without echoing the account identifier, paths, registry, database rows, or output payload:

```powershell
$appData = Join-Path $env:APPDATA 'local.owstats.desktop'
$registry = Get-Content -Raw (Join-Path $appData 'accounts.json') | ConvertFrom-Json
$source = Join-Path $appData "accounts\$($registry.activeAccountId)\owstats.sqlite3"
$copy = Join-Path $env:TEMP 'owstats-s22-before-raw-runtime.sqlite3'
$output = Join-Path $env:TEMP 'owstats-s22-before-raw-runtime.json'
Copy-Item -LiteralPath $source -Destination $copy -Force
cd apps\owstats-desktop\src-tauri
cargo run --bin benchmark_season_snapshot -- $copy 22 $output
```

Record only the single anonymous JSON result. If no usable local S22 database exists, record the baseline as not run; do not inspect credentials or fabricate data.

- [ ] **Step 6: Commit the failing contract and neutral benchmark slice**

```powershell
git add apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs apps/owstats-desktop/src-tauri/src/sync/snapshot.rs apps/owstats-desktop/src-tauri/src/sync/mod.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/src/bin/benchmark_season_snapshot.rs
git commit -m "test: define raw runtime data contract and baseline"
```

### Task 2: Preserve raw JSON through fetch, import, and SQLite

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/importer.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/payload.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/quick_list.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/persist.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Remove the top-level importer sanitization copy**

In `importer.rs`, remove the redaction import and replace the beginning of `import_payload_value_with_lookups` with direct borrowing:

```rust
fn import_payload_value_with_lookups(
    db: &OwDatabase,
    payload: &Value,
    lookups: Lookups,
    behavior: ImportBehavior,
) -> Result<ImportSummary> {
    db.initialize()?;
    let conn = db.connect()?;
    let season = payload.get("season").and_then(Value::as_i64).unwrap_or(0);
```

Do not create `sanitized_payload`.

- [ ] **Step 2: Persist profile, error, and player JSON directly**

Apply these exact ownership changes in `importer.rs`:

```rust
// sync_errors.message
string_field(error, "error").or_else(|| string_field(error, "message")),
// sync_errors.raw_json
serde_json::to_string(error)?,

// upsert_profile_card raw_json
serde_json::to_string(profile)?,

// insert_player_rows raw_json
serde_json::to_string(item)?,
```

Delete `strip_sensitive_json` and `is_sensitive_key` from `importer.rs` after the last caller is removed.

- [ ] **Step 3: Preserve quick-list rows without a recursive clone**

In `sync/quick_list.rs`, replace the sanitized local variable in `upsert_list_entry` with direct reads and serialization:

```rust
let match_id = crate::importer::string_field(entry, "matchId").unwrap_or_default();
let mode = crate::importer::normalized_game_mode(entry);
let map_guid = crate::importer::string_field(entry, "mapGuid").unwrap_or_default();
let hero_guid = crate::importer::string_field(entry, "heroGuid").unwrap_or_default();
// Keep all existing derived-field extraction, but pass `entry` instead of `sanitized`.
serde_json::to_string(entry)?,
```

- [ ] **Step 4: Make full-fetch payload construction ownership-only**

In `sync/payload.rs`, replace `build_full_season_payload` with:

```rust
pub(super) fn build_full_season_payload(
    season: i64,
    cached_player: &Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    list_pages: Vec<Value>,
) -> Value {
    json!({
        "input_bnet": cached_player.get("input_bnet").cloned().unwrap_or(Value::Null),
        "resolved_bnet": cached_player.get("resolved_bnet").cloned().unwrap_or(Value::Null),
        "profile_card": cached_player.get("profile_card").cloned().unwrap_or(Value::Null),
        "fetch_mode": "rust_sync",
        "season": season,
        "matches": {
            "total": entries.len(),
            "entries": entries,
            "details": details,
            "detail_errors": normalize_detail_errors(detail_errors),
            "list_pages": list_pages,
        }
    })
}
```

Replace `sanitize_detail_errors` with an object-preserving normalizer:

```rust
pub(super) fn normalize_detail_errors(detail_errors: Vec<Value>) -> Vec<Value> {
    detail_errors
        .into_iter()
        .map(|mut error| {
            if let Some(object) = error.as_object_mut() {
                object
                    .entry("error_type")
                    .or_insert_with(|| Value::String("DetailFetchError".to_string()));
                if !object.contains_key("message") {
                    if let Some(raw) = object.get("error").cloned() {
                        object.insert("message".to_string(), raw);
                    }
                }
            }
            error
        })
        .collect()
}
```

Delete `strip_sensitive_fields` and the old error truncation helper. Update internal callers in `sync/persist.rs`, `sync/fetch.rs`, and `season_fetch.rs` to use `normalize_detail_errors` or the original payload.

- [ ] **Step 5: Write season payloads directly**

In `sync/persist.rs`, serialize `payload` directly:

```rust
serde_json::to_writer_pretty(temporary.as_file_mut(), payload)
    .with_context(|| format!("failed to serialize season payload for S{season}"))?;
```

Remove the `sanitized` local variable. Keep directory creation, flush, `sync_all`, and atomic persist unchanged.

- [ ] **Step 6: Run focused persistence tests**

```powershell
cargo test --test raw_runtime_data
cargo test --test import_baseline
cargo test --test phase8_real_fetch full_season_fetch_never_persists_local_or_upstream_tokens -- --exact
```

Expected: the new raw contract and importer tests PASS; the old `full_season_fetch_never_persists_local_or_upstream_tokens` test FAILS and is renamed/reversed in the next step.

- [ ] **Step 7: Reverse legacy persistence assertions**

Rename the Phase 8 test to `full_season_fetch_preserves_naturally_occurring_upstream_fields` and assert the synthetic upstream values are present in the returned payload and written season JSON. In `import_baseline.rs`, replace the public redaction helper test with direct raw-storage assertions; do not keep a test-only redaction API.

- [ ] **Step 8: Re-run and commit**

```powershell
cargo test --test raw_runtime_data
cargo test --test import_baseline
cargo test --test phase8_real_fetch
git add apps/owstats-desktop/src-tauri/src/importer.rs apps/owstats-desktop/src-tauri/src/season_fetch.rs apps/owstats-desktop/src-tauri/src/sync/payload.rs apps/owstats-desktop/src-tauri/src/sync/quick_list.rs apps/owstats-desktop/src-tauri/src/sync/persist.rs apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs apps/owstats-desktop/src-tauri/tests/import_baseline.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs
git commit -m "feat: preserve raw sync payloads"
```

### Task 3: Remove recursive processing from snapshots

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync/snapshot.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs`

- [ ] **Step 1: Make snapshot assembly return the rebuilt object directly**

In `sync/snapshot.rs`, end `rebuild_complete_season_payload` with:

```rust
matches.insert("detail_errors".to_string(), Value::Array(detail_errors));
Ok(rebuilt)
```

Delete `sanitize_snapshot_value` and `redact_snapshot_strings`.

- [ ] **Step 2: Serialize the rebuilt payload directly**

In `rebuild_snapshot_for_revision_with_hook`, replace the second transformation with:

```rust
let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
serde_json::to_writer_pretty(temporary.as_file_mut(), &payload)?;
temporary.as_file_mut().flush()?;
temporary.as_file().sync_all()?;
```

Keep both revision checks and the `BEGIN IMMEDIATE` transaction exactly as they are.

- [ ] **Step 3: Run snapshot, benchmark, and preservation tests**

```powershell
cargo test --test incremental_sync snapshot_
cargo test --test raw_runtime_data importer_snapshot_and_season_file_preserve_natural_fields -- --exact
cargo run --bin benchmark_season_snapshot
```

The CLI intentionally has no help mode, so the last command must exit with `missing database path` and must not print local data.

Expected: the tests PASS; atomic failure keeps the old snapshot, stale revision does not replace it, and raw synthetic fields remain present.

- [ ] **Step 4: Commit the snapshot fast path**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/snapshot.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs
git commit -m "perf: remove recursive snapshot redaction"
```

### Task 4: Propagate original request and synchronization errors

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_client.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/client.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/background_worker.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/facade.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/background_season_sync.rs`

- [ ] **Step 1: Add a failing original-error contract**

Extend `raw_runtime_data.rs`:

```rust
#[test]
fn command_error_conversion_preserves_original_text() {
    let source = "request failed token=synthetic-error-token Authorization: Bearer synthetic-header";
    let message = owstats_desktop_lib::runtime_error_message(anyhow::anyhow!(source));
    assert_eq!(message, source);
}
```

Export a testable `runtime_error_message` from `lib.rs` only after Step 4 below.

- [ ] **Step 2: Return reqwest's original errors**

In both `dashen_client.rs` and `sync/client.rs`, replace masking helpers with the original error text:

```rust
fn request_error(context: &str, error: &reqwest::Error) -> anyhow::Error {
    anyhow!("{context}: {error}")
}
```

At send failures, return `request_error(context, &error)`. For non-success HTTP responses, use the response's own status error before consuming the body:

```rust
if !status.is_success() {
    return Err(anyhow!("{context}: {}", response.error_for_status_ref().unwrap_err()));
}
```

Do not append credentials from request objects. If reqwest's error naturally contains the requested URL, preserve it.

- [ ] **Step 3: Preserve actual background detail errors**

In `sync/background_worker.rs`, retain the error from the detail request:

```rust
Err(error) => Err(json!({
    "match_id": match_id,
    "error_type": "DetailFetchError",
    "message": error.to_string(),
})),
```

At `run_background_job_once`, return `Err(error)` after marking the job retryable rather than wrapping it in `safe_error_message`.

- [ ] **Step 4: Remove orchestration-boundary masking**

In `commands.rs`, replace `to_message` with direct conversion and expose the pure helper:

```rust
pub fn runtime_error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn to_message(error: impl std::fmt::Display) -> String {
    runtime_error_message(error)
}
```

Re-export `runtime_error_message` from `lib.rs` for integration testing. In `sync/facade.rs`, build the committed-snapshot boundary message with `error` directly:

```rust
anyhow!("数据库更新成功，但本地快照更新失败：{error}")
```

- [ ] **Step 5: Reverse error-redaction tests**

Update the corresponding tests so synthetic error fragments are expected to remain. Keep status code, retry count, 429 backoff, retryable job, and transaction-boundary assertions unchanged.

- [ ] **Step 6: Run and commit**

```powershell
cargo test --test raw_runtime_data command_error_conversion_preserves_original_text -- --exact
cargo test --test phase8_real_fetch
cargo test --test background_season_sync
git add apps/owstats-desktop/src-tauri/src/dashen_client.rs apps/owstats-desktop/src-tauri/src/sync/client.rs apps/owstats-desktop/src-tauri/src/sync/background_worker.rs apps/owstats-desktop/src-tauri/src/sync/facade.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs apps/owstats-desktop/src-tauri/tests/background_season_sync.rs
git commit -m "feat: preserve original runtime errors"
```

### Task 5: Remove login, query, and display masking without adding fields

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/browser_session.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/credential_store.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/credentials.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/login_flow.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/login_target.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/settings.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs`

- [ ] **Step 1: Make login errors direct strings**

Replace calls to `credential_store::redact_sensitive_text` with `.to_string()` or the original `&str`:

```rust
// browser_session.rs
error.to_string()

// login_flow.rs
message.as_ref().to_string()

// credentials.rs and login_target.rs
format!("自动绑定失败：{message}")
```

Delete the `credential_store::redact_sensitive_text` wrapper after its last caller is removed. Keep credential file serialization, account registry schemas, and login progress DTO fields unchanged.

- [ ] **Step 2: Store settings/profile JSON directly**

In `settings.rs`, remove the redaction import and write:

```rust
serde_json::to_string(raw)?,
```

Do not add credential fields to `PlayerProfile`.

- [ ] **Step 3: Return normal-match raw JSON directly**

In `queries/normal_detail.rs`, replace the raw JSON branch with:

```rust
let raw_json = if match_kind == "fight" {
    Value::Null
} else {
    json!({ "entry": entry, "detail": detail })
};
```

Remove the redaction import from `queries/mod.rs`. Keep fight-detail publication behavior unchanged because it is a product schema decision, not a redaction transform.

- [ ] **Step 4: Reverse login and raw-query tests**

In `phase4_login_redesign.rs`, replace the redaction helper assertion with a login error assertion that expects synthetic fragments unchanged. In `import_baseline.rs`, seed `token`, `cookie`, and `authorization` inside normal entry/detail raw JSON, call `get_match_detail`, and assert the returned `raw_json` contains all three values.

- [ ] **Step 5: Verify no active credential propagation**

Run the existing account and event tests unchanged:

```powershell
cargo test --test phase9_multi_account account_public_json_never_contains_private_identity_or_secrets -- --exact
cargo test --lib event_payloads_are_anonymous_and_bounded
cargo test --test phase4_login_redesign
cargo test --test import_baseline
```

Expected: raw JSON and original errors preserve natural fields, while account registry and background event schemas gain no fields.

- [ ] **Step 6: Commit**

```powershell
git add apps/owstats-desktop/src-tauri/src/browser_session.rs apps/owstats-desktop/src-tauri/src/credential_store.rs apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/src/login_flow.rs apps/owstats-desktop/src-tauri/src/login_target.rs apps/owstats-desktop/src-tauri/src/settings.rs apps/owstats-desktop/src-tauri/src/queries/mod.rs apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs apps/owstats-desktop/src-tauri/tests/import_baseline.rs apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs
git commit -m "feat: return unmodified runtime data"
```

### Task 6: Stop the historical privacy migration

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/db.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Delete: `apps/owstats-desktop/src-tauri/src/privacy_migration.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs`

- [ ] **Step 1: Add a failing initialization-preservation test**

Add to `raw_runtime_data.rs`:

```rust
#[test]
fn database_initialize_does_not_rewrite_existing_raw_json_or_season_files() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let shared = owstats_desktop_lib::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = owstats_desktop_lib::account_store::AccountId::parse(
        "11111111-1111-4111-8111-111111111111",
    )?;
    let paths = shared.account(&account_id);
    std::fs::create_dir_all(paths.seasons())?;
    let db = OwDatabase::for_account(&paths);
    db.initialize()?;
    db.connect()?.execute(
        "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES ('legacy-raw',22,'preset','sport','normal','win','','',1,0,0,0,0,0,0,0,0,0,0,?1,'now')",
        [r#"{"token":"legacy-natural-token"}"#],
    )?;
    let season_path = paths.seasons().join("local_s22_full_season.json");
    std::fs::write(&season_path, r#"{"cookie":"legacy-natural-cookie"}"#)?;

    db.initialize()?;

    let raw: String = db.connect()?.query_row(
        "SELECT raw_entry_json FROM matches WHERE match_id='legacy-raw'",
        [],
        |row| row.get(0),
    )?;
    assert!(raw.contains("legacy-natural-token"));
    assert!(std::fs::read_to_string(season_path)?.contains("legacy-natural-cookie"));
    Ok(())
}
```

- [ ] **Step 2: Remove migration invocation**

In `db.rs`, delete the entire `match (self.seasons.as_ref(), self.backups.as_ref())` privacy migration block after `initialize_connection(&conn)?;`. Return `Ok(())` immediately after schema initialization.

- [ ] **Step 3: Remove the obsolete module**

Delete `src/privacy_migration.rs` and remove `mod privacy_migration;` from `lib.rs`. Do not delete any app-data backups or marker rows; code removal alone stops future rewrites.

- [ ] **Step 4: Replace the old Phase 9 source contract**

Replace `account_database_privacy_migration_uses_explicit_seasons_and_backups_paths` with a source/behavior contract asserting account initialization does not call a privacy migration and does not create a new `privacy-*` backup. Keep schema backup tests unchanged.

- [ ] **Step 5: Run and commit**

```powershell
cargo test --test raw_runtime_data database_initialize_does_not_rewrite_existing_raw_json_or_season_files -- --exact
cargo test --test phase9_multi_account
cargo test --test stab005_database_runtime
git add apps/owstats-desktop/src-tauri/src/db.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/src/privacy_migration.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs apps/owstats-desktop/src-tauri/tests/raw_runtime_data.rs
git commit -m "feat: stop legacy privacy rewriting"
```

### Task 7: Delete the runtime redaction module and dependency

**Files:**
- Delete: `apps/owstats-desktop/src-tauri/src/redaction.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/Cargo.toml`
- Modify: `apps/owstats-desktop/src-tauri/Cargo.lock`
- Test: all Rust source and tests under `apps/owstats-desktop/src-tauri`

- [ ] **Step 1: Prove all runtime callers are gone**

Run:

```powershell
rg -n "crate::redaction|redact_sensitive|redact_json|sanitize_persisted_json|sanitize_snapshot_value|strip_sensitive_json|strip_sensitive_fields" apps/owstats-desktop/src-tauri/src apps/owstats-desktop/src-tauri/tests
```

Expected: only `src/redaction.rs` and obsolete redaction-specific tests remain. If any production caller remains, remove that transform and add a raw-preservation assertion before continuing.

- [ ] **Step 2: Delete the module and public export**

Delete `src/redaction.rs`. Remove these lines from `lib.rs`:

```rust
mod redaction;
pub use redaction::redact_sensitive_json;
```

- [ ] **Step 3: Remove the exclusive regex dependency**

Delete `regex = "1"` from `Cargo.toml`, then update the lockfile with:

```powershell
cargo check
```

The repository search confirmed `regex::` is used only by `redaction.rs`; if current source has gained another legitimate caller, retain the dependency but still delete the redaction module.

- [ ] **Step 4: Verify no replacement no-op API exists**

```powershell
rg -n "redact|sanitiz|REDACTED" apps/owstats-desktop/src-tauri/src apps/owstats-desktop/src-tauri/tests
```

Expected: no runtime redaction transform. Names that describe unrelated SQL input validation or release-resource checks are allowed only after direct inspection.

- [ ] **Step 5: Run all Rust tests and commit**

```powershell
cargo fmt --check
cargo test --tests
git add apps/owstats-desktop/src-tauri/Cargo.toml apps/owstats-desktop/src-tauri/Cargo.lock apps/owstats-desktop/src-tauri/src apps/owstats-desktop/src-tauri/tests
git commit -m "refactor: remove runtime redaction module"
```

Expected: all tests PASS except the existing real-credential live test remains ignored by design.

### Task 8: Update durable rules while retaining release guards

**Files:**
- Modify: `AGENTS.md`
- Modify: `README.md`
- Modify: `docs/maintenance.md`
- Test: `apps/owstats-desktop/scripts/runtimeConfig.test.ts`
- Test: `apps/owstats-desktop/scripts/prepareAssets.test.ts`
- Test: `apps/owstats-desktop/scripts/accountManagement.test.tsx`

- [ ] **Step 1: Replace the runtime privacy rule in AGENTS.md**

Replace the old recursive-redaction requirement with:

```markdown
## 原始运行时数据与发布边界

- 同步、导入、SQLite、赛季快照、错误消息和查询 raw JSON 原样保留上游自然出现的字段，不执行运行时脱敏、遮盖或递归清理。
- 不得主动把凭据文件中的 token、customerToken、cookie、authorization 或 credential 复制到原本不包含它们的账号索引、DTO、事件、日志或业务文件。
- 构建资源、运行时配置、账号索引、提交内容和安装包仍不得包含主动加入的真实凭据、浏览器 profile、真实用户数据 dump 或本地 SQLite。
- 测试只使用合成凭据值；诊断和性能计时只输出阶段、匿名计数、耗时与文件大小。
```

Remove contradictory statements requiring runtime recursive redaction or privacy migration.

- [ ] **Step 2: Update README and maintenance guidance**

Add to `README.md`:

```markdown
- 运行时业务 raw JSON 按上游内容原样保留；应用不会主动把登录凭据扩散到新的输出面。
```

In `docs/maintenance.md`, remove the privacy migration cleaning description and document:

```markdown
- 数据库初始化不再重写历史 raw JSON 或赛季文件，既有 privacy backup 与 marker 仅保留兼容，不主动删除。
- 快照性能诊断只记录 season、entries、details、errors、阶段耗时和输出字节数。
```

- [ ] **Step 3: Prove build and shared-config guards still reject credentials**

Run:

```powershell
cd apps\owstats-desktop
tsx scripts/runtimeConfig.test.ts
tsx scripts/prepareAssets.test.ts
tsx scripts/accountManagement.test.tsx
```

Expected: PASS without weakening `prepareAssets.mjs`, runtime config checks, account registry DTOs, or package-resource validation.

- [ ] **Step 4: Commit documentation and guard tests**

```powershell
git add AGENTS.md README.md docs/maintenance.md apps/owstats-desktop/scripts/runtimeConfig.test.ts apps/owstats-desktop/scripts/prepareAssets.test.ts apps/owstats-desktop/scripts/accountManagement.test.tsx
git commit -m "docs: define raw runtime data boundary"
```

### Task 9: Benchmark S22 and complete release verification

**Files:**
- Modify only if a verified defect is found: files from Tasks 1–8
- Verify: `apps/owstats-desktop/src-tauri/src/bin/benchmark_season_snapshot.rs`
- Verify: full repository diff and build products

- [ ] **Step 1: Prepare a SQLite backup without printing account identifiers**

Close the desktop app before the benchmark so WAL contents are stable. Then run:

```powershell
$appData = Join-Path $env:APPDATA 'local.owstats.desktop'
$registry = Get-Content -Raw (Join-Path $appData 'accounts.json') | ConvertFrom-Json
$active = $registry.activeAccountId
$source = Join-Path $appData "accounts\$active\owstats.sqlite3"
$copy = Join-Path $env:TEMP 'owstats-s22-raw-runtime-benchmark.sqlite3'
Copy-Item -LiteralPath $source -Destination $copy -Force
```

Do not echo `$active`, `$source`, registry content, or database rows.

- [ ] **Step 2: Run the anonymous post-change benchmark**

```powershell
cd apps\owstats-desktop\src-tauri
$output = Join-Path $env:TEMP 'owstats-s22-raw-runtime-benchmark.json'
cargo run --bin benchmark_season_snapshot -- $copy 22 $output
```

Expected: one JSON line containing only `season`, `entries`, `details`, `errors`, `assembleMs`, `serializeMs`, `writeAndSyncMs`, `totalMs`, and `outputBytes`. Compare it with the baseline captured in Task 1 Step 5. If no usable local S22 exists, report the benchmark as not run rather than reading credentials or fabricating data.

- [ ] **Step 3: Run complete backend verification**

```powershell
cargo fmt --check
cargo test --tests
```

Expected: PASS; only the existing live test that requires real local credentials is ignored.

- [ ] **Step 4: Run complete frontend and release verification**

```powershell
cd ..
npm test
npm run build
npm run tauri build
```

Expected: all commands exit 0; MSI and NSIS bundles are produced; release-resource credential guards remain enabled.

- [ ] **Step 5: Audit scope and passive-preservation boundary**

```powershell
git diff --check
rg -n "redact|sanitiz|REDACTED" apps/owstats-desktop/src-tauri/src apps/owstats-desktop/src-tauri/tests
rg -n "customerToken|authorization|cookie|credential" apps/owstats-desktop/scripts/prepareAssets.mjs apps/owstats-desktop/scripts/runtimeConfig.test.ts apps/owstats-desktop/scripts/prepareAssets.test.ts
git status --short
```

Expected:

- no runtime redaction transform remains;
- release guard searches still find their explicit sensitive-key deny lists;
- no real credential, player dump, SQLite, benchmark output, `D:\\Codex` runtime dependency, or `/@fs/` path is tracked;
- status contains only planned files in the isolated worktree.

- [ ] **Step 6: Commit any verification-only corrections**

If verification required code changes, stage only those exact files and commit:

```powershell
git commit -m "test: verify raw runtime data workflow"
```

If no correction was required, do not create an empty commit.

## Completion criteria

The work is complete only when naturally occurring runtime fields and original errors pass unchanged through fetch, SQLite, snapshot, query raw JSON, and existing error surfaces; no runtime recursive redaction or privacy rewrite remains; no new credential field is added to account registries, background events, or ordinary DTOs; build and release-resource guards still reject credentials; historical data is not re-fetched; S22 timing is reported anonymously when available; and all Rust, frontend, production, and Tauri release verification passes.
