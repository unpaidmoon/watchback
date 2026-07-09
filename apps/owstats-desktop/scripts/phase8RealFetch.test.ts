import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";

const root = process.cwd();
const onboarding = readFileSync(join(root, "src/pages/OnboardingPage.tsx"), "utf8");
const api = readFileSync(join(root, "src/shared/api.ts"), "utf8");
const types = readFileSync(join(root, "src/shared/types.ts"), "utf8");

assert.match(types, /PlayerTargetStatus/, "types should define PlayerTargetStatus");
assert.match(api, /get_player_target_status/, "api should expose player target status command");
assert.match(api, /clear_player_target/, "api should expose clear player target command");
assert.match(api, /bind_player_target/, "api should expose bind player target command");
assert.match(onboarding, /目标玩家/, "command center should show target player area");
assert.match(onboarding, /请先绑定目标玩家/, "sync cards should explain missing target player");
assert.match(onboarding, /解析并绑定/, "target player UI should bind BattleTag");
assert.match(onboarding, /玩家名#12345/, "target player input should show Chinese placeholder example");
assert.doesNotMatch(onboarding, /同步全部/, "command center must not add sync-all");
assert.doesNotMatch(onboarding, /dump_season_json/, "dump command must not be wired into UI");
