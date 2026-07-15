# Multi-Account Data Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add multiple locally saved login accounts whose credentials, player target, profile, SQLite database, season files, diagnostics, and backups are physically isolated and switchable from the command center.

**Architecture:** Store a versioned account registry at app-data root and one randomly named directory per account. A managed `AccountContextManager` resolves the active account into an immutable `AccountContextSnapshot`; every player-data command consumes that snapshot, while shared settings and the browser profile stay at app-data root. Login writes to a pending area before identity validation, switch atomically changes the registry and reloads the WebView, and deletion/migration use journals for crash recovery.

**Tech Stack:** Tauri 2, Rust 2021, React 18, TypeScript, SQLite/rusqlite, serde/serde_json, uuid, existing source-contract tests with `tsx`.

---

## File map

### New Rust modules

- `apps/owstats-desktop/src-tauri/src/account_store.rs` — account identifiers, registry/manifest models, atomic persistence, safe summaries.
- `apps/owstats-desktop/src-tauri/src/account_context.rs` — active-account manager and immutable path snapshots.
- `apps/owstats-desktop/src-tauri/src/account_migration.rs` — one-time legacy adoption and migration journal recovery.
- `apps/owstats-desktop/src-tauri/src/account_login.rs` — add/relogin staging, role matching, and atomic credential publication.
- `apps/owstats-desktop/src-tauri/src/account_removal.rs` — deletion journal, quarantine, rollback/recovery, and fallback-account selection.
- `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs` — backend account registry, routing, login, migration, and deletion integration coverage.

### Existing Rust modules

- `apps/owstats-desktop/src-tauri/Cargo.toml` — add `uuid` with v4/serde features.
- `apps/owstats-desktop/src-tauri/src/app_paths.rs` — split shared paths from account-owned paths.
- `apps/owstats-desktop/src-tauri/src/credentials.rs` — make browser capture return an unsaved result; accept explicit credential destinations.
- `apps/owstats-desktop/src-tauri/src/player_target.rs` — remove fixed app-root path resolution from product commands.
- `apps/owstats-desktop/src-tauri/src/commands.rs` — add account commands and route all player-data commands through active context.
- `apps/owstats-desktop/src-tauri/src/models.rs` — safe account DTOs and operation results.
- `apps/owstats-desktop/src-tauri/src/lib.rs` — register modules, managed account state, and Tauri handlers.
- `apps/owstats-desktop/src-tauri/src/sync_coordinator.rs` — cover every account-mutating/database-writing operation with the same permit.
- `apps/owstats-desktop/src-tauri/src/db.rs` — keep database initialization account-local and stop treating account directories as shared app-data roots.

### Frontend

- `apps/owstats-desktop/src/features/command-center/AccountManager.tsx` — isolated account list, add row, three per-account actions, and delete dialog.
- `apps/owstats-desktop/src/pages/OnboardingPage.tsx` — load account state, orchestrate operations, replace logout danger zone, and reload after switch/removing active account.
- `apps/owstats-desktop/src/shared/types.ts` — safe account DTOs and operation result unions.
- `apps/owstats-desktop/src/shared/api.ts` — account command wrappers; remove product use of `logout`.
- `apps/owstats-desktop/src/styles/features/command-center.css` — account rows, responsive actions, danger treatment, and dialog states.
- `apps/owstats-desktop/scripts/accountManagement.test.tsx` — server-rendered component and source-contract tests.
- `apps/owstats-desktop/package.json` — add the account-management test to `test:components`.

### Shared backend test fixture contract

`phase9_multi_account.rs` uses one synthetic fixture family throughout the plan. Define these structs when first needed and extend their `impl` blocks in the task that adds the corresponding production API:

```rust
struct MultiAccountFixture {
    temp: tempfile::TempDir,
    shared: SharedAppPaths,
    manager: AccountContextManager,
    browser_launch_count: std::cell::Cell<usize>,
}

struct LegacyFixture { inner: MultiAccountFixture }
struct AccountLoginFixture { inner: MultiAccountFixture }

#[derive(Clone)]
struct LoginCapture {
    role_id: i64,
    battle_tag: String,
    credentials: DashenCredentials,
}

impl LoginCapture {
    fn synthetic(role_id: i64, battle_tag: &str) -> Self {
        Self {
            role_id,
            battle_tag: battle_tag.to_string(),
            credentials: DashenCredentials {
                role_id,
                token: "synthetic-token".to_string(),
                customer_token: Some("synthetic-customer".to_string()),
                dts: Some("synthetic-dts".to_string()),
                server: 1,
                saved_at: "2026-07-16T00:00:00Z".to_string(),
            },
        }
    }
}
```

Every fixture method named below must call the public production function introduced by that task and may create only temporary synthetic files. Add methods in these groups:

```text
Task 3: new, create_account, seed_match, activate, list_match_ids
Task 4: LegacyFixture::with_database_match, manager, confirm_account,
        adopt_legacy, match_ids, replace_legacy_database, shared
Task 5: AccountLoginFixture::new, with_account, publish_add,
        publish_relogin, only_account_id, accounts, saved_token
Task 6: with_two_accounts, with_three_accounts, first_account_id,
        second_account_id, active_account_id, most_recent_inactive_account_id,
        switch_account, remove_account, database_bytes, browser_launch_count,
        active_snapshot, activate_second
```

Crash-point recovery tests live in `#[cfg(test)]` unit tests inside the relevant migration/removal module, using a dependency-injected local fake filesystem. Integration tests cover corrupt-input rollback, the Windows file-lock failure, and public happy paths. Never use environment variables or production global flags.

## Task 1: Add validated account IDs, account-owned paths, and atomic registry persistence

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/Cargo.toml`
- Modify: `apps/owstats-desktop/src-tauri/src/app_paths.rs`
- Create: `apps/owstats-desktop/src-tauri/src/account_store.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`

- [ ] **Step 1: Write failing path and registry tests**

Add tests that use synthetic UUIDs and never real role IDs:

```rust
use owstats_desktop_lib::account_store::{
    read_registry, write_registry_atomic, AccountId, AccountRegistry, AccountRegistryEntry,
};
use owstats_desktop_lib::app_paths::SharedAppPaths;

#[test]
fn account_paths_are_isolated_under_random_account_directories() {
    let root = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(root.path());
    let first = AccountId::parse("018f95d6-37f0-7c55-b0f7-5ee3e2f65001").unwrap();
    let second = AccountId::parse("018f95d6-37f0-7c55-b0f7-5ee3e2f65002").unwrap();
    let first_paths = shared.account(&first);
    let second_paths = shared.account(&second);

    assert_ne!(first_paths.database(), second_paths.database());
    assert!(first_paths.database().starts_with(shared.accounts_root()));
    assert!(first_paths.credentials().starts_with(first_paths.root()));
    assert!(first_paths.seasons().starts_with(first_paths.root()));
    assert!(AccountId::parse("../escape").is_err());
}

#[test]
fn registry_round_trip_is_atomic_and_contains_no_credentials() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("accounts.json");
    let account_id = AccountId::parse("018f95d6-37f0-7c55-b0f7-5ee3e2f65001").unwrap();
    let registry = AccountRegistry {
        version: 1,
        active_account_id: Some(account_id.clone()),
        accounts: vec![AccountRegistryEntry {
            account_id,
            battle_tag: "Tester#5479".to_string(),
            created_at: "2026-07-16T00:00:00Z".to_string(),
            last_used_at: "2026-07-16T00:00:00Z".to_string(),
            credential_state: "ready".to_string(),
        }],
    };

    write_registry_atomic(&path, &registry).unwrap();
    assert_eq!(read_registry(&path).unwrap(), registry);
    let serialized = std::fs::read_to_string(path).unwrap().to_ascii_lowercase();
    for forbidden in ["token", "cookie", "authorization", "credential\""] {
        assert!(!serialized.contains(forbidden));
    }
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-multi-account-target'
cargo test --test phase9_multi_account account_paths_ registry_round_trip_ -- --nocapture
```

Expected: compilation fails because `account_store` and `SharedAppPaths` do not exist.

- [ ] **Step 3: Add UUID support and implement paths/models**

Add to `[dependencies]`:

```toml
uuid = { version = "1", features = ["v4", "serde"] }
```

Expose these public types and methods:

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(String);

impl AccountId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let parsed = uuid::Uuid::parse_str(value)?;
        Ok(Self(parsed.hyphenated().to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountRegistry {
    pub version: u32,
    pub active_account_id: Option<AccountId>,
    pub accounts: Vec<AccountRegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountRegistryEntry {
    pub account_id: AccountId,
    pub battle_tag: String,
    pub created_at: String,
    pub last_used_at: String,
    pub credential_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountManifest {
    pub version: u32,
    pub account_id: AccountId,
    pub role_id: i64,
    pub battle_tag: String,
    pub created_at: String,
    pub updated_at: String,
}
```

Implement `read_registry`, `write_registry_atomic`, `read_manifest`, and `write_manifest_atomic` with same-directory temporary files, `write_all`, `flush`, `sync_all`, and `rename`. Reject registry/manifest versions other than `1`, duplicate account IDs, mismatched manifest/account directory IDs, and active IDs absent from the registry. BattleTag is not a uniqueness key; two different role IDs may legitimately have the same display text.

Refactor `app_paths.rs` to expose:

```rust
pub struct SharedAppPaths { root: PathBuf }
pub struct AccountPaths { root: PathBuf }

impl SharedAppPaths {
    pub fn from_app_data_dir(root: impl AsRef<Path>) -> Self;
    pub fn from_app(app: &AppHandle) -> Result<Self>;
    pub fn registry(&self) -> PathBuf;
    pub fn accounts_root(&self) -> PathBuf;
    pub fn operations_root(&self) -> PathBuf;
    pub fn browser_profile(&self) -> PathBuf;
    pub fn user_settings(&self) -> PathBuf;
    pub fn legacy_database(&self) -> PathBuf;
    pub fn account(&self, id: &AccountId) -> AccountPaths;
}

impl AccountPaths {
    pub fn root(&self) -> &Path;
    pub fn manifest(&self) -> PathBuf;
    pub fn credentials(&self) -> PathBuf;
    pub fn player_target(&self) -> PathBuf;
    pub fn player_profile(&self) -> PathBuf;
    pub fn database(&self) -> PathBuf;
    pub fn seasons(&self) -> PathBuf;
    pub fn diagnostics(&self) -> PathBuf;
    pub fn backups(&self) -> PathBuf;
}
```

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the Step 2 command again.

Expected: both tests pass and the temporary registry file is absent after success.

- [ ] **Step 5: Commit Task 1**

```powershell
git add apps/owstats-desktop/src-tauri/Cargo.toml apps/owstats-desktop/src-tauri/Cargo.lock apps/owstats-desktop/src-tauri/src/app_paths.rs apps/owstats-desktop/src-tauri/src/account_store.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs
git commit -m "feat: add isolated account storage paths"
```

## Task 2: Add the active-account context manager and safe account DTOs

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/account_context.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/account_store.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`

- [ ] **Step 1: Write failing context and privacy tests**

```rust
use owstats_desktop_lib::account_context::AccountContextManager;
use owstats_desktop_lib::account_store::{AccountManifest, AccountSummary};

#[test]
fn active_context_resolves_one_complete_account_without_exposing_role_id() {
    let root = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(root.path());
    let account_id = AccountId::new();
    let paths = shared.account(&account_id);
    std::fs::create_dir_all(paths.root()).unwrap();
    owstats_desktop_lib::account_store::write_manifest_atomic(
        &paths.manifest(),
        &AccountManifest {
            version: 1,
            account_id: account_id.clone(),
            role_id: 101101,
            battle_tag: "Tester#5479".to_string(),
            created_at: "2026-07-16T00:00:00Z".to_string(),
            updated_at: "2026-07-16T00:00:00Z".to_string(),
        },
    ).unwrap();
    write_registry_atomic(
        &shared.registry(),
        &AccountRegistry {
            version: 1,
            active_account_id: Some(account_id.clone()),
            accounts: vec![AccountRegistryEntry {
                account_id: account_id.clone(),
                battle_tag: "Tester#5479".to_string(),
                created_at: "2026-07-16T00:00:00Z".to_string(),
                last_used_at: "2026-07-16T00:00:00Z".to_string(),
                credential_state: "ready".to_string(),
            }],
        },
    ).unwrap();

    let manager = AccountContextManager::load(shared).unwrap();
    let snapshot = manager.active_snapshot().unwrap();
    assert_eq!(snapshot.account_id(), &account_id);
    assert!(snapshot.paths().database().ends_with("owstats.sqlite3"));

    let summaries: Vec<AccountSummary> = manager.list_accounts().unwrap();
    let json = serde_json::to_string(&summaries).unwrap().to_ascii_lowercase();
    assert!(json.contains("tester#5479"));
    assert!(!json.contains("101101"));
    assert!(!json.contains("roleid"));
    assert!(!json.contains("token"));
}

#[test]
fn missing_active_account_returns_a_chinese_product_error() {
    let root = tempfile::tempdir().unwrap();
    let manager = AccountContextManager::load(SharedAppPaths::from_app_data_dir(root.path())).unwrap();
    let error = manager.active_snapshot().unwrap_err().to_string();
    assert_eq!(error, "尚未添加可用账号，请先在指挥台添加账号。");
}
```

- [ ] **Step 2: Run the tests and verify RED**

```powershell
cargo test --test phase9_multi_account active_context_ missing_active_account_ -- --nocapture
```

Expected: compilation fails because `AccountContextManager`, `AccountManifest`, and `AccountSummary` do not exist.

- [ ] **Step 3: Implement account context and summaries**

Create a manager that owns `SharedAppPaths` plus `Arc<RwLock<AccountRegistry>>`:

```rust
#[derive(Clone)]
pub struct AccountContextManager {
    shared: SharedAppPaths,
    registry: Arc<RwLock<AccountRegistry>>,
}

#[derive(Debug, Clone)]
pub struct AccountContextSnapshot {
    account_id: AccountId,
    paths: AccountPaths,
}

impl AccountContextManager {
    pub fn load(shared: SharedAppPaths) -> Result<Self>;
    pub fn active_snapshot(&self) -> Result<AccountContextSnapshot>;
    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>>;
    pub fn registry_snapshot(&self) -> Result<AccountRegistry>;
    pub fn replace_registry(&self, registry: AccountRegistry) -> Result<()>;
    pub fn shared_paths(&self) -> &SharedAppPaths;
}
```

Use `std::sync::RwLock`; never keep a lock held during SQLite or network work. `active_snapshot` clones only account ID and validated account paths.

Add safe serialized DTOs to `models.rs` or re-export them there:

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub account_id: String,
    pub battle_tag: String,
    pub is_active: bool,
    pub credential_state: String,
    pub created_at: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountManagerStatus {
    pub active_account_id: Option<String>,
    pub accounts: Vec<AccountSummary>,
    pub message: String,
}
```

Register `AccountContextManager` in `tauri::Builder::setup`, after loading `app.path().app_data_dir()`. Return startup errors through `anyhow` without serializing registry contents.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the Step 2 command again.

Expected: both tests pass; serialized summaries contain BattleTag but no role ID or token-like keys.

- [ ] **Step 5: Commit Task 2**

```powershell
git add apps/owstats-desktop/src-tauri/src/account_context.rs apps/owstats-desktop/src-tauri/src/account_store.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs
git commit -m "feat: add active account context"
```

## Task 3: Route every player-data path through the active account snapshot

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/credentials.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/player_target.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/db.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_multi_season_pages.rs`

- [ ] **Step 1: Write a failing two-database routing test**

```rust
#[test]
fn switching_active_context_routes_queries_to_separate_databases() {
    let fixture = MultiAccountFixture::new();
    fixture.create_account("018f95d6-37f0-7c55-b0f7-5ee3e2f65001", "First#1111", 101101);
    fixture.create_account("018f95d6-37f0-7c55-b0f7-5ee3e2f65002", "Second#2222", 202202);
    fixture.seed_match("018f95d6-37f0-7c55-b0f7-5ee3e2f65001", "first-match");
    fixture.seed_match("018f95d6-37f0-7c55-b0f7-5ee3e2f65002", "second-match");

    fixture.activate("018f95d6-37f0-7c55-b0f7-5ee3e2f65001");
    assert_eq!(fixture.list_match_ids(), vec!["first-match"]);
    fixture.activate("018f95d6-37f0-7c55-b0f7-5ee3e2f65002");
    assert_eq!(fixture.list_match_ids(), vec!["second-match"]);
}
```

The fixture must call the same context/path helpers used by commands, not manually select a database after activation.

- [ ] **Step 2: Run the routing test and verify RED**

```powershell
cargo test --test phase9_multi_account switching_active_context_ -- --nocapture
```

Expected: FAIL because command helpers still open root `owstats.sqlite3`.

- [ ] **Step 3: Replace fixed path helpers with snapshot helpers**

In `commands.rs`, use one helper for all player-data commands:

```rust
fn active_account(
    accounts: &State<'_, crate::account_context::AccountContextManager>,
) -> Result<crate::account_context::AccountContextSnapshot, String> {
    accounts.active_snapshot().map_err(to_message)
}

fn account_database(snapshot: &crate::account_context::AccountContextSnapshot) -> OwDatabase {
    OwDatabase::new(snapshot.paths().database())
}
```

Add `State<AccountContextManager>` and resolve one snapshot at command entry for:

```text
get_overview
list_match_filters
list_match_timeline
search_match_player_options
get_match_details
refresh_match_evaluations
get_friend_analytics
get_friend_details
get_onboarding_status
get_personal_season_review
get_credential_status
get_player_profile
get_player_target_status
get_login_target_status
confirm_login_target_switch
keep_current_player_target
retry_login_target_binding
clear_player_target
get_rankings
get_season_analysis
get_career_mmr_overview
get_role_mmr_history
get_role_mmr_mode_breakdown
get_role_mmr_season_breakdown
get_mmr_match_explanation
get_recent_mmr_explanations
rebuild_career_mmr
get_career_mmr_build_status
sync_season
```

For each command, use the same snapshot for database, credentials, player target, player profile, seasons, diagnostics, and backups. `get_user_settings`, `save_user_settings`, `check_app_update`, and browser profile resolution stay on `SharedAppPaths`.

Add explicit-path credential helpers:

```rust
pub fn get_credential_status_for_account(paths: &AccountPaths) -> Result<CredentialStatus> {
    credential_status_for_path(&paths.credentials())
}

pub fn read_saved_credentials_for_account(paths: &AccountPaths) -> Result<DashenCredentials> {
    let text = std::fs::read_to_string(paths.credentials())?;
    Ok(serde_json::from_str(&text)?)
}
```

Remove product command calls to `credentials::credential_path(app)` and `player_target::player_target_path(app)`. Keep legacy path helpers only inside migration code.

In `db.rs`, pass `AccountPaths::backups()` into schema backup/privacy migration helpers instead of deriving shared app-data semantics from `database.parent()`.

Keep role ID available internally for target/profile lookup but remove it from public serialization:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatus {
    pub state: String,
    pub message: String,
    pub exposes_secret: bool,
    #[serde(skip_serializing)]
    pub role_id: Option<i64>,
    pub saved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerProfile {
    #[serde(skip_serializing)]
    pub role_id: Option<i64>,
    pub player_id: Option<String>,
    pub avatar_url: Option<String>,
    pub endorsement_level: Option<i64>,
    pub play_time_hours: Option<f64>,
    pub title: Option<String>,
    pub refreshed_at: Option<String>,
}
```

Add tests that serialize both DTOs and assert `roleId`, the synthetic numeric role value, and token-like keys are absent.

- [ ] **Step 4: Run routing and existing page-isolation tests**

```powershell
cargo test --test phase9_multi_account switching_active_context_ -- --nocapture
cargo test --test phase8_command_center -- --nocapture
cargo test --test phase8_multi_season_pages -- --nocapture
```

Expected: all pass; first and second accounts return only their own match IDs.

- [ ] **Step 5: Scan for remaining fixed account paths**

```powershell
rg -n "\.database\(\)|credential_path\(|player_target_path\(|player_profile_path\(" apps/owstats-desktop/src-tauri/src/commands.rs
```

Expected: matches only the new snapshot-based helpers or shared settings helper; no app-root account path calls remain.

- [ ] **Step 6: Commit Task 3**

```powershell
git add apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/src/player_target.rs apps/owstats-desktop/src-tauri/src/db.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs apps/owstats-desktop/src-tauri/tests/phase8_multi_season_pages.rs
git commit -m "refactor: route data through active account"
```

## Task 4: Add legacy single-account adoption with an idempotent migration journal

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/account_migration.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/app_paths.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/account_context.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`

- [ ] **Step 1: Write failing successful/failed/repeated migration tests**

```rust
#[test]
fn legacy_database_is_adopted_only_after_confirmed_account_identity() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    assert_eq!(fixture.manager().migration_state().unwrap(), "waitingForAccount");

    let account = fixture.confirm_account(101101, "Tester#5479");
    fixture.adopt_legacy(&account).unwrap();

    assert!(account.paths().database().is_file());
    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
    assert!(!fixture.shared().legacy_database().exists());
    assert_eq!(fixture.manager().migration_state().unwrap(), "complete");
}

#[test]
fn failed_or_repeated_legacy_migration_never_loses_or_duplicates_data() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let valid_database = std::fs::read(fixture.shared().legacy_database()).unwrap();
    std::fs::write(fixture.shared().legacy_database(), b"not-a-sqlite-database").unwrap();
    let account = fixture.confirm_account(101101, "Tester#5479");
    assert!(fixture.adopt_legacy(&account).is_err());
    assert!(fixture.shared().legacy_database().is_file());

    fixture.replace_legacy_database(&valid_database);
    fixture.adopt_legacy(&account).unwrap();
    fixture.adopt_legacy(&account).unwrap();
    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
}
```

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --test phase9_multi_account legacy_database_ failed_or_repeated_legacy_ -- --nocapture
```

Expected: compilation fails because migration APIs do not exist.

- [ ] **Step 3: Implement journaled migration**

Create these persisted states:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationJournal {
    pub version: u32,
    pub account_id: AccountId,
    pub phase: MigrationPhase,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MigrationPhase {
    Copying,
    Validating,
    Publishing,
    CleaningLegacy,
    Complete,
}
```

Implement:

```rust
pub fn detect_legacy_state(shared: &SharedAppPaths) -> Result<LegacyMigrationState>;
pub fn adopt_legacy_account(
    shared: &SharedAppPaths,
    account: &AccountContextSnapshot,
) -> Result<()>;
pub fn recover_interrupted_migration(shared: &SharedAppPaths) -> Result<()>;
```

Copy legacy `owstats.sqlite3`, player target, player profile, seasons, diagnostics, and backups into a sibling temporary account directory. Validate the copied SQLite with `OwDatabase::initialize()` and a read query before publishing. Write each phase atomically. Publish the account directory and registry before removing old root files. A second call after `Complete` must return success without copying again.

- [ ] **Step 4: Run migration tests and verify GREEN**

Run the Step 2 command again.

Expected: all migration tests pass; failure leaves legacy files intact; retry creates one copy of the match.

- [ ] **Step 5: Commit Task 4**

```powershell
git add apps/owstats-desktop/src-tauri/src/account_migration.rs apps/owstats-desktop/src-tauri/src/app_paths.rs apps/owstats-desktop/src-tauri/src/account_context.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs
git commit -m "feat: migrate legacy account data safely"
```

## Task 5: Stage add-account and relogin results before publishing credentials

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/account_login.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/credentials.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs`

- [ ] **Step 1: Write failing identity-result tests**

```rust
#[test]
fn add_account_creates_new_or_updates_existing_role_without_duplicates() {
    let fixture = AccountLoginFixture::new();
    let first = fixture.publish_add(LoginCapture::synthetic(101101, "First#1111"));
    let updated = fixture.publish_add(LoginCapture::synthetic(101101, "Renamed#1111"));

    assert_eq!(first.kind, "newAccountAdded");
    assert_eq!(updated.kind, "existingAccountUpdated");
    assert_eq!(first.account_id, updated.account_id);
    assert_eq!(fixture.accounts().len(), 1);
    assert_eq!(fixture.accounts()[0].battle_tag, "Renamed#1111");
}

#[test]
fn relogin_with_another_role_never_overwrites_selected_account() {
    let fixture = AccountLoginFixture::with_account(101101, "First#1111", "old-secret");
    let selected = fixture.only_account_id();
    let result = fixture.publish_relogin(
        &selected,
        LoginCapture::synthetic(202202, "Second#2222"),
    );

    assert_eq!(result.kind, "differentNewAccount");
    assert_eq!(fixture.saved_token(&selected), "old-secret");
    assert_eq!(fixture.accounts().len(), 1);
}
```

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --test phase9_multi_account add_account_ relogin_with_another_ -- --nocapture
```

Expected: compilation fails because `account_login` and account operation results do not exist.

- [ ] **Step 3: Separate browser capture from persistence**

Change `credentials.rs` so capture never writes the final credential path:

```rust
pub(crate) async fn capture_dashen_login(app: AppHandle) -> Result<BrowserLoginOutcome> {
    // Existing browser launch, role/token exchange, BattleTag stability, and redaction logic.
    // Return credentials + detected_battle_tag without calling save_credentials_atomic.
}
```

Keep `cancel_dashen_login` and progress events unchanged. Add a regression assertion in `phase4_login_redesign.rs` that `capture_dashen_login` has no `credential_path` or direct final save call.

- [ ] **Step 4: Implement staged account publication**

Define a result that never includes role ID or secrets:

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginResult {
    pub kind: String,
    pub account_id: Option<String>,
    pub battle_tag: Option<String>,
    pub message: String,
    pub should_offer_switch: bool,
}
```

Implement:

```rust
pub async fn add_account(
    app: AppHandle,
    accounts: &AccountContextManager,
) -> Result<AccountLoginResult>;

pub async fn relogin_account(
    app: AppHandle,
    accounts: &AccountContextManager,
    expected: &AccountId,
) -> Result<AccountLoginResult>;
```

Write captured credentials and manifest to `.account-operations/pending-logins/<operation_id>`. Validate credential completeness and the stable BattleTag. For add, match role ID against manifests and either publish a new UUID directory or atomically replace credentials/BattleTag for the existing account. For relogin, compare against the expected manifest before any replacement. Return `differentExistingAccount` or `differentNewAccount` on mismatch and remove pending files.

After successful publication, call `adopt_legacy_account` only when the migration state is `waitingForAccount`.

- [ ] **Step 5: Add Tauri handlers under the shared account-operation permit**

```rust
#[tauri::command]
pub async fn add_account(
    app: AppHandle,
    accounts: State<'_, AccountContextManager>,
    operations: State<'_, SyncCoordinator>,
) -> Result<AccountLoginResult, String> {
    let _permit = operations.try_acquire_account_operation().map_err(str::to_string)?;
    crate::account_login::add_account(app, accounts.inner()).await.map_err(to_message)
}

#[tauri::command]
pub async fn relogin_account(
    app: AppHandle,
    account_id: String,
    accounts: State<'_, AccountContextManager>,
    operations: State<'_, SyncCoordinator>,
) -> Result<AccountLoginResult, String> {
    let _permit = operations.try_acquire_account_operation().map_err(str::to_string)?;
    let account_id = AccountId::parse(&account_id).map_err(|_| "账号标识无效。".to_string())?;
    crate::account_login::relogin_account(app, accounts.inner(), &account_id).await.map_err(to_message)
}
```

Register both handlers and remove product use of `start_dashen_login` once the frontend switches in Task 8.

- [ ] **Step 6: Run login regression tests and verify GREEN**

```powershell
cargo test --test phase9_multi_account add_account_ relogin_with_another_ -- --nocapture
cargo test --test phase4_login_redesign -- --nocapture
```

Expected: all pass; existing BattleTag stability/security tests remain green.

- [ ] **Step 7: Commit Task 5**

```powershell
git add apps/owstats-desktop/src-tauri/src/account_login.rs apps/owstats-desktop/src-tauri/src/credentials.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs apps/owstats-desktop/src-tauri/tests/phase4_login_redesign.rs
git commit -m "feat: add staged multi-account login"
```

## Task 6: Implement account listing, atomic switching, and journaled deletion

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/account_removal.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/account_context.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync_coordinator.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`

- [ ] **Step 1: Write failing switch/delete/recovery tests**

```rust
#[test]
fn switch_updates_active_account_and_recent_use_without_opening_browser() {
    let fixture = MultiAccountFixture::with_two_accounts();
    let second = fixture.second_account_id();
    let result = fixture.switch_account(&second).unwrap();
    assert_eq!(result.active_account_id.as_deref(), Some(second.as_str()));
    assert_eq!(fixture.manager().active_snapshot().unwrap().account_id(), &second);
    assert_eq!(fixture.browser_launch_count(), 0);
}

#[test]
fn removing_active_account_deletes_only_its_directory_and_selects_recent_fallback() {
    let fixture = MultiAccountFixture::with_three_accounts();
    let active = fixture.active_account_id();
    let expected_fallback = fixture.most_recent_inactive_account_id();
    let unrelated_database = fixture.database_bytes(&expected_fallback);

    let result = fixture.remove_account(&active).unwrap();
    assert!(result.removed);
    assert_eq!(result.active_account_id.as_deref(), Some(expected_fallback.as_str()));
    assert!(!fixture.shared().account(&active).root().exists());
    assert_eq!(fixture.database_bytes(&expected_fallback), unrelated_database);
}

#[cfg(windows)]
#[test]
fn failed_delete_restores_account_and_keeps_registry_entry() {
    use std::os::windows::fs::OpenOptionsExt;

    let fixture = MultiAccountFixture::with_two_accounts();
    let target = fixture.second_account_id();
    let locked_database = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(fixture.shared().account(&target).database())
        .unwrap();
    assert!(fixture.remove_account(&target).is_err());
    assert!(fixture.shared().account(&target).root().exists());
    assert!(fixture
        .manager()
        .registry_snapshot()
        .unwrap()
        .accounts
        .iter()
        .any(|entry| entry.account_id == target));
    drop(locked_database);
}
```

- [ ] **Step 2: Run tests and verify RED**

```powershell
cargo test --test phase9_multi_account switch_updates_ removing_active_ failed_delete_ -- --nocapture
```

Expected: compilation fails because switching/removal APIs do not exist.

- [ ] **Step 3: Implement switching and list status**

Add manager methods:

```rust
pub fn status(&self) -> Result<AccountManagerStatus>;
pub fn switch_active(&self, account_id: &AccountId, now: &str) -> Result<AccountManagerStatus>;
```

`switch_active` validates the account directory and manifest, updates active ID and last-used time in one registry write, then swaps the in-memory registry only after disk success.

- [ ] **Step 4: Implement deletion journal and recovery**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletionJournal {
    pub version: u32,
    pub account_id: AccountId,
    pub original_path: PathBuf,
    pub quarantine_path: PathBuf,
    pub phase: DeletionPhase,
    pub fallback_account_id: Option<AccountId>,
}

pub fn remove_account(
    accounts: &AccountContextManager,
    account_id: &AccountId,
) -> Result<RemoveAccountResult>;

pub fn recover_interrupted_deletion(shared: &SharedAppPaths) -> Result<()>;
```

Before rename/delete, resolve both paths and verify they remain under app-data `accounts` or `.account-operations`. Reject symlinks/junctions and any path outside the intended root. Write the journal, rename the account directory to quarantine, recursively delete quarantine, update the registry, and remove the journal. If recursive deletion fails, rename back and keep the registry unchanged. If rollback also fails, keep the journal and return a recovery-required Chinese error.

On Windows, reject reparse points before traversal:

```rust
#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
```

- [ ] **Step 5: Add safe command results and handlers**

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveAccountResult {
    pub removed: bool,
    pub active_account_id: Option<String>,
    pub message: String,
    pub reload_required: bool,
}
```

Add `list_accounts`, `switch_account`, and `remove_account` Tauri commands. All mutation commands must acquire `try_acquire_account_operation`; sync and database rebuild operations must use the same coordinator.

- [ ] **Step 6: Run tests and verify GREEN**

Run the Step 2 command again plus:

```powershell
cargo test sync_coordinator -- --nocapture
```

Expected: switch/delete tests pass and every account mutation is mutually exclusive with sync.

- [ ] **Step 7: Commit Task 6**

```powershell
git add apps/owstats-desktop/src-tauri/src/account_removal.rs apps/owstats-desktop/src-tauri/src/account_context.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/src/sync_coordinator.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs
git commit -m "feat: switch and remove local accounts"
```

## Task 7: Add frontend account contracts and a reusable account manager component

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Create: `apps/owstats-desktop/src/features/command-center/AccountManager.tsx`
- Create: `apps/owstats-desktop/scripts/accountManagement.test.tsx`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Write a failing server-rendered component test**

```tsx
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountManager } from "../src/features/command-center/AccountManager";

const html = renderToStaticMarkup(createElement(AccountManager, {
  accounts: [
    { accountId: "account-a", battleTag: "Tester#5479", isActive: true, credentialState: "ready", createdAt: "", lastUsedAt: "2026-07-16T00:00:00Z" },
    { accountId: "account-b", battleTag: "Second#2222", isActive: false, credentialState: "stale", createdAt: "", lastUsedAt: "2026-07-15T00:00:00Z" },
  ],
  busyAction: null,
  onAdd: () => undefined,
  onSwitch: () => undefined,
  onRelogin: () => undefined,
  onRemove: () => undefined,
}));

assert.match(html, /添加新的网易大神账号/);
assert.match(html, /Tester#5479/);
assert.match(html, /Second#2222/);
assert.equal((html.match(/重新登录/g) ?? []).length, 2);
assert.equal((html.match(/退出账号/g) ?? []).length, 2);
assert.match(html, /disabled=""[^>]*>当前账号</);
assert.match(html, />切换账号</);
assert.doesNotMatch(html, /roleId|token|customerToken/);
```

- [ ] **Step 2: Add the test script and verify RED**

Add `tsx scripts/accountManagement.test.tsx` to `test:components`, then run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/accountManagement.test.tsx
```

Expected: module-not-found failure for `AccountManager`.

- [ ] **Step 3: Define frontend contracts and API wrappers**

Remove `roleId` from the public `CredentialStatus` and `PlayerProfile` TypeScript types before adding account contracts:

```ts
export type CredentialStatus = {
  state: string;
  message: string;
  exposesSecret: boolean;
  savedAt: string | null;
};

export type PlayerProfile = {
  playerId: string | null;
  avatarUrl: string | null;
  endorsementLevel: number | null;
  playTimeHours: number | null;
  title: string | null;
  refreshedAt: string | null;
};
```

```ts
export type AccountSummary = {
  accountId: string;
  battleTag: string;
  isActive: boolean;
  credentialState: "ready" | "stale" | "incomplete" | "missing" | string;
  createdAt: string;
  lastUsedAt: string;
};

export type AccountManagerStatus = {
  activeAccountId: string | null;
  accounts: AccountSummary[];
  message: string;
};

export type AccountLoginResult = {
  kind: "newAccountAdded" | "existingAccountUpdated" | "differentExistingAccount" | "differentNewAccount" | string;
  accountId: string | null;
  battleTag: string | null;
  message: string;
  shouldOfferSwitch: boolean;
};

export type RemoveAccountResult = {
  removed: boolean;
  activeAccountId: string | null;
  message: string;
  reloadRequired: boolean;
};
```

```ts
export const listAccounts = () => invoke<AccountManagerStatus>("list_accounts");
export const addAccount = () => invoke<AccountLoginResult>("add_account");
export const switchAccount = (accountId: string) => invoke<AccountManagerStatus>("switch_account", { accountId });
export const reloginAccount = (accountId: string) => invoke<AccountLoginResult>("relogin_account", { accountId });
export const removeAccount = (accountId: string) => invoke<RemoveAccountResult>("remove_account", { accountId });
```

Do not add roleId or secret-bearing fields to these account types. Update any frontend fallback that referenced `credentialStatus.roleId` or `profile.roleId` to use the active account BattleTag or a neutral `未登录玩家` label.

- [ ] **Step 4: Implement the stateless AccountManager component**

Props must be:

```tsx
type AccountManagerProps = {
  accounts: AccountSummary[];
  busyAction: { kind: "add" | "switch" | "relogin" | "remove"; accountId?: string } | null;
  onAdd: () => void;
  onSwitch: (account: AccountSummary) => void;
  onRelogin: (account: AccountSummary) => void;
  onRemove: (account: AccountSummary) => void;
};
```

Render one add row followed by one row per account. Every account row must always have three action positions. Current account renders a disabled `当前账号` first button; inactive accounts render `切换账号`. Use a native `<dialog>` or the project’s existing accessible dialog pattern for removal confirmation, with BattleTag, permanent-delete explanation, cancel, and `永久退出账号`.

- [ ] **Step 5: Run component test and verify GREEN**

```powershell
npx tsx scripts/accountManagement.test.tsx
```

Expected: exit 0 with no secret field names in rendered markup.

- [ ] **Step 6: Commit Task 7**

```powershell
git add apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/shared/api.ts apps/owstats-desktop/src/features/command-center/AccountManager.tsx apps/owstats-desktop/scripts/accountManagement.test.tsx apps/owstats-desktop/package.json
git commit -m "feat: add account manager component"
```

## Task 8: Integrate account management into the command center and enforce reload isolation

**Files:**
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`
- Modify: `apps/owstats-desktop/scripts/accountManagement.test.tsx`
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Modify: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add failing source-contract assertions**

```ts
const onboardingSource = readFileSync(new URL("../src/pages/OnboardingPage.tsx", import.meta.url), "utf8");
assert.match(onboardingSource, /listAccounts\(\)/);
assert.match(onboardingSource, /<AccountManager/);
assert.match(onboardingSource, /window\.location\.reload\(\)/);
assert.doesNotMatch(onboardingSource, /runLogout/);
assert.doesNotMatch(onboardingSource, />退出登录</);
assert.match(onboardingSource, /dashboardCache = null/);
```

- [ ] **Step 2: Run source-contract tests and verify RED**

```powershell
npx tsx scripts/accountManagement.test.tsx
npx tsx scripts/phase6Shell.test.ts
```

Expected: assertions fail because the old logout danger zone is still present.

- [ ] **Step 3: Load account state with dashboard state**

Extend `DashboardSnapshot`:

```ts
type DashboardSnapshot = {
  accountStatus: AccountManagerStatus;
  status: OnboardingStatus | null;
  profile: PlayerProfile | null;
  targetStatus: LoginTargetStatus | null;
};
```

Load `listAccounts()` first under the existing request-generation guard. When `activeAccountId` is null, apply `{ accountStatus, status: null, profile: null, targetStatus: null }` and render the account manager plus an “添加账号后即可同步和查看本地数据” empty state; do not call account-scoped commands. When an active account exists, load `getOnboardingStatus`, `getPlayerProfile`, and `getLoginTargetStatus` in `Promise.all`, then attach the already-read account status. Never cache a dashboard snapshot without its active account ID. If cached active ID differs from the fresh account status, discard the cache.

- [ ] **Step 4: Replace logout handlers with four account operations**

Implement handlers with the existing frontend operation lock:

```tsx
async function runSwitchAccount(account: AccountSummary) {
  if (!beginAccountOperation()) return;
  setBusyAccountAction({ kind: "switch", accountId: account.accountId });
  try {
    await switchAccount(account.accountId);
    dashboardCache = null;
    window.location.reload();
  } catch (err) {
    setError(`切换账号失败：${productErrorMessage(err)}。`);
    releaseOperationLock();
    setBusyAccountAction(null);
  }
}
```

Implement matching `runAddAccount`, `runReloginAccount`, and `runRemoveAccount`. For add/update/mismatch results, show the backend’s safe Chinese message and refresh the account list. For successful removal with `reloadRequired`, clear `dashboardCache` and reload. On cancel, do not invoke `removeAccount`.

- [ ] **Step 5: Replace the danger zone with AccountManager**

Remove the `commandCenterDangerZone`, `runLogout`, `loggingOut`, `logout` import, and product-visible `退出登录`. Stop using `credentialStatus.roleId` as a display fallback; the active account BattleTag is the identity surface. Render `AccountManager` after the main command-center content. Preserve the existing sync target and season workbench.

Remove `commands::logout` and `commands::start_dashen_login` from the Tauri handler list after product calls use `remove_account`, `add_account`, and `relogin_account`. Keep `cancel_dashen_login` because add/relogin still use browser capture. Add source-contract assertions that both obsolete handlers are absent from `lib.rs`.

- [ ] **Step 6: Add design-system CSS and responsive behavior**

Add selectors scoped under `.phase6CommandCenter`:

```css
.phase6CommandCenter .accountManagerPanel {
  padding: 24px;
  border: 1px solid var(--glass-border);
  border-radius: 18px;
  background: var(--glass-secondary);
  box-shadow: none;
}

.phase6CommandCenter .accountManagerAddRow,
.phase6CommandCenter .accountManagerRow {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) auto;
  gap: 16px;
  align-items: center;
  min-height: 76px;
  border-top: 1px solid var(--glass-border);
}

.phase6CommandCenter .accountManagerActions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

@media (max-width: 900px) {
  .phase6CommandCenter .accountManagerAddRow,
  .phase6CommandCenter .accountManagerRow {
    grid-template-columns: 1fr;
  }
}
```

Reuse existing primary/ghost/danger button tokens and 44px minimum hit areas. Do not add new colors, fonts, shadows, or nested white cards.

- [ ] **Step 7: Run frontend focused tests and build**

```powershell
npx tsx scripts/accountManagement.test.tsx
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/rankingCommandCenterRedesign.test.ts
npm run build
```

Expected: all focused tests pass; TypeScript/Vite build exits 0; only the existing large-chunk warning is acceptable.

- [ ] **Step 8: Commit Task 8**

```powershell
git add apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/accountManagement.test.tsx apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts apps/owstats-desktop/src-tauri/src/lib.rs
git commit -m "feat: manage accounts from command center"
```

## Task 9: Complete privacy, crash-recovery, cross-page, and release verification

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`
- Modify: `apps/owstats-desktop/scripts/accountManagement.test.tsx`
- Modify: `docs/maintenance.md`
- Modify only if tests expose a defect: files changed in Tasks 1–8

- [ ] **Step 1: Add the final privacy and isolation matrix**

Extend Rust tests to cover:

```rust
#[test]
fn account_public_json_never_contains_private_identity_or_secrets() {
    let status = MultiAccountFixture::with_two_accounts().manager().status().unwrap();
    let json = serde_json::to_string(&status).unwrap().to_ascii_lowercase();
    for forbidden in ["roleid", "role_id", "token", "customertoken", "cookie", "authorization", "credential\""] {
        assert!(!json.contains(forbidden), "public account JSON exposed {forbidden}");
    }
}

#[test]
fn all_account_owned_paths_change_together_after_switch() {
    let fixture = MultiAccountFixture::with_two_accounts();
    let first = fixture.active_snapshot();
    fixture.activate_second();
    let second = fixture.active_snapshot();
    assert_ne!(first.paths().credentials(), second.paths().credentials());
    assert_ne!(first.paths().player_target(), second.paths().player_target());
    assert_ne!(first.paths().player_profile(), second.paths().player_profile());
    assert_ne!(first.paths().database(), second.paths().database());
    assert_ne!(first.paths().seasons(), second.paths().seasons());
    assert_ne!(first.paths().diagnostics(), second.paths().diagnostics());
    assert_ne!(first.paths().backups(), second.paths().backups());
}
```

Add test cases for empty account list, corrupt registry, missing active directory, invalid UUID/path traversal, delete last account, deletion journal recovery, migration journal recovery, relogin existing other account, and credential-expired local-data access.

- [ ] **Step 2: Run the complete backend suite**

```powershell
cd apps\owstats-desktop\src-tauri
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'owstats-multi-account-target'
cargo fmt --check
cargo test --tests --quiet
```

Expected: exit 0; the real-upstream test remains ignored; no test output contains real credentials or player IDs.

- [ ] **Step 3: Run the complete frontend suite and build**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: exit 0; text-integrity check passes; only the existing Vite chunk-size warning is acceptable.

- [ ] **Step 4: Run sanitized Chromium acceptance at required widths**

Use Playwright CLI with a synthetic Tauri invoke mock and verify at 2540×1344, 1920×1080, and 1440×900:

```text
1. Add row is separate from account rows.
2. Each account row exposes exactly three action positions.
3. Current account button is disabled and labeled 当前账号.
4. Inactive account shows 切换账号.
5. Delete dialog names only the synthetic BattleTag and permanent-delete scope.
6. Narrow layout wraps actions without horizontal scrolling.
7. Keyboard focus reaches add, switch, relogin, remove, cancel, and confirm.
```

Save only sanitized screenshots under ignored `output/playwright/`; remove generated `.playwright-cli` and `test-results` artifacts before final status checks.

- [ ] **Step 5: Verify a clean upgrade fixture without real app data**

Create a temporary app-data fixture containing synthetic legacy `owstats.sqlite3`, credentials, target, profile, and season files. Start the backend migration helpers against that temporary directory, confirm one account, and verify:

```text
- the old match appears once in the account database;
- legacy account-owned files are removed only after validation;
- shared settings remain at root;
- a second startup makes no additional changes;
- no raw role ID or token is printed.
```

- [ ] **Step 6: Update maintenance documentation**

Document the runtime shape without secrets:

```markdown
## 多账号运行时数据

- `accounts.json` 保存安全账号索引与当前账号指针。
- `accounts/<account_id>/` 保存单个账号的数据库、同步文件和私密凭据。
- 排障报告只能记录账号数量、活动状态和安全错误，不得复制账号清单、凭据或真实路径内容。
- 删除账号会删除该账号目录；删除失败时应保留恢复 journal 并重新启动应用恢复。
```

- [ ] **Step 7: Run final hygiene checks**

```powershell
git diff --check
git status --short
git diff --name-only
rg -n "roleId|role_id|customerToken|authorization|cookie|token" apps/owstats-desktop/src/features/command-center/AccountManager.tsx apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/scripts/accountManagement.test.tsx
```

Expected:

- `git diff --check` exits 0.
- Status contains only intended source, test, and documentation files.
- Frontend account DTO/component files contain no role ID or token fields; test assertions may mention forbidden names only in negative checks.
- No real credential, browser profile, local database, generated screenshot, `target`, `dist`, or `node_modules` file is staged.

- [ ] **Step 8: Commit Task 9**

```powershell
git add apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs apps/owstats-desktop/scripts/accountManagement.test.tsx docs/maintenance.md
git commit -m "test: verify multi-account isolation"
```

## Final acceptance checklist

- [ ] Two synthetic accounts can be added and switched without opening a browser for switch.
- [ ] Account credentials, target, profile, database, seasons, diagnostics, and backups have distinct paths.
- [ ] Command center, matches, review, ranking, friend analysis, and career MMR read only the active database.
- [ ] Same role ID updates BattleTag and credentials without duplicating an account.
- [ ] Relogin with another role never overwrites the selected account.
- [ ] Removing an account physically deletes its complete account directory and preserves all other accounts.
- [ ] Removing the active account chooses the most recently used fallback; removing the last account yields the empty state.
- [ ] Legacy single-account data migrates once and remains intact after injected failure.
- [ ] Public DTOs, logs, errors, tests, and diagnostics expose no role ID or token-like values.
- [ ] `cargo fmt --check`, `cargo test --tests`, `npm test`, and `npm run build` all pass.
- [ ] Sanitized Chromium acceptance passes at the three required viewports.
