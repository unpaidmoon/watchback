# Matches Workbench Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按 `docs/superpowers/specs/2026-07-14-ui-architecture-visual-redesign-design.md` 的对局切片重构 `/matches`，同时严格保留现有 URL、页面内赛季、缓存、选中详情、滚动恢复以及普通/角斗数据语义。

**Architecture:** `MatchesPage` 继续只协调 URL、内存缓存、请求、选中项和窄屏进入状态；筛选工具栏、有效筛选标签、列表面板、列表卡、详情面板、详情头和队伍表格下沉到 `features/matches/`。先做声明级 CSS 抽取和状态正确性修复，再逐层替换展示组件；不修改后端、SQLite、同步协议或全局赛季状态。

**Tech Stack:** React 18、TypeScript 5、React Router 6、Tauri 2 前端 API、CSS、Node `assert` + `tsx` 源码/SSR 契约测试、Windows Edge headless 视觉夹具。

---

## Execution constraints and file map

- 在隔离 worktree 中执行；开始每个任务前运行 `git status --short`，不得清理用户改动。
- 不读取 app data、凭据、真实数据 dump、浏览器 profile 或原始玩家隐私 ID。视觉夹具只能使用本计划给出的中文匿名值。
- Slice 1 必须先提供 `apps/owstats-desktop/src/shared/page-shell/PageHeader.tsx` 与 `PageSeasonSelect.tsx`；本计划只消费这两个共享组件，不复制其实现。
- 保留 `filtersFromParams`、`paramsFromFilters`、`matchFiltersKey`、`preloadMatchesPageCache`、滚动锚点算法和 `detail.arenaDetail` 分流。
- 保留角斗汇总/回合切换、`showPerks={false}`、行点击展开、汇总/回合 economy mode、4 个 `traitGuids` 槽、6 个 `modGuids` 槽以及未映射 GUID 原值 fallback。

**Create:**

- `apps/owstats-desktop/src/features/matches/MatchFilterToolbar.tsx`：高频筛选和高级筛选入口。
- `apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx`：英雄、地图、复盘标签和日期。
- `apps/owstats-desktop/src/features/matches/ActiveFilterChips.tsx`：有效筛选摘要与单项移除。
- `apps/owstats-desktop/src/features/matches/MatchListPane.tsx`：列表滚动、空状态和恢复 ref。
- `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`：单张对局卡。
- `apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx`：嵌入详情面板与窄屏返回动作。
- `apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx`：结果、地图、模式、比分、时间和前后场切换。
- `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`：普通/角斗共用的中性玩家表格外观。
- `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`：生成匿名静态视觉夹具到指定临时目录。
- `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts`：夹具状态与隐私检查。

**Modify:**

- `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- `apps/owstats-desktop/src/shared/matchesPageCache.ts`
- `apps/owstats-desktop/src/shared/matchListFormat.ts`
- `apps/owstats-desktop/src/features/matches/matchFilters.ts`
- `apps/owstats-desktop/src/features/matches/FilterBar.tsx`
- `apps/owstats-desktop/src/features/matches/MatchTimeline.tsx`
- `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- `apps/owstats-desktop/src/styles/shared.css`
- `apps/owstats-desktop/src/styles/features/matches.css`
- `apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts`
- `apps/owstats-desktop/scripts/matchesPageCache.test.ts`
- `apps/owstats-desktop/scripts/filterBarUi.test.ts`
- `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- `apps/owstats-desktop/scripts/matchListFormat.test.ts`
- `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`
- `apps/owstats-desktop/package.json`

### Task 1: Establish the new match feature CSS seam without migrating legacy declarations

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts`

- [ ] **Step 1: Write the failing ownership test**

Append to `stab006ModuleContracts.test.ts`:

```ts
const matchesCss = readFileSync(join(root, "styles", "features", "matches.css"), "utf8");
for (const selector of [
  ".matchesPage",
  ".matchesWorkbench",
  ".matchListPane",
  ".matchListCard",
  ".matchDetailPane",
  ".matchTeamTable",
]) {
  assert.ok(matchesCss.includes(selector), `${selector} is owned by matches.css`);
}
```

Do not assert that legacy `.phase6*` or `.phase7*` selectors leave `shared.css`; maintenance policy requires those declarations to stay until a separately approved declaration-only migration with visual regression evidence.

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/stab006ModuleContracts.test.ts`

Expected: FAIL with `.matchesPage is owned by matches.css` because `matches.css` is still the one-line placeholder.

- [ ] **Step 3: Add only the new redesign selectors**

Replace the placeholder with the minimal feature seam that later tasks will extend:

```css
.matchesPage,
.matchesWorkbench,
.matchListPane,
.matchListCard,
.matchDetailPane,
.matchTeamTable {
  min-width: 0;
}

.matchListCard,
.matchDetailPane,
.matchTeamTable {
  border: 1px solid var(--border-subtle);
  background: var(--surface-secondary);
}
```

Do not move, delete, or rewrite declarations in `shared.css` during this task. Subsequent tasks use the new classes alongside legacy classes and add narrowly scoped overrides in `features/matches.css`.

- [ ] **Step 4: Run GREEN and focused regressions**

Run: `cd apps/owstats-desktop; npx tsx scripts/stab006ModuleContracts.test.ts; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/arenaDetailFormat.test.ts`

Expected: all three commands exit 0; existing match visuals and arena semantics remain untouched.

- [ ] **Step 5: Check diff and commit**

Run: `git diff --check; git status --short`

Expected: only `matches.css` and `stab006ModuleContracts.test.ts` are modified and `git diff --check` is silent.

```powershell
git add apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts
git commit -m "style: establish match feature css seam"
```

### Task 2: Restore selected match from cache and prevent stale detail rendering

**Files:**
- Modify: `apps/owstats-desktop/src/shared/matchesPageCache.ts`
- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/scripts/matchesPageCache.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: Write RED tests for selection precedence and stale detail**

Add to `matchesPageCache.test.ts`:

```ts
updateMatchesPageCache({ selectedMatchId: "cached-match" });
assert.equal(getMatchesPageCache()?.selectedMatchId, "cached-match");
assert.equal(chooseInitialMatchId("url-match", "cached-match", true), "url-match");
assert.equal(chooseInitialMatchId(null, "cached-match", true), "cached-match");
assert.equal(chooseInitialMatchId(null, "cached-match", false), null);
```

Add to `matchWorkbench.test.ts`:

```ts
assert.match(
  matchesPageSource,
  /setDetailLoading\(true\);\s*setSelectedDetail\(null\);\s*setDetailError\(null\);/s,
  "selection changes must clear stale detail before requesting the next match",
);
```

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchesPageCache.test.ts`

Expected: TypeScript fails because `selectedMatchId` and `chooseInitialMatchId` do not exist.

- [ ] **Step 3: Implement the minimal cache contract**

In `matchesPageCache.ts` add:

```ts
export type MatchesPageCache = {
  // existing fields stay unchanged
  selectedMatchId: string | null;
};

export function chooseInitialMatchId(
  urlMatchId: string | null,
  cachedMatchId: string | null | undefined,
  canHydrateCache: boolean,
) {
  return urlMatchId ?? (canHydrateCache ? cachedMatchId ?? null : null);
}
```

Set the cache default to `selectedMatchId: null`. In `MatchesPage`, initialize selection with URL precedence:

```ts
const [selectedMatchId, setSelectedMatchId] = useState<string | null>(() =>
  chooseInitialMatchId(searchParams.get("matchId"), cachedPage?.selectedMatchId, canHydrateCache),
);
```

Include `selectedMatchId` in the existing cache update effect. In the detail request effect, clear the previous object before starting the request:

```ts
setDetailLoading(true);
setSelectedDetail(null);
setDetailError(null);
void getMatchDetails(selectedMatchId)
```

Do not cache `MatchDetailView`; returning to a selection must refetch through the existing sanitized API.

- [ ] **Step 4: Run GREEN and URL/scroll regressions**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchesPageCache.test.ts; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/stab006ModuleContracts.test.ts; npx tsx scripts/phase8MultiSeasonPages.test.ts`

Expected: all commands exit 0; existing filter-key and scroll-anchor assertions remain unchanged.

- [ ] **Step 5: Check diff and commit**

Run: `git diff --check; git status --short`

Expected: only the four task files are modified.

```powershell
git add apps/owstats-desktop/src/shared/matchesPageCache.ts apps/owstats-desktop/src/pages/MatchesPage.tsx apps/owstats-desktop/scripts/matchesPageCache.test.ts apps/owstats-desktop/scripts/matchWorkbench.test.ts
git commit -m "fix: restore match selection safely"
```

### Task 3: Establish page-level match component boundaries without behavior changes

**Files:**
- Create: `apps/owstats-desktop/src/features/matches/MatchListPane.tsx`
- Create: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
- Create: `apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/MatchTimeline.tsx`
- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: Write the failing module-boundary test**

In `stab006ModuleContracts.test.ts`, read the three new files and assert:

```ts
for (const file of ["MatchListPane.tsx", "MatchListCard.tsx", "MatchDetailPane.tsx"]) {
  assert.ok(existsSync(join(root, "features", "matches", file)), `${file} must exist`);
}
assert.match(matchesSource, /<MatchListPane/);
assert.match(matchesSource, /<MatchDetailPane/);
assert.doesNotMatch(matchesSource, /<MatchTimeline/);
assert.doesNotMatch(matchesSource, /<CompactMatchDetail/);
```

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/stab006ModuleContracts.test.ts`

Expected: FAIL with `MatchListPane.tsx must exist`.

- [ ] **Step 3: Extract the smallest behavior-preserving components**

`MatchListCard` receives one `MatchSummary`, selected state and `onSelect`; move the current button JSX and `MetricBlock` from `MatchTimeline` unchanged. `MatchTimeline` retains streak segmentation and maps each item to `MatchListCard`.

```ts
export type MatchListPaneProps = {
  matches: MatchSummary[];
  selectedMatchId: string | null;
  onSelect: (matchId: string) => void;
  onScroll: () => void;
};

export const MatchListPane = forwardRef<HTMLDivElement, MatchListPaneProps>(
  function MatchListPane(props, ref) {
    return <MatchTimeline ref={ref} {...props} />;
  },
);
```

```ts
export function MatchDetailPane(props: {
  detail: MatchDetailView | null;
  loading: boolean;
  error: string | null;
  matchId: string | null;
}) {
  return (
    <aside className="matchDetailPane phase6WorkbenchDetail" aria-label="对局详情">
      {props.matchId ? <CompactMatchDetail {...props} matchId={props.matchId} /> :
        <div className="emptyState phase6WorkbenchEmpty">选择左侧对局查看详情。</div>}
    </aside>
  );
}
```

`MatchesPage` keeps the same refs, callbacks and URL/cache/request effects; only replace its list/detail JSX with these components.

- [ ] **Step 4: Run GREEN and regression tests**

Run: `cd apps/owstats-desktop; npx tsx scripts/stab006ModuleContracts.test.ts; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/matchesPageCache.test.ts; npx tsc --noEmit`

Expected: all commands exit 0.

- [ ] **Step 5: Check diff and commit**

Run: `git diff --check; git status --short`

```powershell
git add apps/owstats-desktop/src/features/matches/MatchListPane.tsx apps/owstats-desktop/src/features/matches/MatchListCard.tsx apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx apps/owstats-desktop/src/features/matches/MatchTimeline.tsx apps/owstats-desktop/src/pages/MatchesPage.tsx apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts apps/owstats-desktop/scripts/matchWorkbench.test.ts
git commit -m "refactor: split match workbench panes"
```

### Task 4: Move season to the page header and add frequent, advanced, chip, and clear filters

**Files:**
- Create: `apps/owstats-desktop/src/features/matches/MatchFilterToolbar.tsx`
- Create: `apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx`
- Create: `apps/owstats-desktop/src/features/matches/ActiveFilterChips.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/FilterBar.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/matchFilters.ts`
- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/scripts/filterBarUi.test.ts`
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- Modify: `apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts`

- [ ] **Step 1: Write RED tests for filter semantics**

Export and test these pure helpers in `filterBarUi.test.ts`:

```ts
assert.equal(countAdvancedMatchFilters({ hero: "ana", map: "ilios", result: "win" }), 2);
assert.deepEqual(clearVisibleMatchFilters({ season: 23, hero: "ana", result: "win" }), { season: 23 });
assert.deepEqual(removeMatchFilter({ season: 23, hero: "ana", result: "win" }, "hero"), { season: 23, result: "win" });
assert.deepEqual(
  activeMatchFilterChips(
    { hero: "ana", dateFrom: "2026-07-01" },
    { seasons: [], heroes: [{ id: "ana", label: "安娜" }], maps: [], results: [], modes: [] },
  ).map((chip) => chip.label),
  ["英雄：安娜", "开始日期：2026-07-01"],
);
```

Update `phase8MultiSeasonPages.test.ts` so it requires `PageSeasonSelect` in `MatchesPage`, forbids season markup in `FilterBar`, and keeps the existing `paramsFromFilters(filters, selectedMatchId)` assertion.

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/filterBarUi.test.ts`

Expected: import failure for `countAdvancedMatchFilters`.

- [ ] **Step 3: Implement pure helpers and presentation components**

In `matchFilters.ts` define advanced keys exactly once:

```ts
export const advancedMatchFilterKeys = ["hero", "map", "primaryTag", "dateFrom", "dateTo"] as const;
export type VisibleMatchFilterKey = Exclude<keyof MatchFilters, "season">;

export function countAdvancedMatchFilters(filters: MatchFilters) {
  return advancedMatchFilterKeys.filter((key) => Boolean(filters[key])).length;
}

export function clearVisibleMatchFilters(filters: MatchFilters): MatchFilters {
  return filters.season === undefined ? {} : { season: filters.season };
}

export function removeMatchFilter(filters: MatchFilters, key: VisibleMatchFilterKey): MatchFilters {
  const next = { ...filters };
  delete next[key];
  return next;
}
```

`activeMatchFilterChips` returns stable labels in this order: `mode`, `result`, `playerName`, `hero`, `map`, `primaryTag`, `dateFrom`, `dateTo`. It uses option labels, Chinese result labels and `modeLabel`; it never displays raw GUID fallback.

`MatchFilterToolbar` renders only 模式、胜负、玩家名称、“更多筛选 N”和“清除”. `AdvancedMatchFilters` renders 英雄、地图、复盘标签、开始日期、结束日期 with visible `<label>`. `ActiveFilterChips` renders one Chinese remove button per descriptor.

In `MatchesPage`, consume the Slice 1 shared components:

```tsx
<PageHeader
  title="对局记录"
  description={`${matches.length} 场本地对局`}
  actions={
    <PageSeasonSelect
      value={filters.season}
      seasons={options.seasons}
      includeAll
      allLabel="全部赛季"
      onChange={(season) => setFilters((current) => {
        const next = { ...current };
        if (season === undefined) delete next.season;
        else next.season = season;
        return next;
      })}
    />
  }
/>
```

Clearing filters must preserve `season`; only the page-level season selector may explicitly choose “全部赛季” and remove it. Every change still flows through `setFilters`, the existing URL effect and `listMatchTimeline`.

- [ ] **Step 4: Run GREEN and URL regressions**

Run: `cd apps/owstats-desktop; npx tsx scripts/filterBarUi.test.ts; npx tsx scripts/phase8MultiSeasonPages.test.ts; npx tsx scripts/stab006ModuleContracts.test.ts; npx tsx scripts/matchesPageCache.test.ts; npx tsc --noEmit`

Expected: all commands exit 0.

- [ ] **Step 5: Check diff and commit**

Run: `git diff --check; git status --short`

```powershell
git add apps/owstats-desktop/src/features/matches/MatchFilterToolbar.tsx apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx apps/owstats-desktop/src/features/matches/ActiveFilterChips.tsx apps/owstats-desktop/src/features/matches/FilterBar.tsx apps/owstats-desktop/src/features/matches/matchFilters.ts apps/owstats-desktop/src/pages/MatchesPage.tsx apps/owstats-desktop/scripts/filterBarUi.test.ts apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts
git commit -m "feat: restructure match filters"
```

### Task 5: Separate result and score and use grouped list statistics

**Files:**
- Modify: `apps/owstats-desktop/src/shared/matchListFormat.ts`
- Modify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/matchListFormat.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: Flip the old formatting tests to the new contract**

In `matchListFormat.test.ts` require:

```ts
assert.equal(formatScoreLine(baseMatch), "3-0");
assert.equal(formatMatchBannerTitle(baseMatch), "苏拉瓦萨 · 闪点作战");
assert.equal(formatCompactStat(5960), "5,960");
assert.equal(formatCompactStat(15000), "15,000");
assert.equal(formatCompactStat(10000), "10,000");
```

In `matchWorkbench.test.ts` require distinct result and score classes, `match.heroName`, `match.beginLabel`, and `kills/deaths/assists` order.

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchListFormat.test.ts`

Expected: FAIL showing actual `胜：3-0` instead of `3-0`.

- [ ] **Step 3: Implement the minimal formatter and card markup**

```ts
const groupedNumber = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 0 });

export function formatScoreLine(match: MatchSummary) {
  return `${match.teamScore}-${match.opponentScore}`;
}

export function formatMatchBannerTitle(match: MatchSummary) {
  return `${match.mapName} · ${match.modeLabel}`;
}

export function formatCompactStat(value: number) {
  return groupedNumber.format(value);
}
```

`MatchListCard` must render result text and semantic icon/marker in `.matchListCardResult`, score alone in `.matchListCardScore`, `mapName · modeLabel`, `beginLabel`, hero avatar plus `heroName`, and these columns:

```tsx
<MetricBlock label="K/D/A" value={`${match.kills}/${match.deaths}/${match.assists}`} />
<MetricBlock label="伤害" value={formatCompactStat(match.damage)} />
<MetricBlock label="承伤" value={formatCompactStat(match.mitigation)} />
```

Keep streak and evaluation labels secondary. Selected cards use a pale background and purple border token; result colors use semantic success/failure/neutral tokens, not the selection token.

- [ ] **Step 4: Run GREEN and regressions**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchListFormat.test.ts; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/matchesPageCache.test.ts; npx tsc --noEmit`

Expected: all commands exit 0.

- [ ] **Step 5: Check diff and commit**

Run: `git diff --check; git status --short`

```powershell
git add apps/owstats-desktop/src/shared/matchListFormat.ts apps/owstats-desktop/src/features/matches/MatchListCard.tsx apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/matchListFormat.test.ts apps/owstats-desktop/scripts/matchWorkbench.test.ts
git commit -m "feat: redesign match list cards"
```

### Task 6: Add the shared embedded detail shell and neutral team tables

**Files:**
- Create: `apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx`
- Create: `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx`
- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`

- [ ] **Step 1: Write RED tests for shared header and both data branches**

In `matchWorkbench.test.ts`, require the new files and assert the normal detail markup contains `本人表现`, `队伍对比`, `队友数据`, `对手数据`, `上一场`, `下一场`. Render one anonymous normal fixture and one anonymous `arenaDetail` fixture with `renderToStaticMarkup`; require `aria-label="普通对局详情"` and `aria-label="角斗对局详情"` respectively.

Keep these existing arena assertions unchanged in `arenaDetailFormat.test.ts`:

```ts
assert.equal(compactSource.includes('showPerks={false}'), true);
assert.equal(compactSource.includes('economyMode={selected.key === "summary" ? "final" : "round"}'), true);
assert.equal(compactSource.includes('slots={4}'), true);
assert.equal(compactSource.includes('slots={6}'), true);
assert.equal(compactSource.includes('const displayName = info?.name ?? guid'), true);
```

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts`

Expected: FAIL because `MatchDetailHeader.tsx` and `MatchTeamTable.tsx` do not exist.

- [ ] **Step 3: Implement the shared outer shell without changing arena projection**

```ts
export type MatchDetailHeaderProps = {
  resultLabel: string;
  result: string | null;
  mapName: string;
  modeLabel: string;
  score: string;
  duration: string;
  startTime: string;
  hasPrevious: boolean;
  hasNext: boolean;
  onPrevious: () => void;
  onNext: () => void;
};
```

`MatchesPage` derives adjacent ids from the current `matches` array and passes callbacks through `MatchDetailPane`. Disabled buttons remain visible when no adjacent match exists.

`MatchTeamTable` receives `title`, `tone`, `players`, `showPerks`, optional expanded keys/toggle and optional `renderExpansion`. Move the current player identity, rank, perk and metric rendering into it without changing field sources. Render neutral rows with a narrow `.team` or `.enemy` marker; remove full-row `#1e8ee7` and `#dc304f` backgrounds.

Normal order:

```tsx
<MatchDetailHeader {...headerProps} />
<section aria-labelledby="self-performance-title">本人表现 + MatchPerformanceBadges</section>
<section aria-labelledby="team-comparison-title">
  <h2 id="team-comparison-title">队伍对比</h2>
  <MatchTeamTable title="队友数据" tone="team" players={detail.teammates.slice(0, 6)} showPerks />
  <MatchTeamTable title="对手数据" tone="enemy" players={detail.enemies.slice(0, 6)} showPerks />
</section>
```

Arena order keeps `phase7ArenaSelector`, builds the same header from the selected summary/round, and passes `showPerks={false}`, the existing economy mode and existing expansion renderer to `MatchTeamTable`. Do not change `selectedKey`, expansion reset, slot count or fallback code.

- [ ] **Step 4: Run GREEN and branch regressions**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/arenaDetailFormat.test.ts; npx tsx scripts/matchDetailHeaderFormat.test.ts; npx tsx scripts/playerHighlights.test.ts; npx tsc --noEmit`

Expected: all commands exit 0; normal and arena SSR fixtures select different branches.

- [ ] **Step 5: Check diff and commit**

Run: `git diff --check; git status --short`

```powershell
git add apps/owstats-desktop/src/features/matches/MatchDetailHeader.tsx apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx apps/owstats-desktop/src/pages/MatchesPage.tsx apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/matchWorkbench.test.ts apps/owstats-desktop/scripts/arenaDetailFormat.test.ts
git commit -m "feat: unify embedded match details"
```

### Task 7: Implement 32/68 sibling scrolling and sub-1100 entry navigation

**Files:**
- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/MatchListPane.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchesPageCache.test.ts`

- [ ] **Step 1: Write RED layout and entry-flow tests**

In `matchWorkbench.test.ts`, assert `MatchesPage` owns `mobileDetailOpen`, selection sets it true, and detail back sets it false. Assert CSS contains:

```ts
assert.match(matchesCss, /grid-template-columns:\s*minmax\([^)]*,\s*32fr\)\s+minmax\(0,\s*68fr\)/);
assert.match(matchesCss, /@media\s*\(max-width:\s*1099px\)/);
assert.match(matchesCss, /\.matchesWorkbench\.detailOpen\s+\.matchListPane\s*\{[^}]*display:\s*none/s);
assert.match(matchesCss, /\.matchesWorkbench:not\(\.detailOpen\)\s+\.matchDetailPane\s*\{[^}]*display:\s*none/s);
```

Retain the existing anchor tests in `matchesPageCache.test.ts` without changing expected offsets.

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts`

Expected: FAIL because the workbench still uses `380px minmax(0, 1fr)` and the old 1080px stacked layout.

- [ ] **Step 3: Implement one scroll owner per pane and narrow entry state**

In `MatchesPage`:

```ts
const [mobileDetailOpen, setMobileDetailOpen] = useState(false);

function selectMatch(matchId: string) {
  rememberScroll();
  setSelectedMatchId(matchId);
  setMobileDetailOpen(true);
}
```

Pass `onBack={() => setMobileDetailOpen(false)}` to `MatchDetailPane`. The back button is visually hidden at `min-width: 1100px` but remains a real button under 1100px.

Replace the fixed clamp with a page grid whose third row consumes remaining height:

```css
.matchesPage {
  height: calc(100dvh - var(--app-shell-header-height, 78px));
  min-height: 0;
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  overflow: hidden;
}

.matchesWorkbench {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(300px, 32fr) minmax(0, 68fr);
  gap: 16px;
  overflow: hidden;
}

.matchListPane,
.matchDetailPane {
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
}

@media (max-width: 1099px) {
  .matchesWorkbench { grid-template-columns: 1fr; }
  .matchesWorkbench.detailOpen .matchListPane { display: none; }
  .matchesWorkbench:not(.detailOpen) .matchDetailPane { display: none; }
}
```

Remove `overflow-y:auto` from descendants inside `MatchListPane` and `MatchDetailPane`; cards and arena expansion must never become a third scrolling level. Keep `timelineRef` on the actual `.matchListPane` scroll owner so existing `readMatchScrollAnchor` and restore code continue working.

- [ ] **Step 4: Run GREEN and scroll regressions**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbench.test.ts; npx tsx scripts/matchesPageCache.test.ts; npx tsx scripts/arenaDetailFormat.test.ts; npx tsc --noEmit`

Expected: all commands exit 0; no anchor expectation changes.

- [ ] **Step 5: Check diff and commit**

Run: `git diff --check; git status --short`

```powershell
git add apps/owstats-desktop/src/pages/MatchesPage.tsx apps/owstats-desktop/src/features/matches/MatchListPane.tsx apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/matchWorkbench.test.ts apps/owstats-desktop/scripts/matchesPageCache.test.ts
git commit -m "feat: add responsive match workbench flow"
```

### Task 8: Add sanitized visual fixtures and complete four-width acceptance

**Files:**
- Create: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`
- Create: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Write the failing fixture contract**

The test imports `renderMatchWorkbenchFixture` and requires three static states:

```ts
for (const state of ["wide-normal", "wide-arena", "narrow-list", "narrow-detail"] as const) {
  const html = renderMatchWorkbenchFixture(state);
  assert.match(html, /对局记录/);
  assert.doesNotMatch(html, /token|customerToken|authorization|credential|bnetId/i);
}
assert.match(renderMatchWorkbenchFixture("wide-normal"), /普通对局详情/);
assert.match(renderMatchWorkbenchFixture("wide-arena"), /角斗对局详情/);
assert.match(renderMatchWorkbenchFixture("narrow-list"), /data-fixture-state="narrow-list"/);
assert.match(renderMatchWorkbenchFixture("narrow-detail"), /data-fixture-state="narrow-detail"/);
```

- [ ] **Step 2: Run RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/matchWorkbenchVisualFixture.test.ts`

Expected: module-not-found failure for `matchWorkbenchVisualFixture.tsx`.

- [ ] **Step 3: Implement the anonymous fixture generator**

Use `renderToStaticMarkup` with fixed values only: match ids `fixture-normal` / `fixture-arena`, players `示例玩家A` through `示例玩家L`, empty `bnetId`, maps `香巴里寺院` / `绿洲城 - 花园`, heroes `西格玛` / `安娜`. Inline the compiled CSS text from `tokens.css`, `base.css`, `shell.css`, `shared.css` and `features/matches.css`; write HTML only to the `--output` directory.

```ts
export type MatchWorkbenchFixtureState = "wide-normal" | "wide-arena" | "narrow-list" | "narrow-detail";

export function renderMatchWorkbenchFixture(state: MatchWorkbenchFixtureState): string {
  const detail = state === "wide-arena" ? anonymousArenaDetail : anonymousNormalDetail;
  return `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><style>${css}</style></head>` +
    `<body data-fixture-state="${state}">${renderToStaticMarkup(<FixtureApp state={state} detail={detail} />)}</body></html>`;
}
```

Add `tsx scripts/matchWorkbenchVisualFixture.test.ts` to `test:components`.

- [ ] **Step 4: Run GREEN and generate screenshots outside the repository**

Run:

```powershell
cd apps/owstats-desktop
npx tsx scripts/matchWorkbenchVisualFixture.test.ts
$out = Join-Path $env:TEMP 'owstats-match-workbench-visual'
npx tsx scripts/matchWorkbenchVisualFixture.tsx --output $out
$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "${env:ProgramFiles}\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw 'Microsoft Edge headless executable not found' }
& $edge --headless --disable-gpu --hide-scrollbars --window-size=2540,1344 --screenshot="$out\2540x1344-normal.png" "file:///$($out.Replace('\','/'))/wide-normal.html"
& $edge --headless --disable-gpu --hide-scrollbars --window-size=1920,1080 --screenshot="$out\1920x1080-arena.png" "file:///$($out.Replace('\','/'))/wide-arena.html"
& $edge --headless --disable-gpu --hide-scrollbars --window-size=1440,900 --screenshot="$out\1440x900-normal.png" "file:///$($out.Replace('\','/'))/wide-normal.html"
& $edge --headless --disable-gpu --hide-scrollbars --window-size=1024,768 --screenshot="$out\1024x768-list.png" "file:///$($out.Replace('\','/'))/narrow-list.html"
& $edge --headless --disable-gpu --hide-scrollbars --window-size=1024,768 --screenshot="$out\1024x768-detail.png" "file:///$($out.Replace('\','/'))/narrow-detail.html"
```

Expected: test exits 0 and all five PNG files exist under `%TEMP%\owstats-match-workbench-visual`; no screenshot is added to Git.

- [ ] **Step 5: Inspect every screenshot against explicit acceptance criteria**

- 2540×1344: content is centered, list/detail approximate 32/68, no stretched empty margin, both panes scroll independently.
- 1920×1080: arena tabs, rank, 4 trait slots, 6 mod slots and dark expansion remain readable without clipping.
- 1440×900: filter controls/chips wrap without horizontal scrolling; result and score are separate; numbers use thousands separators.
- 1024×768 list: only the list is visible; selected card is pale with a purple edge, not a saturated full-card fill.
- 1024×768 detail: only the detail is visible; Chinese back button is visible; no list/detail/card triple scroll.
- All widths: team/enemy rows use neutral surfaces with narrow semantic markers; no raw player id or credential-like text appears.

- [ ] **Step 6: Run full frontend regression, Git checks, and commit the fixture infrastructure**

Run: `cd apps/owstats-desktop; npm run test:unit; npm run test:components; npm run test:legacy; npm run build`

Expected: every command exits 0. Then run from repository root: `git diff --check; git status --short`.

Expected: only the two fixture scripts and `package.json` are modified; `%TEMP%` screenshots are absent from Git status.

```powershell
git add apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts apps/owstats-desktop/package.json
git commit -m "test: add match workbench visual fixtures"
```

## Final verification after all eight commits

- [ ] Run focused frontend contracts:

```powershell
cd apps/owstats-desktop
npx tsx scripts/matchesPageCache.test.ts
npx tsx scripts/filterBarUi.test.ts
npx tsx scripts/matchListFormat.test.ts
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/arenaDetailFormat.test.ts
npx tsx scripts/stab006ModuleContracts.test.ts
npx tsx scripts/phase8MultiSeasonPages.test.ts
npx tsx scripts/matchWorkbenchVisualFixture.test.ts
npx tsc --noEmit
```

Expected: every command exits 0.

- [ ] Run the required frontend suite and build:

```powershell
npm test
npm run build
```

Expected: both commands exit 0; `dist` path scan reports no `/@fs/`, developer drive path or `docs/reference` dependency.

- [ ] Run repository checks:

```powershell
cd ..\..
git diff --check
git status --short
git log --oneline -8
```

Expected: `git diff --check` is silent, status is clean, and the eight focused commits appear in order. Report that no real privacy data was accessed and that native Tauri packaging was not required because this slice changes tracked frontend source/styles/tests only.
