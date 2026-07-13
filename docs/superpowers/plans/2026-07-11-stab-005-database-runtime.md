# STAB-005 数据库迁移与运行稳定性实施计划

> 前提：STAB-003、STAB-004 已验证。版本号以 STAB-004 的最终 schema 状态为准。

## 任务 1：建立恢复行为特征测试

新增 `src-tauri/tests/stab005_database_runtime.rs`，锁定旧库升级、重复初始化、多赛季基线、失败回滚、失败后重试、错误不增长、逐赛季错误隔离和详情恢复清错。先确认新增稳定性断言在旧实现失败。

提交：`test: characterize STAB-005 database recovery`。

## 任务 2：显式迁移注册表

1. 抽出 `database/migrations.rs`。
2. 注册现有 v1/v2/v3 和 STAB-004 最终版本之后的迁移。
3. 每步独立事务内执行 apply、更新 `user_version`、commit。
4. 拒绝高于当前程序版本的数据库。
5. 保持现有逐赛季 arena marker 行为。

验证 `import_baseline` 和新 migration 测试。提交：`refactor: add explicit database migration registry`。

## 任务 3：迁移前备份

1. 先写 backup、空库不备份、失败保留源库测试。
2. 新增 `database/backup.rs`，使用 SQLite 一致性备份到 `AppPaths.backup_dir`。
3. 文件名包含源/目标版本和 UTC 时间，不输出数据库内容。

提交：`fix: back up databases before schema migration`。

## 任务 4：SQLite 锁等待

1. 两连接测试先证明旧实现会立即锁错误。
2. 所有连接设置 5 秒 busy timeout；文件库设置 WAL + NORMAL。
3. 验证等待后成功和超时可诊断；不引入连接池。

提交：`fix: configure SQLite lock waiting`。

## 任务 5：按赛季幂等治理同步错误

1. 先测试同季重复错误不增长、成功恢复清错、其他赛季不变。
2. 迁移清理重复项并建立归一唯一索引。
3. 导入事务先删除目标 season 的旧错误，再 `INSERT OR IGNORE` 本次错误。
4. 保持指挥台 `failed_detail_count` 与“最近一次同步”一致。

提交：`fix: keep sync errors idempotent by season`。

## 任务 6：串行化桌面赛季同步

1. 先测试第二次 acquire 返回中文稳定错误，guard 在成功和错误后释放。
2. 新增 `SyncCoordinator`，由 Tauri `.manage` 注册。
3. `commands::sync_season` 在任何 I/O 前获取 guard，并覆盖完整同步。

提交：`fix: serialize desktop season sync`。

## 任务 7：证明中断恢复

用临时 trigger 注入 S22 导入失败，验证 S21/S22 旧状态不变；移除 trigger 后重试成功；第三次相同同步计数稳定；arena marker、projection 和 errors 仍逐赛季隔离。

提交：`test: prove interrupted sync recovery`。

## 任务 8：全量验证和文档

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --test stab005_database_runtime
cargo test --test import_baseline
cargo test --test sync_client
cargo test --test phase8_command_center
cargo test --test phase8_multi_season_pages
cargo test --tests
cd ..
npm test
npm run build
cd ..\..
git diff --check
git status --short
```

静态复核 `busy_timeout`、`SyncCoordinator`、迁移注册表、错误唯一索引以及禁止的仓库绝对路径。更新维护文档和进度表，记录未接触真实数据、未制作安装版、未推送远端。

提交：`docs: complete STAB-005 runtime stability`。
