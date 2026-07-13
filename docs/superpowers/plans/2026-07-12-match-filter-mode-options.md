# 对局模式筛选选项 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让对局页面 MAT-FILTERS 的模式下拉框固定提供六个正式模式，排除旧的 `leisure` 与 `sport`。

**Architecture:** `FilterBar` 已直接使用后端 `FilterOptions.modes` 渲染，并通过原始 `gameMode` 值提交筛选。本次仅在查询层将 `modes` 改为固定、稳定排序的正式模式常量；筛选 SQL、URL 参数和模式中文标签保持不变。

**Tech Stack:** Rust、rusqlite、Tauri DTO、既有 Rust 集成测试。

---

### Task 1: 锁定模式选项契约

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`

- [ ] **Step 1: 为已含旧模式记录的数据库写失败回归测试**

在现有 `filter_options_are_populated_from_imported_matches` 测试之后，加入：

```rust
#[test]
fn filter_options_expose_only_the_six_supported_match_modes() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for (index, game_mode) in ["leisure", "sport"].iter().enumerate() {
        conn.execute(
            "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name, hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, 22, 'preset', ?2, 'normal', 'win', 'map', '地图', 'hero', '英雄', ?3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 60, '{}', '2026-07-12T00:00:00Z')",
            rusqlite::params![format!("legacy-mode-{index}"), game_mode, index as i64],
        ).unwrap();
    }

    let options = list_filter_options(&db).unwrap();

    assert_eq!(options.modes, vec![
        "Leisure6v6", "LeisurePreset", "SportPreset", "SportOpen", "SportFight", "LeisureFight",
    ]);
}
```

- [ ] **Step 2: 运行单个测试，确认其因当前动态去重逻辑而失败**

Run: `cargo test --test import_baseline filter_options_expose_only_the_six_supported_match_modes`

Expected: FAIL；实际 `options.modes` 含有 `leisure`、`sport`，且缺少未入库的角斗模式。

### Task 2: 固定正式模式选项

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/queries/lookups.rs:3-12`

- [ ] **Step 1: 用稳定正式模式常量替换 `game_mode` 去重查询**

在 `lookups.rs` 顶部声明，并令 `FilterOptions.modes` 使用它：

```rust
const MATCH_FILTER_MODES: &[&str] = &[
    "Leisure6v6",
    "LeisurePreset",
    "SportPreset",
    "SportOpen",
    "SportFight",
    "LeisureFight",
];

// FilterOptions 初始化中：
modes: MATCH_FILTER_MODES.iter().map(|mode| (*mode).to_string()).collect(),
```

不修改 `list_matches_impl`，以继续让每个选项的原始值按现有 SQL 精确筛选。

- [ ] **Step 2: 重新运行单个测试，确认通过**

Run: `cargo test --test import_baseline filter_options_expose_only_the_six_supported_match_modes`

Expected: PASS；返回顺序为快速 6v6、快速预设、竞技预设、竞技开放、角斗竞技、角斗快速对应的原始模式值。

### Task 3: 回归验证与工作树检查

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/lookups.rs`

- [ ] **Step 1: 运行格式检查与完整 Rust 集成测试**

Run: `cargo fmt --check; cargo test --tests`

Expected: 两个命令均以退出码 0 结束。

- [ ] **Step 2: 运行前端构建，确认 DTO 契约未影响筛选器渲染**

Run: `npm run build`

Expected: 退出码 0。

- [ ] **Step 3: 检查补丁边界**

Run: `git diff --check; git status --short`

Expected: `git diff --check` 退出码 0；除既有用户改动外，仅本计划列出的两个代码文件发生新改动。此任务不暂存或提交任何文件，因为当前工作树已有用户未提交改动且用户未授权提交。
