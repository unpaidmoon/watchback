# 指挥台单赛季更新按钮视觉增强实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让指挥台每个赛季的“更新”按钮以清晰但克制的次级按钮样式从表格背景中分离。

**Architecture:** 保持 React 组件、同步逻辑和表格布局不变，只在指挥台功能样式中增强 `.syncedSeasonUpdateButton`。使用现有语义令牌定义常态、悬停和禁用状态，并通过既有脚本测试锁定样式边界。

**Tech Stack:** React、TypeScript、CSS、Node.js 测试脚本、Vite

---

## 文件结构

- 修改 `apps/owstats-desktop/scripts/phase6Shell.test.ts`：增加单赛季更新按钮的视觉样式回归断言。
- 修改 `apps/owstats-desktop/src/styles/features/command-center.css`：实现局部次级按钮表面、边框和交互状态。
- 不修改 `SeasonSyncWorkbench.tsx`：现有语义按钮、文案、禁用条件与焦点入口已满足需求。

### Task 1：增加失败的视觉回归测试

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] **Step 1：写入按钮常态、悬停和禁用状态断言**

在指挥台样式断言附近加入：

```ts
assertEqual(
  /\.syncedSeasonUpdateButton\s*\{(?=[^}]*border:\s*1px solid var\(--border-subtle\))(?=[^}]*background:\s*var\(--surface-control\))(?=[^}]*color:\s*var\(--text-primary\))[^}]*\}/s.test(commandCenterStyles),
  true,
  "season update buttons use a distinct secondary surface",
);
assertEqual(
  /\.syncedSeasonUpdateButton:hover:not\(:disabled\)\s*\{[^}]*background:\s*color-mix\(in srgb, var\(--text-primary\) 5%, var\(--surface-control\)\);[^}]*\}/s.test(commandCenterStyles),
  true,
  "season update buttons provide restrained hover feedback",
);
assertEqual(
  /\.syncedSeasonUpdateButton:disabled\s*\{(?=[^}]*cursor:\s*not-allowed)(?=[^}]*opacity:\s*0\.58)[^}]*\}/s.test(commandCenterStyles),
  true,
  "disabled season update buttons remain visibly distinct",
);
```

- [ ] **Step 2：运行测试并确认因缺少样式而失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
```

Expected: 命令非零退出，失败信息包含 `season update buttons use a distinct secondary surface`。

### Task 2：实现局部按钮视觉层级

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css:345`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] **Step 1：添加最小样式实现**

将 `.syncedSeasonUpdateButton` 扩展为：

```css
.syncedSeasonUpdateButton {
  width: 80px;
  min-width: 80px;
  min-height: 44px;
  padding: 0 12px;
  border: 1px solid var(--border-subtle);
  background: var(--surface-control);
  color: var(--text-primary);
  transition: background-color 180ms ease, border-color 180ms ease, color 180ms ease, opacity 180ms ease;
}

.syncedSeasonUpdateButton:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text-primary) 5%, var(--surface-control));
}

.syncedSeasonUpdateButton:disabled {
  cursor: not-allowed;
  opacity: 0.58;
}
```

- [ ] **Step 2：重新运行定向测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
```

Expected: 命令零退出，输出包含 `phase6 shell tests passed`。

### Task 3：完整验证与交付检查

**Files:**
- Verify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Verify: `apps/owstats-desktop/src/styles/features/command-center.css`

- [ ] **Step 1：运行全部前端测试**

Run:

```powershell
cd apps\owstats-desktop
npm test
```

Expected: 零退出，所有测试通过。

- [ ] **Step 2：运行生产构建**

Run:

```powershell
cd apps\owstats-desktop
npm run build
```

Expected: 零退出，Vite 构建成功。

- [ ] **Step 3：检查补丁格式和工作树范围**

Run:

```powershell
git diff --check
git status --short
git diff -- apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/src/styles/features/command-center.css docs/superpowers/specs/2026-07-23-command-center-season-update-button-design.md docs/superpowers/plans/2026-07-23-command-center-season-update-button.md
```

Expected: `git diff --check` 零退出；差异只包含本任务样式、测试和两份说明文档，用户原有修改未被改动。

- [ ] **Step 4：报告验证与隐私边界**

交付说明列出实际执行命令、结果、未执行的后端/Tauri 发布验证，并明确本任务未读取或接触真实凭据及隐私数据。

### Task 4：增强设置工具按钮

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`

- [ ] **Step 1：写入失败的专用按钮样式断言**

断言 `.phase6SettingsPlaceholderAction` 使用 `1px solid var(--border-subtle)`、`var(--surface-control)`、悬停混色、禁用透明度和现有紫色焦点外环。

- [ ] **Step 2：运行定向测试并确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
```

Expected: 非零退出，失败信息指向设置工具按钮缺少独立次级表面。

- [ ] **Step 3：添加局部样式**

```css
.phase6CommandCenter .phase6SettingsPlaceholderAction {
  border: 1px solid var(--border-subtle);
  background: var(--surface-control);
  color: var(--text-primary);
  transition: background-color 180ms ease, border-color 180ms ease, color 180ms ease, opacity 180ms ease;
}

.phase6CommandCenter .phase6SettingsPlaceholderAction:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text-primary) 5%, var(--surface-control));
}

.phase6CommandCenter .phase6SettingsPlaceholderAction:disabled {
  cursor: not-allowed;
  opacity: 0.58;
}

.phase6CommandCenter .phase6SettingsPlaceholderAction:focus-visible {
  outline: 2px solid var(--data-primary);
  outline-offset: 2px;
}
```

- [ ] **Step 4：重新运行定向测试并确认通过**

Run: `npx tsx scripts/phase6Shell.test.ts`

Expected: 零退出。

### Task 5：对齐并着色数据健康标签

**Files:**
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] **Step 1：写入失败的结构与样式断言**

断言 `HealthItem` 不渲染 `CheckCircle2`，异常类名为 `bad`；宽屏为四列、每项占满列且文字居中，`good` 使用成功状态背景/边框，`bad` 使用危险状态背景/边框，`max-width: 720px` 下为两列。

- [ ] **Step 2：运行定向测试并确认失败**

Run: `npx tsx scripts/phase6Shell.test.ts`

Expected: 非零退出，失败信息指向健康标签仍为透明表面或仍含图标。

- [ ] **Step 3：实现无图标状态结构**

```tsx
function HealthItem({ label, ok }: { label: string; ok: boolean }) {
  return <span className={ok ? "good" : "bad"}>{label}</span>;
}
```

- [ ] **Step 4：实现四列等宽与状态表面**

```css
.phase6CommandCenter .phase6DataHealthGrid {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.phase6CommandCenter .phase6DataHealthGrid span {
  width: 100%;
  min-height: 36px;
  justify-content: center;
  padding: 0 8px;
  border-radius: var(--radius-control);
  color: var(--text-primary);
  font-weight: 400;
  text-align: center;
  white-space: normal;
}

.phase6CommandCenter .phase6DataHealthGrid span.good {
  border-color: color-mix(in srgb, var(--status-success) 34%, var(--glass-border));
  background: var(--status-success-bg);
}

.phase6CommandCenter .phase6DataHealthGrid span.bad {
  border-color: color-mix(in srgb, var(--status-danger) 34%, var(--glass-border));
  background: var(--status-danger-bg);
}
```

在现有 `@media (max-width: 720px)` 中改为两列。

- [ ] **Step 5：重新运行定向测试并确认通过**

Run: `npx tsx scripts/phase6Shell.test.ts`

Expected: 零退出。

### Task 6：将固定赛季目录改为无上限目标赛季

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/review/command_center.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/stab006_contracts.rs`
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css`
- Modify: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Modify: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`

- [ ] **Step 1：先写后端失败测试**

覆盖以下行为：

```rust
assert_eq!(empty_status.selected_season, Some(23));
assert_eq!(empty_status.minimum_sync_season, 15);
assert_eq!(empty_status.default_season, 23);
assert_eq!(status_with_s24.selected_season, Some(24));
assert!(get_onboarding_status(&db, OnboardingStatusRequest { season: Some(14) }).is_err());
assert_eq!(get_onboarding_status(&db, OnboardingStatusRequest { season: Some(25) })?.selected_season, Some(25));
```

DTO JSON 契约改为包含 `minimumSyncSeason`、`defaultSeason`，且不再包含 `currentSeason`、`syncableSeasons`。

- [ ] **Step 2：运行后端定向测试并确认失败**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test phase8_command_center
cargo test --test stab006_contracts
```

Expected: 编译或断言失败，因为新 DTO 与选择规则尚未实现。

- [ ] **Step 3：实现后端选择规则**

将 DTO 字段改为：

```rust
pub selected_season: Option<i64>,
pub minimum_sync_season: i64,
pub default_season: i64,
pub available_seasons: Vec<i64>,
```

将常量改为：

```rust
pub(super) const MIN_SYNC_SEASON: i64 = 15;
pub(super) const DEFAULT_SYNC_SEASON: i64 = 23;
```

选择逻辑：

```rust
let selected = match request.season {
    Some(season) if season < MIN_SYNC_SEASON => {
        anyhow::bail!("最低支持 S{MIN_SYNC_SEASON}");
    }
    Some(season) => season,
    None => available_seasons
        .iter()
        .copied()
        .filter(|season| *season >= MIN_SYNC_SEASON)
        .max()
        .unwrap_or(DEFAULT_SYNC_SEASON),
};
```

状态列表只过滤下限并降序，不设置上限。

- [ ] **Step 4：更新 TypeScript DTO 与页面快照选择**

```ts
export type OnboardingStatus = {
  selectedSeason: number | null;
  minimumSyncSeason: number;
  defaultSeason: number;
  availableSeasons: number[];
  // existing status fields remain unchanged
};
```

页面首次加载使用后端 `selectedSeason ?? defaultSeason`；已有页面内选择只要不低于 `minimumSyncSeason` 就保留。

- [ ] **Step 5：先写前端步进器失败测试**

断言组件：

- 不含 `<select>`、`syncableSeasons` 或“当前赛季”；
- 显示“目标赛季”和“最低支持 S15”；
- 减少按钮调用 `onSelectSeason(selectedSeason - 1)`；
- 增加按钮调用 `onSelectSeason(selectedSeason + 1)`；
- S15 时减少按钮禁用；
- 同步期间两个按钮都禁用。

- [ ] **Step 6：实现步进器**

新增 `.seasonTargetStepper`，内部使用两个带中文 `aria-label` 的按钮和中间 `output`：

```tsx
<div className="seasonTargetStepper" aria-label="选择目标赛季">
  <button type="button" aria-label="减少目标赛季" onClick={() => onSelectSeason(selectedSeason - 1)} disabled={syncingSeason !== null || selectedSeason <= minimumSyncSeason}>−</button>
  <output aria-live="polite">{`S${selectedSeason}`}</output>
  <button type="button" aria-label="增加目标赛季" onClick={() => onSelectSeason(selectedSeason + 1)} disabled={syncingSeason !== null}>+</button>
</div>
```

样式使用三列、44px 高度、现有控件表面与焦点令牌。

- [ ] **Step 7：验证未来赛季显式请求安全边界**

在既有真实抓取合成测试中确认 S24/S25 的 `request_season_candidates` 只产生 `Some(season)`，不会产生 `None`；S23 继续保留既有兼容候选顺序。不得读取真实凭据。

- [ ] **Step 8：运行前后端定向测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/phase8RealFetch.test.ts
cd src-tauri
cargo test --test phase8_command_center
cargo test --test stab006_contracts
cargo test --test phase8_real_fetch
```

Expected: 全部零退出。

### Task 7：完整验证与范围检查

- [ ] **Step 1：后端格式与测试**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

- [ ] **Step 2：前端测试与构建**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

- [ ] **Step 3：补丁检查**

```powershell
git diff --check
git status --short
```

确认只包含本任务和本轮已经明确修复的两个过期测试契约；不暂存、不提交、不读取真实隐私数据。
