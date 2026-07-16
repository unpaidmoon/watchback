# Safe Incremental Season Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make ordinary season sync fetch every list page for correctness while requesting details only for new matches or locally missing details.

**Architecture:** Add a SQLite-backed season inventory and keep the existing list transport as the source of truth. Pass the known-detail set into the fetch layer, persist the resulting delta with an explicit incremental import mode, rebuild only affected indexes, then reconstruct the complete sanitized season snapshot from SQLite. Expose non-sensitive counts and timings through the existing sync DTO so the UI can report what was actually updated.

**Tech Stack:** Rust, Tauri 2, Tokio, reqwest, rusqlite, serde/serde_json, tempfile, React, TypeScript, source-level `tsx` tests.

**Design:** `docs/superpowers/specs/2026-07-16-incremental-season-sync-design.md`

---

## Execution prerequisites

- Execute in an isolated worktree created with `superpowers:using-git-worktrees`; the current workspace contains unrelated account-login edits, including an overlapping `Cargo.toml`.
- Base the worktree on commit `a0fe944` or a descendant that already contains `docs/superpowers/specs/2026-07-16-incremental-season-sync-design.md`.
- Do not copy the current workspace's unrelated unstaged files into the implementation worktree.
- Never run a real upstream sync during automated tests. All request-count assertions use local recording servers.
- Do not print or persist raw match IDs in diagnostics. Test fixture IDs are synthetic.

## File map

**Create**

- `apps/owstats-desktop/src-tauri/src/sync/incremental.rs`
  - Loads existing match/detail IDs, selects the detail delta, builds safe per-page diagnostics, and queries database totals.
- `apps/owstats-desktop/src-tauri/src/sync/snapshot.rs`
  - Reconstructs a complete sanitized season payload from SQLite and atomically writes it.
- `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`
  - Covers inventory selection, incremental persistence, complete snapshot reconstruction, idempotency, and preserved indexes/errors.
- `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`
  - Locks frontend DTO/copy wiring and prevents the UI from reporting the full list size as newly imported matches.

**Modify**

- `apps/owstats-desktop/src-tauri/Cargo.toml`
  - Promote `tempfile` from dev-only to a normal dependency for same-directory atomic snapshot replacement.
- `apps/owstats-desktop/src-tauri/Cargo.lock`
  - Refresh dependency-section placement if Cargo changes the lockfile after promoting `tempfile`.
- `apps/owstats-desktop/src-tauri/src/sync/mod.rs`
  - Register the two focused sync modules and internal types.
- `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
  - Add a detail-filtered fetch entrypoint while preserving the full-fetch entrypoint used by diagnostic binaries.
- `apps/owstats-desktop/src-tauri/src/importer.rs`
  - Add explicit incremental import behavior, preserve detail-derived match fields, update indexes/errors by match, and compute season counts from SQLite.
- `apps/owstats-desktop/src-tauri/src/sync/persist.rs`
  - Route desktop sync through incremental import metrics.
- `apps/owstats-desktop/src-tauri/src/sync/facade.rs`
  - Load inventory, call filtered detail fetch, persist the delta, rebuild the full snapshot, and populate progress/summary diagnostics.
- `apps/owstats-desktop/src-tauri/src/models.rs`
  - Extend `SyncSummary` with safe incremental counts and diagnostics.
- `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
  - Prove full pagination continues and detail requests are filtered.
- `apps/owstats-desktop/src/shared/types.ts`
  - Mirror the Rust summary/diagnostic DTOs.
- `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
  - Report new, repaired, and total match counts in Chinese.
- `apps/owstats-desktop/src/shared/phase6Shell.ts`
  - Render the new progress phases without exposing IDs.
- `apps/owstats-desktop/package.json`
  - Add the focused frontend regression test to `test:unit`.

## Task 1: Add the local season inventory and delta selector

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/sync/incremental.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing inventory and selection tests**

Add these tests to `incremental_sync.rs` using synthetic IDs only:

```rust
use owstats_desktop_lib::{
    load_season_sync_inventory, select_detail_entries, OwDatabase,
};
use rusqlite::params;
use serde_json::json;

#[test]
fn inventory_is_scoped_to_the_requested_season() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
            begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
            mitigation, total_worth, duration_sec, raw_entry_json, updated_at
         ) VALUES (?1, ?2, 'preset', 'sport', 'normal', 'win', 'map', 'hero',
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, '{}', 'now')",
        params!["s23-complete", 23],
    ).unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
         VALUES ('s23-complete', 'normal', '{}', 'now')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
            begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
            mitigation, total_worth, duration_sec, raw_entry_json, updated_at
         ) VALUES (?1, ?2, 'preset', 'sport', 'normal', 'win', 'map', 'hero',
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, '{}', 'now')",
        params!["s22-complete", 22],
    ).unwrap();

    let inventory = load_season_sync_inventory(&db, 23).unwrap();
    assert!(inventory.match_ids.contains("s23-complete"));
    assert!(inventory.detail_ids.contains("s23-complete"));
    assert!(!inventory.match_ids.contains("s22-complete"));
}

#[test]
fn detail_selector_returns_only_new_or_missing_details() {
    let mut inventory = owstats_desktop_lib::SeasonSyncInventory::default();
    inventory.match_ids.extend([
        "complete".to_string(),
        "missing".to_string(),
    ]);
    inventory.detail_ids.insert("complete".to_string());
    let entries = vec![
        json!({"matchId":"complete","beginTs":3}),
        json!({"matchId":"missing","beginTs":2}),
        json!({"matchId":"new","beginTs":1}),
        json!({"matchId":"new","beginTs":1}),
    ];

    let selection = select_detail_entries(&entries, &inventory);
    let ids = selection.entries.iter()
        .filter_map(|entry| entry["matchId"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["missing", "new"]);
    assert_eq!(selection.new_matches, 1);
    assert_eq!(selection.missing_details, 1);
}
```

- [ ] **Step 2: Run the focused test and verify the missing API failure**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test incremental_sync inventory_is_scoped_to_the_requested_season -- --exact
```

Expected: FAIL because `load_season_sync_inventory` and the public incremental types do not exist.

- [ ] **Step 3: Implement the focused inventory module**

Create `sync/incremental.rs` with these public-in-crate units:

```rust
use crate::db::OwDatabase;
use anyhow::Result;
use rusqlite::params;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonSyncInventory {
    pub match_ids: HashSet<String>,
    pub detail_ids: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetailSelection {
    pub entries: Vec<Value>,
    pub new_matches: usize,
    pub missing_details: usize,
}

pub fn load_season_sync_inventory(
    db: &OwDatabase,
    season: i64,
) -> Result<SeasonSyncInventory> {
    db.initialize()?;
    let conn = db.connect()?;
    let mut statement = conn.prepare(
        "SELECT m.match_id,
                CASE WHEN d.match_id IS NULL THEN 0 ELSE 1 END
         FROM matches m
         LEFT JOIN match_details d ON d.match_id = m.match_id
         WHERE m.season = ?1",
    )?;
    let rows = statement.query_map(params![season], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
    })?;
    let mut inventory = SeasonSyncInventory::default();
    for row in rows {
        let (match_id, has_detail) = row?;
        inventory.match_ids.insert(match_id.clone());
        if has_detail {
            inventory.detail_ids.insert(match_id);
        }
    }
    Ok(inventory)
}

pub fn select_detail_entries(
    entries: &[Value],
    inventory: &SeasonSyncInventory,
) -> DetailSelection {
    let mut seen = HashSet::<String>::new();
    let mut selected = Vec::new();
    let mut new_matches = 0;
    let mut missing_details = 0;
    for entry in entries {
        let Some(match_id) = entry.get("matchId").and_then(Value::as_str) else {
            continue;
        };
        if match_id.is_empty()
            || inventory.detail_ids.contains(match_id)
            || !seen.insert(match_id.to_string())
        {
            continue;
        }
        if inventory.match_ids.contains(match_id) {
            missing_details += 1;
        } else {
            new_matches += 1;
        }
        selected.push(entry.clone());
    }
    DetailSelection {
        entries: selected,
        new_matches,
        missing_details,
    }
}
```

Register `mod incremental;` in `sync/mod.rs`, re-export the four units through `sync/mod.rs`, then re-export them from `lib.rs` for integration tests.

- [ ] **Step 4: Run both focused tests**

Run:

```powershell
cargo test --test incremental_sync inventory_is_scoped_to_the_requested_season -- --exact
cargo test --test incremental_sync detail_selector_returns_only_new_or_missing_details -- --exact
```

Expected: both PASS.

- [ ] **Step 5: Commit the inventory slice**

```powershell
git add apps/owstats-desktop/src-tauri/src/sync/incremental.rs apps/owstats-desktop/src-tauri/src/sync/mod.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
git commit -m "feat(sync): select incremental season details"
```

## Task 2: Keep full pagination while filtering detail requests

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Add a recording-server regression test**

Add a server fixture that returns:

- `sport page=1`: one locally complete match.
- `sport page=2`: one new match.
- `sport page=3` and later: empty.
- `leisure`: empty.
- Detail response only for the new match.

Add this test:

```rust
#[tokio::test]
async fn incremental_fetch_scans_past_known_pages_and_requests_only_missing_details() {
    use owstats_desktop_lib::dashen_client::DashenCredentials;
    use owstats_desktop_lib::season_fetch::{
        fetch_season_increment_with_base_url, FullSeasonRequest,
    };
    use std::collections::HashSet;

    let server = spawn_incremental_pagination_server();
    let known_details = HashSet::from(["known-page-one".to_string()]);
    let fetch = fetch_season_increment_with_base_url(
        FullSeasonRequest {
            credentials: DashenCredentials {
                role_id: 123,
                token: "account-auth-token".to_string(),
                dts: Some(2026),
                server: 1,
            },
            customer_token: "player-customer-token".to_string(),
            input_bnet: "Tester#12345".to_string(),
            resolved_bnet: Some("Tester#12345".to_string()),
            season: 23,
            customer_base_url: format!("{}/customer", server.base_url),
            pages_per_batch: 1,
            detail_concurrency: 2,
            include_fight: false,
        },
        &known_details,
    ).await.unwrap();

    assert_eq!(fetch.entries, 2);
    assert_eq!(fetch.details_requested, 1);
    assert_eq!(fetch.details, 1);
    let requests = server.requests.lock().unwrap();
    assert!(requests.iter().any(|line| line.contains("page=2")));
    assert!(requests.iter().any(|line| line.contains("matchId=new-page-two")));
    assert!(!requests.iter().any(|line| line.contains("matchId=known-page-one")));
}
```

- [ ] **Step 2: Run the test and verify it fails at the new entrypoint**

Run:

```powershell
cargo test --test phase8_real_fetch incremental_fetch_scans_past_known_pages_and_requests_only_missing_details -- --exact
```

Expected: FAIL because `fetch_season_increment_with_base_url` and `details_requested` do not exist.

- [ ] **Step 3: Introduce a shared filtered fetch implementation**

In `season_fetch.rs`:

1. Add `details_requested: usize`, `list_duration_ms: u64`, and `detail_duration_ms: u64` to `FullSeasonFetch`.
2. Keep `fetch_full_season_with_base_url(request)` as a compatibility wrapper that passes an empty known-detail set.
3. Add:

```rust
pub async fn fetch_season_increment_with_base_url(
    request: FullSeasonRequest,
    known_detail_ids: &HashSet<String>,
) -> Result<FullSeasonFetch> {
    fetch_season_with_rate_limit_base_url(
        request,
        DashenRateLimitConfig::default(),
        known_detail_ids,
    ).await
}
```

4. Preserve the existing public rate-limit entrypoint for CLI/tests:

```rust
pub async fn fetch_full_season_with_rate_limit_base_url(
    request: FullSeasonRequest,
    rate_limit: DashenRateLimitConfig,
) -> Result<FullSeasonFetch> {
    fetch_season_with_known_details(request, rate_limit, &HashSet::new()).await
}
```

5. Move the shared implementation behind:

```rust
async fn fetch_season_with_known_details(
    request: FullSeasonRequest,
    rate_limit: DashenRateLimitConfig,
    known_detail_ids: &HashSet<String>,
) -> Result<FullSeasonFetch>
```

6. After list deduplication, filter only the input to `fetch_details`:

```rust
let detail_entries = entries.iter()
    .filter(|entry| {
        entry.get("matchId")
            .and_then(Value::as_str)
            .map(|match_id| !known_detail_ids.contains(match_id))
            .unwrap_or(false)
    })
    .cloned()
    .collect::<Vec<_>>();
let details_requested = detail_entries.len();
let (details, detail_errors) = fetch_details(
    &client,
    &request.customer_token,
    &detail_entries,
    request.detail_concurrency,
).await;
```

7. Measure the list section and detail section separately with `std::time::Instant`.
8. Continue passing the complete `entries` vector to `build_payload`; pass only delta `details` and `detail_errors`.
9. Keep the rate-limit test and CLI signature unchanged; the compatibility wrapper supplies the empty known-detail set.

- [ ] **Step 4: Run the transport regression group**

Run:

```powershell
cargo test --test phase8_real_fetch incremental_fetch_scans_past_known_pages_and_requests_only_missing_details -- --exact
cargo test --test phase8_real_fetch full_season_fetch_never_persists_local_or_upstream_tokens -- --exact
cargo test --test phase8_real_fetch full_season_fetch_tries_current_season_without_season_param_first -- --exact
cargo test --test phase8_real_fetch full_season_fetch_fetches_details_with_configured_concurrency -- --exact
```

Expected: all PASS. The incremental test must observe page 2 and exactly one detail request.

- [ ] **Step 5: Commit the transport slice**

```powershell
git add apps/owstats-desktop/src-tauri/src/season_fetch.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs
git commit -m "feat(sync): skip existing season details"
```

## Task 3: Add explicit incremental persistence semantics

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/importer.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/persist.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`

- [ ] **Step 1: Write persistence regression tests**

Add this shared synthetic fixture helper:

```rust
fn seed_incremental_match(
    conn: &rusqlite::Connection,
    match_id: &str,
    season: i64,
    with_detail: bool,
    player_name: &str,
) {
    let entry = json!({
        "matchId": match_id,
        "gameMode": "SportPreset",
        "beginTs": 1_700_000_000_000_i64
    });
    conn.execute(
        "INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
            begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
            mitigation, total_worth, duration_sec, raw_entry_json, updated_at
         ) VALUES (?1, ?2, 'preset', 'SportPreset', 'normal', 'win', 'map', 'hero',
            1700000000, 1, 0, 1, 2, 3, 4, 5, 6, 7, 600, ?3, 'now')",
        params![match_id, season, entry.to_string()],
    ).unwrap();
    if with_detail {
        let detail = json!({
            "match_id": match_id,
            "match_kind": "normal",
            "payload": {"data": {
                "gameTimeSec": 600,
                "teammateList": [],
                "enemyList": []
            }}
        });
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
             VALUES (?1, 'normal', ?2, 'now')",
            params![match_id, detail.to_string()],
        ).unwrap();
    }
    conn.execute(
        "INSERT INTO match_players (
            match_id, season, side, bnet_id, player_name, hero_guid, raw_json, updated_at
         ) VALUES (?1, ?2, 'teammate', ?3, ?4, 'hero', '{}', 'now')",
        params![match_id, season, format!("{match_id}-player"), player_name],
    ).unwrap();
}

fn incremental_payload(entries: Vec<Value>, details: Vec<Value>, errors: Vec<Value>) -> Value {
    json!({
        "season": 23,
        "profile_card": {"data": {"bnetId": "current-player"}},
        "matches": {
            "entries": entries,
            "details": details,
            "detail_errors": errors
        }
    })
}

#[test]
fn incremental_import_preserves_existing_detail_fields_and_unaffected_indexes() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    seed_incremental_match(&conn, "untouched", 23, true, "keep-me");
    seed_incremental_match(&conn, "refreshed", 23, true, "replace-me");
    drop(conn);

    let entries = vec![
        json!({"matchId":"untouched","gameMode":"SportPreset","beginTs":1700000000000_i64}),
        json!({"matchId":"refreshed","gameMode":"SportPreset","beginTs":1700000000001_i64}),
    ];
    let refreshed_detail = json!({
        "match_id":"refreshed",
        "match_kind":"normal",
        "payload":{"data":{
            "gameTimeSec":700,
            "teammateList":[{
                "bnetId":"fresh-player",
                "name":"fresh-player",
                "heroGuid":"hero",
                "friendBnetIds":[]
            }],
            "enemyList":[]
        }}
    });
    import_incremental_payload_value_embedded(
        &db,
        &incremental_payload(entries, vec![refreshed_detail], vec![]),
        HashSet::from(["refreshed".to_string()]),
    ).unwrap();

    let conn = db.connect().unwrap();
    let preserved: (String, String, i64) = conn.query_row(
        "SELECT match_kind, bucket, duration_sec FROM matches WHERE match_id='untouched'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(preserved, ("normal".to_string(), "preset".to_string(), 600));
    let untouched_players: i64 = conn.query_row(
        "SELECT COUNT(*) FROM match_players
         WHERE match_id='untouched' AND player_name='keep-me'",
        [],
        |row| row.get(0),
    ).unwrap();
    let stale_refreshed_players: i64 = conn.query_row(
        "SELECT COUNT(*) FROM match_players
         WHERE match_id='refreshed' AND player_name='replace-me'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(untouched_players, 1);
    assert_eq!(stale_refreshed_players, 0);
}

#[test]
fn incremental_import_updates_only_attempted_detail_errors() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for match_id in ["unattempted", "recovered", "failed"] {
        seed_incremental_match(&conn, match_id, 23, false, match_id);
    }
    for match_id in ["unattempted", "recovered"] {
        conn.execute(
            "INSERT INTO sync_errors (
                season, match_id, error_type, message, raw_json, created_at
             ) VALUES (23, ?1, 'DetailFetchError', 'old', '{}', 'now')",
            params![match_id],
        ).unwrap();
    }
    drop(conn);

    import_incremental_payload_value_embedded(
        &db,
        &incremental_payload(
            vec![],
            vec![json!({
                "match_id":"recovered",
                "match_kind":"normal",
                "payload":{"data":{"teammateList":[],"enemyList":[]}}
            })],
            vec![json!({
                "match_id":"failed",
                "error_type":"DetailFetchError",
                "message":"latest safe error"
            })],
        ),
        HashSet::from(["recovered".to_string(), "failed".to_string()]),
    ).unwrap();

    let conn = db.connect().unwrap();
    let ids = {
        let mut statement = conn.prepare(
            "SELECT match_id FROM sync_errors WHERE season=23 ORDER BY match_id",
        ).unwrap();
        statement.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(ids, vec!["failed".to_string(), "unattempted".to_string()]);
}

#[test]
fn incremental_import_uses_database_totals_not_delta_lengths() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for index in 0..10 {
        seed_incremental_match(&conn, &format!("old-{index}"), 23, true, "old");
    }
    drop(conn);

    import_incremental_payload_value_embedded(
        &db,
        &incremental_payload(
            vec![json!({
                "matchId":"new-10",
                "gameMode":"SportPreset",
                "beginTs":1700000000010_i64
            })],
            vec![json!({
                "match_id":"new-10",
                "match_kind":"normal",
                "payload":{"data":{"teammateList":[],"enemyList":[]}}
            })],
            vec![],
        ),
        HashSet::from(["new-10".to_string()]),
    ).unwrap();

    let conn = db.connect().unwrap();
    let totals: (i64, i64) = conn.query_row(
        "SELECT matches_count, details_count FROM seasons WHERE season=23",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert_eq!(totals, (11, 11));
}
```

Import `import_incremental_payload_value_embedded`, `HashSet`, and `Value` at the top of the test file. Re-export the importer entrypoint from `lib.rs` only for crate integration tests and documented internal tooling; do not expose it as a Tauri command.

- [ ] **Step 2: Run the tests and verify current full-snapshot behavior fails**

Run:

```powershell
cargo test --test incremental_sync incremental_import_ -- --nocapture
```

Expected: FAIL because no incremental importer exists; after wiring the current importer temporarily, the preservation assertions would fail because it clears the whole season's player indexes and errors.

- [ ] **Step 3: Add an explicit import behavior**

In `importer.rs`, add:

```rust
#[derive(Debug, Clone)]
enum ImportBehavior {
    FullSnapshot,
    Incremental {
        attempted_detail_ids: HashSet<String>,
    },
}

pub fn import_incremental_payload_value_embedded(
    db: &OwDatabase,
    payload: &Value,
    attempted_detail_ids: HashSet<String>,
) -> Result<ImportSummary> {
    import_payload_value_with_lookups_and_behavior(
        db,
        payload,
        load_embedded_lookup()?,
        ImportBehavior::Incremental { attempted_detail_ids },
    )
}
```

Make the existing embedded/file import functions call the same internal implementation with `ImportBehavior::FullSnapshot`.

- [ ] **Step 4: Preserve existing detail-derived fields during list upsert**

For `ON CONFLICT(match_id) DO UPDATE`, use conditional updates:

```sql
match_kind=CASE
    WHEN excluded.match_kind = '' THEN matches.match_kind
    ELSE excluded.match_kind
END,
bucket=CASE
    WHEN excluded.match_kind = '' AND matches.match_kind <> '' THEN matches.bucket
    ELSE excluded.bucket
END,
duration_sec=CASE
    WHEN excluded.duration_sec <= 0 AND matches.duration_sec > 0 THEN matches.duration_sec
    ELSE excluded.duration_sec
END
```

Keep the remaining list-level columns refreshed from `excluded`.

- [ ] **Step 5: Rebuild only affected player indexes**

Extract the per-detail body of `rebuild_match_players_for_season` into:

```rust
fn rebuild_match_players_for_details(
    tx: &rusqlite::Transaction<'_>,
    season: i64,
    details: &[Value],
    lookups: &Lookups,
    payload: Option<&Value>,
    now: &str,
    clear_entire_season: bool,
) -> Result<()>
```

Behavior:

```rust
if clear_entire_season {
    tx.execute("DELETE FROM match_players WHERE season = ?1", params![season])?;
    tx.execute("DELETE FROM arena_match_players WHERE season = ?1", params![season])?;
} else {
    for detail in details {
        let match_id = string_field(detail, "match_id").unwrap_or_default();
        if match_id.is_empty() {
            continue;
        }
        tx.execute("DELETE FROM match_players WHERE match_id = ?1", params![match_id])?;
        tx.execute("DELETE FROM arena_match_players WHERE match_id = ?1", params![match_id])?;
    }
}
```

After the scoped deletion block, move the current loop that computes `bucket`, reads normal/arena player rows, and calls `insert_player_rows` or `insert_arena_player_index_rows` into this helper without changing its projection rules. Full imports pass `true`; incremental imports pass `false`.

- [ ] **Step 6: Update only attempted error rows**

For `FullSnapshot`, retain `DELETE FROM sync_errors WHERE season = ?`.

For `Incremental`, delete only attempted IDs before inserting the current errors:

```rust
if let ImportBehavior::Incremental { attempted_detail_ids } = &behavior {
    for match_id in attempted_detail_ids {
        tx.execute(
            "DELETE FROM sync_errors WHERE season = ?1 AND match_id = ?2",
            params![season, match_id],
        )?;
    }
}
```

Keep `INSERT OR IGNORE` for the sanitized current errors.

- [ ] **Step 7: Derive season totals from SQLite**

Before upserting `seasons`, query:

```sql
SELECT COUNT(*) FROM matches WHERE season = ?1
```

and:

```sql
SELECT COUNT(*)
FROM match_details d
JOIN matches m ON m.match_id = d.match_id
WHERE m.season = ?1
```

Write those database totals into `seasons.matches_count` and `seasons.details_count`. Keep `ImportSummary.matches_seen` and `details_seen` as counts from the current payload so legacy callers retain their existing semantics.

- [ ] **Step 8: Add a separate incremental persistence entrypoint**

Keep the existing `sync_import_parts` signature and full-snapshot behavior for legacy fetch/dump callers. Add:

```rust
pub(super) fn sync_import_incremental_parts(
    db: &OwDatabase,
    season: i64,
    cached_player: &Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    attempted_detail_ids: HashSet<String>,
) -> Result<SyncSummary>
```

Build the same sanitized payload shape as `sync_import_parts`, call `import_incremental_payload_value_embedded`, and construct the base summary. The desktop target/current-account facades use this new function; legacy JSON import and dump paths continue calling `sync_import_parts`.

- [ ] **Step 9: Run importer tests**

Run:

```powershell
cargo test --test incremental_sync incremental_import_ -- --nocapture
cargo test --test import_baseline sync_import_parts_is_idempotent_and_sanitizes_detail_errors -- --exact
cargo test --test import_baseline interrupted_import_rolls_back_and_a_later_retry_remains_idempotent -- --exact
```

Expected: all PASS.

- [ ] **Step 10: Commit persistence semantics**

```powershell
git add apps/owstats-desktop/src-tauri/src/importer.rs apps/owstats-desktop/src-tauri/src/sync/persist.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
git commit -m "fix(sync): preserve season state during incremental import"
```

## Task 4: Rebuild and atomically replace the complete sanitized snapshot

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/sync/snapshot.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/persist.rs`
- Modify: `apps/owstats-desktop/src-tauri/Cargo.toml`
- Modify: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`

- [ ] **Step 1: Write full-snapshot and failed-replacement tests**

Add:

```rust
#[test]
fn rebuilt_snapshot_contains_old_and_new_matches_without_sensitive_fields() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    seed_incremental_match(&conn, "old", 23, true, "old-player");
    seed_incremental_match(&conn, "new", 23, true, "new-player");
    drop(conn);

    let current = json!({
        "season":23,
        "profile_card":{"data":{"customerToken":"profile-secret"}},
        "matches":{
            "entries":[{"matchId":"new"}],
            "details":[{"match_id":"new"}],
            "detail_errors":[],
            "list_pages":[]
        }
    });
    let rebuilt = rebuild_complete_season_payload(&db, 23, &current).unwrap();
    let entries = rebuilt["matches"]["entries"].as_array().unwrap();
    let details = rebuilt["matches"]["details"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(details.len(), 2);
    let text = serde_json::to_string(&rebuilt).unwrap();
    for forbidden in ["token", "customerToken", "cookie", "authorization", "credential"] {
        assert!(!text.to_ascii_lowercase().contains(&forbidden.to_ascii_lowercase()));
    }
}

#[test]
fn atomic_snapshot_write_replaces_the_old_file_without_truncation() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path();
    let final_path = output.join("local_s23_full_season.json");
    std::fs::write(&final_path, r#"{"season":23,"old":true}"#).unwrap();
    let payload = json!({
        "season":23,
        "matches":{"entries":[{"matchId":"new"}],"details":[],"detail_errors":[]}
    });

    let written = write_season_payload(output, 23, &payload).unwrap();
    assert_eq!(written, final_path);
    let parsed: Value = serde_json::from_slice(&std::fs::read(&final_path).unwrap()).unwrap();
    assert_eq!(parsed["matches"]["entries"][0]["matchId"], "new");
    let remaining = std::fs::read_dir(output).unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert_eq!(remaining, vec!["local_s23_full_season.json".to_string()]);
}
```

Expose `rebuild_complete_season_payload` and `write_season_payload` from `sync/mod.rs` and `lib.rs` for integration tests only; keep them as ordinary Rust functions, not Tauri commands.

- [ ] **Step 2: Run the tests and verify the snapshot module is missing**

Run:

```powershell
cargo test --test incremental_sync rebuilt_snapshot_contains_old_and_new_matches_without_sensitive_fields -- --exact
cargo test --test incremental_sync atomic_snapshot_write_replaces_the_old_file_without_truncation -- --exact
```

Expected: FAIL because the snapshot APIs do not exist.

- [ ] **Step 3: Promote `tempfile` to a runtime dependency**

In `Cargo.toml`, keep one declaration:

```toml
[dependencies]
tempfile = "3"
```

Remove the duplicate from `[dev-dependencies]`. Before editing, inspect the isolated worktree's `Cargo.toml`; do not overwrite unrelated dependency changes from a newer base commit.

- [ ] **Step 4: Implement complete payload reconstruction**

Create `sync/snapshot.rs` with:

```rust
pub(super) fn rebuild_complete_season_payload(
    db: &OwDatabase,
    season: i64,
    current_fetch_payload: &Value,
) -> Result<Value>
```

Implementation requirements:

1. Query `matches.raw_entry_json` for the target season ordered by `begin_ts DESC, match_id`.
2. Query `match_details.raw_detail_json` joined to target-season matches.
3. Query `sync_errors.raw_json` for the target season.
4. Parse each row as JSON; return a contextual error instead of silently emitting malformed JSON.
5. Clone `current_fetch_payload`, replace only:

```rust
payload["matches"]["total"] = json!(entries.len());
payload["matches"]["entries"] = Value::Array(entries);
payload["matches"]["details"] = Value::Array(details);
payload["matches"]["detail_errors"] = Value::Array(errors);
```

6. Apply `sanitize_persisted_json` to the final payload before returning.

- [ ] **Step 5: Implement same-directory atomic replacement**

Replace direct `std::fs::write` in `write_season_payload` with:

```rust
let mut temporary = tempfile::NamedTempFile::new_in(output_dir)?;
serde_json::to_writer_pretty(&mut temporary, &sanitized)?;
temporary.as_file_mut().sync_all()?;
temporary.persist(&path)
    .map_err(|error| error.error)
    .with_context(|| format!("failed to replace season payload {}", path.display()))?;
```

Create the output directory first. Keep the temporary file in the same directory so replacement does not cross filesystems.

- [ ] **Step 6: Run snapshot tests and dependency checks**

Run:

```powershell
cargo test --test incremental_sync rebuilt_snapshot_contains_old_and_new_matches_without_sensitive_fields -- --exact
cargo test --test incremental_sync atomic_snapshot_write_replaces_the_old_file_without_truncation -- --exact
cargo check
```

Expected: all PASS.

- [ ] **Step 7: Commit snapshot safety**

```powershell
git add apps/owstats-desktop/src-tauri/Cargo.toml apps/owstats-desktop/src-tauri/Cargo.lock apps/owstats-desktop/src-tauri/src/sync/mod.rs apps/owstats-desktop/src-tauri/src/sync/snapshot.rs apps/owstats-desktop/src-tauri/src/sync/persist.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
git commit -m "feat(sync): rebuild complete season snapshots"
```

## Task 5: Orchestrate default incremental sync and safe diagnostics

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/incremental.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/facade.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync/persist.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/incremental_sync.rs`

- [ ] **Step 1: Add end-to-end desktop-sync assertions**

Add `spawn_all_success_incremental_server()` by copying the existing local recording-server harness and using `full_season_response`, except return HTTP 200 for `matchId=normal-fail`. Give its handle these exact counting helpers:

```rust
impl RecordingServer {
    fn clear_requests(&self) {
        self.requests.lock().unwrap().clear();
    }

    fn detail_request_count(&self) -> usize {
        self.requests.lock().unwrap().iter()
            .filter(|request| request.first_line.contains("queryMatchInfo"))
            .count()
    }

    fn list_request_count(&self) -> usize {
        self.requests.lock().unwrap().iter()
            .filter(|request| request.first_line.contains("queryMatchList"))
            .count()
    }
}
```

Add a test that writes synthetic credentials/target files exactly as the existing target-path desktop test does, then invokes `sync_season_with_credentials_and_target_paths_output_dir_base_url` twice:

```rust
#[tokio::test]
async fn desktop_sync_defaults_to_incremental_details() {
    let temp = tempfile::tempdir().unwrap();
    let app_data = temp.path().join("app-data");
    let season_dir = app_data.join("seasons");
    std::fs::create_dir_all(&app_data).unwrap();
    let credential_path = app_data.join(".dashen_credentials.json");
    let target_path = app_data.join("player_target.json");
    std::fs::write(
        &credential_path,
        r#"{"role_id":123,"token":"account-auth-token","dts":2026,"server":1}"#,
    ).unwrap();
    std::fs::write(
        &target_path,
        r#"{"inputBnet":"Tester#12345","resolvedBnet":"Tester#12345","customerToken":"player-customer-token","boundAt":"2026-07-16T00:00:00Z","sourceRoleId":123}"#,
    ).unwrap();
    let server = spawn_all_success_incremental_server();
    let db = OwDatabase::new(app_data.join("sync.sqlite3"));

    let first = sync_season_with_credentials_and_target_paths_output_dir_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        &format!("{}/customer", server.base_url),
        |_| {},
    ).await.unwrap();
    assert_eq!(first.new_matches, 3);
    assert_eq!(first.details_requested, 3);

    server.clear_requests();
    let second = sync_season_with_credentials_and_target_paths_output_dir_base_url(
        &db,
        &season_dir,
        &credential_path,
        &target_path,
        23,
        &format!("{}/customer", server.base_url),
        |_| {},
    ).await.unwrap();
    assert_eq!(second.new_matches, 0);
    assert_eq!(second.details_requested, 0);
    assert_eq!(second.database_matches, 3);
    assert_eq!(server.detail_request_count(), 0);
    assert!(server.list_request_count() > 0);

    let snapshot: Value = serde_json::from_slice(
        &std::fs::read(season_dir.join("local_s23_full_season.json")).unwrap(),
    ).unwrap();
    assert_eq!(snapshot["matches"]["details"].as_array().unwrap().len(), 3);
}
```

- [ ] **Step 2: Run the test and verify the facade still performs full detail fetch**

Run:

```powershell
cargo test --test phase8_real_fetch desktop_sync_defaults_to_incremental_details -- --exact
```

Expected: FAIL because the facade does not load inventory or expose incremental metrics.

- [ ] **Step 3: Add safe DTOs**

In `models.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncPageDiagnostic {
    pub match_kind: String,
    pub game_mode: String,
    pub page: i64,
    pub entries: usize,
    pub new_matches: usize,
    pub missing_details: usize,
    pub earliest_begin_ts: Option<i64>,
    pub latest_begin_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncDiagnostics {
    pub list_duration_ms: u64,
    pub detail_duration_ms: u64,
    pub persist_duration_ms: u64,
    pub pages: Vec<SyncPageDiagnostic>,
}
```

Extend `SyncSummary` with:

```rust
pub list_entries_seen: usize,
pub new_matches: usize,
pub missing_details: usize,
pub details_requested: usize,
pub repaired_details: usize,
pub database_matches: usize,
pub database_details: usize,
pub diagnostics: SyncDiagnostics,
```

Do not put raw IDs, player identity, credentials, or raw JSON in these DTOs.

- [ ] **Step 4: Build per-page diagnostics**

In `sync/incremental.rs`, add:

```rust
pub fn build_page_diagnostics(
    list_pages: &[Value],
    inventory: &SeasonSyncInventory,
) -> Vec<SyncPageDiagnostic>
```

For each stored page:

- Extract `kind`, `gameMode`, `page`, and the page's match entries.
- Count entries, new matches, and missing details using sets.
- Compute min/max `beginTs`.
- Do not copy `matchId` into the returned DTO.
- Sort diagnostics by `(match_kind, game_mode, page)` for deterministic tests.

- [ ] **Step 5: Change the desktop sync sequence**

In the shared implementation used by both:

- `sync_season_with_credentials_and_target_paths_output_dir_base_url`
- `sync_season_with_current_account_credentials_path_output_dir_base_url`

perform:

```rust
let inventory = load_season_sync_inventory(db, season)?;
let fetch = fetch_season_increment_with_base_url(request, &inventory.detail_ids).await?;
let entries = payload_entries(&fetch.payload);
let selection = select_detail_entries(&entries, &inventory);
let attempted_detail_ids = selection.entries.iter()
    .filter_map(|entry| entry.get("matchId").and_then(Value::as_str))
    .map(str::to_string)
    .collect::<HashSet<_>>();
```

Then:

1. Pass all fetched list entries, delta details/errors, and `attempted_detail_ids` to incremental persistence.
2. Count repaired details as successful returned detail IDs that existed in `inventory.match_ids` but not `inventory.detail_ids`.
3. Query database totals after commit.
4. Rebuild the complete snapshot from SQLite.
5. Write the snapshot only after the database transaction succeeds.
6. Populate `SyncSummary` and diagnostics.

Map a post-commit snapshot failure to a redacted, explicit error:

```rust
write_season_payload(season_output_dir, season, &complete_payload).map_err(|error| {
    anyhow!(
        "数据库更新成功，但本地快照更新失败：{}",
        crate::redaction::redact_sensitive_text(&error.to_string())
    )
})?;
```

If list fetch returns zero entries, retain the current safety behavior: do not modify SQLite. A diagnostic JSON may be written only if it contains no credentials or player identity; the preferred first-phase implementation is to return the Chinese error without writing a replacement snapshot.

- [ ] **Step 6: Emit meaningful progress phases**

Emit phases in this order:

```text
正在检查本地数据
正在获取赛季对局列表
发现 N 场新对局，M 场缺少详情
正在获取详情 X / Y
正在更新本地数据库
同步完成
```

Keep `current_match_id: None` for UI-facing progress. `processed/total` during the detail phase must mean detail requests, not full list entries.

- [ ] **Step 7: Run orchestration and privacy tests**

Run:

```powershell
cargo test --test phase8_real_fetch desktop_sync_defaults_to_incremental_details -- --exact
cargo test --test phase8_real_fetch full_season_fetch_never_persists_local_or_upstream_tokens -- --exact
cargo test --test incremental_sync -- --nocapture
```

Expected: all PASS. The second sync must issue list requests but zero detail requests.

- [ ] **Step 8: Commit orchestration**

```powershell
git add apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/sync/incremental.rs apps/owstats-desktop/src-tauri/src/sync/facade.rs apps/owstats-desktop/src-tauri/src/sync/persist.rs apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
git commit -m "feat(sync): default season updates to incremental details"
```

## Task 6: Update frontend copy and DTO contracts

**Files:**

- Create: `apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts`
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/shared/phase6Shell.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Write a focused source/format regression test**

Create:

```ts
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const types = fs.readFileSync(path.join(root, "src/shared/types.ts"), "utf8");
const onboarding = fs.readFileSync(path.join(root, "src/pages/OnboardingPage.tsx"), "utf8");

assert.match(types, /newMatches:\s*number/);
assert.match(types, /detailsRequested:\s*number/);
assert.match(types, /databaseMatches:\s*number/);
assert.match(types, /diagnostics:\s*SyncDiagnostics/);
assert.match(
  onboarding,
  /新增 \$\{summary\.newMatches\} 场.*补全 \$\{summary\.repairedDetails\} 场详情.*共 \$\{summary\.databaseMatches\} 场/s,
);
assert.doesNotMatch(
  onboarding,
  /本次导入 \$\{summary\.importedMatches\} 场/,
);

console.log("incremental season sync frontend contract passed");
```

- [ ] **Step 2: Run the test and verify DTO/copy failure**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/incrementalSeasonSync.test.ts
```

Expected: FAIL because the TypeScript DTO and copy do not contain incremental metrics.

- [ ] **Step 3: Mirror the Rust DTOs**

Add `SyncPageDiagnostic` and `SyncDiagnostics` to `types.ts` with camelCase fields matching Rust exactly. Extend `SyncSummary` with all fields added in Task 5.

- [ ] **Step 4: Update success copy**

In `runSeasonSync`, replace the old imported-list wording with:

```tsx
setSyncNotice(
  summary.newMatches === 0 && summary.missingDetails === 0
    ? `S${season} 已是最新数据，共校验 ${summary.listEntriesSeen} 场对局，无需重新获取详情。`
    : `S${season} 更新完成：新增 ${summary.newMatches} 场，补全 ${summary.repairedDetails} 场详情，当前赛季共 ${summary.databaseMatches} 场。`,
);
```

Keep `clearMatchesPageCache()` immediately after successful `syncSeason(season)`.

- [ ] **Step 5: Update progress rendering**

Change `readableSyncProgress` so a non-empty backend `phase` is the leading text and counts remain safe:

```ts
export function readableSyncProgress(progress: SyncProgress | null): string {
  if (!progress) {
    return "等待同步";
  }
  if (progress.total <= 0) {
    return progress.phase;
  }
  return `${progress.phase}：${progress.processed}/${progress.total}，失败详情 ${progress.failedDetails}`;
}
```

Update the existing `phase6Shell.test.ts` fixture expectations to the exact Chinese output.

- [ ] **Step 6: Register and run focused frontend tests**

Append `tsx scripts/incrementalSeasonSync.test.ts` to `test:unit`.

Run:

```powershell
npx tsx scripts/incrementalSeasonSync.test.ts
npx tsx scripts/phase6Shell.test.ts
npx tsx scripts/matchesPageCache.test.ts
npx tsc --noEmit
```

Expected: all PASS.

- [ ] **Step 7: Commit frontend reporting**

```powershell
git add apps/owstats-desktop/scripts/incrementalSeasonSync.test.ts apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/shared/phase6Shell.ts apps/owstats-desktop/scripts/phase6Shell.test.ts apps/owstats-desktop/package.json
git commit -m "feat(sync): report incremental season updates"
```

## Task 7: Full verification and delivery audit

**Files:**

- Modify only if verification reveals a defect in files already listed above.

- [ ] **Step 1: Verify formatting**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
```

Expected: PASS with exit code 0.

- [ ] **Step 2: Run focused Rust regression suites**

Run:

```powershell
cargo test --test incremental_sync -- --nocapture
cargo test --test phase8_real_fetch -- --nocapture
cargo test --test import_baseline sync_import_parts_is_idempotent_and_sanitizes_detail_errors -- --exact
```

Expected: all PASS.

- [ ] **Step 3: Run the complete backend test suite**

Run:

```powershell
cargo test --tests
```

Expected: PASS with zero failed tests.

- [ ] **Step 4: Run frontend tests and build**

Run:

```powershell
cd ..
npm test
npm run build
```

Expected: both PASS. If asset preparation fails with `EPERM`, stop the preview/dev server holding `public/assets`, then rerun the same command; do not treat that lock as a code failure.

- [ ] **Step 5: Verify repository hygiene**

Run from repository root:

```powershell
git diff --check
git status --short
git log --oneline -7
```

Expected:

- `git diff --check` exits 0.
- Status contains only intentional implementation files, or is clean after commits.
- No `.codebase-memory/`, credential files, browser profiles, app-data databases, raw dumps, or absolute development-machine paths are tracked.

- [ ] **Step 6: Inspect privacy-sensitive output paths**

Run:

```powershell
rg -n -i "customerToken|authorization|credential|cookie|token" apps/owstats-desktop/src-tauri/src/sync apps/owstats-desktop/src-tauri/src/season_fetch.rs apps/owstats-desktop/src-tauri/tests/incremental_sync.rs
```

Expected: matches are limited to request construction, redaction assertions, or synthetic test fixtures. Manually verify no new user-visible DTO, diagnostic, log, error, or snapshot path exposes a raw value.

- [ ] **Step 7: Optional real-upstream verification requires explicit authorization**

Do not execute a real sync by default. If the user explicitly authorizes it, report only:

- List pages requested per mode.
- List entries observed.
- Detail requests issued.
- New and repaired counts.
- Phase durations.

Do not report raw IDs, names, credentials, tokens, or raw JSON.

- [ ] **Step 8: Commit any verification-only correction**

If verification required a correction, stage the exact corrected paths shown by `git status --short`, review `git diff --cached`, and commit:

```powershell
git commit -m "fix(sync): close incremental verification gaps"
```

If no correction was needed, do not create an empty commit.

## Final delivery checklist

- Ordinary season sync defaults to full-list validation plus incremental detail requests.
- First sync requests every detail.
- A no-change second sync requests zero details.
- A new match on a later page is still discovered.
- Missing details are retried.
- Existing complete details and unaffected player indexes survive.
- Error rows are updated only for attempted matches.
- Season totals come from SQLite.
- The complete sanitized snapshot contains old and new data.
- UI copy distinguishes list validation, new matches, repaired details, and database total.
- No fixed old-page threshold exists.
- No real privacy data was accessed unless separately authorized.
