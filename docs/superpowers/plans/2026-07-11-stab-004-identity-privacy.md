# STAB-004 同步身份与敏感数据治理实施计划

> 前提：STAB-003 已验证。严格测试先行，小提交串行执行，不读取真实隐私数据。

## 任务 1：建立唯一脱敏策略

**文件：** `redaction.rs`、`credential_store.rs`、`commands.rs` 及测试。

1. 先写失败测试，覆盖递归 JSON 键、URL query、紧凑 JSON、Authorization/header、cookie 和无空格文本。
2. 验证 `token-style` 等普通值不被误伤。
3. 实现 `sanitize_persisted_json`、`redact_json_for_display`、`redact_sensitive_text`、`safe_error_message`。
4. 让其他模块删除重复实现或委托唯一入口。
5. 提交：`fix: centralize sensitive data redaction`。

## 任务 2：反转赛季 JSON 隐私契约

**文件：** `season_fetch.rs`、`sync.rs`、`phase8_real_fetch.rs`、`sync_client.rs`。

1. 将现有“保留 raw tokens”测试改为禁止任何深度敏感键和合成 secret，确认旧实现失败。
2. payload 不再构造 `customer_token`。
3. 原子写入前再次清理完整 JSON。
4. 保留业务字段、对局/详情计数和 `target_mode: current_login_account`。
5. 提交：`fix: sanitize season payload persistence`。

## 任务 3：收口 SQLite 新写入

**文件：** `importer.rs`、`import_baseline.rs`。

1. 用临时 SQLite 和合成 secret 覆盖 entry、detail、detail error、profile、player。
2. 断言所有 raw JSON 与 error message 均不含 secret。
3. importer 入口生成安全 payload，后续只读安全副本。
4. 提交：`fix: sanitize all sqlite persistence`。

## 任务 4：切换为当前登录账号身份

**文件：** `credential_store.rs`、`dashen_auth.rs`、`credentials.rs`、`commands.rs`、`sync.rs`、`models.rs` 及后端测试。

1. 先测试 ready、current-player-missing、登出、旧 target 不解锁、账号 token 不代替 customer token。
2. 新增 `CurrentAccountIdentity`、`CurrentAccountStatus`、`SyncIdentityError`。
3. `sync_season` 只从当前 credentials 取得本人目标。
4. 保留旧 target 文件但运行链路不读取。
5. 提交：`feat: switch sync to current account identity`。

## 任务 5：移除 BattleTag 产品入口

**文件：** `OnboardingPage.tsx`、`shared/api.ts`、`shared/types.ts` 和页面测试。

1. 先写 UI 测试：无 BattleTag/绑定/解析入口，有“当前同步账号”，未识别时同步按钮禁用。
2. 移除旧 UI/API 可达链路；不恢复 `searchBnetAccount`。
3. 保持固定 S21/S22/S23 独立按钮，不新增同步全部。
4. 提交：`feat: show current account sync status`。

## 任务 6：存量数据备份与隐私迁移

**文件：** 新增 `privacy_migration.rs`，`db.rs` 仅接入，增加临时目录/SQLite 测试。

1. 先测试备份、幂等、失败不写 marker、原子文件替换和 SQLite 单事务回滚。
2. 备份仅写 `app_data_dir/privacy-backups`。
3. 清理旧赛季 JSON 和 SQLite raw 字段；不读取、不删除旧 player target。
4. 封装为 STAB-005 可注册的迁移函数。
5. 提交：`fix: back up and sanitize legacy persisted data`。

## 任务 7：阶段复核与文档

执行：

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
cd ..
npm test
npm run build
cd ..\..
git diff --check
git status --short
```

补充扫描 BattleTag 产品入口、持久化 customer token 和 raw `to_message`。更新开发状态和进度表，记录未读取真实隐私数据、未制作安装版、未推送远端。提交：`docs: complete STAB-004 privacy stabilization`。
