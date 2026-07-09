# 第八阶段真实赛季抓取设计

## 背景

当前指挥台已经具备固定 S21 / S22 / S23 三张赛季卡片和单赛季同步入口，但真实上游同步验证失败。现有 Rust 抓取逻辑主要问题不是 UI，也不是 SQLite 导入，而是抓取协议没有完整复刻已验证的 Python 主脚本流程。

可参考的已验证实现位于另一个项目：

- `D:/Workspace/OW数据/astrbot_plugin_owstats777/scripts/fetch_player_data.py`
- `D:/Workspace/OW数据/astrbot_plugin_owstats777/scripts/fetch_dashen_credentials.py`
- `D:/Workspace/OW数据/astrbot_plugin_owstats777/Overstats/src/client/apiclient.py`
- `D:/Workspace/OW数据/astrbot_plugin_owstats777/Overstats/src/modules/dashen_match/requests.py`

Python 主脚本的关键链路是：

1. 获取网易大神账号级 `role_id` / `token`。
2. 通过 BattleTag 搜索目标玩家。
3. 从搜索结果提取目标玩家 `customerToken`。
4. 使用目标玩家 `customerToken` 抓资料卡、统计、历史列表和单局详情。
5. 先保存完整 JSON，再由下游逻辑导入或分析。

当前桌面端的核心缺口是：没有明确的目标玩家绑定流程。在凭据里没有 `customer_token` 时，代码会把账号级 `token` 当成玩家 `customerToken` 使用，这会导致上游返回空列表或不可用结果。

## 目标

本设计目标是让桌面端具备真正可验证的数据抓取能力：

- 能通过网易大神登录凭据和 BattleTag 解析目标玩家。
- 能保存目标玩家 `customerToken` 的安全摘要状态，但不在 UI 或日志中显示 secret。
- 能按单赛季抓取完整历史战绩列表。
- 能抓取每一局的详情 payload。
- 每次同步必须先保存完整本地 JSON，再导入 SQLite。
- 即使 SQLite 导入失败，也要保留已抓到的完整 JSON 作为诊断和离线数据源。
- 指挥台继续只提供 S21 / S22 / S23 单赛季同步入口，不新增“同步全部赛季”。

最低验收标准：

- 点击 `同步 S23` 后，至少生成结构完整的 `local_s23_full_season.json`。
- 如果上游返回 0 场，也生成 JSON，并在 JSON 中保留脱敏后的列表响应诊断信息。
- 如果抓到 entries 但部分 details 失败，JSON 保留 entries、成功 details 和 detail_errors。

## 非目标

- 不把另一个项目作为运行时依赖长期绑定到桌面端。
- 不在桌面端 UI 中接入 `dump_season_json` 诊断入口。
- 不做“同步全部赛季”。
- 不做全局当前赛季。
- 不改对局页、战报页、排行页、好友分析页的主要布局。
- 不读取或展示真实凭据文件原文、cookie、浏览器 storage 或 Chrome profile 内容。
- 不在 UI、日志、测试输出中显示 `token`、`customerToken`、cookie、authorization 或浏览器 storage。

## 推荐方案

采用 Rust 原生重写 Python 主脚本的抓取链路，并把 Python 实现作为协议参考和验收对照。

不建议桌面端长期直接调用 Python 脚本，原因是：

- Tauri 打包后 Python 解释器和依赖不可控。
- 另一个项目路径不是桌面端稳定依赖。
- 浏览器登录、凭据保存、错误脱敏和进度事件会变成跨语言边界问题。
- 用户安装桌面端后不应再要求配置 Python 环境。

短期允许保留一个命令行级诊断对照工具，但不接入指挥台 UI。

## 数据流

真实同步链路如下：

```text
登录网易大神
  -> 保存账号级 role_id / token
  -> 用户输入或选择目标 BattleTag
  -> searchBnetAccount(BattleTag)
  -> 得到目标玩家 customerToken
  -> queryCard / queryCountInfo
  -> queryMatchList(sport/leisure, season, page...)
  -> fight/queryMatchList(QuickFight/LeisureFight/SportFight, season, page...)
  -> queryMatchInfo / fight/queryMatchInfo
  -> 保存 local_sXX_full_season.json
  -> entries > 0 时导入 SQLite
  -> 刷新指挥台三赛季状态
```

关键约束：

- 账号级 `token` 只用于认证请求头和 BattleTag 搜索。
- 玩家 `customerToken` 才用于 `queryCard`、`queryCountInfo`、`queryMatchList`、`queryMatchInfo`。
- 如果没有目标玩家 `customerToken`，禁止同步，不允许 fallback 到账号级 `token`。

## 后端模块设计

### `dashen_client.rs`

新增 Rust API 客户端，职责是复刻 Python `DashenAPIClient` 的必要能力。

建议提供：

```rust
pub struct DashenClient { ... }

impl DashenClient {
    pub async fn search_bnet_account(&self, bnet: &str) -> Result<Value>;
    pub async fn query_card(&self, customer_token: &str) -> Result<Value>;
    pub async fn query_count_info(&self, customer_token: &str, game_mode: &str, season: Option<i64>) -> Result<Value>;
    pub async fn query_match_list(&self, customer_token: &str, game_mode: &str, page: i64, season: Option<i64>) -> Result<Value>;
    pub async fn query_match_info(&self, customer_token: &str, match_id: &str) -> Result<Value>;
    pub async fn fight_query_match_list(&self, customer_token: &str, game_mode: &str, page: i64, season: Option<i64>) -> Result<Value>;
    pub async fn fight_query_match_info(&self, customer_token: &str, match_id: &str) -> Result<Value>;
}
```

请求头对齐 Python：

- `Accept: application/json, text/plain, */*`
- `GL-Bigdata-Auth-Token: <账号级 token>`
- `GL-Bigdata-Dts: <bigdata dts>`
- `GL-Bigdata-Role-Id: <role_id>`
- `GL-Bigdata-Server: <server>`
- `GL-ClientType: 60`
- `Origin: https://act.ds.163.com`
- `Referer: https://act.ds.163.com/`
- `User-Agent: <与 Python 配置一致的 UA>`

需要注意 `GL-Bigdata-Dts`：Python 的 customer API 统一使用 `DASHEN_BIGDATA_DTS`，不是简单使用凭据里的 `dts`。桌面端应显式定义同等常量或保存为配置，并通过测试锁定。

### `player_target.rs`

新增目标玩家绑定模块。

职责：

- 保存当前目标玩家的安全摘要。
- 支持通过 BattleTag 搜索目标玩家。
- 从搜索 payload 提取 `customerToken`。
- 将 secret 写入本地 app data 文件，但 UI 只展示安全摘要。

建议结构：

```rust
pub struct PlayerTarget {
    pub input_bnet: String,
    pub resolved_bnet: Option<String>,
    pub customer_token: String,
    pub bound_at: String,
}

pub struct PlayerTargetStatus {
    pub state: String,
    pub input_bnet: Option<String>,
    pub resolved_bnet: Option<String>,
    pub bound_at: Option<String>,
}
```

本地保存位置建议：

```text
<app_data_dir>/.dashen_player_target.json
```

该文件可包含 `customer_token`，但绝不能在 UI、日志、测试输出和错误消息中显示原值。

### `season_fetch.rs`

新增整赛季抓取模块，职责是复刻 Python `fetch_full_season_player_data` 和 `DashenMatchRequests.list_history_matches`。

输入：

- 网易大神登录凭据。
- 目标玩家 `customerToken`。
- `input_bnet` / `resolved_bnet`。
- `season`。
- 抓取参数：`pages_per_batch`、`detail_concurrency`、`include_fight`。

输出：

```rust
pub struct FullSeasonFetch {
    pub payload: serde_json::Value,
    pub entries: usize,
    pub details: usize,
    pub failed_details: usize,
}
```

普通历史列表抓取：

- `game_modes = ["sport", "leisure"]`
- endpoint: `/customer/queryMatchList`
- params: `token=<customerToken>`, `gameMode=<sport|leisure>`, `page=<page>`, `season=<season>`

角斗历史列表抓取：

- `fight_modes = ["QuickFight", "LeisureFight", "SportFight"]`
- endpoint: `/customer/fight/queryMatchList`
- params: `token=<customerToken>`, `gameMode=<fight_mode>`, `page=<page>`, `season=<season>`

分页策略：

- 从 page 1 开始。
- 每个 mode 可以按 `pages_per_batch` 并发请求。
- 一个 batch 解析不到任何 match entries 时停止该 mode。
- 以 `matchId` 去重。
- 按 `beginTs` 倒序排序。

详情抓取：

- 普通对局使用 `/customer/queryMatchInfo`。
- 角斗对局使用 `/customer/fight/queryMatchInfo`。
- 是否角斗由 `gameMode` 包含 `fight` 判断。
- 单局详情失败不应中断整个赛季，写入 `detail_errors`。

## JSON 格式

同步必须先写本地 JSON，再导入 SQLite。

输出路径：

```text
apps/owstats-desktop/data/players/local_s21_full_season.json
apps/owstats-desktop/data/players/local_s22_full_season.json
apps/owstats-desktop/data/players/local_s23_full_season.json
```

建议 payload：

```json
{
  "input_bnet": "玩家名#12345",
  "resolved_bnet": "玩家名#12345",
  "profile_card": {},
  "count_info": {
    "sport": {},
    "leisure": {}
  },
  "match_lists": {
    "sport": {},
    "leisure": {}
  },
  "fetch_mode": "full_season",
  "season": 23,
  "matches": {
    "total": 0,
    "entries": [],
    "details": [],
    "detail_errors": []
  },
  "fetch_diagnostics": {
    "list_pages": []
  }
}
```

脱敏规则：

- 保存 JSON 前递归移除或替换字段名包含以下内容的字段：
  - `token`
  - `cookie`
  - `authorization`
  - `credential`
- `customerToken` 不应出现在 JSON 原文中。
- 错误消息要走已有脱敏逻辑。

## 指挥台 UI 设计

指挥台保留现有能力：

- 登录网易大神 / 重新登录。
- 取消登录。
- 退出登录。
- 检查更新。
- 登录状态展示。
- 数据库健康状态展示。
- 账号基础信息展示。
- S21 / S22 / S23 三张赛季卡片。

新增一个“目标玩家”区域，放在登录状态和赛季卡片附近。

最小交互：

```text
目标玩家
[ BattleTag 输入框 ] [解析并绑定]
状态：未绑定目标玩家 / 已绑定目标玩家 / 绑定失败
```

同步按钮启用条件：

- 登录凭据可用。
- 目标玩家已绑定。
- 没有其他赛季正在同步。

如果缺目标玩家，三张赛季卡片同步按钮禁用，并显示中文状态：

```text
请先绑定目标玩家
```

不新增“同步全部赛季”。

## 错误处理

错误必须分层，不允许统一显示“同步失败”。

建议错误类型：

```text
MissingCredentials：未登录网易大神
MissingPlayerTarget：未绑定目标玩家
SearchBnetFailed：BattleTag 搜索失败
CustomerTokenMissing：搜索结果缺少玩家标识
UpstreamUnauthorized：登录凭据失效
UpstreamRateLimited：请求过快，请稍后重试
UpstreamEmptySeason：接口成功但赛季无对局
ListFetchFailed：赛季列表抓取失败
DetailFetchPartialFailure：部分详情抓取失败
JsonWriteFailed：本地 JSON 保存失败
ImportFailed：SQLite 导入失败
```

处理规则：

- 没有登录凭据：禁止同步，提示登录。
- 没有目标玩家：禁止同步，提示绑定 BattleTag。
- BattleTag 搜索失败：不进入赛季抓取。
- 搜索成功但没有 `customerToken`：不进入赛季抓取。
- 列表抓取失败：保存已有诊断信息，提示稍后重试或重新登录。
- 列表成功但 entries 为 0：保存 JSON，提示“未拉取到该赛季对局”。
- 详情部分失败：保存 JSON，允许导入成功部分，并展示失败详情数。
- JSON 保存失败：停止，不导入 SQLite。
- SQLite 导入失败：保留 JSON，提示“JSON 已保存，导入失败”。

## 测试计划

### 后端协议测试

使用本地模拟 HTTP server，不访问真实网易接口。

覆盖：

1. `search_bnet_account` 使用账号级 token 和 role_id。
2. `query_match_list` 使用目标玩家 `customerToken`，不是账号级 token。
3. 没有目标玩家 `customerToken` 时同步失败，不 fallback 到账号级 token。
4. 普通历史列表只请求 `sport` / `leisure`。
5. 角斗历史列表请求 `QuickFight` / `LeisureFight` / `SportFight`。
6. 请求带 `season` 参数。
7. customer API 请求头包含 Python 等价的 bigdata headers。
8. 响应结构支持 `data.matchList`、`data.recentMatchList`、`result.matchList`。

### 后端 JSON 测试

覆盖：

1. 成功抓取 S23 后生成 `local_s23_full_season.json`。
2. JSON 包含 `profile_card`、`count_info`、`match_lists`、`matches.entries`、`matches.details`、`matches.detail_errors`。
3. entries 为 0 时仍保存 JSON 和列表页诊断。
4. 单局详情部分失败时保留成功详情和失败详情。
5. JSON 不包含 token、customerToken、cookie、authorization、credential。
6. JSON 保存失败时不导入 SQLite。

### 后端导入测试

覆盖：

1. JSON 保存成功后再导入 SQLite。
2. S21 / S22 / S23 同库共存不混赛季。
3. 重复同步同一赛季幂等。
4. 普通对局和角斗对局计数分开。
5. `failedDetailCount` 正确统计。

### 前端源码测试

覆盖：

1. 指挥台包含目标玩家绑定区域。
2. 未绑定目标玩家时 S21 / S22 / S23 同步按钮禁用。
3. 绑定目标玩家后允许单赛季同步。
4. 不存在“同步全部”。
5. 新增用户可见文案全部为中文。
6. 不把 `dump_season_json` 接入 UI。

## 实施切片

### 切片 A：禁止错误 fallback

- 移除“没有 customerToken 就使用账号 token”的逻辑。
- 缺目标玩家时返回明确错误。
- 增加测试锁定该行为。

验收：

- 没有目标玩家时无法同步。
- 错误文案中文且不泄露 secret。

### 切片 B：目标玩家绑定

- 新增 BattleTag 解析命令。
- 实现 `searchBnetAccount`。
- 保存目标玩家摘要和 customerToken。
- 指挥台增加目标玩家绑定区域。

验收：

- 用户能输入 BattleTag 并绑定。
- UI 只显示安全摘要。
- 本地状态能判断是否已绑定。

### 切片 C：完整 JSON 抓取

- 实现 Rust 原生 `DashenClient`。
- 实现完整赛季列表和详情抓取。
- 保存 `local_sXX_full_season.json`。
- 暂时可以先不导入 SQLite，只证明 JSON 完整落盘。

验收：

- S21 / S22 / S23 任一赛季可生成结构完整 JSON。
- 0 场、部分详情失败、上游错误都有可诊断 JSON 或中文错误。

### 切片 D：JSON 导入 SQLite

- JSON 成功保存后调用现有 importer。
- 指挥台状态从 SQLite 刷新。
- 保持多赛季隔离。

验收：

- 同步完成后卡片计数更新。
- 本地 JSON 与 SQLite 计数可对应。

### 切片 E：错误体验和回归

- 完善错误分类。
- 完善进度事件。
- 增加源码级 UI 测试和后端集成测试。

验收：

- 用户能区分“未登录”“未绑定玩家”“上游空赛季”“详情部分失败”“导入失败”。

## 验证命令

后端：

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
cargo build --bin dump_season_json
```

前端：

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

差异检查：

```powershell
git diff --check
```

## 待确认问题

1. 目标玩家是否只支持一个全局绑定玩家，还是允许保存最近多个 BattleTag。
2. `GL-Bigdata-Dts` 的稳定取值应直接固定为 Python 配置值，还是随登录凭据保存。
3. 真实上游同步是否允许在开发机手动触发一次，用于确认 Rust 与 Python 输出结构一致。

