# OWStatsLocal 当前开发状态

> 更新日期：2026-07-11
> 稳定化阶段：STAB-007 已实现，等待以提交后的 tracked-only archive 进行最终验收。资源、运行路径、同步身份、隐私持久化、数据库恢复、核心模块职责边界与统一验证入口均已收口。

## 产品与架构

- 主产品位于 `apps/owstats-desktop`，采用 Tauri 2、React、TypeScript、Rust 和 SQLite；前端通过 Tauri command 调用 Rust 查询、同步与持久化能力。
- 产品固定支持 S21、S22、S23。对局、个人战报、排行和好友分析页面各自维护赛季选择与 URL 状态，不存在全局赛季状态或“一键同步全部赛季”。
- STAB-002 任务 1–13 已按小提交重建，提交范围为 `d8d8054..88a9dbb`；STAB-003 的资源与路径提交范围为 `0533f41..b8a10bd`。
- 角斗玩家索引使用 `arena_match_players` 投影；schema v3 以前向、幂等迁移建立表与索引。导入按赛季写入投影，查询前用 `arena_player_index_v1:<season>` 完成 marker 判断缺失回填，空投影赛季也可被标记为完成，避免重复重建。
- 任务 11–13 已实现并通过本轮测试：角斗玩家按名称或 bnet 标识搜索但只返回显示名、按玩家筛选角斗对局、逐赛季 marker 回填；`Shion` 中文别名改为“死怨”；五张已确认 S23 预览图及映射记录已保留。任务 14 完成文档收尾、36 路径对账和最终验证。

## 资源与隐私边界

- 正式资源由 tracked `assets-inventory.json` 白名单和三份 manifest 校验后确定性准备；源 symlink/junction、路径越界、未跟踪文件和敏感 runtime config 会使准备失败。
- 前端统一使用 `/assets/...`，Vite 不再放行开发机绝对目录；构建产物扫描禁止 `/@fs/`、开发机盘符路径和 `docs/reference`。
- `ow-lookups.json` 是前端和 Rust importer 共用的最小正式字典；桌面运行不再读取 ignored `docs/reference` 或仓库 `data/players`。
- `AppPaths` 统一派生数据库、设置、资料、凭据、旧 target、浏览器 profile、seasons、diagnostics 和 backups；桌面同步 JSON 直接写入 `app_data/seasons`。
- 同步只使用当前浏览器登录账号的凭据身份；产品入口不再提供 BattleTag 目标绑定或 `searchBnetAccount`。`ready` 表示已识别当前账号，`currentPlayerMissing` 表示需重新登录，不能解锁同步。
- 新写入的赛季 JSON、SQLite raw JSON、同步错误和前端可见错误都经过统一递归脱敏。首次打开旧数据库会先备份 SQLite/赛季 JSON 至 `privacy-backups`，再对旧数据进行幂等清洗；不读取或操作旧 target 文件、真实凭据、浏览器 profile、真实数据库或玩家 dump。
- SQLite 使用显式 v1-v4 迁移注册表；旧库升级前生成一致性副本至 `app_data/backups`，新库不生成冗余备份。所有连接统一配置 WAL、NORMAL 和 5 秒锁等待，且高于程序版本的数据库会被拒绝打开。
- 同步错误按赛季保存为最新快照，重复同步不增长；桌面端只允许一个赛季同步在运行，中断导入会回滚，移除故障后可以重试。
- 核心 Rust 代码已从单文件拆为 `queries/`、`review/`、`sync/` 的兼容 façade 与内部职责模块；对局详情和指挥台赛季健康卡均为独立 React 组件，URL、DTO、路由与用户可见中文保持原契约。
- 当前仍刻意保留 `analytics/mod.rs` 的 repository/metrics 共用行模型，以及 `styles/shared.css` 的既有声明；这两项有明确模块入口和回归覆盖，但在补齐视觉快照/组件渲染基础设施前不做机械细切，避免纯重构引入不可见 UI 漂移。

## 快照 36 路径对账

以下清单逐路径覆盖快照 `648531e` 的 36 个路径，无未知项。

### 重建（22）

- `AGENTS.md`
- `apps/owstats-desktop/package.json`
- `apps/owstats-desktop/scripts/phase6Assets.test.ts`
- `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- `apps/owstats-desktop/src-tauri/src/db.rs`（只重建 schema v3；字典片段延期）
- `apps/owstats-desktop/src-tauri/src/importer.rs`（只重建投影；字典片段延期）
- `apps/owstats-desktop/src-tauri/src/queries.rs`（只重建安全查询与回填；字典片段延期）
- `apps/owstats-desktop/src-tauri/src/review.rs`（只重建职责模式修正；字典片段延期）
- `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`
- `apps/owstats-desktop/src-tauri/tests/phase3_review.rs`
- `apps/owstats-desktop/src-tauri/tests/phase8_multi_season_pages.rs`
- `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx`
- `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- `apps/owstats-desktop/src/pages/RankingPage.tsx`
- `apps/owstats-desktop/src/styles.css`
- `assets/manifest.json`
- `assets/maps/esperanca2.png`
- `assets/maps/ilios2.png`
- `assets/maps/nepal2.png`
- `assets/maps/oasis-university.png`
- `assets/maps/samoa2.png`
- `docs/superpowers/specs/main-thread-log.md`

### 重写（1）

- `docs/current-development-status.md`：依据当前事实重写，不恢复旧分支、旧脏工作树或安装版结论。

### 延期（4）

- `apps/owstats-desktop/src-tauri/src/analytics.rs`
- `apps/owstats-desktop/src-tauri/src/ranking.rs`
- `apps/owstats-desktop/src-tauri/tests/phase4_ranking_analysis.rs`
- `apps/owstats-desktop/src/shared/assets.ts`

上述路径的字典或资源解析职责延期到 STAB-003；`db.rs`、`importer.rs`、`queries.rs`、`review.rs` 中同类快照片段也未恢复。

### 排除（9）

- `assets/maps/拉克瓦广场.png`
- `assets/maps/泰晤士区.png`
- `assets/maps/火药桶矿井.png`
- `assets/maps/红杉水坝.png`
- `assets/maps/维多利亚竞技场.png`
- `assets/maps/花冈.png`
- `assets/maps/釜山-寺院.png`
- `docs/superpowers/specs/2026-07-09-s23-arena-map-preview-gaps.csv`
- `test-results/.last-run.json`

前七张图片没有已接受的运行时映射；CSV 含开发机路径与被表格舍入的 GUID；最后一项为生成物。它们均未重引入正式成果。

## 本轮实际验证

- `cargo fmt --check`：通过。
- `cargo test --tests`：通过；真实上游凭据测试 `live_rust_sync_imports_a_complete_season` 按设计忽略。
- `npm test`：通过。
- `npm run build`：通过；Vite 报告既有大 chunk 警告，不影响退出码。
- `git archive HEAD` tracked-only 临时副本中：`npm ci --offline`、`npm test`、`npm run build`、`cargo fmt --check`、`cargo test --tests` 通过。
- tracked-only 副本的 `npm run tauri:dev -- --no-watch` 完成编译并启动 `target\\debug\\owstats-desktop.exe`，随后正常终止烟测进程树。
- 本轮未执行 `npm run tauri build`、安装包构建或真实上游同步；不声称安装版已验证。
