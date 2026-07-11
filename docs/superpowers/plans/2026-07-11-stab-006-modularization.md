# STAB-006 渐进式模块化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不改变桌面产品行为、Tauri command 或 DTO 契约的前提下，将核心大文件拆为单一职责模块，并为拆分结果补充行为级回归测试。

**Architecture:** Rust 使用稳定 façade + 内部职责模块，`lib.rs` 和 commands 保持现有公共入口。前端先抽纯函数、再抽展示组件、最后留下页面状态编排；全局 CSS 改为聚合入口导入模块化样式，不改 className 或视觉规则。

**Tech Stack:** Rust 2021、rusqlite、serde_json、React 18、TypeScript、Vite、现有 `tsx` 测试脚本。

---

### Task 1: 建立模块化回归基线

**Files:**
- Create: `apps/owstats-desktop/src-tauri/tests/stab006_contracts.rs`
- Create: `apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: 写失败的 Rust DTO 契约测试**

使用现有匿名 fixture 和临时数据库，分别调用 `list_matches`、`get_match_detail`、`get_personal_season_review`、`get_friend_analytics`，将结果序列化为 JSON，与测试中固定的字段集合和 season 隔离断言比较；不要断言 raw ID 或真实数据。

- [ ] **Step 2: 运行 Rust 测试确认失败**

Run: `cargo test --test stab006_contracts`（工作目录 `apps/owstats-desktop/src-tauri`）

Expected: FAIL，因为测试文件和 fixture helper 尚不存在。

- [ ] **Step 3: 写失败的前端行为契约测试**

在 `stab006ModuleContracts.test.ts` 对 `filtersFromParams` / `paramsFromFilters`、缓存键、`selectedMatchId`、角斗 GUID 中文兜底、固定 S21/S22/S23 同步禁用逻辑使用真实导入和合成 DTO 断言，不读取页面源码字符串。

- [ ] **Step 4: 运行前端测试确认失败**

Run: `npx tsx scripts/stab006ModuleContracts.test.ts`（工作目录 `apps/owstats-desktop`）

Expected: FAIL，因为相关纯函数尚未导出。

- [ ] **Step 5: 仅导出既有纯函数并建立 fixture helper**

将不含 React state 的 `MatchesPage` URL/filter 函数迁入 `features/matches/matchFilters.ts` 并导出；为 Rust 测试添加仅使用临时数据库的构造 helper。不得改 DTO、SQL、路由或 UI。

- [ ] **Step 6: 验证并提交**

Run: `cargo test --test stab006_contracts; npm test`

Expected: PASS。

Commit:

```text
test: establish STAB-006 behavior contracts
```

### Task 2: 模块化 CSS 且保持视觉选择器

**Files:**
- Create: `apps/owstats-desktop/src/styles/tokens.css`
- Create: `apps/owstats-desktop/src/styles/base.css`
- Create: `apps/owstats-desktop/src/styles/shell.css`
- Create: `apps/owstats-desktop/src/styles/shared.css`
- Create: `apps/owstats-desktop/src/styles/features/matches.css`
- Create: `apps/owstats-desktop/src/styles/features/command-center.css`
- Create: `apps/owstats-desktop/src/styles/features/review.css`
- Create: `apps/owstats-desktop/src/styles/features/ranking.css`
- Create: `apps/owstats-desktop/src/styles/features/friends.css`
- Modify: `apps/owstats-desktop/src/styles.css`
- Test: `apps/owstats-desktop/scripts/stab006ModuleContracts.test.ts`

- [ ] **Step 1: 添加失败的 CSS import 与选择器完整性断言**

断言聚合 `styles.css` 只导入 tokens、base、shell、shared 和五个 feature 文件；读取模块化 CSS 合并文本后断言 `.phase6ProductShell`、`.phase6MatchWorkbench`、`.phase6CommandCenter`、`.phase6RankingTable`、`.phase6Friend`、`.phase6AchievementWall` 和 `.phase7ArenaSelector` 各出现一次。

- [ ] **Step 2: 运行测试确认失败**

Run: `npx tsx scripts/stab006ModuleContracts.test.ts`

Expected: FAIL，因为模块 CSS 尚未存在。

- [ ] **Step 3: 机械移动原有规则**

按选择器职责移动，不改变 declaration、媒体查询顺序、CSS variable 或 className。`styles.css` 仅保留 `@import`，导入顺序为 tokens、base、shell、shared、matches、command-center、review、ranking、friends。

- [ ] **Step 4: 验证构建与 CSS 契约**

Run: `npm run build; npx tsx scripts/stab006ModuleContracts.test.ts`

Expected: PASS，且 Vite 产物无 CSS import 错误。

- [ ] **Step 5: 提交**

```text
refactor: split frontend styles by feature
```

### Task 3: 拆分 Rust queries

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/queries/mod.rs`
- Create: `apps/owstats-desktop/src-tauri/src/queries/match_list.rs`
- Create: `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`
- Create: `apps/owstats-desktop/src-tauri/src/queries/arena_detail.rs`
- Create: `apps/owstats-desktop/src-tauri/src/queries/lookups.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Delete after migration: `apps/owstats-desktop/src-tauri/src/queries.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/stab006_contracts.rs`, existing query tests

- [ ] **Step 1: 写失败的角斗隐私 fallback 测试**

向临时数据库导入合成 arena detail，断言缺失 display name 时返回中文稳定兜底而非 GUID，且 S21/S22 的详情和筛选不混读。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --test stab006_contracts arena_fallback_is_private_and_season_scoped`

Expected: FAIL，直到该契约被加入测试。

- [ ] **Step 3: 以 façade 保留公共函数**

`queries/mod.rs` re-export `list_matches`、`list_filter_options`、`search_match_players`、`get_match_detail`；只移动函数和其最小依赖：列表 SQL 到 `match_list`，普通详情到 `normal_detail`，arena JSON/round/player 解析到 `arena_detail`，字典和 filter helpers 到 `lookups`。

- [ ] **Step 4: 验证**

Run: `cargo fmt --check; cargo test --test stab006_contracts; cargo test --test import_baseline; cargo test --tests`

Expected: PASS，`lib.rs` 公开 re-export 不变。

- [ ] **Step 5: 提交**

```text
refactor: split match query responsibilities
```

### Task 4: 拆分 review 与 analytics

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/review/{mod,personal,roles,command_center}.rs`
- Create: `apps/owstats-desktop/src-tauri/src/analytics/{mod,repository,metrics,boards}.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Delete after migration: `apps/owstats-desktop/src-tauri/src/review.rs`, `apps/owstats-desktop/src-tauri/src/analytics.rs`
- Test: `stab006_contracts.rs`, `phase8_command_center.rs`, friend analytics tests

- [ ] **Step 1: 写失败的 season/DTO 契约测试**

测试 `get_personal_season_review` 的固定 season 选择、`get_onboarding_status` 的三赛季状态顺序，以及 friend board 的最小样本过滤和中文显示兜底。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --test stab006_contracts review_and_friend_contracts`

Expected: FAIL，直到契约测试加入。

- [ ] **Step 3: 移动 review 计算域**

`review/mod.rs` 仅暴露两个既有 public 函数；角色段位进入 `roles.rs`，个人战报/证据进入 `personal.rs`，指挥台赛季健康进入 `command_center.rs`。

- [ ] **Step 4: 移动 analytics 计算域**

`analytics/repository.rs` 只包含 SQL rows 和 filter，`metrics.rs` 只聚合/计算亲密度，`boards.rs` 只生成榜单和阵容 DTO；不改排序规则或 sample 常量。

- [ ] **Step 5: 验证并分两次提交**

Run: `cargo fmt --check; cargo test --test stab006_contracts; cargo test --test phase8_command_center; cargo test --tests`

Expected: PASS。

Commits:

```text
refactor: split review responsibilities
refactor: split friend analytics responsibilities
```

### Task 5: 拆分同步边界

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/sync/{mod,client,fetch,payload,persist}.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`, `apps/owstats-desktop/src-tauri/src/commands.rs`
- Delete after migration: `apps/owstats-desktop/src-tauri/src/sync.rs`
- Test: `phase8_real_fetch.rs`, `sync_client.rs`, `stab006_contracts.rs`

- [ ] **Step 1: 写失败的 façade/安全契约测试**

对现有 `sync_season_with_*` 测试入口使用 local recording server，断言 retry/429、固定 season 请求形状、STAB-004 脱敏、STAB-005 同步锁均经唯一 façade 生效。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test --test phase8_real_fetch sync_facade_contract`

Expected: FAIL，直到新增测试实施。

- [ ] **Step 3: 分职责迁移**

`client.rs` 只保留 headers/retry/throttle/safe transport errors；`fetch.rs` 负责页面/详情调度；`payload.rs` 负责容器解析、去重和合成；`persist.rs` 只调用 STAB-004 原子脱敏写入和 importer；`mod.rs` 保留兼容 public 函数与 STAB-005 coordinator 接口。

- [ ] **Step 4: 验证并提交**

Run: `cargo fmt --check; cargo test --test phase8_real_fetch; cargo test --test sync_client; cargo test --tests`

Expected: PASS。

```text
refactor: split sync transport and persistence
```

### Task 6: 拆分 MatchesPage 与 Command Center

**Files:**
- Create: `apps/owstats-desktop/src/features/matches/{MatchesPage,CompactMatchDetail,CompactArenaDetail,matchFilters}.tsx`
- Create: `apps/owstats-desktop/src/features/command-center/{OnboardingPage,useCommandCenter,ProfilePanel,SeasonHealthPanel}.tsx`
- Modify: `apps/owstats-desktop/src/App.tsx`
- Delete after migration: `apps/owstats-desktop/src/pages/MatchesPage.tsx`, `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Test: `stab006ModuleContracts.test.ts`, existing match/command-center tests

- [ ] **Step 1: 写失败的真实组件行为测试**

新增 `vitest`、`jsdom`、`@testing-library/react` 和 `@testing-library/jest-dom` 开发依赖，以 jsdom 渲染合成 props：筛选 URL 往返、selected match 不导航离页、arena 行的 `aria-expanded`、current account 未 ready 时三个同步按钮禁用。不得读取组件源文本。

- [ ] **Step 2: 运行确认失败**

Run: `npm run test:components`

Expected: FAIL，因为组件测试入口及导出的展示组件尚未存在。

- [ ] **Step 3: 提取 Matches 页面**

先迁 `matchFilters.ts`，再迁 `CompactMatchDetail.tsx` / `CompactArenaDetail.tsx`，最后迁页面状态控制器。`App.tsx` 改 import 路径但保持 `/matches`、`/matches/:matchId` 路由不变。

- [ ] **Step 4: 提取 Command Center 页面**

`useCommandCenter.ts` 负责 invoke/refresh/sync 状态；展示组件仅接收安全 DTO 与 callbacks。`App.tsx` 保持 `/onboarding` 路由不变。

- [ ] **Step 5: 验证并分两次提交**

Run: `npm run test:components; npm test; npm run build`

Expected: PASS。

Commits:

```text
refactor: split matches page components
refactor: split command center components
```

### Task 7: STAB-006 收口

**Files:**
- Modify: `docs/maintenance.md`
- Modify: `docs/current-development-status.md`
- Modify: `docs/stabilization-progress.md`

- [ ] **Step 1: 静态检查模块边界**

Run:

```powershell
rg -n "mod queries;|mod review;|mod analytics;|mod sync;" apps/owstats-desktop/src-tauri/src/lib.rs
rg -n "pages/(MatchesPage|OnboardingPage)" apps/owstats-desktop/src
```

Expected: Rust 指向目录模块，前端仅保留明确兼容 re-export 或零旧路径。

- [ ] **Step 2: 执行全量验证**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
cd ..
npm test
npm run build
cd ..\..
git diff --check
git status --short
```

Expected: PASS；状态只含当前阶段预期文档和代码。

- [ ] **Step 3: 更新文档并提交**

记录模块边界、保留 façade、行为契约和未延后项；将 STAB-006 标记为已验证。

```text
docs: complete STAB-006 modularization
```
