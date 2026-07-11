import assert from "node:assert/strict";
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
