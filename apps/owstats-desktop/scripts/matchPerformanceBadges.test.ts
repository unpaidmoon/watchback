import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { awardMedalText, awardsFromEvidenceTags, gradeBadgeTone, primaryTagBadgeTone } from "../src/shared/matchPerformanceBadges.ts";
import type { MatchPerformanceAward } from "../src/shared/types.ts";

const awards = awardsFromEvidenceTags(["击杀最多", "伤害最高", "治疗最高", "格挡最多", "死亡最多", "伯仲局"]);

assert.deepEqual(
  awards,
  [
    { code: "kill", label: "击杀最多", tone: "gold" },
    { code: "damage", label: "伤害最高", tone: "gold" },
    { code: "healing", label: "治疗最高", tone: "gold" },
    { code: "mitigation", label: "格挡最多", tone: "gold" },
    { code: "death", label: "死亡最多", tone: "danger" },
  ],
  "evidence tags map to medal awards",
);

const medalTexts = ([
  { code: "kill", label: "击杀最多", tone: "gold" },
  { code: "damage", label: "伤害最高", tone: "gold" },
  { code: "healing", label: "治疗最高", tone: "gold" },
  { code: "mitigation", label: "格挡最多", tone: "gold" },
  { code: "death", label: "死亡最多", tone: "danger" },
] satisfies MatchPerformanceAward[]).map(awardMedalText);

assert.deepEqual(medalTexts, ["杀", "伤", "疗", "挡", "死"], "award medals use compact Chinese characters");

assert.equal(gradeBadgeTone("S"), "gold", "S grade should share the gold badge tone");
assert.equal(gradeBadgeTone("A"), "blue", "A grade should share the blue badge tone");
assert.equal(gradeBadgeTone("B"), "muted", "B grade should use the muted purple badge tone");
assert.equal(gradeBadgeTone("C"), "muted", "C grade should use the muted purple badge tone");
assert.equal(primaryTagBadgeTone("带飞局"), "gold", "carry tag should share the gold badge tone");
assert.equal(primaryTagBadgeTone("尽力局"), "gold", "try-hard tag should share the gold badge tone");
assert.equal(primaryTagBadgeTone("躺赢局"), "blue", "free-win tag should share the blue badge tone");
assert.equal(primaryTagBadgeTone("背锅局"), "blue", "blame tag should share the blue badge tone");

const timelineSource = readFileSync(new URL("../src/features/matches/MatchTimeline.tsx", import.meta.url), "utf8");
const heroIndex = timelineSource.indexOf("phase6HeroStackCell");
const gradeIndex = timelineSource.indexOf("phase6MatchGrade");
assert.ok(heroIndex > -1 && gradeIndex > -1 && heroIndex < gradeIndex, "hero identity should remain left of match performance in the row layout");

const badgesSource = readFileSync(new URL("../src/shared/PerformanceBadgesView.tsx", import.meta.url), "utf8");
const primaryTagIndex = badgesSource.indexOf("primaryTagBadgeTone(evaluation.primaryTag)");
const gradePillIndex = badgesSource.indexOf("gradeBadgeTone(evaluation.grade)");
assert.ok(
  primaryTagIndex > -1 && gradePillIndex > -1 && primaryTagIndex < gradePillIndex,
  "primary match tag should render before the grade pill so circular badges stay grouped on the right",
);

const styleSource = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const matchGradeStyle = styleSource.match(/\.phase6MatchGrade \.gradePill \{(?<body>[\s\S]*?)\}/)?.groups?.body ?? "";
assert.match(matchGradeStyle, /width:\s*27px;/, "match grade circles should match award medal width");
assert.match(matchGradeStyle, /height:\s*27px;/, "match grade circles should match award medal height");
assert.match(matchGradeStyle, /flex:\s*0\s+0\s+27px;/, "match grade circles should match award medal flex size");
