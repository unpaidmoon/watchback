import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { phase6NavItems } from "../src/shared/phase6Shell";

const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const hook = readFileSync(new URL("../src/features/player-query/usePlayerQuerySession.ts", import.meta.url), "utf8");
const coordinator = readFileSync(new URL("../src-tauri/src/sync_coordinator.rs", import.meta.url), "utf8");

assert.deepEqual(phase6NavItems.slice(0, 3).map((item) => item.label), ["对局", "查询", "战报"]);
assert.match(app, /path="\/player-query\/\*"/);
assert.match(app, /location\.pathname === "\/player-query\/matches"/);
assert.doesNotMatch(app, /player-query\?[^"\s]*/);
assert.match(app, /if \(location\.pathname === "\/player-query\/matches" && !handoff\)/);
assert.match(app, /<div hidden=\{isMore\}>\s*<PlayerQueryPage/s, "returning from more matches must reveal the same live query page instance");
assert.match(app, /sessionRef\.current = null;[\s\S]*endPlayerQuery/);
assert.match(app, /const navigateRef = useRef\(navigate\)/);
assert.match(app, /navigateRef\.current\("\/player-query", \{ replace: true \}\)/);
assert.match(app, /const refreshRuntime = useCallback\([\s\S]*?\}, \[\]\);/, "internal query routes must not recreate the runtime effect and end the live session");
assert.match(app, /get_operation_status/);
assert.match(app, /credentialState === "ready"/);
assert.match(app, /getUserSettings/);
assert.match(app, /playerQueryModeOrder: \[\.\.\.order\]/);
assert.match(hook, /onSessionChange\?\.\(session\.sessionId\)/);
assert.match(hook, /onSessionChange\?\.\(null\)/);
assert.match(coordinator, /pub fn status\(&self\) -> OperationStatus/);
assert.doesNotMatch(coordinator, /token|customerToken/i);

console.log("player query navigation tests passed");
