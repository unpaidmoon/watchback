# OWStatsLocal 稳定化进度

> 总设计：`docs/superpowers/specs/2026-07-10-personal-stable-sustainable-development-design.md`
>
> 安全快照：`codex/stabilization-snapshot`

| 编号 | 问题 | 优先级 | 状态 | 验证证据 | 提交 |
| --- | --- | ---: | --- | --- | --- |
| STAB-001 | 安全快照与恢复验证 | P0 | 已验证 | Git bundle 恢复哈希一致 | `codex/stabilization-snapshot` |
| STAB-002 | 拆分当前未提交成果 | P0 | 已验证 | Rust 全量测试、前端测试与构建通过；36 路径对账完成 | `d8d8054..88a9dbb`；`docs: complete STAB-002 decomposition` |
| STAB-003 | 可复现资源与运行路径 | P0 | 已验证 | tracked inventory 两次准备哈希一致；主目录和 `git archive HEAD` 副本的前端测试/构建、Rust 全量测试通过；副本 `tauri:dev` 启动到 `owstats-desktop.exe` | `0533f41..b8a10bd`；`docs: complete STAB-003 portability` |
| STAB-004 | 同步身份与敏感数据治理 | P0 | 已验证 | 递归脱敏覆盖 JSON、错误和 SQLite；同步只使用当前登录账号；旧 BattleTag 入口已移除；历史 SQLite/赛季 JSON 先备份后原子清洗，临时 SQLite/目录、后端全量测试、前端测试和构建通过 | `00a4590..db5ba0b`，`docs: complete STAB-004 privacy stabilization` |
| STAB-005 | 数据库迁移与运行稳定性 | P0 | 进行中 | 已完成审计与实施计划；下一步建立迁移注册、备份恢复与并发同步边界 | — |
| STAB-006 | 渐进式模块化 | P1 | 待开始 | 未执行 | — |
| STAB-007 | 统一验证脚本与 CI | P1 | 待开始 | 未执行 | — |

状态只使用：`待开始`、`进行中`、`已验证`、`阻塞`。任意时刻只允许一个任务处于 `进行中`。
