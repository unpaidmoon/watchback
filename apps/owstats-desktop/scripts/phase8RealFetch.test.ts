import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";
import { isCurrentDashboardRequest, normalizeLoginTargetName } from "../src/pages/OnboardingPage.tsx";

const root = process.cwd();
const onboarding = readFileSync(join(root, "src/pages/OnboardingPage.tsx"), "utf8");
const api = readFileSync(join(root, "src/shared/api.ts"), "utf8");
const types = readFileSync(join(root, "src/shared/types.ts"), "utf8");
const commandCenterStyles = readFileSync(join(root, "src/styles/features/command-center.css"), "utf8");
const commands = readFileSync(join(root, "src-tauri/src/commands.rs"), "utf8");
const lib = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
const models = readFileSync(join(root, "src-tauri/src/models.rs"), "utf8");

assert.doesNotMatch(api, /bindPlayerTarget|bind_player_target/, "product API removes arbitrary BattleTag binding");
for (const [wrapper, command] of [
  ["getLoginTargetStatus", "get_login_target_status"],
  ["confirmLoginTargetSwitch", "confirm_login_target_switch"],
  ["keepCurrentPlayerTarget", "keep_current_player_target"],
  ["retryLoginTargetBinding", "retry_login_target_binding"],
]) {
  assert.match(api, new RegExp(`function ${wrapper}\\(\\)[\\s\\S]*?${command}`), `${wrapper} invokes ${command} without a BattleTag argument`);
}
assert.match(api, /clear_player_target/, "product API exposes target clearing");
assert.match(onboarding, /当前目标玩家/, "command center shows the bound target player");
assert.match(onboarding, /赛季号/, "command center accepts an explicit season number");
assert.match(onboarding, /syncTargetStrip/, "command center uses a compact target identity strip");
assert.match(onboarding, /syncSeasonWorkbench/, "command center groups season controls into one workbench");
assert.match(onboarding, /syncSeasonPicker/, "command center uses a season picker");
assert.match(onboarding, /syncSecondaryActions/, "command center separates secondary actions");
assert.doesNotMatch(onboarding, /同步全部/, "command center must not add sync-all");
assert.doesNotMatch(onboarding, /FIXED_COMMAND_CENTER_SEASONS/, "command center no longer fixes the season list");
assert.doesNotMatch(onboarding, /aria-label="BattleTag"|placeholder="BattleTag|setBattleTag|bindPlayerTarget|runBindTarget/, "command center has no manual BattleTag input path");
for (const label of ["切换到此账号", "保留原目标", "重新绑定", "重新识别"]) {
  assert.match(onboarding, new RegExp(label), `command center exposes ${label}`);
}
assert.match(onboarding, /const showDetectedLogin = Boolean\(/, "target comparison is derived independently of the workflow state");
assert.match(onboarding, /targetStatus\?\.currentBnet &&[\s\S]*?targetStatus\.detectedBnet &&[\s\S]*?normalizeLoginTargetName/, "target comparison checks normalized current and detected accounts");
assert.match(onboarding, /\{showDetectedLogin && \([\s\S]*?当前目标[\s\S]*?检测到的登录账号/, "kept mismatches continue to show both account names");
assert.doesNotMatch(onboarding, /state === "switchRequired" && \([\s\S]{0,80}?syncTargetComparison/, "account comparison is not limited to switch confirmation");
assert.match(onboarding, /credentialReady && Boolean\(targetStatus\?\.syncAllowed\)/, "sync eligibility includes backend target authorization");
assert.match(onboarding, /role=\{error \? "alert" : "status"\}/, "recoverable messages expose an accessible role");
assert.match(onboarding, /aria-live=\{error \? "assertive" : "polite"\}/, "recoverable messages are announced accessibly");
assert.match(commandCenterStyles, /\.syncTargetDecision\s*\{[\s\S]*?display:\s*flex;[\s\S]*?flex-wrap:\s*wrap;[\s\S]*?gap:/, "target decisions wrap without horizontal overflow");
assert.match(commandCenterStyles, /\.syncTargetDecision[^}]*button[\s\S]*?min-height:\s*(?:3[8-9]|[4-9]\d)px;/, "target actions preserve the existing primary-button touch height");
assert.match(commandCenterStyles, /\.syncTargetDecision button\s*\{[\s\S]*?min-height:\s*44px;/, "target actions provide a 44px touch target");

assert.match(types, /type LoginTargetStatus\s*=\s*\{[\s\S]*?state:\s*"ready"\s*\|\s*"switchRequired"\s*\|\s*"mismatchKept"\s*\|\s*"bindFailed"\s*\|\s*"unidentified"\s*\|\s*"verificationRequired"\s*\|\s*string;/, "frontend models every login-target state");
for (const field of ["detectedBnet", "currentBnet", "message", "syncAllowed", "canConfirmSwitch", "canRetry"]) {
  assert.match(types, new RegExp(`${field}\\??:`), `login target status exposes ${field}`);
}
assert.match(types, /type CredentialLoginResult\s*=\s*\{[\s\S]*?status:\s*CredentialStatus;[\s\S]*?target:\s*LoginTargetStatus;/, "login result returns credential and target status together");
assert.match(types, /type ClearPlayerTargetResult\s*=\s*\{[\s\S]*?cleared:\s*boolean;[\s\S]*?target:\s*LoginTargetStatus;/, "clear result returns the replacement login target status");

assert.equal(normalizeLoginTargetName(" Alice＃1234 "), "alice#1234", "frontend target normalization matches backend hash and case handling");
assert.notEqual(
  normalizeLoginTargetName("Ａｌｉｃｅ#1234"),
  normalizeLoginTargetName("Alice#1234"),
  "frontend target normalization must not fold full-width letters",
);
assert.equal(isCurrentDashboardRequest(2, 2, true), true, "latest mounted dashboard request may update state");
assert.equal(isCurrentDashboardRequest(1, 2, true), false, "stale dashboard request cannot update state");
assert.equal(isCurrentDashboardRequest(2, 2, false), false, "unmounted dashboard request cannot update state");

assert.doesNotMatch(commands, /bind_player_target/, "backend must not expose arbitrary BattleTag binding");
assert.doesNotMatch(lib, /commands::bind_player_target/, "handler must not register arbitrary BattleTag binding");
assert.doesNotMatch(models, /BindPlayerTarget(?:Request|Result)/, "backend models remove arbitrary BattleTag payloads");
for (const command of [
  "get_login_target_status",
  "confirm_login_target_switch",
  "keep_current_player_target",
  "retry_login_target_binding",
]) {
  assert.match(commands, new RegExp(`fn ${command}\\b`), `backend exposes safe command ${command}`);
  assert.match(lib, new RegExp(`commands::${command}\\b`), `handler registers safe command ${command}`);
}
assert.match(lib, /manage\(login_target::LoginTargetCoordinator::default\(\)\)/, "login target coordinator is managed state");
assert.match(
  commands,
  /start_dashen_login[\s\S]*?coordinator\.clear\(\)[\s\S]*?start_dashen_login_capture[\s\S]*?reconcile_detected_target[\s\S]*?CredentialLoginResult/,
  "new login clears the old round and reconciles detected target",
);
assert.match(
  commands,
  /sync_season[\s\S]*?ensure_sync_target_ready[\s\S]*?sync_season_with_credentials_and_target_paths_output_dir_base_url/,
  "desktop sync validates the login target before starting the fetch",
);
