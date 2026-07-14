import assert from "node:assert/strict";
import {
  allVisualFixtures,
  fixtureArenaDetail,
  fixtureCareerOverview,
  fixtureCommandCenter,
  fixtureMatchDetailsById,
  fixtureMatches,
  fixtureNormalDetail,
  fixtureRanking,
  fixtureReview,
  fixtureShell,
  visualInvokeResponses,
} from "./uiVisualAcceptance.fixture.ts";

const serialized = JSON.stringify({
  allVisualFixtures,
  fixtureMatchDetailsById,
  visualInvokeResponses,
});

assert.doesNotMatch(
  serialized,
  /"(?:token|customerToken|cookie|authorization|credential|browser[_-]?profile)"\s*:/i,
  "visual fixtures must not contain secret-bearing field names",
);
assert.doesNotMatch(serialized, /\b\d{16,}\b/, "visual fixtures must not contain raw-looking long numeric identifiers");
assert.doesNotMatch(
  serialized,
  /\/@fs\/|[A-Z]:\\(?:Codex|Users)\\|app[_ -]?data|5479_s22|\.dashen_credentials/i,
  "visual fixtures must not contain developer paths, app data, credentials files, or real dump names",
);
assert.match(serialized, /示例玩家/, "visual fixtures use an explicit anonymous player label");
assert.match(serialized, /普通对局/, "visual fixtures include the normal match branch");
assert.match(serialized, /角斗竞技/, "visual fixtures include the arena match branch");

assert.deepEqual(
  fixtureShell.navigation.map((item) => item.label),
  ["对局", "战报", "生涯 MMR", "排行", "指挥台"],
  "shell fixture covers the exact primary navigation order",
);
assert.equal(fixtureMatches.length, 2, "match fixture contains one normal row and one arena row");
assert.equal(fixtureNormalDetail.arenaDetail, null, "normal detail exercises the non-arena workbench branch");
assert.equal(fixtureArenaDetail.arenaDetail?.rounds.length, 2, "arena detail exercises summary and round visual states");
assert.deepEqual(
  Object.keys(fixtureMatchDetailsById).sort(),
  ["fixture-arena-1", "fixture-normal-1"],
  "detail lookup is deterministic for both synthetic match rows",
);
assert.equal(fixtureReview.summary.season, 23, "review fixture exposes the S23 success state");
assert.equal(fixtureCareerOverview.roles.length, 3, "MMR fixture covers tank, damage, and support role cards");
assert.ok(fixtureRanking.heroes.length > 0 && fixtureRanking.maps.length > 0, "ranking fixture covers hero and map tabs");
assert.equal(fixtureCommandCenter.accountLabel, "示例玩家", "command-center fixture uses a neutral anonymous account label");
assert.equal(fixtureCommandCenter.loginState, "ready", "command-center fixture exposes a usable neutral state");

for (const command of [
  "get_overview",
  "list_match_filters",
  "list_match_timeline",
  "get_match_details",
  "get_personal_season_review",
  "get_season_analysis",
  "get_career_mmr_overview",
  "get_role_mmr_history",
  "get_role_mmr_mode_breakdown",
  "get_role_mmr_season_breakdown",
  "get_career_mmr_build_status",
  "get_rankings",
  "get_onboarding_status",
  "get_player_profile",
  "get_player_target_status",
] as const) {
  assert.equal(command in visualInvokeResponses, true, `visual invoke map includes ${command}`);
}

visitFixtureValues(allVisualFixtures);

function visitFixtureValues(value: unknown): void {
  if (Array.isArray(value)) {
    value.forEach(visitFixtureValues);
    return;
  }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if ((key === "matchId" || key === "bnetId") && typeof child === "string") {
      assert.match(child, /^fixture-/, `${key} values must be visibly synthetic`);
    }
    visitFixtureValues(child);
  }
}
