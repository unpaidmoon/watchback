import assert from "node:assert/strict";
import { renderMatchWorkbenchFixture, type MatchWorkbenchFixtureState } from "./matchWorkbenchVisualFixture";

const states: MatchWorkbenchFixtureState[] = ["wide-normal", "wide-arena", "narrow-list", "narrow-detail"];

for (const state of states) {
  const html = renderMatchWorkbenchFixture(state);
  assert.match(html, /对局记录/);
  assert.doesNotMatch(html, /token|customerToken|authorization|credential|bnetId/i);
  assert.match(html, new RegExp(`data-fixture-state="${state}"`));
}

assert.match(renderMatchWorkbenchFixture("wide-normal"), /aria-label="普通对局详情"/);
assert.match(renderMatchWorkbenchFixture("wide-arena"), /aria-label="角斗对局详情"/);
assert.match(renderMatchWorkbenchFixture("narrow-list"), /data-fixture-state="narrow-list"/);
assert.match(renderMatchWorkbenchFixture("narrow-detail"), /data-fixture-state="narrow-detail"/);
