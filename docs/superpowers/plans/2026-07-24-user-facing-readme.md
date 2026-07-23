# Watchback User-Facing README Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将根目录 README 改写为普通用户能够理解并据此完成首次使用的 Watchback 产品入口。

**Architecture:** 保持单文件 README，不引入额外站点或截图依赖。正文按产品价值、功能、系统要求、首次使用、数据边界、常见问题和开发者入口排列，并以当前仓库事实约束所有表述。

**Tech Stack:** Markdown、PowerShell、Git

---

### Task 1: 改写普通用户 README

**Files:**
- Modify: `README.md`
- Reference: `docs/superpowers/specs/2026-07-24-user-facing-readme-design.md`
- Reference: `docs/maintenance.md`
- Reference: `docs/windows-release.md`

- [ ] **Step 1: 保留已确认的 Watchback 品牌修改**

确认 `README.md` 标题和开头均使用 `Watchback`，不恢复 `OWStatsLocal`。

- [ ] **Step 2: 用产品主页结构重写正文**

正文依次写入：

```text
产品简介
功能亮点
系统要求
安装与首次使用
本地数据与隐私
常见问题
开发者入口
```

功能描述只覆盖仓库已实现能力；没有正式下载地址时使用“取得安装包后”的表述，不添加虚构链接。

- [ ] **Step 3: 检查旧流程和敏感边界**

运行：

```powershell
rg -n "OWStatsLocal|searchBnetAccount|BattleTag 输入|token 值|customerToken 值|D:\\Codex|/@fs/" README.md
```

预期：无匹配。README 可以使用通用字段名解释隐私边界，但不得包含真实凭据值、真实用户数据或开发机路径。

- [ ] **Step 4: 检查 Markdown 差异**

运行：

```powershell
git diff --check -- README.md docs/superpowers/specs/2026-07-24-user-facing-readme-design.md docs/superpowers/plans/2026-07-24-user-facing-readme.md
git diff -- README.md
git status --short
```

预期：`git diff --check` 退出码为 0；README 差异仅包含已确认的品牌修改和普通用户内容改写；工作树中的其他既有文件保持不变。

- [ ] **Step 5: 交付而不提交**

报告修改内容、实际检查结果、未运行的测试，以及未接触真实凭据、浏览器 profile、SQLite 或玩家数据。根据仓库协作说明，不暂存或提交工作树。
