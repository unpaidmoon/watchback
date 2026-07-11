# OWStatsLocal 当前开发状态

> 更新日期：2026-07-11
> 稳定化阶段：STAB-002 已验证。任务 11–14 已按独立提交纳入 `codex/stabilization`。

## 产品与架构

- 主产品位于 `apps/owstats-desktop`，采用 Tauri 2、React、TypeScript、Rust 和 SQLite；前端通过 Tauri command 调用 Rust 查询、同步与持久化能力。
- 产品固定支持 S21、S22、S23。对局、个人战报、排行和好友分析页面各自维护赛季选择与 URL 状态，不存在全局赛季状态或“一键同步全部赛季”。
- STAB-002 任务 1–13 已按小提交重建，当前已提交范围为 `d8d8054..88a9dbb`；其中包含协作约束、生成物治理、竞技模式修正、多赛季查询与页面隔离，以及角斗玩家索引 schema v3、投影、回填查询和经确认素材。
- 角斗玩家索引使用 `arena_match_players` 投影；schema v3 以前向、幂等迁移建立表与索引。导入按赛季写入投影，查询前用 `arena_player_index_v1:<season>` 完成 marker 判断缺失回填，空投影赛季也可被标记为完成，避免重复重建。
- 任务 11–13 已实现并通过本轮测试：角斗玩家按名称或 bnet 标识搜索但只返回显示名、按玩家筛选角斗对局、逐赛季 marker 回填；`Shion` 中文别名改为“死怨”；五张已确认 S23 预览图及映射记录已保留。任务 14 完成文档收尾、36 路径对账和最终验证。

## 资源与隐私边界

- `esperanca2.png`、`ilios2.png`、`nepal2.png`、`oasis-university.png`、`samoa2.png` 仅作为经复审的候选预览素材保留，尚未接入运行时资源解析。
- 字典提取、正式资源打包、仓库相对资源 URL 和运行路径治理延期到 STAB-003；本阶段未恢复 `docs/reference` 输入、`D:\Codex\...` 或 `/@fs/` 运行依赖。
- 登录身份、凭据结构、持久化 raw JSON 的统一递归脱敏和存量数据迁移延期到 STAB-004；本轮未读取真实凭据、浏览器 profile、真实数据库或玩家 dump。

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
- 本轮未执行 `npm run tauri build`、安装包构建、干净 checkout 构建、原生 Tauri WebView 烟测或真实上游同步，因此不声称安装版或异机可用性已验证。
