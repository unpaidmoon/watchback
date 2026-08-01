import assert from "node:assert/strict";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

import { recoverLoginTarget } from "../src/shared/api";
import type { LoginTargetStatus } from "../src/shared/types";

Object.assign(globalThis, { window: globalThis });

const target: LoginTargetStatus = {
  state: "ready",
  detectedBnet: "Fixture#1234",
  currentBnet: "Fixture#1234",
  message: "目标已更新",
  syncAllowed: true,
  canConfirmSwitch: false,
  canRetry: false,
};
const calls: string[] = [];

mockIPC((command) => {
  calls.push(command);
  return command === "clear_player_target" ? { cleared: true, target } : target;
});

for (const [action, command] of [
  ["confirm", "confirm_login_target_switch"],
  ["keep", "keep_current_player_target"],
  ["retry", "retry_login_target_binding"],
  ["clear", "clear_player_target"],
] as const) {
  assert.deepEqual(await recoverLoginTarget(action), target, `${action} returns the refreshed target status`);
  assert.equal(calls.at(-1), command, `${action} invokes the matching desktop command`);
}
assert.deepEqual(calls, [
  "confirm_login_target_switch",
  "keep_current_player_target",
  "retry_login_target_binding",
  "clear_player_target",
]);

clearMocks();
mockIPC((command) => {
  throw new Error(`synthetic failure: ${command}`);
});
await assert.rejects(
  recoverLoginTarget("retry"),
  /synthetic failure: retry_login_target_binding/,
  "desktop failures propagate to the page recovery handler",
);
clearMocks();
