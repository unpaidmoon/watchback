# Review and Achievements Navigation Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge overview and teammate collaboration into one continuous “战报” page, move achievements to the independent `/achievements` primary page, remove the review secondary menu, and reorder the top capsule navigation to `对局 | 战报 | 成就 | 排行 | 生涯 MMR | 指挥台` without changing backend behavior.

**Architecture:** Keep `PersonalReviewPage` responsible for personal-review plus season-analysis composition, add an `AchievementsPage` that requests only personal-review data, and share the personal-review season/request state through a focused hook. Extract the existing achievement wall UI into a reusable review feature component, normalize legacy `view` URLs at the route boundary, and keep friend-detail return state pointed at the unified report URL.

**Tech Stack:** React 18, TypeScript 5.7, React Router 6, CSS, Node/tsx assertion tests, Vite.

---

## File structure

**Create:**

- `apps/owstats-desktop/src/features/review/usePersonalSeasonReview.ts` — shared URL-season parsing, latest-request guard, dynamic season fallback, personal-review loading/error state, retry, and season changes.
- `apps/owstats-desktop/src/features/review/AchievementWall.tsx` — existing achievement wall cards, evidence links, progress, and accessible detail dialog.
- `apps/owstats-desktop/src/pages/AchievementsPage.tsx` — independent route-level achievements page and its header/loading/error/empty states.

**Modify:**

- `apps/owstats-desktop/src/shared/phase6Shell.ts` — exact primary navigation order and `/achievements` item.
- `apps/owstats-desktop/src/App.tsx` — `/achievements` route and legacy review route wrapper.
- `apps/owstats-desktop/src/features/review/reviewNavigation.ts` — canonical report/achievement URLs, legacy `view` normalization, and unified friend return context.
- `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx` — remove secondary navigation and render role/hero, teammate collaboration, and map analysis in order.
- `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx` — remove obsolete `ReviewView`/`sourceView` plumbing while preserving modules and detail navigation.
- `apps/owstats-desktop/src/styles/features/review.css` — remove tab/data-view selectors and scope the continuous report flow and independent achievement page.
- Frontend scripts listed below — update route, navigation, component-source, multi-season, visual fixture, and accessibility contracts.

No Rust, Tauri, SQLite, DTO, API, asset-manifest, or runtime-data file belongs in this plan.

### Task 1: Lock the primary navigation contract

**Files:**

- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts:21-23`
- Modify: `apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts:9`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.test.ts:32-37`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts:20-29`
- Modify: `apps/owstats-desktop/src/shared/phase6Shell.ts:17-23`

- [ ] **Step 1: Change the navigation assertions first**

Use the same exact expected order in all shell tests:

```ts
const expectedPrimaryNav = "对局,战报,成就,排行,生涯 MMR,指挥台";
assertEqual(phase6NavItems.map((item) => item.label).join(","), expectedPrimaryNav, "phase 6 navigation labels");
```

In `uiVisualAcceptance.test.ts`, assert the array form:

```ts
assert.deepEqual(
  fixtureShell.navigation.map((item) => item.label),
  ["对局", "战报", "成就", "排行", "生涯 MMR", "指挥台"],
  "shell fixture covers the exact primary navigation order",
);
```

Update `fixtureShell.navigation` so “成就” uses `/achievements?season=23`, “排行” precedes “生涯 MMR”, and the existing synthetic fixture remains free of credential-like fields.

- [ ] **Step 2: Run the narrow tests and confirm they fail**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/rankingCommandCenterRedesign.test.ts
npx tsx scripts/uiVisualAcceptance.test.ts
```

Expected: each affected assertion fails because the current navigation is `对局,战报,生涯 MMR,排行,指挥台`.

- [ ] **Step 3: Implement the exact navigation order**

Replace `phase6NavItems` with:

```ts
export const phase6NavItems: Phase6NavItem[] = [
  { label: "对局", path: "/matches", title: "浏览和筛选对局" },
  { label: "战报", path: "/review", title: "个人赛季战报" },
  { label: "成就", path: "/achievements", title: "赛季成就与进度" },
  { label: "排行", path: "/ranking", title: "英雄与地图排行" },
  { label: "生涯 MMR", path: "/career-mmr", title: "本地估算的三职责生涯 MMR" },
  { label: "指挥台", path: "/onboarding", title: "登录、同步与数据状态" },
];
```

- [ ] **Step 4: Re-run the narrow tests**

Run the three commands from Step 2.

Expected: PASS with no assertion output.

- [ ] **Step 5: Commit only the navigation slice**

```powershell
git add -- apps/owstats-desktop/src/shared/phase6Shell.ts apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/scripts/rankingCommandCenterRedesign.test.ts apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts apps/owstats-desktop/scripts/uiVisualAcceptance.test.ts
git commit -m "feat: add achievements to primary navigation"
```

### Task 2: Define canonical review URLs and legacy compatibility

**Files:**

- Modify: `apps/owstats-desktop/scripts/reviewNavigation.test.ts`
- Modify: `apps/owstats-desktop/src/features/review/reviewNavigation.ts`

- [ ] **Step 1: Replace view-tab tests with canonical-route tests**

Test these exact cases:

```ts
import {
  buildAchievementsHref,
  buildFriendDetailState,
  buildReviewHref,
  legacyReviewRedirectHref,
  parseFriendDetailState,
  reviewScrollKey,
  scheduleReviewScrollRestore,
} from "../src/features/review/reviewNavigation";

assert.equal(buildReviewHref(23), "/review?season=23");
assert.equal(buildAchievementsHref(23), "/achievements?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=overview")), "/review?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=friends")), "/review?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=achievements")), "/achievements?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=maps")), "/review?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23&view=invalid")), "/review?season=23");
assert.equal(legacyReviewRedirectHref(new URLSearchParams("season=23")), null);

const detailState = buildFriendDetailState("private-id", 21, 640);
assert.deepEqual(detailState.returnTo, {
  href: "/review?season=21",
  scrollKey: "review-scroll:/review?season=21",
  scrollY: 640,
});
```

Keep the existing invalid-state and asynchronous scroll-restoration assertions.

- [ ] **Step 2: Run the navigation test and confirm it fails**

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewNavigation.test.ts
```

Expected: FAIL because `buildAchievementsHref` and `legacyReviewRedirectHref` do not exist and `buildReviewHref` still requires a view.

- [ ] **Step 3: Implement canonical helpers**

Add these helpers while retaining `scheduleReviewScrollRestore` unchanged. Keep the existing `ReviewView` and `parseReviewView` exports temporarily so the pre-merge report page still compiles; Task 5 removes them after all call sites disappear:

```ts
export function buildReviewHref(season: number, _legacyView?: ReviewView) {
  return `/review?season=${season}`;
}

export function buildAchievementsHref(season: number) {
  return `/achievements?season=${season}`;
}

export function legacyReviewRedirectHref(params: URLSearchParams) {
  const view = params.get("view");
  if (view === null) return null;
  const season = Number(params.get("season"));
  const hasSeason = Number.isInteger(season) && season > 0;
  if (view === "achievements") return hasSeason ? buildAchievementsHref(season) : "/achievements";
  return hasSeason ? buildReviewHref(season) : "/review";
}

export function buildReviewRankingHref(season: number) {
  return `/ranking?tab=maps&season=${season}`;
}

export function reviewScrollKey(href: string) {
  return `review-scroll:${href}`;
}
```

Change the friend-state builder and validator to the canonical report URL. Accept the old four-argument call temporarily so Tasks 3-4 remain buildable before the obsolete `sourceView` prop is removed:

```ts
export function buildFriendDetailState(
  bnetId: string,
  season: number,
  viewOrScrollY: ReviewView | number,
  legacyScrollY?: number,
): FriendDetailLocationState {
  const scrollY = typeof viewOrScrollY === "number" ? viewOrScrollY : (legacyScrollY ?? 0);
  const href = buildReviewHref(season);
  return { bnetId, season, returnTo: { href, scrollKey: reviewScrollKey(href), scrollY } };
}

// Inside parseFriendDetailState after URL parsing:
if (`${parsed.pathname}${parsed.search}` !== buildReviewHref(candidate.season)) return null;
if (scrollKey !== reviewScrollKey(href)) return null;
```

Delete `reviewViews`, `ReviewView`, `parseReviewView`, the optional `_legacyView` parameter, and the four-argument compatibility branch after their call sites are removed in Task 5.

- [ ] **Step 4: Run the navigation test**

```powershell
npx tsx scripts/reviewNavigation.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit the backward-compatible URL model**

```powershell
git add -- apps/owstats-desktop/src/features/review/reviewNavigation.ts apps/owstats-desktop/scripts/reviewNavigation.test.ts
git commit -m "refactor: canonicalize review navigation URLs"
```

### Task 3: Share personal-review season request state

**Files:**

- Create: `apps/owstats-desktop/src/features/review/usePersonalSeasonReview.ts`
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts:7-49`
- Modify: `apps/owstats-desktop/scripts/accountScopedFrontendCaches.test.ts:4`

- [ ] **Step 1: Point multi-season tests at the new module**

Change imports so the request-state helpers come from the feature hook:

```ts
import {
  buildReviewSeasonFallbackFromLatest,
  buildReviewSeasonFallbackParams,
  createLatestRequestGuard,
  hasReviewSeasonData,
  parseReviewSeasonParam,
  shouldRequestReviewSeason,
} from "../src/features/review/usePersonalSeasonReview";
```

Read the new hook source instead of assuming the logic remains inside `PersonalReviewPage.tsx`, and assert:

```ts
const reviewStateSource = readFileSync(new URL("../src/features/review/usePersonalSeasonReview.ts", import.meta.url), "utf8");
assert.match(reviewStateSource, /review\?\.availableSeasons \?\? \[\]/);
assert.match(reviewStateSource, /requestPersonalSeasonReviewOnce\(selectedSeason/);
assert.doesNotMatch(reviewStateSource, /getSeasonAnalysis/);
```

Import `requestPersonalSeasonReviewOnce` directly from `../src/shared/personalReviewRequestCache` in `accountScopedFrontendCaches.test.ts`; the public cache boundary already lives there.

- [ ] **Step 2: Run the affected tests and confirm the missing module failure**

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase8MultiSeasonPages.test.ts
npx tsx scripts/accountScopedFrontendCaches.test.ts
```

Expected: the multi-season test fails with module-not-found for `usePersonalSeasonReview.ts`.

- [ ] **Step 3: Create the shared hook**

Implement the hook with the existing guard and fallback semantics:

```ts
import { useCallback, useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { requestPersonalSeasonReviewOnce } from "../../shared/personalReviewRequestCache";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { PersonalSeasonReview } from "../../shared/types";

export function parseReviewSeasonParam(value: string | null) {
  if (!value) return undefined;
  const season = Number(value);
  return Number.isInteger(season) && season > 0 ? season : undefined;
}

export function createLatestRequestGuard() {
  let latestRequest = 0;
  return {
    begin: () => ++latestRequest,
    isCurrent: (request: number) => latestRequest === request,
    invalidate(request?: number) {
      if (request === undefined || latestRequest === request) latestRequest += 1;
    },
  };
}

export function hasReviewSeasonData(review: Pick<PersonalSeasonReview, "summary">) {
  return typeof review.summary.season === "number" && review.summary.season > 0;
}

export function shouldRequestReviewSeason(selectedSeason: number | undefined, loadedSeason: number | null) {
  return selectedSeason === undefined || selectedSeason !== loadedSeason;
}

export function buildReviewSeasonFallbackParams(current: URLSearchParams, backendSeason: number) {
  const next = new URLSearchParams(current);
  next.set("season", String(backendSeason));
  return next;
}

export function buildReviewSeasonFallbackFromLatest(latest: { current: URLSearchParams }, backendSeason: number) {
  return buildReviewSeasonFallbackParams(latest.current, backendSeason);
}

export function usePersonalSeasonReview() {
  const [params, setParams] = useSearchParams();
  const latestParamsRef = useRef(params);
  latestParamsRef.current = params;
  const requestedSeason = parseReviewSeasonParam(params.get("season"));
  const [selectedSeason, setSelectedSeason] = useState<number | undefined>(requestedSeason);
  const [review, setReview] = useState<PersonalSeasonReview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requestGuard = useRef(createLatestRequestGuard());
  const loadedSeasonRef = useRef<number | null>(null);

  useEffect(() => {
    if (requestedSeason === selectedSeason) return;
    requestGuard.current.invalidate();
    loadedSeasonRef.current = null;
    setSelectedSeason(requestedSeason);
    setReview(null);
    setError(null);
  }, [requestedSeason, selectedSeason]);

  const requestReview = useCallback(() => {
    if (!shouldRequestReviewSeason(selectedSeason, loadedSeasonRef.current)) return;
    const request = requestGuard.current.begin();
    setReview(null);
    setError(null);
    void requestPersonalSeasonReviewOnce(selectedSeason)
      .then((nextReview) => {
        if (!requestGuard.current.isCurrent(request)) return;
        setReview(nextReview);
        const backendSeason = nextReview.summary.season;
        loadedSeasonRef.current = typeof backendSeason === "number" && backendSeason > 0 ? backendSeason : null;
        if (typeof backendSeason === "number" && backendSeason > 0 && backendSeason !== selectedSeason) {
          setSelectedSeason(backendSeason);
          setParams(buildReviewSeasonFallbackFromLatest(latestParamsRef, backendSeason), { replace: true });
        }
      })
      .catch((cause) => {
        if (requestGuard.current.isCurrent(request)) setError(productErrorMessage(cause));
      });
    return () => requestGuard.current.invalidate(request);
  }, [selectedSeason, setParams]);

  useEffect(() => requestReview(), [requestReview]);

  const changeSeason = useCallback((season: number) => {
    setSelectedSeason(season);
    setReview(null);
    const next = new URLSearchParams(latestParamsRef.current);
    next.set("season", String(season));
    setParams(next);
  }, [setParams]);

  return {
    selectedSeason,
    review,
    error,
    availableSeasons: review?.availableSeasons ?? [],
    changeSeason,
    requestReview,
  };
}
```

If TypeScript rejects the predicate narrowing used by existing page code, retain the current explicit type-predicate return signature from `PersonalReviewPage.tsx` rather than weakening callers with casts.

- [ ] **Step 4: Run the focused tests**

```powershell
npx tsx scripts/phase8MultiSeasonPages.test.ts
npx tsx scripts/accountScopedFrontendCaches.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit the shared state boundary**

```powershell
git add -- apps/owstats-desktop/src/features/review/usePersonalSeasonReview.ts apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts apps/owstats-desktop/scripts/accountScopedFrontendCaches.test.ts
git commit -m "refactor: share personal review page state"
```

### Task 4: Extract the achievement wall and add the independent page

**Files:**

- Create: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Create: `apps/owstats-desktop/src/pages/AchievementsPage.tsx`
- Modify: `apps/owstats-desktop/src/App.tsx`
- Modify: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`
- Modify: `apps/owstats-desktop/scripts/uiVisualContracts.test.ts`

- [ ] **Step 1: Move source-contract assertions to the new ownership boundary**

Read the new feature and page files:

```ts
const achievementSource = readFileSync(new URL("../src/features/review/AchievementWall.tsx", import.meta.url), "utf8");
const achievementPageSource = readFileSync(new URL("../src/pages/AchievementsPage.tsx", import.meta.url), "utf8");
const achievementModelSource = readFileSync(new URL("../src/shared/reviewAchievements.ts", import.meta.url), "utf8");

for (const label of ["成就墙", "常胜将军", "胜率机器", "高光制造者", "战局核心", "招牌打法", "巅峰一战", "本命专精", "战术变阵"]) {
  assert.equal(
    achievementSource.includes(label) || achievementPageSource.includes(label) || achievementModelSource.includes(label),
    true,
    `achievement page should render: ${label}`,
  );
}
assert.match(achievementPageSource, /title="成就"/);
assert.match(achievementPageSource, /<AchievementWall review=\{review\}/);
assert.doesNotMatch(achievementPageSource, /getSeasonAnalysis/);
```

In `uiVisualContracts.test.ts`, include `AchievementsPage.tsx` in the pages required to render `PageHeader`. In `reviewVisualPolish.test.ts`, read focus-management assertions from `AchievementWall.tsx` instead of `PersonalReviewPage.tsx`.

- [ ] **Step 2: Run the source-contract tests and confirm they fail**

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewAchievementWall.test.ts
npx tsx scripts/reviewVisualPolish.test.ts
npx tsx scripts/uiVisualContracts.test.ts
```

Expected: FAIL because the new files and `/achievements` page do not exist.

- [ ] **Step 3: Extract the existing wall without behavior changes**

Create `AchievementWall.tsx` with this public boundary:

```tsx
import { ExternalLink, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { achievementBadgeUrl } from "../../shared/assets";
import { GlassPanel, HeroAvatar, MapPreview } from "../../shared/phase6Components";
import {
  activeBadgeLevel,
  buildReviewAchievements,
  formatThreshold,
  romanLevels,
  type ReviewAchievement,
} from "../../shared/reviewAchievements";
import type { EvidenceLink, PersonalSeasonReview } from "../../shared/types";

export function AchievementWall({ review }: { review: PersonalSeasonReview }) {
  const [selectedAchievement, setSelectedAchievement] = useState<ReviewAchievement | null>(null);
  const achievements = buildReviewAchievements(review);
  return (
    <>
      <GlassPanel className="phase6AchievementPanel" tone="solid">
        <header className="phase6AchievementHeader"><div><h3>成就墙</h3></div></header>
        <div className="phase6AchievementWall">
          {achievements.map((achievement) => (
            <AchievementCard key={achievement.key} achievement={achievement} onOpen={() => setSelectedAchievement(achievement)} />
          ))}
        </div>
      </GlassPanel>
      {selectedAchievement ? <AchievementDialog achievement={selectedAchievement} onClose={() => setSelectedAchievement(null)} /> : null}
    </>
  );
}
```

Move the existing `AchievementCard`, `AchievementDialog`, and `Evidence` JSX bodies from `PersonalReviewPage.tsx:549-677` unchanged. Replace their local helper calls with the already exported `activeBadgeLevel`, `formatThreshold`, and `romanLevels` imports from `shared/reviewAchievements.ts`. Keep the current focus transfer, Escape behavior, Tab focus constraint, backdrop close, evidence route, asset lookup, Chinese labels, and CSS class names byte-for-byte.

Temporarily keep the old achievement tab functional by importing `AchievementWall` in `PersonalReviewPage.tsx` and replacing its branch with:

```tsx
{view === "achievements" && <AchievementWall review={review} />}
```

At the same time remove `selectedAchievement`, `AchievementCard`, `AchievementDialog`, `Evidence`, the local achievement types/builders, and their now-unused icon/asset imports from `PersonalReviewPage.tsx`. Task 5 deletes the entire legacy branch after the independent route is proven.

- [ ] **Step 4: Create `AchievementsPage` using only personal-review state**

Use this route-level structure:

```tsx
import { useNavigate } from "react-router-dom";
import { AchievementWall } from "../features/review/AchievementWall";
import { hasReviewSeasonData, usePersonalSeasonReview } from "../features/review/usePersonalSeasonReview";
import { PageEmptyState, PageErrorState, PageHeader, PageLoadingState, PageSeasonSelect } from "../shared/page-shell";

export function AchievementsPage() {
  const navigate = useNavigate();
  const { selectedSeason, review, error, availableSeasons, changeSeason, requestReview } = usePersonalSeasonReview();
  const titleSeason = selectedSeason ?? review?.summary.season;
  const header = (
    <PageHeader
      title="成就"
      description={titleSeason ? `S${titleSeason} 赛季成就与进度` : "查看赛季成就与进度"}
      actions={<PageSeasonSelect value={selectedSeason} seasons={availableSeasons} disabled={availableSeasons.length === 0} onChange={(season) => season !== undefined && changeSeason(season)} />}
    />
  );

  return (
    <section className="workspace reviewWorkspace achievementsWorkspace phase6ReviewWorkspace">
      {header}
      {error ? <PageErrorState message={error} onRetry={() => void requestReview()} /> : null}
      {!error && !review ? <PageLoadingState message="正在加载赛季成就..." /> : null}
      {!error && review && !hasReviewSeasonData(review) ? (
        <PageEmptyState title="暂无可用赛季成就" description="当前本地数据库没有可用于生成成就的赛季数据。" actionLabel="前往指挥台" onAction={() => navigate("/onboarding")} />
      ) : null}
      {!error && review && hasReviewSeasonData(review) ? <AchievementWall review={review} /> : null}
    </section>
  );
}
```

- [ ] **Step 5: Register the independent route**

Add the import and route in `App.tsx`:

```tsx
import { AchievementsPage } from "./pages/AchievementsPage";

<Route path="/achievements" element={<AchievementsPage />} />
```

- [ ] **Step 6: Run the focused tests and TypeScript build**

```powershell
npx tsx scripts/reviewAchievementWall.test.ts
npx tsx scripts/reviewVisualPolish.test.ts
npx tsx scripts/uiVisualContracts.test.ts
npm run build
```

Expected: PASS. `AchievementsPage.tsx` must not import or call `getSeasonAnalysis`.

- [ ] **Step 7: Commit the independent achievement page**

```powershell
git add -- apps/owstats-desktop/src/features/review/AchievementWall.tsx apps/owstats-desktop/src/pages/AchievementsPage.tsx apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/src/App.tsx apps/owstats-desktop/scripts/reviewAchievementWall.test.ts apps/owstats-desktop/scripts/reviewVisualPolish.test.ts apps/owstats-desktop/scripts/uiVisualContracts.test.ts
git commit -m "feat: add independent achievements page"
```

### Task 5: Merge the report flow and remove all secondary navigation

**Files:**

- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- Modify: `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx`
- Modify: `apps/owstats-desktop/src/App.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/review.css`
- Modify: `apps/owstats-desktop/scripts/analysisMovedToReview.test.ts`
- Modify: `apps/owstats-desktop/scripts/reviewTrimmedSections.test.ts`
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`

- [ ] **Step 1: Write the final report-composition assertions**

Replace the tab assertions in `analysisMovedToReview.test.ts` with:

```ts
assert.doesNotMatch(reviewSource, /reviewTabs|ReviewTabButton|parseReviewView|changeView/);
assert.match(
  reviewSource,
  /<div className="reviewReportFlow">[\s\S]*?<RoleAndHeroView review=\{review\}[\s\S]*?<FriendAnalysisModules[\s\S]*?<section className="reviewMapAnalysis">[\s\S]*?<MapAnalysisModules/,
  "战报应按段位总览、队友协作、地图分析的顺序组合",
);
assert.doesNotMatch(reviewSource, /AchievementWall|phase6AchievementPanel/);
assert.match(reviewSource, /<ReviewScrollRestorer season=\{titleSeason\}/);
```

Update `reviewVisualPolish.test.ts` to assert `.reviewReportFlow { display: grid; align-items: start; }` and remove the obsolete `[data-view="overview"]` assertion. Change the visual fixture route from `/review?season=23&view=overview` to `/review?season=23`.

- [ ] **Step 2: Run the affected tests and confirm they fail**

```powershell
cd apps\owstats-desktop
npx tsx scripts/analysisMovedToReview.test.ts
npx tsx scripts/reviewTrimmedSections.test.ts
npx tsx scripts/reviewVisualPolish.test.ts
npx tsx scripts/phase8MultiSeasonPages.test.ts
```

Expected: FAIL on the old tabs, old data-view selector, and old source ownership.

- [ ] **Step 3: Refactor `PersonalReviewPage` to the shared hook**

Import:

```tsx
import { Navigate, useNavigate, useSearchParams } from "react-router-dom";
import { legacyReviewRedirectHref, buildReviewHref, reviewScrollKey, scheduleReviewScrollRestore } from "../features/review/reviewNavigation";
import { createLatestRequestGuard, hasReviewSeasonData, usePersonalSeasonReview } from "../features/review/usePersonalSeasonReview";
```

Add the route wrapper used by `App.tsx`:

```tsx
export function PersonalReviewRoute() {
  const [params] = useSearchParams();
  const redirect = legacyReviewRedirectHref(params);
  return redirect ? <Navigate to={redirect} replace /> : <PersonalReviewPage />;
}
```

Inside `PersonalReviewPage`, replace the personal-review request state with:

```tsx
const { selectedSeason, review, error, availableSeasons, changeSeason, requestReview } = usePersonalSeasonReview();
```

Keep the existing season-analysis guard/effect, keyed only by `selectedSeason`. When `selectedSeason` changes, clear `analysis`, `analysisError`, and `analysisLoading` before starting the new analysis request. Keep the existing page header, personal-review error/loading/empty states, and `titleSeason = selectedSeason ?? review.summary.season`.

- [ ] **Step 4: Render one continuous report flow**

After the header and scroll restorer, render exactly:

```tsx
<div className="reviewReportFlow">
  <RoleAndHeroView review={review} />
  <FriendAnalysisModules
    analytics={analysis?.friend ?? null}
    loading={analysisLoading}
    error={analysisError}
    season={titleSeason}
  />
  <section className="reviewMapAnalysis">
    <MapAnalysisModules analysis={analysis} loading={analysisLoading} error={analysisError} season={titleSeason} />
  </section>
</div>
```

Delete `reviewViewLabels`, `changeView`, `ReviewTabButton`, `ReviewOverview`, the selected-achievement state, and all achievement UI/helper code now owned by `AchievementWall.tsx`. Keep `RoleAndHeroView`, `RoleRankCard`, `Metric`, analysis loading/error behavior, and Chinese header copy.

Change the scroll restorer to:

```tsx
function ReviewScrollRestorer({ season }: { season: number }) {
  useLayoutEffect(() => {
    const href = buildReviewHref(season);
    const key = reviewScrollKey(href);
    // retain the existing stored-position validation and retry runtime unchanged
  }, [season]);
  return null;
}
```

- [ ] **Step 5: Remove obsolete friend view plumbing**

In `FriendAnalyticsPage.tsx`:

```tsx
import { buildFriendDetailState, buildReviewRankingHref } from "../features/review/reviewNavigation";
```

Remove `sourceView` from `SeasonAnalysisModules`, `FriendAnalysisModules`, and `FriendBoardCard` props. Build detail state with:

```ts
const scrollY = typeof window === "undefined" ? 0 : window.scrollY;
return buildFriendDetailState(friend.bnetId, season, scrollY);
```

This preserves the same detail route, private `bnetId` location state, session scroll key, and click behavior while returning to `/review?season=<season>`.

- [ ] **Step 6: Use the route wrapper and remove obsolete navigation types**

In `App.tsx`:

```tsx
import { PersonalReviewRoute } from "./pages/PersonalReviewPage";

<Route path="/review" element={<PersonalReviewRoute />} />
```

Finish Task 2 by deleting `ReviewView`, `reviewViews`, and `parseReviewView`, then confirm `rg -n "ReviewView|parseReviewView|sourceView|view=friends" apps/owstats-desktop/src` returns no matches.

- [ ] **Step 7: Replace obsolete CSS selectors**

Delete all `.reviewTabs` rules and replace `.reviewViewGrid`/`[data-view="overview"]` rules with:

```css
.reviewReportFlow {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  align-items: start;
  gap: var(--space-4);
  min-width: 0;
}

.reviewReportFlow > * {
  min-width: 0;
}

.reviewMapAnalysis {
  display: grid;
  gap: var(--space-3);
}

.achievementsWorkspace .phase6AchievementPanel {
  width: 100%;
}
```

Remove the obsolete 899px `[data-view="overview"]` and 640px `.reviewTabs` media rules. Keep the role/hero, friend, achievement, 1440px wall, 899px facts, and 640px role-grid rules unchanged.

- [ ] **Step 8: Run the focused regression set and build**

```powershell
npx tsx scripts/reviewNavigation.test.ts
npx tsx scripts/analysisMovedToReview.test.ts
npx tsx scripts/reviewTrimmedSections.test.ts
npx tsx scripts/reviewVisualPolish.test.ts
npx tsx scripts/phase8MultiSeasonPages.test.ts
npx tsx scripts/reviewAchievementWall.test.ts
npm run build
```

Expected: PASS. The build must contain `/review` and `/achievements` routes with no unresolved `ReviewView` imports.

- [ ] **Step 9: Commit the merged report slice**

Stage only:

```powershell
git add -- apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx apps/owstats-desktop/src/App.tsx apps/owstats-desktop/src/styles/features/review.css apps/owstats-desktop/src/features/review/reviewNavigation.ts apps/owstats-desktop/scripts/reviewNavigation.test.ts apps/owstats-desktop/scripts/analysisMovedToReview.test.ts apps/owstats-desktop/scripts/reviewTrimmedSections.test.ts apps/owstats-desktop/scripts/reviewVisualPolish.test.ts apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts
git commit -m "refactor: merge review report sections"
```

### Task 6: Full frontend and visual acceptance

**Files:**

- Modify only if a contract is stale: `apps/owstats-desktop/scripts/*.test.ts`
- Do not modify backend files to make frontend tests pass.

- [ ] **Step 1: Search for stale UI contracts**

```powershell
rg -n "对局,战报,生涯 MMR,排行,指挥台|view=overview|view=friends|view=achievements|reviewTabs|ReviewTabButton|reviewViewGrid|ReviewView|parseReviewView" apps/owstats-desktop/src apps/owstats-desktop/scripts
```

Expected: no production-source matches. Test matches are allowed only when explicitly asserting legacy URL redirection or absence of old UI.

- [ ] **Step 2: Run the complete frontend test suite**

```powershell
cd apps\owstats-desktop
npm test
```

Expected: all resource, unit, component, legacy, and text-integrity tests pass. If an existing test still reads achievement markup from `PersonalReviewPage.tsx`, update it to `AchievementWall.tsx` or `AchievementsPage.tsx`; do not reintroduce achievement markup into the report page.

- [ ] **Step 3: Run the production frontend build**

```powershell
npm run build
```

Expected: `tsc`, Vite build, and `verifyDistPaths.mjs` all exit 0; no development-machine absolute path is emitted.

- [ ] **Step 4: Perform anonymous visual checks**

Using the existing synthetic visual fixture only, inspect:

```text
2540×1344  /review?season=23
1920×1080  /review?season=23
1440×900   /review?season=23
2540×1344  /achievements?season=23
1920×1080  /achievements?season=23
1440×900   /achievements?season=23
```

Confirm the exact top order, independent active item, no review secondary menu, report order `段位总览 → 队友协作 → 地图分析`, one page scroll container, unchanged friend interactions, four-/two-column achievement separators, visible focus, and unchanged light achievement dialog.

- [ ] **Step 5: Run repository hygiene checks**

```powershell
cd D:\Codex\OW_data\owstats-local
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. Compare `git status --short` with the recorded dirty baseline; only the planned frontend files may be new changes from this implementation. Do not stage or alter the user's pre-existing `AGENTS.md`, backend, onboarding, match-favorite, command-center, maintenance, or spreadsheet changes.

- [ ] **Step 6: Commit any final test-only corrections**

Only when Step 2 required stale-contract updates:

```powershell
git add -- apps/owstats-desktop/scripts
git commit -m "test: align review navigation acceptance"
```

Before committing, use `git diff --cached --name-only` and unstage any unrelated script already modified before this work.

## Completion report

Report:

- exact files changed and commits created;
- `npm test`, `npm run build`, and `git diff --check` results;
- which target resolutions were visually inspected;
- any unverified Tauri packaging item (Tauri build is not required because the approved scope is frontend layout only);
- that no real credentials, browser profile, SQLite, private IDs, or real data dump were read or emitted.
