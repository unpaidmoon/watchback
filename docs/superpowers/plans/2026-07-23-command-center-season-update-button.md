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
