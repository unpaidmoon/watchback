$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("owstats-checkout-test-" + [guid]::NewGuid().ToString('N'))
$archive = Join-Path $tempRoot 'checkout.tar'
$checkout = Join-Path $tempRoot 'checkout'

try {
  New-Item -ItemType Directory -Force $checkout | Out-Null
  git -C $repoRoot archive --format=tar HEAD -o $archive
  if ($LASTEXITCODE -ne 0) { throw '无法创建 tracked-only archive' }
  tar -xf $archive -C $checkout

  foreach ($required in @(
    'apps/owstats-desktop/package.json',
    'apps/owstats-desktop/package-lock.json',
    'apps/owstats-desktop/src-tauri/Cargo.toml',
    'apps/owstats-desktop/runtime-config/ow-lookups.json'
  )) {
    if (-not (Test-Path (Join-Path $checkout $required))) { throw "archive 缺少必需文件：$required" }
  }
  foreach ($forbidden in @('.git', 'data/.dashen_credentials.json', 'docs/reference', 'chrome-dashen-profile')) {
    if (Test-Path (Join-Path $checkout $forbidden)) { throw "archive 包含禁止内容：$forbidden" }
  }
} finally {
  if ((Test-Path $tempRoot) -and ([IO.Path]::GetFullPath($tempRoot).StartsWith([IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())))) {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force
  }
}
