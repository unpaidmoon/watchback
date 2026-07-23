import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const detailPageUrl = new URL("../src/pages/MatchDetailPage.tsx", import.meta.url);

assert.doesNotMatch(appSource, /MatchDetailPage/);
assert.doesNotMatch(appSource, /path="\/matches\/:matchId"/);
assert.equal(existsSync(detailPageUrl), false, "standalone match detail page should be removed");
