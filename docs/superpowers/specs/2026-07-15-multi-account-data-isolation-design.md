# OWStatsLocal 多账号与数据隔离设计

## 背景

OWStatsLocal 当前只有一个应用数据空间：登录凭据、玩家目标、玩家资料、赛季原始文件和 `owstats.sqlite3` 都使用固定路径。登录另一个网易大神账号不会自动切换数据库，因此现有结构不能安全支持多个账号。

本设计将产品从“单个当前登录账号”扩展为“本机保存多个已登录账号，任一时刻只有一个活动账号”。每个账号保存自己的 BattleTag、role_id、凭据、玩家目标、SQLite 和同步产物。切换账号只改变 OWStatsLocal 的活动账号，不打开浏览器。

本设计仍不恢复任意 BattleTag 输入或查询他人数据。账号只能通过浏览器真实登录添加，BattleTag 只能来自已验证的登录采集结果。

## 已确认决策

- 使用“账号注册表 + 每账号独立目录”架构。
- role_id 是稳定身份键；BattleTag 是可更新的显示名称。
- 目录使用随机 account_id，不在目录名或前端 DTO 中暴露 role_id。
- 添加账号和重新登录才打开浏览器；切换账号不打开浏览器。
- 重新登录指定账号时，若浏览器检测到不同 role_id，不覆盖原账号。
- 同一 role_id 的 BattleTag 变化只更新显示名称，不创建新账号。
- 退出账号永久删除该账号的全部本地数据，而不只是删除凭据。
- 删除当前账号后，自动切换到最近使用的剩余账号；没有剩余账号时进入空账号状态。
- 旧版单账号数据保留，并在首次确认原账号后迁移到该账号目录。
- 旧数据迁移是一次性兼容逻辑，不改变多账号稳态架构。
- 指挥台原“退出登录”区域替换为“账号管理”。

## 目标

- 用户可以添加多个网易大神账号。
- 每个账号独立保存 BattleTag、role_id、token 和其他同步凭据。
- 每个账号使用独立 SQLite、玩家资料、同步 JSON、诊断和备份目录。
- 切换账号后，指挥台、对局、复盘、排行、队友分析和生涯 MMR 全部读取新账号数据库。
- 账号之间不存在查询、缓存、同步输出或凭据串用。
- 添加、切换、重新登录和退出账号都有明确中文状态、失败回滚和可恢复行为。
- 前端、日志和错误不暴露 role_id 或 token-like 字段。

## 非目标

- 不支持手工输入 BattleTag 添加账号。
- 不支持搜索或同步未登录的其他玩家。
- 不提供跨账号合并数据库、跨账号统计或账号数据共享。
- 不在本轮提供账号导入、导出或云同步。
- 不为每个账号保存独立浏览器 profile；浏览器 profile 仍只是登录采集工具。
- 不改变各页面现有赛季筛选规则和同步算法。
- 不把本地凭据加密包装宣称为系统级安全存储；本轮继续沿用现有本机文件安全边界。

## 术语与身份规则

### account_id

OWStatsLocal 生成的随机 UUID，只用于目录、注册表和 Tauri 命令参数。前端可以持有 account_id 作为不透明操作句柄，但不得从中推导 role_id。

### role_id

网易大神登录身份的稳定唯一键。它只保存在账号清单和凭据文件中，不返回前端，不出现在用户可见路径、日志、错误或诊断摘要中。

添加账号或重新登录后，后端用 role_id 查找已有账号：

- 找到相同 role_id：更新该账号凭据和 BattleTag。
- 未找到：创建新的 account_id 和账号目录。
- 重新登录指定账号时发现其他 role_id：停止覆盖流程，返回安全的账号错配结果。

### BattleTag

用户可见账号名称。它来自浏览器登录后的稳定采集结果。相同 role_id 出现新 BattleTag 时更新账号清单和账号清单文件，旧数据库继续归属同一账号。

## 文件与目录结构

```text
<app_data_dir>/
├─ accounts.json
├─ user_settings.json
├─ chrome-dashen-profile/
├─ .account-operations/
│  ├─ pending-logins/
│  ├─ deletion-journal.json
│  └─ migration-journal.json
└─ accounts/
   ├─ <account_id>/
   │  ├─ account.json
   │  ├─ credentials.json
   │  ├─ player_target.json
   │  ├─ player_profile.json
   │  ├─ owstats.sqlite3
   │  ├─ seasons/
   │  ├─ diagnostics/
   │  └─ backups/
   └─ <another_account_id>/...
```

应用级共享内容仅包括用户设置、登录浏览器 profile、正式打包资源和账号操作日志。玩家身份、数据库和同步产物都必须位于账号目录。

### accounts.json

账号注册表使用版本化结构并原子写入：

```rust
struct AccountRegistry {
    version: u32,
    active_account_id: Option<String>,
    accounts: Vec<AccountRegistryEntry>,
}

struct AccountRegistryEntry {
    account_id: String,
    battle_tag: String,
    created_at: String,
    last_used_at: String,
    credential_state: String,
}
```

注册表不保存 token、customerToken、cookie、authorization 或原始上游响应。`credential_state` 只允许 `ready | stale | incomplete | missing` 等安全枚举。

### account.json

账号清单文件保存内部身份关系：

```rust
struct AccountManifest {
    version: u32,
    account_id: String,
    role_id: i64,
    battle_tag: String,
    created_at: String,
    updated_at: String,
}
```

该文件属于本地隐私数据，不直接序列化给前端。前端账号列表来自专用安全 DTO。

## 路径解析与数据库路由

现有 `AppPaths` 拆分为两个职责：

- `SharedAppPaths`：注册表、用户设置、浏览器 profile 和操作日志。
- `AccountPaths`：指定 account_id 的凭据、玩家目标、玩家资料、数据库、赛季、诊断和备份路径。

新增 `AccountContextManager` 负责：

- 启动时校验并加载账号注册表。
- 解析和校验 active_account_id。
- 返回不可变的当前账号路径快照。
- 原子切换活动账号。
- 在注册表损坏、活动账号缺失或账号目录缺失时返回安全状态。

所有账号相关 Tauri 命令必须在入口处获得 `AccountContextSnapshot`，不得自行拼接账号路径。现有 `app_database(app)` 改为从快照构造 `OwDatabase`。凭据、玩家目标、玩家资料、同步输出和数据库必须来自同一个快照。

用户设置继续使用共享路径。任何需要玩家数据的命令在没有活动账号时返回统一中文错误，不得回退到旧版根目录数据库。

## 前端切换与缓存边界

切换账号成功后，前端立即重新加载当前 Tauri WebView：

- 销毁旧账号的 React state、页面缓存、筛选结果和未完成 Promise 回调。
- 重新读取账号列表、当前账号、数据库状态和各页面数据。
- 保留 URL 页面路径与页面自己的赛季 query；数据内容由新账号数据库重新加载。

后端在切换前取得账号操作互斥锁。同步、登录、迁移、删除和其他写入操作进行中时禁止切换。切换命令只在注册表原子落盘成功后返回成功。

凭据过期不阻止切换和查看本地数据，只阻止需要上游请求的同步操作，并引导重新登录。

## 账号管理接口

### list_accounts

返回账号管理安全 DTO：

```rust
struct AccountSummary {
    account_id: String,
    battle_tag: String,
    is_active: bool,
    credential_state: String,
    created_at: String,
    last_used_at: String,
}
```

不返回 role_id、token、customerToken、玩家目标、数据库绝对路径或原始错误。

### add_account

- 在 `.account-operations/pending-logins/<operation_id>/` 中准备临时登录结果。
- 完成 role_id、BattleTag 和凭据完整性校验后才写正式账号目录。
- 新 role_id 创建账号；已有 role_id 原子更新对应账号的凭据和 BattleTag。
- 已有账号被更新后返回 `existingAccountUpdated`，前端可以询问是否切换。
- 登录取消、超时或验证失败时删除临时目录，活动账号和已有账号不变。

### switch_account

- 校验 account_id 存在且账号目录可读。
- 拒绝在互斥操作进行时切换。
- 原子更新 active_account_id 和 last_used_at。
- 返回安全摘要，前端随后重新加载。

### relogin_account

- 记录预期 account_id，并在临时区域采集新凭据。
- role_id 与目标账号一致时，原子替换凭据并更新 BattleTag。
- 检测到不同且已存在的账号时返回 `differentExistingAccount`，不修改任何账号，并允许用户切换到该账号。
- 检测到不同且尚不存在的账号时返回 `differentNewAccount`，不覆盖目标账号，并允许用户把它添加为新账号。
- 新凭据验证完成前保留原凭据。

### remove_account

- 前端先显示包含 BattleTag 的二次确认对话框。
- 后端取得账号操作互斥锁并写入删除 journal。
- 将账号目录原子移动到同卷隔离路径，再递归删除。
- 删除失败时尝试恢复原目录并保留注册表记录；恢复失败时保留 journal，启动时进入恢复流程，不宣称删除成功。
- 物理删除完成后才从注册表移除账号。
- 删除当前账号时选择 last_used_at 最新的剩余账号；没有剩余账号时 active_account_id 置空。
- 删除范围包括账号清单、凭据、玩家目标、玩家资料、SQLite、赛季 JSON、诊断和账号备份。

## 指挥台账号管理设计

原“退出当前账号”危险区替换为“账号管理”区域。

### 添加账号行

添加账号独占一行：

```text
添加新的网易大神账号
打开浏览器完成登录，不影响已有账号数据                  [添加账号]
```

### 账号行

每个账号一行，仅显示 BattleTag、安全凭据状态和最近使用时间：

```text
Tester#5479    当前账号 · 凭据可用 · 最近使用 07-15 17:42
                                      [当前账号] [重新登录] [退出账号]
```

非当前账号显示：

```text
Second#2288    凭据需要更新 · 最近使用 07-10 09:18
                                      [切换账号] [重新登录] [退出账号]
```

交互规则：

- 每行固定保留三个操作位。
- 当前账号的“切换账号”按钮变为禁用的“当前账号”。
- 非当前账号的“切换账号”是该行主操作。
- “重新登录”使用次按钮。
- “退出账号”使用克制危险样式，并打开可访问的二次确认对话框。
- 添加、切换、重新登录、退出、同步期间，互斥操作禁用并显示中文原因。
- 宽屏账号名称与按钮同一行；窄屏按钮换到下一行，但仍保持至少 44px 命中区。
- 不显示 role_id、数据库文件名、绝对路径或 token 状态细节。

账号管理区域遵循根目录 `design.md`：灰绿画布、低对比表面、细分隔线、深炭黑主操作、有限危险色，不新增品牌色、阴影或卡片层级。

## 四个操作的用户流程

### 添加账号

```text
点击添加账号
  -> 打开登录浏览器
  -> 采集并稳定验证 role_id + BattleTag + 凭据
  -> 新账号：创建目录和注册表记录
  -> 已有账号：更新凭据和 BattleTag
  -> 询问是否切换到该账号
```

### 切换账号

```text
点击切换账号
  -> 检查互斥操作与账号目录
  -> 原子更新 active_account_id
  -> 重新加载前端
  -> 所有页面读取新账号数据库
```

### 重新登录

```text
点击账号 A 的重新登录
  -> 打开登录浏览器
  -> 相同 role_id：更新 A
  -> 登录为账号 B：不覆盖 A
       -> B 已存在：允许切换 B
       -> B 不存在：允许添加 B
```

### 退出账号

```text
点击退出账号
  -> 二次确认永久删除范围
  -> 写删除 journal
  -> 原子隔离并物理删除账号目录
  -> 更新注册表
  -> 删除当前账号时切换到最近使用账号或进入空状态
```

## 并发、失败与恢复

- 登录、添加、重新登录、切换、删除、旧数据迁移和赛季同步共用账号操作协调器。
- 数据库重建或其他会写账号数据库的后台任务也必须纳入协调器。
- 只读命令在入口获得账号路径快照；切换后通过前端重载丢弃旧页面上下文。
- 注册表、账号清单和凭据使用临时文件、flush、sync 和同目录 rename 原子替换。
- 注册表 JSON 损坏时不猜测活动账号，不扫描并自动绑定未知目录；UI 显示恢复错误。
- active_account_id 指向缺失目录时，启动流程选择最近使用且结构完整的账号，并返回中文恢复提示。
- 删除和迁移使用 journal，应用崩溃后按 journal 恢复或完成，不产生同时可见的半账号。
- Windows 文件占用导致删除失败时，不从账号列表移除该账号，也不声称退出成功。

## 旧版单账号迁移

仅在以下条件同时满足时启动兼容迁移：

- 尚无有效 accounts.json。
- 检测到旧版根目录 `owstats.sqlite3`、旧凭据、玩家目标或赛季目录之一。
- 尚无完成的迁移标记。

迁移流程：

1. 保持旧文件原位并显示“等待确认原账号”。
2. 用户成功登录原账号后，用已验证 role_id 和 BattleTag 创建目标账号临时目录。
3. 复制旧数据库、玩家资料、玩家目标、赛季文件、诊断和备份。
4. 检查复制后的 SQLite 可打开、schema 可迁移，账号凭据完整。
5. 写 migration journal，原子发布账号目录和注册表。
6. 注册成功后移除旧根目录账号文件并写完成标记。
7. 任一步失败都保留旧文件，并允许下次启动幂等重试。

`user_settings.json` 和浏览器 profile 始终保留在共享路径，不迁入账号目录。

## 隐私与安全边界

- UI、Tauri DTO、日志、事件、错误、测试输出和诊断摘要不得包含 role_id、token、customerToken、cookie、authorization、credential 原文或浏览器 storage。
- account_id 是不透明随机值，不包含 role_id、BattleTag 或其他玩家标识。
- BattleTag 可以在账号管理 UI 显示，但测试、截图和设计 fixture 必须使用合成名称。
- 账号路径只能由注册表中的已验证 UUID 解析，禁止直接接受任意路径或拼接未验证字符串。
- 删除操作必须验证目标位于 `<app_data_dir>/accounts/` 下，且不得跟随目录外符号链接或 junction。
- 诊断和 season JSON 继续执行递归敏感字段剥离。
- 真实凭据、真实浏览器 profile 和真实玩家数据不得进入测试、设计文档或提交。

## 测试策略

### Rust 单元与集成测试

- 注册表原子读写、版本校验、损坏恢复和 active_account_id 修复。
- AccountPaths 保持在 app_data/accounts 下，拒绝路径穿越和非法 account_id。
- 相同 role_id 去重、BattleTag 更新和不同 role_id 错配。
- 添加或重新登录失败不覆盖已有凭据。
- 两个账号的数据库、凭据、玩家目标、玩家资料和 seasons 路径完全不同。
- 所有查询和同步命令切换后读取对应数据库。
- 同步、登录、迁移、重建和删除期间拒绝切换。
- 删除非当前账号、删除当前账号、删除最后一个账号和 Windows 文件占用失败。
- 删除 journal 与迁移 journal 的崩溃恢复和幂等重试。
- 旧单账号数据库迁移成功、失败回滚和重复启动。
- 安全 DTO 和错误序列化不出现 role_id 或 token-like 字段。

### 前端测试

- 添加账号独立行和账号列表逐行渲染。
- 每个账号固定三个操作位。
- 当前账号显示禁用的“当前账号”。
- 非当前账号可以切换，切换成功触发页面重新加载。
- 重新登录错配显示添加或切换建议，不覆盖原账号。
- 退出账号对话框明确永久删除范围，取消不会调用后端。
- 操作互斥、加载、空列表、凭据过期、注册表错误和删除失败中文状态。
- 1440×900、1920×1080 和 2540×1344 布局；窄屏按钮换行与键盘焦点。

### 端到端验收

- 使用两个合成账号 fixture，在不同 SQLite 中导入同赛季不同对局。
- 切换账号后依次验证指挥台、对局、复盘、排行、队友和生涯 MMR 不串数据。
- 验证切换前发出的延迟请求不能污染切换后的页面。
- 验证错误账号重新登录不会覆盖目标账号。
- 验证删除一个账号后其他账号文件、列表和查询结果不变。
- 验证旧数据库迁入第一个确认账号且没有重复导入。
- 真实桌面验收只记录动作结果，不记录 BattleTag、role_id 或凭据值。

## 验收标准

- 可以连续添加至少两个账号，并在不打开浏览器的情况下切换。
- 每个账号有独立凭据、数据库和同步产物。
- 任意页面都不会在切换后展示前一个账号的数据。
- 相同 role_id 不会重复创建账号；BattleTag 改名会更新原账号。
- 重新登录错误账号不会覆盖所选账号。
- 退出账号删除其全部本地数据，并且不影响其他账号。
- 删除当前账号后自动选择最近使用账号；删除最后一个账号后进入空状态。
- 旧单账号数据可以安全迁移且迁移失败不丢数据。
- 前端、日志、错误和 DTO 不暴露 role_id 或 token-like 字段。
- 所有新增用户可见文本为中文，并符合 `design.md` 的视觉与可访问性规则。

## 实施边界

该功能可以在一个总计划中分为四个顺序切片：

1. 账号注册表、AccountPaths、旧数据迁移和路径测试。
2. 登录凭据按账号落盘、添加/重新登录错配处理和账号操作协调器。
3. 所有数据库与同步命令改用活动账号上下文，完成切换隔离。
4. 指挥台账号管理 UI、删除流程、前端重载和端到端验收。

每个切片都必须保持旧数据可恢复，并在进入下一切片前通过隐私检查和针对性回归。
