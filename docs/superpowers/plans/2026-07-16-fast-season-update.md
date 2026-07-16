# Fast Season Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a very fast per-season “更新” path that stops at a local data watermark, while retaining a separate “全量抓取” path that scans and refreshes the complete season.

**Architecture:** Extend the existing Tauri sync request with an explicit `quickUpdate` / `fullFetch` mode. Keep the existing account, rate-limit, incremental persistence, snapshot, and privacy boundaries, but split list acquisition into a bounded per-mode quick scanner and the existing exhaustive scanner. The command-center UI exposes full fetch beside the season selector and quick update as an independent action on each synced-season row.

**Tech Stack:** Tauri 2, Rust, Tokio/Futures, SQLite/Rusqlite, React, TypeScript, CSS, TSX script tests.

---

## File Structure

- Modify `apps/owstats-desktop/src-tauri/src/models.rs`
  - Define the public sync mode and expose quick-scan result fields.
- Modify `apps/owstats-desktop/src-tauri/src/sync/incremental.rs`
  - Build per-mode local watermarks and provide a pure quick-page classifier.
- Modify `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
  - Implement bounded sequential-per-mode, concurrent-across-modes quick list scanning.
  - Keep exhaustive list scanning and full-detail requests for full fetch.
- Modify `apps/owstats-desktop/src-tauri/src/sync/facade.rs`
  - Dispatch explicit sync modes, reject quick first sync, and share persistence/snapshot handling.
- Modify `apps/owstats-desktop/src-tauri/src/sync/persist.rs`
  - Preserve valid old details when a full refresh attempt fails.
- Modify `apps/owstats-desktop/src-tauri/src/commands.rs`
  - Pass the request mode through the existing account-scoped Tauri command.
- Modify `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`
  - Unit-test mode inventory and quick stopping decisions.
- Modify `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
  - Lock down quick/full request counts and end-to-end persistence behavior.
- Modify `apps/owstats-desktop/src/shared/types.ts`
  - Mirror the Rust request, summary, progress, and diagnostic contracts.
- Modify `apps/owstats-desktop/src/shared/api.ts`
  - Require callers to specify quick update or full fetch.
- Modify `apps/owstats-desktop/src/shared/phase6Shell.ts`
  - Format mode-specific success and range-limited notices.
- Modify `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
  - Track the running mode and invoke the requested season without changing selection.
- Modify `apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx`
  - Render “全量抓取” and row-level “更新” as separate legal controls.
- Modify `apps/owstats-desktop/src/styles/features/command-center.css`
  - Add the operation column and compact 44px update control.
- Modify `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`
  - Verify mode-aware API and completion copy.
- Modify `apps/owstats-desktop/scripts/phase6Shell.test.ts`
  - Verify row interaction structure, button labels, and design contracts.
- Modify `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`
  - Verify the command-center wiring and public protocol.

### Task 1: Add the Explicit Sync Mode Contract

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs:485-540`
- Modify: `apps/owstats-desktop/src/shared/types.ts:378-430`
- Modify: `apps/owstats-desktop/src/shared/api.ts:82-84`
- Test: `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`

- [ ] **Step 1: Write the failing frontend protocol test**

Add assertions that require a mode parameter and camel-case values:

```ts
assert.match(
  apiSource,
  /syncSeason\(season:\s*number,\s*mode:\s*SyncMode\)/,
  "syncSeason requires an explicit mode",
);
assert.match(
  apiSource,
  /request:\s*\{\s*season,\s*mode\s*\}/,
  "syncSeason forwards season and mode",
);
assert.match(typesSource, /export type SyncMode = "quickUpdate" \| "fullFetch"/);
```

- [ ] **Step 2: Run the protocol test and verify it fails**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/incrementalSeasonSync.test.ts
```

Expected: FAIL because `SyncMode` and the second `syncSeason` argument do not exist.

- [ ] **Step 3: Add the Rust request and result types**

In `models.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SyncMode {
    QuickUpdate,
    FullFetch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    pub season: i64,
    pub mode: SyncMode,
}
```

Extend `SyncPageDiagnostic` with:

```rust
pub stop_reason: Option<String>,
pub watermark_reached: bool,
```

Extend `SyncDiagnostics` with:

```rust
pub quick_scan_limited: bool,
```

Extend `SyncSummary` with:

```rust
pub mode: SyncMode,
pub quick_scan_limited: bool,
```

Update every existing constructor to supply `SyncMode::FullFetch` for legacy dump/import-only helpers and `false` for quick-scan flags.

- [ ] **Step 4: Mirror the exact TypeScript contract**

In `shared/types.ts`, add:

```ts
export type SyncMode = "quickUpdate" | "fullFetch";

export type SyncRequest = {
  season: number;
  mode: SyncMode;
};
```

Add `stopReason`, `watermarkReached`, `quickScanLimited`, and `mode` fields matching Rust camel-case serialization.

Change `shared/api.ts` to:

```ts
export function syncSeason(season: number, mode: SyncMode) {
  return invoke<SyncSummary>("sync_season", { request: { season, mode } });
}
```

- [ ] **Step 5: Run focused tests**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/incrementalSeasonSync.test.ts
cd src-tauri
cargo test --test incremental_sync
```

Expected: frontend protocol test PASS; Rust compilation may expose constructors that still need the new fields, and all such constructors must be updated before proceeding.

- [ ] **Step 6: Commit the contract**

```powershell
git add apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/shared/api.ts apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts
git commit -m "feat(sync): add explicit season sync modes"
```

### Task 2: Build Per-Mode Local Watermarks and a Pure Stop Classifier

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync/incremental.rs:1-174`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/mod.rs:85-93`
- Test: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`

- [ ] **Step 1: Write failing inventory and classifier tests**

Add tests covering normal and fight modes independently:

```rust
#[test]
fn inventory_tracks_latest_timestamp_per_upstream_mode() {
    let inventory = seeded_inventory(&[
        ("normal-old", "normal", "sport", 100),
        ("normal-new", "normal", "sport", 300),
        ("fight-new", "fight", "QuickFight", 200),
    ]);
    assert_eq!(inventory.mode_watermark("normal", "sport"), Some(300));
    assert_eq!(inventory.mode_watermark("fight", "QuickFight"), Some(200));
}

#[test]
fn known_page_at_or_before_watermark_stops_quick_scan() {
    let decision = classify_quick_page(
        &[entry("known", "SportPreset", 300)],
        &HashSet::from(["known".to_string()]),
        Some(300),
    );
    assert_eq!(decision, QuickPageDecision::WatermarkReached);
}

#[test]
fn page_with_unknown_id_continues_even_when_timestamp_is_old() {
    let decision = classify_quick_page(
        &[entry("late-backfill", "SportPreset", 100)],
        &HashSet::new(),
        Some(300),
    );
    assert_eq!(decision, QuickPageDecision::Continue);
}

#[test]
fn invalid_nonempty_page_cannot_claim_watermark() {
    let decision = classify_quick_page(
        &[json!({"beginTs": 300})],
        &HashSet::new(),
        Some(300),
    );
    assert_eq!(decision, QuickPageDecision::Unsafe);
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test incremental_sync inventory_tracks_latest_timestamp_per_upstream_mode
cargo test --test incremental_sync known_page_at_or_before_watermark_stops_quick_scan
```

Expected: FAIL because mode watermarks and `QuickPageDecision` are undefined.

- [ ] **Step 3: Extend the local inventory**

Add focused types:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SeasonModeKey {
    pub match_kind: String,
    pub game_mode: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModeSyncInventory {
    pub match_ids: HashSet<String>,
    pub detail_ids: HashSet<String>,
    pub latest_begin_ts: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonSyncInventory {
    pub match_ids: HashSet<String>,
    pub detail_ids: HashSet<String>,
    pub modes: HashMap<SeasonModeKey, ModeSyncInventory>,
}
```

Change the inventory query to select `match_kind`, `game_mode`, and `begin_ts`. Populate a mode only from rows with nonempty mode identity and positive `begin_ts`.

- [ ] **Step 4: Implement the pure page classifier**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickPageDecision {
    Empty,
    WatermarkReached,
    Continue,
    Unsafe,
}

pub fn classify_quick_page(
    raw_entries: &[Value],
    known_match_ids: &HashSet<String>,
    watermark: Option<i64>,
) -> QuickPageDecision {
    if raw_entries.is_empty() {
        return QuickPageDecision::Empty;
    }
    let Some(watermark) = watermark else {
        return QuickPageDecision::Unsafe;
    };
    let mut saw_valid = false;
    for entry in raw_entries {
        let Some(match_id) = entry.get("matchId").and_then(Value::as_str).filter(|id| !id.is_empty()) else {
            return QuickPageDecision::Unsafe;
        };
        let Some(begin_ts) = entry.get("beginTs").and_then(Value::as_i64).filter(|ts| *ts > 0) else {
            return QuickPageDecision::Unsafe;
        };
        saw_valid = true;
        if !known_match_ids.contains(match_id) || begin_ts > watermark {
            return QuickPageDecision::Continue;
        }
    }
    if saw_valid {
        QuickPageDecision::WatermarkReached
    } else {
        QuickPageDecision::Unsafe
    }
}
```

Use the raw protocol entries for safety classification and the existing normalized extractor for persistence candidates.

- [ ] **Step 5: Run all incremental unit tests**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test incremental_sync
```

Expected: all incremental sync tests PASS.

- [ ] **Step 6: Commit the inventory and classifier**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/incremental.rs apps/owstats-desktop/src-tauri/src/sync/mod.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
git commit -m "feat(sync): classify quick season page boundaries"
```

### Task 3: Implement the Bounded Concurrent Quick List Scanner

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/season_fetch.rs:8-268`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Add failing request-count tests**

Create a recording server fixture with all five modes returning a known first page. Assert:

```rust
#[tokio::test]
async fn quick_update_requests_one_page_per_unchanged_mode_and_zero_details() {
    let fetch = fetch_quick_season_with_base_url(request(), &complete_inventory()).await.unwrap();
    assert_eq!(fetch.details_requested, 0);
    assert!(!fetch.quick_scan_limited);
    assert_eq!(list_requests_by_mode(&server), HashMap::from([
        ("sport", vec![1]),
        ("leisure", vec![1]),
        ("QuickFight", vec![1]),
        ("LeisureFight", vec![1]),
        ("SportFight", vec![1]),
    ]));
}
```

Add a cross-page test where `sport` has new data on pages 1 and 2, a known boundary on page 3, while other modes stop on page 1. Add a cap test where pages 1-5 remain unsafe or new and page 6 must never be requested.

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test phase8_real_fetch quick_update_requests_one_page_per_unchanged_mode_and_zero_details -- --nocapture
```

Expected: FAIL because `fetch_quick_season_with_base_url` does not exist.

- [ ] **Step 3: Add explicit quick fetch output**

Extend `FullSeasonFetch`:

```rust
pub quick_scan_limited: bool,
pub mode_stop_reasons: HashMap<String, String>,
```

Add:

```rust
const QUICK_UPDATE_MAX_PAGES_PER_MODE: i64 = 5;
```

Keep `fetch_full_season_with_base_url` exhaustive and detail-complete.

- [ ] **Step 4: Implement sequential-per-mode scanning**

Add a helper with this contract:

```rust
async fn fetch_quick_history_for_mode(
    client: &DashenClient,
    customer_token: &str,
    mode: &str,
    season_candidates: &[Option<i64>],
    fight: bool,
    inventory: &ModeSyncInventory,
) -> Result<ModeFetchPages>
```

For each season candidate:

```rust
for page in 1..=QUICK_UPDATE_MAX_PAGES_PER_MODE {
    let payload = if fight {
        client
            .fight_query_match_list(customer_token, mode, page, *request_season)
            .await?
    } else {
        client
            .query_match_list(customer_token, mode, page, *request_season)
            .await?
    };
    let raw_entries = extract_raw_match_entries(&payload);
    let mut valid_entries = extract_match_entries(&payload);
    list_pages.push(json!({
        "kind": if fight { "fight" } else { "normal" },
        "gameMode": mode,
        "requestSeason": request_season,
        "page": page,
        "payload": payload.clone(),
    }));
    for item in &mut valid_entries {
        item["gameMode"] = Value::String(mode.to_string());
        if fight {
            item["match_kind"] = Value::String("fight".to_string());
        }
    }
    entries.extend(valid_entries);
    match classify_quick_page(&raw_entries, &inventory.match_ids, inventory.latest_begin_ts) {
        QuickPageDecision::Empty => return stopped("empty"),
        QuickPageDecision::WatermarkReached => return stopped("watermark"),
        QuickPageDecision::Continue | QuickPageDecision::Unsafe => {}
    }
}
return limited_at_cap();
```

Do not request page `N + 1` until page `N` has been classified.

- [ ] **Step 5: Run mode scanners concurrently**

Build one future per normal/fight mode and collect with bounded concurrency equal to the number of configured modes:

```rust
let mode_results = stream::iter(mode_requests)
    .map(|mode_request| fetch_quick_mode(mode_request))
    .buffer_unordered(NORMAL_MODES.len() + FIGHT_MODES.len())
    .collect::<Vec<_>>()
    .await;
```

Merge results only after every requested mode completes. Preserve per-mode pages and stop reasons for diagnostics.

- [ ] **Step 6: Filter quick detail requests to the scanned range**

After deduplication:

```rust
let detail_entries = entries
    .iter()
    .filter(|entry| {
        string_field(entry, "matchId")
            .is_some_and(|id| !inventory.detail_ids.contains(&id))
    })
    .cloned()
    .collect::<Vec<_>>();
```

Do not append historical missing-detail IDs that were not returned by the quick list scan.

- [ ] **Step 7: Run the quick-fetch tests**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test phase8_real_fetch quick_update_ -- --nocapture
```

Expected: unchanged modes request `[1]`; cross-page sport requests `[1, 2, 3]`; cap case requests `[1, 2, 3, 4, 5]`; no quick test requests page 6.

- [ ] **Step 8: Commit the scanner**

```powershell
git add apps/owstats-desktop/src-tauri/src/season_fetch.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs
git commit -m "feat(sync): stop quick season scans at local watermarks"
```

### Task 4: Dispatch Quick Update and Full Fetch Through One Facade

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync/facade.rs:140-440`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs:570-607`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/persist.rs:153-210`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`

- [ ] **Step 1: Write failing mode-dispatch tests**

Add end-to-end tests:

```rust
#[tokio::test]
async fn quick_update_rejects_a_season_without_local_watermark() {
    let error = sync_with_mode(&empty_db(), SyncMode::QuickUpdate).await.unwrap_err();
    assert!(error.to_string().contains("请先使用全量抓取"));
}

#[tokio::test]
async fn full_fetch_scans_to_empty_and_requests_every_detail() {
    seed_complete_season(&db);
    let summary = sync_with_mode(&db, SyncMode::FullFetch).await.unwrap();
    assert_eq!(summary.mode, SyncMode::FullFetch);
    assert_eq!(detail_request_ids(&server), all_upstream_match_ids());
}
```

Add a failed-refresh test proving an old valid detail and its player index remain after the full fetch returns an invalid replacement.

- [ ] **Step 2: Run the tests and verify they fail**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test phase8_real_fetch quick_update_rejects_a_season_without_local_watermark
cargo test --test phase8_real_fetch full_fetch_scans_to_empty_and_requests_every_detail
```

Expected: FAIL because the mode is not dispatched.

- [ ] **Step 3: Pass mode from the Tauri command**

In `commands.rs`, pass `request.mode` beside `request.season`:

```rust
sync_season_with_credentials_and_target_paths_output_dir_base_url(
    &db,
    &root,
    &credential_path,
    &target_path,
    request.season,
    request.mode,
    CUSTOMER_BASE_URL,
    progress,
)
```

Update test-only helper signatures explicitly rather than adding an implicit default to the product command.

- [ ] **Step 4: Add facade dispatch**

At the account-scoped facade boundary:

```rust
match mode {
    SyncMode::QuickUpdate => {
        let inventory = load_season_sync_inventory(db, season)?;
        if inventory.match_ids.is_empty() || inventory.modes.values().all(|mode| mode.latest_begin_ts.is_none()) {
            return Err(anyhow!("该赛季尚无可用本地水位，请先使用全量抓取。"));
        }
        sync_quick_update_with_request(db, output_dir, request, inventory, progress).await
    }
    SyncMode::FullFetch => {
        sync_full_fetch_with_request(db, output_dir, request, progress).await
    }
}
```

Keep credential and target validation before this dispatch.

- [ ] **Step 5: Share one persistence and snapshot finalizer**

Extract:

```rust
fn finalize_season_sync(
    db: &OwDatabase,
    output_dir: &Path,
    season: i64,
    mode: SyncMode,
    fetch: FullSeasonFetch,
    attempted_detail_ids: HashSet<String>,
) -> Result<SyncSummary>
```

Both modes must:

1. Normalize only attempted detail outcomes.
2. Import entries and successful details incrementally.
3. Query SQLite season totals.
4. Rebuild the complete sanitized season payload.
5. Atomically replace the snapshot.
6. Populate mode, request counts, stop reasons, timing, and `quick_scan_limited`.

- [ ] **Step 6: Preserve old valid details during failed full refresh**

Use `sync_import_incremental_parts` for both modes with different attempted-ID sets:

```rust
let attempted_detail_ids = match mode {
    SyncMode::QuickUpdate => quick_scanned_detail_ids,
    SyncMode::FullFetch => all_deduplicated_list_ids,
};
```

The incremental importer upserts successful details and leaves an existing valid detail untouched when its replacement fails. Update error reconciliation so a failed full refresh records a safe current error without deleting the old detail or player index.

- [ ] **Step 7: Run backend mode tests**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test incremental_sync
cargo test --test phase8_real_fetch
```

Expected: all tests PASS, including request counts and failed full-refresh preservation.

- [ ] **Step 8: Commit facade dispatch**

```powershell
git add apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/sync/facade.rs apps/owstats-desktop/src-tauri/src/sync/persist.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs
git commit -m "feat(sync): dispatch quick updates and full fetches"
```

### Task 5: Add Honest Mode-Specific Progress and Completion Copy

**Files:**
- Modify: `apps/owstats-desktop/src/shared/phase6Shell.ts:56-79`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/facade.rs:238-400`
- Test: `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Write failing copy tests**

Add fixtures for:

```ts
assert.equal(
  formatSyncNotice(23, quickUnchanged),
  "S23 已检查最新对局，没有发现更新。",
);
assert.equal(
  formatSyncNotice(23, quickLimited),
  "S23 已更新扫描范围内的数据；快速更新已达到扫描范围，如需完整校验请使用全量抓取。",
);
assert.equal(
  formatSyncNotice(23, fullComplete),
  "S23 全量抓取完成：当前赛季共 300 场。",
);
```

- [ ] **Step 2: Run the copy test and verify it fails**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/incrementalSeasonSync.test.ts
```

Expected: FAIL because notices do not inspect `summary.mode` or `quickScanLimited`.

- [ ] **Step 3: Emit distinct backend phases**

Quick update phases:

```text
正在检查本地水位
正在快速检查最新对局
正在获取最新对局详情
正在更新本地数据库
更新完成
```

Full fetch phases:

```text
正在准备全量抓取
正在扫描完整赛季列表
正在重新获取全部详情
正在更新本地数据库
正在重建完整快照
全量抓取完成
```

Keep `processed`, `total`, and `failedDetails` monotonic within the current phase.

- [ ] **Step 4: Implement mode-specific completion copy**

Order the frontend branches:

```ts
if (summary.mode === "quickUpdate" && summary.quickScanLimited) {
  return `S${season} 已更新扫描范围内的数据；快速更新已达到扫描范围，如需完整校验请使用全量抓取。`;
}
if (summary.failedDetails > 0) {
  return `S${season} 对局列表已更新，${summary.failedDetails} 场详情获取失败，可再次操作重试。`;
}
if (summary.mode === "fullFetch") {
  return `S${season} 全量抓取完成：当前赛季共 ${summary.databaseMatches} 场。`;
}
if (summary.newMatches === 0 && summary.missingDetails === 0) {
  return `S${season} 已检查最新对局，没有发现更新。`;
}
return `S${season} 更新完成：新增 ${summary.newMatches} 场，当前赛季共 ${summary.databaseMatches} 场。`;
```

Do not claim “已是最新” when `quickScanLimited` is true or a page was classified unsafe.

- [ ] **Step 5: Run copy and backend progress tests**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/incrementalSeasonSync.test.ts
cd src-tauri
cargo test --test phase8_real_fetch progress
```

Expected: PASS with distinct quick/full phases and honest limited-scan copy.

- [ ] **Step 6: Commit progress behavior**

```powershell
git add apps/owstats-desktop/src/shared/phase6Shell.ts apps/owstats-desktop/src-tauri/src/sync/facade.rs apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs
git commit -m "feat(sync): report quick and full season progress"
```

### Task 6: Build the Full-Fetch and Row-Level Update UI

**Files:**
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx:61-75,300-340,595-609`
- Modify: `apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/command-center.css:171-270`
- Test: `apps/owstats-desktop/scripts/phase6Shell.test.ts`
- Test: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`
- Test: `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`

- [ ] **Step 1: Write failing UI contract tests**

Require:

```ts
assert.match(workbenchSource, />全量抓取</);
assert.match(workbenchSource, />更新</);
assert.match(workbenchSource, /onQuickUpdate:\s*\(season:\s*number\)\s*=>\s*void/);
assert.match(workbenchSource, /onFullFetch:\s*\(season:\s*number\)\s*=>\s*void/);
assert.doesNotMatch(
  workbenchSource,
  /<button[^>]*className=.*syncedSeasonLedgerRow[\s\S]*?<button/,
  "season rows must not nest buttons",
);
```

Add a source contract proving `onQuickUpdate(seasonStatus.season)` is used directly and does not call `onSelectSeason` first.

- [ ] **Step 2: Run UI contract tests and verify they fail**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/phase8RealFetch.test.ts
```

Expected: FAIL because only `onSync` and a button row exist.

- [ ] **Step 3: Make the page operation mode-aware**

Add:

```ts
const [syncingMode, setSyncingMode] = useState<SyncMode | null>(null);

async function runSeasonSync(season: number, mode: SyncMode) {
  setSyncingSeason(season);
  setSyncingMode(mode);
  try {
    const summary = await syncSeason(season, mode);
    clearMatchesPageCache();
    await loadDashboard();
    setSyncNotice(formatSyncNotice(season, summary));
  } finally {
    setSyncingSeason(null);
    setSyncingMode(null);
  }
}
```

Pass:

```tsx
onFullFetch={(season) => void runSeasonSync(season, "fullFetch")}
onQuickUpdate={(season) => void runSeasonSync(season, "quickUpdate")}
syncingMode={syncingMode}
```

Do not call `setSelectedSeason` from `onQuickUpdate`.

- [ ] **Step 4: Split the ledger row into legal controls**

Replace the outer row button with:

```tsx
<div
  className={`syncedSeasonLedgerRow${selectedSeason === seasonStatus.season ? " selected" : ""}${rowSyncing ? " syncing" : ""}`}
  role="row"
>
  <button
    className="syncedSeasonSelect"
    type="button"
    aria-pressed={selectedSeason === seasonStatus.season}
    onClick={() => onSelectSeason(seasonStatus.season)}
  >
    {/* season, coverage, count, last sync cells */}
  </button>
  <button
    className="ghostButton syncedSeasonUpdateButton"
    type="button"
    onClick={() => onQuickUpdate(seasonStatus.season)}
    disabled={!credentialReady || !targetSyncAllowed || accountOperationBusy || syncingSeason !== null}
  >
    {rowSyncing && syncingMode === "quickUpdate" ? "更新中" : "更新"}
  </button>
</div>
```

Give the left button an accessible name that includes the season. Give the update button `aria-label={`更新 S${seasonStatus.season}`}`.

- [ ] **Step 5: Rename and wire the full-fetch button**

Use:

```tsx
<button
  className="primaryButton seasonSyncButton"
  onClick={() => onFullFetch(selectedSeason)}
    disabled={!credentialReady || !targetSyncAllowed || accountOperationBusy || syncingSeason !== null}
>
  <RefreshCw size={18} />
  {isSelectedFullFetch ? "全量抓取中" : "全量抓取"}
</button>
```

The left button acts only on `selectedSeason`.

- [ ] **Step 6: Update the table grid and responsive CSS**

Use a five-column row with a compact operation column:

```css
.syncedSeasonLedgerHeader,
.syncedSeasonLedgerRow {
  display: grid;
  grid-template-columns:
    minmax(64px, 0.55fr)
    minmax(150px, 1.5fr)
    minmax(88px, 0.75fr)
    minmax(112px, 0.9fr)
    minmax(56px, auto);
  gap: 12px;
  align-items: center;
}

.syncedSeasonSelect {
  display: contents;
}

.syncedSeasonUpdateButton {
  min-width: 56px;
  min-height: 44px;
  padding: 0 12px;
}
```

If `display: contents` harms focus rendering in the existing browser target, use a nested four-column grid spanning columns 1-4 and keep an explicit focus outline on that button. Verify keyboard focus visually or by computed-style test.

- [ ] **Step 7: Run frontend focused tests**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/phase8RealFetch.test.ts
npx tsx scripts/incrementalSeasonSync.test.ts
```

Expected: PASS; source contracts confirm separate buttons and direct row-season update.

- [ ] **Step 8: Commit the UI**

```powershell
git add apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/scripts/phase8RealFetch.test.ts apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts
git commit -m "feat(sync): add quick update and full fetch controls"
```

### Task 7: Lock Down End-to-End Speed, Recovery, and Privacy

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`
- Modify: `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`

- [ ] **Step 1: Add a single end-to-end quick-update request ledger**

Seed a complete S23 database, run quick update against a recording server, and assert:

```rust
assert_eq!(summary.mode, SyncMode::QuickUpdate);
assert!(!summary.quick_scan_limited);
assert_eq!(summary.details_requested, 0);
assert_eq!(list_request_count(&server, "sport"), 1);
assert_eq!(list_request_count(&server, "leisure"), 1);
assert_eq!(list_request_count(&server, "QuickFight"), 1);
assert_eq!(list_request_count(&server, "LeisureFight"), 1);
assert_eq!(list_request_count(&server, "SportFight"), 1);
assert_eq!(detail_request_count(&server), 0);
```

- [ ] **Step 2: Add a late historical insertion fixture**

Place a synthetic unknown match after the quick five-page limit:

```rust
let quick = sync_with_mode(&db, SyncMode::QuickUpdate).await.unwrap();
assert!(quick.quick_scan_limited || !database_has("deep-late-match"));

let full = sync_with_mode(&db, SyncMode::FullFetch).await.unwrap();
assert!(database_has("deep-late-match"));
assert_eq!(full.mode, SyncMode::FullFetch);
```

This test documents the accepted quick/full product boundary.

- [ ] **Step 3: Add snapshot and failure preservation assertions**

After both modes:

```rust
assert_complete_snapshot_contains_old_and_new(&snapshot);
assert_no_sensitive_keys_or_values(&snapshot);
assert_old_valid_detail_survives_failed_full_refresh(&db);
assert_old_player_index_survives_failed_full_refresh(&db);
```

Use synthetic token-shaped values only; do not read local credentials or real app data.

- [ ] **Step 4: Run the complete sync-focused suite**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test incremental_sync
cargo test --test phase8_real_fetch
cd ..
npx tsx scripts/incrementalSeasonSync.test.ts
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/phase8RealFetch.test.ts
```

Expected: all focused tests PASS with exact request counts.

- [ ] **Step 5: Commit end-to-end coverage**

```powershell
git add apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts
git commit -m "test(sync): verify quick update request budgets"
```

### Task 8: Full Verification and Delivery Audit

**Files:**
- Verify only; fix only files already listed in this plan if failures expose a requirement gap.

- [ ] **Step 1: Format and check Rust**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: formatting clean; all backend tests PASS with zero failures.

- [ ] **Step 2: Run the complete frontend suite**

Run:

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: all script tests PASS; TypeScript/Vite build succeeds. Existing chunk-size warnings may be reported but are not failures.

- [ ] **Step 3: Run the desktop release build**

Stop any running preview/dev server that owns `public/assets`, then run:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

Expected: MSI and NSIS artifacts build successfully.

- [ ] **Step 4: Audit privacy and packaged paths**

Run:

```powershell
rg -n "D:\\\\Codex|/@fs/|customerToken|authorization|OW_DASHEN_TOKEN" apps/owstats-desktop/src apps/owstats-desktop/src-tauri/src apps/owstats-desktop/scripts
```

Expected: no new development-machine paths; any protocol-field matches are confined to existing redaction/transport boundaries and are not logged, serialized to diagnostics, or exposed in UI.

- [ ] **Step 5: Check diff and worktree scope**

Run:

```powershell
git diff --check
git status --short
git diff --stat HEAD~8..HEAD
```

Expected: no whitespace errors; only planned feature files plus pre-existing user changes are present. Do not stage, reset, or alter unrelated account-login work.

- [ ] **Step 6: Record final performance evidence**

In the delivery report, include:

```text
Quick unchanged list requests: one per configured mode
Quick unchanged detail requests: 0
Quick maximum pages per mode: 5
Full fetch termination: empty page
Full fetch detail scope: all deduplicated season matches
Real credentials/private data accessed: no
```

- [ ] **Step 7: Commit any verification-only corrections**

If verification required a correction, stage only the affected planned files:

```powershell
git add apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/sync/incremental.rs apps/owstats-desktop/src-tauri/src/season_fetch.rs apps/owstats-desktop/src-tauri/src/sync/facade.rs apps/owstats-desktop/src-tauri/src/sync/persist.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/shared/api.ts apps/owstats-desktop/src/shared/phase6Shell.ts apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/features/command-center/SeasonSyncWorkbench.tsx apps/owstats-desktop/src/styles/features/command-center.css apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/scripts/phase8RealFetch.test.ts
git commit -m "fix(sync): complete fast update verification"
```

If no correction was required, do not create an empty commit.
