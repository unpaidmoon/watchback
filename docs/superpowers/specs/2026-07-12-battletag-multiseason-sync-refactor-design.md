# BattleTag 多赛季同步重构设计

- 日期：2026-07-12
- 状态：已确认，取代此前“仅当前浏览器账号、固定 S21/S22/S23”的同步边界。

## 目标

桌面端原生实现已验证 Python 脚本的工作流：浏览器登录取得大神账号凭据；用户输入一个当前 BattleTag；后端经 `searchBnetAccount` 解析目标 `customerToken`；用户输入任意单赛季号并同步完整对局与详情。

## 产品边界

- 一个本地当前目标玩家，不做目标玩家历史列表。
- BattleTag 是显式输入，可重新绑定和清除。
- 赛季号为正整数；每次只同步一个赛季，不增加“同步全部”。
- `role_id`、账号 token、目标 `customerToken` 持久化到 app data；不在 UI、日志、错误或赛季 payload 显示。
- 赛季 JSON 与 SQLite 原始数据继续递归移除 token-like 字段；本地持久化凭据不等于允许把凭据混入业务数据。

## 架构

```text
OnboardingPage
  -> start_dashen_login -> DashenCredentials
  -> bind_player_target(BattleTag) -> PlayerTarget
  -> sync_season(season) -> DashenClient / sync facade
  -> sanitized season JSON -> SQLite importer
```

### 凭据

`credentials.rs` 保留浏览器 CDP + 签名 `getReportToken` 登录实现，只以 `role_id/token/dts/server` 判定登录可用。移除“必须从登录回包取得 customerToken”的当前账号等待逻辑。

### 目标玩家

恢复 `player_target.rs` 作为唯一目标存储。`DashenClient::search_bnet_account` 使用本地账号凭据发出 Python 等价 POST；成功后保存规范化 BattleTag、上游解析名称、`customerToken` 与绑定时间。`queryCard` 仅用于确认目标可读取。

### 多赛季同步

`sync` 从目标文件读取 customer token，按 Python `fetch_full_season_player_data` 语义抓取普通与斗技模式、分页批次和详情并发。赛季参数由用户显式提供，所有查询、落盘和 SQLite 投影继续带 season。

## 前端

指挥台显示：登录凭据状态、当前 BattleTag 目标、BattleTag 输入/绑定/清除操作、赛季号输入框与“同步 S{n}”按钮。已有赛季作为快捷选择，不再硬编码三张赛季卡。

## 迁移

旧 `.dashen_credentials.json` 和 `.dashen_player_target.json` 继续兼容解析。此前为“当前浏览器账号识别”加入的等待/性能资源扫描逻辑被移除，避免两套目标来源竞争。

## 验收

录制 HTTP 服务覆盖登录凭据、BattleTag 解析、目标状态、任意赛季请求、普通/斗技列表、详情并发与失败容错；临时 app data 验证重启后可复用凭据和目标。真实账号仅由用户手工完成一次端到端验收，开发/测试日志不回显凭据。
