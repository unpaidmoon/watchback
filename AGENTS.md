# OWStatsLocal 协作说明

## 项目边界

- 工作根目录：`D:\Codex\OW_data\owstats-local`。
- 主产品：`apps/owstats-desktop`，技术栈为 Tauri 2、React、TypeScript、Rust、SQLite。
- 开始工作前先执行 `git status --short`；工作树经常包含用户正在进行的改动，除非明确要求，不要清理、覆盖、暂存或提交它们。
- 优先用 `rg` 定位代码。Tauri 问题从 `src-tauri/src/commands.rs` 进入，UI 问题从对应页面或组件进入。

## Codebase Memory MCP

- 进行架构分析、跨文件调用链追踪、变更影响分析或模块边界调查时，agent 应自主优先使用 `codebase-memory-mcp`，无需等待用户逐次指定。
- 开始图谱查询前先运行 `index_status`；索引不存在或明显过期时，只允许索引当前项目根目录。
- 架构调查优先使用 `get_architecture`、`search_graph`、`trace_path` 和 `detect_changes`；关键结论仍需用 `rg` 与源码读取复核。
- 简单的精确文本查找继续优先使用 `rg`，不必为每个小修改调用代码图谱。
- 不得索引项目根目录之外的路径，不得提交 `.codebase-memory/` 图谱产物，不得读取或输出凭据及真实隐私数据。

## 文档与产品准则

按以下顺序读取与任务相关的材料：

1. 本文件与 `README.md`。
2. 涉及新页面、组件视觉调整或页面重构时，必须先完整阅读根目录 `design.md`；它是当前视觉规范，若与较早视觉说明冲突，以它为准。
3. `docs/maintenance.md`、`docs/windows-release.md`。
4. `docs/superpowers/specs/` 中最新且与任务直接相关的设计，以及 `main-thread-log.md`。

- 用户可见文本必须使用中文；内部 API、字段名和上游协议字段保持原名。
- 新页面和视觉重构必须遵循 `design.md` 的颜色、字体、间距、表面层级、可访问性和视觉验收规则；不得自行引入新的品牌色、字体、圆角、阴影或卡片语言。
- 当前赛季产品边界是固定 S21/S22/S23，页面各自维护赛季筛选；不要新增全局赛季状态或“一键同步全部”。
- 同步产品方向以 `docs/superpowers/specs/2026-07-03-phase-8-login-account-season-sync-design.md` 为准：仅同步当前浏览器登录账号的数据，BattleTag 绑定与 `searchBnetAccount` 属于旧流程，除非用户明确要求，不要恢复或扩展它。
- `data/players/5479_s22_full_season.json` 是原始匿名完整赛季 fixture；不得把真实用户数据、凭据或浏览器 profile 加入提示词、提交或文档。

## 隐私与数据安全

绝不打印、提交或暴露：

- `OW_DASHEN_TOKEN`、`token`、`customerToken`、cookie、authorization、credential。
- `data/.dashen_credentials.json`、浏览器 profile、原始玩家隐私 ID、真实数据 dump。

- UI、错误消息、日志、测试输出、诊断 JSON 和会返回给前端的 raw JSON 都必须递归脱敏 token-like 字段。
- 任何新增的持久化 payload 或 SQLite raw JSON 都要先确认不会保留敏感字段；仅 Git 忽略不足以构成安全边界。
- 显示名、英雄、地图等 fallback 不得直接向发布 UI 泄露原始 GUID 或隐私 ID；使用中文稳定兜底。角斗详情的 `traitGuids` 与 `modGuids` 是资产校对例外：无法映射时必须向发布 UI 显示原始 GUID，不能显示“待补充”文本；此例外不得扩展到玩家、英雄、地图、凭据或其他隐私标识。

## 资源与安装版要求

- 不得新增 `D:\Codex\...`、`/@fs/` 或其他开发机绝对路径依赖。
- 构建或运行时必需的 JSON、图片、字典必须被跟踪并由 Vite/Tauri 正式打包，或通过 Tauri 的资源目录与 app data 目录解析；不得依赖被 `.gitignore` 忽略的本机文件。
- 同步、导入与设置必须使用 Tauri `app_data_dir`/资源目录，不得用 `CARGO_MANIFEST_DIR` 推导安装版数据路径。
- 涉及发布、资源或路径的改动，至少验证干净 checkout 构建和安装后烟测；仅本机 `tauri build` 成功不能证明异机可用。

## 角斗映射协作

- 人工编辑主表：`docs/superpowers/specs/2026-07-13-stadium-asset-mapping.xlsx`。中立单独 sheet、每位英雄各自一个 sheet，固定五列：GUID、装备/异能、名称、描述、资产路径。
- 主表中的 GUID 使用十进制文本，必须存在；名称、描述、资产路径可以留空。运行时 manifest 与文件名继续使用十六进制 GUID。
- 用户编辑主表后，以“同步角斗映射表”为指令：先校验十进制 GUID、sheet 归属、资产路径和重复项，再将名称、描述与已验证的资产路径同步到 `assets/stadium/manifest.json`，随后运行 `npm run prepare:assets` 和资源测试。
- 对于新增或替换图标，用户可以先填可访问的本地源文件路径；同步时将其复制/转换到正式 `assets/stadium/` 路径，并把主表回写为该正式路径。不得把开发机绝对路径写入 manifest 或运行时 UI。

## 数据库与同步

- SQLite schema 改动必须有前向兼容、幂等迁移和升级库测试；新投影或索引要考虑历史数据回填及逐赛季行为。
- 多赛季查询、缓存、详情定位和聚合都必须带上或明确选择 season，避免混读。
- 重复同步同一赛季必须保持对局、详情、投影和错误记录的可控幂等性。
- 使用真实上游接口时保留限速、429 重试和脱敏错误；不要在人工审查中读取本机真实凭据。

## 验证与交付

后端：

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

前端：

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

发布验证：

```powershell
cd apps\owstats-desktop
npm run tauri build
```

- 有额外 `src/bin` 后，使用 `cargo run --bin <name>`；默认桌面程序由 `default-run = "owstats-desktop"` 指定。
- 交付前报告实际执行的命令、结果、未验证项和是否接触过真实隐私数据。
- 修改后运行 `git diff --check`，并确认 `git status --short` 中只包含预期文件。
