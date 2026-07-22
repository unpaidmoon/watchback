# Achievement Wall Card Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将成就墙卡片精简为图标、名称和简短说明，同时保留点击详情弹窗并移除紫色聚焦描边。

**Architecture:** 只修改现有 `AchievementWall` 的卡片渲染和成就页局部样式，不改变共享成就模型及详情弹窗。通过源码契约测试锁定卡片内容与焦点样式，再运行前端回归测试和构建。

**Tech Stack:** React、TypeScript、CSS、Node.js `assert` 源码契约测试。

---

### Task 1: 锁定精简后的卡片契约

**Files:**
- Modify: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 添加会失败的卡片内容断言**

将组件源码与页面局部样式分别读取，并断言卡片函数仅包含目标内容：

```ts
const component = readFileSync(new URL("../src/features/review/AchievementWall.tsx", import.meta.url), "utf8");
const reviewStyles = readFileSync(new URL("../src/styles/features/review.css", import.meta.url), "utf8");
const cardSource = component.slice(component.indexOf("function AchievementCard"), component.indexOf("function AchievementDialog"));

for (const required of ["phase6AchievementMedalImage", "achievement.title", "achievement.detail", "onClick={onOpen}"]) {
  assert.equal(cardSource.includes(required), true, `achievement card should keep ${required}`);
}
for (const removed of ["achievement.metric", "phase6AchievementProgress", "phase6AchievementTierRail", "phase6AchievementNext", "<Evidence"]) {
  assert.equal(cardSource.includes(removed), false, `achievement card should remove ${removed}`);
}
assert.equal(component.includes("function AchievementDialog"), true, "achievement detail dialog should remain available");
assert.doesNotMatch(
  reviewStyles,
  /\.reviewWorkspace\s+\.phase6AchievementCard:focus-within\s*\{[^}]*var\(--data-primary\)/s,
  "achievement card focus should not use the purple data accent",
);
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: FAIL，失败原因包含卡片仍有 `achievement.metric` 或聚焦态仍使用 `--data-primary`。

### Task 2: 精简成就卡片组件

**Files:**
- Modify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx:1-25`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 删除卡片专用的对局依赖与渲染元素**

保留弹窗依赖，将卡片实现改为：

```tsx
function AchievementCard({ achievement, onOpen }: { achievement: ReviewAchievement; onOpen: () => void }) {
  const activeLevel = activeBadgeLevel(achievement);
  return <article className="phase6AchievementCard"><button className="phase6AchievementCardButton" type="button" onClick={onOpen}><img className={`phase6AchievementMedalImage ${achievement.currentLevel === 0 ? "locked" : ""}`} src={achievementBadgeUrl(achievement.key, activeLevel)} alt={achievement.title} /><div className="phase6AchievementCopy"><strong>{achievement.title}</strong><small>{achievement.detail}</small></div></button></article>;
}
```

同时删除 `ExternalLink`、`Link`、`HeroAvatar`、`MapPreview`、`EvidenceLink` 和 `Evidence`；保留 `X`、弹窗所需的 `formatThreshold` 与 `romanLevels`。

- [ ] **Step 2: 运行测试确认内容契约只剩样式断言失败**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: FAIL，仅剩紫色聚焦描边相关断言失败。

### Task 3: 收紧布局并移除紫色描边

**Files:**
- Modify: `apps/owstats-desktop/src/styles/shared.css:4471-4646`
- Modify: `apps/owstats-desktop/src/styles/features/review.css:132-203`
- Test: `apps/owstats-desktop/scripts/reviewAchievementWall.test.ts`

- [ ] **Step 1: 调整共享卡片基础布局**

将卡片最小高度收紧，并使按钮内容居中；删除只服务于已移除元素的选择器：

```css
.phase6AchievementCard {
  min-height: 240px;
}

.phase6AchievementCardButton {
  justify-content: center;
  gap: 8px;
  padding: 24px 16px;
}

.phase6AchievementCopy {
  gap: 8px;
}
```

删除 `.phase6AchievementCopy strong span`、`.phase6AchievementCopy > span`、`.phase6AchievementProgress`、`.phase6AchievementTierRail`、`.phase6AchievementNext` 和卡片 `.phase6EvidenceLink` 的规则；弹窗进度样式必须保留。

- [ ] **Step 2: 将聚焦态改为无描边的背景反馈**

将页面局部聚焦样式改为：

```css
.reviewWorkspace .phase6AchievementCard:focus-within {
  border-color: transparent;
  background: color-mix(in srgb, var(--action-on-primary) 6%, transparent);
  box-shadow: none;
  outline: none;
}
```

删除页面局部样式中只服务于等级标签、指标、卡片进度、等级轨道、下一等级和证据入口的覆盖规则；保留 `.phase6AchievementModalProgress` 规则。

- [ ] **Step 3: 运行目标测试并确认通过**

Run: `cd apps/owstats-desktop; npx tsx scripts/reviewAchievementWall.test.ts`

Expected: PASS，进程退出码为 0。

### Task 4: 回归与交付验证

**Files:**
- Verify: `apps/owstats-desktop/src/features/review/AchievementWall.tsx`
- Verify: `apps/owstats-desktop/src/styles/shared.css`
- Verify: `apps/owstats-desktop/src/styles/features/review.css`

- [ ] **Step 1: 运行完整前端测试**

Run: `cd apps/owstats-desktop; npm test`

Expected: PASS，全部前端测试退出码为 0。

- [ ] **Step 2: 运行生产构建**

Run: `cd apps/owstats-desktop; npm run build`

Expected: PASS，Vite/TypeScript 构建退出码为 0。

- [ ] **Step 3: 检查补丁格式和文件范围**

Run: `git diff --check; git status --short`

Expected: `git diff --check` 无输出；状态仅包含本次设计、计划、测试、组件和样式文件，以及用户原有改动。

- [ ] **Step 4: 报告隐私与未验证项**

交付信息必须列出实际执行命令、结果、未运行的后端与 Tauri 发布验证，并明确本次未读取真实凭据或真实隐私数据。
