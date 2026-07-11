# OWStatsLocal 稳定化工作报告（STAB-003 至 STAB-007）

日期：2026-07-12
分支：`codex/stabilization`
范围：资源与运行路径、同步身份与隐私、数据库稳定性、模块化、统一验证与 CI。

## 结论

STAB-003 至 STAB-007 已按既定审计设计和实施计划完成，采用小提交交付并完成验证。当前工作树干净；没有推送远端、创建稳定标签或构建安装包。

本轮未读取、打印、提交或复制真实账号凭据、cookie、浏览器 profile、app data、真实 SQLite 数据库或玩家原始 dump。

## 阶段成果

| 阶段 | 已完成内容 | 主要提交 |
| --- | --- | --- |
| STAB-003 | 将正式运行所需字典、资源清单与资源准备流程纳入跟踪；统一桌面端 `app_data_dir` 路径；移除开发机绝对路径运行依赖；验证 archive 副本可构建。 | `0533f41..b8a10bd`、`36d0e7c` |
| STAB-004 | 建立递归脱敏边界，覆盖赛季 JSON、SQLite raw JSON、同步错误和前端返回值；同步只使用当前浏览器登录账号；移除旧 BattleTag 绑定入口；为历史持久化数据提供先备份、后幂等清洗。 | `00a4590..7d982b5` |
| STAB-005 | 建立 SQLite v1-v4 显式迁移注册表与未来版本拒绝；升级前一致性备份；连接统一使用 WAL、NORMAL、5 秒锁等待；同步错误按赛季快照化；串行化桌面同步并验证中断恢复。 | `279477a..b295325` |
| STAB-006 | 将 Rust 查询、复盘、同步拆为稳定 facade 和内部职责模块；提取好友榜单、对局紧凑详情、指挥台赛季健康卡；建立模块契约测试和样式聚合入口。 | `b87ebc7..c6c1a8a` |
| STAB-007 | 新增发布面安全扫描、分层前端测试命令、根验证入口、tracked-only archive 验收、Windows CI 及验证证据；补齐严格 Clippy 门禁。 | `0423559`、`8189d30`、`d9a5a4a` |

## 技术改进摘要

### 资源、路径与发布面

- 正式资源由受跟踪 inventory/manifest 与确定性准备脚本管理。
- 前端资源使用 `/assets/...`，运行时字典由打包资源或 Tauri 资源路径提供。
- 桌面端数据库、设置、赛季数据、诊断和备份均从 Tauri app data 目录派生，不依赖仓库位置。
- 验证扫描会检查发布面文本及构建产物中的敏感字段和开发机绝对路径；命中时只输出路径、行号和规则名，不回显内容。

### 隐私与同步

- `token`、`customerToken`、cookie、authorization、credential 等字段在持久化和可见错误链路中递归脱敏。
- 同步模型收敛到当前浏览器登录账号，未恢复旧的 BattleTag 搜索/绑定流程。
- 历史 SQLite/赛季 JSON 清洗先创建 app-data 内备份，再进行原子、可重试的清洗；凭据和浏览器 profile 不在迁移读取范围内。

### 数据库与可靠性

- schema 版本采用可审查的 v1-v4 迁移注册表，每个迁移在独立事务中完成。
- 已存在数据库升级前创建一致性备份；新库不产生冗余备份；未来版本数据库被明确拒绝。
- 同步错误记录语义调整为“目标赛季最近一次失败详情快照”，重复同步不会无限累积。
- 通过全局协调器避免桌面端同一时间多个赛季同步任务并发写入；中断导入可回滚并重试。

### 可维护性

- `queries/`、`review/`、`sync/` 都保留原有公共入口，并按列表/详情、个人/角色/指挥台、transport/fetch/payload/persist 分层。
- `MatchesPage` 只协调 URL、缓存与请求；紧凑详情移动至独立组件。
- 好友分析榜单与指挥台赛季健康卡已独立，减少页面文件职责混杂。
- 前端测试按 `resources`、`unit`、`components`、`legacy` 分层编排，保留原有回归覆盖。

## 验证与证据

完整入口：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

该入口已验证通过以下内容：

- 受跟踪发布面与构建产物的敏感信息/绝对路径扫描；
- 资源准备、`npm test`、`npm run build`；
- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --tests`；
- STAB-005/006 专项契约测试；
- `git diff --check`；
- 基于 `git archive HEAD` 的干净副本：`npm ci`、前端测试/构建、Rust 格式检查和 Rust 全量测试。

CI 位于 `.github/workflows/verify.yml`，使用 Windows runner、Node 24 和 Rust stable（含 rustfmt、clippy），调用同一根验证脚本，不复制验证命令，也不使用秘密、用户数据或 app-data artifact。

更详细的逐项记录见 [STAB-007 验证证据](../verification/STAB-007-evidence.md)。

## 受控后续项与风险

1. `analytics/mod.rs` 中 repository/metrics 仍共享行模型，尚未继续拆分；继续拆分前应先补足数据映射回归测试。
2. 现有样式规则暂保留在 `styles/shared.css`；在补齐视觉快照或组件渲染基线之前，不建议机械拆分声明。
3. Vite 构建仍报告单个 JavaScript chunk 超过 500 kB。这不影响本轮退出码或功能，但后续可独立评估路由级懒加载与手动分块。
4. 本轮按范围未验证安装包。需要发布安装版时，应单独执行干净 checkout 的 `tauri build`、安装后启动烟测和隐私/路径复核。

## 建议的后续工作方式

- 日常开发先执行 `scripts/verify.ps1 -ChangedOnly`；准备合并或发布时执行不带跳过参数的完整入口。
- 保持“一个问题、一组契约测试、一个小提交”的粒度；schema 改动必须同时包含迁移和升级库测试。
- CI 失败先用同一根脚本在本地复现，不在 workflow 中添加临时的重复命令。
- 稳定标签暂未创建；只有在用户明确授权后，才执行文档中记录的 `personal-stable-YYYYMMDD` 标签命令。
