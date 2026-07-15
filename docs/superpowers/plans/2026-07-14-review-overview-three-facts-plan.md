# 战报概览三项事实带 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将战报概览精简为职责段位、三个常用英雄头像、赛季对局和总胜率，并移除两块摘要面板。

**Architecture:** 只修改战报页面的概览 JSX、页面专属 CSS 与静态回归脚本。页面继续使用已经加载的 `PersonalSeasonReview`：`heroPool.mainHeroes` 提供已排序的英雄，`summary` 提供赛季场次和胜率；不改变请求、类型或后端。

**Tech Stack:** React、TypeScript、CSS、Node `assert` 静态回归脚本。

---

## 已执行补充：角斗竞技职责段位

- [x] 在 `PersonalSeasonReview` 增加独立 `stadiumRoleSummaries`，固定聚合 `SportFight` 的重装、输出、支援；预设四项保持不变。
- [x] 使用当前账号在角斗对局总计中的安全本地数据读取段位、场次、胜率和 KDA，并在后端测试中覆盖数字型账号标识。
- [x] 在职责区新增“角斗竞技”三卡分组，复用原卡片格式并优先使用打包的角斗段位资产。
- [x] 验证：`cargo fmt --check`、`cargo test --tests`、目标战报回归和前端生产构建通过。

## 文件结构

- 修改：`apps/owstats-desktop/src/pages/PersonalReviewPage.tsx` — 移除两个摘要 `GlassPanel`，渲染三项事实带并复用 `HeroAvatar`。
- 修改：`apps/owstats-desktop/src/styles/features/review.css` — 将事实带设为低层级三列分隔布局并定义窄屏降级。
- 修改：`apps/owstats-desktop/scripts/analysisMovedToReview.test.ts` — 断言概览顺序变为职责与英雄后直接进入地图分析。
- 修改：`apps/owstats-desktop/scripts/reviewTrimmedSections.test.ts` — 断言两个摘要文案不再存在、三个事实及头像映射存在。
- 修改：`apps/owstats-desktop/scripts/reviewVisualPolish.test.ts` — 将已删除摘要的样式断言改为三项事实带的视觉约束。

### Task 1: 锁定页面结构回归

**Files:**
- Modify: `apps/owstats-desktop/scripts/analysisMovedToReview.test.ts`
- Modify: `apps/owstats-desktop/scripts/reviewTrimmedSections.test.ts`

- [ ] **Step 1: 写入失败断言**

将 `analysisMovedToReview.test.ts` 的概览顺序断言替换为：

```ts
assert.match(
  reviewSource,
  /function ReviewOverview[\\s\\S]*?<RoleAndHeroView review=\\{review\\}[\\s\\S]*?<section className="reviewMapAnalysis">[\\s\\S]*?<MapAnalysisModules/,
  "概览应按职责与英雄、地图的顺序组合完整分析",
);
assert.doesNotMatch(reviewSource, /reviewOverviewPrimary|reviewOverviewSecondary/);
```

在 `reviewTrimmedSections.test.ts` 追加：

```ts
assert.equal(source.includes("赛季概览"), false, "概览应移除赛季概览摘要");
assert.equal(source.includes("关键表现"), false, "概览应移除本季摘要");
assert.match(source, /review\.heroPool\.mainHeroes\.slice\(0, 3\)\.map/);
assert.match(source, /<HeroAvatar heroKey=\{hero\.hero\} name=\{hero\.hero\} size="sm" \/>/);
assert.match(source, /<Metric label="赛季对局" value=\{`\$\{review\.summary\.totalMatches\} 场`\} \/>/);
assert.match(source, /<Metric label="总胜率" value=\{formatWinRate\(review\.summary\.winRate, 0\)\} \/>/);
```

- [ ] **Step 2: 运行并确认失败**

运行：`npx tsx scripts/analysisMovedToReview.test.ts; npx tsx scripts/reviewTrimmedSections.test.ts`

预期：失败；当前页面仍含 `reviewOverviewPrimary`、`赛季概览` 和 `关键表现`。

### Task 2: 实现三项事实带

**Files:**
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx:360-417`

- [ ] **Step 1: 最小化页面实现**

将 `ReviewOverview` 中 `RoleAndHeroView` 后的两个 `GlassPanel` 删除，使其后直接保留地图分析：

```tsx
      <RoleAndHeroView review={review} />
      <section className="reviewMapAnalysis">
        <MapAnalysisModules analysis={analysis} loading={analysisLoading} error={analysisError} season={season} />
      </section>
```

在 `RoleAndHeroView` 中删除 `mainHero`，并将现有四个 `Metric` 替换为：

```tsx
      <div className="reviewHeroSummary" aria-label="赛季事实">
        <div className="reviewHeroAvatars">
          <span>常用英雄</span>
          <div className="reviewHeroAvatarList">
            {review.heroPool.mainHeroes.slice(0, 3).map((hero) => (
              <HeroAvatar key={hero.hero} heroKey={hero.hero} name={hero.hero} size="sm" />
            ))}
          </div>
        </div>
        <Metric label="赛季对局" value={`${review.summary.totalMatches} 场`} />
        <Metric label="总胜率" value={formatWinRate(review.summary.winRate, 0)} />
      </div>
```

- [ ] **Step 2: 运行结构回归**

运行：`npx tsx scripts/analysisMovedToReview.test.ts; npx tsx scripts/reviewTrimmedSections.test.ts`

预期：通过。

### Task 3: 实现压平的事实带样式

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/review.css:67-135, 285-310`
- Modify: `apps/owstats-desktop/scripts/reviewVisualPolish.test.ts`

- [ ] **Step 1: 写入失败的视觉断言**

将 `reviewVisualPolish.test.ts` 中依赖 `.reviewOverviewPanel` 和 `.reviewOverviewMetrics` 的两条断言替换为：

```ts
assert.match(reviewCss, /\.reviewHeroSummary\s*\{[^}]*grid-template-columns:\s*repeat\(3, minmax\(0, 1fr\)\);[^}]*border:\s*1px solid var\(--glass-border\);/s);
assert.match(reviewCss, /\.reviewHeroSummary\s*>\s*\*\s*\+\s*\*\s*\{[^}]*border-left:\s*1px solid var\(--glass-border\);/s);
assert.match(reviewCss, /\.reviewHeroAvatarList\s*\{[^}]*display:\s*flex;[^}]*gap:\s*var\(--space-2\);/s);
```

- [ ] **Step 2: 运行并确认失败**

运行：`npx tsx scripts/reviewVisualPolish.test.ts`

预期：失败；当前 CSS 没有三列事实带及头像列表规则。

- [ ] **Step 3: 最小化 CSS 实现**

删除不再使用的 `.reviewOverviewPanel*` 与 `.reviewOverviewMetrics*` 规则；将 `.reviewHeroSummary` 设为：

```css
.reviewHeroSummary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: var(--space-4);
  border: 1px solid var(--glass-border);
  background: rgba(250, 251, 247, 0.38);
}

.reviewHeroSummary > * {
  min-width: 0;
  padding: var(--space-3);
}

.reviewHeroSummary > * + * {
  border-left: 1px solid var(--glass-border);
}

.reviewHeroSummary .metric,
.reviewHeroAvatars {
  display: grid;
  gap: var(--space-1);
  background: transparent;
  box-shadow: none;
}

.reviewHeroAvatarList {
  display: flex;
  gap: var(--space-2);
  min-height: 32px;
  align-items: center;
}
```

在 `@media (max-width: 899px)` 中把 `.reviewHeroSummary` 改为两列；在 `@media (max-width: 640px)` 改为单列并将相邻项分隔线由 `border-left` 换成 `border-top`。

- [ ] **Step 4: 运行视觉回归**

运行：`npx tsx scripts/reviewVisualPolish.test.ts`

预期：通过。

### Task 4: 整体验证

**Files:**
- Verify only.

- [ ] **Step 1: 运行受影响测试与完整前端测试**

运行：`npm test`

预期：退出码为 `0`。

- [ ] **Step 2: 运行生产构建**

运行：`npm run build`

预期：TypeScript 编译和 Vite 构建均成功。

- [ ] **Step 3: 检查差异与工作树**

运行：`git diff --check; git status --short`

预期：无空白错误；仅包含本切片的设计说明、实施计划、页面、样式与测试文件。按仓库协作说明，不暂存或提交任何文件，除非用户另行明确要求。
