import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { qaRegionsForPath } from "../src/shared/qaFeedback";

const source = readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const reviewRegions = qaRegionsForPath("/review").map((region) => region.id);

assert.equal(source.includes("phase6AchievementWall"), true, "review page should keep the achievement wall");
assert.equal(source.includes("phase6RoleRankGrid"), true, "review page should keep role rank overview");
assert.equal(source.includes("phase6ConclusionList"), false, "review page should remove review evidence conclusions");
assert.equal(source.includes("phase6BestList"), false, "review page should remove evidence-backed personal best panel");
assert.equal(source.includes("review.baseline"), false, "review page should remove season summary baseline panel");
assert.equal(source.includes("review.heroPool.mainHeroes.map"), false, "review page should remove detailed hero-pool summary panel");
assert.equal(source.includes("review.streaks.longestLossStreak"), false, "review page should remove detailed streak summary panel");
assert.deepEqual(reviewRegions, ["NAV-CAPSULE", "REV-RANKS", "FRD-BOARDS", "FRD-LINEUP", "FRD-MAPS"], "removed review sections should not remain as QA feedback regions");
