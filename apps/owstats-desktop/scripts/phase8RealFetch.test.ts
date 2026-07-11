import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";

const root = process.cwd();
const onboarding = readFileSync(join(root, "src/pages/OnboardingPage.tsx"), "utf8");
const api = readFileSync(join(root, "src/shared/api.ts"), "utf8");
assert.doesNotMatch(api, /get_player_target_status|clear_player_target|bind_player_target/, "product API should not expose legacy player target commands");
assert.doesNotMatch(onboarding, /BattleTag|绑定目标玩家|解析并绑定|玩家名#12345/, "command center should not expose legacy target binding");
assert.match(onboarding, /当前同步账号/, "command center should show current account sync status");
assert.match(onboarding, /当前登录账号/, "sync cards should use the current login account");
assert.doesNotMatch(onboarding, /同步全部/, "command center must not add sync-all");
assert.doesNotMatch(onboarding, /dump_season_json/, "dump command must not be wired into UI");
