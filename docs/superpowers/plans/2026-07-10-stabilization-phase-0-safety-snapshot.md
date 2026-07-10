# Stabilization Phase 0 Safety Snapshot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve every valid current worktree change in a recoverable local snapshot, create an external Git bundle, and start a clean `codex/stabilization` branch without changing product behavior.

**Architecture:** The current dirty worktree is committed once on an immutable `codex/stabilization-snapshot` branch after path and secret checks. A Git bundle outside the repository provides a second recovery mechanism. Formal stabilization starts from the pre-snapshot base commit on `codex/stabilization`, where later plans reapply the preserved work as small verified commits.

**Tech Stack:** Git, PowerShell 7/Windows PowerShell, Markdown

---

## Scope

This plan implements only Phase 0 of `docs/superpowers/specs/2026-07-10-personal-stable-sustainable-development-design.md`.

It does not split the current changes, modify application code, run real upstream requests, clean local data, or fix release blockers. Separate plans will cover current-change decomposition, portable resources, privacy, database migrations, modularization, and quality gates.

## Resulting Branches and Artifacts

```text
93c5ec4 or current plan-bearing base
├─ codex/stabilization-snapshot
│  └─ chore: checkpoint pre-stabilization worktree
└─ codex/stabilization
   └─ docs: add stabilization progress tracker

D:\Codex\OW_data\owstats-local-backups\<timestamp>\
├─ baseline.txt
├─ status-before.txt
├─ snapshot-files.txt
├─ snapshot.bundle
└─ restore-check\
```

The exact base commit must be captured at execution time because this plan document may be committed after `93c5ec4`.

### Task 1: Capture the Existing State Without Mutating It

**Files:**
- Read: repository Git metadata
- Create outside repository: `D:\Codex\OW_data\owstats-local-backups\<timestamp>\baseline.txt`
- Create outside repository: `D:\Codex\OW_data\owstats-local-backups\<timestamp>\status-before.txt`

- [ ] **Step 1: Confirm the repository root and active branch**

Run:

```powershell
Set-Location 'D:\Codex\OW_data\owstats-local'
$repoRoot = (git rev-parse --show-toplevel).Trim()
$branch = (git branch --show-current).Trim()
$baseCommit = (git rev-parse HEAD).Trim()
$repoRoot
$branch
$baseCommit
```

Expected:

```text
D:/Codex/OW_data/owstats-local
bundled
<40-character commit hash>
```

Stop if the root or branch differs. Do not switch branches until the worktree has been snapshotted.

- [ ] **Step 2: Create an external evidence directory**

Run:

```powershell
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$backupRoot = Join-Path 'D:\Codex\OW_data\owstats-local-backups' $stamp
New-Item -ItemType Directory -Path $backupRoot -Force | Out-Null
$backupRoot
```

Expected: one new directory outside the repository.

- [ ] **Step 3: Record the baseline commit and branch**

Run:

```powershell
@(
  "repo=$repoRoot"
  "branch=$branch"
  "base_commit=$baseCommit"
  "captured_at=$(Get-Date -Format o)"
) | Set-Content -Encoding utf8 (Join-Path $backupRoot 'baseline.txt')
Get-Content -Encoding utf8 (Join-Path $backupRoot 'baseline.txt')
```

Expected: four non-secret metadata lines.

- [ ] **Step 4: Record the dirty worktree file list**

Run:

```powershell
git status --short --branch | Set-Content -Encoding utf8 (Join-Path $backupRoot 'status-before.txt')
Get-Content -Encoding utf8 (Join-Path $backupRoot 'status-before.txt')
```

Expected: the existing modified and untracked source/test/document/asset paths. Do not continue if credential files, browser profiles, local databases, `data/players`, `docs/reference`, `target`, `dist`, or `node_modules` appear.

- [ ] **Step 5: Confirm no operation changed the worktree**

Run:

```powershell
$before = Get-Content -Raw -Encoding utf8 (Join-Path $backupRoot 'status-before.txt')
$after = (git status --short --branch | Out-String)
if ($before.TrimEnd() -ne $after.TrimEnd()) { throw 'Worktree changed during baseline capture' }
```

Expected: exit code 0 and no output.

### Task 2: Stage and Audit the Raw Snapshot

**Files:**
- Stage: all current non-ignored worktree changes
- Create outside repository: `D:\Codex\OW_data\owstats-local-backups\<timestamp>\snapshot-files.txt`

- [ ] **Step 1: Create the immutable snapshot branch**

Run:

```powershell
git switch -c codex/stabilization-snapshot
git branch --show-current
```

Expected:

```text
codex/stabilization-snapshot
```

If the branch already exists, stop and inspect it; do not delete or overwrite it.

- [ ] **Step 2: Stage all non-ignored current changes**

Run:

```powershell
git add -A
git diff --cached --name-status
```

Expected: the same meaningful source, test, documentation, asset, and tracked deletion entries recorded in `status-before.txt`.

- [ ] **Step 3: Reject forbidden snapshot paths**

Run:

```powershell
$stagedPaths = @(git diff --cached --name-only)
$forbidden = $stagedPaths | Where-Object {
  $_ -match '^(data/|docs/reference/|node_modules/|dist/|target/|apps/.*/src-tauri/target/|data/chrome-dashen-profile/)' -or
  $_ -match '\.(sqlite|sqlite3|db|log)$'
}
if ($forbidden) {
  $forbidden
  throw 'Forbidden local/runtime paths are staged'
}
```

Expected: exit code 0 and no paths.

- [ ] **Step 4: List files that require secret-value review without printing values**

Run:

```powershell
$secretPattern = '-----BEGIN .*PRIVATE KEY-----|(?i)(OW_DASHEN_TOKEN|customerToken|authorization|cookie)\s*[:=]\s*["''][^"'']{16,}'
$flagged = foreach ($path in $stagedPaths) {
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
  $content = git show ":$path" 2>$null
  if ($LASTEXITCODE -ne 0) { continue }
  $content | rg -q --pcre2 $secretPattern -
  if ($LASTEXITCODE -eq 0) { $path }
}
$flagged | Sort-Object -Unique
```

Expected: no production secret files. Test files containing explicit dummy values may be listed; inspect only their staged diff and confirm the values are synthetic. Never print or paste a real secret into the terminal or report.

- [ ] **Step 5: Review flagged files safely**

For each flagged test file, run:

```powershell
git diff --cached --numstat -- '<flagged-path>'
git diff --cached --name-status -- '<flagged-path>'
```

Expected: file metadata only. If a flagged path is not an intentional test fixture or source-level protocol name, stop and unstage it with:

```powershell
git restore --staged -- '<flagged-path>'
```

Do not use `git restore` without `--staged`; the working copy is evidence that must remain intact.

- [ ] **Step 6: Run staged-diff checks and save the file manifest**

Run:

```powershell
git diff --cached --check
git diff --cached --name-status | Set-Content -Encoding utf8 (Join-Path $backupRoot 'snapshot-files.txt')
Get-Content -Encoding utf8 (Join-Path $backupRoot 'snapshot-files.txt')
```

Expected: `git diff --cached --check` exits 0. LF/CRLF conversion warnings are allowed; whitespace errors are not.

### Task 3: Commit and Bundle the Snapshot

**Files:**
- Commit: all reviewed staged files on `codex/stabilization-snapshot`
- Create outside repository: `D:\Codex\OW_data\owstats-local-backups\<timestamp>\snapshot.bundle`

- [ ] **Step 1: Commit the reviewed snapshot**

Run:

```powershell
git commit -m 'chore: checkpoint pre-stabilization worktree'
$snapshotCommit = (git rev-parse HEAD).Trim()
$snapshotCommit
```

Expected: one commit on `codex/stabilization-snapshot` and a 40-character hash.

- [ ] **Step 2: Verify the snapshot commit contains the recorded file set**

Run:

```powershell
$committed = @(git diff-tree --no-commit-id --name-status -r $snapshotCommit)
$recorded = @(Get-Content -Encoding utf8 (Join-Path $backupRoot 'snapshot-files.txt'))
if (Compare-Object $recorded $committed) {
  Compare-Object $recorded $committed
  throw 'Snapshot commit does not match the reviewed manifest'
}
```

Expected: exit code 0 and no output.

- [ ] **Step 3: Create a portable Git bundle**

Run:

```powershell
$bundlePath = Join-Path $backupRoot 'snapshot.bundle'
git bundle create $bundlePath codex/stabilization-snapshot
git bundle verify $bundlePath
```

Expected: bundle verification reports `codex/stabilization-snapshot` and completes successfully.

- [ ] **Step 4: Confirm the snapshot branch is clean**

Run:

```powershell
git status --short
```

Expected: no output. If files remain, stop; do not switch branches until every intentional worktree change is either committed or explicitly documented as excluded.

### Task 4: Prove Recovery and Start the Formal Stabilization Branch

**Files:**
- Create outside repository: `D:\Codex\OW_data\owstats-local-backups\<timestamp>\restore-check\`
- Create branch: `codex/stabilization`

- [ ] **Step 1: Clone the bundle into an external restore-check directory**

Run:

```powershell
$restorePath = Join-Path $backupRoot 'restore-check'
git clone -b codex/stabilization-snapshot $bundlePath $restorePath
```

Expected: clone completes without network access.

- [ ] **Step 2: Verify the restored commit and file manifest**

Run:

```powershell
$restoredCommit = (git -C $restorePath rev-parse HEAD).Trim()
if ($restoredCommit -ne $snapshotCommit) { throw 'Restored snapshot hash differs' }
git -C $restorePath status --short
```

Expected: matching commit hash and no worktree output.

- [ ] **Step 3: Return to the original repository and create the formal branch from the captured base**

Run:

```powershell
Set-Location 'D:\Codex\OW_data\owstats-local'
git switch -c codex/stabilization $baseCommit
git branch --show-current
git status --short
```

Expected:

```text
codex/stabilization
```

The status command must produce no output. The snapshot changes are intentionally absent from this branch and remain recoverable from the snapshot branch and bundle.

### Task 5: Add the Stabilization Progress Tracker

**Files:**
- Create: `docs/stabilization-progress.md`

- [ ] **Step 1: Write the progress tracker**

Create `docs/stabilization-progress.md` with exactly this initial content:

```markdown
# OWStatsLocal 稳定化进度

> 总设计：`docs/superpowers/specs/2026-07-10-personal-stable-sustainable-development-design.md`
>
> 安全快照：`codex/stabilization-snapshot`

| 编号 | 问题 | 优先级 | 状态 | 验证证据 | 提交 |
| --- | --- | ---: | --- | --- | --- |
| STAB-001 | 安全快照与恢复验证 | P0 | 已验证 | Git bundle 恢复哈希一致 | `codex/stabilization-snapshot` |
| STAB-002 | 拆分当前未提交成果 | P0 | 待开始 | 未执行 | — |
| STAB-003 | 可复现资源与运行路径 | P0 | 待开始 | 未执行 | — |
| STAB-004 | 同步身份与敏感数据治理 | P0 | 待开始 | 未执行 | — |
| STAB-005 | 数据库迁移与运行稳定性 | P0 | 待开始 | 未执行 | — |
| STAB-006 | 渐进式模块化 | P1 | 待开始 | 未执行 | — |
| STAB-007 | 统一验证脚本与 CI | P1 | 待开始 | 未执行 | — |

状态只使用：`待开始`、`进行中`、`已验证`、`阻塞`。任意时刻只允许一个任务处于 `进行中`。
```

- [ ] **Step 2: Commit the governance baseline**

Run:

```powershell
git add -- docs/stabilization-progress.md
git diff --cached --check
git diff --cached --name-status
git commit -m 'docs: add stabilization progress tracker'
```

Expected: exactly one staged and committed file: `docs/stabilization-progress.md`.

### Task 6: Final Phase 0 Verification and Handoff

**Files:**
- Read: Git branches, commits, worktree state, external bundle metadata

- [ ] **Step 1: Verify both branches point to the intended commits**

Run:

```powershell
git show-ref --verify refs/heads/codex/stabilization-snapshot
git show-ref --verify refs/heads/codex/stabilization
git merge-base --is-ancestor $baseCommit codex/stabilization-snapshot
git merge-base --is-ancestor $baseCommit codex/stabilization
```

Expected: all commands exit 0.

- [ ] **Step 2: Verify the formal branch is clean**

Run:

```powershell
git branch --show-current
git status --short
```

Expected:

```text
codex/stabilization
```

No status entries may follow.

- [ ] **Step 3: Verify the external recovery artifact again**

Run:

```powershell
git bundle verify $bundlePath
$restoredCommit = (git -C $restorePath rev-parse HEAD).Trim()
if ($restoredCommit -ne $snapshotCommit) { throw 'Final recovery verification failed' }
```

Expected: bundle verification succeeds and the restored hash matches.

- [ ] **Step 4: Record the Phase 0 handoff**

The execution report must include:

```text
Base commit: <hash>
Snapshot branch: codex/stabilization-snapshot
Snapshot commit: <hash>
Formal branch: codex/stabilization
Bundle path: <absolute external path>
Restore verification: passed
Real credentials or player dumps read: no
Formal branch worktree clean: yes
Next task: write the STAB-002 decomposition plan
```

Do not start STAB-002 in the same execution task. Its exact patch and commit boundaries must be planned from the immutable snapshot diff.
