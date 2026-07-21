# Stadium Match List Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在对局列表中为角斗竞技和角斗快速展示与普通模式一致的对局表现主标签和团队表现奖章。

**Architecture:** 保留现有普通模式的 `match_players` 读取路径。角斗模式从详情 JSON 的最后一个包含队友数据的有效回合构造相同的表现统计输入，评价缓存复用既有标签规则，列表奖章复用既有最高值规则；不改变生涯分训练或运行时规则。

**Tech Stack:** Rust、rusqlite、serde_json、Tauri 2 integration tests

---

### Task 1: 固化角斗列表表现合同

**Files:**
- Create: `apps/owstats-desktop/src-tauri/tests/stadium_match_performance.rs`

- [ ] **Step 1: Write the failing tests**

  构造 `SportFight` 与 `LeisureFight` 合成详情，分别断言刷新评价后存在 `evaluation`，并断言当前玩家获得击杀、伤害、治疗、抵挡和死亡奖章。详情末尾追加空回合，证明选择的是最后一个包含队友数据的有效回合。

- [ ] **Step 2: Run tests to verify RED**

  Run: `cargo test --test stadium_match_performance`

  Expected: FAIL，因为评价刷新仅查询 `bucket='preset'`，奖章仅读取普通模式 `match_players`。

### Task 2: 为角斗详情提供统一表现快照

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/queries/arena_detail.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/match_list.rs`

- [ ] **Step 1: Select the final valid arena round**

  增加只接受对象且 `teammateList` 非空的最终回合选择器，让列表数值与奖章使用同一快照。

- [ ] **Step 2: Build list awards from arena players**

  将该回合的队友转换成现有 `PlayerRow`，调用 `team_awards_from_players`；普通模式继续调用 `team_awards_from_match_players`。

- [ ] **Step 3: Run focused tests to verify GREEN**

  Run: `cargo test --test stadium_match_performance`

  Expected: 奖章断言通过，评价断言仍失败。

### Task 3: 扩展评价缓存到角斗模式

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/match_evaluation.rs`

- [ ] **Step 1: Load arena candidate rows**

  评价候选包含 `bucket='preset'` 以及 `SportFight`、`LeisureFight`、`QuickFight`，并携带可选详情 JSON。

- [ ] **Step 2: Normalize arena player statistics**

  普通模式读取 `match_players`；角斗模式读取最后一个有效回合，首位队友作为当前玩家，双方均进入团队比较。缺少有效队友时继续使用既有胜负兜底标签。

- [ ] **Step 3: Run focused tests to verify GREEN**

  Run: `cargo test --test stadium_match_performance`

  Expected: PASS。

### Task 4: 回归与交付验证

**Files:**
- Verify only

- [ ] **Step 1: Run backend checks**

  Run: `cargo fmt --check` and `cargo test --tests` in `apps/owstats-desktop/src-tauri`.

- [ ] **Step 2: Run frontend checks**

  Run: `npm test` and `npm run build` in `apps/owstats-desktop`.

- [ ] **Step 3: Check the patch**

  Run: `git diff --check` and `git status --short` from the repository root. Confirm only the intended new plan, test, and backend files are attributable to this task; preserve all pre-existing user changes.
