# BattleTag 多赛季同步重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 原生 Rust/Tauri 支持本地登录凭据、一个 BattleTag 目标和任意单赛季同步。

**Architecture:** 凭据、目标和同步分离。浏览器登录只产生大神账号凭据；BattleTag 绑定产生目标 customer token；同步按显式 season 读取两者并导入已脱敏 payload。

**Tech Stack:** Tauri 2、Rust、reqwest、SQLite、React、TypeScript。

---

### Task 1: 凭据和目标后端契约

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/credentials.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/credential_store.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/player_target.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase4_credentials.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] 写失败测试：登录 token 存在时状态为可绑定；规范化 BattleTag 可保存/读取/清除且状态不泄露 token。
- [ ] 运行 `cargo test --test phase4_credentials --test phase8_real_fetch`，确认新契约失败。
- [ ] 实现本地凭据状态、目标记录和安全 DTO；删除当前浏览器身份等待分支。
- [ ] 重跑目标测试并确认通过。

### Task 2: BattleTag 绑定命令

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_client.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] 写 recording-server 失败测试：绑定请求必须使用账号凭据 POST 到 `searchBnetAccount`，并只保存响应 data 内的 customer token。
- [ ] 运行 `cargo test --test phase8_real_fetch bind_player_target`，确认命令不存在或失败。
- [ ] 实现 `bind_player_target`、`get_player_target_status`、`clear_player_target`；绑定后用 `queryCard` 验证目标可访问。
- [ ] 重跑测试，确认请求体、状态 DTO 和脱敏断言通过。

### Task 3: 任意赛季同步

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync/facade.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/sync_client.rs`

- [ ] 写失败测试：`sync_season(20)` 使用目标文件中的 customer token，且 season=20 出现在列表和详情请求中。
- [ ] 运行 `cargo test --test sync_client`，确认失败。
- [ ] 将 `sync_season` 改为目标玩家路径；保留 Python 等价分页、斗技模式、详情并发和错误容错。
- [ ] 重跑 `cargo test --test sync_client --test phase8_real_fetch`。

### Task 4: 指挥台 UI

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/features/command-center/SeasonHealthPanel.tsx`
- Test: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`

- [ ] 写失败源码契约测试：存在 BattleTag 输入/绑定、当前目标状态、正整数赛季输入，且不存在固定三赛季常量。
- [ ] 运行 `npm test -- --runInBand` 或项目既有脚本，确认失败。
- [ ] 实现中文 UI：绑定、清除目标、输入赛季、单赛季同步和已有赛季快捷入口。
- [ ] 运行前端测试与 `npm run build`。

### Task 5: 回归和安装验证

**Files:**
- Modify: `docs/superpowers/specs/main-thread-log.md`

- [ ] 运行 `cargo fmt --check`、`cargo test --tests`、`npm test`、`npm run build`、`git diff --check`。
- [ ] 从干净构建运行桌面端，手工验证“登录 -> 绑定 BattleTag -> 同步一个非 S21/S22/S23 赛季”。
- [ ] 记录验证结果，不记录凭据或真实原始响应。
