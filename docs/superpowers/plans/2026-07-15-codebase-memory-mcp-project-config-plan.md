# Codebase Memory MCP Project Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Install Codebase Memory MCP v0.9.0 for Codex, restrict it to `D:\Codex\OW_data\owstats-local`, refresh the project graph, and verify the privacy boundary.

**Architecture:** Codex launches the installed STDIO server from its user configuration and injects `CBM_ALLOWED_ROOT` plus `CBM_INDEX_SUPERVISOR=0` into only that server process. Disabling the v0.9.0 index supervisor ensures the root check runs before indexing; the repository keeps its existing `.cbmignore`, graph data remains in the tool's local cache, and no machine-specific runtime path is added to tracked application files.

**Tech Stack:** Codex CLI MCP configuration, PowerShell, TOML, `codebase-memory-mcp 0.9.0`, persistent SQLite knowledge graph.

---

### Task 1: Preserve and validate the current machine configuration

**Files:**
- Read: `C:\Users\cyb\.codex\config.toml`
- Create: `C:\Users\cyb\.codex\config.toml.pre-codebase-memory-mcp.bak`
- Read: `D:\Codex\OW_data\owstats-local\.cbmignore`

- [ ] **Step 1: Confirm the working tree and installed binary**

Run:

```powershell
git status --short
& 'C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe' --version
codex mcp list
```

Expected: the pre-existing user changes remain visible, the original binary reports `codebase-memory-mcp 0.8.1`, and no MCP server named `codebase-memory-mcp` is registered.

- [ ] **Step 2: Back up the Codex configuration without overwriting an older backup**

Run:

```powershell
$source = 'C:\Users\cyb\.codex\config.toml'
$backup = 'C:\Users\cyb\.codex\config.toml.pre-codebase-memory-mcp.bak'
if (Test-Path -LiteralPath $backup) {
    throw "Backup already exists: $backup"
}
Copy-Item -LiteralPath $source -Destination $backup
Get-FileHash -Algorithm SHA256 -LiteralPath $source, $backup
```

Expected: both SHA-256 hashes are identical.

- [ ] **Step 3: Confirm project exclusions before indexing**

Run:

```powershell
Get-Content -LiteralPath 'D:\Codex\OW_data\owstats-local\.cbmignore'
git check-ignore -v -- 'data/.dashen_credentials.json'
```

Expected: `.cbmignore` excludes database, SQLite, log, and `.codebase-memory/` artifacts; Git reports that `.gitignore` excludes `data/.dashen_credentials.json`. Do not open the credentials file.

- [ ] **Step 4: Install the checksum-verified v0.9.0 standard binary without configuring other clients**

Run:

```powershell
$installer = Join-Path $env:TEMP 'codebase-memory-mcp-install-v0.9.0.ps1'
Invoke-WebRequest `
  -Uri 'https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/v0.9.0/install.ps1' `
  -OutFile $installer `
  -UseBasicParsing
Unblock-File -LiteralPath $installer
& $installer --standard --skip-config
Remove-Item -LiteralPath $installer -Force
& 'C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe' --version
```

Expected: the installer prints `Checksum verified`, skips agent configuration, and the binary reports `codebase-memory-mcp 0.9.0`.

### Task 2: Register the project-restricted MCP server

**Files:**
- Modify: `C:\Users\cyb\.codex\config.toml`

- [ ] **Step 1: Add the STDIO MCP server through the supported Codex CLI**

Run:

```powershell
codex mcp add codebase-memory-mcp `
  --env 'CBM_ALLOWED_ROOT=D:\Codex\OW_data\owstats-local' `
  --env 'CBM_INDEX_SUPERVISOR=0' `
  -- 'C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe'
```

Expected: Codex reports that `codebase-memory-mcp` was added successfully.

- [ ] **Step 2: Verify the registered command and environment boundary**

Run:

```powershell
codex mcp get codebase-memory-mcp
codex mcp list
```

Expected: the server is enabled, transport is STDIO, the command is the v0.9.0 executable, `CBM_ALLOWED_ROOT` equals the repository root, and `CBM_INDEX_SUPERVISOR` equals `0`.

- [ ] **Step 3: Verify that Codex configuration remains parseable**

Run:

```powershell
codex mcp list | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Codex rejected config.toml' }
```

Expected: exit code `0`.

### Task 3: Prove the directory restriction before refreshing the graph

**Files:**
- Read: `C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe`

- [ ] **Step 1: Attempt an out-of-root index operation with the configured environment**

Run:

```powershell
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = 'C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe'
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.CreateNoWindow = $true
$psi.EnvironmentVariables['CBM_ALLOWED_ROOT'] = 'D:\Codex\OW_data\owstats-local'
$psi.EnvironmentVariables['CBM_INDEX_SUPERVISOR'] = '0'
$process = New-Object System.Diagnostics.Process
$process.StartInfo = $psi
[void]$process.Start()
$stderrTask = $process.StandardError.ReadToEndAsync()

function Read-McpResponse([int]$id) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        $task = $process.StandardOutput.ReadLineAsync()
        if (-not $task.Wait(3000)) { throw "Timed out waiting for MCP response $id" }
        if (-not $task.Result) { continue }
        $response = $task.Result | ConvertFrom-Json
        if ($response.id -eq $id) { return $response }
    }
    throw "MCP response $id not found"
}

$initialize = @{
    jsonrpc = '2.0'; id = 1; method = 'initialize'
    params = @{
        protocolVersion = '2025-06-18'; capabilities = @{}
        clientInfo = @{ name = 'boundary-probe'; version = '1.0' }
    }
} | ConvertTo-Json -Compress -Depth 8
$process.StandardInput.WriteLine($initialize)
$process.StandardInput.Flush()
$null = Read-McpResponse 1
$process.StandardInput.WriteLine('{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}')
$call = @{
    jsonrpc = '2.0'; id = 2; method = 'tools/call'
    params = @{ name = 'index_repository'; arguments = @{ repo_path = 'C:\Windows\Temp' } }
} | ConvertTo-Json -Compress -Depth 8
$process.StandardInput.WriteLine($call)
$process.StandardInput.Flush()
$result = Read-McpResponse 2
$text = $result.result.content[0].text
if ($text -ne 'repo_path is outside the allowed root') { throw "Boundary failed: $text" }
$process.StandardInput.Close()
if (-not $process.WaitForExit(3000)) { $process.Kill() }
```

Expected: the command refuses the path because it is outside `CBM_ALLOWED_ROOT`; it must not begin indexing `C:\Windows\Temp`.

### Task 4: Refresh and query the OWStatsLocal graph

**Files:**
- Read/index: `D:\Codex\OW_data\owstats-local`
- Update locally: Codebase Memory MCP cache outside the Git repository

- [ ] **Step 1: Explicitly refresh the repository index under the same restriction**

Run:

```powershell
$env:CBM_ALLOWED_ROOT = 'D:\Codex\OW_data\owstats-local'
$env:CBM_INDEX_SUPERVISOR = '0'
try {
    & 'C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe' cli index_repository --repo-path 'D:\Codex\OW_data\owstats-local'
    if ($LASTEXITCODE -ne 0) { throw 'Index refresh failed' }
} finally {
    Remove-Item Env:CBM_ALLOWED_ROOT -ErrorAction SilentlyContinue
    Remove-Item Env:CBM_INDEX_SUPERVISOR -ErrorAction SilentlyContinue
}
```

Expected: JSON reports an indexed or successfully refreshed project, with a non-zero node and edge count.

- [ ] **Step 2: Locate the exact persisted project name**

Run:

```powershell
$binary = 'C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe'
$projects = (& $binary cli list_projects | Where-Object { $_ -like '{*' } | Select-Object -First 1) | ConvertFrom-Json
$project = $projects.projects | Where-Object {
    ($_.root_path -replace '/', '\').TrimEnd('\') -ieq 'D:\Codex\OW_data\owstats-local'
} | Select-Object -First 1
if (-not $project) { throw 'OWStatsLocal graph was not found' }
$project | ConvertTo-Json -Depth 4
```

Expected: the project is `D-Codex-OW_data-owstats-local` and has non-zero `nodes` and `edges`.

- [ ] **Step 3: Query index status and architecture**

Run:

```powershell
$name = $project.name
& $binary cli index_status --project $name
& $binary cli get_architecture --project $name --aspects overview
```

Expected: status is ready/current and architecture returns non-empty language or package data covering the repository's TypeScript/TSX and Rust surfaces.

### Task 5: Final integrity and handoff

**Files:**
- Verify: `C:\Users\cyb\.codex\config.toml`
- Verify: `D:\Codex\OW_data\owstats-local` working tree

- [ ] **Step 1: Verify only the intended tracked plan/design files were introduced by this task**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` succeeds; pre-existing user changes remain, and MCP installation/configuration introduces no new tracked application file.

- [ ] **Step 2: Record the operational handoff**

Report:

```text
Installed binary: codebase-memory-mcp 0.9.0, checksum verified
Codex MCP registration: configured and enabled
Allowed root: D:\Codex\OW_data\owstats-local
Index supervisor: disabled so the allowed-root gate executes before indexing
Project graph: refreshed and queried
Out-of-root probe: rejected
Privacy data touched: no
Restart requirement: restart Codex or open a new task before using the newly registered MCP tools in the UI
```

- [ ] **Step 3: Commit only this implementation plan**

Run:

```powershell
git add -- 'docs/superpowers/plans/2026-07-15-codebase-memory-mcp-project-config-plan.md'
git commit -m 'docs: plan project-scoped codebase memory MCP setup'
```

Expected: one commit containing only the plan document. Machine-local MCP configuration and graph cache are not committed.
