import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

function source(relativePath: string) {
  return readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), "utf8");
}

const onboarding = source("../src/pages/OnboardingPage.tsx");
const frontendApi = source("../src/shared/api.ts");
const frontendTypes = source("../src/shared/types.ts");
const rustCommands = source("../src-tauri/src/commands.rs");
const tauriRegistration = source("../src-tauri/src/lib.rs");
const sharedStyles = source("../src/styles/shared.css");

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
assert.equal(frontendTypes.includes("export type PlayerTargetStatus"), false);
assert.equal(frontendTypes.includes("export type ClearPlayerTargetResult"), false);

for (const obsoleteRustSurface of [
  "pub mod login_target",
  "LoginTargetCoordinator",
  ".manage(login_target_coordinator)",
]) {
  assert.equal(
    tauriRegistration.includes(obsoleteRustSurface),
    false,
    `Tauri runtime must not restore obsolete login-target state: ${obsoleteRustSurface}`,
  );
}

for (const obsoleteSelector of ["syncTargetStrip", "targetRecoveryPanel", "targetRecoveryActions"]) {
  assert.equal(sharedStyles.includes(obsoleteSelector), false, `shared styles must not restore .${obsoleteSelector}`);
}

assert.equal(
  existsSync(fileURLToPath(new URL("../src-tauri/src/login_target.rs", import.meta.url))),
  false,
  "obsolete login_target.rs must remain deleted",
);

console.log("账号同步身份边界测试通过");
