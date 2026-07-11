# STAB-004 同步身份与敏感数据治理审计设计

- 日期：2026-07-11
- 状态：依据已批准总设计自主确认
- 前置：STAB-003 已验证后实施
- 范围：只同步当前登录账号本人；阻止 secret 进入文件、SQLite、UI、日志和错误

## 1. 审计结论

当前存在四条相互连接的 P0 风险：

1. 产品仍通过 BattleTag 绑定任意目标玩家，而不是当前浏览器登录账号本人。
2. `season_fetch.rs` 主动把 `customer_token` 写入赛季 payload，并保留上游嵌套 token-like 字段。
3. `importer.rs` 将 entry、detail 和 error 原文写入 SQLite raw JSON；只有部分 player JSON 局部脱敏。
4. `commands.rs::to_message` 直接返回 `error.to_string()`，多个模块各自维护不完整的文本脱敏。

因此 STAB-004 不是局部 UI 改名，而是同步身份、落盘边界和错误边界的一次安全收口。

## 2. 身份现状

旧产品链路为：

```text
OnboardingPage BattleTag 输入
  -> bind_player_target
  -> search_bnet_account
  -> .dashen_player_target.json
  -> sync_season 读取任意目标 customer_token
```

证据集中在：

- `src/pages/OnboardingPage.tsx`
- `src/shared/api.ts`
- `src-tauri/src/commands.rs`
- `src-tauri/src/player_target.rs`
- `scripts/phase8RealFetch.test.ts`

登录流水线已经能从当前登录会话的 report-token 结果取得账号认证信息和当前账号 customer token。新权威模型应为：

```text
浏览器当前登录会话
  -> credentials 中的当前账号身份
  -> CurrentAccountStatus 安全摘要
  -> syncSeason(season)
```

customer token 缺失时状态为 `current_player_missing` 并禁止同步；不得用账号认证 token 代替，也不得读取旧 BattleTag target 兜底。

旧 `.dashen_player_target.json` 可能指向其他玩家，必须保留但停止读取；不得自动迁移成本人身份。

## 3. 持久化与输出风险

### 3.1 赛季 JSON

`season_fetch.rs::build_payload` 当前写入顶层 `customer_token`，并把 profile、count、entries、details、list pages 原样组合。现有 `phase8_real_fetch` 测试还明确要求 secret 保留，必须先反转测试契约。

### 3.2 SQLite

`importer.rs::import_payload_value` 对以下字段没有统一安全入口：

- `matches.raw_entry_json`
- `match_details.raw_detail_json`
- `sync_errors.raw_json`
- `sync_errors.message`

查询时再脱敏只能保护 UI，不能清除磁盘风险。

### 3.3 错误与日志

`redaction.rs`、`credential_store.rs`、`sync.rs` 和 `season_fetch.rs` 存在重复实现。按空格切分的文本脱敏无法可靠处理 URL query、紧凑 JSON、header 和无空格字符串。

## 4. 选定设计

### 4.1 单一当前账号身份

引入内部 `CurrentAccountIdentity` 和对前端安全的 `CurrentAccountStatus`：

- `state`: `missing_credentials | current_player_missing | ready | stale`
- `message`: 中文安全摘要
- `display_name`: 仅在当前登录上下文有可靠显示名时返回
- `identified_at`: 可选

DTO 不含 token、customer token、role ID、原始 GUID 或任意目标 BattleTag。无可靠名称时显示“已识别当前账号”。登出后凭据失效即不可同步。

### 4.2 唯一脱敏模块

`redaction.rs` 成为唯一入口：

- `sanitize_persisted_json`：递归移除敏感键。
- `redact_json_for_display`：递归替换敏感值。
- `redact_sensitive_text`：处理结构化文本、URL query、JSON/header 片段。
- `safe_error_message`：所有 Tauri command、同步状态和持久化错误共用。

敏感键至少覆盖 token、customerToken/customer_token、cookie、authorization、credential、storage 的大小写和命名变体。仅按键判断，避免误伤 `token-style` 资源名等普通值。

### 4.3 双层落盘保护

赛季 payload 构造阶段不再创建 secret 字段；原子写文件前再次执行递归清理。importer 在入口创建安全 payload，所有解析和持久化从安全副本读取；错误 message 另过文本脱敏。

### 4.4 存量治理

用独立、幂等的 privacy migration：

1. 只检测目标文件和表，不输出内容。
2. 首次处理前备份 SQLite 与赛季 JSON 到 `app_data_dir/privacy-backups/<timestamp>/`。
3. JSON 通过临时文件原子替换。
4. SQLite 在单事务内清理 raw JSON 和 message。
5. 成功后写 marker；失败不写 marker。

迁移逻辑在 STAB-004 封装；STAB-005 负责把它登记到正式迁移注册表。两阶段不得重复实现或同时假定拥有 schema v4。

## 5. STAB-003 接口依赖

复用已提交的 `AppPaths`：credentials、database、season payload、backup 路径。若 STAB-003 提供原子 JSON 写入则直接复用。产品运行不再需要 `player_target_path()`；遗留路径只用于保留旧文件。

## 6. 验收边界

- BattleTag 输入、绑定和搜索不再从产品 UI/Tauri invoke 可达。
- 同步请求只接受 season，目标身份只来自当前登录会话。
- 合成 secret 在 DTO、错误、赛季 JSON、SQLite raw JSON、同步状态和诊断输出中均为零。
- S21/S22/S23 固定赛季行为不变，不新增“同步全部”或全局赛季状态。
- 测试只用本地 recording server、tempdir、临时 SQLite 和合成 secret。
- 不读取真实 credentials、player target、浏览器 profile、app-data SQLite 或真实玩家 dump。

## 7. 延后项

迁移注册表、busy timeout、同步互斥和 `sync_errors` 最新快照语义属于 STAB-005。大文件拆分属于 STAB-006。
