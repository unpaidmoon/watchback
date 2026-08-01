import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const workflow = readFileSync(resolve(root, ".github/workflows/verify.yml"), "utf8");
const verifyScript = readFileSync(resolve(root, "scripts/verify.ps1"), "utf8");

for (const required of [
  "runs-on: windows-latest",
  "actions/checkout@v7",
  "actions/setup-node@v7",
  "node-version: 24",
  "dtolnay/rust-toolchain@stable",
  "components: rustfmt, clippy",
  "working-directory: apps/owstats-desktop",
  "run: ./scripts/verify.ps1 -SkipCheckout",
]) {
  assert.ok(workflow.includes(required), `workflow 缺少：${required}`);
}
assert.doesNotMatch(workflow, /tauri build|OW_DASHEN_TOKEN|credential|chrome-dashen-profile|upload-artifact/i);

for (const required of [
  "Invoke-VerifyStep '前端分层测试' { npm test }",
  "Invoke-VerifyStep '前端构建' { npm run build }",
  "Invoke-VerifyStep 'Rust 全量测试' { cargo test --tests }",
]) {
  assert.ok(verifyScript.includes(required), `verify.ps1 缺少：${required}`);
}

assert.doesNotMatch(
  verifyScript,
  /Invoke-VerifyStep '资源准备'\s*\{\s*npm run prepare:assets\s*\}/,
  "npm test/build 的生命周期已准备资源，verify.ps1 不应再次显式准备",
);
assert.doesNotMatch(
  verifyScript,
  /cargo test --test stab00[56]_/,
  "cargo test --tests 已覆盖 STAB 集成测试，不应重复执行",
);
