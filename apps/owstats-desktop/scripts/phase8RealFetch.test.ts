import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";
import { isCurrentDashboardRequest, normalizeLoginTargetName } from "../src/pages/OnboardingPage.tsx";

const root = process.cwd();
const onboarding = readFileSync(join(root, "src/pages/OnboardingPage.tsx"), "utf8");
const seasonHealth = readFileSync(join(root, "src/features/command-center/SeasonHealthPanel.tsx"), "utf8");
const seasonWorkbench = readFileSync(join(root, "src/features/command-center/SeasonSyncWorkbench.tsx"), "utf8");
const api = readFileSync(join(root, "src/shared/api.ts"), "utf8");
const types = readFileSync(join(root, "src/shared/types.ts"), "utf8");
const commandCenterStyles = readFileSync(join(root, "src/styles/features/command-center.css"), "utf8");
const commands = readFileSync(join(root, "src-tauri/src/commands.rs"), "utf8");
const lib = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
const models = readFileSync(join(root, "src-tauri/src/models.rs"), "utf8");
const commandCenterBackend = readFileSync(join(root, "src-tauri/src/review/command_center.rs"), "utf8");

function commandBody(source: string, commandName: string) {
  const declaration = new RegExp(`pub (?:async )?fn ${commandName}\\b`).exec(source);
  assert.ok(declaration, `backend exposes ${commandName}`);
  const bodyStart = source.indexOf("{", declaration.index);
  assert.notEqual(bodyStart, -1, `${commandName} has a body`);
  let depth = 0;
  for (let index = bodyStart; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(bodyStart + 1, index);
    }
  }
  assert.fail(`${commandName} body is balanced`);
}

function assertAcquiresBefore(commandName: string, acquireCall: string, firstSideEffect: string) {
  const body = commandBody(commands, commandName);
  const acquireIndex = body.indexOf(acquireCall);
  const sideEffectIndex = body.indexOf(firstSideEffect);
  assert.notEqual(acquireIndex, -1, `${commandName} acquires the shared operation permit`);
  assert.notEqual(sideEffectIndex, -1, `${commandName} keeps its expected first state access or side effect`);
  assert.ok(acquireIndex < sideEffectIndex, `${commandName} acquires before reading or changing account state`);
}

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
assert.doesNotMatch(onboarding, /COMMAND_CENTER_SEASONS/, "command center removes the fixed season array");
assert.match(onboarding, /status\.minimumSyncSeason/, "command center consumes the backend minimum season");
assert.match(onboarding, /<SeasonSyncWorkbench/, "command center renders the dynamic sync workbench");
assert.match(seasonWorkbench, /aria-label="选择目标赛季"/, "command center exposes an accessible target season stepper");
assert.match(seasonWorkbench, /onFullFetch:\s*\(season:\s*number\)\s*=>\s*void/, "workbench exposes full fetch separately");
assert.match(seasonWorkbench, /onQuickUpdate:\s*\(season:\s*number\)\s*=>\s*void/, "workbench exposes row quick update separately");
assert.match(seasonWorkbench, />\s*\{isSelectedFullFetch \? "全量抓取中…" : "全量抓取"\}\s*</, "primary action is full fetch");
assert.match(seasonWorkbench, /backgroundBusy \? "后台处理中" : "更新"/, "ledger rows expose quick update and background state");
assert.match(seasonWorkbench, /runSeasonQuickUpdate\(seasonStatus\.season, onQuickUpdate\)/, "row update acts on that row season through the tested handler");
assert.match(seasonWorkbench, /disabled=\{!credentialReady \|\| !targetSyncAllowed \|\| accountOperationBusy \|\| syncingSeason !== null \|\| backgroundBusy\}/, "row updates include the runtime and background sync gates");
assert.match(seasonWorkbench, /aria-label="减少目标赛季"[\s\S]*?disabled=\{syncingSeason !== null \|\| selectedSeason <= minimumSyncSeason\}/, "the decrement control respects syncing and the minimum");
assert.match(seasonWorkbench, /aria-label="增加目标赛季"[\s\S]*?disabled=\{syncingSeason !== null\}/, "the increment control is disabled during every sync mode");
assert.doesNotMatch(seasonWorkbench, /<button[^>]*className=.*syncedSeasonLedgerRow[\s\S]*?<button/, "season rows must not nest buttons");
assert.match(seasonWorkbench, /<div className="syncedSeasonSelectCell" role="cell">\s*<button\s+className="syncedSeasonSelect"/, "the row owns a season cell before the season selection button");
assert.match(seasonWorkbench, /<div className="syncedSeasonCoverageCell" role="cell">[\s\S]*?<div className="syncedSeasonMatchCountCell" role="cell">[\s\S]*?<div className="syncedSeasonLastSyncCell" role="cell">[\s\S]*?<div className="syncedSeasonUpdateCell" role="cell">/, "the row directly owns coverage, count, last-sync and operation cells");
assert.match(seasonWorkbench, /<div className="syncedSeasonUpdateCell" role="cell">\s*<button\s+className="ghostButton syncedSeasonUpdateButton"/, "the row owns an operation cell before the update button");
assert.doesNotMatch(seasonWorkbench, /<button\s+className="syncedSeasonSelect"(?:(?!<\/button>)[\s\S])*role="cell"/, "native buttons must not own ARIA table cells");
assert.doesNotMatch(seasonWorkbench, /aria-colspan/, "season facts must not be flattened into a spanning interactive cell");
assert.match(models, /pub struct OnboardingStatus\s*\{[\s\S]*?pub minimum_sync_season: i64,[\s\S]*?pub default_season: i64,/, "backend DTO exposes the minimum and default target seasons");
assert.match(commandCenterBackend, /MIN_SYNC_SEASON: i64 = 15;[\s\S]*?DEFAULT_SYNC_SEASON: i64 = 23;/, "backend keeps a lower bound and fallback without a fixed upper bound");
assert.match(commandCenterBackend, /minimum_sync_season: MIN_SYNC_SEASON,[\s\S]*?default_season: DEFAULT_SYNC_SEASON,/, "backend returns the target season bounds");
assert.doesNotMatch(onboarding, /aria-label="赛季号"/, "command center no longer exposes a free-form season number input");
assert.match(onboarding, /syncTargetStrip/, "command center uses a compact target identity strip");
assert.doesNotMatch(onboarding, /commandCenterSeasonSegments/, "fixed season segments are removed");
assert.match(onboarding, /syncSecondaryActions/, "command center separates secondary actions");
assert.doesNotMatch(onboarding, /同步全部/, "command center must not add sync-all");
assert.doesNotMatch(onboarding, /aria-label="BattleTag"|placeholder="BattleTag|setBattleTag|bindPlayerTarget|runBindTarget/, "command center has no manual BattleTag input path");
for (const label of ["切换到此账号", "保留原目标", "重新绑定", "重新识别"]) {
  assert.match(onboarding, new RegExp(label), `command center exposes ${label}`);
}
assert.match(onboarding, /const showDetectedLogin = Boolean\(/, "target comparison is derived independently of the workflow state");
assert.match(onboarding, /targetStatus\?\.currentBnet &&[\s\S]*?targetStatus\.detectedBnet &&[\s\S]*?normalizeLoginTargetName/, "target comparison checks normalized current and detected accounts");
assert.match(onboarding, /\{showDetectedLogin && \([\s\S]*?当前目标[\s\S]*?检测到的登录账号/, "kept mismatches continue to show both account names");
assert.doesNotMatch(onboarding, /state === "switchRequired" && \([\s\S]{0,80}?syncTargetComparison/, "account comparison is not limited to switch confirmation");
assert.match(onboarding, /credentialReady=\{credentialReady\}[\s\S]*?targetSyncAllowed=\{Boolean\(targetStatus\?\.syncAllowed\)\}[\s\S]*?accountOperationBusy=\{accountOperationBusy\}/, "season card receives separate credential, target and operation gates");
assert.match(seasonWorkbench, /!credentialReady \|\| !targetSyncAllowed \|\| accountOperationBusy \|\| syncingSeason !== null/, "sync eligibility combines all independent gates in the dynamic workbench");
assert.match(onboarding, /role=\{error \? "alert" : "status"\}/, "recoverable messages expose an accessible role");
assert.match(onboarding, /aria-live=\{error \? "assertive" : "polite"\}/, "recoverable messages are announced accessibly");
assert.match(commandCenterStyles, /\.syncTargetDecision\s*\{[\s\S]*?display:\s*flex;[\s\S]*?flex-wrap:\s*wrap;[\s\S]*?gap:/, "target decisions wrap without horizontal overflow");
assert.match(commandCenterStyles, /\.syncTargetDecision[^}]*button[\s\S]*?min-height:\s*(?:3[8-9]|[4-9]\d)px;/, "target actions preserve the existing primary-button touch height");
assert.match(commandCenterStyles, /\.syncTargetDecision button\s*\{[\s\S]*?min-height:\s*44px;/, "target actions provide a 44px touch target");

assert.match(types, /type LoginTargetStatus\s*=\s*\{[\s\S]*?state:\s*"ready"\s*\|\s*"switchRequired"\s*\|\s*"mismatchKept"\s*\|\s*"bindFailed"\s*\|\s*"unidentified"\s*\|\s*"verificationRequired"\s*\|\s*string;/, "frontend models every login-target state");
for (const field of ["detectedBnet", "currentBnet", "message", "syncAllowed", "canConfirmSwitch", "canRetry"]) {
  assert.match(types, new RegExp(`${field}\\??:`), `login target status exposes ${field}`);
}
assert.doesNotMatch(types, /type CredentialLoginResult\b/, "obsolete single-account login result is removed");
assert.doesNotMatch(models, /struct CredentialLoginResult\b/, "obsolete backend single-account login result is removed");
assert.match(types, /type ClearPlayerTargetResult\s*=\s*\{[\s\S]*?cleared:\s*boolean;[\s\S]*?target:\s*LoginTargetStatus;/, "clear result returns the replacement login target status");
assert.doesNotMatch(types, /type CredentialStatus\s*=\s*\{[^}]*roleId/s, "public credential status does not expose roleId");
assert.doesNotMatch(types, /type PlayerProfile\s*=\s*\{[^}]*roleId/s, "public player profile does not expose roleId");
const credentialStatusBody = models.match(/pub struct CredentialStatus\s*\{([^}]*)\}/s)?.[1];
const playerProfileBody = models.match(/pub struct PlayerProfile\s*\{([^}]*)\}/s)?.[1];
assert.ok(credentialStatusBody, "Rust CredentialStatus struct exists");
assert.ok(playerProfileBody, "Rust PlayerProfile struct exists");
assert.match(credentialStatusBody, /#\[serde\(skip_serializing\)\]\s*pub role_id:\s*Option<i64>/, "credential roleId is never serialized to the frontend");
assert.match(playerProfileBody, /#\[serde\(skip_serializing\)\]\s*pub role_id:\s*Option<i64>/, "player profile roleId is never serialized to the frontend");

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
assert.match(commands, /add_account[\s\S]*?capture_and_publish_add/, "add account uses the staged capture-and-publish flow");
assert.match(commands, /relogin_account[\s\S]*?capture_and_publish_relogin/, "relogin uses the staged capture-and-publish flow");
assert.match(
  commands,
  /sync_season[\s\S]*?ensure_sync_target_ready[\s\S]*?sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url/,
  "desktop sync validates the login target before starting the fetch",
);

for (const [commandName, firstSideEffect] of [
  ["add_account", "capture_and_publish_add"],
  ["relogin_account", "capture_and_publish_relogin"],
  ["confirm_login_target_switch", "active_account(&accounts)"],
  ["keep_current_player_target", "active_account(&accounts)"],
  ["retry_login_target_binding", "active_account(&accounts)"],
  ["clear_player_target", "active_account(&accounts)"],
] as const) {
  assertAcquiresBefore(commandName, "try_acquire_account_operation()", firstSideEffect);
}
assertAcquiresBefore("sync_season", "try_acquire_sync()", "active_account(&accounts)");
assert.doesNotMatch(
  commandBody(commands, "cancel_dashen_login"),
  /try_acquire_(?:account_operation|sync)/,
  "cancel remains available while a long-running login holds the shared permit",
);
