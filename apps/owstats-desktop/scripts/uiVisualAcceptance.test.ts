import assert from "node:assert/strict";
import {
  allVisualFixtures,
  fixtureArenaDetail,
  fixtureCareerOverview,
  fixtureCommandCenter,
  fixtureEmptyCareerOverview,
  fixtureEmptyOnboarding,
  fixtureEmptyRanking,
  fixtureEmptyReview,
  fixtureMatchDetailsById,
  fixtureMatches,
  fixtureNormalDetail,
  fixtureRanking,
  fixtureReview,
  fixtureShell,
  invokeVisualFixture,
  resolveVisualInvokePlan,
  visualAcceptanceScenarios,
  visualInvokeResponses,
  visualPrimaryRequests,
} from "./uiVisualAcceptance.fixture.ts";

const serialized = JSON.stringify({
  allVisualFixtures,
  visualAcceptanceScenarios,
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
assert.equal(
  resolveVisualInvokePlan("get_match_details", { matchId: "fixture-normal-1" }).response,
  fixtureNormalDetail,
  "detail resolver selects the normal branch from the current direct invoke arguments",
);
assert.equal(
  resolveVisualInvokePlan("get_match_details", { request: { matchId: "fixture-arena-1" } }).response,
  fixtureArenaDetail,
  "detail resolver selects the arena branch from wrapped invoke arguments",
);
assert.throws(
  () => resolveVisualInvokePlan("get_match_details", { request: { matchId: "not-a-fixture" } }),
  (error: unknown) => {
    assert.match(String(error), /合成视觉验收错误/);
    assert.doesNotMatch(String(error), /not-a-fixture/);
    return true;
  },
  "unknown match ids fail with a Chinese synthetic error without echoing the input",
);
assert.throws(
  () => resolveVisualInvokePlan("get_match_details", {}),
  /合成视觉验收错误/,
  "missing match ids fail deterministically",
);
assert.equal(fixtureReview.summary.season, 23, "review fixture exposes the S23 success state");
assert.equal(fixtureCareerOverview.roles.length, 3, "MMR fixture covers tank, damage, and support role cards");
assert.ok(fixtureRanking.heroes.length > 0 && fixtureRanking.maps.length > 0, "ranking fixture covers hero and map tabs");
assert.equal(fixtureCommandCenter.accountLabel, "示例玩家", "command-center fixture uses a neutral anonymous account label");
assert.equal(fixtureCommandCenter.loginState, "ready", "command-center fixture exposes a usable neutral state");

assert.deepEqual(
  visualPrimaryRequests,
  {
    matches: "list_match_timeline",
    review: "get_personal_season_review",
    mmr: "get_career_mmr_overview",
    ranking: "get_rankings",
    commandCenter: "get_onboarding_status",
  },
  "state scenarios target each primary page request without guessing post-merge login commands",
);

for (const command of Object.values(visualPrimaryRequests)) {
  assert.ok(command in visualAcceptanceScenarios.success.commands, `success scenario describes ${command}`);
  assert.ok(command in visualAcceptanceScenarios.loading.commands, `loading scenario describes ${command}`);
  assert.ok(command in visualAcceptanceScenarios.empty.commands, `empty scenario describes ${command}`);
  assert.ok(command in visualAcceptanceScenarios.error.commands, `error scenario describes ${command}`);
  assert.ok(
    (visualAcceptanceScenarios.loading.commands[command]?.delayMs ?? 0) > 300,
    `loading scenario keeps ${command} pending long enough to expose page feedback`,
  );
  assert.match(
    visualAcceptanceScenarios.error.commands[command]?.errorMessage ?? "",
    /^合成视觉验收错误：/,
    `error scenario supplies a Chinese synthetic failure for ${command}`,
  );
}

assert.deepEqual(
  resolveVisualInvokePlan(visualPrimaryRequests.matches, {}, "empty").response,
  [],
  "matches empty scenario returns the timeline's expected empty array",
);
assert.equal(
  resolveVisualInvokePlan(visualPrimaryRequests.review, { request: { season: 23 } }, "empty").response,
  fixtureEmptyReview,
  "review empty scenario returns a complete empty review DTO",
);
assert.equal(fixtureEmptyReview.summary.season, null, "empty review triggers the page's no-season-data branch");
assert.equal(
  resolveVisualInvokePlan(visualPrimaryRequests.mmr, {}, "empty").response,
  fixtureEmptyCareerOverview,
  "MMR empty scenario returns a complete empty overview DTO",
);
assert.deepEqual(fixtureEmptyCareerOverview.roles, [], "empty MMR overview carries no role results");
assert.equal(
  resolveVisualInvokePlan(visualPrimaryRequests.ranking, { season: 23 }, "empty").response,
  fixtureEmptyRanking,
  "ranking empty scenario returns an empty ranking DTO",
);
assert.deepEqual(fixtureEmptyRanking.heroes, [], "empty ranking has no hero rows");
assert.equal(
  resolveVisualInvokePlan(visualPrimaryRequests.commandCenter, { request: { season: 23 } }, "empty").response,
  fixtureEmptyOnboarding,
  "command-center empty scenario returns a neutral no-data status DTO",
);
assert.deepEqual(fixtureEmptyOnboarding.seasonStatuses, [], "empty command center has no season data rows");

const recordedDelays: number[] = [];
const delayedRanking = await invokeVisualFixture(
  visualPrimaryRequests.ranking,
  { season: 23 },
  "loading",
  async (delayMs) => { recordedDelays.push(delayMs); },
);
assert.equal(delayedRanking, fixtureRanking, "loading scenario resolves to the success fixture after its delay");
assert.deepEqual(recordedDelays, [450], "browser harness can inject and observe the configured loading delay");
await assert.rejects(
  invokeVisualFixture(visualPrimaryRequests.matches, {}, "error", async () => undefined),
  /^Error: 合成视觉验收错误：/,
  "browser harness receives the configured Chinese synthetic error",
);

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
