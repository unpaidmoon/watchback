# STAB-003 可复现资源与运行路径实施计划

> **执行要求：** 严格按任务顺序实施；每个任务先写失败测试，再做最小实现并验证。不要读取真实凭据或玩家数据，不制作安装版。

**目标：** 移除桌面应用的开发机绝对路径、`/@fs/` 和被忽略运行输入，建立可复现的前端资源准备流程及统一 Tauri `AppPaths`。

**方案：** 已跟踪最小 lookup 配置是前后端正式输入；Node 脚本从已跟踪 `assets/` 确定性准备 Vite 静态资源，前端只生成 `/assets/...` URL。Rust 桌面 commands 通过 `AppPaths` 获取 app-data 路径，并使用编译期嵌入的同一份 lookup，不再反推仓库根。

**技术栈：** Node/TypeScript、Vite 6、React 18、Tauri 2、Rust、serde_json。

---

## 任务 1：建立最小脱敏 lookup 配置

**文件：**

- 新增：`apps/owstats-desktop/runtime-config/ow-lookups.json`
- 新增：`apps/owstats-desktop/scripts/runtimeConfig.test.ts`
- 修改：`apps/owstats-desktop/package.json`

**步骤：**

1. 写测试，要求配置具有 `attrs`、`heroPerkList`、`perkList`、`heroAttrList`，能解析当前测试使用的死神强化和“单独消灭”属性，并递归拒绝 token-like 键。
2. 运行 `npx tsx scripts/runtimeConfig.test.ts`，确认因文件缺失失败。
3. 从本机参考 JSON 中仅提取运行代码实际读取的字段；生成后扫描键和值，不保留凭据、账号或请求数据。
4. 把测试接入 `npm test` 并运行通过。
5. 提交：`assets: track minimal runtime lookups`。

## 任务 2：建立确定性资源准备与完整性校验

**文件：**

- 新增：`apps/owstats-desktop/scripts/prepareAssets.mjs`
- 新增：`apps/owstats-desktop/scripts/prepareAssets.test.ts`
- 修改：`apps/owstats-desktop/package.json`
- 修改：`.gitignore`

**步骤：**

1. 写临时目录测试，覆盖正常准备、manifest 缺失文件、绝对路径、`..` 越界和 token-like 配置五类行为。
2. 确认测试在脚本不存在时失败。
3. 实现仓库相对输入定位、manifest 解析、路径规范化、递归敏感键扫描、清空后重建 `public/generated`。
4. 根据实际运行引用复制完整已跟踪 `assets/` 树及最小配置；保持 `/assets/...` 所需目录结构。
5. 将 `prepare:assets` 接入 `predev`、`prebuild` 和 `pretest`，生成目录加入忽略规则。
6. 运行资源脚本测试及两次准备，比较文件清单与哈希证明确定性。
7. 提交：`build: add deterministic asset preparation`。

## 任务 3：迁移前端稳定资源 URL

**文件：**

- 修改：`apps/owstats-desktop/src/shared/assets.ts`
- 修改：`apps/owstats-desktop/scripts/phase6Assets.test.ts`
- 修改：`apps/owstats-desktop/scripts/matchDetailEnrichment.test.ts`
- 修改：`apps/owstats-desktop/vite.config.ts`
- 新增或修改：构建产物路径扫描测试脚本

**步骤：**

1. 先把测试期望改为 `/assets/...`，并新增断言禁止 `/@fs/`、盘符绝对路径和 `docs/reference`；确认旧实现失败。
2. 改用已跟踪最小配置，统一资源根为 `/assets`，安全规范化 stadium `localPath`。
3. 移除 Vite `server.fs.allow` 的机器路径。
4. 运行 `npm test` 和 `npm run build`。
5. 扫描 `dist/`，确认无 `/@fs/`、`D:/Codex/`、仓库路径或 `docs/reference`。
6. 提交：`fix: use portable frontend asset urls`。

## 任务 4：新增并迁移 Rust AppPaths

**文件：**

- 新增：`apps/owstats-desktop/src-tauri/src/app_paths.rs`
- 修改：`apps/owstats-desktop/src-tauri/src/lib.rs`
- 修改：`apps/owstats-desktop/src-tauri/src/commands.rs`
- 修改：`apps/owstats-desktop/src-tauri/src/credentials.rs`
- 修改：`apps/owstats-desktop/src-tauri/src/player_target.rs`
- 测试：相应 Rust 单元/集成测试

**步骤：**

1. 写纯路径单元测试，证明所有运行文件位于给定 app-data 根内、文件名兼容现状，并且路径服务不接受仓库根。
2. 实现 `AppPaths::from_app_data_dir` 及从 `AppHandle` 构造入口。
3. 迁移 `commands.rs` 的数据库、设置、玩家资料、凭据和当前玩家目标路径调用。
4. 让 credentials/player-target 的 Tauri 入口委托路径服务，保留可注入 path 的测试函数。
5. 删除桌面 command 的 `project_root()` 和 `CARGO_MANIFEST_DIR` 使用。
6. 运行 `cargo fmt --check`、相关测试和 `cargo test --tests`。
7. 提交：`refactor: centralize desktop app paths`。

## 任务 5：移除桌面 lookup 的仓库根依赖

**文件：**

- 修改：`apps/owstats-desktop/src-tauri/src/importer.rs`
- 修改：`apps/owstats-desktop/src-tauri/src/settings.rs`
- 修改：`apps/owstats-desktop/src-tauri/src/commands.rs`
- 修改或新增：Rust lookup/导入测试

**步骤：**

1. 写测试证明不提供 project root 时仍能解析已知英雄/地图/属性显示名；保留 fixture 注入能力。
2. 将最小 lookup 通过 `include_str!` 作为正式默认输入，解析失败返回明确错误而不是静默空字典。
3. 把 importer/settings 的桌面入口改为正式 lookup；测试辅助入口继续允许显式 fixture lookup。
4. 确认桌面运行模块中 `CARGO_MANIFEST_DIR` 为零，仅测试和 `src/bin` 开发工具保留。
5. 运行 Rust 全量测试。
6. 提交：`fix: embed portable runtime lookups`。

## 任务 6：异目录验证与 STAB-003 收口

**文件：**

- 修改：`docs/maintenance.md`
- 修改：`docs/stabilization-progress.md`
- 必要时修改：`README.md`

**步骤：**

1. 在仓库外临时目录用 `git archive HEAD` 建立只包含 tracked 文件的副本，不复制 `.git`、真实 app-data、凭据或玩家数据。
2. 在副本安装现有锁文件依赖并执行 `npm test`、`npm run build`；执行 Rust 格式与测试。
3. 从副本启动 `npm run tauri:dev`，记录编译和窗口启动证据后正常终止；不执行 `tauri build`。
4. 在主工作区执行：

   ```powershell
   cd apps\owstats-desktop
   npm test
   npm run build
   cd src-tauri
   cargo fmt --check
   cargo test --tests
   cd ..\..\..
   git diff --check
   git status --short
   ```

5. 扫描源码、生成目录和 `dist/` 的绝对路径、`/@fs/`、`docs/reference` 与 token-like 输出。
6. 更新维护文档和进度表，将 STAB-003 标记为“已验证”，写明实际命令、结果、未执行安装版以及未接触真实隐私数据。
7. 提交：`docs: complete STAB-003 portability`。

## 完成门禁

- 所有六个提交主题可独立审查与回退。
- 主工作区与异目录 tracked-only 副本均通过规定验证。
- 桌面运行代码不存在开发机绝对路径、`/@fs/`、被忽略配置输入或仓库根推导。
- 未读取、输出、提交真实凭据或玩家隐私数据。
- 不制作安装版，不推送远端，不执行破坏性 Git 操作。
