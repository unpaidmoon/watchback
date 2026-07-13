# STAB-007 验证证据

状态：已验证（2026-07-12）。

## 验证入口

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

- 根入口：`scripts/verify.ps1`。
- CI：`.github/workflows/verify.yml`（Windows runner、Node 24、Rust stable/rustfmt/clippy）。
- tracked-only：通过 `git archive HEAD` 创建临时副本；不会复制 app data、凭据、浏览器 profile 或真实玩家数据。
- 安全扫描：只读取 Git tracked 的发布面文本与显式构建产物；命中时只输出规则名与行号，不回显匹配值。

## 已完成的本地检查

- 扫描器 fixtures 与仓库扫描通过。
- 前端分层脚本契约、CI workflow 结构与 archive 内容检查通过。
- `cargo clippy --all-targets -- -D warnings` 通过。
- 快速验证入口（跳过构建、Clippy、archive）通过。
- 在 `8189d30` 上运行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` 通过：扫描、资源准备、前端测试、前端构建与产物扫描、Rust 格式、严格 Clippy、Rust 全量测试、STAB-005/006 契约、`git diff --check` 和 tracked-only archive 均通过。
- archive 中重新执行了 `npm ci`、`npm test`、`npm run build`、`cargo fmt --check` 和 `cargo test --tests`；临时目录已清理。

## 最终验收要求

本阶段未执行 `tauri build`、未创建安装包、未创建或推送稳定标签，也未读取真实账号凭据、浏览器 profile、app data 或玩家 dump。

待用户授权创建标签时使用：

```powershell
git tag -a personal-stable-YYYYMMDD -m "个人稳定版：STAB-003 至 STAB-007 已验证"
```
