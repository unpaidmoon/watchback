# 第八阶段：取消 BattleTag 绑定，改为当前登录账号赛季同步设计

## 背景

第八阶段原先的真实抓取方案依赖 `searchBnetAccount`，也就是：

```text
用户输入 BattleTag
  -> 网易大神搜索接口解析目标玩家
  -> 得到目标玩家 customerToken
  -> 抓取该目标玩家赛季数据
```

实测后确认，网易大神已经关闭或限制了该解析接口。继续在桌面端保留“输入 BattleTag 并绑定玩家”的产品入口，会导致用户卡在“未找到目标玩家”，即使登录凭据可用也无法真正同步。

因此，本设计将产品边界调整为：桌面端只同步当前浏览器登录的网易大神账号对应的守望先锋数据，不再支持输入 BattleTag 同步任意玩家。

## 已确认决策

- 不再做“解析并绑定玩家 ID / BattleTag”的功能。
- 不再把 `searchBnetAccount` 作为指挥台同步的前置依赖。
- 桌面端同步目标固定为“当前登录网易大神账号本人”。
- 继续保留浏览器登录网易大神账号流程。
- 继续保留固定 S21 / S22 / S23 三张赛季卡片。
- 继续不做“同步全部赛季”。
- 继续不做全局当前赛季。
- 继续不改对局页、战报页、排行页、好友分析页主要布局。
- 每次真实同步仍然先保存完整本地 JSON，再导入 SQLite。
- 所有新增用户可见文案必须是中文。

## 目标

本轮后续实现的目标是让指挥台具备稳定的本人赛季同步能力：

- 通过现有浏览器登录流程获得网易大神登录态。
- 从已登录账号上下文中识别当前账号对应的守望先锋数据目标。
- 同步 S21 / S22 / S23 任一单赛季数据。
- 同步前先保存结构完整的本地 JSON。
- JSON 保存成功后再导入 SQLite。
- UI 只展示安全摘要，不显示任何 secret。
- 如果无法识别当前账号的守望先锋数据目标，给出明确中文提示，引导重新登录或打开大神战绩页。

最低验收标准：

- 指挥台不再出现需要用户输入 BattleTag 并绑定目标玩家的主流程。
- 点击 `同步 S23` 时，不再依赖 `searchBnetAccount`。
- 若已登录账号可识别出本人数据目标，能生成 `data/players/local_s23_full_season.json`。
- 若上游返回空数据，也必须保存脱敏后的诊断 JSON。
- 若不能识别本人数据目标，必须明确报错，不能伪装为“同步成功 0 场”。

## 非目标

- 不支持输入任意 BattleTag 同步他人数据。
- 不接入 `dump_season_json` 到指挥台 UI。
- 不读取或展示本地凭据文件原文。
- 不读取 `data/chrome-dashen-profile/`。
- 不打印 `OW_DASHEN_TOKEN`。
- 不打印 `customerToken`。
- 不打印 cookie、浏览器 storage 或 authorization 信息。
- 不在 UI、日志、测试输出、JSON 诊断中展示 secret 原值。
- 不把旧 Python 项目作为桌面端运行时依赖。

## 推荐方案

采用“当前登录账号本人同步”方案。

登录态仍由现有 Tauri 后端浏览器登录流程负责。同步目标不再由用户输入 BattleTag 决定，而是由登录后的网易大神账号上下文决定。后端需要在安全边界内获得一个可用于 `queryCard`、`queryCountInfo`、`queryMatchList`、`queryMatchInfo` 的本人数据标识。

这个标识在内部可以仍然是 `customerToken` 或上游新的等价字段，但产品和 UI 层不应再暴露“绑定 customerToken”概念。UI 只显示“当前账号已识别 / 当前账号未识别”。

## 方案对比

### 方案 A：继续 BattleTag 绑定

流程继续依赖 `searchBnetAccount`。

优点：

- 旧设计改动最少。
- 理论上支持同步任意 BattleTag。

缺点：

- 核心接口已不可用。
- 用户会持续遇到“未找到目标玩家”。
- 同步能力不可验证，不适合作为产品主流程。

结论：放弃。

### 方案 B：只同步当前登录账号本人

流程使用现有浏览器登录态，从当前账号上下文识别本人守望先锋数据目标。

优点：

- 符合现在可用的登录能力。
- 不依赖已关闭的 BattleTag 搜索接口。
- 产品边界清晰：桌面端是“我的本地战绩中心”。
- 安全风险更低，不需要支持任意玩家查询。

缺点：

- 不再支持同步他人 BattleTag。
- 需要补充“已登录账号 -> 本人数据目标”的识别逻辑。

结论：采用。

### 方案 C：允许手工导入旧 JSON

保留同步入口之外，提供手工导入旧项目 JSON。

优点：

- 能利用历史数据文件。
- 不依赖上游接口。

缺点：

- 不是在线同步。
- 无法解决 S23 新数据抓取。
- 容易和真实同步主流程混淆。

结论：可作为后续工具能力，不作为本轮主目标。

## 新数据流

```text
登录网易大神
  -> 后端保存脱敏后的登录状态摘要
  -> 后端从登录上下文识别当前账号本人数据目标
  -> 指挥台显示“当前账号已识别”或“当前账号未识别”
  -> 用户点击同步 S21 / S22 / S23
  -> 后端使用本人数据目标抓取 profile_card / count_info / match list / detail
  -> 保存 data/players/local_sXX_full_season.json
  -> entries > 0 时导入 SQLite
  -> 刷新指挥台三赛季状态卡片
```

关键约束：

- 登录凭据用于认证请求。
- 本人数据目标用于玩家数据查询。
- 不允许把账号认证 token 当作玩家数据目标使用。
- 没有本人数据目标时必须阻止同步。
- 0 场数据必须保存 JSON 并返回可诊断错误，不能显示为成功同步。

## 后端设计

### 登录态

现有登录流程继续负责：

- 打开网易大神浏览器登录。
- 采集账号级登录凭据。
- 保存凭据到 app data。
- 对外只返回安全摘要。

需要新增或调整：

- 登录完成后尝试识别当前账号对应的守望先锋数据目标。
- 识别失败时不影响“登录凭据可用”状态，但同步按钮应显示需要识别账号。
- 识别状态与登录状态分开表达。

建议 DTO：

```rust
pub struct CurrentPlayerStatus {
    pub state: String, // missing | detected | stale | unsupported
    pub display_name: Option<String>,
    pub detected_at: Option<String>,
    pub message: Option<String>,
}
```

用户可见中文状态：

- `已识别当前账号`
- `未识别守望先锋账号`
- `请重新登录并打开网易大神守望先锋战绩页`
- `登录凭据可用，当前账号未识别`

### 本人数据目标存储

内部可以继续复用类似 `.dashen_player_target.json` 的本地文件，但语义需要改名或重定义为“当前登录账号本人目标”。

建议文件：

```text
<app_data_dir>/.dashen_current_player.json
```

建议结构：

```rust
pub struct CurrentPlayerTarget {
    pub display_name: Option<String>,
    pub data_target: String,
    pub detected_at: String,
    pub source: String,
}
```

约束：

- `data_target` 可包含上游查询所需 secret，因此不得返回给 UI。
- UI 只能看到 `display_name`、`detected_at`、`state`。
- 保存文件前可以保留 secret，但任何日志、错误、测试输出必须脱敏。

### 本人数据目标识别

识别逻辑优先从已登录上下文获取，不再调用 BattleTag 搜索。

候选来源按优先级：

1. 浏览器登录流程返回的网易大神页面上下文中已有的玩家数据目标。
2. 登录后访问守望先锋战绩页时，前端页面或网络响应中可识别的本人数据目标。
3. 已登录账号可调用的“本人资料 / 我的角色 / 当前角色”类接口。

安全要求：

- 可以在后端内部解析页面上下文或 API 响应。
- 不打印原始响应。
- 不保存浏览器 storage 原文。
- 不把 cookie、token、customerToken 原值写入日志。
- 诊断 JSON 只能保存脱敏后的结构摘要和接口返回状态。

如果以上来源都无法识别，返回明确错误：

```text
未识别当前网易大神账号的守望先锋数据，请重新登录并打开守望先锋战绩页后再试。
```

### 赛季抓取

抓取链路继续使用 Rust 原生实现：

- `queryCard`
- `queryCountInfo`
- `queryMatchList`
- `fight/queryMatchList`
- `queryMatchInfo`
- `fight/queryMatchInfo`

S23 当前赛季请求策略沿用旧项目行为：

- 若请求赛季是当前大神赛季，先尝试不带 `season` 参数。
- 若不带 `season` 返回空，再尝试显式 `season=23`。
- S21 / S22 使用显式 `season=21/22`。

列表分页策略：

- `sport`、`leisure` 分开抓取。
- `QuickFight`、`LeisureFight`、`SportFight` 分开抓取。
- 每个模式从 page 1 开始。
- 当前页或当前批次无 entries 时停止该模式。
- 以 `matchId` 去重。
- 按 `beginTs` 倒序排序。

详情策略：

- 普通对局调用 `queryMatchInfo`。
- 角斗对局调用 `fight/queryMatchInfo`。
- 详情失败写入 `detail_errors`，不丢弃 entries。
- JSON 保存成功后再进入 SQLite 导入。

## 前端设计

### 指挥台保留能力

继续保留：

- 登录网易大神 / 重新登录。
- 取消登录。
- 退出登录。
- 检查更新。
- 登录状态展示。
- 数据库健康状态展示。
- 账号基础信息展示。
- S21 / S22 / S23 三张固定赛季卡片。
- 每张赛季卡片单独同步按钮。

### 移除 BattleTag 绑定入口

指挥台不再展示：

- BattleTag 输入框。
- `绑定玩家` 按钮。
- `解析玩家` 按钮。
- 任意依赖 `searchBnetAccount` 的用户流程。

可以保留内部诊断命令或测试代码，但不能接入产品 UI。

### 新增当前账号识别状态

在登录状态附近展示一个小区域：

```text
当前同步账号
已识别当前账号：风偏偏呢#5479
```

或：

```text
当前同步账号
未识别守望先锋账号，请重新登录并打开网易大神守望先锋战绩页
```

同步按钮启用条件：

- 登录凭据可用。
- 当前账号本人数据目标已识别。
- 当前没有其他赛季正在同步。

同步按钮禁用状态：

- 未登录：`请先登录网易大神`
- 已登录但未识别：`请重新登录并打开守望先锋战绩页`
- 同步中：只显示当前赛季同步中，其他赛季按钮禁用。

## JSON 设计

真实同步继续写入：

```text
data/players/local_s21_full_season.json
data/players/local_s22_full_season.json
data/players/local_s23_full_season.json
```

建议结构：

```json
{
  "profile_card": {},
  "count_info": {
    "sport": {},
    "leisure": {}
  },
  "fetch_mode": "rust_sync",
  "target_mode": "current_login_account",
  "season": 23,
  "matches": {
    "total": 0,
    "entries": [],
    "details": [],
    "detail_errors": [],
    "list_pages": []
  }
}
```

必须脱敏字段名：

- `token`
- `customerToken`
- `customer_token`
- `cookie`
- `authorization`
- `credential`
- `storage`

## 错误处理

错误信息必须区分原因：

```text
MissingCredentials：未登录网易大神。
CurrentPlayerMissing：未识别当前网易大神账号的守望先锋数据。
CurrentPlayerStale：当前账号识别信息已失效，请重新登录。
UpstreamUnauthorized：登录状态已失效，请重新登录。
UpstreamEmptySeason：接口成功但该赛季没有对局。
ListFetchFailed：赛季列表抓取失败。
DetailFetchPartialFailure：部分详情抓取失败。
JsonWriteFailed：本地 JSON 保存失败。
ImportFailed：SQLite 导入失败。
```

处理规则：

- 未登录：禁止同步。
- 未识别当前账号：禁止同步。
- 登录失效：禁止同步，并提示重新登录。
- 上游空赛季：保存 JSON，提示未拉取到该赛季对局。
- 部分详情失败：保存 JSON，允许导入成功部分，并展示失败详情数。
- JSON 保存失败：停止，不导入 SQLite。
- SQLite 导入失败：保留 JSON，提示本地导入失败。

## 测试计划

### 后端测试

1. 当前账号未识别时，同步 S21 / S22 / S23 都失败，且错误为中文安全摘要。
2. 不允许从账号认证 token fallback 到玩家数据目标。
3. S23 当前赛季先请求不带 `season` 参数，空了再回退 `season=23`。
4. S21 / S22 请求显式 `season=21/22`。
5. 普通对局和角斗对局分开抓取。
6. 列表抓取成功但详情部分失败时，生成 `detail_errors`。
7. JSON 保存先于 SQLite 导入。
8. JSON 不包含 token、customerToken、cookie、authorization、storage。
9. 多赛季数据不会混入同一个 season。

### 前端测试

1. 指挥台不再包含 BattleTag 绑定输入主流程。
2. 指挥台不再包含 `绑定玩家` / `解析玩家` 主流程文案。
3. 指挥台包含当前同步账号识别状态。
4. 未识别当前账号时，S21 / S22 / S23 同步按钮禁用。
5. 不存在 `同步全部`。
6. 不存在全局 `currentSeason` 同步状态。
7. 不把 `dump_season_json` 接入 UI。
8. 新增用户可见文案全部为中文。

## 迁移策略

现有 `.dashen_player_target.json` 属于旧 BattleTag 绑定方案。

迁移建议：

- 不在 UI 中继续使用旧绑定状态。
- 同步前只检查新的当前账号识别状态。
- 如果本地存在旧绑定文件，不自动删除，避免破坏用户本地调试数据。
- 后端可以忽略旧绑定文件，或只在开发诊断命令中使用。
- 旧的 `bind_player_target` 命令可以先保留但不在 UI 调用，后续再删除。

## 实施切片

### 切片 A：产品入口收口

- 移除或隐藏指挥台 BattleTag 绑定入口。
- 增加当前同步账号识别状态展示。
- 同步按钮改为依赖当前账号识别状态。

验收：

- UI 不再要求用户输入 BattleTag。
- 未识别当前账号时不能同步。

### 切片 B：当前账号目标识别

- 从登录流程或已登录页面上下文中识别本人数据目标。
- 保存为新的当前账号目标文件。
- UI 只展示安全摘要。

验收：

- 登录后能看到“已识别当前账号”。
- 不显示任何 secret。

### 切片 C：同步链路切换

- `syncSeason(season)` 改为读取当前账号目标。
- 不再读取旧 BattleTag 绑定目标。
- 保留 JSON-first 同步逻辑。

验收：

- S21 / S22 / S23 单季同步不依赖 `searchBnetAccount`。
- 本地 JSON 仍正常生成。

### 切片 D：错误和测试收敛

- 完善错误类型。
- 补后端协议测试和前端源码测试。
- 验证脱敏。

验收：

- 后端和前端测试通过。
- 错误文案清晰、中文、安全。

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

## 开放问题

1. 当前账号本人数据目标能否稳定从登录流程里直接获得，还是必须要求用户登录后打开网易大神守望先锋战绩页。
2. 本人数据目标识别失败时，是否需要提供“重新识别当前账号”按钮，还是只复用“重新登录”。
3. 旧的 `bind_player_target` 后端命令是立即删除，还是先保留为不可见诊断接口。
