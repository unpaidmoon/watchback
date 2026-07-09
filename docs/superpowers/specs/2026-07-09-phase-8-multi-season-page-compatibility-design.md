# 第八阶段多赛季页面兼容回归设计

## 背景

前一轮第八阶段已经把指挥台升级为固定 S21 / S22 / S23 三张赛季状态卡片，并接入单赛季同步入口。项目最早的大量页面开发基于 `data/players/5479_s22_full_season.json` 这一份 S22 本地数据完成，现在本地 SQLite 和同步流程已经开始承载多赛季数据，下一阶段必须把旧的 S22 单赛季假设收束掉。

本阶段不是继续扩展同步协议，而是做多赛季页面兼容回归：确保对局、战报、排行、好友分析等页面在 S21 / S22 / S23 同库共存时按页面自己的赛季选择读取数据，不混赛季、不写死 S22、不引入全局当前赛季。

## 已确认决策

- 支持赛季仍固定为 S21、S22、S23。
- 不做全局当前赛季状态。
- 不做“同步全部赛季”入口。
- 指挥台只负责登录、单季同步和本地赛季健康状态，不驱动其他页面的筛选状态。
- 每个功能页自己持有赛季筛选或默认赛季选择。
- 同一个 SQLite 数据库继续承载多赛季数据。
- 所有新增或改动的用户可见前端文本必须为中文。
- 不暴露 token、cookie、`customerToken`、本地凭据文件、浏览器 profile 或 raw 玩家隐私 ID。

## 目标

核心目标：

- 多赛季同时存在时，每个页面都能明确展示当前读取的是哪个赛季。
- 页面查询、聚合、排行、筛选项和空状态不会把 S21 / S22 / S23 混在一起。
- 旧的 S22 默认值改为“URL 指定赛季优先，其次使用本地最新可用赛季或后端返回的 selected season”。
- 后端新增或补齐多赛季隔离测试，覆盖至少两个赛季同库共存。
- 前端补齐必要的页面级赛季选择和构建回归。

## 非目标

- 不重写网易大神登录流程。
- 不重写赛季同步协议。
- 不新增动态赛季发现。
- 不新增跨页面共享的当前赛季 store。
- 不把 `dump_season_json` 接入 UI。
- 不在本阶段重做页面视觉风格。
- 不处理第七阶段角斗资产校对尾巴，除非它直接阻塞多赛季隔离测试。

## 现状观察

需要优先处理的已知旧假设：

- `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx` 仍把缺省赛季写为 S22。
- `apps/owstats-desktop/src/pages/RankingPage.tsx` 当前默认调用 `getRankings()`，依赖后端选择最新本地赛季，页面缺少显式赛季选择。
- `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx` 的赛季战报和赛季分析需要使用同一个选中赛季，避免战报是 S22、分析却是后端最新赛季。
- `apps/owstats-desktop/src-tauri/src/ranking.rs`、`review.rs`、`analytics.rs` 已经存在 `season: Option<i64>` 过滤入口，但需要用同库双赛季测试证明所有聚合路径都隔离。
- 对局页已有 URL / filter season 入口，下一阶段重点是筛选项、详情跳转、缓存和空状态是否跟当前页面赛季一致。

## 页面适配策略

### 对局页

对局页继续作为唯一筛选入口，保留页面内 filter state 和 URL query。

要求：
- URL 中存在 `season` 时，按 URL season 查询。
- URL 中没有 `season` 时，可以展示全部本地赛季的对局，但筛选栏必须清楚提供 S21 / S22 / S23 选项。
- 选择某个赛季后，对局列表、统计摘要、分页或缓存数据必须只来自该赛季。
- 如果当前 URL season 不在后端返回的可用赛季中，移除该 season 参数并显示稳定空状态或回到未筛选状态。
- 从对局列表进入详情时，详情页要能防止跨赛季同 `match_id` 误命中；如果现有数据能保证 `match_id` 全局唯一，可以用测试锁定这个假设，否则详情入口和后端查询要携带 season。

### 个人战报页

个人战报页需要摆脱 S22 写死默认值。

要求：
- URL `season` 优先。
- URL 无 season 时，由后端返回的 selected season 或本地最新可用赛季作为默认展示。
- 页面内提供赛季选择控件，选项来自本地可用赛季或固定 S21 / S22 / S23 状态。
- `getPersonalSeasonReview(season)` 和 `getSeasonAnalysis(season)` 必须使用同一个 season。
- 空库或当前赛季无数据时显示中文空状态，不展示旧 S22 回退内容。
- 页面标题使用实际 selected season，例如 `S23 赛季表现报告`。

### 排行页

排行页当前适合默认展示最新本地赛季，但不适合多赛季对比使用。

要求：
- 新增页面内赛季选择。
- URL `season` 优先；无 URL season 时使用后端 selected season。
- 调用 `getRankings(season)`，不再只依赖 `getRankings()`。
- 地图排行、英雄排行、最佳英雄推断和样本数全部按 selected season 聚合。
- 当前赛季无排行数据时显示中文空状态，不回落到其他赛季。

### 好友分析页

好友分析页必须明确分析范围。

要求：
- 新增或复用页面内赛季选择。
- 调用 `getSeasonAnalysis(season)`。
- 好友亲密度、常见队友、模型面板和趋势摘要全部使用同一个 selected season。
- 当前赛季样本不足时显示中文空状态或样本不足提示，不混入其他赛季补样本。
- 保持好友分析路径和模型看板路径边界，不因为赛季适配重新改动亲密度排序规则。

### 指挥台

指挥台不参与页面筛选状态。

要求：
- 继续只展示 S21 / S22 / S23 三张卡片。
- 继续只允许单季同步。
- 同步完成后刷新三季状态。
- 不新增“设为当前赛季”“同步全部赛季”或跨页面跳转时自动带入全局赛季。

## 后端适配策略

### 查询隔离

所有可被页面调用的查询路径都要确认以下规则：

- 当 request 中包含 season 时，SQL 必须包含明确的 season 条件。
- 当 request 中不包含 season 时，后端可以选择最新本地赛季作为默认，但响应 DTO 必须返回实际 selected season。
- 聚合类查询不能在内部某一步丢失 season 条件，例如先筛选 S23 match rows，后续 join 到 `match_players` 或 `match_evaluations` 时又跨赛季聚合。
- 筛选项生成可以返回所有本地可用赛季；其他维度是否随当前 season 收窄，按页面需要决定，但文案必须清楚。

重点文件：
- `apps/owstats-desktop/src-tauri/src/queries.rs`
- `apps/owstats-desktop/src-tauri/src/review.rs`
- `apps/owstats-desktop/src-tauri/src/ranking.rs`
- `apps/owstats-desktop/src-tauri/src/analytics.rs`
- `apps/owstats-desktop/src-tauri/src/models.rs`

### 详情定位

需要确认 `match_id` 在本地数据库中是否跨赛季唯一。

验收方式：
- 如果认为唯一，新增测试构造不同赛季不同 match_id，证明详情查询不会误读。
- 如果不能保证唯一，详情 DTO、前端路由或后端命令要携带 season，并用 `(season, match_id)` 定位详情。

### 导入和同步幂等

同步层继续沿用现有共享 fetch/import 路径。

要求：
- 同步 S21 不删除 S22 / S23。
- 重复同步 S22 不让 `matches`、`match_players`、详情投影或 `sync_errors` 无限制膨胀。
- `rebuild_match_players_for_season` 这类重建逻辑只能清理当前 season 的投影。
- 本阶段不读取真实凭据文件做人工审计；测试使用构造数据或脱敏 fixture。

## 数据和 DTO 约定

前后端 DTO 需要保持一致：

- `OnboardingStatus.availableSeasons` 继续表示本地已有对局数据的赛季。
- `OnboardingStatus.seasonStatuses` 继续表示固定 S21 / S22 / S23 三张指挥台卡片。
- 页面级响应必须返回实际 `season` 或 `selectedSeason`，让前端标题和空状态有依据。
- TypeScript 字段名和 Rust DTO 字段名保持现有 camelCase / snake_case 映射，不为了中文显示改内部字段。

## 测试策略

### 后端测试

新增或扩展测试，构造至少两个赛季同库数据。

必须覆盖：
- S21 对局列表不返回 S22 对局。
- S22 对局列表不返回 S21 对局。
- 战报按 selected season 聚合。
- 排行按 selected season 聚合。
- 好友分析按 selected season 聚合。
- 重复导入同一赛季不会导致对局或玩家投影重复膨胀。
- 指挥台三季状态不会把一个赛季的数据复制到另一个赛季。

建议命令：

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

### 前端测试

必须覆盖：
- 战报页不再默认写死 S22。
- 排行页选择 S21 / S22 / S23 时调用对应 season。
- 好友分析页分析请求和页面标题使用同一 selected season。
- 对局页 URL season 与筛选栏状态一致。
- 所有新增可见文本为中文。

建议命令：

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

### 视觉检查

如果本阶段改动页面筛选控件，需要补一轮截图检查：
- 战报页桌面宽度。
- 排行页桌面宽度。
- 好友分析页桌面宽度。
- 窄宽度下赛季选择控件不挤压标题或指标卡。

## 验收标准

本阶段完成时必须满足：

- 空库时，各页面给出中文空状态，不默认伪造 S22。
- 只有 S22 数据时，S21 / S23 页面查询不显示 S22 数据。
- S21 与 S22 同时存在时，选择 S21 不返回 S22 数据，选择 S22 不返回 S21 数据。
- 战报、排行、好友分析页面标题显示实际 selected season。
- 页面内赛季选择不影响其他页面。
- 指挥台仍不提供同步全部、全局当前赛季或诊断 dump UI。
- 重复同步或重复导入同一赛季不会造成明显重复膨胀。
- `cargo fmt --check`、`cargo test --tests`、`npm test`、`npm run build` 通过，或在交付报告中明确说明未通过原因和剩余风险。

## 推荐执行切片

### 切片 1：后端隔离测试和缺口审计

先构造双赛季测试数据，锁定当前查询是否混赛季。不要先改 UI。

输出：
- 一份测试结果。
- 明确列出需要改动的后端查询路径。
- 不读取真实凭据和本地浏览器 profile。

### 切片 2：个人战报页赛季适配

处理最明显的 S22 写死默认值，并保证战报和赛季分析使用同一个 season。

输出：
- 战报页 URL season、默认 season、空状态和标题一致。
- 对应后端/前端测试通过。

### 切片 3：排行页赛季适配

给排行页补页面内赛季选择，并传入 `getRankings(season)`。

输出：
- 排行页不再只能看最新本地赛季。
- 地图和英雄排行按 selected season 聚合。

### 切片 4：好友分析页赛季适配

给好友分析页补 selected season，并保持亲密度和模型看板边界。

输出：
- 好友分析页所有请求和展示使用同一 season。
- 样本不足和空状态为中文。

### 切片 5：对局页和详情页回归

验证对局筛选、缓存、详情跳转和详情查询是否需要携带 season。

输出：
- 对局页 URL/filter/cache 行为稳定。
- 详情页不存在跨赛季误命中风险，或已改成 `(season, match_id)` 定位。

## 交付报告要求

执行线程完成任一切片后，报告必须包含：

- 修改的文件列表。
- 新增或更新的测试名。
- 执行过的验证命令和结果。
- 是否读取过任何真实凭据或本地隐私数据；默认答案应为“没有”。
- 是否存在未解决的跨赛季混读风险。
- 是否改动了指挥台产品边界；默认答案应为“没有”。
