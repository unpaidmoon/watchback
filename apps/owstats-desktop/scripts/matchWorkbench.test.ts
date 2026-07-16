import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as React from "react";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { CompactMatchDetail } from "../src/features/matches/CompactMatchDetail";
import { MatchDetailReport } from "../src/pages/MatchDetailPage";
import type { MatchDetailView, PlayerRow } from "../src/shared/types";

const matchesPageSource = readFileSync(new URL("../src/pages/MatchesPage.tsx", import.meta.url), "utf8");
const compactDetailSource = readFileSync(new URL("../src/features/matches/CompactMatchDetail.tsx", import.meta.url), "utf8");
const detailHeaderSource = readFileSync(new URL("../src/features/matches/MatchDetailHeader.tsx", import.meta.url), "utf8");
const teamTableSource = readFileSync(new URL("../src/features/matches/MatchTeamTable.tsx", import.meta.url), "utf8");
const detailPaneSource = readFileSync(new URL("../src/features/matches/MatchDetailPane.tsx", import.meta.url), "utf8");
const listCardSource = readFileSync(new URL("../src/features/matches/MatchListCard.tsx", import.meta.url), "utf8");
const listPaneSource = readFileSync(new URL("../src/features/matches/MatchListPane.tsx", import.meta.url), "utf8");
const timelineSource = readFileSync(new URL("../src/features/matches/MatchTimeline.tsx", import.meta.url), "utf8");
const detailPageSource = readFileSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url), "utf8");
const phase6ComponentsSource = readFileSync(new URL("../src/shared/phase6Components.tsx", import.meta.url), "utf8");
const styleSource = readFileSync(new URL("../src/styles/shared.css", import.meta.url), "utf8");
const matchStyleSource = readFileSync(new URL("../src/styles/features/matches.css", import.meta.url), "utf8");

function sliceMediaBlock(source: string, marker: string): string {
  const start = source.indexOf(marker);
  assert.notEqual(start, -1, `missing media block: ${marker}`);
  const nextMedia = source.indexOf("\n@media", start + marker.length);
  return source.slice(start, nextMedia === -1 ? source.length : nextMedia);
}

const compactDesktopStyles = sliceMediaBlock(matchStyleSource, "@media (min-width: 1401px) and (max-width: 1500px) {");
const twoRowCardStyles = sliceMediaBlock(matchStyleSource, "@media (min-width: 641px) and (max-width: 1400px) {");
const narrowHandsetStyles = sliceMediaBlock(matchStyleSource, "@media (max-width: 400px) {");

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
  /const \[busy, setBusy\] = useState\(!canHydrateCache\)/,
  "uncached match pages should expose loading state on the first render",
);
assert.match(
  matchesPageSource,
  /error \? \(\s*<PageErrorState[\s\S]*?actionLabel="重新加载"[\s\S]*?onRetry=\{retryMatches\}[\s\S]*?\) : busy \? \(\s*<PageLoadingState message="正在加载对局列表\.\.\." \/>/,
  "the match list should render shared error and loading states before the empty timeline",
);
assert.match(
  matchesPageSource,
  /async function loadMatches\(nextFilters: MatchFilters\) \{[\s\S]*?setBusy\(true\);\s*setError\(null\);/,
  "filter and retry requests should enter a visible loading state",
);
assert.match(
  matchesPageSource,
  /function retryMatches\(\) \{\s*if \(pageDataReadyRef\.current\) \{\s*void loadMatches\(currentFiltersRef\.current\);\s*return;\s*\}\s*void hydrateInitialMatches\(currentFiltersRef\.current\);\s*\}/,
  "list retry should repeat the failed request without changing filter state",
);
assert.match(
  matchesPageSource,
  /matches\.length === 0 \? \(\s*<PageEmptyState[\s\S]*?title="暂无符合条件的对局"[\s\S]*?actionLabel=\{hasVisibleFilters \? "清除筛选" : "重新加载"\}/,
  "the match list should use the shared empty state with a useful context-aware action",
);
assert.match(
  matchesPageSource,
  /onAction=\{\(\) => hasVisibleFilters \? setFilters\(clearVisibleMatchFilters\(filters\)\) : retryMatches\(\)\}/,
  "the match empty state should clear visible filters while preserving the page season, or retry an unfiltered request",
);
assert.match(
  matchesPageSource,
  /const \[detailRetryKey, setDetailRetryKey\] = useState\(0\);[\s\S]*?\}, \[selectedMatchId, detailRetryKey\]\);/,
  "detail retry should re-run the selected detail request",
);
assert.equal(
  matchesPageSource.includes("onRetry={() => setDetailRetryKey((value) => value + 1)}"),
  true,
  "the detail pane should expose an explicit retry action",
);
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
assert.equal(matchesPageSource.includes("previousMatchId"), false, "matches page should stop deriving previous match state");
assert.equal(matchesPageSource.includes("nextMatchId"), false, "matches page should stop deriving next match state");
assert.equal(matchesPageSource.includes("mobileDetailOpen"), true, "MatchesPage should own the narrow detail-entry state");
assert.equal(matchesPageSource.includes("setMobileDetailOpen(true)"), true, "selecting a match should open narrow detail");
assert.equal(detailPaneSource.includes("onBack"), true, "detail pane should expose narrow back navigation");
assert.equal(detailPaneSource.includes("matchDetailBackButton"), true, "detail pane should render a real narrow back button");
assert.equal(detailPaneSource.includes("CompactMatchDetail"), true, "MatchDetailPane should own compact detail rendering");
assert.equal(detailPaneSource.includes('"onRetry"'), true, "MatchDetailPane should forward the retry contract");
assert.equal(detailPaneSource.includes("选择左侧对局查看详情。"), true, "MatchDetailPane should own the unchanged empty state");
assert.equal(detailHeaderSource.includes("hasPrevious"), false, "detail header should remove previous navigation state");
assert.equal(detailHeaderSource.includes("hasNext"), false, "detail header should remove next navigation state");
assert.equal(detailHeaderSource.includes("上一场"), false, "detail header should remove the previous button");
assert.equal(detailHeaderSource.includes("下一场"), false, "detail header should remove the next button");
assert.equal(teamTableSource.includes("matchTeamTableMarker"), true, "team tables should use a narrow semantic marker");
assert.equal(teamTableSource.includes("renderExpansion"), true, "the shared table should preserve arena expansion content");
assert.equal(teamTableSource.includes("<small>K/D/A</small>"), true, "detail tables should use the same K/D/A order as list cards");
assert.match(teamTableSource, /value=\{player\.kills\}[\s\S]*\/>\/\{player\.deaths\}\/\{player\.assists\}/);
assert.match(matchStyleSource, /\.matchTeamTableMarker\s*\{[\s\S]*background:\s*var\(--data-secondary\)/);
assert.match(matchStyleSource, /\.matchTeamTableMarker\.enemy\s*\{[\s\S]*background:\s*var\(--status-danger\)/);
assert.match(matchStyleSource, /\.matchTeamTable\.enemy \.phase6CompactPlayerRow\s*\{[\s\S]*border-left-color:\s*var\(--status-danger\)/);
assert.doesNotMatch(matchStyleSource, /#1e8ee7|#dc304f/i, "feature styles should not restore saturated team-row fills");
assert.equal(listPaneSource.includes("forwardRef<HTMLDivElement"), false, "MatchListPane should no longer own the scroll ref");
assert.equal(listPaneSource.includes("<MatchTimeline"), true, "MatchListPane should delegate streak rendering to MatchTimeline");
assert.equal(matchesPageSource.includes("ref={timelineRef}"), true, "MatchesPage should attach the cache ref to the pane scroll owner");

assert.equal(timelineSource.includes("selectedMatchId"), true, "MatchTimeline should accept the selected match id");
assert.equal(timelineSource.includes("<MatchListCard"), true, "MatchTimeline should map streak segments to list cards");
assert.equal(listCardSource.includes("phase6MatchRowSelected"), true, "MatchListCard should render selected state on the active row");
assert.equal(listCardSource.includes("<MatchPerformanceBadges"), true, "MatchListCard should render compact grade and award badges");
assert.equal(listCardSource.includes("compact"), true, "list badges should use the compact layout");
assert.equal(listCardSource.includes("primaryTagBadgeTone"), false, "MatchListCard should delegate primary tags to shared badges");
assert.equal(listCardSource.includes('className="matchListCardAvatar"'), true, "MatchListCard should render the avatar as the first reference-layout column");
assert.equal(listCardSource.includes("phase6VerticalPerformanceTag"), false, "MatchListCard should replace the legacy performance tag with shared badges");
assert.equal(listCardSource.includes("<span>对局表现</span>"), true, "MatchListCard should label the fourth reference-layout column");
assert.equal(listCardSource.includes("<span>{match.heroName}</span>"), false, "MatchListCard should not render the hero name in compact list cards");
assert.equal(listCardSource.includes("phase6MatchListStats"), false, "MatchListCard should not nest fact columns in the old hero-row layout");
assert.equal(listCardSource.includes("phase6MatchOpen"), false, "MatchListCard should not render a redundant open affordance");
assert.equal(listCardSource.includes("matchListCardHeader"), true, "MatchListCard should render the map summary banner");
assert.equal(listCardSource.includes("matchListCardResultBlock"), true, "MatchListCard should render the result block inside the banner");
assert.equal(listCardSource.includes("decorative"), true, "MatchListCard banner map should be decorative");
assert.equal(listCardSource.includes("matchListCardBannerCopy"), true, "MatchListCard banner should show the map name and mode on the image");
assert.match(
  listCardSource,
  /<span className="matchListCardBannerCopy"[^>]*>\s*<span>\{match\.mapName\}<\/span>\s*<span>\{match\.modeLabel\}<\/span>\s*<\/span>/,
  "MatchListCard banner copy should order map name immediately before mode label",
);
assert.equal(
  listCardSource.includes('title={`${match.mapName} · ${match.modeLabel}`}'),
  true,
  "MatchListCard banner should expose the complete map and mode as its title",
);
assert.match(listCardSource, /<MatchPerformanceBadges[\s\S]*?showGrade=\{false\}/, "MatchListCard should omit the grade from its evaluation row");
assert.equal(listCardSource.includes('className="matchListCardHeroName"'), false, "MatchListCard third row should not repeat the hero name");
assert.equal(listCardSource.includes("match.beginLabel"), false, "MatchListCard should omit the match time");
assert.equal(listCardSource.includes("matchListCardModeTime"), false, "MatchListCard should not render a separate mode/time row");
assert.equal(listCardSource.includes('className="matchListCardPerformance"'), true, "MatchListCard should place tags and medals in the fourth reference-layout column");
assert.equal(listCardSource.includes('label="K/A/D"'), true, "MatchListCard should use the reference K/A/D label");
assert.equal(listCardSource.includes('label="伤害/治疗/抵挡"'), true, "MatchListCard should group the three reference metrics");
assert.equal(
  listCardSource.includes('className="matchListCardCombinedStats"'),
  true,
  "MatchListCard should identify the combined stats block with a semantic class",
);
assert.equal(
  listCardSource.includes('className={`phase6MatchMetricBlock ${className}`}'),
  true,
  "MetricBlock should append its optional semantic class to the shared metric class",
);
assert.equal(
  listCardSource.includes("`${match.kills}/${match.assists}/${match.deaths}`"),
  true,
  "MatchListCard should render the K/A/D value in the reference order",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList button\.matchListCard\s*\{(?=[^}]*height:\s*152px)(?=[^}]*grid-template-rows:\s*64px\s+88px)[^}]*\}/s,
  "MatchListCard should use the fixed 64px banner and 88px data rows",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardResultBlock\s*\{[^}]*background:\s*transparent/s,
  "MatchListCard result block should leave the six-point map outline unobscured",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardHeader\s*\{[^}]*background:\s*color-mix\(in srgb,\s*var\(--surface-secondary\) 88%,\s*transparent\)/s,
  "MatchListCard header should use the light gray surface beneath the layered map preview",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardData\s*\{[^}]*background:\s*var\(--action-on-primary\)/s,
  "MatchListCard data area should use the white surface after the vertical color swap",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardResultBlock\s*\{(?=[^}]*gap:\s*var\(--space-1\))(?=[^}]*color:\s*var\(--text-primary\))(?=[^}]*font-size:\s*20px)[^}]*\}/s,
  "MatchListCard result label should use the same 20px dark text treatment as the score without wrapping",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardResultBlock strong\s*\{(?=[^}]*color:\s*var\(--text-primary\))(?=[^}]*font-size:\s*20px)[^}]*\}/s,
  "MatchListCard score should match the result label size and color",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardResultBlock\.result-win::before\s*\{[^}]*background:\s*var\(--status-success\)/s,
  "winning cards should show the semantic success dot",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardResultBlock\.result-loss::before\s*\{[^}]*background:\s*var\(--status-danger\)/s,
  "losing cards should show the semantic danger dot",
);
assert.doesNotMatch(
  matchStyleSource,
  /\.matchListCardResultBlock::after\s*\{/,
  "MatchListCard result block should not recreate the removed triangle overlay",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardHeaderMap\s*\{[^}]*clip-path:\s*polygon\(26% 0,\s*100% 0,\s*100% 100%,\s*58% 100%,\s*55% 82%,\s*29% 82%\)/s,
  "MatchListCard map preview should use the approved layered six-point outline",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardHeaderMap\.phase6AssetFallback\s*\{[^}]*background:\s*color-mix\(in srgb,\s*var\(--action-primary\) 78%,\s*var\(--surface-secondary\)\)/s,
  "missing MatchListCard map previews should use the stable dark semantic-token surface",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardBannerCopy\s*\{[^}]*right:\s*var\(--space-4\)[^}]*bottom:\s*20px/s,
  "MatchListCard banner copy should align to the approved 20px baseline",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardBannerCopy > span \+ span\s*\{(?=[^}]*flex:\s*0 0 auto)(?=[^}]*max-width:\s*none)[^}]*\}/s,
  "MatchListCard mode label should keep its full intrinsic width instead of inheriting a percentage cap from the map name",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardData\s*\{[^}]*grid-template-columns:\s*60px\s+minmax\(0,\s*0\.7fr\)\s+minmax\(0,\s*1\.2fr\)\s+minmax\(0,\s*1\.3fr\)/s,
  "MatchListCard data row should use avatar, KAD, combined stats, and performance columns",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardData \.phase6MatchMetricBlock strong\s*\{[^}]*font-size:\s*16px/s,
  "wide MatchListCard values should use the approved compact 16px size",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardPerformance \.phase6PerformanceBadges\s*\{[^}]*gap:\s*var\(--space-1\)/s,
  "wide MatchListCard performance badges should use the compact 4px gap",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.phase6Timeline\s*\{[^}]*gap:\s*var\(--space-3\);[^}]*\}/s,
  "match list cards should use the approved 12px vertical rhythm",
);
assert.match(
  compactDesktopStyles,
  /\.phase6WorkbenchList \.matchListCardData\s*\{[^}]*grid-template-columns:\s*50px\s+70px\s+minmax\(0,\s*1fr\)\s+84px[^}]*gap:\s*var\(--space-2\)[^}]*padding-inline:\s*var\(--space-3\)/s,
  "1440px desktop cards should keep four compact data columns",
);
assert.match(
  compactDesktopStyles,
  /\.phase6WorkbenchList \.matchListCardAvatar,\s*\.phase6WorkbenchList \.matchListCardAvatar \.phase6HeroAvatarShared\s*\{[^}]*width:\s*50px[^}]*height:\s*50px/s,
  "1440px desktop cards should use 50px avatars",
);
assert.match(
  compactDesktopStyles,
  /\.phase6WorkbenchList \.matchListCardData \.phase6MatchMetricBlock strong\s*\{[^}]*font-size:\s*16px/s,
  "1440px desktop cards should retain readable 16px values",
);
assert.match(
  compactDesktopStyles,
  /\.phase6WorkbenchList \.matchListCardPerformance \.phase6PerformanceBadges\s*\{[^}]*gap:\s*2px/s,
  "1440px desktop cards should use a compact badge gap",
);
assert.match(
  compactDesktopStyles,
  /\.phase6WorkbenchList \.matchListCardPerformance \.primaryTag\s*\{[^}]*min-height:\s*20px[^}]*padding:\s*0 1px[^}]*font-size:\s*12px/s,
  "1440px desktop primary tags should remain at the 12px visible-text minimum",
);
assert.match(
  compactDesktopStyles,
  /\.phase6WorkbenchList \.matchListCardPerformance \.phase6AwardMedal\s*\{[^}]*width:\s*20px[^}]*height:\s*20px[^}]*flex:\s*0 0 20px[^}]*font-size:\s*12px/s,
  "1440px desktop cards should compact avatars, values, tags, and medals without hiding them",
);
assert.match(
  twoRowCardStyles,
  /\.phase6WorkbenchList button\.matchListCard\s*\{[^}]*height:\s*auto[^}]*min-height:\s*152px[^}]*grid-template-rows:\s*64px auto/s,
  "641px to 1400px cards should fall back to an auto-height two-row layout",
);
assert.match(
  twoRowCardStyles,
  /\.phase6WorkbenchList \.matchListCardData\s*\{[^}]*height:\s*auto[^}]*min-height:\s*104px[^}]*grid-template-columns:\s*54px\s+minmax\(0,\s*0\.8fr\)\s+minmax\(0,\s*1\.4fr\)[^}]*gap:\s*var\(--space-3\)/s,
  "641px to 1400px cards should use three data columns without truncating stats",
);
assert.match(
  twoRowCardStyles,
  /\.phase6WorkbenchList \.matchListCardAvatar,\s*\.phase6WorkbenchList \.matchListCardAvatar \.phase6HeroAvatarShared\s*\{[^}]*width:\s*54px[^}]*height:\s*54px/s,
  "641px to 1400px cards should use 54px avatars",
);
assert.match(
  twoRowCardStyles,
  /\.phase6WorkbenchList \.matchListCardData \.phase6MatchMetricBlock strong\s*\{[^}]*font-size:\s*16px/s,
  "641px to 1400px card values should remain 16px",
);
assert.match(
  twoRowCardStyles,
  /\.phase6WorkbenchList \.matchListCardPerformance\s*\{[^}]*grid-column:\s*2\s*\/\s*-1/s,
  "641px to 1400px performance badges should occupy the second data row",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList button\.matchListCard\s*\{[^}]*height:\s*auto/s,
  "400px and narrower cards should remain auto-height",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardResultBlock\s*\{(?=[^}]*width:\s*40%)(?=[^}]*min-width:\s*96px)(?=[^}]*padding-inline:\s*var\(--space-1\))(?=[^}]*gap:\s*var\(--space-1\))(?=[^}]*font-size:\s*20px)[^}]*\}/s,
  "400px and narrower result blocks should reserve a compact non-overlapping banner region",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardResultBlock strong\s*\{[^}]*font-size:\s*20px/s,
  "400px and narrower scores should use the compact 20px size",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardBannerCopy\s*\{(?=[^}]*right:\s*var\(--space-2\))(?=[^}]*max-width:\s*calc\(100% - 112px\))[^}]*\}/s,
  "400px and narrower banner copy should remain outside the result rectangle",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardHeaderMap\s*\{[^}]*clip-path:\s*polygon\(40% 0,\s*100% 0,\s*100% 100%,\s*58% 100%,\s*55% 82%,\s*43% 82%\)/s,
  "400px and narrower map previews should keep the full result copy on the white layered block",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardData\s*\{(?=[^}]*min-height:\s*188px)(?=[^}]*grid-template-columns:\s*48px\s+minmax\(0,\s*1fr\))(?=[^}]*row-gap:\s*var\(--space-2\))(?=[^}]*column-gap:\s*var\(--space-3\))[^}]*\}/s,
  "400px and narrower data rows should use the approved two-column three-row layout",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardCombinedStats\s*\{[^}]*grid-column:\s*1\s*\/\s*-1/s,
  "400px and narrower combined stats should occupy a full row",
);
assert.doesNotMatch(
  narrowHandsetStyles,
  /\.phase6MatchMetricBlock:nth-child/,
  "400px and narrower combined stats should not depend on DOM child position",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardPerformance\s*\{[^}]*grid-column:\s*1\s*\/\s*-1/s,
  "400px and narrower performance badges should occupy a full row",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardAvatar,\s*\.phase6WorkbenchList \.matchListCardAvatar \.phase6HeroAvatarShared\s*\{[^}]*width:\s*48px[^}]*height:\s*48px/s,
  "400px and narrower cards should use 48px avatars",
);
assert.match(
  narrowHandsetStyles,
  /\.phase6WorkbenchList \.matchListCardData \.phase6MatchMetricBlock strong\s*\{[^}]*font-size:\s*16px/s,
  "400px and narrower core values should remain 16px",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardAvatar,\s*\.phase6WorkbenchList \.matchListCardAvatar \.phase6HeroAvatarShared\s*\{(?=[^}]*width:\s*60px)(?=[^}]*height:\s*60px)[^}]*\}/s,
  "MatchListCard avatar container and image should both remain 60px square",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList button\.matchListCard\.phase6MatchRowSelected\s*\{[^}]*box-shadow:\s*inset\s+0\s+0\s+0\s+2px\s+var\(--action-primary\)/s,
  "MatchListCard selected state should use the approved inset action ring",
);
assert.doesNotMatch(
  matchStyleSource,
  /\.matchListCardBadgeLine\b/,
  "MatchListCard styles should remove the unused legacy badge-line selectors",
);
assert.match(matchStyleSource, /grid-template-columns:\s*minmax\([^)]*,\s*32fr\)\s+minmax\(0,\s*68fr\)/);
assert.match(
  matchStyleSource,
  /\.appFrame:has\(\.matchesPage\)\s*\{[^}]*height:\s*calc\(100dvh - 16px\)[^}]*display:\s*flex[^}]*flex-direction:\s*column/s,
  "the match route should constrain the shell to the available viewport without changing long-form pages",
);
assert.match(
  matchStyleSource,
  /\.appCanvas:has\(> \.matchesPage\)\s*\{[^}]*flex:\s*1[^}]*min-height:\s*0/s,
  "the match route canvas should give its remaining height to the workbench",
);
assert.match(
  matchStyleSource,
  /\.matchesPage\s*\{[^}]*height:\s*100%[^}]*min-height:\s*0/s,
  "the match page should fill the shell remainder instead of recalculating the viewport",
);
assert.match(matchStyleSource, /@media\s*\(max-width:\s*1099px\)/);
assert.match(matchStyleSource, /\.matchesWorkbench\.detailOpen\s+\.matchListPane\s*\{[^}]*display:\s*none/s);
assert.match(matchStyleSource, /\.matchesWorkbench:not\(\.detailOpen\)\s+\.matchDetailPane\s*\{[^}]*display:\s*none/s);
assert.match(matchStyleSource, /\.matchListPane,\s*\.matchDetailPane\s*\{[^}]*overflow-y:\s*auto/s);
assert.match(matchStyleSource, /\.matchListPane \.phase6Timeline,\s*\.matchDetailPane \.phase6CompactDetailScroll\s*\{[^}]*overflow:\s*visible/s);
assert.match(
  matchStyleSource,
  /\.filterBar\.phase6FilterBar\s*\{[^}]*border:\s*1px solid var\(--glass-border\);[^}]*background:\s*var\(--glass-primary\);[^}]*box-shadow:\s*none;/s,
  "筛选栏应使用低边界玻璃表面且不叠加阴影",
);
assert.match(
  matchStyleSource,
  /\.matchListPane,\s*\.matchDetailPane\s*\{[^}]*border:\s*1px solid var\(--glass-border\);[^}]*background:\s*var\(--glass-primary\);/s,
  "工作台主从面板应使用同级玻璃表面和低对比边线",
);
assert.match(matchStyleSource, /\.activeFilterChips button\s*\{[^}]*border-radius:\s*999px/s);
assert.match(
  matchStyleSource,
  /\.phase7CompactArenaDetail \.phase7ArenaTab\.active,\s*\.phase6DetailPage > \.phase7ArenaSelector \.phase7ArenaTab\.active\s*\{[^}]*background:\s*var\(--action-primary\)/s,
  "standalone arena detail should share the dark action selector",
);
assert.match(matchStyleSource, /\.matchTeamTable \.phase6CompactPlayerMetric strong\s*\{[^}]*white-space:\s*nowrap/s);

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

assert.equal(teamTableSource.includes("phase6CompactPerkStrip"), true, "shared player rows should render perk icons under the player id");
assert.equal(teamTableSource.includes("phase6PerkTooltip"), true, "perk icons should expose a visible hover tooltip with descriptions");
assert.match(
  matchStyleSource,
  /\.matchTeamTable \.phase6PerkTooltip\s*\{[^}]*background:\s*var\(--surface-primary\)/s,
  "match detail perk icons should sit on the existing white primary surface",
);
assert.match(
  matchStyleSource,
  /\.matchTeamTable \.phase6CompactPerkIcon\s*\{[^}]*background:\s*transparent[^}]*filter:\s*brightness\(0\)[^}]*opacity:\s*0\.46/s,
  "match detail perk glyphs should become light gray while remaining legible on the white backing",
);
assert.match(
  matchStyleSource,
  /\.matchTeamTable \.phase6PerkTooltipPanel\s*\{[^}]*left:\s*0[^}]*max-width:\s*min\(300px,\s*60vw\)[^}]*overflow-wrap:\s*anywhere[^}]*transform:\s*translate\(0,\s*6px\)/s,
  "match detail perk tooltips should open rightward and wrap long copy inside the pane",
);
assert.match(
  matchStyleSource,
  /\.matchTeamTable \.phase6PerkTooltip:hover \.phase6PerkTooltipPanel,\s*\.matchTeamTable \.phase6PerkTooltip:focus-within \.phase6PerkTooltipPanel\s*\{[^}]*transform:\s*translate\(0,\s*0\)/s,
  "visible match detail perk tooltips should keep their left edge anchored",
);
assert.match(
  matchStyleSource,
  /\.matchTeamTable \.phase6CompactPlayerRow \.phase6PerkTooltipPanel strong\s*\{[^}]*color:\s*var\(--action-on-primary\)/s,
  "match detail perk tooltip titles should stay light on the dark panel",
);
assert.equal(teamTableSource.includes("player.damageTaken"), true, "shared player rows should render damage taken");
assert.equal(teamTableSource.includes("player.healingTaken"), true, "shared player rows should render healing taken");
assert.equal(teamTableSource.includes("player.finalHits"), true, "shared player rows should render final hits");
assert.equal(teamTableSource.includes("最后一击"), true, "shared player rows should label final hits in Chinese");
assert.equal(teamTableSource.includes("player.mitigation"), true, "shared player rows should render raw mitigation values");
assert.equal(teamTableSource.includes("formatCompactNumber(player.mitigation)"), false, "shared player rows should not compact mitigation values");
assert.equal(compactDetailSource.includes("phase7CompactExpandButton"), false, "compact arena rows should not use a separate expansion button");
assert.equal(teamTableSource.includes("phase7CompactPlayerExpandable"), true, "compact arena rows should preserve row-click expansion");
assert.equal(styleSource.includes("minmax(96px, 0.34fr)"), true, "player rows should reserve a compact final-hit column");

assert.equal(compactDetailSource.includes("compactHeroStats"), false, "compact detail should remove hero stat card helpers");
assert.equal(compactDetailSource.includes("heroUsagePercent"), false, "compact detail should remove hero usage percent helper");
assert.equal(compactDetailSource.includes("fallbackHeroStats"), false, "compact detail should not synthesize base-field fallback stats");
assert.equal(compactDetailSource.includes("slice(0, 6)"), true, "compact detail should support 6v6 rows");
assert.equal(compactDetailSource.includes('<section className="matchDetailPerformance"'), false, "normal detail should remove the personal summary panel");
assert.equal(compactDetailSource.includes("本人表现"), false, "normal detail should not retain the removed summary title");

assert.ok(
  compactDetailSource.indexOf("MatchDetailHeader") < compactDetailSource.indexOf("队伍对比") &&
    compactDetailSource.indexOf("队伍对比") < compactDetailSource.indexOf('title="队友数据"') &&
    compactDetailSource.indexOf('title="队友数据"') < compactDetailSource.indexOf('title="对手数据"'),
  "normal detail should keep the agreed information order",
);

assert.equal(detailHeaderSource.includes("MapPreview"), true, "detail header should render the existing map preview asset");
assert.match(detailHeaderSource, /mapGuid:\s*string\s*\|\s*null/, "detail header should accept the map guid for the preview");
assert.equal(detailHeaderSource.includes("mapKey={mapGuid ?? mapName}"), true, "detail header should prefer the map guid when resolving its preview");
assert.equal(detailHeaderSource.includes("resultEnglishLabel"), true, "detail header should derive the English result label");
assert.equal(compactDetailSource.includes("mapGuid={detail.mapGuid}"), true, "normal detail should pass its map guid to the shared header");
assert.equal(compactDetailSource.includes("mapGuid={selected.mapGuid}"), true, "arena detail should pass the selected slice map guid to the shared header");
assert.equal(detailPageSource.includes("<MatchDetailHeader"), true, "standalone normal detail should reuse the shared result map banner");
assert.equal(detailHeaderSource.includes("decorative"), true, "detail banner map preview should be hidden from assistive technology");
assert.equal(detailHeaderSource.includes("matchDetailHeaderInfo"), true, "header should own one right-aligned information group");
assert.ok(
  detailHeaderSource.indexOf("<h2>{mapName}</h2>") < detailHeaderSource.indexOf("matchDetailHeaderScore") &&
    detailHeaderSource.indexOf("matchDetailHeaderScore") < detailHeaderSource.indexOf("matchDetailHeaderMeta"),
  "header information order should be map, score, then metadata",
);
assert.equal(phase6ComponentsSource.includes("decorative?: boolean"), true, "MapPreview should expose an explicit decorative mode");
assert.equal(phase6ComponentsSource.includes('aria-hidden={decorative ? "true" : undefined}'), true, "decorative image fallback should be hidden from assistive technology");
assert.equal(detailHeaderSource.includes("--space-5"), false, "detail banner should only use defined spacing tokens");
assert.match(matchStyleSource, /\.matchDetailHeaderResultBlock\.result-win[\s\S]*?background:\s*var\(--status-success-bg\)/);
assert.match(matchStyleSource, /\.matchDetailHeaderResultBlock\.result-loss[\s\S]*?background:\s*var\(--surface-control\)/);
assert.match(matchStyleSource, /\.matchDetailHeaderResultBlock\.result-draw[\s\S]*?background:\s*var\(--surface-control\)/);
assert.doesNotMatch(matchStyleSource, /#ff8[0-9a-f]{3}|orange/i, "match result header must not introduce orange");
assert.match(matchStyleSource, /\.matchDetailHeaderContent\s*\{[^}]*color:\s*var\(--action-on-primary\)/s, "map-side content should use the inverse text token");
assert.doesNotMatch(matchStyleSource, /\.matchDetailHeaderContent \.matchDetailHeaderMeta\s*\{[^}]*color:\s*var\(--action-on-primary\)/s, "metadata should use the reference image's lower-emphasis white");
assert.match(matchStyleSource, /\.matchDetailHeaderInfo\s*\{[^}]*justify-items:\s*end;[^}]*text-align:\s*right;/s);
assert.match(matchStyleSource, /\.matchDetailHeaderInfo h2\s*\{[^}]*font-size:\s*22px;/s);
assert.match(matchStyleSource, /\.matchDetailHeaderScore\s*\{[^}]*font-size:\s*30px;/s);
assert.match(matchStyleSource, /\.matchDetailHeaderMeta\s*\{[^}]*font-size:\s*13px;/s);
assert.doesNotMatch(matchStyleSource, /\.matchDetailHeaderNavigation\s*\{/);

const anonymousPlayer: PlayerRow = {
  side: "teammate",
  name: "匿名队友",
  bnetId: "",
  rankLabel: null,
  rankKey: null,
  heroGuid: "",
  heroName: "匿名英雄",
  kills: 12,
  assists: 8,
  deaths: 4,
  damage: 12345,
  healing: 2345,
  mitigation: 3456,
  finalHits: 5,
  targetTimeSec: 30,
  healingTaken: 4567,
  damageTaken: 6789,
  worth: null,
  endWorth: null,
  worthMax: false,
  modGuids: [],
  traitGuids: [],
  hasSourceMaxFlags: false,
  killMax: false,
  heroDamageMax: false,
  cureMax: false,
  resistDamageMax: false,
  perks: [],
  heroStats: [],
};

const normalDetail: MatchDetailView = {
  matchId: "anonymous-normal",
  gameMode: "QuickPlay",
  modeLabel: "快速比赛",
  result: "win",
  resultLabel: "胜利",
  mapGuid: "",
  mapName: "匿名地图",
  heroGuid: "",
  heroName: "匿名英雄",
  beginTs: 0,
  beginLabel: "70/01/01 08:00",
  teamScore: 3,
  opponentScore: 1,
  durationSec: 600,
  teammates: [anonymousPlayer],
  enemies: [{ ...anonymousPlayer, side: "enemy", name: "匿名对手" }],
  arenaDetail: null,
  evaluation: null,
  teamAwards: [],
  sameRoleComparison: null,
  favorite: null,
  rawJson: null,
};

const navigationProps = {
  onRetry: () => undefined,
};

const detailLoadingMarkup = renderToStaticMarkup(
  createElement(CompactMatchDetail, {
    detail: null,
    loading: true,
    error: null,
    matchId: "fixture-loading",
    ...navigationProps,
  }),
);
assert.match(detailLoadingMarkup, /role="status"/);
assert.match(detailLoadingMarkup, /aria-busy="true"/);
assert.match(detailLoadingMarkup, /正在读取对局详情\.\.\./);

const detailErrorMarkup = renderToStaticMarkup(
  createElement(CompactMatchDetail, {
    detail: null,
    loading: false,
    error: "对局详情读取失败",
    matchId: "fixture-error",
    ...navigationProps,
  }),
);
assert.match(detailErrorMarkup, /role="alert"/);
assert.match(detailErrorMarkup, /对局详情读取失败/);
assert.match(detailErrorMarkup, /<button[^>]*>重新加载<\/button>/);
const normalMarkup = renderToStaticMarkup(
  createElement(CompactMatchDetail, { detail: normalDetail, loading: false, error: null, matchId: normalDetail.matchId, ...navigationProps }),
);
assert.match(normalMarkup, /aria-label="普通对局详情"/);
assert.doesNotMatch(normalMarkup, />本人表现</);
assert.match(normalMarkup, />队友数据</);
assert.match(normalMarkup, />对手数据</);

const standaloneNormalMarkup = renderToStaticMarkup(
  createElement(MatchDetailReport, {
    detail: {
      ...normalDetail,
      evaluation: { grade: "A", primaryTag: "带飞局", evidenceTags: [], confidence: 0.9 },
      teamAwards: [{ code: "damage", label: "本队伤害最高", tone: "gold" }],
    },
    loading: false,
    error: null,
    matchId: normalDetail.matchId,
  }),
);
assert.match(standaloneNormalMarkup, /<strong>胜利<\/strong>/);
assert.match(standaloneNormalMarkup, />VICTORY</, "standalone win detail should render the shared English result label");
assert.doesNotMatch(standaloneNormalMarkup, />对局表现</, "standalone normal detail should remove the personal summary eyebrow");
assert.doesNotMatch(standaloneNormalMarkup, />评分与同队奖牌</, "standalone normal detail should remove the personal summary title");
assert.doesNotMatch(standaloneNormalMarkup, /phase6PerformanceBadges/, "standalone normal detail should not render performance badges");

for (const [result, resultLabel, englishLabel] of [
  ["loss", "失败", "DEFEAT"],
  ["draw", "平局", "DRAW"],
] as const) {
  const standaloneMarkup = renderToStaticMarkup(
    createElement(MatchDetailReport, {
      detail: { ...normalDetail, matchId: `anonymous-${result}`, result, resultLabel },
      loading: false,
      error: null,
      matchId: `anonymous-${result}`,
    }),
  );
  assert.match(standaloneMarkup, new RegExp(`>${englishLabel}<`), `standalone ${result} detail should render ${englishLabel}`);
}

const standaloneUnknownMarkup = renderToStaticMarkup(
  createElement(MatchDetailReport, {
    detail: { ...normalDetail, matchId: "anonymous-unknown", result: null, resultLabel: "未知结果" },
    loading: false,
    error: null,
    matchId: "anonymous-unknown",
  }),
);
assert.match(standaloneUnknownMarkup, />UNKNOWN</, "unknown result should not be announced as a draw");

const arenaDetail: MatchDetailView = {
  ...normalDetail,
  matchId: "anonymous-arena",
  gameMode: "LeisureFight",
  modeLabel: "角斗竞技",
  arenaDetail: {
    summary: {
      key: "summary",
      label: "角斗汇总",
      result: "win",
      resultLabel: "胜利",
      teamScore: 4,
      opponentScore: 2,
      durationSec: 720,
      beginTs: 0,
      beginLabel: "70/01/01 08:00",
      mapGuid: "",
      mapName: "匿名角斗地图",
      teammates: [anonymousPlayer],
      enemies: [{ ...anonymousPlayer, side: "enemy", name: "匿名角斗对手" }],
    },
    rounds: [
      {
        key: "round-1",
        label: "第 1 回合",
        result: "win",
        resultLabel: "胜利",
        teamScore: 1,
        opponentScore: 0,
        durationSec: 180,
        beginTs: 0,
        beginLabel: "70/01/01 08:00",
        mapGuid: "",
        mapName: "第 1 回合地图",
        teammates: [{ ...anonymousPlayer, name: "第 1 回合队友" }],
        enemies: [{ ...anonymousPlayer, side: "enemy", name: "匿名角斗对手" }],
      },
    ],
  },
};
const arenaMarkup = renderToStaticMarkup(
  createElement(CompactMatchDetail, { detail: arenaDetail, loading: false, error: null, matchId: arenaDetail.matchId, ...navigationProps }),
);
assert.match(arenaMarkup, /aria-label="角斗对局详情"/);
assert.match(arenaMarkup, /<button[^>]*><span>角斗汇总<\/span><\/button>/, "arena detail should keep the summary switch");
assert.match(arenaMarkup, /<button[^>]*><span>回合-1<\/span><strong>1 - 0<\/strong><\/button>/, "arena detail should keep the round switch and label");
assert.match(arenaMarkup, />队友数据</, "arena detail should keep the teammate table");
assert.match(arenaMarkup, />对手数据</, "arena detail should keep the enemy table");

type TestElement = React.ReactElement<Record<string, unknown>>;

function findReactElement(tree: React.ReactNode, predicate: (element: TestElement) => boolean, message: string): TestElement {
  let found: TestElement | null = null;
  const visit = (node: React.ReactNode): void => {
    if (found || !React.isValidElement(node)) return;
    const element = node as TestElement;
    if (predicate(element)) {
      found = element;
      return;
    }
    React.Children.forEach(element.props.children as React.ReactNode, visit);
  };
  visit(tree);
  assert.ok(found, message);
  return found;
}

type StateSetter<T> = (next: T | ((current: T) => T)) => void;
type TestDispatcher = {
  useState<T>(initialState: T | (() => T)): [T, StateSetter<T>];
  useEffect(effect: React.EffectCallback, dependencies?: React.DependencyList): void;
};
type ReactInternals = { ReactCurrentDispatcher: { current: unknown } };

const dispatcherRef = (React as typeof React & { __SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED: ReactInternals })
  .__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED.ReactCurrentDispatcher;
const arenaStateSlots: unknown[] = [];
let arenaHookIndex = 0;
const arenaHookDispatcher = new Proxy({} as TestDispatcher, {
  get(_target, hookName) {
    if (hookName === "useState") {
      return <T,>(initialState: T | (() => T)): [T, StateSetter<T>] => {
        const slot = arenaHookIndex++;
        if (slot === arenaStateSlots.length) {
          arenaStateSlots.push(typeof initialState === "function" ? (initialState as () => T)() : initialState);
        }
        const setState: StateSetter<T> = (next) => {
          arenaStateSlots[slot] = typeof next === "function" ? (next as (current: T) => T)(arenaStateSlots[slot] as T) : next;
        };
        return [arenaStateSlots[slot] as T, setState];
      };
    }
    if (hookName === "useEffect") {
      return (_effect: React.EffectCallback, _dependencies?: React.DependencyList): void => {
        // A server-style tree render must retain hook order but never commit effects.
        arenaHookIndex++;
      };
    }
    throw new Error(`arena interaction test does not support React hook ${String(hookName)}`);
  },
});

function renderArenaWithTestHooks(): React.ReactElement {
  const previousDispatcher = dispatcherRef.current;
  dispatcherRef.current = arenaHookDispatcher;
  arenaHookIndex = 0;
  try {
    const compactTree = CompactMatchDetail({ detail: arenaDetail, loading: false, error: null, matchId: arenaDetail.matchId, ...navigationProps });
    const ArenaDetail = compactTree.type as (props: Record<string, unknown>) => React.ReactElement;
    return ArenaDetail(compactTree.props as Record<string, unknown>);
  } finally {
    dispatcherRef.current = previousDispatcher;
  }
}

const initialArenaTree = renderArenaWithTestHooks();
const roundSwitch = findReactElement(
  initialArenaTree,
  (element) =>
    element.type === "button" &&
    String(element.props.className).includes("phase7ArenaTab win") &&
    typeof element.props.onClick === "function",
  "arena detail should expose a clickable first-round switch",
);
(roundSwitch.props.onClick as () => void)();
const selectedRoundTree = renderArenaWithTestHooks();
const selectedRoundHeader = findReactElement(
  selectedRoundTree,
  (element) => typeof element.type === "function" && element.type.name === "MatchDetailHeader",
  "selected arena round should render through the shared detail header",
);
assert.equal(selectedRoundHeader.props.mapName, "地图：第 1 回合地图", "round selection should update the header map");
assert.equal(selectedRoundHeader.props.score, "1 - 0", "round selection should update the header score");
const selectedRoundTeamTable = findReactElement(
  selectedRoundTree,
  (element) => typeof element.type === "function" && element.type.name === "MatchTeamTable" && element.props.title === "队友数据",
  "selected arena round should keep the teammate table",
);
assert.equal(
  (selectedRoundTeamTable.props.players as PlayerRow[])[0]?.name,
  "第 1 回合队友",
  "round selection should switch the displayed player data",
);
