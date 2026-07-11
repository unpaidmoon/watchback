# STAB-005 数据库迁移与运行稳定性审计设计

- 日期：2026-07-11
- 状态：依据已批准总设计自主确认
- 前置：STAB-003、STAB-004 已验证后实施

## 1. 审计结论

现有导入事务、upsert、多赛季查询和角斗逐赛季 marker 已有良好基础，但仍缺少正式迁移注册表、迁移前备份、连接锁等待、桌面同步互斥、错误记录幂等和中断恢复证明。

## 2. 迁移现状

`db.rs` 当前 `CURRENT_SCHEMA_VERSION = 3`，初始化先无条件建全量 schema，再用连续 `if current < N` 迁移：

- v1 只设置版本，不在显式事务中。
- v2 的加列、索引和版本更新不构成单一事务。
- v3 使用事务。
- 无迁移名称/注册表、迁移前备份、失败回滚测试或“数据库版本高于程序”的拒绝逻辑。

已有 v1/v2 升级和重复初始化测试，但 fixture 是测试内即时建表，不是可复用旧版 SQLite fixture。

## 3. 已确认可保留行为

- `import_payload_value` 的主体导入使用单事务。
- matches/details 使用 upsert。
- 投影按 season 删除并重建。
- `arena_player_index_v1:<season>` marker 和按缺失赛季回填已被测试证明。
- 多赛季聚合隔离已有测试。
- 当前协议假设 `match_id` 全局唯一，已有测试锁定；本阶段不改为 `(season, match_id)`。

## 4. 稳定性缺口与选定设计

### 4.1 显式迁移注册表

新增 `database/migrations.rs`，每个 migration 具有版本、名称和事务内 apply 函数。每一步在独立事务内执行变更、回填、更新 `user_version` 并提交。数据库版本高于当前程序时拒绝打开。

STAB-004 和 STAB-005 必须协调版本：若隐私迁移已占 v4，本阶段从 v5 开始；否则由最终已提交状态确定，不能预先硬编码冲突版本。

### 4.2 迁移前一致性备份

仅当已有文件库需要升级时，在 `AppPaths` 的 backup 目录创建 SQLite 一致性备份；空库首次初始化不备份。迁移失败时源库保持原样，备份保留。数据库模块接收普通路径，不反向依赖 Tauri `AppHandle`。

### 4.3 连接配置

所有连接统一设置 5 秒 busy timeout。文件数据库使用 WAL + synchronous NORMAL；内存测试库不强制 WAL。继续使用短连接和显式事务，不引入连接池或 ORM。

### 4.4 全局同步互斥

新增进程级 `SyncCoordinator`。任一赛季同步进行时，第二个桌面同步立即得到稳定中文错误。guard 覆盖身份读取、网络抓取、原子文件写入和 SQLite 导入，靠 RAII 在成功、错误或取消时释放。

采用单全局锁而不是逐赛季锁，因为各赛季共享一个 SQLite 和运行目录，并行收益不足以抵消恢复复杂度。

### 4.5 `sync_errors` 最新快照语义

`sync_errors` 表示“该赛季最近一次同步仍未成功取得的详情”，而不是无限历史日志：

1. 同一导入事务内先删除目标 season 的旧错误。
2. 清理存量重复项。
3. 建立 season/match/type/message 归一唯一索引。
4. 使用 `INSERT OR IGNORE` 防御输入内重复。
5. 成功同步无错误时该赛季旧错误归零，其他赛季不受影响。

### 4.6 中断恢复

用临时 SQLite trigger 注入事务中途失败，证明旧数据不变、删除 trigger 后可重试、再次重复同步所有表计数稳定，并保持逐赛季 marker 和错误隔离。

## 5. 文件边界

新增：

- `src-tauri/src/database/migrations.rs`
- `src-tauri/src/database/backup.rs`
- `src-tauri/src/sync_coordinator.rs`
- `src-tauri/tests/stab005_database_runtime.rs`

修改 `db.rs`、`importer.rs`、`commands.rs`、`lib.rs` 和必要的 Cargo feature。大业务文件拆分留给 STAB-006。

## 6. 验收边界

- 旧库升级前确实生成一致性备份。
- 每个迁移独立事务并更新版本；失败时源库不变。
- 所有连接应用 busy timeout。
- 第二个桌面同步不能并发进入。
- 同季重复同步 matches/details/projections/errors 均不增长。
- 中断后可重试；S21/S22/S23 状态、错误和投影互不污染。
- 测试只用 tempdir、合成 SQLite 和本地 recording，不读取真实数据。
