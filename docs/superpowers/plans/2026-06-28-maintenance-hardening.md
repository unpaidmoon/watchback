# Maintenance Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current working desktop app into a maintainable project baseline by fixing corrupted Chinese text, making release settings explicit, introducing schema migrations, wiring frontend tests, and documenting the maintenance workflow.

**Architecture:** Keep the current Tauri 2 + React + Rust + SQLite shape. Add small guardrails around the existing code instead of broad refactors: text integrity checks live in scripts, release hardening stays in Tauri config, database migration state stays in SQLite, and maintenance workflow stays in docs.

**Tech Stack:** Tauri 2, React, TypeScript, Vite, Rust, rusqlite, PowerShell, SQLite.

---

## File Map

- Modify: `README.md` - restore readable Chinese and add the standard maintenance commands.
- Modify: `docs/windows-release.md` - update release checklist with text-integrity, tests, bundle, and CSP checks.
- Create: `docs/maintenance.md` - explain branch, commit, verification, release, and rollback discipline.
- Create: `apps/owstats-desktop/scripts/checkTextIntegrity.mjs` - fail CI/local checks when mojibake patterns appear in user-facing files.
- Modify: `apps/owstats-desktop/package.json` - add `test`, `test:text`, and `check` scripts.
- Modify: `apps/owstats-desktop/scripts/rankingUtils.test.ts` - restore readable Chinese expected labels.
- Modify: `apps/owstats-desktop/src/**` and `apps/owstats-desktop/src-tauri/src/**` - restore readable Chinese UI/backend DTO messages.
- Modify: `apps/owstats-desktop/src-tauri/tauri.conf.json` - enable bundling, icons, and a constrained CSP.
- Modify: `apps/owstats-desktop/src-tauri/src/db.rs` - add schema versioning and migration functions.
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs` - add migration idempotency coverage.

---

## Task 1: Add Text Integrity Check

**Files:**
- Create: `apps/owstats-desktop/scripts/checkTextIntegrity.mjs`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Create the failing text-integrity script**

Create `apps/owstats-desktop/scripts/checkTextIntegrity.mjs`:

```js
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = process.cwd();
const scanRoots = [
  "README.md",
  "docs",
  "src",
  "scripts",
  "src-tauri/src",
  "src-tauri/tests"
];
const ignoredDirs = new Set(["node_modules", "target", "dist", ".git"]);
const mojibakePatterns = [
  /鏄|璇|鏈|瀵|鐧|鍚|璧|甯|棰|鈥|�/,
  /銆|€\?|涓|绛|姝|妗|鍦|瀛/
];

function walk(path, files = []) {
  const full = join(root, path);
  const stat = statSync(full, { throwIfNoEntry: false });
  if (!stat) return files;
  if (stat.isDirectory()) {
    if (ignoredDirs.has(path.split(/[\\/]/).pop())) return files;
    for (const child of readdirSync(full)) {
      walk(join(path, child), files);
    }
    return files;
  }
  if (/\.(md|ts|tsx|rs|json)$/.test(path)) {
    files.push(full);
  }
  return files;
}

const failures = [];
for (const entry of scanRoots) {
  for (const file of walk(entry)) {
    const text = readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (mojibakePatterns.some((pattern) => pattern.test(line))) {
        failures.push(`${relative(root, file)}:${index + 1}: ${line.trim()}`);
      }
    });
  }
}

if (failures.length > 0) {
  console.error("Mojibake-like text found:");
  console.error(failures.join("\n"));
  process.exit(1);
}
```

- [ ] **Step 2: Wire the script into `package.json`**

Update `apps/owstats-desktop/package.json` scripts:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "test:text": "node scripts/checkTextIntegrity.mjs",
    "test": "tsx scripts/rankingUtils.test.ts && npm run test:text",
    "check": "npm run test && npm run build"
  }
}
```

- [ ] **Step 3: Run and confirm the current failure**

Run:

```powershell
cd apps\owstats-desktop
npm run test:text
```

Expected: FAIL with mojibake locations such as `src/App.tsx`, `src-tauri/src/credentials.rs`, or `scripts/rankingUtils.test.ts`.

- [ ] **Step 4: Commit the guardrail**

```powershell
git add apps/owstats-desktop/package.json apps/owstats-desktop/scripts/checkTextIntegrity.mjs
git commit -m "chore: add text integrity check"
```

---

## Task 2: Restore Readable Chinese Text

**Files:**
- Modify: `README.md`
- Modify: `docs/windows-release.md`
- Modify: `apps/owstats-desktop/scripts/rankingUtils.test.ts`
- Modify: files reported by `npm run test:text`

- [ ] **Step 1: Replace the highest-impact product copy first**

Use these canonical strings:

```text
OWStatsLocal 是一个 Windows 优先的守望先锋本地数据分析桌面应用，主项目位于 `apps/owstats-desktop`。
本地优先的 Overwatch 战绩复盘
首次设置
对局
复盘
排行
分析
同步
```

- [ ] **Step 2: Fix `rankingUtils.test.ts` expected labels**

Replace the corrupted assertions with:

```ts
assertEqual(modeLabel("preset"), "预设/开放", "preset mode label");
assertEqual(modeLabel("stadium"), "体育场", "stadium mode label");
assertEqual(modeLabel("custom"), "custom", "unknown mode label falls back");
```

- [ ] **Step 3: Run the text check**

Run:

```powershell
cd apps\owstats-desktop
npm run test:text
```

Expected: PASS with no mojibake output.

- [ ] **Step 4: Run app checks**

Run:

```powershell
cd apps\owstats-desktop
npm run test
npm run build
cd src-tauri
cargo fmt --check
cargo test --tests
```

Expected: all commands exit 0.

- [ ] **Step 5: Commit restored text**

```powershell
git add README.md docs/windows-release.md apps/owstats-desktop
git commit -m "fix: restore readable Chinese copy"
```

---

## Task 3: Harden Windows Release Configuration

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tauri.conf.json`
- Modify: `docs/windows-release.md`

- [ ] **Step 1: Update Tauri bundle and CSP settings**

Use this `bundle` and `security` shape:

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' asset: https://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 2: Verify the configured icon exists**

Run:

```powershell
Test-Path apps\owstats-desktop\src-tauri\icons\icon.ico
```

Expected: `True`.

- [ ] **Step 3: Run release-adjacent checks**

Run:

```powershell
cd apps\owstats-desktop
npm run build
cd src-tauri
cargo test --tests
npm run tauri build
```

Expected: build artifacts are produced under `apps/owstats-desktop/src-tauri/target/release/bundle`.

- [ ] **Step 4: Commit release config**

```powershell
git add apps/owstats-desktop/src-tauri/tauri.conf.json docs/windows-release.md
git commit -m "chore: harden windows release config"
```

---

## Task 4: Add SQLite Schema Versioning

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/db.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`

- [ ] **Step 1: Add migration constants and runner**

In `db.rs`, add near the imports:

```rust
const CURRENT_SCHEMA_VERSION: i64 = 2;
```

At the end of `initialize_connection`, before `Ok(())`, call:

```rust
run_migrations(conn)?;
```

Add:

```rust
fn run_migrations(conn: &Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current < 1 {
        conn.pragma_update(None, "user_version", 1)?;
    }
    if current < 2 {
        add_column_if_missing(conn, "match_players", "role_type", "TEXT")?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_match_players_role ON match_players(season, is_current_player, role_type)",
            [],
        )?;
        conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;
    }
    Ok(())
}
```

- [ ] **Step 2: Remove duplicate inline migration calls**

Remove the direct `add_column_if_missing(... role_type ...)` and role index creation from `initialize_connection` if they now exist inside `run_migrations`.

- [ ] **Step 3: Add idempotency test**

In `import_baseline.rs`, add:

```rust
#[test]
fn database_initialize_sets_schema_version_idempotently() {
    let db = import_synthetic_season();
    db.initialize().unwrap();
    db.initialize().unwrap();

    let conn = db.connect().unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();

    assert!(version >= 2);
}
```

- [ ] **Step 4: Verify backend**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: all tests pass.

- [ ] **Step 5: Commit migration baseline**

```powershell
git add apps/owstats-desktop/src-tauri/src/db.rs apps/owstats-desktop/src-tauri/tests/import_baseline.rs
git commit -m "chore: add sqlite schema versioning"
```

---

## Task 5: Document Maintenance Workflow

**Files:**
- Create: `docs/maintenance.md`
- Modify: `README.md`

- [ ] **Step 1: Create `docs/maintenance.md`**

```markdown
# Maintenance Guide

## Daily Development

Work in small branches. Keep each commit focused on one reason to change: text repair, release config, database migration, feature code, or tests.

## Required Checks

Backend:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Frontend:

```powershell
cd apps\owstats-desktop
npm run test
npm run build
```

Release:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

## Commit Discipline

- Do not mix product behavior changes with release config changes.
- Do not commit local credentials, browser profiles, raw player dumps, or generated build output.
- Before release, verify `git status --short` only shows intentional files.
- If a migration changes SQLite schema, add or update a test that initializes the database twice.

## Rollback

If a release fails after installation, revert the smallest commit that introduced the failure. Database migrations must be forward-compatible with existing local data.
```

- [ ] **Step 2: Link the guide from `README.md`**

Add:

```markdown
## 维护

长期维护流程见 `docs/maintenance.md`。发布前至少运行后端测试、前端测试、前端构建和 Tauri 打包。
```

- [ ] **Step 3: Verify documentation and checks**

Run:

```powershell
cd apps\owstats-desktop
npm run check
cd src-tauri
cargo fmt --check
cargo test --tests
```

Expected: all commands exit 0.

- [ ] **Step 4: Commit docs**

```powershell
git add README.md docs/maintenance.md
git commit -m "docs: add maintenance workflow"
```

---

## Final Verification

- [ ] Run backend verification:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

- [ ] Run frontend verification:

```powershell
cd apps\owstats-desktop
npm run test
npm run build
```

- [ ] Run release verification on Windows:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

- [ ] Confirm Git state:

```powershell
git status --short --branch
```

Expected: branch is clean after commits, or only intentionally uncommitted files remain.

---

## Rollout Order

1. Text integrity guardrail.
2. Chinese text restoration.
3. Release configuration hardening.
4. SQLite schema versioning.
5. Maintenance documentation.

This order prevents new corrupted text from entering before the cleanup, then stabilizes release and data compatibility.
