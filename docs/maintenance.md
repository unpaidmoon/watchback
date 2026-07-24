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
- Tauri 桌面运行路径统一由 `SharedAppPaths` 从当前可执行文件同级的 `data` 派生。数据库、设置、凭据、浏览器 profile、赛季 JSON、诊断和备份都必须位于该数据根，且不得从仓库根推导。
- 旧 `%APPDATA%\local.owstats.desktop` 只允许由 NSIS 提升安装流程的专用迁移入口作为只读迁移源；正常桌面运行不得读写 AppData，也不得静默回退。
- NSIS 通过受信任 helper 在安装阶段完成数据根创建、迁移、边界校验与权限设置。旧根、迁移源或目标存在重解析点、类型错误、路径冲突或校验失败时必须 fail closed 并停止安装；不得尝试读取内容后再回退或覆盖已发布的 `data`。
- `data` 使用仅作用于该精确目录的受保护 DACL，供当前交互用户写入；安装根和程序文件继续保持不可由普通用户修改。安装后还必须以原始交互用户令牌验证该目录可写。
- `CARGO_MANIFEST_DIR` 只允许显式开发 CLI 和测试使用；桌面 commands 不得依赖仓库布局。
- `npm run build` 的 postbuild 会扫描 `dist`，发现 `/@fs/`、开发机盘符路径或 `docs/reference` 时失败。

更新资源或路径代码后，除常规检查外还应从 `git archive HEAD` 创建 tracked-only 临时副本，执行前端测试、构建、Rust 测试和 `npm run tauri:dev -- --no-watch` 启动烟测。不要把真实 app-data、凭据、浏览器 profile 或玩家数据复制到临时副本。

## Commit Discipline

- Do not mix unrelated changes in one commit. Keep product behavior, release configuration, database migrations, text repair, and test-only changes separate when they can be reviewed or rolled back independently.
- Do not commit credentials, tokens, cookies, browser profiles, raw player dumps, local SQLite data, dependency folders, or generated build output.
- Check `git status --short` before every commit and before release. Only intentional source and documentation changes should be listed.
- If a change modifies the SQLite schema, add or update migration tests. At minimum, cover initializing the database twice so migrations stay idempotent.

## 原始运行时数据与历史记录

- 上游自然出现的字段和错误文本在抓取、导入、SQLite、赛季快照、查询 raw JSON 与既有错误面中原样通过；不得重新加入运行时递归脱敏、遮盖、清理、空操作或功能开关。
- 这是当前维护边界，并覆盖旧规格、旧计划与旧测试中的运行时脱敏要求；不得因为历史文档仍出现 `redact`、`sanitize`、`mask` 或“递归脱敏”等描述而恢复相关处理。
- 不主动把凭据文件中的 token、customerToken、cookie、authorization 或 credential 复制到原本不包含它们的 DTO、账号索引、登录进度或后台事件、日志及其他 payload；账号和事件的现有字段最小化约束保持不变。
- 数据库初始化不再重写历史 raw JSON 或赛季文件；既有 privacy backup、marker 与已存在数据仅为兼容而保留，不主动删除、重抓或恢复历史已脱敏值。
- 赛季快照重建只组装一次并直接原子序列化；临时文件、`sync_all`、revision 稳定性检查和原子替换语义保持不变。
- 快照性能基准只可对复制后的本地 SQLite 与临时输出运行，输出仅含 season、entries、details、errors、阶段耗时和输出字节数；不得打印 ID、路径、数据库行、payload 或凭据，临时副本和输出由操作者清理。

## 多账号运行时数据

- 根目录的 `accounts.json` 只保存安全账号索引和当前账号指针；不得在其中加入角色私密标识、登录凭据或任何 token-like 字段。
- `accounts/<account_id>/` 保存单个账号的凭据、玩家目标、玩家资料、SQLite 数据库、赛季文件、诊断和备份。切换账号时，这些账号所属路径必须作为一个完整快照同时切换，不能混用其他账号文件。
- 用户设置和浏览器登录 profile 是共享运行资源，保留在安装目录 `data` 根目录。它们不随账号切换，也不能被“退出账号”误删。
- 首次升级旧版单账号数据时，后端先复制到临时目录并校验内容，再发布到已确认账号目录，最后清理旧文件。迁移 journal 用于断电恢复；不要手工移动、修改或删除未完成 journal。
- “退出账号”会永久删除该账号目录中的凭据与本地数据库等全部账号数据。删除失败时保留恢复 journal，重启应用后应先完成恢复；恢复完成前不得继续添加、切换、重新登录或删除账号。
- 凭据过期只影响需要上游登录的操作，不应阻止用户读取该账号已保存的本地数据；使用“重新登录”刷新当前账号凭据。
- 安全排障只能记录账号数量、当前账号是否存在、凭据状态和错误分类。不得复制账号列表、BattleTag、真实目录内容、角色私密标识、凭据、浏览器 profile、数据库内容或错误正文到日志和报告。
- 索引损坏、活动账号目录缺失或恢复失败时应停止账号域命令并显示中文安全错误。不要通过编辑 `accounts.json` 或拼接 `account_id` 路径绕过校验。

## Database Migrations and Sync Stability

- 数据库版本由 `src-tauri/src/database/migrations.rs` 的注册表管理。每一步必须在独立事务中完成变更、更新 `user_version` 并提交；不得修改已经发布的迁移版本。
- 已存在且需要升级的 SQLite 文件会在 schema 变更前使用 SQLite 一致性备份写入当前账号的 `backups/`。空库不生成备份；备份失败会阻止迁移，源库保持原样。
- 所有文件数据库连接使用 WAL、`synchronous=NORMAL` 和 5 秒 `busy_timeout`。不要绕过 `OwDatabase::connect` 直接打开产品数据库。
- `sync_errors` 表示某赛季最近一次同步未完成详情的快照，而不是无限历史。导入目标赛季时先清旧错误，成功重试会清零该赛季错误；其他赛季必须保持不变。
- 桌面同步由全局 `SyncCoordinator` 串行化。不要在 command、后台任务或新 UI 入口绕过该 guard 并行写同一个 SQLite/安装目录数据目录。
- 赛季行的“更新”是列表优先路径：前台严格并发请求 `sport`、`leisure`、`QuickFight`、`LeisureFight`、`SportFight` 五个模式的第一页，并在一个短事务内提交新列表、游标和后台任务后立即返回。前台不得请求详情、继续分页、重建快照或全量刷新指挥台。
- `season_sync_jobs` 是可恢复后台账本。后台按账号和赛季去重，继续第 2～5 页、以并发 2 补详情并更新投影，最后在 `season_revision` 稳定时重建快照；详情失败必须保留 `pending_detail_count` 并进入 `retryable`，不能伪装成完成。
- 无新增的“更新”只写检查时间和可靠游标，不清比赛缓存、不重建快照。新增列表只失效当前账号和赛季的列表缓存；SQLite 始终是查询事实来源，快照落后不得阻止用户查询。
- “全量抓取”继续扫描完整历史并刷新全部详情。开始前拒绝同赛季活动后台任务；成功后对齐 `season_revision`/`snapshot_revision`，并把遗留快速任务标记为 `superseded`。
- 后台事件与状态 DTO 只允许账号内部作用域、赛季、阶段、匿名计数、时间和错误分类；不得加入请求 URL、match/player id、raw JSON 或任何 token-like 字段。

## Module Boundaries

- Rust 领域模块以 `mod.rs` 保持兼容 façade：`queries/` 负责列表、普通详情、竞技场详情与 lookup；`review/` 分为个人战报、角色段位和指挥台状态；`sync/` 分为 façade、client、fetch、payload 和 persist。新增调用必须继续经由 `lib.rs` 的既有公开入口。
- 好友分析已经将榜单 DTO 生成放入 `analytics/boards.rs`；repository/filter SQL 与聚合指标暂在 `analytics/mod.rs`，因为它们共享行模型与统计辅助函数。后续拆分时不得改变样本阈值、排序或中文 fallback。
- 前端 `MatchesPage` 仅编排 URL、缓存和请求状态；紧凑详情位于 `features/matches/CompactMatchDetail.tsx`。指挥台的赛季健康卡、数据事实和时间格式化位于 `features/command-center/SeasonHealthPanel.tsx`。
- `styles.css` 仅是稳定的样式聚合入口。现有规则先保留在 `styles/shared.css`，避免未经视觉回归的声明级迁移；新的 feature 样式应写入对应 `styles/features/` 文件。

## Rollback

If a release fails after installation, identify the smallest commit that introduced the failure and revert that commit first.

For code or UI regressions, prefer reverting the focused commit instead of mixing rollback with new fixes. For database migrations, preserve existing local data: migrations should be forward-compatible, idempotent, and safe to run on databases created by older releases. If a schema change cannot be safely rolled back, ship a new forward migration that restores application behavior without deleting user data.
