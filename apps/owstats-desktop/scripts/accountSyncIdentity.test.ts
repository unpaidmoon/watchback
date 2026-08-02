import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

function source(relativePath: string) {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), "utf8");
}

const onboarding = source("../src/pages/OnboardingPage.tsx");
const frontendApi = source("../src/shared/api.ts");
const frontendTypes = source("../src/shared/types.ts");
const rustCommands = source("../src-tauri/src/commands.rs");
const tauriRegistration = source("../src-tauri/src/lib.rs");

for (const obsoleteSurface of [
  "TargetRecoveryPanel",
  "syncTargetStrip",
  "LoginTargetStatus",
  "getLoginTargetStatus",
  "recoverLoginTarget",
]) {
  assert.equal(
    onboarding.includes(obsoleteSurface),
    false,
    `command center must not expose obsolete login-target surface: ${obsoleteSurface}`,
  );
}

for (const obsoleteCommand of [
  "get_login_target_status",
  "confirm_login_target_switch",
  "keep_current_player_target",
  "retry_login_target_binding",
  "clear_player_target",
]) {
  assert.equal(frontendApi.includes(obsoleteCommand), false, `frontend IPC must not expose ${obsoleteCommand}`);
  assert.equal(rustCommands.includes(`fn ${obsoleteCommand}`), false, `Rust must not publish ${obsoleteCommand}`);
  assert.equal(tauriRegistration.includes(`commands::${obsoleteCommand}`), false, `Tauri must not register ${obsoleteCommand}`);
}

assert.equal(frontendTypes.includes("export type LoginTargetStatus"), false);

console.log("账号同步身份边界测试通过");
