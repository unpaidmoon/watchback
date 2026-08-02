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
  $startInfo.Arguments = '-n "\bapp_data_dir\s*\(" "' + $RuntimeSource.Replace('"', '\"') + '" -g "!**/bin/**"'
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
  $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("owstats-verify-rg-" + [guid]::NewGuid().ToString('N'))
  try {
    $sourceRoot = Join-Path $tempRoot 'src'
    $sourceFile = Join-Path $sourceRoot 'runtime.rs'
    $binFile = Join-Path $sourceRoot 'bin\developer.rs'
    New-Item -ItemType Directory -Force (Split-Path -Parent $binFile) | Out-Null

    [System.IO.File]::WriteAllText($sourceFile, 'app_data_dir(')
    $matchedExitCode = Get-RuntimeAppDataScanExitCode $sourceRoot
    if ($matchedExitCode -ne 0) { throw "普通源码匹配应返回 0，实际为 $matchedExitCode" }
    Assert-ExpectedFailure { Assert-RuntimeAppDataScanExitCode $matchedExitCode } '桌面业务路径不得从 app_data_dir 派生'

    Remove-Item -LiteralPath $sourceFile -Force
    $noMatchExitCode = Get-RuntimeAppDataScanExitCode $sourceRoot
    if ($noMatchExitCode -ne 1) { throw "普通源码无匹配应返回 1，实际为 $noMatchExitCode" }
    Assert-RuntimeAppDataScanExitCode $noMatchExitCode

    [System.IO.File]::WriteAllText($binFile, 'app_data_dir(')
    $binOnlyExitCode = Get-RuntimeAppDataScanExitCode $sourceRoot
    if ($binOnlyExitCode -ne 1) { throw "开发 bin 应被排除并返回 1，实际为 $binOnlyExitCode" }
    Assert-RuntimeAppDataScanExitCode $binOnlyExitCode

    $missingExitCode = Get-RuntimeAppDataScanExitCode (Join-Path $tempRoot 'missing')
    if ($missingExitCode -in @(0, 1)) { throw "不存在目录应返回扫描错误，实际为 $missingExitCode" }
    Assert-ExpectedFailure { Assert-RuntimeAppDataScanExitCode $missingExitCode } "无法扫描桌面运行时路径（退出码 $missingExitCode）"

    $runtimeSource = Join-Path $tauriRoot 'src'
    Assert-RuntimeAppDataScanExitCode (Get-RuntimeAppDataScanExitCode $runtimeSource)
    $tauriConfig = Get-Content -Raw (Join-Path $tauriRoot 'tauri.conf.json') | ConvertFrom-Json
    if ($tauriConfig.bundle.targets -ne 'nsis') { throw 'Windows 发布目标必须仅为 NSIS' }
    Write-Host '[验证] 安装目录数据边界退出码测试通过。'
  } finally {
    $tempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    $resolvedTempRoot = [System.IO.Path]::GetFullPath($tempRoot)
    if ((Test-Path $tempRoot) -and $resolvedTempRoot.StartsWith($tempBase, [System.StringComparison]::OrdinalIgnoreCase)) {
      Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
  }
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
      node scripts/auditDependencies.mjs
      if ($LASTEXITCODE -ne 0) { throw 'archive 生产依赖安全审计失败' }
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
Invoke-VerifyStep '验证流程契约' { node (Join-Path $repoRoot 'scripts\verify-workflow.test.mjs') }
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
  Invoke-VerifyStep '生产依赖安全审计' { node scripts/auditDependencies.mjs }
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
} finally { Pop-Location }

Invoke-VerifyStep 'Git diff 检查' { git -C $repoRoot diff --check }
if ($SkipCheckout -or $ChangedOnly) { Skip-VerifyStep 'tracked-only checkout' } else { Invoke-VerifyStep 'tracked-only checkout' { Invoke-TrackedCheckout } }
Write-Host '[验证] 全部已完成。未执行 tauri build，未读取真实隐私数据。'
