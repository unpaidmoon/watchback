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
assert.equal(source.includes("赛季概览"), false, "overview should remove the season overview summary");
assert.equal(source.includes("关键表现"), false, "overview should remove the key performance summary");
assert.equal(source.includes("职责与英雄"), false, "role panel should remove the descriptive eyebrow");
assert.equal(source.includes("职责段位与英雄池"), false, "role panel should remove the descriptive title");
assert.equal(source.includes("保留预设竞技、开放竞技与角斗竞技"), false, "role panel should remove the descriptive copy");
assert.match(source, /review\.heroPool\.mainHeroes\.slice\(0, 3\)\.map/);
assert.match(source, /<HeroAvatar key=\{hero\.hero\} heroKey=\{hero\.hero\} name=\{hero\.hero\} size="md" \/>/);
assert.match(source, /<Metric label="赛季对局" value=\{`\$\{review\.summary\.totalMatches\} 场`\} \/>/);
assert.match(source, /<Metric label="总胜率" value=\{formatWinRate\(review\.summary\.winRate, 0\)\} \/>/);
assert.match(source, /review\.stadiumRoleSummaries\.map/);
assert.match(source, /<RoleRankCard key=\{`stadium-\$\{role\.roleType\}`\} role=\{role\} arena \/>/);
assert.ok(
  source.indexOf('className="reviewHeroSummary"') < source.indexOf('className="reviewRoleRankGroup"'),
  "season facts should lead the role panel before rank groups",
);
assert.deepEqual(reviewRegions, ["NAV-CAPSULE", "REV-RANKS", "FRD-BOARDS", "FRD-LINEUP", "FRD-MAPS"], "removed review sections should not remain as QA feedback regions");
