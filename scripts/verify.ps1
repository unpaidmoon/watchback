[CmdletBinding()]
param(
  [switch]$SkipBuild,
  [switch]$SkipClippy,
  [switch]$SkipCheckout,
  [switch]$ChangedOnly,
  [switch]$TestInstallDirectoryDataBoundary
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$desktopRoot = Join-Path $repoRoot 'apps\owstats-desktop'
$tauriRoot = Join-Path $desktopRoot 'src-tauri'

function Invoke-VerifyStep([string]$Name, [scriptblock]$Action) {
  Write-Host "[验证] 开始：$Name"
  & $Action
  if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "验证失败：$Name（退出码 $LASTEXITCODE）"
  }
  Write-Host "[验证] 通过：$Name"
}

function Skip-VerifyStep([string]$Name) {
  Write-Host "[验证] 已跳过：$Name"
}

function Get-RuntimeAppDataScanExitCode([string]$RuntimeSource) {
  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = 'rg'
  $startInfo.Arguments = '-n "\bapp_data_dir\s*\(" "' + $RuntimeSource.Replace('"', '\"') + '" -g "!bin/**"'
  $startInfo.UseShellExecute = $false
  $startInfo.RedirectStandardOutput = $true
  $startInfo.RedirectStandardError = $true

  $process = [System.Diagnostics.Process]::new()
  $process.StartInfo = $startInfo
  try {
    if (-not $process.Start()) {
      throw '无法启动桌面运行时路径扫描'
    }
    $standardOutput = $process.StandardOutput.ReadToEndAsync()
    $standardError = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    [System.Threading.Tasks.Task]::WaitAll([System.Threading.Tasks.Task[]]@($standardOutput, $standardError))
    return $process.ExitCode
  } finally {
    $process.Dispose()
  }
}

function Assert-RuntimeAppDataScanExitCode([int]$ExitCode) {
  switch ($ExitCode) {
    0 { throw '桌面业务路径不得从 app_data_dir 派生' }
    1 { return }
    default { throw "无法扫描桌面运行时路径（退出码 $ExitCode）" }
  }
}

function Assert-ExpectedFailure([scriptblock]$Action, [string]$ExpectedMessage) {
  $failure = $null
  try {
    & $Action
  } catch {
    $failure = $_
  }
  if ($null -eq $failure) {
    throw "预期验证失败：$ExpectedMessage"
  }
  if ($failure.Exception.Message -notlike "*$ExpectedMessage*") {
    throw "验证失败信息不正确：$($failure.Exception.Message)"
  }
}

if ($TestInstallDirectoryDataBoundary) {
  Assert-RuntimeAppDataScanExitCode 1
  Assert-ExpectedFailure { Assert-RuntimeAppDataScanExitCode 0 } '桌面业务路径不得从 app_data_dir 派生'
  Assert-ExpectedFailure { Assert-RuntimeAppDataScanExitCode 2 } '无法扫描桌面运行时路径（退出码 2）'
  $runtimeSource = Join-Path $tauriRoot 'src'
  Assert-RuntimeAppDataScanExitCode (Get-RuntimeAppDataScanExitCode $runtimeSource)
  Write-Host '[验证] 安装目录数据边界退出码测试通过。'
  exit 0
}

function Invoke-TrackedCheckout {
  $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("owstats-verify-" + [guid]::NewGuid().ToString('N'))
  $archive = Join-Path $tempRoot 'checkout.tar'
  $checkout = Join-Path $tempRoot 'checkout'
  try {
    New-Item -ItemType Directory -Force $checkout | Out-Null
    git -C $repoRoot archive --format=tar HEAD -o $archive
    if ($LASTEXITCODE -ne 0) { throw '无法创建 tracked-only archive' }
    tar -xf $archive -C $checkout
    foreach ($required in @('apps/owstats-desktop/package.json', 'apps/owstats-desktop/package-lock.json', 'apps/owstats-desktop/src-tauri/Cargo.toml', 'apps/owstats-desktop/runtime-config/ow-lookups.json')) {
      if (-not (Test-Path (Join-Path $checkout $required))) { throw "archive 缺少必需文件：$required" }
    }
    foreach ($forbidden in @('.git', 'data/.dashen_credentials.json', 'docs/reference', 'chrome-dashen-profile')) {
      if (Test-Path (Join-Path $checkout $forbidden)) { throw "archive 包含禁止内容：$forbidden" }
    }
    Push-Location (Join-Path $checkout 'apps/owstats-desktop')
    try {
      npm ci
      if ($LASTEXITCODE -ne 0) { throw 'archive npm ci 失败' }
      npm test
      if ($LASTEXITCODE -ne 0) { throw 'archive npm test 失败' }
      npm run build
      if ($LASTEXITCODE -ne 0) { throw 'archive npm run build 失败' }
    } finally { Pop-Location }
    Push-Location (Join-Path $checkout 'apps/owstats-desktop/src-tauri')
    try {
      cargo fmt --check
      if ($LASTEXITCODE -ne 0) { throw 'archive cargo fmt 失败' }
      cargo test --tests
      if ($LASTEXITCODE -ne 0) { throw 'archive cargo test 失败' }
    } finally { Pop-Location }
  } finally {
    if ((Test-Path $tempRoot) -and ([IO.Path]::GetFullPath($tempRoot).StartsWith([IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())))) {
      Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
  }
}

Invoke-VerifyStep '敏感信息与绝对路径扫描' { node (Join-Path $repoRoot 'scripts\verify-sensitive-paths.mjs') --root $repoRoot }
Write-Host '[验证] 开始：安装目录数据发布边界'
try {
  $runtimeSource = Join-Path $tauriRoot 'src'
  Assert-RuntimeAppDataScanExitCode (Get-RuntimeAppDataScanExitCode $runtimeSource)

  $tauriConfig = Get-Content -Raw (Join-Path $tauriRoot 'tauri.conf.json') | ConvertFrom-Json
  if ($tauriConfig.bundle.targets -ne 'nsis') {
    throw 'Windows 发布目标必须仅为 NSIS'
  }
} catch {
  throw "验证失败：安装目录数据发布边界（$($_.Exception.Message)）"
}
Write-Host '[验证] 通过：安装目录数据发布边界'

Push-Location $desktopRoot
try {
  Invoke-VerifyStep '资源准备' { npm run prepare:assets }
  Invoke-VerifyStep '前端分层测试' { npm test }
  if ($SkipBuild) {
    Skip-VerifyStep '前端构建'
  } else {
    Invoke-VerifyStep '前端构建' { npm run build }
    Invoke-VerifyStep '构建产物敏感信息与绝对路径扫描' { node (Join-Path $repoRoot 'scripts\verify-sensitive-paths.mjs') --root $repoRoot --include-dist }
  }
} finally { Pop-Location }

Push-Location $tauriRoot
try {
  Invoke-VerifyStep 'Rust 格式检查' { cargo fmt --check }
  if ($SkipClippy) { Skip-VerifyStep 'Rust Clippy' } else { Invoke-VerifyStep 'Rust Clippy' { cargo clippy --all-targets -- -D warnings } }
  Invoke-VerifyStep 'Rust 全量测试' { cargo test --tests }
  Invoke-VerifyStep 'STAB-005/006 契约测试' { cargo test --test stab005_database_runtime; if ($LASTEXITCODE -eq 0) { cargo test --test stab006_contracts } }
} finally { Pop-Location }

Invoke-VerifyStep 'Git diff 检查' { git -C $repoRoot diff --check }
if ($SkipCheckout -or $ChangedOnly) { Skip-VerifyStep 'tracked-only checkout' } else { Invoke-VerifyStep 'tracked-only checkout' { Invoke-TrackedCheckout } }
Write-Host '[验证] 全部已完成。未执行 tauri build，未读取真实隐私数据。'
