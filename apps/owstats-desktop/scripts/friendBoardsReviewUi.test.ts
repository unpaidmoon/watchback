import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("../src/pages/FriendAnalyticsPage.tsx", import.meta.url), "utf8");

assert.equal(source.includes("FriendExplainDialog"), false, "review friend boards should not open a separate explanation dialog");
assert.equal(source.includes("useState<FriendBoardEntry"), false, "review friend boards should not keep clickable friend state");
assert.equal(source.includes("<span>{board.friends.length}</span>"), false, "review friend board headers should not show the returned count");
assert.equal(source.includes("<button type=\"button\" className=\"friendCard phase6FriendCard\""), false, "review friend cards should be static cards, not buttons");
assert.equal(source.includes("<article className=\"friendCard phase6FriendCard\""), true, "review friend cards should render as non-clickable articles");
