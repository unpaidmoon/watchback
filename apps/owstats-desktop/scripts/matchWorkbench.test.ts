import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { CompactMatchDetail } from "../src/features/matches/CompactMatchDetail";
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
assert.equal(matchesPageSource.includes("const previousMatchId = selectedMatchIndex > 0"), true, "detail navigation should derive the previous visible match");
assert.equal(matchesPageSource.includes("const nextMatchId = selectedMatchIndex >= 0"), true, "detail navigation should derive the next visible match");
assert.equal(matchesPageSource.includes("hasPrevious={previousMatchId !== null}"), true, "detail navigation should expose the previous boundary");
assert.equal(matchesPageSource.includes("hasNext={nextMatchId !== null}"), true, "detail navigation should expose the next boundary");
assert.equal(matchesPageSource.includes("mobileDetailOpen"), true, "MatchesPage should own the narrow detail-entry state");
assert.equal(matchesPageSource.includes("setMobileDetailOpen(true)"), true, "selecting a match should open narrow detail");
assert.equal(detailPaneSource.includes("onBack"), true, "detail pane should expose narrow back navigation");
assert.equal(detailPaneSource.includes("matchDetailBackButton"), true, "detail pane should render a real narrow back button");
assert.equal(detailPaneSource.includes("CompactMatchDetail"), true, "MatchDetailPane should own compact detail rendering");
assert.equal(detailPaneSource.includes('"onRetry"'), true, "MatchDetailPane should forward the retry contract");
assert.equal(detailPaneSource.includes("选择左侧对局查看详情。"), true, "MatchDetailPane should own the unchanged empty state");
assert.equal(detailHeaderSource.includes("上一场"), true, "the shared detail header should keep previous navigation visible");
assert.equal(detailHeaderSource.includes("下一场"), true, "the shared detail header should keep next navigation visible");
assert.equal(detailHeaderSource.includes("disabled={!hasPrevious}"), true, "previous navigation should remain visibly disabled at the boundary");
assert.equal(detailHeaderSource.includes("disabled={!hasNext}"), true, "next navigation should remain visibly disabled at the boundary");
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
assert.match(matchStyleSource, /grid-template-columns:\s*minmax\([^)]*,\s*32fr\)\s+minmax\(0,\s*68fr\)/);
assert.match(matchStyleSource, /@media\s*\(max-width:\s*1099px\)/);
assert.match(matchStyleSource, /\.matchesWorkbench\.detailOpen\s+\.matchListPane\s*\{[^}]*display:\s*none/s);
assert.match(matchStyleSource, /\.matchesWorkbench:not\(\.detailOpen\)\s+\.matchDetailPane\s*\{[^}]*display:\s*none/s);
assert.match(matchStyleSource, /\.matchListPane,\s*\.matchDetailPane\s*\{[^}]*overflow-y:\s*auto/s);
assert.match(matchStyleSource, /\.matchListPane \.phase6Timeline,\s*\.matchDetailPane \.phase6CompactDetailScroll\s*\{[^}]*overflow:\s*visible/s);
assert.match(matchStyleSource, /\.filterBar\.phase6FilterBar\s*\{[^}]*background:\s*var\(--surface-secondary\)/s);
assert.match(matchStyleSource, /\.activeFilterChips button\s*\{[^}]*border-radius:\s*999px/s);
assert.match(matchStyleSource, /\.matchDetailPerformance \.phase6Eyebrow\s*\{[^}]*color:\s*var\(--data-primary\)/s);
assert.match(matchStyleSource, /\.phase7CompactArenaDetail \.phase7ArenaTab\.active\s*\{[^}]*background:\s*var\(--data-primary\)/s);
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

assert.ok(
  compactDetailSource.indexOf("MatchDetailHeader") < compactDetailSource.indexOf("本人表现") &&
    compactDetailSource.indexOf("本人表现") < compactDetailSource.indexOf("队伍对比") &&
    compactDetailSource.indexOf("队伍对比") < compactDetailSource.indexOf('title="队友数据"') &&
    compactDetailSource.indexOf('title="队友数据"') < compactDetailSource.indexOf('title="对手数据"'),
  "normal detail should keep the agreed information order",
);

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
  rawJson: null,
};

const navigationProps = {
  hasPrevious: false,
  hasNext: true,
  onPrevious: () => undefined,
  onNext: () => undefined,
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
assert.match(normalMarkup, />本人表现</);
assert.match(normalMarkup, />队友数据</);
assert.match(normalMarkup, />对手数据</);

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
    rounds: [],
  },
};
const arenaMarkup = renderToStaticMarkup(
  createElement(CompactMatchDetail, { detail: arenaDetail, loading: false, error: null, matchId: arenaDetail.matchId, ...navigationProps }),
);
assert.match(arenaMarkup, /aria-label="角斗对局详情"/);
