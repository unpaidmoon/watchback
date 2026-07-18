# 战报与成就一级导航重构设计

## 目标

将战报页现有的“概览”“队友协作”“成就”三个页内二级入口重组为两个一级页面：

- “战报”合并原概览与队友协作，形成连续的赛季复盘阅读流。
- “成就”使用独立路由和顶部胶囊导航入口，保留现有成就墙与详情交互。
- 仅调整 React 前端的信息架构、路由、组件组合与样式，不修改 Tauri、Rust、SQLite、DTO、统计口径或数据接口。

## 已确认的产品决策

1. 采用独立页面与独立路由方案。
2. 顶部胶囊导航顺序固定为：`对局 | 战报 | 成就 | 排行 | 生涯 MMR | 指挥台`。
3. 战报页删除整个页内二级菜单，不保留隐藏标签、分段按钮或同级替代控件。
4. 队友协作完整放在段位总览与地图分析之间，内部模块、排序、跳转和空状态保持不变。
5. 成就单独成页，现有深炭黑成就墙、八项成就、等级进度、证据入口和详情弹窗保持不变。
6. 本轮不重新设计模块视觉，不引入新的颜色、字体、圆角、阴影或卡片语言。

## 信息架构

### 顶部胶囊导航

`phase6NavItems` 新增“成就”，并调整“排行”与“生涯 MMR”的位置：

```text
对局 → 战报 → 成就 → 排行 → 生涯 MMR → 指挥台
```

- “战报”指向 `/review`。
- “成就”指向 `/achievements`。
- 两个入口使用现有 `NavLink` 激活规则，各自只在对应一级页面高亮。
- 顶部导航继续使用 `design.md` 规定的浅灰绿胶囊容器、深炭黑当前项和最小交互尺寸，不增加图标或彩色强调。

### 战报页

战报页标准地址为 `/review?season=<season>`，页面内容顺序固定为：

```text
页面标题与赛季选择
段位总览
队友协作
地图分析
```

其中：

- “段位总览”复用现有 `RoleAndHeroView`，包含常用英雄、赛季对局、总胜率、预设/开放竞技段位与角斗竞技段位。
- “队友协作”完整复用现有 `FriendAnalysisModules`，包含好友亲密度、高频组合和队友协作榜单；不得拆分、删项或改变样本门槛。
- “地图分析”继续复用现有 `MapAnalysisModules`，并保持地图卡片跳转排行页时携带当前赛季。
- 三个区域在同一个主滚动容器内纵向排列。宽屏可以保留各模块内部的多列网格，但不得把三个一级区域并排或引入内部滚动。

### 成就页

新增一级页面 `/achievements?season=<season>`：

- 页面标题为“成就”。
- 页面说明使用当前选中赛季，例如“S23 赛季成就与进度”。
- 页面保留独立的 `PageSeasonSelect`，赛季选项继续来自 `PersonalSeasonReview.availableSeasons`，不引入全局赛季状态。
- 成就墙复用现有成就计算结果、徽章资源、四列/两列响应式网格和浅色详情弹窗。
- 点击、键盘焦点、Escape 关闭、焦点约束、证据链接和未激活状态全部保持现状。
- 成就页不请求 `getSeasonAnalysis`，因为队友协作和地图分析不在该页面显示。

## 前端组件边界

### 页面层

- `PersonalReviewPage` 只负责战报标题、赛季状态、个人战报请求、赛季分析请求，以及“段位总览 → 队友协作 → 地图分析”的组合。
- 新增 `AchievementsPage`，负责成就标题、赛季状态、个人战报请求、成就墙和详情弹窗。
- `App.tsx` 注册 `/achievements` 路由；现有 `/review`、好友详情与旧好友分析路由继续保留。

### 共享层

- 将两页共同使用的个人战报加载、动态赛季选择、后端赛季回退、加载/错误/空状态逻辑整理为前端共享 hook 或小型状态模块，避免复制两套请求竞态处理。
- 继续调用 `requestPersonalSeasonReviewOnce`，保留按账号隔离的现有请求缓存与失效行为。
- 将成就墙与成就详情弹窗整理为可由 `AchievementsPage` 直接使用的前端组件；成就计算继续复用 `shared/reviewAchievements.ts`，不改变阈值和文案。
- `FriendAnalysisModules` 与 `MapAnalysisModules` 保持可复用模块边界，不改其数据契约。

## 数据流与功能保持

### 战报

1. 从 URL 读取可选 `season`。
2. 使用现有个人战报请求取得段位总览、动态赛季列表及其他战报数据。
3. 使用同一个选中赛季调用 `getSeasonAnalysis`。
4. 将 `analysis.friend` 交给 `FriendAnalysisModules`，将完整 `analysis` 交给 `MapAnalysisModules`。
5. 切换赛季时同时刷新个人战报和赛季分析；不得出现两个模块混用不同赛季。

### 成就

1. 从 URL 读取可选 `season`。
2. 使用同一个个人战报请求取得动态赛季列表与成就源数据。
3. 使用现有 `buildReviewAchievements` 生成八项成就。
4. 切换赛季只刷新个人战报，不发起赛季分析请求。

本轮不得增加、删除或变更任何后端 command、DTO 字段、SQLite 查询、成就阈值、好友分析门槛或地图统计规则。

## 好友详情返回行为

- 队友协作榜单进入 `/analytics/friends/detail` 的功能保持不变。
- 新生成的返回上下文使用 `/review?season=<season>`，不再写入 `view=friends`。
- 返回时继续通过 `sessionStorage` 恢复战报页原滚动位置，使合并成长页后不会丢失阅读位置。
- 好友详情中的“返回战报”、失效入口说明和错误恢复动作继续使用中文，页面本身不改版。

## 旧 URL 兼容

前端对旧书签和历史跳转做一次无新增请求的规范化：

- `/review?season=23&view=overview` 替换为 `/review?season=23`。
- `/review?season=23&view=friends` 替换为 `/review?season=23`。
- `/review?season=23&view=achievements` 替换为 `/achievements?season=23`。
- 历史 `view=roles`、`view=maps` 或未知值统一清理为 `/review?season=23`。

规范化使用前端 replace 导航，避免浏览器返回键在旧地址与新地址之间循环。`season` 之外的无关 query 不作为本轮新增契约。

## 样式调整

- 删除 `.reviewTabs` 及其按钮、激活态和窄屏规则。
- 将依赖 `data-view="overview"` 的布局改为战报连续内容流的明确类名，不保留不可达的视图样式。
- 段位总览、队友协作和地图分析之间使用 `16-24px` 模块间距；不增加新的外层嵌套卡片。
- 成就页继续沿用 `2026-07-19-review-achievement-wall-dark-design.md` 的深色陈列面规则。
- 在 `2540×1344`、`1920×1080` 和 `1440×900` 下保持单一页面纵向滚动；模块内部按现有断点重排。

## 加载、空状态与错误处理

- 战报个人数据加载失败时继续显示页面级 `PageErrorState`，重试只重发个人战报请求。
- 战报赛季分析加载或失败时，队友协作与地图分析继续显示各自现有的加载、空或可恢复错误状态；个人段位总览不因分析失败消失。
- 成就页使用独立的页面级加载、错误和“暂无可用赛季战报”空状态，重试只重发个人战报请求。
- 后端返回实际赛季与 URL 不一致时，两页都沿用现有回退逻辑，以 replace 方式更新 `season`。
- 请求竞态防护、账号切换后的缓存失效和错误文案保持现有行为。

## 测试与验收

### 静态与组件契约

- 顶部导航标签和顺序严格等于：`对局,战报,成就,排行,生涯 MMR,指挥台`。
- `/review` 不再渲染 `reviewTabs`、`ReviewTabButton` 或“概览 / 队友协作 / 成就”二级入口。
- 战报源码按 `RoleAndHeroView`、`FriendAnalysisModules`、`MapAnalysisModules` 的顺序组合。
- `/achievements` 注册独立页面，渲染现有八项成就和详情弹窗。
- 战报切换赛季同时刷新个人战报与分析；成就切换赛季不调用 `getSeasonAnalysis`。
- 旧 `view` URL 按兼容规则规范化且保留 `season`。
- 好友详情返回地址不再包含 `view=friends`，并继续恢复滚动位置。

### 现有回归

- 更新 `phase6Shell.test.ts`、`rankingCommandCenterRedesign.test.ts`、`analysisMovedToReview.test.ts`、`uiVisualAcceptance.fixture.ts` 与受路由变化影响的战报导航测试。
- 保留并运行成就墙交互与视觉断言，包括 `reviewAchievementWall.test.ts` 和 `reviewVisualPolish.test.ts`。
- 保留账号级战报缓存、多赛季隔离、页面空状态与错误状态测试。

### 验证命令

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

随后运行：

```powershell
git diff --check
git status --short
```

视觉验收使用匿名 fixture，在 `2540×1344`、`1920×1080`、`1440×900` 检查：

1. 顶部导航顺序、当前项高亮和胶囊宽度没有拥挤或换行。
2. 战报二级菜单完全消失。
3. 队友协作完整位于段位总览与地图分析之间。
4. 战报长页只有主滚动，好友详情返回后位置正确。
5. 成就页深色墙、响应式分隔线、键盘焦点和浅色详情弹窗保持现状。

测试、截图与报告只使用合成数据，不读取或输出真实凭据、真实 SQLite、隐私 ID 或真实数据 dump。

## 非目标

- 不改战报、队友协作、地图分析或成就的统计内容。
- 不改好友详情页布局。
- 不建立全局赛季 store，也不要求从一个一级页面跳到另一个一级页面时自动继承赛季。
- 不删除旧 `/analytics/friends` 路由。
- 不调整排行或生涯 MMR 页面内容；两者仅交换顶部导航位置。
- 不修改 Rust、Tauri、SQLite、资源 manifest 或发布路径。
