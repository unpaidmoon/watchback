# Watchback Product Name Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Watchback the only current product name in user-visible surfaces, current documentation, and descriptive package metadata while preserving every historical compatibility identifier.

**Architecture:** Treat branding and compatibility as separate contracts. A focused integration test will assert the current product name in active surfaces and documents while continuing to assert the intentionally retained credential and Tauri identifiers; document edits will be semantic and limited to present-tense product references.

**Tech Stack:** Markdown, TypeScript/Node assertions, Tauri JSON configuration, Cargo manifest metadata

---

## File map

- `apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts`: executable contract for current Watchback branding and retained compatibility identifiers.
- `AGENTS.md`, `README.md`, `design.md`: root-level active project, product, and visual guidance.
- `docs/current-development-status.md`, `docs/stabilization-progress.md`: active status documents.
- `docs/superpowers/specs/main-thread-log.md`: durable current workflow guide whose product references must use Watchback while literal paths remain unchanged.
- `docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md`: approved brand specification whose goal sentence must describe Watchback as the product.
- `apps/owstats-desktop/src-tauri/Cargo.toml`: user-readable package description and author metadata; package, binary, and crate names remain unchanged.

### Task 1: Extend the Watchback naming contract

**Files:**

- Modify: `apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts`
- Test: `apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts`

- [ ] **Step 1: Add repository-document and Cargo metadata assertions**

Add a helper beside the existing `source` helper:

```ts
function repositorySource(relativePath: string) {
  return readFileSync(new URL(`../../../${relativePath}`, import.meta.url), "utf8");
}
```

Load the current documents and Cargo manifest:

```ts
const agents = repositorySource("AGENTS.md");
const readme = repositorySource("README.md");
const design = repositorySource("design.md");
const currentDevelopment = repositorySource("docs/current-development-status.md");
const stabilization = repositorySource("docs/stabilization-progress.md");
const mainThreadLog = repositorySource("docs/superpowers/specs/main-thread-log.md");
const brandDesign = repositorySource(
  "docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md",
);
const cargoManifest = source("src-tauri/Cargo.toml");
```

Add explicit current-name assertions after the existing UI assertions:

```ts
assert.match(agents, /^# Watchback 协作说明$/m);
assert.match(readme, /^# Watchback$/m);
assert.match(readme, /^Watchback 是一个 Windows 优先的守望先锋本地数据分析桌面应用/m);
assert.match(design, /^# Watchback 设计规范$/m);
assert.match(design, /本文件是 Watchback 新页面与页面重构的视觉控制规范/);
assert.match(currentDevelopment, /^# Watchback 当前开发状态$/m);
assert.match(stabilization, /^# Watchback 稳定化进度$/m);
assert.match(mainThreadLog, /^# Watchback Main Thread Log$/m);
assert.match(mainThreadLog, /durable ledger for the Watchback main-thread workflow/);
assert.match(brandDesign, /为 Watchback 建立正式的产品名称与图标体系/);
assert.match(cargoManifest, /description = "Watchback desktop app"/);
assert.match(cargoManifest, /authors = \["Watchback"\]/);
```

Keep the existing assertions for `local.owstats.desktop` and the `OWStatsLocal` credential target. These are the executable compatibility allowlist.

- [ ] **Step 2: Run the focused test and verify it fails for the current old-name references**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/watchbackProductIntegration.test.ts
```

Expected: FAIL at the first new document or Cargo assertion because those files still use `OWStatsLocal`.

- [ ] **Step 3: Commit the failing contract if commits are explicitly authorized**

Do not commit by default. If the user explicitly authorizes commits:

```powershell
git add apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts
git commit -m "test: define Watchback product naming contract"
```

### Task 2: Rename current product references

**Files:**

- Modify: `AGENTS.md`
- Modify: `README.md`
- Modify: `design.md`
- Modify: `docs/current-development-status.md`
- Modify: `docs/stabilization-progress.md`
- Modify: `docs/superpowers/specs/main-thread-log.md`
- Modify: `docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md`
- Modify: `apps/owstats-desktop/src-tauri/Cargo.toml`
- Test: `apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts`

- [ ] **Step 1: Update root active documentation**

Make these exact present-tense replacements:

```text
# OWStatsLocal 协作说明
→ # Watchback 协作说明

# OWStatsLocal
→ # Watchback

OWStatsLocal 是一个 Windows 优先的守望先锋本地数据分析桌面应用
→ Watchback 是一个 Windows 优先的守望先锋本地数据分析桌面应用

# OWStatsLocal 设计规范
→ # Watchback 设计规范

本文件是 OWStatsLocal 新页面与页面重构的视觉控制规范
→ 本文件是 Watchback 新页面与页面重构的视觉控制规范
```

Do not change `D:\Codex\OW_data\owstats-local`, `apps/owstats-desktop`, command examples, or `default-run`.

- [ ] **Step 2: Update active status and workflow documentation**

Apply these semantic replacements:

```text
# OWStatsLocal 当前开发状态
→ # Watchback 当前开发状态

# OWStatsLocal 稳定化进度
→ # Watchback 稳定化进度

# OWStatsLocal Main Thread Log
→ # Watchback Main Thread Log

OWStatsLocal main-thread workflow
→ Watchback main-thread workflow

OWStatsLocal execution thread
→ Watchback execution thread

OWStatsLocal verification thread
→ Watchback verification thread

OWStatsLocal main thread
→ Watchback main thread

OWStatsLocal task
→ Watchback task
```

Keep every literal project-root and application-directory path unchanged.

- [ ] **Step 3: Correct the approved brand specification**

Change only the current goal sentence:

```text
为 OWStatsLocal 建立正式的产品名称与图标体系
→ 为 Watchback 建立正式的产品名称与图标体系
```

Keep the compatibility entry for the Windows Credential Manager `OWStatsLocal` target unchanged.

- [ ] **Step 4: Update descriptive Cargo metadata**

Change:

```toml
description = "Watchback desktop app"
authors = ["Watchback"]
```

Keep these identifiers unchanged:

```toml
name = "owstats-desktop"
default-run = "owstats-desktop"

[lib]
name = "owstats_desktop_lib"

[[bin]]
name = "owstats-desktop"
```

- [ ] **Step 5: Run the focused contract and verify it passes**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/watchbackProductIntegration.test.ts
```

Expected: PASS with `Watchback product integration contracts passed`.

- [ ] **Step 6: Validate Cargo metadata**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo metadata --no-deps --format-version 1
```

Expected: exit code 0; package name remains `owstats-desktop`, while description and author report Watchback.

- [ ] **Step 7: Commit the product-name cleanup if commits are explicitly authorized**

Do not commit by default. If the user explicitly authorizes commits:

```powershell
git add AGENTS.md README.md design.md docs/current-development-status.md docs/stabilization-progress.md docs/superpowers/specs/main-thread-log.md docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md apps/owstats-desktop/src-tauri/Cargo.toml apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts
git commit -m "docs: standardize current product name as Watchback"
```

### Task 3: Audit retained names and run final verification

**Files:**

- Verify: all modified files
- Verify: remaining tracked occurrences of `OWStatsLocal`, `owstats-local`, and `OWStats`

- [ ] **Step 1: Audit all remaining old-name occurrences**

Run:

```powershell
rg -n -i --hidden --glob '!node_modules/**' --glob '!target/**' --glob '!.git/**' --glob '!data/**' 'OWStatsLocal|owstats-local|OWStats' .
```

Review every remaining category. Accept only:

- real repository or application paths;
- npm/Rust/crate/binary/module identifiers;
- Tauri identifier, database paths, and credential targets;
- historical specs, plans, reports, fixtures, and migration records that describe past state;
- tests that assert retained compatibility identifiers.

Any present-tense user-facing or current-document use must be corrected before continuing.

- [ ] **Step 2: Run frontend verification**

Run:

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: both commands exit 0.

- [ ] **Step 3: Run Rust formatting verification**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
```

Expected: exit code 0.

No Rust behavior changed, so `cargo test --tests` is not required for this metadata-and-document-only change.

- [ ] **Step 4: Check whitespace and final scope**

Run from the repository root:

```powershell
git diff --check
git diff -- AGENTS.md README.md design.md docs/current-development-status.md docs/stabilization-progress.md docs/superpowers/specs/main-thread-log.md docs/superpowers/specs/2026-07-24-watchback-brand-icon-design.md docs/superpowers/specs/2026-07-24-watchback-product-name-cleanup-design.md docs/superpowers/plans/2026-07-24-watchback-product-name-cleanup.md apps/owstats-desktop/src-tauri/Cargo.toml apps/owstats-desktop/scripts/watchbackProductIntegration.test.ts
git status --short
```

Expected:

- `git diff --check` exits 0.
- Only the approved naming, design, plan, test, and metadata files are newly modified by this task.
- No file is staged or committed without explicit user authorization.
- No real credential, private payload, local SQLite database, or user data was read.
