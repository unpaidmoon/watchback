# STAB-007 验证证据

状态：待最终 archive 验收。

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

## 最终验收要求

在 STAB-007 提交后执行不带跳过参数的统一入口；记录其通过结果、`git diff --check` 和 `git status --short`。本阶段不执行 `tauri build`、不创建安装包、不创建或推送稳定标签。

待用户授权创建标签时使用：

```powershell
git tag -a personal-stable-YYYYMMDD -m "个人稳定版：STAB-003 至 STAB-007 已验证"
```
