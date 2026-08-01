$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$verify = Join-Path $repoRoot 'scripts\verify.ps1'
$ErrorActionPreference = 'Continue'
$output = & powershell -NoProfile -ExecutionPolicy Bypass -File $verify -ChangedOnly -SkipBuild -SkipClippy -SkipCheckout 2>&1
$verifyExitCode = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
if ($verifyExitCode -ne 0) { throw "verify 快速模式失败：$output" }
foreach ($expected in @('已跳过：前端构建', '已跳过：Rust Clippy', '已跳过：tracked-only checkout')) {
  if (-not ($output -join "`n").Contains($expected)) { throw "缺少跳过摘要：$expected" }
}
