# Watchback 程序内检查更新与更新公告实施方案

> 文档状态：客户端功能与公告服务已实施；`v0.5.2` 首次公开发布待执行  
> 编写日期：2026-08-04  
> 实施完成日期：2026-08-04  
> 适用项目：Watchback Windows 桌面端  
> 技术栈：Tauri 2、Rust、React、TypeScript、阿里云 OSS  
> GitHub 仓库：<https://github.com/fengpianpianne/watchback>

## 实施结果摘要

- 第一版包含检查更新能力的版本已确定并统一为 `0.5.2`；
- 客户端、Tauri 命令、Rust 校验模块、公告弹窗、版本读取和自动化测试均已完成；
- OSS 正式公告已更新为 `0.5.2`，公网 HTTPS 验证为 HTTP 200，当前保持 `enabled=false`；
- NSIS 安装包和 SHA-256 文件已成功生成，但尚未上传到 GitHub、百度网盘或夸克网盘；
- 首次公开发布、干净环境安装烟测和离线烟测属于发布操作，仍需在正式对外发布时执行；
- 以后推送新版本时，必须先完成三个渠道的文件上传与校验，最后才把 OSS 公告改为 `enabled=true`。

## 1. 目标

为 Watchback 增加程序内检查更新能力。用户每次启动程序时，客户端异步读取部署在阿里云 OSS 上的更新公告；远端稳定版版本号高于当前程序版本时，显示更新公告弹窗。

弹窗必须包含：

- 新版本号；
- 发布时间；
- 更新标题；
- 更新内容列表；
- GitHub Releases 下载入口；
- 百度网盘下载入口；
- 夸克网盘下载入口；
- “稍后更新”关闭操作。

本功能只负责检查、公告和打开下载页面，不自动下载、替换、安装或执行任何远端文件。

## 2. 固定发布入口

以下链接属于客户端内置的可信发布入口，不由远端公告控制：

| 渠道 | 固定入口 |
| --- | --- |
| GitHub 仓库 | <https://github.com/fengpianpianne/watchback> |
| GitHub Releases | <https://github.com/fengpianpianne/watchback/releases> |
| 百度网盘 | <https://pan.baidu.com/s/5GbzDPA8NkteOswgwKHN5uw> |
| 夸克网盘 | <https://pan.quark.cn/s/0d7bae24fca3> |

远端公告不得携带或覆盖下载地址。这样即使公告文件被错误修改，也不能把用户引导到任意下载网站。

## 3. 范围

### 3.1 本期包含

- 每次程序进程启动后检查一次更新；
- 从阿里云 OSS 固定 HTTPS 地址读取 JSON 公告；
- 按 SemVer 比较当前版本和远端版本；
- 有新版本时显示公告弹窗；
- 从 Rust 后端按固定枚举打开三个下载页面；
- 检查失败时静默跳过并记录诊断日志；
- 修复当前版本号来源不一致；
- 提供 OSS 配置、公告模板、发布步骤和回滚步骤；
- 增加 Rust、TypeScript、UI 和发布契约测试。

### 3.2 本期不包含

- 自动下载；
- 增量更新；
- 静默安装；
- 强制更新；
- 后台常驻推送；
- WebSocket、消息队列或移动推送；
- 用户分组、灰度发布或渠道版本；
- 更新包签名验证和自动执行；
- 程序运行期间的周期检查。

如以后需要让已经打开的程序也收到公告，可在不改变公告协议的前提下增加低频定时检查，例如每 6 小时一次。当前需求只在每次进入程序时执行一次。

## 4. 关键限制与首次上线策略

已经安装的旧版 Watchback 没有检查更新代码，远端公告服务无法主动唤醒它们。因此：

1. 第一版包含检查更新功能的安装包仍需通过 GitHub、百度网盘、夸克网盘和现有用户通知方式发布。
2. 第一版启动时读取公告，但远端版本号与自身相同，所以不会弹窗。
3. 从下一次发布开始，旧客户端才能通过 `latest.json` 自动发现新版本。

示例：

- `v0.5.2`：首次包含检查更新功能，需要人工通知用户升级；
- `v0.5.3`：发布完成后修改 OSS 公告，`v0.5.2` 用户下次启动即可看到弹窗。

## 5. 用户体验

### 5.1 启动流程

1. Watchback 正常创建窗口并加载主界面。
2. 前端异步调用 Tauri command，不等待检查结果才显示主界面。
3. Rust 后端请求更新公告，单次请求总超时 5 秒。
4. 后端校验响应和版本。
5. 有新版本时，前端在当前页面上方显示模态弹窗。
6. 没有新版本、公告关闭或检查失败时，不显示任何提示。

更新服务不可用不得影响登录、同步、查询、本地数据或主路由初始化。

### 5.2 弹窗内容

建议布局：

```text
┌──────────────────────────────────────────┐
│ Watchback v0.5.3 已发布               × │
│                                          │
│ 当前版本 v0.5.2 · 发布于 2026-08-10      │
│                                          │
│ 更新内容                                 │
│ • 新增……                                 │
│ • 优化……                                 │
│ • 修复……                                 │
│                                          │
│ [GitHub] [百度网盘] [夸克网盘]           │
│                              [稍后更新]  │
└──────────────────────────────────────────┘
```

### 5.3 交互规则

- 远端版本高于当前版本：显示弹窗；
- 远端版本等于或低于当前版本：不显示；
- `enabled` 为 `false`：不显示；
- 用户点击“稍后更新”、关闭按钮、遮罩或按 Escape：关闭本次弹窗；
- 不提供“不再提醒”，用户未升级时下次启动仍显示；
- 点击下载入口后使用系统默认浏览器打开页面，弹窗保持打开，方便用户改选其他渠道；
- 打开链接失败时在弹窗内显示简短错误，不关闭弹窗；
- 公告内容只按纯文本渲染，不解析 HTML 或 Markdown；
- 键盘焦点进入弹窗后不得落到背景页面；关闭后恢复到之前的焦点；
- 窄窗口下三个下载按钮允许换行，但不得超出可视区域。

## 6. 总体设计

本功能建立一个“更新检查模块”。它的 Interface 保持很小，复杂性集中在 Implementation 内部。

### 6.1 模块 Interface

Rust 更新检查模块对 Tauri seam 提供：

```rust
impl UpdateChecker {
    pub async fn check_once(&self) -> UpdateCheckResult;
    pub fn current_version(&self) -> &str;
}

pub enum UpdateDownloadSource {
    Github,
    Baidu,
    Quark,
}

pub fn download_url(source: UpdateDownloadSource) -> &'static str;
```

调用者不需要知道 HTTP、超时、响应体限制、JSON 校验、SemVer、日志或进程内去重的细节。删除这个模块后，这些复杂性会重新散落到命令、前端和测试中，因此该模块具备足够 Depth。

### 6.2 Tauri command seam

对前端只暴露三个命令：

```text
check_app_update() -> UpdateCheckResult
get_app_version() -> String
open_update_download(source) -> void
```

`open_update_download` 只接收 `github`、`baidu`、`quark` 三个枚举值，不接受 URL 字符串。

### 6.3 前端职责

前端负责：

- 程序启动后调用一次检查命令；
- 根据返回状态决定是否显示弹窗；
- 渲染经过后端校验的纯文本公告；
- 把下载来源枚举传给后端；
- 管理弹窗焦点、关闭动作和打开链接错误。

前端不负责：

- 直接请求互联网；
- 比较版本号；
- 决定远端 URL；
- 解析不受信任的 HTML；
- 拼接下载地址。

### 6.4 数据流

```text
App 启动
  → invoke("check_app_update")
    → UpdateChecker.check_once()
      → HTTPS GET latest.json
      → 大小、状态码、字段、SemVer 校验
      → 生成 UpdateCheckResult
  → updateAvailable 时渲染 UpdateAnnouncementDialog
  → 用户点击下载来源
    → invoke("open_update_download", { source })
      → 后端枚举映射固定 URL
      → 系统默认浏览器
```

## 7. 更新公告协议

### 7.1 服务端地址

正式地址已经部署并通过公网验证：

```text
https://watchback-updates-cn-202608.oss-cn-chengdu.aliyuncs.com/latest.json
```

Rust 代码中的生产地址必须是上述固定 HTTPS 常量。不得从用户设置、网页参数或运行时环境任意覆盖。该地址无需自有域名、备案或运行中的 ECS 服务器。

### 7.2 JSON 示例

```json
{
  "schemaVersion": 1,
  "enabled": true,
  "latestVersion": "0.5.3",
  "publishedAt": "2026-08-10",
  "title": "Watchback v0.5.3 已发布",
  "notes": [
    "新增程序内更新公告",
    "优化赛季同步稳定性",
    "修复部分对局无法正常显示的问题"
  ]
}
```

### 7.3 字段约束

| 字段 | 类型 | 约束 |
| --- | --- | --- |
| `schemaVersion` | number | 必须严格等于 `1` |
| `enabled` | boolean | `false` 时忽略其余公告并不显示弹窗 |
| `latestVersion` | string | 标准 SemVer，不带 `v` 前缀，不允许预发布标记 |
| `publishedAt` | string | `YYYY-MM-DD`，仅用于展示，不参与版本判断 |
| `title` | string | 去除首尾空白后 1～80 个字符 |
| `notes` | string[] | 1～20 项，每项去除首尾空白后 1～500 个字符 |

其他约束：

- HTTP 响应体最大 64 KiB；
- 必须返回 HTTP 200；
- JSON 顶层必须是对象；
- 未知字段可忽略，以便后续兼容扩展；
- 空更新项、超长内容、无效日期或无效版本使整份公告失效；
- 不支持 HTML、Markdown、图片、脚本和远端链接；
- 本地版本来自编译后的应用版本，不来自公告。

### 7.4 后端返回模型

建议使用可判别状态：

```ts
type UpdateCheckStatus =
  | "updateAvailable"
  | "upToDate"
  | "disabled"
  | "unavailable";

type UpdateAnnouncement = {
  latestVersion: string;
  publishedAt: string;
  title: string;
  notes: string[];
};

type UpdateCheckResult = {
  status: UpdateCheckStatus;
  currentVersion: string;
  announcement: UpdateAnnouncement | null;
};
```

只有 `updateAvailable` 允许携带非空 `announcement`。其他状态必须返回 `null`。

## 8. Rust 后端实施

### 8.1 新建文件

新建：

```text
apps/owstats-desktop/src-tauri/src/update_check.rs
```

主要内容：

- 公告 DTO；
- 返回 DTO；
- `UpdateChecker`；
- `reqwest::Client` 配置；
- 进程内 `tokio::sync::OnceCell`；
- 响应体大小限制；
- 公告字段校验；
- SemVer 比较；
- 固定下载来源枚举和 URL 映射；
- 诊断日志；
- 纯函数单元测试。

### 8.2 依赖调整

在 `apps/owstats-desktop/src-tauri/Cargo.toml` 增加：

```toml
semver = "1"
```

继续复用现有 `reqwest`、`serde`、`tokio` 和 `tauri-plugin-opener`，不增加更新器插件。

### 8.3 HTTP 客户端约束

生产 `reqwest::Client` 应满足：

- 仅允许 HTTPS 更新地址；
- 总超时 5 秒；
- 连接超时 3 秒；
- 禁止或严格限制重定向，避免跳转到非预期主机；
- User-Agent 只包含产品名和版本，例如 `Watchback/0.5.3`；
- 不携带 Cookie、账号 ID、BattleTag、数据库信息或任何登录凭据；
- 流式累计响应，超过 64 KiB 立即终止；
- 不把原始响应正文写入日志。

### 8.4 单次进程检查

`App` 运行在 React StrictMode 下，开发环境可能执行挂载、卸载、再挂载。仅靠 React `useRef` 不能保证整个进程只请求一次。

因此 `UpdateChecker` 内部使用 `tokio::sync::OnceCell<UpdateCheckResult>`：

- 同一进程第一次调用执行网络请求；
- 同时发生的调用共享同一个结果；
- 后续调用返回缓存结果；
- 重新启动程序后创建新的检查状态并重新请求。

这与“每次进入程序执行一次”一致。

### 8.5 错误处理

以下情况返回 `unavailable`，不得向前端抛出阻断性错误：

- DNS 失败；
- TLS 失败；
- 超时；
- 非 200 状态；
- 响应过大；
- JSON 无法解析；
- 协议版本不支持；
- 字段校验失败；
- 远端版本无法解析。

后端记录结构化诊断信息：

```text
domain=application_update
event=check_failed | update_available | up_to_date | disabled
currentVersion=<version>
latestVersion=<version，仅校验成功后记录>
reason=<稳定错误分类，不记录公告原文>
```

失败不得显示全局错误提示，也不得影响程序其他功能。

### 8.6 命令接入

修改：

```text
apps/owstats-desktop/src-tauri/src/lib.rs
apps/owstats-desktop/src-tauri/src/commands.rs
```

在 `lib.rs` 中：

- 声明 `mod update_check;`；
- 构造并 `.manage(UpdateChecker)`；
- 注册 `check_app_update`、`get_app_version`、`open_update_download`。

在 `commands.rs` 中：

- command 只做 Tauri seam 的参数/结果适配；
- `check_app_update` 调用受管 `UpdateChecker`；
- `get_app_version` 返回编译期版本；
- `open_update_download` 将来源枚举交给更新模块，并调用现有 opener；
- 不复制版本比较、URL 或公告校验逻辑。

## 9. React 前端实施

### 9.1 新建文件

```text
apps/owstats-desktop/src/features/update/UpdateAnnouncementDialog.tsx
apps/owstats-desktop/src/features/update/useStartupUpdateCheck.ts
apps/owstats-desktop/src/styles/features/update.css
```

### 9.2 修改文件

```text
apps/owstats-desktop/src/App.tsx
apps/owstats-desktop/src/shared/api.ts
apps/owstats-desktop/src/shared/types.ts
apps/owstats-desktop/src/styles.css
apps/owstats-desktop/src/pages/OnboardingPage.tsx
```

### 9.3 启动检查 Hook

`useStartupUpdateCheck` 的 Interface：

```ts
type StartupUpdateState = {
  announcement: UpdateAnnouncement | null;
  currentVersion: string | null;
  dismiss(): void;
};
```

Implementation：

- 在桌面后端可用时调用 `checkAppUpdate()`；
- 组件卸载后不更新状态；
- 只有 `updateAvailable` 设置公告；
- 失败时保持 `announcement = null`；
- 不创建轮询定时器；
- 不阻塞路由或 Suspense；
- 关闭后只清除当前内存状态，不写“永久忽略”设置。

`App.tsx` 在路由外、主外壳同级渲染弹窗，保证用户位于任何页面都能看到。

### 9.4 弹窗可访问性

`UpdateAnnouncementDialog` 必须具备：

- `role="dialog"`；
- `aria-modal="true"`；
- `aria-labelledby`；
- `aria-describedby`；
- 打开时聚焦“稍后更新”或第一个明确操作；
- Tab/Shift+Tab 焦点循环；
- Escape 关闭；
- 关闭后恢复之前焦点；
- 图标设置 `aria-hidden="true"`；
- 按钮具有完整中文可访问名称；
- 公告列表使用语义化 `<ul><li>`。

### 9.5 打开下载入口

`shared/api.ts` 增加：

```ts
export const checkAppUpdate = () =>
  invoke<UpdateCheckResult>("check_app_update");

export const getAppVersion = () =>
  invoke<string>("get_app_version");

export const openUpdateDownload = (source: UpdateDownloadSource) =>
  invoke<void>("open_update_download", { source });
```

前端枚举：

```ts
type UpdateDownloadSource = "github" | "baidu" | "quark";
```

不得增加 `openArbitraryUrl(url)` 之类的通用命令。

### 9.6 版本号弹窗修复

实施前 `OnboardingPage.tsx` 把 `APP_VERSION` 硬编码为 `0.5.0`，且各发布文件版本并不一致。现已完成以下收口：

- 删除硬编码 `APP_VERSION`；
- 版本号弹窗打开时通过 `getAppVersion()` 读取真实版本；
- 加载期间显示“正在读取版本”；
- 读取失败显示“版本信息不可用”，不回退到另一个硬编码值。

## 10. 样式实施

`update.css` 独立维护更新弹窗样式，避免把全局 `shared.css` 继续加深。

要求：

- `position: fixed` 覆盖整个窗口；
- z-index 高于应用内普通弹窗；
- 主体宽度建议 `min(560px, calc(100vw - 32px))`；
- 最大高度不超过视口，公告过长时内部滚动；
- 下载按钮具有明显焦点态、悬停态和禁用态；
- 打开某一链接期间只禁用对应按钮，防止重复调用；
- 深色与浅色主题均使用现有 token；
- 不依赖远端图片、字体或 CSS；
- 动画只使用 `opacity` 和 `transform`；
- 遵守 `prefers-reduced-motion`；
- 980×680 最小窗口和窄弹窗状态均可完整操作。

## 11. 版本一致性

### 11.1 版本来源

运行时版本以 Rust 编译包版本为准：

```rust
env!("CARGO_PKG_VERSION")
```

发布前必须保证以下文件版本一致：

```text
apps/owstats-desktop/package.json
apps/owstats-desktop/package-lock.json
apps/owstats-desktop/src-tauri/Cargo.toml
apps/owstats-desktop/src-tauri/Cargo.lock
apps/owstats-desktop/src-tauri/tauri.conf.json
README.md 中的当前版本说明
```

### 11.2 自动校验

新增版本一致性测试，例如：

```text
apps/owstats-desktop/scripts/versionConsistency.test.ts
```

测试读取各文件并断言版本完全一致，加入 `npm run test:resources`。从此任何遗漏都会在发布构建前失败。

### 11.3 可选版本准备脚本

后续可增加：

```text
scripts/set-watchback-version.mjs
```

维护者执行：

```powershell
node scripts/set-watchback-version.mjs 0.5.3
```

脚本只负责同步版本文件，不负责提交、推送、创建 GitHub Release 或修改 OSS 公告。脚本必须先校验输入为稳定 SemVer，并在修改后运行一致性检查。

## 12. 阿里云 OSS 公告服务

### 12.1 已部署资源

| 项目 | 当前配置 |
| --- | --- |
| Bucket | `watchback-updates-cn-202608` |
| 地域 | 中国西南 1（成都） |
| 存储类型 | 标准存储、本地冗余 |
| Bucket ACL | 私有 |
| 公告对象 | `latest.json` |
| 公告 Content-Type | `application/json` |
| 正式 URL | <https://watchback-updates-cn-202608.oss-cn-chengdu.aliyuncs.com/latest.json> |

Bucket 的“阻止公共访问”已关闭，但 Bucket ACL 仍为私有。Bucket Policy 只允许匿名用户通过 HTTPS 读取精确对象 `latest.json`，不允许列出文件、上传、覆盖或删除对象，也不允许通过 HTTP 读取。

### 12.2 当前公告状态

当前线上公告为安全关闭状态：

```json
{
  "schemaVersion": 1,
  "enabled": false,
  "latestVersion": "0.5.2",
  "publishedAt": "2026-08-04",
  "title": "Watchback v0.5.2",
  "notes": [
    "加入程序内检查更新与更新公告功能"
  ]
}
```

`0.5.2` 是首个具备检查更新能力的版本。它尚未上传到三个公开渠道，因此公告保持 `enabled=false`；首次发布 `0.5.2` 时也无需向同版本客户端弹窗。下一版本发布时，在安装包全部可下载后再把公告更新为更高版本并设置 `enabled=true`。

### 12.3 公网验证结果

2026-08-04 已从 OSS 控制台外部验证：

- HTTPS 精确对象地址返回 HTTP 200；
- `Content-Type` 为 `application/json`；
- JSON 为 UTF-8 且中文可正确解析；
- HTTP 地址返回 403；
- Bucket 根目录返回 403，不能列出对象；
- 公告不包含凭据、账号、内部路径或下载地址。

OSS 当前返回 `Content-Disposition: attachment`，浏览器直接打开时可能下载 JSON。这不影响 Rust `reqwest` 获取和解析公告。

### 12.4 成本和生命周期

OSS 使用按量付费。本对象只有数百字节，应用每次启动请求一次，主要费用来自极少量存储、请求和公网下行流量；应在阿里云费用中心设置余额或费用提醒。不要为本功能启用日志分析、传输加速、跨区域复制等额外付费能力。

Bucket 名和正式 URL 已写入客户端契约，不能随意删除或改名。若必须迁移，应先发布一个使用新地址的客户端，并给旧地址保留足够长的兼容期。

## 13. 测试方案

### 13.1 Rust 单元测试

在 `update_check.rs` 覆盖：

1. 远端版本高于当前版本，返回 `updateAvailable`；
2. 版本相等，返回 `upToDate`；
3. 远端版本更低，返回 `upToDate`；
4. `enabled=false` 返回 `disabled`；
5. `schemaVersion` 不支持；
6. `latestVersion` 非 SemVer；
7. 带 `v` 前缀被拒绝；
8. 预发布版本被拒绝；
9. 日期格式错误；
10. 标题为空或超长；
11. notes 为空、过多、单项为空或超长；
12. 未知字段不影响解析；
13. 固定下载枚举映射到三个精确 URL；
14. 任意未知下载来源无法反序列化；
15. 公告内容不进入日志字段。

### 13.2 Rust HTTP 测试

使用本地临时 HTTP 测试服务器或注入的测试端点覆盖：

- 200 合法 JSON；
- 404；
- 500；
- 超时；
- 连接中断；
- 超过 64 KiB；
- 重定向；
- 并发调用只触发一次请求。

测试不得访问真实阿里云 OSS 或真实 GitHub。

### 13.3 TypeScript 状态测试

新增：

```text
apps/owstats-desktop/scripts/updateCheckState.test.ts
```

覆盖：

- 只有 `updateAvailable` 产生弹窗状态；
- `upToDate`、`disabled`、`unavailable` 均保持关闭；
- dismiss 只影响当前进程状态；
- 卸载后异步结果不写入状态；
- 打开链接使用来源枚举，不传 URL。

### 13.4 UI 渲染测试

新增：

```text
apps/owstats-desktop/scripts/updateAnnouncementUi.test.tsx
```

覆盖：

- 标题、当前版本、远端版本、日期和全部 notes；
- 三个下载按钮及其中文名称；
- “稍后更新”和关闭按钮；
- `role="dialog"`、`aria-modal`、labelledby、describedby；
- 远端字符串按文本转义，不能生成 HTML 标签或脚本；
- 不渲染远端 URL；
- notes 较长时结构仍稳定。

把测试加入 `npm run test:components`。

### 13.5 契约测试

扩展或新增静态契约测试，断言：

- 生产更新地址是 HTTPS；
- 三个下载地址与本方案完全一致；
- 前端没有通用任意 URL opener；
- `App.tsx` 在路由外挂载更新弹窗；
- CSP 无需添加远端 connect-src；
- Tauri command 已注册；
- 版本一致性测试进入默认测试命令。

### 13.6 手工验收矩阵

| 场景 | 预期结果 |
| --- | --- |
| 离线启动 | 正常进入程序，无更新错误弹窗 |
| OSS 请求超时 | 5 秒内结束检查，程序其他功能正常 |
| 远端版本相同 | 不显示公告 |
| 远端版本更低 | 不显示公告 |
| 远端版本更高 | 每个程序进程只显示一次公告 |
| 关闭后不重启 | 不再次弹出 |
| 关闭并重启旧版 | 再次弹出 |
| 点击 GitHub | 打开固定 GitHub Releases 页面 |
| 点击百度网盘 | 打开固定百度网盘分享页 |
| 点击夸克网盘 | 打开固定夸克分享页 |
| 公告含 `<script>` 文本 | 只显示文本，不执行、不生成标签 |
| 公告 `enabled=false` | 不显示公告 |
| React 开发 StrictMode | 网络只请求一次 |

## 14. 隐私与安全

### 14.1 客户端发送的数据

更新检查只发送普通 HTTPS GET。除互联网请求天然包含的 IP、TLS/HTTP 元数据外，客户端最多通过 User-Agent 暴露：

- 产品名 Watchback；
- 当前应用版本。

严禁发送：

- BattleTag；
- 玩家 ID；
- 账号 ID；
- Cookie、token 或网易大神凭据；
- DeepSeek API Key；
- 数据库路径；
- 安装路径；
- 对局或赛季数据；
- 设备唯一标识。

README 隐私章节需要增加更新检查网络行为说明。

### 14.2 供应链防护

- 公告只能提供文本和版本，不提供下载 URL；
- 下载目标由 Rust 固定枚举决定；
- 只接受 HTTPS 公告；
- 客户端不自动下载或执行文件；
- GitHub Release 提供安装包 SHA-256；
- 百度和夸克渠道的用户应能在公告中看到核验提示；
- 阿里云账号启用多因素认证，日常操作遵循最小权限；
- `latest.json` 发布前保留历史副本；
- 不在 Git 仓库保存阿里云密码、AccessKey、Cookie 或控制台凭据。

### 14.3 日志安全

- 不记录公告正文；
- 不记录响应体；
- 不记录用户点击时间与身份；
- 只记录稳定的成功/失败分类和版本；
- 不为此功能额外启用访问日志分析或建立用户画像。

## 15. 维护者发布流程

### 15.1 发布前准备

假设从 `0.5.2` 发布 `0.5.3`：

1. 把所有项目版本号同步为 `0.5.3`；
2. 填写版本更新内容；
3. 运行版本一致性测试；
4. 运行仓库验证；
5. 构建 NSIS 安装包；
6. 计算 SHA-256；
7. 在干净环境安装并烟测。

建议验证命令：

```powershell
pwsh -File scripts\verify.ps1

cd apps\owstats-desktop
npm run tauri build
```

计算哈希：

```powershell
Get-FileHash "<安装包完整路径>" -Algorithm SHA256
```

本次 `0.5.2` 构建产物：

```text
apps/owstats-desktop/src-tauri/target/release/bundle/nsis/Watchback_0.5.2_x64-setup.exe
apps/owstats-desktop/src-tauri/target/release/bundle/nsis/Watchback_0.5.2_x64-setup.exe.sha256
SHA-256: C80483CEAEFA7BF5C4EBB39AA983DECF587CC32BC83633D857F5C91EFC821722
```

以上产物已完成构建与哈希复核，但尚未完成干净环境安装烟测，也尚未上传到公开渠道。

### 15.2 安装包命名

统一使用：

```text
Watchback_0.5.3_x64-setup.exe
Watchback_0.5.3_x64-setup.exe.sha256
```

如果 Tauri 生成的实际文件名不同，可以在发布渠道展示时保持生成名称，但三个渠道必须一致，不得出现同名不同内容的安装包。

### 15.3 上传顺序

1. 上传夸克固定分享目录；
2. 上传百度网盘固定分享目录；
3. 在 GitHub 创建 `v0.5.3` Release；
4. 上传安装包和 SHA-256；
5. 从三个公开入口分别验证文件可见、名称和大小正确；
6. 至少重新下载一个公开渠道的文件并核对 SHA-256；
7. 最后在 OSS 覆盖发布 `latest.json`。

OSS 公告是发布开关，绝不能在安装包全部可用前提前提高版本号。

### 15.4 公告发布

先在本地生成并校验：

```json
{
  "schemaVersion": 1,
  "enabled": true,
  "latestVersion": "0.5.3",
  "publishedAt": "2026-08-10",
  "title": "Watchback v0.5.3 已发布",
  "notes": [
    "第一项更新内容",
    "第二项更新内容",
    "第三项更新内容"
  ]
}
```

发布时不要在 OSS 控制台里逐字编辑 JSON。按以下步骤操作：

1. 在本地复制上一版 `latest.json` 作为备份，例如 `latest-0.5.2.json`；
2. 用文本编辑器生成新的 `latest.json`，保存为 UTF-8；
3. 确认 `latestVersion` 不带 `v`，日期为 `YYYY-MM-DD`，`notes` 是字符串数组；
4. 在本地解析一次 JSON，确认没有多余逗号或语法错误；
5. 登录阿里云控制台，进入“对象存储 OSS”→ Bucket `watchback-updates-cn-202608` →“文件列表”；
6. 上传新的 `latest.json`，同意覆盖同名对象；不要改对象名或目录；
7. 上传完成后请求正式 HTTPS URL，确认状态为 200、版本号和中文公告正确；
8. 启动一个旧版 Watchback，验证弹窗和三个下载入口；
9. 启动最新版 Watchback，确认不会提示更新到自身版本。

覆盖对象后，精确路径的 Bucket Policy 继续生效，无需重复设置公共读取权限，也无需重启服务器。若需要完全无感的切换，可后续增加带最小权限 RAM 用户的 `ossutil` 发布脚本；第一阶段使用控制台覆盖即可。

本地 JSON 校验示例：

```powershell
Get-Content -Raw -Encoding UTF8 .\latest.json | ConvertFrom-Json | ConvertTo-Json -Depth 5
```

公网验证示例：

```powershell
curl.exe --fail --show-error --silent `
  "https://watchback-updates-cn-202608.oss-cn-chengdu.aliyuncs.com/latest.json"
```

### 15.5 发布完成标准

必须同时满足：

- GitHub Release 已公开；
- 百度固定分享页包含正确安装包；
- 夸克固定分享页包含正确安装包；
- 三个渠道文件版本和 SHA-256 一致；
- `latest.json` 返回 200 且内容正确；
- 旧版启动显示一次公告；
- 新版启动不显示自身公告；
- 三个按钮打开精确固定入口；
- 离线启动仍正常。

## 16. 撤回与回滚

### 16.1 公告错误但安装包正常

修正本地文件中的 `title`、`publishedAt` 或 `notes`，校验后在 OSS 覆盖 `latest.json`。版本号保持不变。旧版用户下次启动读取修正后的公告。

### 16.2 安装包存在严重问题

立即把公告改为：

```json
{
  "schemaVersion": 1,
  "enabled": false,
  "latestVersion": "0.5.3",
  "publishedAt": "2026-08-10",
  "title": "Watchback v0.5.3 暂停发布",
  "notes": [
    "该版本已暂停下载"
  ]
}
```

然后：

1. 验证客户端不再弹窗；
2. 在三个渠道标记或移除有问题的安装包；
3. 不把公告版本降回旧版本；
4. 修复后发布更高版本，例如 `0.5.4`；
5. 恢复 `enabled=true` 并指向新版本。

已经安装有问题版本的客户端不会接受更低版本，因此回滚应通过发布更高的修复版本完成。

### 16.3 OSS 暂时不可用

客户端自动返回 `unavailable` 并继续运行，无需紧急发布客户端。OSS 恢复后，下次启动自然恢复检查。

## 17. 分阶段实施任务

### 阶段 0：确认部署信息

- [x] 确定正式公告 HTTPS URL；
- [x] 创建成都地域 OSS Bucket；
- [x] 上传初始公告并配置精确路径的 HTTPS 匿名只读策略；
- [x] 验证 HTTPS 200、HTTP 403、目录访问 403 和 UTF-8 JSON；
- [x] 确认第一版带更新检查能力的版本号为 `0.5.2`；
- [x] 将三个固定下载入口写入客户端安全契约和 README。

完成条件：公告服务、首次上线版本和固定入口契约均已确定；正式发布前仍需核验三个渠道中的实际文件。

### 阶段 1：版本来源收口

- [x] 删除 `OnboardingPage.tsx` 的硬编码版本；
- [x] 实现 `get_app_version`；
- [x] 更新版本号弹窗；
- [x] 增加版本一致性测试；
- [x] 修正 README 当前版本。

完成条件：所有版本源一致，遗漏会导致默认测试失败。

### 阶段 2：Rust 更新检查模块

- [x] 增加 `semver`；
- [x] 实现公告 DTO 和字段校验；
- [x] 实现受限 HTTPS 获取；
- [x] 实现响应体限制；
- [x] 实现进程内单次检查；
- [x] 实现状态返回和诊断日志；
- [x] 实现固定下载来源映射；
- [x] 完成 Rust 单元和 HTTP 测试。

完成条件：后端 Interface 在无前端时已能可靠给出四种状态。

### 阶段 3：Tauri 接入

- [x] 管理 `UpdateChecker` 状态；
- [x] 注册三个 command；
- [x] 增加 TypeScript 调用和类型；
- [x] 验证不需要修改 CSP；
- [x] 增加命令和 URL 安全契约测试。

完成条件：安装版可通过 IPC 检查并打开固定来源，不能打开任意 URL。

### 阶段 4：更新公告弹窗

- [x] 实现启动检查 Hook；
- [x] 实现公告弹窗；
- [x] 接入 `App.tsx` 全局位置；
- [x] 实现焦点管理和 Escape；
- [x] 实现三个下载按钮状态；
- [x] 实现响应式样式；
- [x] 完成状态与 UI 自动化测试。

完成条件：实现与自动化测试已完成；安装版手工路径在首次公开发布验收中执行。

### 阶段 5：阿里云 OSS 部署

- [x] 创建私有 Bucket 和公告对象；
- [x] 配置 `latest.json` 精确路径 HTTPS 匿名只读策略；
- [x] 发布 `enabled=false` 的初始公告；
- [x] 完成公网访问、权限边界、响应头和 UTF-8 验证；
- [x] 在本文记录资源位置，不记录账号或凭据。

完成条件：正式客户端能够稳定获取公告，离线/故障不影响启动。

### 阶段 6：发布流程与文档

- [x] 更新 README 下载、隐私和检查更新说明；
- [x] 建立发布公告模板 `deploy/watchback-update/latest.json`；
- [x] 建立 SHA-256 操作步骤并生成首个校验文件；
- [ ] 可选实现版本同步脚本；
- [ ] 完整演练一次发布和一次撤回。

完成条件：维护者只需构建、上传三个渠道并最后替换一个 JSON，即可完成推送。

### 阶段 7：首次发布验收

- [x] 运行 `scripts/verify.ps1 -SkipCheckout`（当前新增文件未提交，因此仅跳过 tracked-only 临时检出）；
- [x] 运行 `npm run tauri build`；
- [ ] 安装版烟测；
- [ ] 离线烟测；
- [x] 通过自动化测试验证同版本不弹窗；
- [x] 通过自动化测试模拟高版本弹窗；
- [x] 验证三个固定入口的代码映射和安全契约；
- [x] 恢复正式公告到 `0.5.2` 且 `enabled=false`；
- [ ] 发布第一版带检查更新能力的安装包。

完成条件：第一版稳定发布，下一版能够使用公告触达它。

## 18. 完整验收标准

客户端实现和公告服务已经完成并通过自动化验证。以下第 1～10、13～14 项已满足；第 11 项中的安装版手工烟测以及第 12 项属于首次公开发布验收，尚未执行：

1. 每个程序进程最多执行一次远端检查；
2. 检查不阻塞窗口和主功能；
3. 远端版本更高时显示完整中文公告；
4. 同版本、低版本、关闭公告和网络失败均不弹窗；
5. 用户关闭后本次不再出现，重启旧版后再次出现；
6. 三个按钮只打开本文记录的固定入口；
7. 远端公告不能注入 URL、HTML 或脚本；
8. 客户端不下载、不执行、不静默安装远端文件；
9. 检查请求不携带账号和玩家数据；
10. 当前版本号在所有发布文件和 UI 中一致；
11. Rust、前端、构建、安装版和 OSS 验证全部通过；
12. 维护者已实际演练发布、撤回和服务不可用三种流程；
13. README 已说明下载入口、更新检查网络行为和 SHA-256 核验方式；
14. 第一版上线限制已向维护者说明，不宣称旧版能够收到不存在的检查逻辑。

## 19. 已确认信息与剩余发布动作

正式更新公告 HTTPS URL 和首个客户端版本均已确定：

```text
第一版包含检查更新功能的版本号：0.5.2
```

`0.5.2` 仍需通过 GitHub、百度网盘、夸克网盘和现有通知方式人工发布。发布前完成干净环境安装烟测与离线烟测，再把安装包和 `.sha256` 文件上传到三个渠道并逐一核验。线上公告在此期间保持 `enabled=false`。从再下一版开始，已安装 `0.5.2` 的用户才能自动收到公告。

阿里云账号、密码、AccessKey、Cookie 和其他凭据不得填写到本文件或提交到 Git。
