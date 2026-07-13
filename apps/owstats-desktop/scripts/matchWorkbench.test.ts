import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const matchesPageSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");
const compactDetailSource = readFileSync(new URL("../src/features/matches/CompactMatchDetail.tsx", import.meta.url), "utf8");
const detailPaneSource = readFileSync(new URL("../src/features/matches/MatchDetailPane.tsx", import.meta.url), "utf8");
const listCardSource = readFileSync(new URL("../src/features/matches/MatchListCard.tsx", import.meta.url), "utf8");
const listPaneSource = readFileSync(new URL("../src/features/matches/MatchListPane.tsx", import.meta.url), "utf8");
const timelineSource = readFileSync(new URL("../src/features/matches/MatchTimeline.tsx", import.meta.url), "utf8");
const detailPageSource = readFileSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url), "utf8");
const styleSource = readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");
const matchStyleSource = readFileSync(new URL("../src/styles/features/matches.css", import.meta.url), "utf8");

assert.equal(matchesPageSource.includes("phase6MatchWorkbench"), true, "MatchesPage should render the combined match workbench layout");
assert.equal(matchesPageSource.includes("phase6MatchesHeader"), false, "MatchesPage should remove the match workbench header");
assert.equal(matchesPageSource.includes("<MetricCard"), false, "MatchesPage should remove the metric card strip above the workbench");
assert.equal(matchesPageSource.includes("getMatchDetails"), true, "MatchesPage should load selected match detail in place");
assert.equal(matchesPageSource.includes("MatchListPane"), true, "MatchesPage should coordinate the list pane component");
assert.equal(matchesPageSource.includes("MatchDetailPane"), true, "MatchesPage should coordinate the detail pane component");
assert.equal(matchesPageSource.includes("<CompactMatchDetail"), false, "MatchesPage should delegate compact detail rendering to the detail pane");
assert.equal(matchesPageSource.includes("<MatchTimeline"), false, "MatchesPage should delegate timeline rendering to the list pane");
assert.equal(matchesPageSource.includes("MatchDetailReport"), false, "MatchesPage should not reuse the full-size match detail page inside the workbench");
assert.equal(matchesPageSource.includes("selectedMatchId"), true, "MatchesPage should keep selected match state");
assert.equal(matchesPageSource.includes("navigate(`/matches/${matchId}`)"), false, "MatchesPage should not navigate away when selecting a match row");
assert.match(
  matchesPageSource,
  /setDetailStateMatchId\(selectedMatchId\);\s*setDetailLoading\(true\);\s*setSelectedDetail\(null\);\s*setDetailError\(null\);\s*void getMatchDetails\(selectedMatchId\)/,
  "MatchesPage should bind loading and error state to the requested match before loading detail",
);
assert.equal(
  matchesPageSource.includes("detail={selectedDetail?.matchId === selectedMatchId ? selectedDetail : null}"),
  true,
  "MatchesPage should hide detail that does not belong to the current selection before effects run",
);
assert.equal(matchesPageSource.includes("detail={selectedDetail}"), false, "MatchesPage should not pass unbound detail to the workbench");
assert.equal(
  matchesPageSource.includes("loading={detailStateMatchId === selectedMatchId ? detailLoading : true}"),
  true,
  "a new selection renders loading immediately instead of inheriting the previous request state",
);
assert.equal(
  matchesPageSource.includes("error={detailStateMatchId === selectedMatchId ? detailError : null}"),
  true,
  "detail errors are only rendered for the selected match that owns the state",
);
assert.equal(detailPaneSource.includes("CompactMatchDetail"), true, "MatchDetailPane should own compact detail rendering");
assert.equal(detailPaneSource.includes("选择左侧对局查看详情。"), true, "MatchDetailPane should own the unchanged empty state");
assert.equal(listPaneSource.includes("forwardRef<HTMLDivElement"), true, "MatchListPane should preserve the timeline ref boundary");
assert.equal(listPaneSource.includes("<MatchTimeline"), true, "MatchListPane should delegate streak rendering to MatchTimeline");

assert.equal(timelineSource.includes("selectedMatchId"), true, "MatchTimeline should accept the selected match id");
assert.equal(timelineSource.includes("<MatchListCard"), true, "MatchTimeline should map streak segments to list cards");
assert.equal(listCardSource.includes("phase6MatchRowSelected"), true, "MatchListCard should render selected state on the active row");
assert.equal(listCardSource.includes("MatchPerformanceBadges"), false, "Match list cards should not render grade or award badges");
assert.equal(listCardSource.includes("primaryTagBadgeTone"), true, "MatchListCard should keep the primary performance tag only");
assert.equal(listCardSource.includes("phase6MatchListHeroLine"), true, "MatchListCard should render a compact full-data hero row");
assert.equal(listCardSource.includes("phase6VerticalPerformanceTag"), true, "MatchListCard should show the performance tag as a vertical hero-side badge");
assert.equal(listCardSource.includes("<span>对局表现</span>"), false, "MatchListCard should omit the horizontal match performance label");
assert.equal(listCardSource.includes("<span>{match.heroName}</span>"), false, "MatchListCard should not render the hero name in compact list cards");
assert.ok(
  listCardSource.indexOf("phase6MatchListHeroLine") < listCardSource.indexOf("phase6MatchListStats") &&
    listCardSource.indexOf("phase6MatchListStats") < listCardSource.indexOf("phase6MatchOpen"),
  "MatchListCard should place list stats inside the hero row before the open affordance",
);
assert.equal(listCardSource.includes("phase6MatchListStats"), true, "MatchListCard should render full stats without horizontal clipping");
assert.equal(listCardSource.includes("matchListCardResult"), true, "MatchListCard should render result separately");
assert.equal(listCardSource.includes("matchListCardScore"), true, "MatchListCard should render score separately");
assert.equal(listCardSource.includes("match.heroName"), true, "MatchListCard should label the played hero");
assert.equal(listCardSource.includes("match.beginLabel"), true, "MatchListCard should show the match time");
assert.equal(
  listCardSource.includes("`${match.kills}/${match.deaths}/${match.assists}`"),
  true,
  "MatchListCard should render K/D/A in that order",
);
assert.equal(listCardSource.includes('label="K/A/D"'), false, "MatchListCard should not keep the ambiguous K/A/D order");
assert.match(matchStyleSource, /\.matchListCard\.phase6MatchRowSelected[\s\S]*border-color:\s*var\(--data-primary\)/);
assert.match(matchStyleSource, /\.matchListCardResult\.result-win \.matchListCardResultMarker[\s\S]*background:\s*var\(--status-success\)/);
assert.match(matchStyleSource, /\.matchListCardResult\.result-loss \.matchListCardResultMarker[\s\S]*background:\s*var\(--status-danger\)/);
assert.match(matchStyleSource, /\.matchListCard \.matchListCardMap[\s\S]*position:\s*static/);
assert.match(matchStyleSource, /\.matchListCardHeroLine \.phase6VerticalPerformanceTag[\s\S]*writing-mode:\s*horizontal-tb/);

assert.equal(detailPageSource.includes("export function MatchDetailReport"), true, "Match detail page should export reusable detail content for the workbench");

assert.equal(styleSource.includes(".phase6CompactDetail"), true, "styles should include the compact detail panel");
assert.equal(styleSource.includes("overflow-y: auto"), true, "workbench list should support independent vertical scrolling");
assert.equal(styleSource.includes("grid-template-columns: 380px minmax(0, 1fr)"), true, "workbench should reserve more width for the detail panel");
assert.equal(styleSource.includes("grid-template-columns: 1fr"), true, "compact player teams should stack vertically");
assert.equal(styleSource.includes(".phase6CompactDetailScroll"), true, "compact detail should have a scrollable content body");
assert.doesNotMatch(
  styleSource,
  /\.phase6WorkbenchList \.phase6MatchRowSelected::after\s*\{/,
  "selected row should not add a duplicate orange arrow",
);

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
