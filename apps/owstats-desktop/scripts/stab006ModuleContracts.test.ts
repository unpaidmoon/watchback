import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { filtersFromParams, paramsFromFilters, validateFilters } from "../src/features/matches/matchFilters.ts";
import { matchFiltersKey } from "../src/shared/matchesPageCache.ts";

const roundTrip = filtersFromParams(new URLSearchParams("season=22&hero=ana&startDate=2026-07-01&endDate=2026-07-31"));
assert.deepEqual(roundTrip, { season: 22, hero: "ana", dateFrom: "2026-07-01", dateTo: "2026-07-31" });
assert.equal(paramsFromFilters(roundTrip, "match-1").toString(), "matchId=match-1&season=22&hero=ana&startDate=2026-07-01&endDate=2026-07-31");
assert.equal(matchFiltersKey({ hero: "ana", season: 22 }), matchFiltersKey({ season: 22, hero: "ana" }));
assert.deepEqual(
  validateFilters(
    { season: 22, hero: "missing", primaryTag: "invalid" },
    { seasons: [22], heroes: [], maps: [], results: [], modes: [] },
  ),
  { season: 22 },
);

const root = fileURLToPath(new URL("../src/", import.meta.url));
const styleEntry = readFileSync(join(root, "styles.css"), "utf8");
for (const modulePath of [
  "./styles/tokens.css",
  "./styles/base.css",
  "./styles/shell.css",
  "./styles/shared.css",
  "./styles/features/matches.css",
  "./styles/features/command-center.css",
  "./styles/features/review.css",
  "./styles/features/ranking.css",
  "./styles/features/friends.css",
]) {
  assert.match(styleEntry, new RegExp(`@import "${modulePath}"`));
}
const css = [
  "shared.css",
  "tokens.css",
  "base.css",
  "shell.css",
  "features/matches.css",
  "features/command-center.css",
  "features/review.css",
  "features/ranking.css",
  "features/friends.css",
].map((file) => readFileSync(join(root, "styles", file), "utf8")).join("\n");
for (const selector of [".phase6ProductShell", ".phase6MatchWorkbench", ".phase6CommandCenter", ".phase6RankingTable", ".phase6Friend", ".phase6AchievementWall", ".phase7ArenaSelector"]) {
  assert.ok(css.includes(selector), `${selector} should remain available across style modules`);
}
