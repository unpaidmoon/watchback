# Install-Directory Data Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Store every OWStatsLocal-owned writable artifact under `<install directory>\data`, migrate legacy data during elevated NSIS installation, and publish no MSI.

**Architecture:** `SharedAppPaths` remains the sole business-path façade but receives `<executable parent>\data`. The main executable gains a non-GUI `--prepare-install-data` mode that NSIS calls after installing program files; this elevated path copies a selected source into sibling `data.migrating`, validates account metadata and SQLite, then atomically publishes `data`. Normal desktop startup only verifies and opens the already-published directory.

**Tech Stack:** Rust 2021, Tauri 2, rusqlite, serde/serde_json, NSIS hooks, TypeScript contract tests, PowerShell release verification.

---

## File Map

- `apps/owstats-desktop/src-tauri/src/app_paths.rs`: install/data path derivation.
- `apps/owstats-desktop/src-tauri/src/install_data_migration.rs`: elevated copy, validation, marker, and atomic publish.
- `apps/owstats-desktop/src-tauri/src/main.rs`: select migration CLI mode or GUI mode.
- `apps/owstats-desktop/src-tauri/src/lib.rs`: normal startup verification and Tauri initialization.
- `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`: synthetic path and migration integration tests.
- `apps/owstats-desktop/src-tauri/windows/installer-hooks.nsh`: source selection, migration invocation, ACLs, write probe, uninstall choice.
- `apps/owstats-desktop/src-tauri/tauri.conf.json`: NSIS-only packaging.
- `apps/owstats-desktop/scripts/installDirectoryData.test.ts`: installer/config contract.
- `apps/owstats-desktop/package.json`: resource-test registration.
- `scripts/verify.ps1`: release-boundary scans.
- `README.md`, `docs/maintenance.md`, `docs/windows-release.md`: operator and product documentation.

### Task 1: Derive Business Data from the Executable

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/app_paths.rs`

- [ ] **Step 1: Add failing path tests**

Add to the existing unit-test module:

```rust
use super::{data_root_from_executable, SharedAppPaths};
use std::path::{Path, PathBuf};

#[test]
fn executable_parent_owns_data() {
    assert_eq!(
        data_root_from_executable(Path::new(r"D:\应用\Watchback\Watchback.exe")).unwrap(),
        PathBuf::from(r"D:\应用\Watchback\data"),
    );
}

#[test]
fn empty_executable_path_is_rejected() {
    assert!(data_root_from_executable(Path::new("")).is_err());
}

#[test]
fn shared_business_paths_stay_under_install_data() {
    let root = Path::new(r"D:\Watchback\data");
    let paths = SharedAppPaths::from_data_root(root);
    assert_eq!(paths.root(), root);
    assert!(paths.registry().starts_with(root));
    assert!(paths.user_settings().starts_with(root));
    assert!(paths.browser_profile().starts_with(root));
    assert!(paths.accounts_root().starts_with(root));
}
```

- [ ] **Step 2: Verify the tests fail**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test app_paths::tests --lib
```

Expected: compilation fails because `data_root_from_executable` and `from_data_root` are missing.

- [ ] **Step 3: Implement the path API**

Remove `tauri::{AppHandle, Manager}` from this module and add:

```rust
use anyhow::{anyhow, Result};

pub fn data_root_from_executable(executable: &Path) -> Result<PathBuf> {
    let install = executable
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("无法定位安装目录"))?;
    Ok(install.join("data"))
}
```

Rename the public constructor:

```rust
pub fn from_data_root(root: impl AsRef<Path>) -> Self {
    Self {
        root: root.as_ref().to_path_buf(),
    }
}
```

Retain this hidden compatibility alias because tests and developer-only CLIs explicitly inject temporary roots:

```rust
#[doc(hidden)]
pub fn from_app_data_dir(root: impl AsRef<Path>) -> Self {
    Self::from_data_root(root)
}
```

Add:

```rust
pub fn install_migration_staging(&self) -> PathBuf {
    self.root.with_file_name("data.migrating")
}

pub fn install_migration_marker(&self) -> PathBuf {
    self.root.join(".install-data-migration-v1.json")
}
```

Delete both `from_app(&AppHandle)` methods so no business path can be built from Tauri AppData.

- [ ] **Step 4: Verify the path tests pass**

Run:

```powershell
cargo test app_paths::tests --lib
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```powershell
git add apps/owstats-desktop/src-tauri/src/app_paths.rs
git commit -m "refactor: derive runtime data from executable"
```

### Task 2: Build a Safe Elevated Migration Function

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/install_data_migration.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing copy and idempotency tests**

Create `tests/install_directory_data.rs`:

```rust
use std::fs;
use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::install_data_migration::{
    prepare_install_data, InstallDataMigrationOutcome,
};

#[test]
fn copies_source_atomically_and_retains_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("user_settings.json"), b"{}").unwrap();

    let outcome = prepare_install_data(
        Some(&source),
        &SharedAppPaths::from_data_root(&data),
    )
    .unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(source.join("user_settings.json").is_file());
    assert!(data.join("user_settings.json").is_file());
    assert!(data.join(".install-data-migration-v1.json").is_file());
    assert!(!data.with_file_name("data.migrating").exists());
}

#[test]
fn existing_data_is_never_overwritten() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::write(source.join("old.txt"), b"old").unwrap();
    fs::write(data.join("new.txt"), b"new").unwrap();

    let outcome = prepare_install_data(
        Some(&source),
        &SharedAppPaths::from_data_root(&data),
    )
    .unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert!(!data.join("old.txt").exists());
}

#[test]
fn no_source_creates_an_empty_data_directory() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");
    let outcome =
        prepare_install_data(None, &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Created);
    assert!(data.is_dir());
}
```

- [ ] **Step 2: Export an empty module and observe failure**

Add `pub mod install_data_migration;` to `lib.rs`. Create the module with only:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDataMigrationOutcome {
    Created,
    ExistingData,
    Migrated,
}
```

Run:

```powershell
cargo test --test install_directory_data
```

Expected: compilation fails because `prepare_install_data` is missing.

- [ ] **Step 3: Implement bounded copying and atomic publication**

Implement:

```rust
use crate::app_paths::SharedAppPaths;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDataMigrationOutcome {
    Created,
    ExistingData,
    Migrated,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationMarker {
    version: u32,
    source_retained: bool,
}

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source).context("无法读取迁移源")?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!("迁移源包含不受支持的链接"));
    }
    if metadata.is_file() {
        fs::copy(source, target).context("无法复制迁移文件")?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(anyhow!("迁移源包含不受支持的文件类型"));
    }
    fs::create_dir(target).context("无法创建迁移目录")?;
    for entry in fs::read_dir(source).context("无法枚举迁移源")? {
        let entry = entry.context("无法读取迁移目录项")?;
        copy_tree(&entry.path(), &target.join(entry.file_name()))?;
    }
    Ok(())
}

pub fn prepare_install_data(
    source: Option<&Path>,
    destination: &SharedAppPaths,
) -> Result<InstallDataMigrationOutcome> {
    if destination.root().exists() {
        if !destination.root().is_dir() {
            return Err(anyhow!("安装目录中的数据路径不是目录"));
        }
        return Ok(InstallDataMigrationOutcome::ExistingData);
    }

    let Some(source) = source.filter(|path| path.is_dir()) else {
        fs::create_dir_all(destination.root()).context("无法创建安装数据目录")?;
        return Ok(InstallDataMigrationOutcome::Created);
    };

    let staging = destination.install_migration_staging();
    if staging.exists() {
        fs::remove_dir_all(&staging).context("无法重置迁移临时目录")?;
    }
    copy_tree(source, &staging)?;
    validate_staged_data(&staging)?;
    let marker = serde_json::to_vec_pretty(&MigrationMarker {
        version: 1,
        source_retained: true,
    })?;
    fs::write(staging.join(".install-data-migration-v1.json"), marker)
        .context("无法写入迁移完成标记")?;
    fs::rename(&staging, destination.root()).context("无法发布安装数据目录")?;
    Ok(InstallDataMigrationOutcome::Migrated)
}

fn validate_staged_data(staging: &Path) -> Result<()> {
    if staging.is_dir() {
        Ok(())
    } else {
        Err(anyhow!("迁移临时目录无效"))
    }
}
```

- [ ] **Step 4: Run the focused tests**

Run:

```powershell
cargo test --test install_directory_data
```

Expected: all three tests pass.

- [ ] **Step 5: Commit**

```powershell
git add apps/owstats-desktop/src-tauri/src/install_data_migration.rs apps/owstats-desktop/src-tauri/tests/install_directory_data.rs apps/owstats-desktop/src-tauri/src/lib.rs
git commit -m "feat: prepare install data atomically"
```

### Task 3: Validate Account Metadata and SQLite

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/install_data_migration.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`

- [ ] **Step 1: Add failing invalid-data tests**

Append:

```rust
#[test]
fn invalid_registry_is_not_published() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("accounts.json"), b"{invalid").unwrap();
    assert!(prepare_install_data(
        Some(&source),
        &SharedAppPaths::from_data_root(&data),
    ).is_err());
    assert!(!data.exists());
    assert!(source.join("accounts.json").is_file());
}

#[test]
fn invalid_sqlite_is_not_published() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let account = source.join("accounts").join("default");
    fs::create_dir_all(&account).unwrap();
    fs::write(
        source.join("accounts.json"),
        br#"{"version":1,"activeAccountId":"default","accounts":[{"accountId":"default","battleTag":"合成账号#0000","createdAt":"2026-01-01T00:00:00Z","lastUsedAt":"2026-01-01T00:00:00Z","credentialState":"saved"}]}"#,
    ).unwrap();
    fs::write(account.join("owstats.sqlite3"), b"not sqlite").unwrap();
    assert!(prepare_install_data(
        Some(&source),
        &SharedAppPaths::from_data_root(&data),
    ).is_err());
    assert!(!data.exists());
}
```

- [ ] **Step 2: Verify both tests fail**

Run:

```powershell
cargo test --test install_directory_data invalid_
```

Expected: both tests fail because the first implementation only checks that staging is a directory.

- [ ] **Step 3: Implement read-only validation**

Replace `validate_staged_data` with:

```rust
use crate::account_store::read_registry;
use rusqlite::{Connection, OpenFlags};

fn validate_staged_data(staging: &Path) -> Result<()> {
    let registry_path = staging.join("accounts.json");
    if !registry_path.exists() {
        return Ok(());
    }
    let registry = read_registry(&registry_path).context("旧账号索引校验失败")?;
    for entry in registry.accounts {
        let account = staging.join("accounts").join(entry.account_id.as_str());
        if !account.is_dir() {
            return Err(anyhow!("旧账号目录校验失败"));
        }
        let database = account.join("owstats.sqlite3");
        if database.exists() {
            let connection = Connection::open_with_flags(
                database,
                OpenFlags::SQLITE_OPEN_READ_ONLY,
            ).context("旧账号数据库校验失败")?;
            let result: String = connection
                .query_row("PRAGMA quick_check", [], |row| row.get(0))
                .context("旧账号数据库完整性校验失败")?;
            if result != "ok" {
                return Err(anyhow!("旧账号数据库完整性校验失败"));
            }
        }
    }
    Ok(())
}
```

Do not log paths, IDs, rows, payloads, or credential contents.

- [ ] **Step 4: Run migration and account regressions**

Run:

```powershell
cargo test --test install_directory_data
cargo test --test phase9_multi_account
```

Expected: both targets pass.

- [ ] **Step 5: Commit**

```powershell
git add apps/owstats-desktop/src-tauri/src/install_data_migration.rs apps/owstats-desktop/src-tauri/tests/install_directory_data.rs
git commit -m "feat: validate install data before publish"
```

### Task 4: Add a Non-GUI Migration Entry Point

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/main.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`

- [ ] **Step 1: Add failing CLI parser tests**

Add a pure parser API:

```rust
pub enum StartupMode {
    Desktop,
    PrepareInstallData {
        source: Option<std::path::PathBuf>,
        destination: std::path::PathBuf,
    },
}
```

Append:

```rust
#[test]
fn migration_cli_accepts_source_and_destination() {
    let mode = owstats_desktop_lib::parse_startup_mode([
        "Watchback.exe",
        "--prepare-install-data",
        r"D:\old\data",
        r"D:\new\data",
    ]).unwrap();
    assert!(matches!(
        mode,
        owstats_desktop_lib::StartupMode::PrepareInstallData { .. }
    ));
}

#[test]
fn migration_cli_rejects_missing_destination() {
    assert!(owstats_desktop_lib::parse_startup_mode([
        "Watchback.exe",
        "--prepare-install-data",
        "-",
    ]).is_err());
}

#[test]
fn migration_execution_rejects_a_destination_outside_the_executable_directory() {
    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join("install").join("Watchback.exe");
    let outside = temp.path().join("outside").join("data");
    assert!(owstats_desktop_lib::run_install_data_mode(
        &executable,
        None,
        &outside,
    ).is_err());
    assert!(!outside.exists());
}
```

The literal `-` means no migration source.

- [ ] **Step 2: Verify parser tests fail**

Run:

```powershell
cargo test --test install_directory_data migration_cli
```

Expected: compilation fails because the parser and enum are absent.

- [ ] **Step 3: Implement the exact CLI contract**

In `lib.rs`:

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum StartupMode {
    Desktop,
    PrepareInstallData {
        source: Option<std::path::PathBuf>,
        destination: std::path::PathBuf,
    },
}

pub fn parse_startup_mode<I, S>(args: I) -> anyhow::Result<StartupMode>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let Some(mode) = args.next() else {
        return Ok(StartupMode::Desktop);
    };
    if mode != "--prepare-install-data" {
        return Err(anyhow::anyhow!("不支持的启动参数"));
    }
    let source = args.next().ok_or_else(|| anyhow::anyhow!("缺少迁移源参数"))?;
    let destination = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("缺少迁移目标参数"))?;
    if args.next().is_some() {
        return Err(anyhow::anyhow!("迁移参数过多"));
    }
    Ok(StartupMode::PrepareInstallData {
        source: if source == "-" { None } else { Some(source.into()) },
        destination: destination.into(),
    })
}

pub fn run_install_data_mode(
    executable: &std::path::Path,
    source: Option<&std::path::Path>,
    destination: &std::path::Path,
) -> anyhow::Result<()> {
    let expected = app_paths::data_root_from_executable(executable)?;
    if destination != expected {
        return Err(anyhow::anyhow!("迁移目标不是当前安装目录的数据目录"));
    }
    let paths = app_paths::SharedAppPaths::from_data_root(destination);
    install_data_migration::prepare_install_data(source, &paths)?;
    Ok(())
}
```

In `main.rs`:

```rust
fn main() {
    match owstats_desktop_lib::parse_startup_mode(std::env::args_os()) {
        Ok(owstats_desktop_lib::StartupMode::Desktop) => {
            owstats_desktop_lib::run();
        }
        Ok(owstats_desktop_lib::StartupMode::PrepareInstallData {
            source,
            destination,
        }) => {
            let executable = match std::env::current_exe() {
                Ok(value) => value,
                Err(_) => std::process::exit(2),
            };
            if owstats_desktop_lib::run_install_data_mode(
                &executable,
                source.as_deref(),
                &destination,
            ).is_err() {
                std::process::exit(2);
            }
        }
        Err(_) => std::process::exit(2),
    }
}
```

This mode must not initialize Tauri, print arguments, or expose paths.

- [ ] **Step 4: Run parser and migration tests**

Run:

```powershell
cargo test --test install_directory_data
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```powershell
git add apps/owstats-desktop/src-tauri/src/main.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/install_directory_data.rs
git commit -m "feat: add installer data migration mode"
```

### Task 5: Make Normal Startup Read Only the Published Data Root

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/install_directory_data.rs`

- [ ] **Step 1: Add failing normal-startup tests**

Append:

```rust
#[test]
fn normal_startup_resolves_published_install_data() {
    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join("安装目录").join("Watchback.exe");
    let data = executable.parent().unwrap().join("data");
    fs::create_dir_all(&data).unwrap();
    let paths = owstats_desktop_lib::published_install_paths(&executable).unwrap();
    assert_eq!(paths.root(), data);
}

#[test]
fn normal_startup_rejects_missing_data() {
    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join("Watchback.exe");
    assert!(owstats_desktop_lib::published_install_paths(&executable).is_err());
}
```

- [ ] **Step 2: Verify tests fail**

Run:

```powershell
cargo test --test install_directory_data normal_startup
```

Expected: compilation fails because `published_install_paths` is missing.

- [ ] **Step 3: Implement published-root verification**

In `lib.rs`:

```rust
pub fn published_install_paths(
    executable: &std::path::Path,
) -> anyhow::Result<app_paths::SharedAppPaths> {
    let root = app_paths::data_root_from_executable(executable)?;
    if !root.is_dir() {
        return Err(anyhow::anyhow!("安装目录中的数据目录不存在"));
    }
    let probe = root.join(format!(".runtime-write-test-{}", uuid::Uuid::new_v4()));
    std::fs::write(&probe, b"ok")
        .map_err(|_| anyhow::anyhow!("安装目录中的数据目录不可写"))?;
    std::fs::remove_file(&probe)
        .map_err(|_| anyhow::anyhow!("无法清理数据目录写入测试"))?;
    Ok(app_paths::SharedAppPaths::from_data_root(root))
}
```

Replace `.setup` path initialization with:

```rust
let executable = std::env::current_exe()
    .map_err(|_| anyhow::anyhow!("无法定位应用程序文件"))?;
let shared = published_install_paths(&executable)?;
```

Remove the desktop runtime call to `app.path().app_data_dir()`.

- [ ] **Step 4: Run focused regressions**

Run:

```powershell
cargo test --test install_directory_data
cargo test --test raw_runtime_data
cargo test --test phase9_multi_account
```

Expected: all targets pass.

- [ ] **Step 5: Commit**

```powershell
git add apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/install_directory_data.rs
git commit -m "feat: open published install data on startup"
```

### Task 6: Configure NSIS-Only Packaging and Migration Hooks

**Files:**
- Create: `apps/owstats-desktop/src-tauri/windows/installer-hooks.nsh`
- Create: `apps/owstats-desktop/scripts/installDirectoryData.test.ts`
- Modify: `apps/owstats-desktop/src-tauri/tauri.conf.json`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Write a failing installer contract**

Create:

```ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const config = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);
assert.equal(config.bundle.targets, "nsis");
assert.equal(config.bundle.windows.nsis.installMode, "both");
assert.equal(
  config.bundle.windows.nsis.installerHooks,
  "./windows/installer-hooks.nsh",
);

const hooks = readFileSync(
  new URL("../src-tauri/windows/installer-hooks.nsh", import.meta.url),
  "utf8",
);
for (const macro of ["NSIS_HOOK_POSTINSTALL", "NSIS_HOOK_PREUNINSTALL"]) {
  assert.match(hooks, new RegExp(`!macro ${macro}`));
}
assert.match(hooks, /--prepare-install-data/);
assert.match(hooks, /InstallLocation/);
assert.match(hooks, /icacls/i);
assert.match(hooks, /\$INSTDIR\\data/);
assert.match(hooks, /\/SD IDNO/);
assert.doesNotMatch(hooks, /grant[^\r\n]*"\$INSTDIR"/i);
```

Append `&& tsx scripts/installDirectoryData.test.ts` to `test:resources`.

- [ ] **Step 2: Verify the contract fails**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/installDirectoryData.test.ts
```

Expected: FAIL because the current target is `all` and hooks are absent.

- [ ] **Step 3: Configure NSIS only**

Merge without reverting current brand/product edits:

```json
{
  "bundle": {
    "active": true,
    "targets": "nsis",
    "icon": ["icons/icon.ico"],
    "windows": {
      "nsis": {
        "installMode": "both",
        "installerHooks": "./windows/installer-hooks.nsh",
        "languages": ["SimpChinese"]
      }
    }
  }
}
```

- [ ] **Step 4: Implement installer migration, ACL, and uninstall choice**

Create:

```nsh
!macro NSIS_HOOK_POSTINSTALL
  ReadRegStr $0 SHCTX "Software\local.owstats.desktop" "InstallLocation"
  StrCpy $1 "-"

  ${If} $0 != ""
  ${AndIf} $0 != "$INSTDIR"
  ${AndIf} ${FileExists} "$0\data\*.*"
    StrCpy $1 "$0\data"
  ${ElseIf} ${FileExists} "$APPDATA\local.owstats.desktop\*.*"
    StrCpy $1 "$APPDATA\local.owstats.desktop"
  ${EndIf}

  nsExec::ExecToStack '"$INSTDIR\${MAINBINARYNAME}.exe" --prepare-install-data "$1" "$INSTDIR\data"'
  Pop $2
  Pop $3
  ${If} $2 != 0
    MessageBox MB_OK|MB_ICONSTOP "无法迁移或创建安装目录数据。安装将停止。"
    Abort
  ${EndIf}

  nsExec::ExecToStack '"$SYSDIR\icacls.exe" "$INSTDIR\data" /inheritance:e /grant:r "$USERNAME:(OI)(CI)M"'
  Pop $2
  Pop $3
  ${If} $2 != 0
    MessageBox MB_OK|MB_ICONSTOP "无法配置数据目录写权限。安装将停止。"
    Abort
  ${EndIf}

  FileOpen $2 "$INSTDIR\data\.write-test" w
  ${IfThen} ${Errors} ${|}
    MessageBox MB_OK|MB_ICONSTOP "安装目录中的数据目录不可写。"
    Abort
  ${|}
  FileWrite $2 "ok"
  FileClose $2
  Delete "$INSTDIR\data\.write-test"
  WriteRegStr SHCTX "Software\local.owstats.desktop" "InstallLocation" "$INSTDIR"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  MessageBox MB_YESNO|MB_DEFBUTTON2|MB_ICONEXCLAMATION \
    "是否同时删除本地数据？这将删除账号凭据、数据库、赛季数据、备份和浏览器登录状态。$\r$\n$\r$\n默认选择“否”以保留数据。" \
    /SD IDNO IDYES delete_local_data IDNO keep_local_data
  delete_local_data:
    RMDir /r "$INSTDIR\data"
  keep_local_data:
!macroend
```

Use the macros supported by the resolved Tauri 2 installer. Do not vendor the full upstream NSIS template.

- [ ] **Step 5: Run the installer contract**

Run:

```powershell
npx tsx scripts/installDirectoryData.test.ts
npm run test:resources
```

Expected: both commands pass.

- [ ] **Step 6: Commit**

```powershell
git add apps/owstats-desktop/src-tauri/windows/installer-hooks.nsh apps/owstats-desktop/scripts/installDirectoryData.test.ts apps/owstats-desktop/src-tauri/tauri.conf.json apps/owstats-desktop/package.json
git commit -m "build: migrate install data with NSIS"
```

### Task 7: Lock the Product Boundary in Verification and Docs

**Files:**
- Modify: `scripts/verify.ps1`
- Modify: `README.md`
- Modify: `docs/maintenance.md`
- Modify: `docs/windows-release.md`

- [ ] **Step 1: Add verification rules**

Add:

```powershell
$runtimeAppData = @(rg -n 'app_data_dir\(\)' `
  'apps/owstats-desktop/src-tauri/src' -g '!bin/**')
if ($runtimeAppData.Count -gt 0) {
  throw '桌面业务路径不得从 app_data_dir 派生'
}

$tauriConfig = Get-Content -Raw `
  'apps/owstats-desktop/src-tauri/tauri.conf.json' | ConvertFrom-Json
if ($tauriConfig.bundle.targets -ne 'nsis') {
  throw 'Windows 发布目标必须仅为 NSIS'
}
```

- [ ] **Step 2: Update documentation with exact boundaries**

README:

```markdown
## 数据目录

Windows 安装版把 OWStatsLocal 自有的可写业务数据保存在安装目录下的 `data\`。静态图片和字典随应用打包，不复制到 `data\`。Windows、WebView2 和安装器仍可能使用系统管理的临时目录或运行时缓存。
```

Maintenance:

```markdown
- Tauri 桌面运行路径统一由 `SharedAppPaths` 从当前可执行文件旁的 `data` 派生。旧 AppData 只允许由 NSIS 的专用迁移入口作为只读迁移源；正常桌面运行不得读写 AppData。
```

Windows release:

```markdown
- `bundle.targets` 必须为 `nsis`，不发布 MSI。
- NSIS 在授予普通用户权限前调用非 GUI 迁移入口。
- 全新安装、旧版迁移、路径变更、覆盖升级、默认保留卸载和显式删除均使用合成数据验收。
```

- [ ] **Step 3: Run documentation/resource verification**

Run:

```powershell
cd apps\owstats-desktop
npm run test:resources
cd ..\..
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -ChangedOnly
```

Expected: both pass unless verification reports an unrelated pre-existing working-tree change; preserve and report such changes.

- [ ] **Step 4: Commit**

```powershell
git add scripts/verify.ps1 README.md docs/maintenance.md docs/windows-release.md
git commit -m "docs: define install-directory data boundary"
```

### Task 8: Full Verification and Windows Acceptance

**Files:**
- Verify all Task 1–7 files.
- Use synthetic data only.

- [ ] **Step 1: Run formatting and focused tests**

```powershell
git diff --check
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --test install_directory_data
cargo test --test phase9_multi_account
cargo test --test raw_runtime_data
```

Expected: every command exits 0.

- [ ] **Step 2: Run frontend verification**

```powershell
cd ..
npm test
npm run build
```

Expected: both commands exit 0; dist scanning finds no development-machine absolute path.

- [ ] **Step 3: Run full backend tests without concurrent Cargo builds**

```powershell
cd src-tauri
cargo test --tests
```

Expected: all targets pass. Allow 15–20 minutes. On outer timeout, inspect `Get-Process cargo,rustc -ErrorAction SilentlyContinue` and wait for the original process rather than starting another build.

- [ ] **Step 4: Build the NSIS installer**

```powershell
cd ..
npm run tauri build
```

Expected:

- `src-tauri\target\release\bundle\nsis\` contains `*-setup.exe`.
- This build produces no MSI.
- Installed `/assets/...` URLs work.

- [ ] **Step 5: Validate install and ACL behavior in a disposable Windows VM**

Test:

```text
C:\Program Files\Watchback
D:\应用程序\Watchback
```

For each:

1. Install with a normal interactive user.
2. Confirm that user can create/delete a probe inside `data`.
3. Confirm that user cannot replace the application executable.
4. Confirm settings, browser profile, account index, synthetic SQLite, seasons, diagnostics, and backups stay under `data`.
5. Confirm clean install creates no business data under `%APPDATA%\local.owstats.desktop`.

If `installMode: both` elevation makes `$USERNAME` resolve to a different account, stop release validation and replace the ACL subject with the original interactive user's SID; do not broaden write access to the installation root.

- [ ] **Step 6: Validate migration and upgrade with synthetic data**

1. Install the old build and create synthetic data only.
2. Upgrade in place; confirm `data` remains.
3. Choose a different install directory; confirm the old install `data` is copied, validated, and the old copy remains.
4. Populate synthetic `%APPDATA%\local.owstats.desktop`; confirm it migrates when no prior install data exists.
5. Corrupt a synthetic SQLite source; confirm installation reports a Chinese error and does not publish `data`.
6. Interrupt copying before publish; rerun installation and confirm safe retry.

- [ ] **Step 7: Validate uninstall choices**

1. Uninstall with default “否”; confirm `data` remains and program/static files are removed.
2. Reinstall, then uninstall with “是”; confirm only exact `<install>\data` is removed.
3. Confirm the old AppData source and old-install migration source are never deleted.

- [ ] **Step 8: Run final repository verification**

```powershell
cd D:\Codex\OW_data\owstats-local
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
git diff --check
git status --short
```

Expected: verification passes and status contains only intentional feature files plus explicitly preserved pre-existing user changes. Report actual commands, results, unverified items, and confirm that no real private data was accessed.
