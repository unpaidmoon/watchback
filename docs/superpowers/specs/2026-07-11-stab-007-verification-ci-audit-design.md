# STAB-007 统一验证脚本与 CI 审计设计

- 日期：2026-07-11
- 状态：依据已批准总设计自主确认
- 前置：STAB-003 至 STAB-006 已验证
- 范围：统一本地/CI 验证、可复现 checkout 验收与个人稳定标签准备；不制作安装版。

## 1. 审计结论

当前验证命令分散：

- 前端 `package.json` 的 `test` 是长串 `tsx` 命令，绝大部分是源码字符串断言；`check` 只调用 test/build。
- `scripts/checkTextIntegrity.mjs` 只扫描 mojibake，且扫描根不含 CI、根 scripts、生成资源输出或构建产物。
- Rust 有 `cargo fmt --check` 与大量集成测试，但没有项目级统一入口，未运行 Clippy。
- 没有 `.github/workflows`，没有 CI；仓库根没有 `scripts/verify.ps1`。
- 现有 docs/AGENTS 已列出手工命令，但未定义机器可执行的同一顺序、失败语义或 CI 环境边界。

因此“本机测试通过”不能等价于可复现验证，更不能证明绝对路径、敏感输出、资源完整性、旧库升级和异目录 checkout 都被覆盖。

## 2. 目标

建立唯一入口 `scripts/verify.ps1`，本地和 GitHub Actions 均只调用它。脚本按固定顺序失败即停止：

1. tracked 文件、源码、配置和构建输出的敏感字段/绝对路径扫描。
2. STAB-003 资源 manifest/准备目录完整性检查。
3. 前端类型检查、单元/组件测试和 build。
4. Rust fmt、Clippy（先 report，再由本阶段清零后 `-D warnings`）和集成测试。
5. STAB-005 旧数据库升级、回滚、恢复和多赛季测试。
6. `git diff --check` 与工作树范围报告。

最终增加 Windows CI、tracked-only 异目录 checkout 验收和仅在全部证据通过时创建本地个人稳定标签的程序化准备；标签创建本身需要用户另行明确授权，不能自动执行。

## 3. 安全扫描设计

### 3.1 扫描范围

扫描 Git tracked 文件和明确的生成输出：

- 源码、`docs/`、`package*.json`、Cargo 配置、workflow、根 scripts。
- 前端 `dist/`（构建后存在）。
- 资源 manifest 和 runtime config。

忽略二进制资源、`node_modules`、`target`、`.git`、真实 app-data、浏览器 profile、`docs/reference`、真实 dumps。忽略不等于允许：脚本绝不读取这些路径。

### 3.2 规则

失败规则包括：

- `D:/Codex/`、`C:\\Users\\`、`/@fs/`、仓库绝对路径和构建产物中的 `docs/reference`。
- 明文密钥赋值或 JSON key：token、customerToken/customer_token、cookie、authorization、credential、storage 的命名变体。

规则只针对键和值的危险组合；允许 `token-style` 资产文件名和脱敏测试中的明确占位符。扫描输出只显示相对文件、行号、规则名和脱敏片段，永不回显匹配值。

## 4. 脚本接口

```powershell
.\scripts\verify.ps1
  [-SkipBuild]
  [-SkipClippy]
  [-SkipCheckout]
  [-ChangedOnly]
```

- 默认执行完整本地门禁但不启动桌面窗口、不执行 `tauri build`、不访问网络和不读取真实凭据。
- `-ChangedOnly` 仅用于开发期快速扫描，CI 禁止使用。
- `-SkipCheckout` 仅允许本地诊断；CI 禁止使用。
- `-SkipBuild`/`-SkipClippy` 明确打印跳过项，退出码仍由其他步骤决定；CI 禁止。
- 异目录验证通过 `git archive HEAD` 创建 tracked-only 副本，复制锁文件但不复制 `.git`、ignored 文件、app-data 或 browser profile。

## 5. CI 设计

新增 `.github/workflows/verify.yml`：

- `pull_request` 和 `push` 触发。
- `windows-latest`，与用户本地 PowerShell/Tauri/Rust 路径最接近。
- `actions/checkout`、Node 24、stable Rust + rustfmt/clippy。
- 使用 `npm ci`（若 lockfile 存在）和 `cargo fetch`；不运行 `tauri build`，不需要浏览器登录或真实上游。
- 唯一验证命令为 `powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1`。
- CI 缓存仅缓存 npm/cargo 公开依赖，不缓存 app-data、profile 或 token。

## 6. 测试分层收口

- 保留已有源码字符串测试作为遗留兼容层。
- STAB-006 的真实组件/纯函数测试成为页面行为层。
- Rust `stab005_database_runtime`、`stab006_contracts` 与已有 importer/sync/season 测试成为数据和协议层。
- 资源、敏感信息和绝对路径由脚本级测试覆盖。
- 将前端长命令拆为命名子命令：`test:unit`、`test:components`、`test:legacy`、`test:resources`，顶层 `test` 只编排它们。

## 7. 完成与稳定标签

STAB-007 已验证要求：当前 checkout 和异目录 tracked-only checkout 都通过唯一脚本；CI 定义存在且可由同脚本运行；没有敏感/绝对路径漏报；所有跳过项为零。

仅在用户明确授权后，基于干净工作树和验证日志创建本地 annotated tag `personal-stable-YYYYMMDD`。不推送 tag，不制作安装包。
