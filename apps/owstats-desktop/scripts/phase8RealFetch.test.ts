import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";

const root = process.cwd();
const onboarding = readFileSync(join(root, "src/pages/OnboardingPage.tsx"), "utf8");
const api = readFileSync(join(root, "src/shared/api.ts"), "utf8");

assert.match(api, /get_player_target_status/, "product API exposes target status");
assert.match(api, /clear_player_target/, "product API exposes target clearing");
assert.match(api, /bind_player_target/, "product API exposes BattleTag binding");
assert.match(onboarding, /BattleTag/, "command center exposes BattleTag binding");
assert.match(onboarding, /当前目标玩家/, "command center shows the bound target player");
assert.match(onboarding, /赛季号/, "command center accepts an explicit season number");
assert.match(onboarding, /syncTargetStrip/, "command center uses a compact target identity strip");
assert.match(onboarding, /syncSeasonWorkbench/, "command center groups season controls into one workbench");
assert.match(onboarding, /syncSeasonPicker/, "command center uses a season picker");
assert.match(onboarding, /syncSecondaryActions/, "command center separates secondary actions");
assert.doesNotMatch(onboarding, /同步全部/, "command center must not add sync-all");
assert.doesNotMatch(onboarding, /FIXED_COMMAND_CENTER_SEASONS/, "command center no longer fixes the season list");
