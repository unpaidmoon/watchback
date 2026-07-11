# Maintenance Guide

## Daily Development

Work in small branches and small commits. Each branch should have one purpose, such as fixing text, changing release configuration, adding a database migration, implementing feature code, or updating tests.

Keep commits focused on one reason to change. If two changes would need different rollback decisions, split them into separate commits.

## Required Checks

首选从仓库根目录运行统一入口：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

该入口依次执行发布面敏感信息与绝对路径扫描、资源准备、分层前端测试和构建、构建产物扫描、Rust 格式/Clippy/全量测试、STAB-005/006 契约测试、`git diff --check`，以及基于 `git archive HEAD` 的 tracked-only checkout 验收。它不会读取本机 app data、凭据或浏览器 profile，也不会执行 `tauri build`。日常只想验证当前工作树时可使用 `-ChangedOnly`；不要把该快捷模式当作发布验收。

GitHub Actions 使用同一个 `scripts/verify.ps1`，不要在 workflow 中复制或另行删减验证命令。

Backend:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Frontend:

```powershell
cd apps\owstats-desktop
npm run test
npm run build
```

Release:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

## Portable Development Inputs

- `apps/owstats-desktop/runtime-config/ow-lookups.json` 是前后端共用的正式最小字典；桌面运行和构建不得读取被忽略的 `docs/reference/`。
- `apps/owstats-desktop/runtime-config/assets-inventory.json` 是资源构建白名单。资源变更后显式运行 `node scripts/prepareAssets.mjs --refresh-inventory` 并审查差异；普通 test/build 不调用 Git。
- `npm test`、`npm run build` 和 `npm run dev` 会先确定性重建 `public/assets`。生成目录不提交，资源 URL 固定为 `/assets/...`。
- Tauri 桌面运行路径统一由 `AppPaths` 从 `app_data_dir` 派生。数据库、设置、凭据、浏览器 profile、赛季 JSON、诊断和备份不得从仓库根推导。
- `CARGO_MANIFEST_DIR` 只允许显式开发 CLI 和测试使用；桌面 commands 不得依赖仓库布局。
- `npm run build` 的 postbuild 会扫描 `dist`，发现 `/@fs/`、开发机盘符路径或 `docs/reference` 时失败。

更新资源或路径代码后，除常规检查外还应从 `git archive HEAD` 创建 tracked-only 临时副本，执行前端测试、构建、Rust 测试和 `npm run tauri:dev -- --no-watch` 启动烟测。不要把真实 app-data、凭据、浏览器 profile 或玩家数据复制到临时副本。

## Commit Discipline

- Do not mix unrelated changes in one commit. Keep product behavior, release configuration, database migrations, text repair, and test-only changes separate when they can be reviewed or rolled back independently.
- Do not commit credentials, tokens, cookies, browser profiles, raw player dumps, local SQLite data, dependency folders, or generated build output.
- Check `git status --short` before every commit and before release. Only intentional source and documentation changes should be listed.
- If a change modifies the SQLite schema, add or update migration tests. At minimum, cover initializing the database twice so migrations stay idempotent.

## Privacy Migration

- 桌面程序初始化数据库时会执行一次 `privacy_sanitization_v1`：仅处理 app data 下的 SQLite raw 字段和 `seasons/` JSON，并先写入 `privacy-backups/<timestamp>/` 备份。
- 迁移会递归删除 token、cookie、authorization、credential 和 storage 类键；失败时不会写完成 marker，可在修复环境后安全重试。
- 不要手动删除 `privacy-backups` 以掩盖问题；它可能是恢复旧数据的唯一副本。凭据、浏览器 profile 和旧 `.dashen_player_target.json` 不属于该迁移的读取或备份范围。

## Database Migrations and Sync Stability

- 数据库版本由 `src-tauri/src/database/migrations.rs` 的注册表管理。每一步必须在独立事务中完成变更、更新 `user_version` 并提交；不得修改已经发布的迁移版本。
- 已存在且需要升级的 SQLite 文件会在 schema 变更前使用 SQLite 一致性备份写入 `app_data/backups/`。空库不生成备份；备份失败会阻止迁移，源库保持原样。
- 所有文件数据库连接使用 WAL、`synchronous=NORMAL` 和 5 秒 `busy_timeout`。不要绕过 `OwDatabase::connect` 直接打开产品数据库。
- `sync_errors` 表示某赛季最近一次同步未完成详情的快照，而不是无限历史。导入目标赛季时先清旧错误，成功重试会清零该赛季错误；其他赛季必须保持不变。
- 桌面同步由全局 `SyncCoordinator` 串行化。不要在 command、后台任务或新 UI 入口绕过该 guard 并行写同一个 SQLite/app-data 目录。

## Module Boundaries

- Rust 领域模块以 `mod.rs` 保持兼容 façade：`queries/` 负责列表、普通详情、竞技场详情与 lookup；`review/` 分为个人战报、角色段位和指挥台状态；`sync/` 分为 façade、client、fetch、payload 和 persist。新增调用必须继续经由 `lib.rs` 的既有公开入口。
- 好友分析已经将榜单 DTO 生成放入 `analytics/boards.rs`；repository/filter SQL 与聚合指标暂在 `analytics/mod.rs`，因为它们共享行模型与统计辅助函数。后续拆分时不得改变样本阈值、排序或中文 fallback。
- 前端 `MatchesPage` 仅编排 URL、缓存和请求状态；紧凑详情位于 `features/matches/CompactMatchDetail.tsx`。指挥台的赛季健康卡、数据事实和时间格式化位于 `features/command-center/SeasonHealthPanel.tsx`。
- `styles.css` 仅是稳定的样式聚合入口。现有规则先保留在 `styles/shared.css`，避免未经视觉回归的声明级迁移；新的 feature 样式应写入对应 `styles/features/` 文件。

## Rollback

If a release fails after installation, identify the smallest commit that introduced the failure and revert that commit first.

For code or UI regressions, prefer reverting the focused commit instead of mixing rollback with new fixes. For database migrations, preserve existing local data: migrations should be forward-compatible, idempotent, and safe to run on databases created by older releases. If a schema change cannot be safely rolled back, ship a new forward migration that restores application behavior without deleting user data.
