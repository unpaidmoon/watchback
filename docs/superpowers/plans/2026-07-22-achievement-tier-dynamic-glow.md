# 成就等级动态图片光效 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用正式打包的透明 WebP 光效资源，为二、三级成就提供接近参考图的徽章中心发光与低成本动态效果。

**Architecture:** 两张可复用透明 WebP 分别提供紫蓝和金色射线/粒子纹理，通过 React 装饰层放在徽章后方。CSS 根据既有 `tier-N`、`unlocked`、悬停和焦点状态控制淡入、呼吸、慢旋转与火花闪烁，并为 reduced-motion 提供静态降级。

**Tech Stack:** React 18、TypeScript、CSS animations、WebP、Vite 静态资源管线、Node `assert` 契约测试

---

### Task 1: 生成并纳入正式光效资源

**Files:**
- Create: `assets/achievements/achievement-glow-tier-2.webp`
- Create: `assets/achievements/achievement-glow-tier-3.webp`
- Modify: `apps/owstats-desktop/runtime-config/assets-inventory.json`
- Test: `apps/owstats-desktop/scripts/prepareAssets.test.ts`

- [ ] **Step 1: 生成二级紫蓝光效源图**

使用 imagegen 生成正方形光效：深紫蓝中心光、放射线、少量冷色粒子；背景必须为均匀纯绿 `#00ff00`，不包含徽章、文字、边框或卡片。

- [ ] **Step 2: 生成三级金色光效源图**

使用 imagegen 生成正方形光效：暖金中心光、放射线、少量火花；背景必须为均匀纯绿 `#00ff00`，不包含徽章、文字、边框或卡片。

- [ ] **Step 3: 移除色键并转换为透明 WebP**

对两张源图运行：

```powershell
python C:\Users\cyb\.codex\skills\.system\imagegen\scripts\remove_chroma_key.py --input <source.png> --out <tier.webp> --auto-key border --soft-matte --transparent-threshold 12 --opaque-threshold 220 --despill
```

用图像检查确认四角透明、光线边缘无明显绿色污染，并将最终文件保存到 `assets/achievements/`。

- [ ] **Step 4: 刷新并验证资源清单**

Run: `cd apps/owstats-desktop; node scripts/prepareAssets.mjs --refresh-inventory; npm run prepare:assets`

Expected: 两张 WebP 出现在 `public/assets/achievements/`，路径来自正式资源清单，无绝对路径。

### Task 2: 用失败测试固定装饰层和动画契约

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 添加失败断言**

新增断言，要求组件包含两层装饰结构且样式满足显示、动画与降级边界：

```ts
assert.equal(component.includes("phase6AchievementGlow"), true);
assert.equal(component.includes("phase6AchievementGlowParticles"), true);
assert.match(reviewStyles, /pointer-events:\s*none/);
assert.match(reviewStyles, /achievementGlowBreathe/);
assert.match(reviewStyles, /achievementGlowRotate/);
assert.match(reviewStyles, /achievementGlowSparkle/);
assert.match(reviewStyles, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
assert.match(reviewStyles, /achievement-glow-tier-2\.webp/);
assert.match(reviewStyles, /achievement-glow-tier-3\.webp/);
```

- [ ] **Step 2: 运行测试并确认 RED**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: FAIL，指出 `phase6AchievementGlow` 尚不存在。

### Task 3: 接入装饰层与动态显示规则

**Files:**
- Modify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/review.css`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 添加可复用光效装饰组件**

在 `AchievementWall.tsx` 中添加：

```tsx
function AchievementGlow() {
  return <span className="phase6AchievementGlow" aria-hidden="true"><span className="phase6AchievementGlowParticles" /></span>;
}
```

将其放入成就墙按钮和每个详情等级区，位于徽章图片之前。

- [ ] **Step 2: 定义资源、堆叠与交互边界**

在 `review.css` 中让卡片按钮和等级区建立定位上下文；光效层绝对定位于徽章区域、`pointer-events: none`，文字与徽章保持在其上方。二、三级分别引用：

```css
url("/assets/achievements/achievement-glow-tier-2.webp")
url("/assets/achievements/achievement-glow-tier-3.webp")
```

- [ ] **Step 3: 定义成就墙和详情显示条件**

默认光效透明；仅 `.phase6AchievementCard.tier-2/3:is(:hover, :focus-within)` 淡入。详情仅 `.phase6AchievementBadgeStage.tier-2/3.unlocked` 常驻显示；一级、locked 与 tier-0 不添加显示选择器。

- [ ] **Step 4: 定义低成本动画**

创建 `achievementGlowBreathe`、`achievementGlowRotate` 和 `achievementGlowSparkle`。只动画 `opacity` 与 `transform`；呼吸周期 3.2 秒，旋转周期 18 秒，三级粒子闪烁周期约 2.6 秒。

- [ ] **Step 5: 定义 reduced-motion 静态降级**

在既有 `prefers-reduced-motion: reduce` 媒体查询中禁用三个动画，但保留静态光效可见状态。

- [ ] **Step 6: 运行定向测试并确认 GREEN**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: PASS。

### Task 4: 视觉与回归验证

**Files:**
- Verify: `assets/achievements/achievement-glow-tier-2.webp`
- Verify: `assets/achievements/achievement-glow-tier-3.webp`
- Verify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Verify: `apps/owstats-desktop/src/styles/features/review.css`

- [ ] **Step 1: 检查资源透明度和体积**

确认两张 WebP 具有 alpha 通道、透明四角、无徽章或文字，并记录文件大小；若任一明显超过 150 KB，使用无明显色带的 WebP 压缩参数重新导出。

- [ ] **Step 2: 运行完整前端测试**

Run: `cd apps/owstats-desktop; npm test`

Expected: 成就相关测试通过；若仍仅出现已知 `aiMatchRoastUi.test.tsx` 文案断言失败，记录为与本次改动无关的既有阻断项。

- [ ] **Step 3: 运行前端构建**

Run: `cd apps/owstats-desktop; npm run build`

Expected: TypeScript、Vite 构建和 dist 路径扫描通过，两张 WebP 被复制到 `dist/assets/achievements/`。

- [ ] **Step 4: 检查补丁与工作树**

Run: `git diff --check; git status --short`

Expected: 无空白错误；保留用户既有改动，只新增本计划明确列出的资源、清单、组件、样式、测试和文档变更。

> 按仓库协作约束省略提交步骤：用户未授权暂存或提交。
