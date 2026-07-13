# STAB-003 可复现资源与运行路径设计

- 日期：2026-07-11
- 状态：已批准
- 任务：STAB-003
- 基线：`codex/stabilization`
- 上位设计：`2026-07-10-personal-stable-sustainable-development-design.md`

## 1. 目标

消除桌面应用对开发机绝对路径、Vite `/@fs/` 路径和被忽略参考文件的运行依赖，使新的干净 checkout 能仅依靠已跟踪文件完成前端测试、构建和 Tauri 开发启动。同时把桌面运行数据路径统一收口到 Tauri `app_data_dir`。

本阶段不制作安装版，不改变页面产品行为，不调整同步身份模型，不读取真实凭据或玩家数据，也不借机拆分业务大文件。

## 2. 已确认问题

### 2.1 前端资源不可移植

- `src/shared/assets.ts` 使用 `/@fs/D:/Codex/.../assets` 生成图片 URL。
- `vite.config.ts` 放行当前开发机的绝对目录。
- `phase6Assets.test.ts` 把上述绝对 URL 固化为测试期望。
- `src/shared/assets.ts` 直接导入被 `.gitignore` 排除的 `docs/reference/ow_config_cache.json` 和 `docs/reference/res/query_tool.json`。

### 2.2 桌面命令依赖仓库布局

- `commands.rs` 通过 `CARGO_MANIFEST_DIR` 逐级反推仓库根目录。
- 玩家资料和赛季同步把该仓库根目录传入 importer/settings，以读取 `data/ow_config_cache.json` 等开发期文件。
- 数据库、凭据、浏览器 profile、玩家目标和部分设置虽已使用 `app_data_dir`，但路径解析分散，缺少单一边界。

### 2.3 可用输入

- `assets/` 中约 1486 个文件、80 MB 内容已经被 Git 跟踪，可作为正式构建输入。
- 三份资源 manifest 已被跟踪，可用于校验和选择复制范围。
- 本机参考 JSON 只需提取前端和 importer 实际使用的最小字段，不应整体提交。

## 3. 选定方案

采用“跟踪最小运行配置 + 固定资源准备脚本 + 稳定静态 URL + 统一 AppPaths”的方案。

不采用逐文件 Vite 静态 import：当前资源数量较大，会制造庞大且难维护的 import 图。暂不采用 Tauri Resource/Asset Protocol：本阶段目标是开发版可复现，该方案会同时增加浏览器测试、Vite 开发模式和 CSP 的复杂度，且不是消除当前阻断所必需。

## 4. 前端资源流水线

### 4.1 正式输入

新增已跟踪的最小运行配置目录，保存以下脱敏数据：

- 英雄统计属性显示所需的 `attrs` 子集。
- 技能/强化项目解析所需的 `heroPerkList`、`perkList`、`heroAttrList` 子集。

提取结果必须只包含 UI 解析实际读取的字段，不包含 token-like 字段、账号标识、请求头、cookie 或原始响应的无关部分。运行和构建不能再读取 `docs/reference/` 或被忽略的 `data/ow_config_cache.json`。

### 4.2 准备目录

新增确定性 Node 脚本，在前端 dev、test 和 build 前准备应用拥有的生成目录。脚本从仓库相对路径定位已跟踪的 `assets/` 和最小运行配置，校验 manifest 后将需要的文件复制到 Vite 可服务的静态目录。

生成目录视为构建产物，不手工维护；脚本每次从声明的输入重建，避免残留文件让本机通过而干净 checkout 失败。目录结构保持资源相对路径，例如：

```text
generated-assets/
├─ assets/
│  ├─ heroes/
│  ├─ maps/
│  ├─ perks/
│  ├─ ranks/
│  ├─ achievements/
│  └─ stadium/
└─ config/
```

### 4.3 校验规则

准备脚本至少验证：

- 根、排位和角斗 manifest 可解析。
- manifest 中会被运行代码引用的本地文件存在且位于允许的源目录内。
- 不接受绝对路径、`..` 越界或 `/@fs/` 输入。
- 最小配置可解析且不含递归 token-like 键。
- 输出路径只由规范化相对路径组成。

任一条件失败时脚本非零退出，不允许静默跳过缺失资源。

### 4.4 URL 与 Vite

`src/shared/assets.ts` 统一生成以 `/assets/` 开头的稳定 URL；manifest 内的 `localPath` 先规范化为允许的相对路径，再映射到该 URL 根。移除 `vite.config.ts` 的开发机绝对目录白名单。

测试断言改为稳定 URL 和校验行为，不再锁定任意开发机盘符。`npm run build` 后扫描 `dist/`，必须不存在 `/@fs/`、`D:/Codex/`、仓库绝对路径或 `docs/reference`。

## 5. Rust 运行路径

### 5.1 AppPaths 边界

新增小型 `AppPaths` 路径服务，由 Tauri `AppHandle` 的 `app_data_dir` 构造，集中提供：

- SQLite 数据库路径。
- 用户设置和玩家资料路径。
- 凭据、当前玩家目标和浏览器 profile 路径。
- 赛季缓存与诊断目录路径。
- 已跟踪最小 lookup 配置的编译期或正式资源入口。

Tauri commands 只能通过该服务获取桌面运行路径，不再直接拼接文件名或反推仓库根目录。

### 5.2 Lookup 输入

前端和 Rust importer 共用同一份已跟踪最小 lookup 数据，避免两套手工字典漂移。Rust 可在编译期嵌入只读最小 JSON，或通过等价的正式资源入口加载；桌面运行不得依赖仓库目录存在。

测试和显式 CLI 开发工具可以继续使用临时 project root 或 `CARGO_MANIFEST_DIR`，但必须与桌面 command 路径隔离。现有 importer 的可注入路径接口保留给 fixture 测试。

### 5.3 写入规则

本阶段触及的 JSON 设置写入统一采用“同目录临时文件 + rename/replace”的原子替换方式。敏感凭据文件的进一步格式和迁移属于 STAB-004，本阶段只收口路径，不扩大读取或输出范围。

## 6. 兼容性与错误处理

- 保持现有 Tauri command 名称、请求/响应 DTO 和页面行为不变。
- 保留现有 app-data 文件名，避免仅因路径服务引入无意义迁移；规划目录但不在本阶段擅自移动真实用户文件。
- 资源准备错误必须指出 manifest 和相对文件名，但不得输出敏感配置内容。
- UI 缺失资源继续走既有中文稳定兜底，不回退显示 GUID 或隐私 ID。

## 7. 实施切片

1. 加入最小脱敏运行配置及其内容校验测试。
2. 加入资源准备/完整性脚本并接入 npm 生命周期。
3. 将前端资源 URL 改为 `/assets/...`，移除 Vite 绝对路径放行。
4. 新增 Rust `AppPaths` 并迁移桌面 commands。
5. 将桌面 importer/settings lookup 从仓库根依赖改为正式最小配置。
6. 增加异目录 checkout 和构建产物路径扫描验证，更新维护文档与进度表。

每个切片独立测试、独立提交，不混入 STAB-004 身份和隐私持久化改造。

## 8. 验收

至少执行：

```powershell
cd apps\owstats-desktop
npm test
npm run build

cd src-tauri
cargo fmt --check
cargo test --tests

cd ..\..\..
git diff --check
```

另外必须证明：

- 全仓桌面运行代码不存在开发机绝对路径或 `/@fs/`。
- 前端不再导入 `docs/reference` 或其他被忽略的运行输入。
- `dist/` 不包含开发机绝对路径、`/@fs/` 或 `docs/reference`。
- 复制到另一个临时目录的 Git tracked checkout 可完成前端测试和构建。
- `npm run tauri:dev` 能从异目录 checkout 启动到可用窗口；若自动烟测受桌面环境限制，必须报告实际启动证据和未自动化部分。
- 不执行 `npm run tauri build`，不制作安装包。
- 验证过程不读取真实凭据或玩家隐私数据。

## 9. 回退与完成定义

每个实施提交可单独回退。生成目录可以删除并由脚本重建；最小配置是正式源码输入，删除它应使校验明确失败。

只有代码、测试、异目录验证、构建产物扫描、文档和 `docs/stabilization-progress.md` 全部一致，且工作树只包含预期文件时，STAB-003 才能标记为“已验证”。
