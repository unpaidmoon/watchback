import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const matchesPageSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");
const compactDetailSource = readFileSync(new URL("../src/features/matches/CompactMatchDetail.tsx", import.meta.url), "utf8");
const timelineSource = readFileSync(new URL("../src/features/matches/MatchTimeline.tsx", import.meta.url), "utf8");
const detailPageSource = readFileSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url), "utf8");
const styleSource = readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");

assert.equal(matchesPageSource.includes("phase6MatchWorkbench"), true, "MatchesPage should render the combined match workbench layout");
assert.equal(matchesPageSource.includes("phase6MatchesHeader"), false, "MatchesPage should remove the match workbench header");
assert.equal(matchesPageSource.includes("<MetricCard"), false, "MatchesPage should remove the metric card strip above the workbench");
assert.equal(matchesPageSource.includes("getMatchDetails"), true, "MatchesPage should load selected match detail in place");
assert.equal(matchesPageSource.includes("CompactMatchDetail"), true, "MatchesPage should use a compact in-page match detail");
assert.equal(matchesPageSource.includes("MatchDetailReport"), false, "MatchesPage should not reuse the full-size match detail page inside the workbench");
assert.equal(matchesPageSource.includes("selectedMatchId"), true, "MatchesPage should keep selected match state");
assert.equal(matchesPageSource.includes("navigate(`/matches/${matchId}`)"), false, "MatchesPage should not navigate away when selecting a match row");

assert.equal(timelineSource.includes("selectedMatchId"), true, "MatchTimeline should accept the selected match id");
assert.equal(timelineSource.includes("phase6MatchRowSelected"), true, "MatchTimeline should render selected state on the active row");
assert.equal(timelineSource.includes("MatchPerformanceBadges"), false, "MatchTimeline cards should not render grade or award badges");
assert.equal(timelineSource.includes("primaryTagBadgeTone"), true, "MatchTimeline should keep the primary performance tag only");
assert.equal(timelineSource.includes("phase6MatchListHeroLine"), true, "MatchTimeline should render a compact full-data hero row");
assert.equal(timelineSource.includes("phase6VerticalPerformanceTag"), true, "MatchTimeline should show the performance tag as a vertical hero-side badge");
assert.equal(timelineSource.includes("<span>对局表现</span>"), false, "MatchTimeline should remove the horizontal match performance label");
assert.equal(timelineSource.includes("<span>{match.heroName}</span>"), false, "MatchTimeline should not render the hero name in compact list cards");
assert.ok(
  timelineSource.indexOf("phase6MatchListHeroLine") < timelineSource.indexOf("phase6MatchListStats") &&
    timelineSource.indexOf("phase6MatchListStats") < timelineSource.indexOf("phase6MatchOpen"),
  "MatchTimeline should place list stats inside the hero row before the open affordance",
);
assert.equal(timelineSource.includes("phase6MatchListStats"), true, "MatchTimeline should render full stats without horizontal clipping");

assert.equal(detailPageSource.includes("export function MatchDetailReport"), true, "Match detail page should export reusable detail content for the workbench");

assert.equal(styleSource.includes(".phase6CompactDetail"), true, "styles should include the compact detail panel");
assert.equal(styleSource.includes("overflow-y: auto"), true, "workbench list should support independent vertical scrolling");
assert.equal(styleSource.includes("grid-template-columns: 380px minmax(0, 1fr)"), true, "workbench should reserve more width for the detail panel");
assert.equal(styleSource.includes("grid-template-columns: 1fr"), true, "compact player teams should stack vertically");
assert.equal(styleSource.includes(".phase6CompactDetailScroll"), true, "compact detail should have a scrollable content body");

assert.equal(compactDetailSource.includes("expandedPlayerKey,"), false, "compact detail should remove the old player insight expansion state");
assert.equal(compactDetailSource.includes("expandedCompactArenaPlayerKeys"), true, "compact arena detail may store arena player expansion state");
assert.equal(compactDetailSource.includes("toggleCompactArenaPlayer"), true, "compact arena detail should use an arena-specific expansion toggle");
assert.equal(compactDetailSource.includes("phase6CompactHeroButton"), false, "compact detail player rows should no longer limit expansion to avatar-only buttons");
assert.equal(compactDetailSource.includes("phase6CompactPlayerToggle"), false, "compact detail player rows should not be clickable expansion controls");
assert.equal(compactDetailSource.includes("CompactPlayerInsightCard"), false, "compact detail should remove the slide-out hero stat card");
assert.equal(compactDetailSource.includes("resolveHeroStatInfo"), false, "compact detail no longer needs hero stat label translation");
assert.equal(styleSource.includes(".phase6PlayerInsightCard"), false, "styles should remove the slide-out hero stat card");

assert.equal(compactDetailSource.includes("phase6CompactPerkStrip"), true, "compact detail player rows should render perk icons under the player id");
assert.equal(compactDetailSource.includes("phase6PerkTooltip"), true, "perk icons should expose a visible hover tooltip with descriptions");
assert.equal(compactDetailSource.includes("player.damageTaken"), true, "compact detail player rows should render damage taken");
assert.equal(compactDetailSource.includes("player.healingTaken"), true, "compact detail player rows should render healing taken");
assert.equal(compactDetailSource.includes("player.finalHits"), true, "compact detail player rows should render final hits");
assert.equal(compactDetailSource.includes("最后一击"), true, "compact detail player rows should label final hits in Chinese");
assert.equal(compactDetailSource.includes("player.mitigation"), true, "compact detail should render raw mitigation values in player rows");
assert.equal(compactDetailSource.includes("formatCompactNumber(player.mitigation)"), false, "compact detail should not compact mitigation values in player rows");
assert.equal(compactDetailSource.includes("phase7CompactExpandButton"), false, "compact arena rows should not use a separate expansion button");
assert.equal(compactDetailSource.includes("phase7CompactPlayerExpandable"), true, "compact arena rows should restore row-click expansion");
assert.equal(styleSource.includes("minmax(96px, 0.34fr)"), true, "player rows should reserve a compact final-hit column");

assert.equal(compactDetailSource.includes("compactHeroStats"), false, "compact detail should remove hero stat card helpers");
assert.equal(compactDetailSource.includes("heroUsagePercent"), false, "compact detail should remove hero usage percent helper");
assert.equal(compactDetailSource.includes("fallbackHeroStats"), false, "compact detail should not synthesize base-field fallback stats");
assert.equal(compactDetailSource.includes("slice(0, 6)"), true, "compact detail should support 6v6 rows");
