import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const workflow = readFileSync(resolve(root, ".github/workflows/verify.yml"), "utf8");

for (const required of [
  "runs-on: windows-latest",
  "actions/checkout@v4",
  "actions/setup-node@v4",
  "node-version: 24",
  "dtolnay/rust-toolchain@stable",
  "components: rustfmt, clippy",
  "working-directory: apps/owstats-desktop",
  "powershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\verify.ps1",
]) {
  assert.ok(workflow.includes(required), `workflow 缺少：${required}`);
}
assert.doesNotMatch(workflow, /tauri build|OW_DASHEN_TOKEN|credential|chrome-dashen-profile|upload-artifact/i);
