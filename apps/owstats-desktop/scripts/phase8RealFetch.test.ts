import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";

const root = process.cwd();
const onboarding = readFileSync(join(root, "src/pages/OnboardingPage.tsx"), "utf8");
const api = readFileSync(join(root, "src/shared/api.ts"), "utf8");
const commands = readFileSync(join(root, "src-tauri/src/commands.rs"), "utf8");
const lib = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
const models = readFileSync(join(root, "src-tauri/src/models.rs"), "utf8");

assert.match(api, /get_player_target_status/, "product API exposes target status");
assert.match(api, /clear_player_target/, "product API exposes target clearing");
assert.match(onboarding, /当前目标玩家/, "command center shows the bound target player");
assert.match(onboarding, /赛季号/, "command center accepts an explicit season number");
assert.match(onboarding, /syncTargetStrip/, "command center uses a compact target identity strip");
assert.match(onboarding, /syncSeasonWorkbench/, "command center groups season controls into one workbench");
assert.match(onboarding, /syncSeasonPicker/, "command center uses a season picker");
assert.match(onboarding, /syncSecondaryActions/, "command center separates secondary actions");
assert.doesNotMatch(onboarding, /同步全部/, "command center must not add sync-all");
assert.doesNotMatch(onboarding, /FIXED_COMMAND_CENTER_SEASONS/, "command center no longer fixes the season list");

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
