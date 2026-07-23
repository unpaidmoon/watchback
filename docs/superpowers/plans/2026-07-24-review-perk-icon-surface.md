# 战报威能图标底板视觉修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将战报页普通对局威能排行图标改为 `--surface-primary` 底、灰色描边和浅灰线稿，同时保持角斗异能和装备样式不变。

**Architecture:** 在排行行组件中根据已有 `kind` 添加威能专属 CSS 修饰 class，用该 class 覆盖通用图标容器背景并添加边框。现有前端源码契约测试负责锁定作用范围与视觉声明。

**Tech Stack:** React、TypeScript、CSS、Node.js `assert`

---

### Task 1: 添加威能专属图标底板

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts`
- Modify: `apps/owstats-desktop/src/features/review/ChoiceRankingsPanel.tsx`
- Modify: `apps/owstats-desktop/src/styles/shared.css`

- [ ] **Step 1: 写入失败的契约测试**

在 `reviewChoiceRankings.test.ts` 添加：

```ts
assert.match(panel, /phase6ChoiceRankingIcon--perk/);
assert.match(
  styles,
  /\.phase6ChoiceRankingIcon--perk\s*\{[\s\S]*background:\s*var\(--surface-primary\);[\s\S]*border:\s*1px solid var\(--glass-border\);/,
);
assert.match(
  styles,
  /\.phase6ChoiceRankingIcon--perk img\s*\{[\s\S]*filter:\s*brightness\(0\);[\s\S]*opacity:\s*0\.46;/,
);
```

- [ ] **Step 2: 运行测试并确认按预期失败**

运行：

```powershell
cd apps/owstats-desktop
node --import tsx scripts/reviewChoiceRankings.test.ts
```

预期：断言失败，提示尚不存在 `phase6ChoiceRankingIcon--perk`。

- [ ] **Step 3: 添加最小实现**

在 `ChoiceRankingsPanel.tsx` 中将图标容器改为：

```tsx
<span
  className={`phase6ChoiceRankingIcon${kind === "perk" ? " phase6ChoiceRankingIcon--perk" : ""}`}
  aria-hidden="true"
>
```

在 `shared.css` 的通用图标容器规则之后添加：

```css
.phase6ChoiceRankingIcon--perk {
  background: var(--surface-primary);
  border: 1px solid var(--glass-border);
}

.phase6ChoiceRankingIcon--perk img {
  filter: brightness(0);
  opacity: 0.46;
}
```

- [ ] **Step 4: 运行定向测试并确认通过**

运行：

```powershell
cd apps/owstats-desktop
node --import tsx scripts/reviewChoiceRankings.test.ts
```

预期：退出码为 `0`。

- [ ] **Step 5: 执行完整前端验证**

运行：

```powershell
cd apps/owstats-desktop
npm test
npm run build
```

预期：全部测试通过，Vite 构建成功。

- [ ] **Step 6: 检查差异并提交**

运行：

```powershell
git diff --check
git status --short
git add apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts apps/owstats-desktop/src/features/review/ChoiceRankingsPanel.tsx apps/owstats-desktop/src/styles/shared.css
git commit -m "fix: clarify review perk icons"
```

预期：仅三个目标文件被纳入实现提交。
