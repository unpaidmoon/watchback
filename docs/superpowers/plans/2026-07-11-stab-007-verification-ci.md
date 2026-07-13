# STAB-007 统一验证与 CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 提供本地与 Windows CI 共用的单一验证入口，证明资源、隐私、构建、数据库恢复和 tracked-only checkout 可复现，同时不读取真实隐私数据或制作安装版。

**Architecture:** 根目录 PowerShell 脚本只编排明确、可单独运行的 Node/Rust 检查；脚本用退出码聚合结果并输出脱敏摘要。GitHub Actions 不复制命令，只安装公开依赖并调用同一脚本。

**Tech Stack:** PowerShell 7/Windows PowerShell、Node 24、tsx/Vite、Rust stable/cargo、GitHub Actions Windows runner。

---

### Task 1: 建立安全扫描器及其合成 fixture 测试

**Files:**
- Create: `scripts/verify-sensitive-paths.mjs`
- Create: `scripts/verify-sensitive-paths.test.mjs`
- Create: `scripts/fixtures/verify/{clean,absolute-path,sensitive-key,allowed-token-style}/`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: 写失败的 fixture 测试**

创建四个仅含合成文本的 fixture：clean 必须通过；`D:/Codex/example` 必须报 `absolute-path`；`{"customerToken":"synthetic-secret"}` 必须报 `sensitive-key` 且输出不含 `synthetic-secret`；`assets/token-style/icon.png` 必须通过。

- [ ] **Step 2: 运行确认失败**

Run: `node scripts/verify-sensitive-paths.test.mjs`

Expected: FAIL，因为扫描器尚不存在。

- [ ] **Step 3: 实现只读扫描器**

扫描器接收 `--root` 和 `--include-dist`，只遍历 Git tracked 文本文件与指定 `dist`；输出 `relative-path:line:rule:<redacted>`，检测到任一规则后退出 1。禁止读取 ignored credentials、app-data、browser profile、`docs/reference` 和真实 dump。

- [ ] **Step 4: 验证并提交**

Run: `node scripts/verify-sensitive-paths.test.mjs`

Expected: PASS。

```text
test: define verification scanner boundaries
```

### Task 2: 规范化前端验证子命令

**Files:**
- Modify: `apps/owstats-desktop/package.json`
- Create or modify: `apps/owstats-desktop/scripts/verifyResources.test.ts`
- Test: existing frontend scripts

- [ ] **Step 1: 写失败的 npm script 编排断言**

断言 package scripts 存在 `test:resources`、`test:unit`、`test:components`、`test:legacy`，顶层 `test` 只按该顺序调用四项，且 `pretest`/`prebuild` 调用 STAB-003 `prepare:assets`。

- [ ] **Step 2: 运行确认失败**

Run: `npx tsx scripts/verifyResources.test.ts`

Expected: FAIL，因为命名子命令尚未完整存在。

- [ ] **Step 3: 拆分现有长测试命令**

将纯函数/资源测试放入 `test:unit` 与 `test:resources`，STAB-006 真实组件测试进入 `test:components`，现有源码字符串测试进入 `test:legacy`。`test` 只编排四者与 text integrity；不得删除任何现有测试。

- [ ] **Step 4: 验证并提交**

Run: `npm test; npm run build`

Expected: PASS。

```text
build: organize frontend verification commands
```

### Task 3: 实现根验证入口

**Files:**
- Create: `scripts/verify.ps1`
- Create: `scripts/verify-script.test.ps1`
- Modify: `AGENTS.md`
- Modify: `docs/maintenance.md`

- [ ] **Step 1: 写失败的 PowerShell 脚本契约测试**

测试以临时目录运行 `verify.ps1 -ChangedOnly -SkipBuild -SkipClippy -SkipCheckout`：当前目录缺少 tracked Git 文件时返回非零；传入不支持参数返回非零；`-SkipBuild` 输出“已跳过构建”；失败步骤使脚本不继续执行后续步骤。

- [ ] **Step 2: 运行确认失败**

Run: `powershell -ExecutionPolicy Bypass -File scripts/verify-script.test.ps1`

Expected: FAIL，因为 `verify.ps1` 不存在。

- [ ] **Step 3: 实现固定步骤编排**

实现参数验证和 `$ErrorActionPreference = 'Stop'`。默认顺序：安全扫描、`npm run prepare:assets`/资源检查、`npm test`、`npm run build`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --tests`、STAB-005/006 指定测试、`git diff --check`、tracked-only checkout。每一步打印中文开始/成功/跳过摘要；错误只打印命令和退出码，不回显环境变量。

- [ ] **Step 4: 验证快速模式**

Run: `powershell -ExecutionPolicy Bypass -File scripts/verify.ps1 -ChangedOnly -SkipBuild -SkipClippy -SkipCheckout`

Expected: PASS，输出三个明确跳过项。

- [ ] **Step 5: 提交**

```text
build: add unified local verification script
```

### Task 4: 建立 tracked-only 异目录验收

**Files:**
- Modify: `scripts/verify.ps1`
- Create: `scripts/verify-checkout.test.ps1`
- Modify: `docs/maintenance.md`

- [ ] **Step 1: 写失败的 archive 验收测试**

在临时目录执行 `git archive HEAD | tar -x`，断言副本没有 `.git`、`data/.dashen_credentials.json`、browser profile、`docs/reference`，但有 `apps/owstats-desktop/package.json`、`src-tauri/Cargo.toml`、STAB-003 runtime config 和 tracked assets manifest。

- [ ] **Step 2: 运行确认失败**

Run: `powershell -ExecutionPolicy Bypass -File scripts/verify-checkout.test.ps1`

Expected: FAIL，直到 checkout helper 被实现。

- [ ] **Step 3: 实现 checkout helper**

`verify.ps1` 使用临时目录、`git archive HEAD`、`npm ci`（无 lockfile 时明确失败）、`npm test`、`npm run build`、`cargo fmt --check`、`cargo test --tests`。完成后只删除它创建的、已解析并确认位于系统 temp 下的目录。不得运行 `tauri build`、不得复制本机 app-data。

- [ ] **Step 4: 验证**

Run: `powershell -ExecutionPolicy Bypass -File scripts/verify.ps1 -SkipClippy`

Expected: 主 checkout 与 archive checkout 均 PASS；唯一跳过项为 Clippy。

- [ ] **Step 5: 提交**

```text
test: verify tracked-only checkout portability
```

### Task 5: 添加 Windows CI

**Files:**
- Create: `.github/workflows/verify.yml`
- Modify: `scripts/verify.ps1`
- Test: workflow YAML parse / local dry-run

- [ ] **Step 1: 写失败的 workflow 结构检查**

创建 `scripts/verify-workflow.test.mjs`，断言 workflow 使用 `windows-latest`、checkout、Node 24、Rust stable + rustfmt/clippy，并且唯一验证执行行为 `powershell -ExecutionPolicy Bypass -File .\\scripts\\verify.ps1`；禁止 `tauri build`、`OW_DASHEN_TOKEN`、credentials、profile、upload app-data artifact。

- [ ] **Step 2: 运行确认失败**

Run: `node scripts/verify-workflow.test.mjs`

Expected: FAIL，因为 workflow 尚不存在。

- [ ] **Step 3: 创建最小 workflow**

工作流在 `push` 和 `pull_request` 触发，安装 Node/Rust 公开工具链，使用 lockfile 的 `npm ci`，再调用 verify。缓存键只基于 package lock/Cargo lock；不缓存用户数据。

- [ ] **Step 4: 验证并提交**

Run: `node scripts/verify-workflow.test.mjs; powershell -ExecutionPolicy Bypass -File scripts/verify.ps1 -ChangedOnly -SkipBuild -SkipCheckout`

Expected: PASS。

```text
ci: verify Windows checkout with shared script
```

### Task 6: STAB-007 全量复核和稳定标签准备

**Files:**
- Modify: `docs/maintenance.md`
- Modify: `docs/current-development-status.md`
- Modify: `docs/stabilization-progress.md`
- Create: `docs/verification/STAB-007-evidence.md`

- [ ] **Step 1: 执行完整唯一入口**

Run: `powershell -ExecutionPolicy Bypass -File .\scripts\verify.ps1`

Expected: 所有步骤 PASS，无跳过项；不运行 tauri build。

- [ ] **Step 2: 检查 CI 与工作树**

Run:

```powershell
git diff --check
git status --short
git tag --list "personal-stable-*"
```

Expected: diff 检查通过；不自动创建或推送 tag。

- [ ] **Step 3: 写证据与待授权标签命令**

在 evidence 中记录命令、版本、通过结果、CI workflow 路径、异目录 checkout 结果、未读取真实隐私数据和未制作安装版。只记录待用户授权后执行的命令：

```powershell
git tag -a personal-stable-YYYYMMDD -m "个人稳定版：STAB-003 至 STAB-007 已验证"
```

- [ ] **Step 4: 提交**

```text
docs: complete STAB-007 verification gates
```
