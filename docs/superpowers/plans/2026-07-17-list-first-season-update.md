# List-First Season Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将“更新”改成只等待五个模式第一页和一次短事务的列表优先增量同步，并由可恢复后台任务继续分页、补详情、更新投影及重建版本化快照。

**Architecture:** SQLite 是运行时唯一事实来源；前台命令并发请求五个标准模式的第一页，将新列表、游标和后台任务原子提交后立即返回。后台管理器从 SQLite 领取持久化任务，分阶段继续第 2～5 页、补齐详情和派生数据，并仅在数据库版本稳定时原子替换完整赛季快照。

**Tech Stack:** Tauri 2、Rust、Tokio、Rusqlite/SQLite、Reqwest、Serde、React、TypeScript、Vitest、Tauri events

---

## 实施前提与文件边界

当前工作区包含尚未完成的登录改动，尤其与 `src-tauri/src/commands.rs`、`src-tauri/Cargo.toml` 重叠。执行本计划时先使用 `superpowers:using-git-worktrees` 创建隔离工作树，以提交 `c3062ff` 或其后续已确认基线为起点；不要暂存、覆盖或搬移原工作区中的用户改动。

新增文件各自只承担一种职责：

- `src-tauri/src/sync/state.rs`：同步游标、赛季版本、后台任务的 SQLite 读写。
- `src-tauri/src/sync/quick_list.rs`：第一页判定及前台短事务，不执行详情或快照工作。
- `src-tauri/src/background_sync.rs`：任务调度、恢复、取消和按账号/赛季串行化。
- `src-tauri/src/sync/background_worker.rs`：执行后续分页、详情和快照阶段。
- `src/shared/seasonSyncEvents.ts`：带账号与赛季作用域的事件订阅和缓存失效。

保留 `FullFetch` 现有完整抓取路径。`QuickUpdate` 返回新的 `QuickListCommitSummary`；`FullFetch` 继续返回 `SyncSummary`，由带 `kind` 判别字段的 `SeasonSyncResult` 统一承载，避免在快速路径伪造全量统计字段。

### Task 1: Schema v9 与历史库迁移

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/database/migrations.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/season_sync_state.rs`

- [ ] **Step 1: 写出 v8 升级到 v9 的失败测试**

创建测试库并复用现有 v8 schema fixture，插入同一赛季五个标准模式的若干 `matches`。断言初始化后 schema version 为 9、五张同步状态表存在、每个已有模式得到一条最新水位，未出现过的模式仍为 `empty_confirmed = 0`：

```rust
#[test]
fn upgrades_v8_with_idempotent_season_sync_state() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let db_path = dir.path().join("upgrade.sqlite3");
    create_v8_database_with_matches(&db_path)?;

    let db = owstats_desktop::db::OwDatabase::open(&db_path)?;
    db.initialize()?;
    db.initialize()?;

    let conn = rusqlite::Connection::open(&db_path)?;
    assert_eq!(schema_version(&conn)?, 9);
    assert_eq!(table_count(&conn, "season_sync_state")?, 1);
    assert_eq!(table_count(&conn, "season_sync_cursors")?, 1);
    assert_eq!(table_count(&conn, "season_sync_jobs")?, 1);
    assert_eq!(table_count(&conn, "season_sync_job_modes")?, 1);
    assert_eq!(table_count(&conn, "season_sync_job_matches")?, 1);
    assert_eq!(cursor_latest_ts(&conn, 22, "normal", "sport")?, Some(1_720_000_100));
    assert!(!cursor_empty_confirmed(&conn, 22, "fight", "SportFight")?);
    Ok(())
}
```

- [ ] **Step 2: 运行测试并确认失败原因是 schema version 仍为 8**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test season_sync_state upgrades_v8_with_idempotent_season_sync_state -- --exact`

Expected: FAIL，断言显示 schema version 为 `8`，或查询 `season_sync_state` 时提示表不存在。

- [ ] **Step 3: 实现 v9 幂等迁移**

将 `CURRENT_SCHEMA_VERSION` 改为 `9`，在迁移注册表中加入 `migrate_v9`。表定义固定为：

```sql
CREATE TABLE IF NOT EXISTS season_sync_state (
    season INTEGER PRIMARY KEY,
    season_revision INTEGER NOT NULL DEFAULT 0,
    snapshot_revision INTEGER NOT NULL DEFAULT 0,
    last_checked_at TEXT,
    background_status TEXT NOT NULL DEFAULT 'idle',
    pending_detail_count INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS season_sync_cursors (
    season INTEGER NOT NULL,
    match_kind TEXT NOT NULL,
    game_mode TEXT NOT NULL,
    request_shape TEXT NOT NULL CHECK(request_shape IN ('implicitCurrentSeason','explicitSeason')),
    latest_begin_ts INTEGER,
    empty_confirmed INTEGER NOT NULL DEFAULT 0 CHECK(empty_confirmed IN (0,1)),
    last_checked_at TEXT,
    last_full_fetch_at TEXT,
    last_stop_reason TEXT,
    PRIMARY KEY (season, match_kind, game_mode)
);
CREATE TABLE IF NOT EXISTS season_sync_jobs (
    job_id TEXT PRIMARY KEY,
    season INTEGER NOT NULL,
    target_revision INTEGER NOT NULL,
    phase TEXT NOT NULL,
    status TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,
    error_class TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_season_sync_one_active_job
ON season_sync_jobs(season)
WHERE status IN ('queued','running','retryable');
CREATE TABLE IF NOT EXISTS season_sync_job_modes (
    job_id TEXT NOT NULL REFERENCES season_sync_jobs(job_id) ON DELETE CASCADE,
    match_kind TEXT NOT NULL,
    game_mode TEXT NOT NULL,
    request_shape TEXT NOT NULL,
    next_page INTEGER NOT NULL,
    max_page INTEGER NOT NULL DEFAULT 5,
    stop_reason TEXT,
    status TEXT NOT NULL,
    PRIMARY KEY (job_id, match_kind, game_mode)
);
CREATE TABLE IF NOT EXISTS season_sync_job_matches (
    job_id TEXT NOT NULL REFERENCES season_sync_jobs(job_id) ON DELETE CASCADE,
    match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
    detail_status TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_class TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (job_id, match_id)
);
```

迁移从 `matches` 按 `(season, match_kind, game_mode)` 取 `MAX(begin_ts)`；S21/S22 写 `explicitSeason`，当前 S23 写 `implicitCurrentSeason`。把 `season_fetch.rs` 中已有的 `DASHEN_CURRENT_SEASON = 23` 提升为 `pub(crate)` 并复用它，不新增另一份赛季常量或全局赛季状态。

- [ ] **Step 4: 运行迁移测试与数据库回归测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test season_sync_state; cargo test --test match_favorites upgrades_version_seven_database_to_version_eight_idempotently; cargo test --test import_baseline database_initialize_sets_schema_version_idempotently`

Expected: 新测试全部 PASS；旧升级测试更新最终期望为 v9 后 PASS，已有收藏和比赛数据均保留。

- [ ] **Step 5: 提交迁移切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/database/migrations.rs apps/owstats-desktop/src-tauri/src/season_fetch.rs apps/owstats-desktop/src-tauri/tests/season_sync_state.rs apps/owstats-desktop/src-tauri/tests/match_favorites.rs
git commit -m "feat: add durable season sync state"
```

### Task 2: 强类型状态仓储与重启恢复

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/sync/state.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/mod.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/season_sync_state.rs`

- [ ] **Step 1: 写失败测试覆盖原子建任务、领取和恢复**

```rust
#[test]
fn season_job_store_claims_once_and_recovers_orphans() -> anyhow::Result<()> {
    let fixture = initialized_v9_database()?;
    let store = SeasonSyncStore::new(fixture.db.clone());
    let job = store.enqueue(22, 4, mode_work("normal", "sport", 2))?;

    assert_eq!(store.claim_next(22)?.unwrap().job_id, job.job_id);
    assert!(store.claim_next(22)?.is_none());
    assert_eq!(store.requeue_orphaned_running()?, 1);
    assert_eq!(store.claim_next(22)?.unwrap().job_id, job.job_id);
    Ok(())
}
```

另加测试：相同赛季存在 active job 时，`enqueue_or_merge` 合并 `target_revision` 和待处理 match，不产生第二个 active job；状态 DTO 不包含 match id、URL、凭据或 raw JSON。

- [ ] **Step 2: 运行测试并确认模块尚不存在**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test season_sync_state season_job_store_claims_once_and_recovers_orphans -- --exact`

Expected: FAIL，编译错误指出 `sync::state::SeasonSyncStore` 不存在。

- [ ] **Step 3: 实现状态类型和仓储 API**

在 `state.rs` 定义固定枚举和值对象，并用 `OwDatabase::with_connection`/`with_transaction` 的现有模式访问 SQLite：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobPhase { ContinuePages, FetchDetails, RebuildSnapshot }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus { Queued, Running, Retryable, Completed, Failed, Superseded }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeWork {
    pub match_kind: String,
    pub game_mode: String,
    pub request_shape: RequestShape,
    pub next_page: i64,
    pub max_page: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonBackgroundStatus {
    pub season: i64,
    pub phase: String,
    pub status: String,
    pub pending_details: usize,
    pub retryable: bool,
    pub updated_at: String,
}
```

实现 `load_cursors`、`commit_foreground`、`enqueue_or_merge`、`claim_next`、`save_mode_progress`、`save_detail_progress`、`complete_phase`、`mark_retryable`、`mark_superseded`、`requeue_orphaned_running` 和 `season_status`。新任务 ID 使用项目已有依赖 `Uuid::new_v4().to_string()`；所有状态转换均使用条件更新，例如 `UPDATE ... WHERE status='queued'`，用受影响行数保证只领取一次。

- [ ] **Step 4: 运行状态仓储测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test season_sync_state`

Expected: PASS，包括重复 enqueue 合并、单次领取、重启恢复和 DTO 脱敏断言。

- [ ] **Step 5: 提交仓储切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/state.rs apps/owstats-desktop/src-tauri/src/sync/mod.rs apps/owstats-desktop/src-tauri/tests/season_sync_state.rs
git commit -m "feat: add season sync job store"
```

### Task 3: 五模式第一页专用抓取器

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/fetch.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: 写精确请求账本失败测试**

在现有 mock upstream 中记录路由和 query。调用新入口后断言恰好五次列表请求、每个模式只出现 page 1，并且没有 profile、count、detail 路由：

```rust
#[tokio::test]
async fn quick_list_probe_requests_only_page_one_for_five_modes() -> anyhow::Result<()> {
    let upstream = MockDashen::start().await?;
    upstream.seed_page_one_for_all_modes();

    let result = fetch_quick_list_pages(&upstream.client(), 22, RequestShape::ExplicitSeason).await?;

    assert_eq!(result.modes.len(), 5);
    assert_eq!(upstream.ledger().list_pages(), vec![
        ("normal", "sport", 1),
        ("normal", "leisure", 1),
        ("fight", "QuickFight", 1),
        ("fight", "LeisureFight", 1),
        ("fight", "SportFight", 1),
    ]);
    assert_eq!(upstream.ledger().profile_requests(), 0);
    assert_eq!(upstream.ledger().count_requests(), 0);
    assert_eq!(upstream.ledger().detail_requests(), 0);
    Ok(())
}
```

再加一个局部失败测试：一个模式返回 500 时其他四个结果仍返回，失败值仅包含稳定错误分类 `network`，不包含 URL 或响应体。

- [ ] **Step 2: 运行测试并确认新抓取入口不存在**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase8_real_fetch quick_list_probe_requests_only_page_one_for_five_modes -- --exact`

Expected: FAIL，编译错误指出 `fetch_quick_list_pages` 未定义。

- [ ] **Step 3: 实现并发第一页抓取**

```rust
pub const QUICK_LIST_MODES: [SeasonModeKey; 5] = [
    SeasonModeKey::new("normal", "sport"),
    SeasonModeKey::new("normal", "leisure"),
    SeasonModeKey::new("fight", "QuickFight"),
    SeasonModeKey::new("fight", "LeisureFight"),
    SeasonModeKey::new("fight", "SportFight"),
];

pub async fn fetch_quick_list_pages(
    client: &DashenApiClient,
    season: i64,
    shape: RequestShape,
) -> QuickListProbe {
    let probes = QUICK_LIST_MODES.map(|mode| async move {
        client.fetch_list_page(season, mode, 1, shape).await
            .map(|page| ModeProbe::success(mode, page))
            .unwrap_or_else(|error| ModeProbe::failure(mode, classify_remote_error(&error)))
    });
    QuickListProbe::from_modes(futures::future::join_all(probes).await)
}
```

若现有依赖中没有 `futures`，使用 `tokio::join!` 明确列出五个 future，不为此新增 crate。保持现有限速、429 退避和账号级并发限制。

- [ ] **Step 4: 运行请求账本和现有真实抓取契约测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase8_real_fetch quick_list_probe; cargo test --test phase8_real_fetch request_shape`

Expected: PASS；账本中快速入口只有五个 page-1 请求，全量入口行为不变。

- [ ] **Step 5: 提交抓取器切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/season_fetch.rs apps/owstats-desktop/src-tauri/src/sync/fetch.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs
git commit -m "feat: fetch quick update page ones only"
```

### Task 4: 前台列表短事务与无变化快路径

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/sync/quick_list.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`

- [ ] **Step 1: 写失败测试锁定前台事务合同**

```rust
#[test]
fn foreground_commit_inserts_new_lists_without_details_or_snapshot() -> anyhow::Result<()> {
    let fixture = quick_commit_fixture()?;
    let snapshot_before = std::fs::read(&fixture.snapshot_path)?;
    let result = commit_quick_list_probe(&fixture.db, 22, fixture.probe_with_two_new_matches())?;

    assert_eq!(result.new_list_matches, 2);
    assert!(result.data_changed);
    assert_eq!(result.pending_details, 2);
    assert_eq!(fixture.count_matches(22)?, 2);
    assert_eq!(fixture.count_details(22)?, 0);
    assert_eq!(std::fs::read(&fixture.snapshot_path)?, snapshot_before);
    assert_eq!(fixture.active_jobs(22)?, 1);
    Ok(())
}

#[test]
fn unchanged_foreground_commit_only_advances_check_state() -> anyhow::Result<()> {
    let fixture = populated_quick_commit_fixture()?;
    let changes_before = fixture.connection().total_changes();
    let result = commit_quick_list_probe(&fixture.db, 22, fixture.unchanged_probe())?;
    assert!(!result.data_changed);
    assert_eq!(result.new_list_matches, 0);
    assert_eq!(fixture.count_active_jobs(22)?, 0);
    assert!(fixture.connection().total_changes() - changes_before <= 6);
    Ok(())
}
```

局部失败测试断言：成功模式仍提交，失败模式不推进 cursor，`partial_failure = true`，且不会出现“已是最新”的 stop reason。再覆盖一个没有历史水位的有效非空第一页：页面条目必须立即写入列表，并为第 2 页及详情创建后台任务，不能在前台扩展到第 2～5 页。

- [ ] **Step 2: 运行测试并确认提交器不存在**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test incremental_sync foreground_commit -- --nocapture`

Expected: FAIL，编译错误指出 `commit_quick_list_probe` 和 `QuickListCommitSummary` 不存在。

- [ ] **Step 3: 定义稳定返回类型并实现短事务**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuickModeResult {
    pub match_kind: String,
    pub game_mode: String,
    pub outcome: String,
    pub new_matches: usize,
    pub error_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuickListCommitSummary {
    pub season: i64,
    pub new_list_matches: usize,
    pub data_changed: bool,
    pub partial_failure: bool,
    pub pending_details: usize,
    pub background_status: String,
    pub checked_at: String,
    pub modes: Vec<QuickModeResult>,
}
```

`commit_quick_list_probe` 在单个事务中：复用 importer 的列表规范化函数幂等 upsert `matches`；只为可靠页面推进 cursor；更新 `last_checked_at`；为新 match 写 `season_sync_job_matches`；需要后续分页/详情/备用 request shape 验证时创建或合并 job；仅当插入新列表时把 `season_revision` 加一。不得调用 `sync_import_parts` 或 `rebuild_complete_season_payload`。

- [ ] **Step 4: 运行增量持久化测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test incremental_sync foreground_commit; cargo test --test incremental_sync unchanged_foreground_commit`

Expected: PASS；新列表立即可从 `matches` 查询，无详情、快照写入；无变化路径只有游标/检查时间轻量写入。

- [ ] **Step 5: 提交前台持久化切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/quick_list.rs apps/owstats-desktop/src-tauri/src/sync/mod.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
git commit -m "feat: commit quick season lists atomically"
```

### Task 5: 命令分流并在提交后立即返回

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/facade.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync_coordinator.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs`

- [ ] **Step 1: 写失败测试证明 quick 命令不进入旧全流程**

```rust
#[tokio::test]
async fn quick_command_returns_after_list_commit_and_full_fetch_stays_legacy() -> anyhow::Result<()> {
    let fixture = command_fixture().await?;
    let quick = fixture.sync(22, SyncMode::QuickUpdate).await?;
    assert!(matches!(quick, SeasonSyncResult::QuickList(_)));
    assert_eq!(fixture.upstream.ledger().detail_requests(), 0);
    assert_eq!(fixture.snapshot_builds(), 0);

    let full = fixture.sync(22, SyncMode::FullFetch).await?;
    assert!(matches!(full, SeasonSyncResult::Full(_)));
    assert!(fixture.upstream.ledger().detail_requests() > 0);
    assert_eq!(fixture.snapshot_builds(), 1);
    Ok(())
}
```

另加协调器测试：同账号同赛季 active background job 时 quick/full 都返回稳定中文忙碌错误，但只读查询不获取该 permit。再为 `season_sync_status` 与 `retry_season_background_sync` 命令写测试：状态查询只返回匿名 `SeasonBackgroundStatus`；retry 只把当前账号、指定赛季的 `retryable` job 改为 queued 并唤醒 worker。

- [ ] **Step 2: 运行命令测试并确认返回类型不匹配**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase8_real_fetch quick_command_returns_after_list_commit_and_full_fetch_stays_legacy -- --exact`

Expected: FAIL，`sync_season` 仍返回 `SyncSummary`，且 quick 路径发生详情或快照请求。

- [ ] **Step 3: 加入判别联合类型并分流 facade**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SeasonSyncResult {
    QuickList(QuickListCommitSummary),
    Full(SyncSummary),
}
```

`sync_season` 命令保持一个入口：`QuickUpdate` 调用 `fetch_quick_list_pages` + `commit_quick_list_probe`，事务提交后只唤醒后台管理器，不等待 worker；`FullFetch` 调用现有完整 facade。新增只读 `season_sync_status(season)` 和显式 `retry_season_background_sync(season)` 命令，并注册到 Tauri invoke handler。将全局 bool 协调器细化为账号目录 key + season 的 active set，同时保留账号删除/登录切换的账号级 permit。

- [ ] **Step 4: 运行命令、账号隔离和全量回归测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase8_real_fetch quick_command; cargo test --test phase9_multi_account sync; cargo test --test incremental_sync full_fetch`

Expected: PASS；快速命令不等待详情/快照，全量抓取的原有 summary 和持久化结果不变。

- [ ] **Step 5: 提交命令分流切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/sync/facade.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/sync_coordinator.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs apps/owstats-desktop/src-tauri/tests/phase9_multi_account.rs
git commit -m "feat: return season list updates before enrichment"
```

### Task 6: 可恢复后台管理器与生命周期

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/background_sync.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/background_season_sync.rs`

- [ ] **Step 1: 写失败测试覆盖唤醒、串行和重启恢复**

```rust
#[tokio::test]
async fn manager_runs_one_chain_per_account_season_and_recovers_running_job() -> anyhow::Result<()> {
    let fixture = background_fixture().await?;
    fixture.store.seed_running_job(22)?;
    let manager = BackgroundSeasonSyncManager::new(fixture.runner.clone());

    manager.recover_account(fixture.account.clone()).await?;
    manager.wake(fixture.account.clone(), 22);
    manager.wake(fixture.account.clone(), 22);
    fixture.runner.wait_for_idle().await;

    assert_eq!(fixture.runner.max_parallel_for(22), 1);
    assert_eq!(fixture.store.completed_jobs(22)?, 1);
    Ok(())
}
```

再测试删除账号：`stop_account` 返回后不再持有 DB/快照文件句柄；切换账号不取消旧账号任务，但事件带旧账号内部 key。

- [ ] **Step 2: 运行测试并确认管理器不存在**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test background_season_sync manager_runs_one_chain_per_account_season_and_recovers_running_job -- --exact`

Expected: FAIL，编译错误指出 `BackgroundSeasonSyncManager` 未定义。

- [ ] **Step 3: 实现管理器和 Tauri state 接线**

```rust
#[derive(Clone)]
pub struct BackgroundSeasonSyncManager {
    wake_tx: tokio::sync::mpsc::UnboundedSender<WakeRequest>,
    controls: Arc<tokio::sync::Mutex<HashMap<AccountSeasonKey, CancellationToken>>>,
}

impl BackgroundSeasonSyncManager {
    pub fn wake(&self, account: AccountRuntimeTarget, season: i64) {
        let _ = self.wake_tx.send(WakeRequest { account, season });
    }

    pub async fn stop_account(&self, account_key: &str) {
        let tokens = self.take_account_tokens(account_key).await;
        for token in tokens { token.cancel(); }
        self.wait_until_released(account_key).await;
    }
}
```

在 `lib.rs` setup 中 `manage` 单例并对所有账号数据库执行 `requeue_orphaned_running` 后 wake。账号删除命令必须先 `stop_account` 再删除目录；切换账号不等待其他账号 worker。若项目未依赖 `tokio-util::CancellationToken`，用 `tokio::sync::watch<bool>` 实现取消，不新增依赖。

- [ ] **Step 4: 运行后台生命周期与账号测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test background_season_sync; cargo test --test phase9_multi_account account_switch; cargo test --test phase9_multi_account remove_account`

Expected: PASS；重启遗留 job 恢复为 queued，同账号同赛季并行度为 1，删除前句柄释放。

- [ ] **Step 5: 提交后台管理器切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/background_sync.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/tests/background_season_sync.rs
git commit -m "feat: run recoverable background season jobs"
```

### Task 7: 后续分页与增量列表事件

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/sync/background_worker.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/background_sync.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/background_season_sync.rs`

- [ ] **Step 1: 写失败测试覆盖第 2～5 页、边界和失败续跑**

```rust
#[tokio::test]
async fn continuation_commits_each_new_page_and_stops_at_watermark() -> anyhow::Result<()> {
    let fixture = background_fixture().await?;
    fixture.seed_job_with_mode(22, "normal", "sport", 2, 5)?;
    fixture.upstream.page(2).with_new_matches(3);
    fixture.upstream.page(3).with_known_watermark();

    fixture.run_one_job().await?;

    assert_eq!(fixture.upstream.requested_pages(), vec![2, 3]);
    assert_eq!(fixture.count_matches(22)?, 3);
    assert_eq!(fixture.cursor_stop_reason(22, "normal", "sport")?, "watermarkReached");
    assert_eq!(fixture.events.named("season-list-updated").len(), 1);
    assert_eq!(fixture.events.first().anonymous_new_count, 3);
    Ok(())
}
```

失败测试断言 page 3 失败后 page 2 已提交数据不回滚，`next_page = 3`、job 为 `retryable`；事件 payload 不含 match id 或 player id。

- [ ] **Step 2: 运行测试并确认 worker 尚未处理分页**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test background_season_sync continuation_commits_each_new_page_and_stops_at_watermark -- --exact`

Expected: FAIL，分页请求为空或 `run_one_job` 未实现。

- [ ] **Step 3: 实现逐页短事务和防抖事件**

```rust
pub async fn continue_mode_pages(ctx: &WorkerContext, job: &ClaimedJob, mode: &ModeWork) -> Result<()> {
    for page_no in mode.next_page..=mode.max_page {
        let page = ctx.client.fetch_list_page(job.season, mode.key(), page_no, mode.request_shape).await?;
        let decision = classify_quick_page(&page, ctx.store.cursor(job.season, mode.key())?);
        let committed = ctx.store.commit_background_page(job, mode, page_no, &page, &decision)?;
        if committed.new_matches > 0 {
            ctx.events.list_updated(job.account_scope(), job.season, committed.new_matches);
        }
        if decision.is_reliable_boundary() { break; }
    }
    Ok(())
}
```

每次网络请求在事务外执行；每页独立提交。worker 在阶段开始/计数变化时发送 `season-background-progress`，完成所有阶段后发送 `season-background-completed`；列表写入发送 `season-list-updated`。事件只含 `accountScope`、`season`、`phase`、`newMatches`、`pendingDetails`、`status`，并按账号+赛季 250ms 防抖。页预算包含前台第一页，总上限为 5。

- [ ] **Step 4: 运行分页、重试和隐私测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test background_season_sync continuation; cargo test --test background_season_sync event_payloads_are_anonymous`

Expected: PASS；分页最多到 5，边界立即停止，失败续跑不重复已提交页，并且 progress/completed 事件都带正确账号和赛季作用域。

- [ ] **Step 5: 提交后台分页切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/background_worker.rs apps/owstats-desktop/src-tauri/src/sync/mod.rs apps/owstats-desktop/src-tauri/src/background_sync.rs apps/owstats-desktop/src-tauri/tests/background_season_sync.rs
git commit -m "feat: continue season list scans in background"
```

### Task 8: 详情、玩家索引与派生投影的后台补全

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync/background_worker.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/persist.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/background_season_sync.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`

- [ ] **Step 1: 写失败测试锁定只补本轮缺失详情**

```rust
#[tokio::test]
async fn detail_worker_fetches_only_job_matches_missing_valid_details() -> anyhow::Result<()> {
    let fixture = background_fixture().await?;
    fixture.seed_job_matches(22, &["new-a", "new-b", "already-complete"])?;
    fixture.seed_valid_detail("already-complete")?;

    fixture.run_detail_phase().await?;

    assert_eq!(fixture.upstream.requested_detail_aliases(), vec!["new-a", "new-b"]);
    assert_eq!(fixture.count_valid_details(22)?, 3);
    assert_eq!(fixture.count_current_player_index_rows(22)?, 3);
    assert_eq!(fixture.pending_detail_count(22)?, 0);
    Ok(())
}
```

另加覆盖测试：详情请求失败不删除列表行、不覆盖已有有效详情/索引，错误只存稳定分类；默认详情并发峰值不超过 2。

- [ ] **Step 2: 运行测试并确认详情阶段未实现**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test background_season_sync detail_worker_fetches_only_job_matches_missing_valid_details -- --exact`

Expected: FAIL，详情请求数为 0 或详情阶段缺失。

- [ ] **Step 3: 提取批量详情持久化并实现 worker**

在 `persist.rs` 将现有完整导入中的“写详情 + 更新 `match_players`/`arena_match_players` + 派生投影”提取为可复用函数：

```rust
pub fn persist_detail_batch(
    db: &OwDatabase,
    season: i64,
    details: &[SanitizedMatchDetail],
    failures: &[SanitizedDetailFailure],
) -> Result<DetailBatchSummary>;
```

worker 每次最多并发两个详情请求，每 10 条或阶段末尾执行一次短事务。每批成功后更新 `season_sync_job_matches.detail_status`、`pending_detail_count`，并发出匿名 `season-detail-updated`；失败项保持 retryable，绝不把 raw response 放入 SQLite 或事件。

- [ ] **Step 4: 运行详情、索引和幂等回归测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test background_season_sync detail_; cargo test --test incremental_sync detail; cargo test --test stab005_database_runtime`

Expected: PASS；重复运行不增加重复详情/索引，失败不会破坏旧状态，查询在 worker 运行期间可完成。

- [ ] **Step 5: 提交详情补全切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/background_worker.rs apps/owstats-desktop/src-tauri/src/sync/persist.rs apps/owstats-desktop/src-tauri/tests/background_season_sync.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
git commit -m "feat: enrich new season matches in background"
```

### Task 9: 版本化快照合并与原子替换

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync/snapshot.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/background_worker.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/background_season_sync.rs`

- [ ] **Step 1: 写失败测试覆盖无变化、版本竞态和失败保留旧文件**

```rust
#[test]
fn snapshot_replace_requires_stable_revision() -> anyhow::Result<()> {
    let fixture = snapshot_fixture()?;
    let old_bytes = std::fs::read(&fixture.snapshot_path)?;
    fixture.set_revisions(22, 7, 6)?;
    fixture.bump_revision_during_snapshot_read();

    let outcome = rebuild_snapshot_for_revision(&fixture.db, 22, 7, &fixture.snapshot_path)?;

    assert_eq!(outcome, SnapshotOutcome::StaleRevision);
    assert_eq!(std::fs::read(&fixture.snapshot_path)?, old_bytes);
    assert_eq!(fixture.snapshot_revision(22)?, 6);
    Ok(())
}
```

另加：`season_revision == snapshot_revision` 时不写临时文件；序列化/rename 失败时旧文件原样保留；多个列表/详情提交最终只执行一次稳定快照替换。

- [ ] **Step 2: 运行测试并确认旧实现没有 revision 防护**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test incremental_sync snapshot_replace_requires_stable_revision -- --exact`

Expected: FAIL，旧实现替换了快照或没有 `SnapshotOutcome::StaleRevision`。

- [ ] **Step 3: 实现一致性读与版本检查**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotOutcome { Unchanged, Replaced, StaleRevision }

pub fn rebuild_snapshot_for_revision(
    db: &OwDatabase,
    season: i64,
    target_revision: i64,
    destination: &Path,
) -> Result<SnapshotOutcome> {
    if db.snapshot_revision(season)? >= target_revision { return Ok(SnapshotOutcome::Unchanged); }
    let payload = db.read_complete_season_in_transaction(season)?;
    let temp = write_sanitized_snapshot_temp(destination, &payload)?;
    if db.season_revision(season)? != target_revision {
        std::fs::remove_file(temp)?;
        return Ok(SnapshotOutcome::StaleRevision);
    }
    atomic_replace(temp, destination)?;
    db.set_snapshot_revision_if_current(season, target_revision)?;
    Ok(SnapshotOutcome::Replaced)
}
```

worker 仅在分页和详情阶段稳定后进入快照阶段；若 revision 变旧则合并到最新 target 后重新排队，不重新抓取列表或详情。快照 payload 继续走现有递归脱敏。

- [ ] **Step 4: 运行快照、竞态和隐私测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test incremental_sync snapshot_; cargo test --test background_season_sync snapshot_; cargo test --test phase8_real_fetch redacts`

Expected: PASS；旧快照在失败/竞态中保持有效，稳定版本只替换一次，敏感字段不进入文件。

- [ ] **Step 5: 提交快照切片**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/snapshot.rs apps/owstats-desktop/src-tauri/src/sync/background_worker.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs apps/owstats-desktop/src-tauri/tests/background_season_sync.rs
git commit -m "feat: rebuild season snapshots by revision"
```

### Task 10: 前端联合类型、定向缓存失效与后台状态

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Create: `apps/owstats-desktop/src/shared/seasonSyncEvents.ts`
- Modify: `apps/owstats-desktop/src/shared/matchesPageCache.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx`
- Modify: `apps/owstats-desktop/src/shared/phase6Shell.ts`
- Test: `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`
- Test: `apps/owstats-desktop/scripts/matchesPageCache.test.ts`
- Test: `apps/owstats-desktop/scripts/accountScopedFrontendCaches.test.ts`

- [ ] **Step 1: 写失败测试锁定无变化零刷新和有变化定向刷新**

```ts
it("does not clear match cache or reload dashboard when quick data is unchanged", async () => {
  const result = quickResult({ dataChanged: false, newListMatches: 0 });
  await applyQuickListCommit(result, deps);
  expect(deps.clearSeasonMatches).not.toHaveBeenCalled();
  expect(deps.loadDashboard).not.toHaveBeenCalled();
});

it("invalidates only the active account and season when lists changed", async () => {
  const result = quickResult({ season: 22, dataChanged: true, newListMatches: 2 });
  await applyQuickListCommit(result, deps);
  expect(deps.clearSeasonMatches).toHaveBeenCalledWith("account-a", 22);
  expect(deps.loadSeasonStatus).toHaveBeenCalledWith(22);
  expect(deps.loadDashboard).not.toHaveBeenCalled();
});
```

事件测试断言：非当前账号/赛季事件被忽略；列表事件防抖后只失效对应列表；详情事件只失效对应详情；payload 即使多字段也不被写入日志。

- [ ] **Step 2: 运行前端测试并确认当前实现仍全局清缓存和 reload dashboard**

Run: `cd apps/owstats-desktop; npm test -- incrementalSeasonSync matchesPageCache accountScopedFrontendCaches`

Expected: FAIL，现有 `clearMatchesPageCache()` 无作用域，且 `runSeasonSync` 总是调用 `loadDashboard()`。

- [ ] **Step 3: 定义 TS 联合类型和作用域缓存 API**

```ts
export type QuickListCommitSummary = {
  season: number;
  newListMatches: number;
  dataChanged: boolean;
  partialFailure: boolean;
  pendingDetails: number;
  backgroundStatus: string;
  checkedAt: string;
  modes: QuickModeResult[];
};

export type SeasonSyncResult =
  | ({ kind: "quickList" } & QuickListCommitSummary)
  | ({ kind: "full" } & SyncSummary);
```

把 `syncSeason` 返回值改为 `SeasonSyncResult`，并为 `season_sync_status`、`retry_season_background_sync` 增加对应 API wrapper。给 match cache key 加 `accountScope` 和 `season`，新增 `clearMatchesPageCacheForSeason(accountScope, season)`；保留现有全清函数只供账号切换/退出使用。

- [ ] **Step 4: 实现前台完成语义和事件订阅**

`runSeasonSync` 对 `quickList`：收到返回即结束按钮 loading；`dataChanged=false` 不清缓存、不重载 dashboard；`dataChanged=true` 只清当前账号+赛季列表并轻量刷新该赛季状态。对 `full` 保留原完整刷新。

`seasonSyncEvents.ts` 订阅四个事件并校验 `accountScope`、`season`；列表事件 250ms 防抖。赛季行显示“正在继续检查后续页面”“详情处理中：N 场”“正在重建本地快照”“后台处理失败，可重试”。列表无详情时显示“详情处理中”，不隐藏该对局。

- [ ] **Step 5: 运行前端行为和文案测试**

Run: `cd apps/owstats-desktop; npm test -- incrementalSeasonSync matchesPageCache accountScopedFrontendCaches phase6Shell`

Expected: PASS；无变化路径零缓存清理/零 dashboard reload，有变化只刷新当前账号当前赛季，后台事件不会造成跨账号刷新。

- [ ] **Step 6: 提交前端切片**

```powershell
git add apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/shared/api.ts apps/owstats-desktop/src/shared/seasonSyncEvents.ts apps/owstats-desktop/src/shared/matchesPageCache.ts apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx apps/owstats-desktop/src/shared/phase6Shell.ts apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts apps/owstats-desktop/scripts/matchesPageCache.test.ts apps/owstats-desktop/scripts/accountScopedFrontendCaches.test.ts
git commit -m "feat: refresh season lists without blocking on enrichment"
```

### Task 11: 性能合同、端到端恢复与最终验证

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/background_season_sync.rs`
- Modify: `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`
- Modify: `docs/maintenance.md`

- [ ] **Step 1: 添加确定性的性能与请求预算测试**

使用约 1000 场匿名 fixture，只计最后一个 page-1 响应结束后的本地耗时；测试不依赖公网：

```rust
#[tokio::test]
async fn foreground_local_commit_stays_below_250_ms_on_s22_scale_fixture() -> anyhow::Result<()> {
    let fixture = performance_fixture_with_matches(1_000)?;
    let probe = fixture.page_one_probe_with_new_matches(20);
    let started = std::time::Instant::now();
    let summary = commit_quick_list_probe(&fixture.db, 22, probe)?;
    assert!(started.elapsed() <= std::time::Duration::from_millis(250));
    assert_eq!(summary.new_list_matches, 20);
    Ok(())
}
```

无变化端到端测试精确断言：列表请求 5、metadata 0、detail 0、match import 0、snapshot rebuild 0、cache clear 0、dashboard reload 0。该性能测试若在 CI 噪声环境中连续两次超标才失败，同时输出匿名阶段耗时，不输出对局 ID。

前端 fake-timer 集成测试从 `syncSeason` resolve 开始计时，断言 500ms 内调用当前账号、当前赛季的列表查询；无变化结果则断言 500ms 后仍未发起列表查询或 dashboard reload。该测试验证“返回后 500ms 内发起刷新”，不把渲染和 SQLite 查询耗时混入网络性能合同。

- [ ] **Step 2: 添加崩溃恢复和全量 supersede 端到端测试**

模拟在分页、详情、快照三个阶段分别退出并重建 manager，断言最终无重复数据且任务完成。另断言 full fetch 启动前拒绝 active job；full 成功后把旧 queued/retryable job 标记 `superseded`，并将 cursor、`season_revision`、`snapshot_revision` 更新到一致状态。

- [ ] **Step 3: 运行定向端到端测试**

Run: `cd apps/owstats-desktop/src-tauri; cargo test --test phase8_real_fetch foreground_; cargo test --test background_season_sync recovery_; cargo test --test background_season_sync full_fetch_supersedes_stale_jobs`

Expected: PASS；本地提交满足 250ms 合同，三种恢复点最终一致，全量路径仍可用。

- [ ] **Step 4: 更新维护文档**

在 `docs/maintenance.md` 记录：

- “更新”只保证第一页列表已提交，详情/后续页/快照是后台最终一致；
- 四种后台状态和可重试含义；
- SQLite 是查询事实来源，snapshot revision 落后不阻塞查询；
- 诊断只查看匿名计数、phase、revision 和 error class，不记录 URL、raw response、match/player id 或凭据；
- “全量抓取”用于校验第 5 页以后历史延迟数据。

- [ ] **Step 5: 执行完整后端验证**

Run: `cd apps/owstats-desktop/src-tauri; cargo fmt --check; cargo test --tests`

Expected: 全部 PASS，无 ignored/new failure；输出不含凭据、真实玩家 ID 或原始响应。

- [ ] **Step 6: 执行完整前端与发布验证**

Run: `cd apps/owstats-desktop; npm test; npm run build; npm run tauri build`

Expected: 全部命令 exit code 0；安装包不依赖 `D:\Codex\...`、`/@fs/` 或未跟踪本机文件。

- [ ] **Step 7: 检查 diff、隐私和变更范围**

Run: `git diff --check; rg -n "customerToken|authorization|raw_response|D:\\\\Codex|/@fs/" apps/owstats-desktop/src-tauri/src/background_sync.rs apps/owstats-desktop/src-tauri/src/sync apps/owstats-desktop/src/shared/seasonSyncEvents.ts; git status --short`

Expected: `git diff --check` 无输出；搜索结果只允许既有脱敏规则/字段屏蔽代码，不出现持久化或日志写入；status 只包含本计划预期文件。

- [ ] **Step 8: 提交验收与文档切片**

```powershell
git add apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs apps/owstats-desktop/src-tauri/tests/background_season_sync.rs apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts docs/maintenance.md
git commit -m "test: verify list-first season update workflow"
```

## 完成判定

只有同时满足以下条件才可宣称完成：点击“更新”后，前台网络请求严格限定为五个模式第一页；列表短事务提交后命令立即返回；无变化时不导入、不建快照、不清比赛缓存、不重载 dashboard；有新增时列表立即可查；后台可跨重启继续分页、详情和快照；同账号同赛季不并发；全量抓取仍保持完整能力；完整 Rust、前端、构建和 Tauri 发布验证全部通过。
