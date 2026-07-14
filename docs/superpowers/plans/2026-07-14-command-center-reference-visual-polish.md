# 指挥台参考图视觉精修 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把指挥台精修为柔和渐变画布与克制毛玻璃质感的视觉标杆，保持自动 BattleTag、单赛季同步、数据健康和工具行为不变。

**Architecture:** `tokens.css` 定义画布和玻璃令牌，`shell.css` 承载静态渐变，`command-center.css` 定义表面与响应式布局。`OnboardingPage.tsx` 仅把赛季数字输入替换为 S21/S22/S23 分段按钮，继续把相同 `selectedSeason` 交给既有同步与状态卡。

**Tech Stack:** React 18、TypeScript、CSS、tsx 源码契约测试、Tauri WebView。

---

## 文件地图

- `apps/owstats-desktop/src/styles/tokens.css`：画布、玻璃、边线和模糊令牌。
- `apps/owstats-desktop/src/styles/shell.css`：应用画布渐变与回退。
- `apps/owstats-desktop/src/styles/features/command-center.css`：指挥台表面与响应式规则。
- `apps/owstats-desktop/src/pages/OnboardingPage.tsx`：固定赛季分段选择。
- `apps/owstats-desktop/scripts/visualSystem.test.ts`：视觉令牌契约。
- `apps/owstats-desktop/scripts/phase6Shell.test.ts`：指挥台结构与身份流程保护。

### Task 1: 画布与玻璃令牌

**Files:**

- Modify: `apps/owstats-desktop/scripts/visualSystem.test.ts`
- Modify: `apps/owstats-desktop/src/styles/tokens.css`
- Modify: `apps/owstats-desktop/src/styles/shell.css`

- [ ] **Step 1: 写失败的视觉契约**

在 `visualSystem.test.ts` 的 token 断言后添加：

```ts
assert.match(tokens, /--canvas-glow-cool:\s*rgba\(198, 224, 222, 0\.52\)/i);
assert.match(tokens, /--glass-primary:\s*rgba\(244, 246, 240, 0\.68\)/i);
assert.match(tokens, /--glass-blur:\s*16px/i);
assert.match(shell, /radial-gradient\(/i);
assert.match(shell, /@supports\s*\(backdrop-filter:\s*blur\(1px\)\)/i);
assert.doesNotMatch(shell, /animation:\s*(?!none)/i);
```

- [ ] **Step 2: 确认测试为红色**

Run: `cd apps/owstats-desktop && npx tsx scripts/visualSystem.test.ts`

Expected: 新令牌与渐变规则不存在而失败。

- [ ] **Step 3: 最小实现**

在 `tokens.css` 添加：

```css
--canvas-glow-cool: rgba(198, 224, 222, 0.52);
--canvas-glow-light: rgba(250, 249, 241, 0.72);
--glass-primary: rgba(244, 246, 240, 0.68);
--glass-secondary: rgba(250, 251, 247, 0.52);
--glass-border: rgba(255, 255, 255, 0.58);
--glass-shadow: 0 18px 48px rgba(45, 54, 48, 0.08);
--glass-blur: 16px;
```

将 `.appFrame` 改为 `--app-canvas`、`--canvas-glow-cool` 和 `--canvas-glow-light` 的静态径向渐变；增加 `@supports (backdrop-filter: blur(1px))` 玻璃增强。回退保持更不透明浅色背景，且不写动画。

- [ ] **Step 4: 确认测试为绿色并提交**

Run: `cd apps/owstats-desktop && npx tsx scripts/visualSystem.test.ts`

Expected: exit code 0。

Run: `git add apps/owstats-desktop/scripts/visualSystem.test.ts apps/owstats-desktop/src/styles/tokens.css apps/owstats-desktop/src/styles/shell.css; git commit -m "style: add command center glass canvas tokens"`

### Task 2: 指标带与玻璃层级

**Files:**

- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`

- [ ] **Step 1: 写失败的布局契约**

添加：

```ts
const commandCenterStyles = readFileSync(new URL("../src/styles/features/command-center.css", import.meta.url), "utf8");
assertEqual(/\.commandCenterMetricsRow\s*\{[\s\S]*?border:\s*0;/.test(commandCenterStyles), true, "metrics strip removes its heavy outer border");
assertEqual(/\.commandCenterSyncPanel,[\s\S]*?\.commandCenterAccountPanel[\s\S]*?background:\s*var\(--glass-primary\);/.test(commandCenterStyles), true, "main panels use the primary glass surface");
assertEqual(/@media \(max-width: 1440px\)[\s\S]*?\.commandCenterMetricsRow\s*\{[\s\S]*?grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\);/.test(commandCenterStyles), true, "1440px folds metrics into two columns");
```

- [ ] **Step 2: 确认测试为红色**

Run: `cd apps/owstats-desktop && npx tsx scripts/phase6Shell.test.ts`

Expected: 新表面契约失败。

- [ ] **Step 3: 最小实现**

添加以下 CSS，并保留现有 2:1 主从结构、同步和健康状态类：

```css
.commandCenterMetricsRow { border: 0; border-radius: 0; background: transparent; box-shadow: none; }
.commandCenterSyncPanel, .commandCenterAccountPanel { background: var(--glass-primary); border-color: var(--glass-border); box-shadow: var(--glass-shadow); }
.commandCenterTools { background: var(--glass-secondary); border-color: var(--glass-border); box-shadow: none; }
.commandCenterDangerZone { background: rgba(248, 231, 229, 0.34); border-color: rgba(229, 72, 72, 0.22); box-shadow: none; }
@supports (backdrop-filter: blur(1px)) { .commandCenterSyncPanel, .commandCenterAccountPanel, .commandCenterTools { backdrop-filter: blur(var(--glass-blur)); -webkit-backdrop-filter: blur(var(--glass-blur)); } }
@media (max-width: 1440px) { .commandCenterMetricsRow { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
```

- [ ] **Step 4: 确认测试为绿色并提交**

Run: `cd apps/owstats-desktop && npx tsx scripts/phase6Shell.test.ts`

Expected: exit code 0。

Run: `git add apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/src/styles/features/command-center.css; git commit -m "style: refine command center glass hierarchy"`

### Task 3: 固定赛季分段选择

**Files:**

- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`

- [ ] **Step 1: 写失败的结构与身份流程保护断言**

将“未硬编码 S21”断言替换为：

```ts
assertEqual(onboardingSource.includes("const COMMAND_CENTER_SEASONS = [21, 22, 23] as const"), true, "command center exposes fixed season segments");
assertEqual(onboardingSource.includes('aria-label="选择同步赛季"'), true, "season picker has a Chinese accessible name");
assertEqual(onboardingSource.includes('aria-label="赛季号"'), false, "numeric season input is removed");
assertEqual(onboardingSource.includes("confirmLoginTargetSwitch"), true, "automatic target switching remains");
assertEqual(onboardingSource.includes("syncSeason(season)"), true, "single-season sync remains");
```

- [ ] **Step 2: 确认测试为红色**

Run: `cd apps/owstats-desktop && npx tsx scripts/phase6Shell.test.ts`

Expected: 固定分段常量和无输入框契约失败。

- [ ] **Step 3: 最小实现**

在常量区加入：

```ts
const COMMAND_CENTER_SEASONS = [21, 22, 23] as const;
```

替换赛季减号、输入框和加号：

```tsx
<div className="commandCenterSeasonSegments" role="group" aria-label="选择同步赛季">
  {COMMAND_CENTER_SEASONS.map((season) => (
    <button key={season} className={season === selectedSeason ? "active" : ""} type="button" aria-pressed={season === selectedSeason} onClick={() => setSeasonInput(String(season))}>
      {`S${season}`}
    </button>
  ))}
</div>
```

添加浅色分段容器和深炭激活项；按钮保持 44px 命中区。`selectedSeason`、`runSeasonSync`、状态卡与头部同步按钮保持不变。

- [ ] **Step 4: 确认测试为绿色并提交**

Run: `cd apps/owstats-desktop && npx tsx scripts/phase6Shell.test.ts`

Expected: exit code 0，自动 BattleTag 与单赛季同步断言同时通过。

Run: `git add apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/styles/features/command-center.css; git commit -m "style: use segmented command center seasons"`

### Task 4: 账号、工具与危险操作

**Files:**

- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`

- [ ] **Step 1: 写失败的次级区域契约**

```ts
assertEqual(/\.commandCenterAccountPanel \.phase6ProfileAvatar\s*\{[\s\S]*?width:\s*56px;[\s\S]*?height:\s*56px;/.test(commandCenterStyles), true, "account identity uses a compact neutral avatar");
assertEqual(/\.commandCenterTools \.phase6SettingsLauncherGrid\s*\{[\s\S]*?grid-template-columns:\s*repeat\(4, minmax\(0, 1fr\)\);/.test(commandCenterStyles), true, "tools use four columns on wide screens");
assertEqual(/\.commandCenterDangerZone\s*\{[\s\S]*?background:\s*rgba\(248, 231, 229, 0\.34\);/.test(commandCenterStyles), true, "logout stays a light danger treatment");
```

- [ ] **Step 2: 确认测试为红色**

Run: `cd apps/owstats-desktop && npx tsx scripts/phase6Shell.test.ts`

Expected: 头像与工具行契约失败。

- [ ] **Step 3: 最小实现**

```css
.commandCenterAccountPanel .phase6ProfileAvatar { width: 56px; height: 56px; background: var(--glass-secondary); border: 1px solid var(--glass-border); color: var(--data-secondary); }
.commandCenterAccountPanel .phase6ProfileAvatar img { width: 100%; height: 100%; object-fit: cover; }
.commandCenterTools .phase6SettingsLauncherGrid { grid-template-columns: repeat(4, minmax(0, 1fr)); }
@media (max-width: 1440px) { .commandCenterTools .phase6SettingsLauncherGrid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
```

不得修改自动识别、确认切换、保留原目标、重新绑定、重新识别、清除或退出登录行为与禁用条件。

- [ ] **Step 4: 确认测试为绿色并提交**

Run: `cd apps/owstats-desktop && npx tsx scripts/phase6Shell.test.ts`

Expected: exit code 0。

Run: `git add apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/src/styles/features/command-center.css; git commit -m "style: polish command center account and tools"`

### Task 5: 全量验证与匿名视觉验收

**Files:**

- Verify only: `apps/owstats-desktop`

- [ ] **Step 1: 运行前端测试与构建**

Run: `cd apps/owstats-desktop && npm test && npm run build`

Expected: exit code 0。

- [ ] **Step 2: 运行 Rust 验证与打包**

Run: `cd apps/owstats-desktop/src-tauri && cargo fmt --check && cargo test --tests; cd .. && npm run tauri build`

Expected: 所有命令 exit code 0。

- [ ] **Step 3: 匿名视觉检查**

在 `2540x1344`、`1920x1080`、`1440x900` 检查静态渐变、玻璃回退、四/二列指标、工具折行、键盘焦点、空/错误/禁用及 reduced-motion。只使用 `scripts/uiVisualAcceptance.fixture.ts` 合成数据，不读取 app-data、凭据或浏览器 profile。

- [ ] **Step 4: 审计提交与工作区**

Run: `git diff --check; git status --short`

Expected: `git diff --check` 无输出，所有实现文件已由各切片提交。
