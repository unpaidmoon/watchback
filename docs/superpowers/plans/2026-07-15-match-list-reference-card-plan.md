# 对局列表参考式卡片 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将对局列表卡片收敛为 152px 企业级紧凑布局，实现参考图的分层地图底边、右侧地图/模式信息、原有标签奖牌，以及超过一万的 `w` 数字格式。

**Architecture:** 保持现有 `MatchSummary` 和列表交互不变，由 `matchListFormat.ts` 独立负责数字格式，由 `MatchListCard.tsx` 只组合展示数据，由 `matches.css` 提供唯一一套卡片布局和六点地图裁切。现有匿名 `matchWorkbenchVisualFixture` 负责三档桌面分辨率及窄屏的浏览器验收，不读取 Tauri app data 或真实用户数据。

**Tech Stack:** React 18、TypeScript、CSS、Node `assert` 脚本测试、匿名静态 fixture、Playwright 浏览器验收。

**Worktree policy:** 当前工作树已有用户未提交改动。执行前后都运行 `git status --short`；只修改本计划列出的文件，不清理、暂存或提交任何文件，除非用户另行明确授权。

---

## 文件职责

- `apps/owstats-desktop/src/shared/matchListFormat.ts`：对局列表专用的比分、地图键和紧凑数字纯格式化函数。
- `apps/owstats-desktop/scripts/matchListFormat.test.ts`：锁定 `10,000` 阈值及千位四舍五入边界。
- `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`：对局卡片语义结构、信息顺序和可访问文本。
- `apps/owstats-desktop/src/styles/features/matches.css`：唯一的对局卡片布局、152px 高度、六点地图裁切、标签奖牌样式和窄屏回退。
- `apps/owstats-desktop/scripts/matchWorkbench.test.ts`：锁定 JSX 与 CSS 静态契约，并删除当前错误的三角伪元素/96px 数据区断言。
- `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`：补充长名称、多奖牌、空评价等匿名视觉边界样本。
- `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts`：确认视觉样本包含边界状态且保持隐私安全。

### Task 1: 锁定并实现 `w` 数字格式

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchListFormat.test.ts`
- Modify: `apps/owstats-desktop/src/shared/matchListFormat.ts`

- [ ] **Step 1: 将紧凑数字断言改为新契约**

在 `matchListFormat.test.ts` 中用以下边界断言替换现有 `15,000` 断言，并保留比分、地图标题及 `5,960` 断言：

```ts
assert.equal(formatCompactStat(5960), "5,960");
assert.equal(formatCompactStat(10000), "10,000");
assert.equal(formatCompactStat(10001), "1.0w");
assert.equal(formatCompactStat(10499), "1.0w");
assert.equal(formatCompactStat(10500), "1.1w");
assert.equal(formatCompactStat(21499), "2.1w");
assert.equal(formatCompactStat(21500), "2.2w");
```

- [ ] **Step 2: 运行测试确认 RED**

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchListFormat.test.ts
```

Expected: FAIL，首次失败应显示 `formatCompactStat(10001)` 实际仍为 `10,001`。

- [ ] **Step 3: 写入最小纯函数实现**

将 `matchListFormat.ts` 的 `formatCompactStat` 替换为：

```ts
export function formatCompactStat(value: number): string {
  if (value <= 10_000) return groupedNumber.format(value);
  const roundedToThousand = Math.round(value / 1_000) * 1_000;
  return `${(roundedToThousand / 10_000).toFixed(1)}w`;
}
```

不修改 `formatScoreLine`、`formatMatchBannerTitle` 或 `formatMatchPreviewKey`。

- [ ] **Step 4: 运行测试确认 GREEN**

```powershell
npx tsx scripts/matchListFormat.test.ts
```

Expected: PASS，退出码为 0。

- [ ] **Step 5: 检查本任务差异**

```powershell
git diff -- apps/owstats-desktop/src/shared/matchListFormat.ts apps/owstats-desktop/scripts/matchListFormat.test.ts
```

Expected: 仅包含紧凑数字边界测试和纯函数实现，不包含真实数据或无关格式化改动。

### Task 2: 锁定参考卡片的结构与尺寸契约

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Verify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`

- [ ] **Step 1: 替换错误的横幅与高度断言**

在 `matchWorkbench.test.ts` 的列表卡片区域删除以下旧契约：

```ts
assert.match(matchStyleSource, /\.matchListCardResultBlock::after\s*\{[\s\S]*clip-path:\s*polygon/);
assert.match(matchStyleSource, /\.phase6WorkbenchList \.matchListCardData\s*\{[\s\S]*height:\s*96px/);
assert.match(matchStyleSource, /\.matchListCardAvatar \.phase6HeroAvatarShared\s*\{[\s\S]*width:\s*64px[\s\S]*height:\s*64px/);
```

加入最终结构和尺寸断言：

```ts
assert.equal(listCardSource.includes("match.beginLabel"), false, "list cards omit match time");
assert.match(listCardSource, /match\.mapName[\s\S]*match\.modeLabel/, "map name precedes the localized mode label");
assert.match(listCardSource, /<MatchPerformanceBadges[\s\S]*showGrade=\{false\}/, "list cards keep tags and medals but hide grade");
assert.doesNotMatch(matchStyleSource, /\.matchListCardResultBlock::after\s*\{/, "result block must not create a triangle overlay");
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList button\.matchListCard\s*\{[\s\S]*height:\s*152px[\s\S]*grid-template-rows:\s*64px\s+88px/,
  "wide list cards use the approved 64 plus 88 rhythm",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardHeaderMap\s*\{[\s\S]*clip-path:\s*polygon\(26% 0,\s*100% 0,\s*100% 100%,\s*58% 100%,\s*55% 82%,\s*29% 82%\)/,
  "map preview uses the approved layered six-point outline",
);
assert.match(
  matchStyleSource,
  /\.phase6WorkbenchList \.matchListCardData\s*\{[\s\S]*grid-template-columns:\s*60px\s+minmax\(0,\s*0\.8fr\)\s+minmax\(0,\s*1\.4fr\)\s+minmax\(0,\s*1fr\)/,
  "wide data area uses avatar, KAD, combined stats, and performance columns",
);
assert.match(matchStyleSource, /\.phase6WorkbenchList \.matchListCardAvatar \.phase6HeroAvatarShared\s*\{[\s\S]*width:\s*60px[\s\S]*height:\s*60px/);
```

- [ ] **Step 2: 运行结构测试确认 RED**

```powershell
npx tsx scripts/matchWorkbench.test.ts
```

Expected: FAIL 于三角伪元素、152px 总高、六点地图裁切或 60px 头像中的首个未满足契约。

- [ ] **Step 3: 核对组件结构，只有不满足契约时才做最小调整**

`MatchListCard.tsx` 的目标 JSX 必须保持为：

```tsx
<span className="matchListCardHeader">
  <MapPreview
    mapKey={formatMatchPreviewKey(match)}
    name={match.mapName}
    className="matchListCardHeaderMap"
    decorative
  />
  <span className={`matchListCardResultBlock result-${result.tone}`}>
    <span>{result.label}</span>
    <strong data-numeric>{formatScoreLine(match)}</strong>
  </span>
  <span className="matchListCardBannerCopy" title={`${match.mapName} · ${match.modeLabel}`}>
    <span>{match.mapName}</span>
    <span>{match.modeLabel}</span>
  </span>
</span>
<span className="matchListCardData">
  <span className="matchListCardAvatar">
    <HeroAvatar heroKey={match.heroGuid || match.heroName} name={match.heroName} />
  </span>
  <MetricBlock label="K/A/D" value={`${match.kills}/${match.assists}/${match.deaths}`} />
  <MetricBlock
    label="伤害/治疗/抵挡"
    value={[match.damage, match.healing, match.mitigation].map(formatCompactStat).join("/")}
  />
  <span className="matchListCardPerformance">
    <span>对局表现</span>
    <MatchPerformanceBadges evaluation={match.evaluation} teamAwards={match.teamAwards} compact showGrade={false} />
  </span>
</span>
```

当前工作树已接近此结构；不要重写组件、恢复时间、英雄名、评分或进入详情箭头。

- [ ] **Step 4: 重跑结构测试记录仍待 CSS 满足的失败**

```powershell
npx tsx scripts/matchWorkbench.test.ts
```

Expected: JSX 相关断言通过；测试仍因 Task 3 尚未完成的 CSS 契约失败。

### Task 3: 合并重复 CSS 并实现唯一的 152px 卡片样式

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Test: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: 识别并删除三组相互覆盖的旧卡片规则**

只删除不再由当前 JSX 使用、或被后置规则重复覆盖的列表卡片选择器，包括旧的：

```css
.matchListCardSummary
.matchListCardCopy
.matchListCardResultScore
.matchListCardModeTime
.matchListCardDivider
.matchListCardHeroLine
.matchListCardStats
.matchListCardBadgeLine
.matchListCardResultBlock::after
```

保留工作台 `32fr / 68fr` 主从布局、列表滚动、详情面板、筛选器、选中态语义令牌和其他页面样式。删除前后分别用 `rg -n` 核对，避免误删详情页同名规则。

- [ ] **Step 2: 写入唯一的宽屏卡片规则**

在对局列表作用域内保留一份以下规则；不使用原始十六进制品牌色：

```css
.phase6WorkbenchList button.matchListCard {
  position: relative;
  display: grid;
  grid-template-rows: 64px 88px;
  gap: 0;
  width: 100%;
  height: 152px;
  padding: 0;
  overflow: hidden;
  border: 0;
  border-radius: var(--radius-secondary);
  background: var(--action-primary);
  box-shadow: inset 0 0 0 1px var(--glass-border);
}

.phase6WorkbenchList button.matchListCard.phase6MatchRowSelected {
  box-shadow: inset 0 0 0 2px var(--action-primary);
}

.phase6WorkbenchList .matchListCardHeader {
  position: relative;
  isolation: isolate;
  display: block;
  width: 100%;
  height: 64px;
  min-height: 64px;
  overflow: hidden;
  background: var(--action-primary);
}

.phase6WorkbenchList .matchListCardHeaderMap {
  position: absolute;
  z-index: 0;
  inset: 0;
  width: 100%;
  height: 100%;
  min-height: 0;
  object-fit: cover;
  clip-path: polygon(26% 0, 100% 0, 100% 100%, 58% 100%, 55% 82%, 29% 82%);
  filter: saturate(0.72) contrast(0.96);
}

.phase6WorkbenchList .matchListCardResultBlock {
  position: absolute;
  z-index: 1;
  inset: 0 auto auto 0;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 26%;
  height: 82%;
  min-width: 112px;
  padding: 0 var(--space-3);
  background: transparent;
  color: var(--action-on-primary);
  font-size: 15px;
  font-weight: 400;
}

.phase6WorkbenchList .matchListCardResultBlock strong {
  color: var(--action-on-primary);
  font-size: 24px;
  font-weight: 400;
  font-variant-numeric: tabular-nums;
}

.phase6WorkbenchList .matchListCardBannerCopy {
  position: absolute;
  z-index: 1;
  right: var(--space-4);
  bottom: 20px;
  display: flex;
  min-width: 0;
  max-width: 62%;
  align-items: baseline;
  justify-content: flex-end;
  gap: var(--space-2);
  overflow: hidden;
  color: var(--action-on-primary);
  font-size: 15px;
  font-weight: 400;
  text-shadow: 0 1px 3px color-mix(in srgb, var(--action-primary) 48%, transparent);
  white-space: nowrap;
}

.phase6WorkbenchList .matchListCardBannerCopy > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.phase6WorkbenchList .matchListCardBannerCopy > span + span {
  flex: 0 0 auto;
  font-size: 12px;
}

.phase6WorkbenchList .matchListCardBannerCopy > span + span::before {
  margin-right: var(--space-2);
  content: "·";
}

.phase6WorkbenchList .matchListCardData {
  display: grid;
  grid-template-columns: 60px minmax(0, 0.8fr) minmax(0, 1.4fr) minmax(0, 1fr);
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  height: 88px;
  min-height: 88px;
  padding: var(--space-3) var(--space-4);
  overflow: hidden;
  background: color-mix(in srgb, var(--surface-secondary) 88%, transparent);
}

.phase6WorkbenchList .matchListCardAvatar,
.phase6WorkbenchList .matchListCardAvatar .phase6HeroAvatarShared {
  display: block;
  width: 60px;
  height: 60px;
  border-radius: 0;
}

.phase6WorkbenchList .matchListCardData .phase6MatchMetricBlock,
.phase6WorkbenchList .matchListCardPerformance {
  display: grid;
  align-content: center;
  gap: var(--space-2);
  min-width: 0;
  color: var(--text-tertiary);
  font-size: 12px;
  font-weight: 400;
  white-space: nowrap;
}

.phase6WorkbenchList .matchListCardData .phase6MatchMetricBlock strong {
  overflow: hidden;
  color: var(--text-primary);
  font-size: 18px;
  font-weight: 400;
  font-variant-numeric: tabular-nums;
  text-overflow: ellipsis;
}

.phase6WorkbenchList .matchListCardPerformance .phase6PerformanceBadges {
  min-width: 0;
  min-height: 24px;
  gap: var(--space-2);
  flex-wrap: nowrap;
  overflow: hidden;
}

.phase6WorkbenchList .matchListCardPerformance .primaryTag {
  min-height: 24px;
  padding: 0 7px;
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
  background: transparent;
  color: var(--text-primary);
  font-size: 12px;
  font-weight: 400;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.phase6WorkbenchList .matchListCardPerformance .phase6AwardMedal {
  width: 24px;
  height: 24px;
  flex: 0 0 24px;
  border: 1px solid var(--border-subtle);
  border-radius: 50%;
  background: transparent;
  box-shadow: none;
  color: var(--text-primary);
  font-size: 12px;
  font-weight: 400;
}
```

- [ ] **Step 3: 写入窄宽度回退，不影响宽屏 152px 契约**

```css
@media (max-width: 640px) {
  .phase6WorkbenchList button.matchListCard {
    grid-template-rows: 64px auto;
    height: auto;
    min-height: 152px;
  }

  .phase6WorkbenchList .matchListCardResultBlock {
    width: 34%;
    min-width: 104px;
  }

  .phase6WorkbenchList .matchListCardBannerCopy {
    max-width: 58%;
  }

  .phase6WorkbenchList .matchListCardData {
    grid-template-columns: 54px minmax(0, 0.8fr) minmax(0, 1.4fr);
    height: auto;
    min-height: 104px;
    gap: var(--space-3);
  }

  .phase6WorkbenchList .matchListCardAvatar,
  .phase6WorkbenchList .matchListCardAvatar .phase6HeroAvatarShared {
    width: 54px;
    height: 54px;
  }

  .phase6WorkbenchList .matchListCardPerformance {
    grid-column: 2 / -1;
  }
}
```

- [ ] **Step 4: 运行结构和标签奖牌测试确认 GREEN**

```powershell
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/matchPerformanceBadges.test.ts
```

Expected: 两个命令均 PASS。若仍失败，只修正与新卡片契约冲突的旧静态断言，不放宽六点裁切、152px、四列顺序或隐藏评分要求。

- [ ] **Step 5: 检查 CSS 重复与无效选择器**

```powershell
rg -n "matchListCardResultBlock::after|matchListCardModeTime|matchListCardDivider|matchListCardBadgeLine" src/styles/features/matches.css
rg -n "^\.phase6WorkbenchList button\.matchListCard|^\.phase6WorkbenchList \.matchListCardHeaderMap|^\.phase6WorkbenchList \.matchListCardData" src/styles/features/matches.css
```

Expected: 第一条无结果；第二条每个宽屏主选择器只有一个定义，另允许一个明确的 `max-width: 640px` 覆盖。

### Task 4: 扩充匿名视觉边界样本

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts`

- [ ] **Step 1: 写入失败的匿名边界断言**

在 `matchWorkbenchVisualFixture.test.ts` 增加：

```ts
const wideNormal = renderMatchWorkbenchFixture("wide-normal");
assert.match(wideNormal, /预设快速模式长名称示例/);
assert.match(wideNormal, /1\.6w\/2,410\/1\.2w/);
assert.match(wideNormal, /伤害最高/);
assert.match(wideNormal, /治疗最高/);
assert.match(wideNormal, /data-match-id="fixture-match-6"/);
assert.doesNotMatch(wideNormal, /token|customerToken|authorization|credential|bnetId/i);
```

- [ ] **Step 2: 运行 fixture 测试确认 RED**

```powershell
npx tsx scripts/matchWorkbenchVisualFixture.test.ts
```

Expected: FAIL，因为长模式名和多奖牌样本尚未写入。

- [ ] **Step 3: 用纯合成数据覆盖长名称、多奖牌和空评价**

在 `createMatch` 返回前构造以下字段：

```ts
const evaluation = index === 5
  ? null
  : {
      grade: index % 3 === 0 ? "S" : "A",
      primaryTag: index % 3 === 0 ? "尽力局" : "伯仲局",
      evidenceTags: [],
      confidence: 0.86,
    };
const teamAwards = index === 0
  ? [
      { code: "damage" as const, label: "伤害最高", tone: "gold" as const },
      { code: "healing" as const, label: "治疗最高", tone: "gold" as const },
    ]
  : [];
```

并将返回对象中的相关字段改为：

```ts
modeLabel: index === 0 ? "预设快速模式长名称示例" : arena ? "角斗竞技" : "竞技比赛",
damage: index === 0 ? 15_842 : 15_842 - index * 631,
healing: 2_410 + index * 337,
mitigation: index === 0 ? 12_380 : 12_380 - index * 405,
evaluation,
teamAwards,
```

所有 ID 保持 `fixture-*`，不得从 SQLite、app data、浏览器 profile 或真实 dump 复制内容。

- [ ] **Step 4: 运行 fixture 测试确认 GREEN**

```powershell
npx tsx scripts/matchWorkbenchVisualFixture.test.ts
```

Expected: PASS；HTML 包含长模式名、`w` 数值、多奖牌和空评价卡片，且隐私关键字检查通过。

### Task 5: 完成自动化回归验证

**Files:**
- Verify only: all files modified in Tasks 1–4

- [ ] **Step 1: 运行对局卡片聚焦测试**

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchListFormat.test.ts
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/matchPerformanceBadges.test.ts
npx tsx scripts/matchWorkbenchVisualFixture.test.ts
```

Expected: 全部 PASS。

- [ ] **Step 2: 运行完整前端测试**

```powershell
npm test
```

Expected: `test:resources`、`test:unit`、`test:components`、`test:legacy`、`test:text` 全部 PASS。若存在与本任务无关的既有失败，记录完整命令和首个错误，不修改无关文件规避测试。

- [ ] **Step 3: 运行类型检查与生产构建**

```powershell
npm run build
```

Expected: TypeScript 和 Vite 构建均 PASS，`verifyDistPaths.mjs` 不报告 `/@fs/`、开发机绝对路径或未打包资源。

- [ ] **Step 4: 检查差异卫生**

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` 无输出；状态中只新增/修改本计划范围文件以及执行前已存在的用户改动。不暂存、不提交。

### Task 6: 用匿名 fixture 做三档浏览器视觉验收

**Files:**
- Generate outside repository: `$env:TEMP\owstats-match-card-visual\*.html`
- Save screenshots outside repository: `C:\Users\cyb\.codex\visualizations\2026\07\15\match-list-reference-card\`
- Do not commit screenshots、浏览器 profile、trace、`.playwright-cli` 或临时 HTML。

- [ ] **Step 1: 按要求读取 `playwright` skill 并生成匿名静态页面**

```powershell
cd apps\owstats-desktop
$fixtureDir = Join-Path $env:TEMP "owstats-match-card-visual"
npx tsx scripts/matchWorkbenchVisualFixture.tsx --output $fixtureDir
```

Expected: 输出 `wide-normal.html`、`wide-arena.html`、`narrow-list.html`、`narrow-detail.html`，终端只显示匿名输出目录。

- [ ] **Step 2: 启动临时静态服务器**

从 `$fixtureDir` 启动隐藏的本地服务器，不使用真实 Tauri 命令或 app data：

```powershell
python -m http.server 4177 --bind 127.0.0.1
```

Expected: `http://127.0.0.1:4177/wide-normal.html` 可访问。

- [ ] **Step 3: 在三个规定视口检查宽屏卡片**

使用 Playwright 打开 `wide-normal.html`，依次设置：

```text
2540 × 1344
1920 × 1080
1440 × 900
```

每个视口都检查并截图：

- 卡片测量高度为 `152px`，相邻卡片垂直间距为 `12px`。
- 横幅为 `64px`，地图底部呈“抬高底边 → 中段斜落 → 右侧贴底”的两级轮廓。
- 数据区四列无横向溢出；`1.6w/2,410/1.2w` 保持单行。
- 长模式名只在必要时省略，不挤压左侧胜负比分。
- 主标签和两枚奖牌可辨认但不形成鲜艳胶囊墙。
- 选中态、键盘焦点、地图缺失占位均不导致高度跳动。

- [ ] **Step 4: 检查窄屏回退**

打开 `narrow-list.html`，使用 `640 × 900` 和 `375 × 812`：

- 页面无横向滚动。
- 卡片数据区允许增高并改为两行，表现列位于第二行。
- 胜负中文、比分、地图名和模式仍可读取。
- 单卡仍是一个可聚焦按钮，Tab 焦点清晰。

- [ ] **Step 5: 结束服务器并记录证据**

停止临时服务器，删除 `$fixtureDir`；保留外部截图作为本次验证证据，不写入仓库。记录每个视口的实际测量值、通过/失败项和截图路径。

---

## 完成标准

- `10,000` 保持完整格式，所有 `> 10,000` 数字按千位四舍五入后显示一位小数 `w`。
- 地图预览使用六点分层底边，结果块没有三角伪元素。
- 宽屏卡片为 152px，64px 横幅与 88px 数据区形成稳定节奏。
- 地图名/本地化模式在右侧，K/A/D、伤害/治疗/抵挡、原有标签奖牌保持正确。
- 聚焦测试、完整前端测试、构建、差异检查与三档视觉验收均有实际结果。
- 未读取、打印或保存任何真实凭据、真实用户数据 dump、浏览器 profile 或原始隐私 ID。

### Task 7: 结果区白底与状态点

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [ ] **Step 1: 写入失败的静态视觉契约**

要求横幅底层使用 `--action-on-primary`，结果文字和比分均为 `20px`、`--text-primary`，胜利和失败分别通过 `::before` 使用 `--status-success` 与 `--status-danger`。

- [ ] **Step 2: 运行聚焦测试确认 RED**

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchWorkbench.test.ts
```

Expected: FAIL 于白色横幅底层、统一字号或状态点契约。

- [ ] **Step 3: 最小修改 CSS**

只修改 `.matchListCardHeader`、`.matchListCardResultBlock` 及其 `strong`、胜负 tone 伪元素；保留透明结果层和六点地图裁切。

- [ ] **Step 4: 验证 GREEN 与完整前端回归**

```powershell
npx tsx scripts/matchWorkbench.test.ts
npm test
npm run build
git diff --check
```

Expected: 所有命令退出码为 0；构建允许已有的大包体警告。

### Task 8: 交换横幅与数据区表面颜色

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`

- [ ] **Step 1: 更新静态契约并确认 RED**

要求 `.matchListCardHeader` 使用 `color-mix(in srgb, var(--surface-secondary) 88%, transparent)`，`.matchListCardData` 使用 `var(--action-on-primary)`；运行 `npx tsx scripts/matchWorkbench.test.ts`，应因旧颜色失败。

- [ ] **Step 2: 最小交换两个背景声明并确认 GREEN**

不修改其他属性；重新运行聚焦测试应退出码为 0。

- [ ] **Step 3: 完成前端验证**

运行 `npm test`、`npm run build` 与 `git diff --check`，并用匿名 fixture 检查桌面卡片的上下表面颜色。

### Task 9: 为对局表现重新分配宽屏列空间

**Files:**
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts`

- [x] **Step 1: 写入失败契约**

要求宽屏列比例为 `60px / 0.7fr / 1.2fr / 1.3fr`、核心数字为 `16px`，匿名首卡包含四枚奖牌。

- [x] **Step 2: 运行聚焦测试确认 RED**

运行 `npx tsx scripts/matchWorkbench.test.ts` 与 `npx tsx scripts/matchWorkbenchVisualFixture.test.ts`，应分别因旧比例、旧字号或奖牌不足失败。

- [x] **Step 3: 最小实现并确认 GREEN**

只修改宽屏基础列比例、基础核心字号、表现徽章间距和匿名 fixture 首卡奖牌数量，不改变已有媒体查询。

- [x] **Step 4: 浏览器及完整回归**

使用匿名 fixture 检查四枚奖牌完整显示，再运行 `npm test`、`npm run build` 和 `git diff --check`。
