# Achievement Rule Corrections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修正八项生涯成就的统计、阈值与中文展示，并完整删除“胜率机器”。

**Architecture:** 后端 `review/achievements.rs` 继续作为成就定义和计算的唯一来源，返回稳定中文条件、当前等级、可选解锁时间和“招牌打法”的动态标签说明。前端仅根据 `currentLevel` 渲染已达成状态，并对非空 `unlockedAt` 格式化日期；资源和视觉 fixture 同步移除 `win-rate`。

**Tech Stack:** Rust、rusqlite、Tauri 2、React、TypeScript、Node assert 测试。

---

### Task 1: 固化后端规则契约

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`

- [ ] **Step 1: 写删除与静态文案的失败测试**

在 `career_achievement_contract_is_stable` 中移除 `win-rate` 预期，并断言：

```rust
assert!(!keys.contains(&"win-rate"));
assert_eq!(
    achievement("win-streak").tiers.iter().map(|tier| tier.requirement.as_str()).collect::<Vec<_>>(),
    ["单赛季最长连胜达到 5 局", "单赛季最长连胜达到 10 局", "单赛季最长连胜达到 15 局"]
);
assert_eq!(
    achievement("comeback").tiers[0].requirement,
    "完成 1 场角斗领域的让三追四"
);
```

- [ ] **Step 2: 写计算口径的失败测试**

使用合成对局覆盖：同赛季 15 连胜、S 级与带飞局 50 次、同一高光标签 50 次、多个英雄的最终第一英雄占比、玻璃大炮 8,000 承伤，以及招牌标签并列时最早达到次数者。核心断言：

```rust
assert_eq!(achievement("win-streak").current_level, 3);
assert_eq!(achievement("s-grade").current_level, 3);
assert_eq!(achievement("carry").current_level, 3);
assert_eq!(achievement("signature").current_level, 3);
assert_eq!(achievement("signature").detail, "KDA最高");
assert_eq!(achievement("hero-focus").current_level, 2);
assert!(achievement("hero-focus").tiers.iter().all(|tier| tier.unlocked_at.is_none()));
assert_eq!(achievement("glass-cannon").current_level, 3);
```

- [ ] **Step 3: 运行测试并确认按旧实现失败**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test career_achievements
```

Expected: FAIL，失败点包括仍存在 `win-rate`、旧阈值、旧承伤上限、逐场提前解锁的本命专精或静态招牌说明。

### Task 2: 修改后端定义与计算

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`

- [ ] **Step 1: 更新定义表**

将 `DEFINITIONS` 改为 16 项，删除 `win-rate`，并写入：

```rust
requirements: ["单赛季最长连胜达到 5 局", "单赛季最长连胜达到 10 局", "单赛季最长连胜达到 15 局"]
requirements: ["累计获得 10 场 S 级表现", "累计获得 30 场 S 级表现", "累计获得 50 场 S 级表现"]
requirements: ["累计完成 10 场带飞局", "累计完成 30 场带飞局", "累计完成 50 场带飞局"]
requirements: ["同一高光标签累计出现 10 次", "同一高光标签累计出现 30 次", "同一高光标签累计出现 50 次"]
requirements: ["完成 1 场角斗领域的让三追四", "完成 5 场角斗领域的让三追四", "完成 15 场角斗领域的让三追四"]
```

玻璃大炮三级条件的承伤上限均写为 `8,000`。

- [ ] **Step 2: 修改连胜、累计次数与玻璃大炮**

使用 `[5, 10, 15]`、`[10, 30, 50]` 和统一的 `damage_taken <= 8_000` 替换旧阈值；删除胜率累计变量与 `set_dates(..., "win-rate", ...)`。

- [ ] **Step 3: 按最终生涯分布计算本命专精**

完成所有事实扫描后计算一次：

```rust
let total = hero_counts.values().sum::<i64>();
let top = hero_counts.values().copied().max().unwrap_or(0);
let share = (total > 0).then_some(top as f64 * 100.0 / total as f64);
set_current_level_without_dates(
    achievements,
    "hero-focus",
    [25.0, 40.0, 55.0].into_iter().filter(|threshold| share.is_some_and(|value| value >= *threshold)).count() as i64,
);
```

新增的辅助函数只设置 `current_level`，保持三个 `unlocked_at = None`。

- [ ] **Step 4: 计算稳定的招牌标签**

为每个允许的高光标签记录累计次数和首次出现顺序；按“次数降序、首次出现顺序升序”选出展示标签，并把对应 `CareerAchievement.detail` 改成标签原文。三级日期仍以该最终胜出标签第 10 / 30 / 50 次出现的对局时间计算。

- [ ] **Step 5: 运行后端定向测试**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test career_achievements
```

Expected: PASS。

### Task 3: 修正前端已达成状态与资源清单

**Files:**
- Modify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Modify: `apps/owstats-desktop/scripts/reviewAchievements.test.ts`
- Modify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`
- Modify: `apps/owstats-desktop/runtime-config/assets-inventory.json`

- [ ] **Step 1: 写本命专精无日期仍显示已达成的失败测试**

在 `reviewAchievements.test.ts` 读取 `AchievementWall.tsx` 并断言等级状态由 `currentLevel` 决定：

```ts
assert.match(wall, /achievementTier\.level <= achievement\.currentLevel/);
assert.match(wall, /unlockDate \?/);
```

同时断言资源 key 和 fixture key 不包含 `win-rate`。

- [ ] **Step 2: 运行前端定向测试并确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewAchievements.test.ts
npx tsx scripts/phase6Assets.test.ts
```

Expected: 至少一项 FAIL，因为详情当前以 `unlockedAt !== null` 判断已达成，资源列表仍含 `win-rate`。

- [ ] **Step 3: 修改详情等级状态**

在 `AchievementDialog` 中使用：

```tsx
const unlocked = achievementTier.level <= achievement.currentLevel;
```

用 `unlocked` 设置 `unlocked/locked` class；日期仍仅在 `formatAchievementUnlockDate` 返回非空时显示。

- [ ] **Step 4: 移除胜率机器资源引用**

从 `phase6Assets.test.ts`、`uiVisualAcceptance.fixture.ts` 和生成的 `assets-inventory.json` 中删除 `win-rate` key 或资源项，不删除无关资产。若 inventory 由脚本生成，则运行 `npm run prepare:assets` 更新。

- [ ] **Step 5: 运行前端定向测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/reviewAchievements.test.ts
npx tsx scripts/phase6Assets.test.ts
```

Expected: PASS。

### Task 4: 全量验证与隐私边界确认

**Files:**
- Verify only: all modified files

- [ ] **Step 1: 后端格式与测试**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: PASS。

- [ ] **Step 2: 前端测试与构建**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: PASS。

- [ ] **Step 3: 差异检查**

```powershell
cd D:\Codex\OW_data\owstats-local
git diff --check
git status --short
```

Expected: `git diff --check` 无错误；状态只新增本任务预期修改以及开始前已存在的用户改动。

- [ ] **Step 4: 交付记录**

报告实际执行命令、通过/失败结果、未执行的 `npm run tauri build`、未触碰真实凭据或真实隐私数据。除非用户明确要求，不暂存、不提交。
