# Codebase Memory MCP 项目级配置设计

## 目标

将本机的 `codebase-memory-mcp` 升级到 v0.9.0 后接入 Codex，并把可索引范围限制在 `D:\Codex\OW_data\owstats-local`。完成后，Codex 可以查询本项目的代码图谱，但不能通过该 MCP 索引工作区外的路径。

## 当前状态

- v0.9.0 可执行文件位于 `C:\Users\cyb\AppData\Local\Programs\codebase-memory-mcp\codebase-memory-mcp.exe`。
- `C:\Users\cyb\.codex\config.toml` 已存在，但尚未注册 Codebase Memory MCP。
- 工具已启用 `auto_index=true`，文件数上限为 `50000`。
- 项目已有 `.cbmignore`，排除数据库、SQLite 辅助文件、日志和 `.codebase-memory/`。
- `data/.dashen_credentials.json` 已由 `.gitignore` 排除，不得读取、索引或输出其内容。

## 方案

在 Codex 的 MCP 配置中新增 `codebase-memory-mcp` 服务，命令使用上述绝对路径，并为该服务设置：

```text
CBM_ALLOWED_ROOT=D:\Codex\OW_data\owstats-local
CBM_INDEX_SUPERVISOR=0
```

v0.9.0 的 supervisor 子进程会先于 `CBM_ALLOWED_ROOT` 检查启动，而且没有把该限制传给 worker；因此必须设置 `CBM_INDEX_SUPERVISOR=0`，让请求在 MCP 主进程内先执行根目录检查。其代价是关闭索引 worker 的崩溃隔离，但不能用放宽目录边界来换取该隔离。

安装使用官方 Windows 脚本的 `--standard --skip-config` 模式，只安装经过 SHA-256 校验的二进制，避免它额外修改全局技能、Hook、说明文件或其他客户端配置。不增加项目启动脚本，也不修改应用运行时代码。

## 数据与隐私边界

- MCP 只允许解析当前项目根目录及其子目录。
- 沿用 `.gitignore` 和 `.cbmignore` 的分层排除规则。
- 不打开或检查真实凭据、浏览器 profile、token、cookie、authorization 或真实数据 dump。
- 图索引保存在 Codebase Memory MCP 的本机缓存中，不加入 Git。
- 不提交 `.codebase-memory/graph.db.zst`；当前配置仅供本机 Codex 使用。

## 配置流程

1. 备份 Codex 当前配置文件。
2. 以 `--standard --skip-config` 模式安装 v0.9.0。
3. 在保留现有内容的前提下新增 MCP 服务、`CBM_ALLOWED_ROOT` 和 `CBM_INDEX_SUPERVISOR=0`。
4. 校验 TOML 能被 Codex 解析，并检查 MCP 服务注册结果。
5. 使用 CLI 显式索引 `D:\Codex\OW_data\owstats-local`。
6. 查询项目列表、索引状态与架构摘要，确认 TypeScript、TSX 和 Rust 等主要代码被纳入图谱。
7. 通过真实 MCP STDIO 会话尝试索引一个根目录外的无敏感测试路径，确认请求被 `CBM_ALLOWED_ROOT` 拒绝。

## 错误处理与回滚

- 若配置解析失败，立即恢复备份，不启动索引。
- 若 MCP 无法启动，先验证可执行文件路径和标准输入输出模式，再检查 Codex 配置格式。
- 若索引失败，不放宽路径或隐私限制；保留错误信息中的非敏感部分用于诊断。
- 回滚只删除新增的 MCP 配置段并恢复原配置，不删除其他 MCP 服务或用户设置。

## 验收标准

- Codex 配置中存在唯一的 `codebase-memory-mcp` 服务。
- 服务进程使用经过校验的 v0.9.0 可执行文件正常启动。
- `CBM_ALLOWED_ROOT` 精确指向本项目根目录。
- `CBM_INDEX_SUPERVISOR` 固定为 `0`，保证 v0.9.0 在同一进程内先执行根目录检查。
- 当前项目索引成功，并能返回非空的项目、状态和架构查询结果。
- 根目录外索引请求被明确拒绝。
- `git diff --check` 通过；除本设计文档外，不因本任务产生项目业务文件改动。
- 全程不读取或输出真实隐私数据。
