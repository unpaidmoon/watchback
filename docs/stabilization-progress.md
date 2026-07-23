# Watchback 稳定化进度

> 总设计：`docs/superpowers/specs/2026-07-10-personal-stable-sustainable-development-design.md`
>
> 安全快照：`codex/stabilization-snapshot`
>
> 当前运行时数据边界以 `docs/superpowers/specs/2026-07-18-remove-runtime-redaction-design.md` 为准。STAB-004 中的递归脱敏与历史隐私重写是保留的历史验证记录，已不再是当前政策；发布资源、共享配置和账号/event schema 的凭据防线继续有效。

| 编号 | 问题 | 优先级 | 状态 | 验证证据 | 提交 |
| --- | --- | ---: | --- | --- | --- |
| STAB-001 | 安全快照与恢复验证 | P0 | 已验证 | Git bundle 恢复哈希一致 | `codex/stabilization-snapshot` |
| STAB-002 | 拆分当前未提交成果 | P0 | 已验证 | Rust 全量测试、前端测试与构建通过；36 路径对账完成 | `d8d8054..88a9dbb`；`docs: complete STAB-002 decomposition` |
| STAB-003 | 可复现资源与运行路径 | P0 | 已验证 | tracked inventory 两次准备哈希一致；主目录和 `git archive HEAD` 副本的前端测试/构建、Rust 全量测试通过；副本 `tauri:dev` 启动到 `owstats-desktop.exe` | `0533f41..b8a10bd`；`docs: complete STAB-003 portability` |
| STAB-004 | 同步身份与敏感数据治理 | P0 | 已验证 | 当时验证了 JSON、错误和 SQLite 的递归脱敏及历史 SQLite/赛季 JSON 清洗；这些运行时规则现已由 2026-07-18 原始数据直通设计废止。当前仍保留仅同步当前登录账号、发布资源/共享配置检查和账号/event schema 字段最小化防线。 | `00a4590..db5ba0b`，`docs: complete STAB-004 privacy stabilization` |
| STAB-005 | 数据库迁移与运行稳定性 | P0 | 已验证 | v1-v4 显式事务迁移、未来版本拒绝、升级前一致性备份、WAL/5 秒锁等待、赛季错误快照、桌面同步互斥和中断恢复均由临时 SQLite 测试覆盖；后端全量测试通过 | `279477a..273fd41`，`docs: complete STAB-005 runtime stability` |
| STAB-006 | 渐进式模块化 | P1 | 已验证 | queries/review/sync 改为稳定 facade 加内部职责模块；好友榜单、对局详情与赛季健康卡独立；纯函数/DTO/既有功能回归、后端全量测试、前端测试与构建通过。CSS 仅完成聚合入口，好友 repository/metrics 与指挥台数据 hook 的进一步细分已记录为受控后续项 | `b87ebc7..514455b`，`docs: complete STAB-006 modularization` |
| STAB-007 | 统一验证脚本与 CI | P1 | 已验证 | 发布面敏感信息/绝对路径扫描、分层前端命令、根验证入口、tracked-only archive 验收和 Windows CI 已实现；完整入口在 `8189d30` 上通过，未读取真实隐私数据、未执行安装版构建 | `0423559`，`8189d30`，`docs: complete STAB-007 verification gates` |

状态只使用：`待开始`、`进行中`、`已验证`、`阻塞`。任意时刻只允许一个任务处于 `进行中`。
