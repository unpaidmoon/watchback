# Match Favorites and Notes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add account-scoped local match favorites, optional notes, a favorites-only filter, list status markers, and bottom-of-detail editing for normal matches and arena summaries.

**Architecture:** Store user-authored data in a new `match_favorites` table inside each account database and expose it through focused Rust storage functions, the existing match list/detail queries, and two Tauri write commands. Keep list payloads note-free, centralize frontend draft/write behavior in a reusable hook and editor, and let the two page containers coordinate navigation guards and cache updates.

**Tech Stack:** Rust, rusqlite, serde, Tauri 2 commands, React 18, TypeScript, React Router 6, SQLite, source-contract tests, server-rendered component fixtures.

---

## Scope and execution constraints

- Source design: `docs/superpowers/specs/2026-07-16-match-favorites-notes-design.md`.
- Before execution, run `git status --short`. The current working tree has unrelated login work, including an existing modification to `apps/owstats-desktop/src-tauri/src/commands.rs`; use an isolated worktree or wait until that work is committed before editing overlapping files.
- Do not stage, commit, rewrite, or clean unrelated login changes.
- Do not read the real app-data database, real credentials, browser profiles, or real notes. Every test uses a temporary SQLite database or anonymous fixture.
- Every implementation task follows test-first order and ends with a narrow commit.

## File structure

### New files

- `apps/owstats-desktop/src-tauri/src/match_favorites.rs`
  - Owns favorite CRUD, match existence validation, note length validation, and favorite lookup.
- `apps/owstats-desktop/src-tauri/tests/match_favorites.rs`
  - Covers migration, CRUD, isolation, filtering, detail projection, cascade deletion, and privacy.
- `apps/owstats-desktop/src/features/matches/matchFavoriteState.ts`
  - Pure draft reducer and Unicode character-count helpers.
- `apps/owstats-desktop/src/features/matches/useMatchFavoriteEditor.ts`
  - Owns asynchronous favorite/note mutations, dirty state, retryable errors, and discard confirmation.
- `apps/owstats-desktop/src/features/matches/MatchFavoriteEditor.tsx`
  - Renders the bottom action strip without owning page navigation.
- `apps/owstats-desktop/src/shared/UnsavedChangesGuard.tsx`
  - Registers the active editor's synchronous discard guard and protects app-owned navigation plus window unload.
- `apps/owstats-desktop/scripts/matchFavoriteState.test.ts`
  - Tests draft transitions, Unicode limit handling, and cache/list helpers.
- `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`
  - Tests component markup and source contracts for normal/arena placement and command registration.

### Existing files to modify

- `apps/owstats-desktop/src-tauri/src/database/migrations.rs`
  - Adds schema v8.
- `apps/owstats-desktop/src-tauri/src/models.rs`
  - Adds favorite request/view fields and DTO/filter properties.
- `apps/owstats-desktop/src-tauri/src/queries/mod.rs`
  - Imports favorite lookup into detail assembly.
- `apps/owstats-desktop/src-tauri/src/queries/match_list.rs`
  - Joins favorite state into list rows.
- `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`
  - Adds favorite state to the shared normal/arena detail DTO.
- `apps/owstats-desktop/src-tauri/src/lib.rs`
  - Exports storage functions and registers Tauri commands.
- `apps/owstats-desktop/src-tauri/src/commands.rs`
  - Resolves the active account and dispatches favorite writes.
- `apps/owstats-desktop/src/shared/types.ts`
  - Mirrors Rust favorite/filter/summary/detail fields.
- `apps/owstats-desktop/src/shared/api.ts`
  - Adds favorite save/delete invocations.
- `apps/owstats-desktop/src/features/matches/matchFilters.ts`
  - Adds URL, chip, count, clear, and validation behavior.
- `apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx`
  - Adds the “收藏状态” picker.
- `apps/owstats-desktop/src/shared/matchesPageCache.ts`
  - Recognizes the URL key and provides immutable list favorite updates.
- `apps/owstats-desktop/src/pages/MatchesPage.tsx`
  - Owns embedded-detail favorite state, navigation protection, and filtered-list refresh.
- `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
  - Owns standalone-detail favorite state and route-leave protection.
- `apps/owstats-desktop/src/App.tsx`
  - Routes top-level navigation clicks through the shared unsaved-changes guard.
- `apps/owstats-desktop/src/main.tsx`
  - Mounts the guard provider inside the existing `HashRouter`.
- `apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx`
  - Passes the editor controller into compact detail.
- `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
  - Places the editor at the bottom and hides it for arena rounds.
- `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
  - Adds a non-interactive favorite status marker.
- `apps/owstats-desktop/src/styles/features/matches.css`
  - Styles the bottom editor, list marker, narrow fallback, focus, and error states.
- `apps/owstats-desktop/scripts/filterBarUi.test.ts`
  - Covers favorite filter placement and chips.
- `apps/owstats-desktop/scripts/matchesPageCache.test.ts`
  - Covers URL detection, cache keys, immutable row updates, and filtered removal.
- `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
  - Extends detail/list structural contracts.
- `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`
  - Supplies anonymous favorite states for visual verification.
- `apps/owstats-desktop/package.json`
  - Adds the two new focused tests to `test:unit` and `test:components`.

## Task 1: Add schema v8 and isolated favorite storage

**Files:**

- Create: `apps/owstats-desktop/src-tauri/tests/match_favorites.rs`
- Create: `apps/owstats-desktop/src-tauri/src/match_favorites.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/database/migrations.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing migration and CRUD tests**

Create a temporary-database helper and the first tests:

```rust
use owstats_desktop_lib::{
    delete_match_favorite, get_match_detail, get_match_favorite, list_matches,
    save_match_favorite, MatchFilters, OwDatabase,
};
use rusqlite::params;

fn insert_match(db: &OwDatabase, match_id: &str, season: i64, kind: &str) {
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result,
            map_guid, map_name, hero_guid, hero_name, begin_ts,
            team_score, opponent_score, kills, assists, deaths,
            damage, healing, mitigation, total_worth, duration_sec,
            raw_entry_json, updated_at
        ) VALUES (
            ?1, ?2, 'preset', 'SportPreset', ?3, 'win',
            'map-a', '匿名地图', 'hero-a', '匿名英雄', 1700000000000,
            3, 1, 10, 4, 2, 9000, 2000, 500, 0, 600,
            '{}', '2026-07-16T00:00:00Z'
        )
        "#,
        params![match_id, season, kind],
    )
    .unwrap();
}

#[test]
fn schema_v8_creates_match_favorites_idempotently() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 8);
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='match_favorites'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table_count, 1);
}

#[test]
fn favorite_crud_is_optional_bounded_and_cascades() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    insert_match(&db, "match-a", 23, "normal");

    let created = save_match_favorite(&db, "match-a", "").unwrap();
    assert_eq!(created.note, "");
    db.connect()
        .unwrap()
        .execute(
            "UPDATE match_favorites SET created_at='created-sentinel', updated_at='old-update' WHERE match_id='match-a'",
            [],
        )
        .unwrap();
    let updated = save_match_favorite(&db, "match-a", "第一行\n第二行").unwrap();
    assert_eq!(updated.note, "第一行\n第二行");
    assert_eq!(get_match_favorite(&db, "match-a").unwrap().unwrap().note, "第一行\n第二行");
    let timestamps: (String, String) = db
        .connect()
        .unwrap()
        .query_row(
            "SELECT created_at, updated_at FROM match_favorites WHERE match_id='match-a'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(timestamps.0, "created-sentinel");
    assert_ne!(timestamps.1, "old-update");

    delete_match_favorite(&db, "match-a").unwrap();
    delete_match_favorite(&db, "match-a").unwrap();
    assert!(get_match_favorite(&db, "match-a").unwrap().is_none());
    save_match_favorite(&db, "match-a", "重新收藏").unwrap();

    let too_long = "测".repeat(501);
    assert!(save_match_favorite(&db, "match-a", &too_long)
        .unwrap_err()
        .to_string()
        .contains("备注最多 500 字"));
    assert!(save_match_favorite(&db, "missing-match", "")
        .unwrap_err()
        .to_string()
        .contains("未找到对应对局，无法收藏。"));

    db.connect()
        .unwrap()
        .execute("DELETE FROM matches WHERE match_id='match-a'", [])
        .unwrap();
    assert!(get_match_favorite(&db, "match-a").unwrap().is_none());
}

#[test]
fn favorite_storage_is_isolated_by_account_database() {
    let temp = tempfile::tempdir().unwrap();
    let first = OwDatabase::new(temp.path().join("first.sqlite3"));
    let second = OwDatabase::new(temp.path().join("second.sqlite3"));
    insert_match(&first, "same-match", 23, "normal");
    insert_match(&second, "same-match", 23, "normal");
    save_match_favorite(&first, "same-match", "仅第一个账号").unwrap();
    assert!(get_match_favorite(&second, "same-match").unwrap().is_none());
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test match_favorites
```

Expected: compilation fails because favorite models/functions do not exist and schema version is still 7.

- [ ] **Step 3: Add schema v8**

In `database/migrations.rs`, change the version and append this entry after v7:

```rust
pub const CURRENT_SCHEMA_VERSION: i64 = 8;

Migration {
    version: 8,
    name: "match favorites and notes",
    apply: migrate_v8,
},

fn migrate_v8(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS match_favorites (
            match_id TEXT PRIMARY KEY
                REFERENCES matches(match_id) ON DELETE CASCADE,
            note TEXT NOT NULL DEFAULT ''
                CHECK(length(note) <= 500),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_match_favorites_updated
            ON match_favorites(updated_at DESC);
        "#,
    )?;
    Ok(())
}
```

- [ ] **Step 4: Add favorite models**

In `models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchFavoriteRequest {
    pub match_id: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchFavoriteView {
    pub note: String,
}
```

- [ ] **Step 5: Implement storage with validation**

Create `match_favorites.rs`:

```rust
use crate::{MatchFavoriteView, OwDatabase};
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

pub fn get_match_favorite(
    db: &OwDatabase,
    match_id: &str,
) -> Result<Option<MatchFavoriteView>> {
    db.initialize()?;
    let conn = db.connect()?;
    conn.query_row(
        "SELECT note FROM match_favorites WHERE match_id=?1",
        params![match_id],
        |row| Ok(MatchFavoriteView { note: row.get(0)? }),
    )
    .optional()
    .map_err(Into::into)
}

pub fn save_match_favorite(
    db: &OwDatabase,
    match_id: &str,
    note: &str,
) -> Result<MatchFavoriteView> {
    if note.chars().count() > 500 {
        return Err(anyhow!("备注最多 500 字"));
    }
    db.initialize()?;
    let conn = db.connect()?;
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM matches WHERE match_id=?1)",
        params![match_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(anyhow!("未找到对应对局，无法收藏。"));
    }
    let now = Utc::now().to_rfc3339();
    conn.execute(
        r#"
        INSERT INTO match_favorites (match_id, note, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?3)
        ON CONFLICT(match_id) DO UPDATE
        SET note=excluded.note, updated_at=excluded.updated_at
        "#,
        params![match_id, note, now],
    )?;
    Ok(MatchFavoriteView {
        note: note.to_string(),
    })
}

pub fn delete_match_favorite(db: &OwDatabase, match_id: &str) -> Result<()> {
    db.initialize()?;
    db.connect()?
        .execute("DELETE FROM match_favorites WHERE match_id=?1", params![match_id])?;
    Ok(())
}
```

Expose the module from `lib.rs`:

```rust
mod match_favorites;
pub use match_favorites::{
    delete_match_favorite, get_match_favorite, save_match_favorite,
};
```

- [ ] **Step 6: Run focused tests**

Run:

```powershell
cargo fmt
cargo test --test match_favorites
```

Expected: all Task 1 tests pass.

- [ ] **Step 7: Commit Task 1**

```powershell
git add apps/owstats-desktop/src-tauri/src/database/migrations.rs `
        apps/owstats-desktop/src-tauri/src/models.rs `
        apps/owstats-desktop/src-tauri/src/match_favorites.rs `
        apps/owstats-desktop/src-tauri/src/lib.rs `
        apps/owstats-desktop/src-tauri/tests/match_favorites.rs
git commit -m "feat(matches): add account-scoped favorite storage"
```

## Task 2: Project favorite state through list, detail, and filters

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/match_list.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/match_favorites.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`

- [ ] **Step 1: Write failing query tests**

Extend the test fixture so every inserted match has a minimal detail row:

```rust
fn insert_detail(db: &OwDatabase, match_id: &str, kind: &str) {
    db.connect()
        .unwrap()
        .execute(
            r#"
            INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
            VALUES (?1, ?2, '{"payload":{"data":{"teammateList":[],"enemyList":[],"roundCountList":[]}}}', '2026-07-16T00:00:00Z')
            "#,
            params![match_id, kind],
        )
        .unwrap();
}

#[test]
fn list_and_detail_project_favorite_without_exposing_note_in_list() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    insert_match(&db, "normal-favorite", 23, "normal");
    insert_detail(&db, "normal-favorite", "normal");
    insert_match(&db, "normal-plain", 23, "normal");
    insert_detail(&db, "normal-plain", "normal");
    insert_match(&db, "arena-favorite", 22, "fight");
    insert_detail(&db, "arena-favorite", "fight");
    save_match_favorite(&db, "normal-favorite", "本地私密备注").unwrap();
    save_match_favorite(&db, "arena-favorite", "").unwrap();

    let all = list_matches(&db, MatchFilters::default()).unwrap();
    assert!(all.iter().find(|row| row.match_id == "normal-favorite").unwrap().is_favorite);
    assert!(!all.iter().find(|row| row.match_id == "normal-plain").unwrap().is_favorite);

    let favorites = list_matches(
        &db,
        MatchFilters {
            favorite_only: Some(true),
            ..MatchFilters::default()
        },
    )
    .unwrap();
    assert_eq!(favorites.len(), 2);
    assert!(favorites.iter().any(|row| row.match_id == "normal-favorite"));
    assert!(favorites.iter().any(|row| row.match_id == "arena-favorite"));

    let season_23_favorites = list_matches(
        &db,
        MatchFilters {
            season: Some(23),
            favorite_only: Some(true),
            ..MatchFilters::default()
        },
    )
    .unwrap();
    assert_eq!(season_23_favorites.len(), 1);
    assert_eq!(season_23_favorites[0].match_id, "normal-favorite");

    let detail = get_match_detail(&db, "normal-favorite").unwrap();
    assert_eq!(detail.favorite.unwrap().note, "本地私密备注");
    assert!(!detail.raw_json.to_string().contains("本地私密备注"));
}
```

In `import_baseline.rs`, import `get_match_favorite` and `save_match_favorite`, then add:

```rust
#[test]
fn repeated_import_preserves_local_match_favorite() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);
    save_match_favorite(&db, "match-win", "保留这条本地备注").unwrap();
    import_synthetic_season(&db);
    assert_eq!(
        get_match_favorite(&db, "match-win").unwrap().unwrap().note,
        "保留这条本地备注"
    );
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
cargo test --test match_favorites list_and_detail_project_favorite_without_exposing_note_in_list
```

Expected: compilation fails because `favorite_only`, `is_favorite`, and `favorite` are absent.

- [ ] **Step 3: Extend Rust DTOs**

In `models.rs`:

```rust
// Add to MatchFilters:
pub favorite_only: Option<bool>,

// Add to MatchSummary:
pub is_favorite: bool,

// Add to MatchDetailView:
pub favorite: Option<MatchFavoriteView>,
```

- [ ] **Step 4: Join favorite state into the list query**

In `queries/match_list.rs`:

```rust
SELECT
    m.match_id, m.season, m.bucket, m.game_mode, m.result,
    m.map_guid, COALESCE(m.map_name, ''),
    m.hero_guid, COALESCE(m.hero_name, ''), m.begin_ts,
    m.team_score, m.opponent_score, m.kills, m.assists, m.deaths,
    m.damage, m.healing, m.mitigation, m.duration_sec,
    e.grade, e.primary_tag, e.evidence_tags_json, e.confidence,
    m.match_kind, d.raw_detail_json,
    CASE WHEN f.match_id IS NULL THEN 0 ELSE 1 END
FROM matches m
LEFT JOIN match_evaluations e ON e.match_id = m.match_id
LEFT JOIN match_details d ON d.match_id = m.match_id
LEFT JOIN match_favorites f ON f.match_id = m.match_id
```

Read the final column into:

```rust
is_favorite: row.get::<_, i64>(25)? != 0,
```

Use the actual final index after updating the select list; do not shift evaluation offsets 19-22 or the existing `match_kind`/detail indices without updating their reads.

- [ ] **Step 5: Add the conjunctive favorite filter**

At the end of `append_filters` in `queries/normal_detail.rs`:

```rust
if filters.favorite_only == Some(true) {
    sql.push_str(" AND f.match_id IS NOT NULL");
}
```

The list query owns alias `f`; no analytics query uses `MatchFilters`, so this condition stays scoped to the match-list path.

- [ ] **Step 6: Add favorite lookup to shared detail assembly**

Import `favorite_for_match` in `queries/mod.rs`:

```rust
use crate::match_favorites::favorite_for_match;
```

Add this connection-level helper in `match_favorites.rs` so detail assembly reuses its existing SQLite connection:

```rust
pub(crate) fn favorite_for_match(
    conn: &rusqlite::Connection,
    match_id: &str,
) -> Result<Option<MatchFavoriteView>> {
    conn.query_row(
        "SELECT note FROM match_favorites WHERE match_id=?1",
        params![match_id],
        |row| Ok(MatchFavoriteView { note: row.get(0)? }),
    )
    .optional()
    .map_err(Into::into)
}
```

Make `get_match_favorite` call `favorite_for_match(&conn, match_id)`.

Add to the `MatchDetailView` constructor in `normal_detail.rs`:

```rust
favorite: favorite_for_match(&conn, match_id)?,
```

This constructor is shared by normal and fight details, so no separate arena favorite lookup is added.

- [ ] **Step 7: Run focused and existing query tests**

Run:

```powershell
cargo fmt
cargo test --test match_favorites
cargo test --test import_baseline
```

Expected: all tests pass; existing list/detail projections remain unchanged except for favorite fields.

- [ ] **Step 8: Commit Task 2**

```powershell
git add apps/owstats-desktop/src-tauri/src/models.rs `
        apps/owstats-desktop/src-tauri/src/queries/mod.rs `
        apps/owstats-desktop/src-tauri/src/queries/match_list.rs `
        apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs `
        apps/owstats-desktop/src-tauri/tests/match_favorites.rs `
        apps/owstats-desktop/src-tauri/tests/import_baseline.rs
git commit -m "feat(matches): query favorite state and filters"
```

## Task 3: Register Tauri commands and frontend API contracts

**Files:**

- Create: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Modify: `apps/owstats-desktop/scripts/matchListFormat.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`

- [ ] **Step 1: Write a failing command/API contract test**

Create the first part of `matchFavoritesUi.test.tsx`:

```tsx
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const commands = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
const lib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const api = readFileSync(new URL("../src/shared/api.ts", import.meta.url), "utf8");
const types = readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");

for (const name of ["save_match_favorite", "delete_match_favorite"]) {
  assert.match(commands, new RegExp(`pub fn ${name}`));
  assert.match(lib, new RegExp(`commands::${name}`));
}
assert.match(api, /invoke<MatchFavoriteView>\("save_match_favorite"/);
assert.match(api, /invoke<void>\("delete_match_favorite"/);
assert.match(types, /favoriteOnly\?: boolean/);
assert.match(types, /isFavorite: boolean/);
assert.match(types, /favorite: MatchFavoriteView \| null/);
```

- [ ] **Step 2: Run it and verify it fails**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchFavoritesUi.test.tsx
```

Expected: assertion failure for missing commands/types/API.

- [ ] **Step 3: Add Tauri commands**

Import storage functions and models in `commands.rs`, then add:

```rust
#[tauri::command]
pub fn save_match_favorite(
    accounts: State<'_, AccountContextManager>,
    request: MatchFavoriteRequest,
) -> Result<MatchFavoriteView, String> {
    let account = active_account(&accounts)?;
    crate::save_match_favorite(&account.database(), &request.match_id, &request.note)
        .map_err(to_message)
}

#[tauri::command]
pub fn delete_match_favorite(
    accounts: State<'_, AccountContextManager>,
    match_id: String,
) -> Result<(), String> {
    let account = active_account(&accounts)?;
    crate::delete_match_favorite(&account.database(), &match_id).map_err(to_message)
}
```

Register both in `tauri::generate_handler!` immediately after `get_match_details`.

- [ ] **Step 4: Mirror the DTOs in TypeScript**

In `shared/types.ts`:

```ts
export type MatchFavoriteView = {
  note: string;
};

export type MatchFavoriteRequest = {
  matchId: string;
  note: string;
};
```

Add:

```ts
// Add to MatchFilters:
favoriteOnly?: boolean;

// Add to MatchSummary:
isFavorite: boolean;

// Add to MatchDetailView:
favorite: MatchFavoriteView | null;
```

Update every typed anonymous fixture in the four listed script files:

```ts
// Add to each MatchSummary literal:
isFavorite: false,

// Add to each MatchDetailView literal:
favorite: null,
```

Use `isFavorite: true` only for the one list-card fixture that Task 7 will assert displays a marker. Keep every fixture note anonymous.

- [ ] **Step 5: Add frontend API calls**

In `shared/api.ts`:

```ts
export function saveMatchFavorite(request: MatchFavoriteRequest) {
  return invoke<MatchFavoriteView>("save_match_favorite", { request });
}

export function deleteMatchFavorite(matchId: string) {
  return invoke<void>("delete_match_favorite", { matchId });
}
```

- [ ] **Step 6: Run contract and compiler checks**

Run:

```powershell
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsc --noEmit
cd src-tauri
cargo check
```

Expected: contract test, TypeScript compile, and Rust compile pass with all four listed fixture files updated.

- [ ] **Step 7: Commit Task 3**

Stage only the favorite hunks in `commands.rs` if unrelated login edits still exist:

```powershell
git add -p apps/owstats-desktop/src-tauri/src/commands.rs
git add apps/owstats-desktop/src-tauri/src/lib.rs `
        apps/owstats-desktop/src/shared/types.ts `
        apps/owstats-desktop/src/shared/api.ts `
        apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx `
        apps/owstats-desktop/scripts/matchListFormat.test.ts `
        apps/owstats-desktop/scripts/matchWorkbench.test.ts `
        apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx `
        apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts
git diff --cached --check
git commit -m "feat(matches): expose favorite commands"
```

## Task 4: Add favorites-only filter plumbing

**Files:**

- Modify: `apps/owstats-desktop/src/features/matches/matchFilters.ts`
- Modify: `apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx`
- Modify: `apps/owstats-desktop/src/shared/matchesPageCache.ts`
- Modify: `apps/owstats-desktop/scripts/filterBarUi.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchesPageCache.test.ts`

- [ ] **Step 1: Write failing filter tests**

Add to `filterBarUi.test.ts`:

```ts
assert.match(advancedSource, />\s*收藏状态\s*</);
assert.match(advancedSource, />\s*仅看收藏\s*</);
assert.equal(countAdvancedMatchFilters({ favoriteOnly: true }), 1);
assert.deepEqual(clearVisibleMatchFilters({ season: 23, favoriteOnly: true }), { season: 23 });
assert.deepEqual(
  activeMatchFilterChips(
    { favoriteOnly: true },
    { seasons: [], heroes: [], maps: [], results: [], modes: [] },
  ).map((chip) => chip.label),
  ["收藏：仅看收藏"],
);

const roundTrip = filtersFromParams(new URLSearchParams("season=23&favorite=1"));
assert.deepEqual(roundTrip, { season: 23, favoriteOnly: true });
assert.equal(paramsFromFilters(roundTrip).get("favorite"), "1");
assert.equal(filtersFromParams(new URLSearchParams("favorite=true")).favoriteOnly, undefined);
```

Add to `matchesPageCache.test.ts`:

```ts
assert.equal(hasExplicitMatchFilterParams(new URLSearchParams("favorite=1")), true);
assert.notEqual(matchFiltersKey({}), matchFiltersKey({ favoriteOnly: true }));
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```powershell
npx tsx scripts/filterBarUi.test.ts
npx tsx scripts/matchesPageCache.test.ts
```

Expected: failures for missing favorite parsing, UI, and explicit-filter recognition.

- [ ] **Step 3: Implement URL and chip behavior**

In `matchFilters.ts`, extend the two functions with these exact branches:

```ts
export const advancedMatchFilterKeys = [
  "result", "map", "primaryTag", "dateFrom", "dateTo", "favoriteOnly",
] as const;

export function filtersFromParams(params: URLSearchParams): MatchFilters {
  const filters: MatchFilters = {};
  if (params.get("favorite") === "1") filters.favoriteOnly = true;
  return filters;
}

export function paramsFromFilters(filters: MatchFilters, selectedMatchId?: string | null) {
  const params = new URLSearchParams();
  if (filters.favoriteOnly) params.set("favorite", "1");
  return params;
}
```

Add the chip:

```ts
if (filters.favoriteOnly) {
  chips.push({ key: "favoriteOnly", label: "收藏：仅看收藏" });
}
```

Add to `validateFilters`:

```ts
if (next.favoriteOnly !== true) delete next.favoriteOnly;
```

`removeMatchFilter` and `clearVisibleMatchFilters` already work after the type is extended.

- [ ] **Step 4: Add the advanced picker**

In `AdvancedMatchFilters.tsx`, add:

```tsx
<label>
  收藏状态
  <select
    value={props.filters.favoriteOnly ? "favorite" : ""}
    onChange={(event) => props.onPatch({
      favoriteOnly: event.target.value === "favorite" ? true : undefined,
    })}
  >
    <option value="">全部</option>
    <option value="favorite">仅看收藏</option>
  </select>
</label>
```

Change the wide grid to six columns in CSS only in Task 8; the existing narrow two-column fallback already supports six items.

- [ ] **Step 5: Recognize the URL key in cache navigation**

In `matchesPageCache.ts`:

```ts
const filterParamKeys = [
  "season", "hero", "map", "result", "mode", "primaryTag",
  "playerName", "startDate", "endDate", "favorite",
];
```

No special key logic is required because `matchFiltersKey` already sorts and serializes booleans.

Widen its reducer result type:

```ts
.reduce<Record<string, string | number | boolean>>((result, key) => {
```

- [ ] **Step 6: Run focused tests**

Run:

```powershell
npx tsx scripts/filterBarUi.test.ts
npx tsx scripts/matchesPageCache.test.ts
npx tsc --noEmit
```

Expected: all pass.

- [ ] **Step 7: Commit Task 4**

```powershell
git add apps/owstats-desktop/src/features/matches/matchFilters.ts `
        apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx `
        apps/owstats-desktop/src/shared/matchesPageCache.ts `
        apps/owstats-desktop/scripts/filterBarUi.test.ts `
        apps/owstats-desktop/scripts/matchesPageCache.test.ts
git commit -m "feat(matches): add favorites-only filter"
```

## Task 5: Build the favorite draft state, hook, and editor

**Files:**

- Create: `apps/owstats-desktop/src/features/matches/matchFavoriteState.ts`
- Create: `apps/owstats-desktop/src/features/matches/useMatchFavoriteEditor.ts`
- Create: `apps/owstats-desktop/src/features/matches/MatchFavoriteEditor.tsx`
- Create: `apps/owstats-desktop/scripts/matchFavoriteState.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`

- [ ] **Step 1: Write failing pure-state tests**

Create `matchFavoriteState.test.ts`:

```ts
import assert from "node:assert/strict";
import {
  createMatchFavoriteDraft,
  favoriteCharacterCount,
  matchFavoriteDraftReducer,
} from "../src/features/matches/matchFavoriteState.ts";

assert.equal(favoriteCharacterCount("中文🙂"), 3);

const initial = createMatchFavoriteDraft(null);
assert.deepEqual(initial, {
  savedNote: "",
  draftNote: "",
  isFavorite: false,
  busy: false,
  error: null,
  saved: false,
});

const loaded = createMatchFavoriteDraft({ note: "已保存" });
const edited = matchFavoriteDraftReducer(loaded, { type: "edit", note: "已修改" });
assert.equal(edited.draftNote, "已修改");
assert.equal(edited.draftNote !== edited.savedNote, true);

const failed = matchFavoriteDraftReducer(edited, { type: "failure", message: "保存失败" });
assert.equal(failed.draftNote, "已修改");
assert.equal(failed.error, "保存失败");

const deleted = matchFavoriteDraftReducer(loaded, { type: "deleted" });
assert.equal(deleted.isFavorite, false);
assert.equal(deleted.draftNote, "");
```

- [ ] **Step 2: Run and verify failure**

Run:

```powershell
npx tsx scripts/matchFavoriteState.test.ts
```

Expected: module-not-found failure.

- [ ] **Step 3: Implement the pure reducer**

Create `matchFavoriteState.ts` with explicit actions:

```ts
import type { MatchFavoriteView } from "../../shared/types";

export type MatchFavoriteDraft = {
  savedNote: string;
  draftNote: string;
  isFavorite: boolean;
  busy: boolean;
  error: string | null;
  saved: boolean;
};

export type MatchFavoriteDraftAction =
  | { type: "reset"; favorite: MatchFavoriteView | null }
  | { type: "edit"; note: string }
  | { type: "start" }
  | { type: "saved"; favorite: MatchFavoriteView }
  | { type: "deleted" }
  | { type: "failure"; message: string };

export function favoriteCharacterCount(value: string) {
  return Array.from(value).length;
}

export function createMatchFavoriteDraft(
  favorite: MatchFavoriteView | null,
): MatchFavoriteDraft {
  const note = favorite?.note ?? "";
  return {
    savedNote: note,
    draftNote: note,
    isFavorite: favorite !== null,
    busy: false,
    error: null,
    saved: false,
  };
}

export function matchFavoriteDraftReducer(
  state: MatchFavoriteDraft,
  action: MatchFavoriteDraftAction,
): MatchFavoriteDraft {
  switch (action.type) {
    case "reset":
      return createMatchFavoriteDraft(action.favorite);
    case "edit":
      return { ...state, draftNote: action.note, error: null, saved: false };
    case "start":
      return { ...state, busy: true, error: null, saved: false };
    case "saved":
      return {
        ...createMatchFavoriteDraft(action.favorite),
        isFavorite: true,
        saved: true,
      };
    case "deleted":
      return createMatchFavoriteDraft(null);
    case "failure":
      return { ...state, busy: false, error: action.message, saved: false };
  }
}
```

- [ ] **Step 4: Implement the reusable mutation hook**

Create `useMatchFavoriteEditor.ts`:

```ts
import { useEffect, useMemo, useReducer } from "react";
import { deleteMatchFavorite, saveMatchFavorite } from "../../shared/api";
import { productErrorMessage } from "../../shared/phase6Shell";
import type { MatchFavoriteView } from "../../shared/types";
import {
  createMatchFavoriteDraft,
  favoriteCharacterCount,
  matchFavoriteDraftReducer,
} from "./matchFavoriteState";

export type MatchFavoriteController = ReturnType<typeof useMatchFavoriteEditor>;

export function useMatchFavoriteEditor(
  matchId: string | null | undefined,
  favorite: MatchFavoriteView | null | undefined,
  onPersisted: (favorite: MatchFavoriteView | null) => void | Promise<void>,
) {
  const [state, dispatch] = useReducer(
    matchFavoriteDraftReducer,
    favorite ?? null,
    createMatchFavoriteDraft,
  );

  const dirty = state.isFavorite && state.draftNote !== state.savedNote;
  const count = useMemo(
    () => favoriteCharacterCount(state.draftNote),
    [state.draftNote],
  );

  useEffect(() => {
    const incomingNote = favorite?.note ?? "";
    const incomingFavorite = favorite !== null && favorite !== undefined;
    const alreadySynced =
      state.savedNote === incomingNote && state.isFavorite === incomingFavorite;
    if (!dirty && !alreadySynced) {
      dispatch({ type: "reset", favorite: favorite ?? null });
    }
  }, [matchId, favorite?.note, dirty, state.savedNote, state.isFavorite]);

  async function favoriteMatch() {
    if (!matchId) return;
    dispatch({ type: "start" });
    try {
      const saved = await saveMatchFavorite({ matchId, note: "" });
      dispatch({ type: "saved", favorite: saved });
      await onPersisted(saved);
    } catch (error) {
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }

  async function saveNote() {
    if (!matchId || count > 500) return;
    dispatch({ type: "start" });
    try {
      const saved = await saveMatchFavorite({ matchId, note: state.draftNote });
      dispatch({ type: "saved", favorite: saved });
      await onPersisted(saved);
    } catch (error) {
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }

  async function unfavoriteMatch() {
    if (!matchId) return;
    dispatch({ type: "start" });
    try {
      await deleteMatchFavorite(matchId);
      dispatch({ type: "deleted" });
      await onPersisted(null);
    } catch (error) {
      dispatch({ type: "failure", message: productErrorMessage(error) });
    }
  }

  function confirmDiscard() {
    if (!dirty) return true;
    if (!window.confirm("备注尚未保存，是否放弃修改？")) return false;
    dispatch({ type: "reset", favorite: { note: state.savedNote } });
    return true;
  }

  return {
    ...state,
    dirty,
    count,
    canSave: dirty && count <= 500 && !state.busy,
    edit: (note: string) => dispatch({ type: "edit", note }),
    favoriteMatch,
    saveNote,
    unfavoriteMatch,
    confirmDiscard,
  };
}
```

Ensure `onPersisted` is stable at call sites with `useCallback`, so the reset effect does not loop.

- [ ] **Step 5: Implement the presentational editor**

Create `MatchFavoriteEditor.tsx`:

```tsx
import { Star } from "lucide-react";
import type { MatchFavoriteController } from "./useMatchFavoriteEditor";

export function MatchFavoriteEditor({
  controller,
}: {
  controller: MatchFavoriteController;
}) {
  if (!controller.isFavorite) {
    return (
      <section className="matchFavoriteEditor is-empty" aria-label="对局收藏">
        {controller.error ? <p className="matchFavoriteError">{controller.error}</p> : null}
        <button
          type="button"
          className="secondaryButton"
          disabled={controller.busy}
          onClick={() => void controller.favoriteMatch()}
        >
          <Star size={16} aria-hidden="true" />
          {controller.busy ? "正在收藏..." : "收藏此对局"}
        </button>
      </section>
    );
  }

  return (
    <section className="matchFavoriteEditor is-favorite" aria-label="对局收藏与备注">
      <div className="matchFavoriteNoteField">
        <label htmlFor="match-favorite-note">对局备注</label>
        <textarea
          id="match-favorite-note"
          value={controller.draftNote}
          disabled={controller.busy}
          onChange={(event) => controller.edit(event.target.value)}
        />
        <span className={controller.count > 500 ? "is-over-limit" : ""}>
          {controller.count}/500
        </span>
      </div>
      <div className="matchFavoriteActions">
        <button
          type="button"
          className="secondaryButton"
          disabled={controller.busy}
          onClick={() => void controller.unfavoriteMatch()}
        >
          <Star size={16} fill="currentColor" aria-hidden="true" />
          已收藏
        </button>
        <button
          type="button"
          className="primaryButton"
          disabled={!controller.canSave}
          onClick={() => void controller.saveNote()}
        >
          {controller.busy ? "正在保存..." : "保存备注"}
        </button>
        {controller.saved ? <span role="status">已保存</span> : null}
      </div>
      {controller.error ? <p className="matchFavoriteError">{controller.error}</p> : null}
    </section>
  );
}
```

Do not use `maxLength={500}` because HTML counts UTF-16 code units while the backend and counter count Unicode code points; allow typing, show over-limit state, and disable save above 500.

- [ ] **Step 6: Add server-rendered editor contracts**

Extend `matchFavoritesUi.test.tsx` to render the editor with a typed controller fixture and assert:

```tsx
assert.match(renderedUnfavorite, /收藏此对局/);
assert.doesNotMatch(renderedUnfavorite, /对局备注/);
assert.match(renderedFavorite, /对局备注/);
assert.match(renderedFavorite, /保存备注/);
assert.match(renderedFavorite, /已收藏/);
```

Also assert the source does not contain `window.localStorage`, `rawJson`, or a nested list-card button.

- [ ] **Step 7: Run focused tests**

Run:

```powershell
npx tsx scripts/matchFavoriteState.test.ts
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsc --noEmit
```

Expected: all pass.

- [ ] **Step 8: Commit Task 5**

```powershell
git add apps/owstats-desktop/src/features/matches/matchFavoriteState.ts `
        apps/owstats-desktop/src/features/matches/useMatchFavoriteEditor.ts `
        apps/owstats-desktop/src/features/matches/MatchFavoriteEditor.tsx `
        apps/owstats-desktop/scripts/matchFavoriteState.test.ts `
        apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx
git commit -m "feat(matches): add favorite note editor"
```

## Task 6: Integrate bottom placement and unsaved-navigation protection

**Files:**

- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
- Create: `apps/owstats-desktop/src/shared/UnsavedChangesGuard.tsx`
- Modify: `apps/owstats-desktop/src/App.tsx`
- Modify: `apps/owstats-desktop/src/main.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Modify: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: Write failing placement and guard contracts**

Add source assertions:

```tsx
assert.match(compactSource, /<MatchFavoriteEditor/);
assert.match(detailPageSource, /<MatchFavoriteEditor/);
assert.match(compactSource, /selected\.key === "summary"/);
assert.match(detailPageSource, /selectedArena\.key === "summary"/);
assert.match(matchesPageSource, /favoriteController\.confirmDiscard\(\)/);
assert.match(appSource, /guardNavigation/);
assert.match(detailPageSource, /registerUnsavedChangesGuard/);
assert.match(matchesPageSource, /registerUnsavedChangesGuard/);
assert.match(compactSource, /if \(!favoriteController\.confirmDiscard\(\)\) return/);
assert.match(detailPageSource, /if \(!favoriteController\.confirmDiscard\(\)\) return/);
```

Extend server-rendered normal and arena-summary fixtures to assert the editor appears after the team/detail content. Invoke the arena component’s round selection handler and assert the round branch omits the editor.

- [ ] **Step 2: Run tests and verify failure**

Run:

```powershell
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsx scripts/matchWorkbench.test.ts
```

Expected: missing editor/guard assertions fail.

- [ ] **Step 3: Thread the controller through detail boundaries**

Add a required `favoriteController: MatchFavoriteController` prop to:

- `MatchDetailReport`
- `MatchDetailPane`
- `CompactMatchDetail`
- `CompactArenaDetail`

Pass it through without creating a second hook inside child components.

- [ ] **Step 4: Place the editor at the real bottom**

For normal compact detail, keep the editor inside `.phase6CompactDetailScroll` after the team comparison:

```tsx
<MatchFavoriteEditor controller={favoriteController} />
```

Insert that line immediately after the closing `</section>` for `.matchDetailTeamComparison` and before the closing `</div>` for `.phase6CompactDetailScroll`.

For compact arena, insert this after the selected slice’s `.matchDetailTeamComparison`:

```tsx
{selected.key === "summary" ? (
  <MatchFavoriteEditor controller={favoriteController} />
) : null}
```

For standalone normal detail, render after `.detailContent`; for standalone arena, render after the selected summary content and only when `selectedArena.key === "summary"`.

- [ ] **Step 5: Guard arena slice switching**

In compact arena:

```ts
function selectCompactArenaSlice(nextKey: string) {
  if (nextKey !== "summary" && !favoriteController.confirmDiscard()) return;
  setSelectedKey(nextKey);
  setExpandedCompactArenaPlayerKeys(new Set());
}
```

In standalone arena:

```ts
function selectArenaSlice(nextKey: string) {
  if (nextKey !== "summary" && !favoriteController.confirmDiscard()) return;
  setSelectedArenaKey(nextKey);
}
```

Do not prompt when switching between rounds while no editor is visible and no dirty summary draft exists.

- [ ] **Step 6: Own the controller in both page containers**

In `MatchDetailPage.tsx`, use a stable persisted callback:

```ts
const handleFavoritePersisted = useCallback((favorite: MatchFavoriteView | null) => {
  setDetail((current) => current ? { ...current, favorite } : current);
}, []);

const favoriteController = useMatchFavoriteEditor(
  detail?.matchId ?? matchId,
  detail?.favorite,
  handleFavoritePersisted,
);
```

In `MatchesPage.tsx`, create the controller from `selectedDetail` and pass it to `MatchDetailPane`. The actual cache/list update callback is implemented in Task 7.

- [ ] **Step 7: Guard match selection, back actions, and route leaving**

Wrap local actions:

```ts
function selectMatch(matchId: string) {
  if (matchId !== selectedMatchIdRef.current && !favoriteController.confirmDiscard()) return;
  rememberScroll();
  setSelectedMatchIdSafely(matchId);
  setMobileDetailOpen(true);
}
```

Create `shared/UnsavedChangesGuard.tsx`:

```tsx
import {
  createContext,
  type MouseEvent,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
} from "react";

type GuardRegistration = {
  isDirty: () => boolean;
  confirmDiscard: () => boolean;
};

const UnsavedChangesContext = createContext<{
  registerUnsavedChangesGuard: (guard: GuardRegistration | null) => void;
  guardNavigation: (event?: MouseEvent<HTMLElement>) => boolean;
} | null>(null);

export function UnsavedChangesGuardProvider({ children }: { children: ReactNode }) {
  const guardRef = useRef<GuardRegistration | null>(null);
  const registerUnsavedChangesGuard = useCallback((guard: GuardRegistration | null) => {
    guardRef.current = guard;
  }, []);
  const guardNavigation = useCallback((event?: MouseEvent<HTMLElement>) => {
    const guard = guardRef.current;
    const allowed = !guard?.isDirty() || guard.confirmDiscard();
    if (!allowed) event?.preventDefault();
    return allowed;
  }, []);

  useEffect(() => {
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!guardRef.current?.isDirty()) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, []);

  return (
    <UnsavedChangesContext.Provider value={{ registerUnsavedChangesGuard, guardNavigation }}>
      {children}
    </UnsavedChangesContext.Provider>
  );
}

export function useUnsavedChangesGuard() {
  const value = useContext(UnsavedChangesContext);
  if (!value) throw new Error("UnsavedChangesGuardProvider is missing");
  return value;
}
```

In `main.tsx`, place the provider inside `HashRouter` and above `App`:

```tsx
<HashRouter>
  <UnsavedChangesGuardProvider>
    <App />
  </UnsavedChangesGuardProvider>
</HashRouter>
```

In `App`, call `useUnsavedChangesGuard()` and add this handler to every top-level `NavLink`, including “本地数据”:

```tsx
onClick={(event) => {
  guardNavigation(event);
}}
```

In both page containers, register only while dirty:

```ts
const { registerUnsavedChangesGuard } = useUnsavedChangesGuard();

useEffect(() => {
  registerUnsavedChangesGuard(
    favoriteController.dirty
      ? {
          isDirty: () => true,
          confirmDiscard: favoriteController.confirmDiscard,
        }
      : null,
  );
  return () => registerUnsavedChangesGuard(null);
}, [
  favoriteController.dirty,
  favoriteController.confirmDiscard,
  registerUnsavedChangesGuard,
]);
```

Guard explicit back callbacks before navigating or closing the mobile detail. The current app uses `HashRouter`, so do not call `useBlocker`, which requires a data router, and do not migrate the router for this feature. The provider protects app-owned navigation and window close/reload; local match and arena navigation remain explicitly guarded. Do not add a custom modal; use the approved native confirmation text.

- [ ] **Step 8: Run focused interaction contracts**

Run:

```powershell
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsx scripts/matchWorkbench.test.ts
npx tsc --noEmit
```

Expected: bottom placement, summary-only rendering, and guard contracts pass.

- [ ] **Step 9: Commit Task 6**

```powershell
git add apps/owstats-desktop/src/pages/MatchesPage.tsx `
        apps/owstats-desktop/src/pages/MatchDetailPage.tsx `
        apps/owstats-desktop/src/shared/UnsavedChangesGuard.tsx `
        apps/owstats-desktop/src/App.tsx `
        apps/owstats-desktop/src/main.tsx `
        apps/owstats-desktop/src/features/matches/MatchDetailPane.tsx `
        apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx `
        apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx `
        apps/owstats-desktop/scripts/matchWorkbench.test.ts
git commit -m "feat(matches): place favorite editor in match details"
```

## Task 7: Keep list markers and page cache consistent

**Files:**

- Modify: `apps/owstats-desktop/src/shared/matchesPageCache.ts`
- Modify: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/MatchListCard.tsx`
- Modify: `apps/owstats-desktop/scripts/matchFavoriteState.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchesPageCache.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: Write failing immutable-update tests**

Add:

```ts
import { updateMatchFavoriteState } from "../src/shared/matchesPageCache.ts";

const rows = [
  { matchId: "a", isFavorite: false },
  { matchId: "b", isFavorite: true },
];
const updated = updateMatchFavoriteState(rows, "a", true);
assert.deepEqual(updated, [
  { matchId: "a", isFavorite: true },
  { matchId: "b", isFavorite: true },
]);
assert.notEqual(updated, rows);
assert.equal(updated[1], rows[1]);
```

Add source/render assertions that a favorited list row exposes the accessible name “已收藏”, contains no remark text, and has no nested button.

- [ ] **Step 2: Run tests and verify failure**

Run:

```powershell
npx tsx scripts/matchFavoriteState.test.ts
npx tsx scripts/matchesPageCache.test.ts
npx tsx scripts/matchWorkbench.test.ts
```

Expected: missing helper and marker assertions fail.

- [ ] **Step 3: Add immutable cache helper**

In `matchesPageCache.ts`:

```ts
export function updateMatchFavoriteState<T extends { matchId: string; isFavorite: boolean }>(
  matches: T[],
  matchId: string,
  isFavorite: boolean,
) {
  return matches.map((match) =>
    match.matchId === matchId && match.isFavorite !== isFavorite
      ? { ...match, isFavorite }
      : match
  );
}
```

- [ ] **Step 4: Add the non-interactive list marker**

Inside `MatchListCard`’s existing header span:

```tsx
{match.isFavorite ? (
  <span className="matchListFavoriteMark" title="已收藏" aria-label="已收藏">
    <Star size={15} fill="currentColor" aria-hidden="true" />
  </span>
) : null}
```

Import `Star` from `lucide-react`. Do not add an `onClick` and do not render another button inside the card button.

- [ ] **Step 5: Update detail, list, and cache after persistence**

In `MatchesPage.tsx`, use a stable callback:

```ts
const handleFavoritePersisted = useCallback(async (favorite: MatchFavoriteView | null) => {
  setSelectedDetail((current) => current ? { ...current, favorite } : current);
  const matchId = selectedMatchIdRef.current;
  if (!matchId) return;

  if (currentFiltersRef.current.favoriteOnly && favorite === null) {
    await loadMatches(currentFiltersRef.current);
    return;
  }

  setMatches((current) => {
    const next = updateMatchFavoriteState(current, matchId, favorite !== null);
    updateMatchesPageCache({ matches: next });
    return next;
  });
}, []);
```

Use the existing `currentFiltersRef` rather than capturing stale `filters`. Ensure `loadMatches` retains existing request-id protection and `chooseMatchIdForLoadedMatches` chooses the next row after a favorite-only removal.

- [ ] **Step 6: Run focused cache and list tests**

Run:

```powershell
npx tsx scripts/matchFavoriteState.test.ts
npx tsx scripts/matchesPageCache.test.ts
npx tsx scripts/matchWorkbench.test.ts
npx tsc --noEmit
```

Expected: all pass; existing scroll/selection cache contracts remain green.

- [ ] **Step 7: Commit Task 7**

```powershell
git add apps/owstats-desktop/src/shared/matchesPageCache.ts `
        apps/owstats-desktop/src/pages/MatchesPage.tsx `
        apps/owstats-desktop/src/features/matches/MatchListCard.tsx `
        apps/owstats-desktop/scripts/matchFavoriteState.test.ts `
        apps/owstats-desktop/scripts/matchesPageCache.test.ts `
        apps/owstats-desktop/scripts/matchWorkbench.test.ts
git commit -m "feat(matches): keep favorite list state in sync"
```

## Task 8: Apply approved visual treatment and anonymous fixtures

**Files:**

- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx`
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx`
- Modify: `apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`

- [ ] **Step 1: Write failing CSS and fixture contracts**

Add source assertions:

```ts
assert.match(matchesCss, /\.matchFavoriteEditor\s*\{/);
assert.match(matchesCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto/);
assert.match(matchesCss, /@media \(max-width:\s*1099px\)[\s\S]*?\.matchFavoriteEditor/);
assert.match(matchesCss, /\.matchListFavoriteMark\s*\{/);
assert.match(matchesCss, /\.advancedMatchFilters\.phase6FilterRowSecondary[\s\S]*?repeat\(6/);
```

Update the visual fixture test to require:

- normal favorite editor after the detail tables,
- arena summary favorite editor,
- no real IDs or real notes,
- no horizontal overflow marker in narrow-detail output.

- [ ] **Step 2: Run visual contract tests and verify failure**

Run:

```powershell
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/matchWorkbenchVisualFixture.test.ts
```

Expected: CSS/fixture assertions fail.

- [ ] **Step 3: Add design-token-based styles**

Add to `matches.css`:

```css
.matchFavoriteEditor {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--space-3);
  align-items: end;
  margin-top: var(--space-4);
  padding: var(--space-4);
  border-top: 1px solid var(--border-subtle);
  background: color-mix(in srgb, var(--surface-secondary) 72%, transparent);
}

.matchFavoriteEditor.is-empty {
  display: flex;
  justify-content: flex-end;
}

.matchFavoriteNoteField {
  display: grid;
  gap: var(--space-2);
  min-width: 0;
}

.matchFavoriteNoteField textarea {
  min-height: 88px;
  resize: vertical;
  border: 1px solid var(--glass-border);
  border-radius: var(--radius-control);
  background: var(--surface-control);
  color: var(--text-primary);
}

.matchFavoriteActions {
  display: grid;
  gap: var(--space-2);
}

.matchFavoriteError,
.matchFavoriteNoteField .is-over-limit {
  color: var(--status-danger);
}

.matchListFavoriteMark {
  position: absolute;
  top: var(--space-3);
  right: var(--space-3);
  display: inline-flex;
  color: var(--text-primary);
}

.advancedMatchFilters.phase6FilterRowSecondary {
  grid-template-columns: repeat(6, minmax(0, 1fr));
}

@media (max-width: 1099px) {
  .matchFavoriteEditor {
    grid-template-columns: minmax(0, 1fr);
  }

  .matchFavoriteActions {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .advancedMatchFilters.phase6FilterRowSecondary {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
```

Use the exact existing tokens shown above: `--border-subtle`, `--status-danger`, `--radius-control`, `--surface-control`, and `--surface-secondary`. Do not introduce new colors, shadows, radii, or fonts.

- [ ] **Step 4: Update anonymous visual fixtures**

Add `isFavorite` to every `MatchSummary` fixture and `favorite` to every `MatchDetailView` fixture. Use notes such as:

```ts
favorite: { note: "第二波团战站位值得复盘，注意先处理侧翼。" }
```

Do not copy any real account display name, match ID, or note.

Pass a deterministic favorite controller fixture into both full and compact detail renders. Keep the existing fixture-state union unchanged, and render the 500-character note state in `matchFavoritesUi.test.tsx`.

- [ ] **Step 5: Run focused visual tests and generate fixtures**

Run:

```powershell
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/matchWorkbenchVisualFixture.test.ts
```

Expected: all pass and generated fixture HTML contains only anonymous content.

- [ ] **Step 6: Inspect the real fixture at required viewports**

Start the existing fixture/preview path used by `matchWorkbenchVisualFixture.test.ts`, then inspect:

- `2540×1344`
- `1920×1080`
- `1440×900`

Confirm:

- editor is after all detail content,
- arena rounds omit it,
- narrow detail stacks vertically without horizontal scrolling,
- list card height remains `152px`,
- favorite marker does not cover map/mode copy,
- 500-character notes, errors, and save state do not create nested scrolling.

- [ ] **Step 7: Commit Task 8**

```powershell
git add apps/owstats-desktop/src/styles/features/matches.css `
        apps/owstats-desktop/src/features/matches/AdvancedMatchFilters.tsx `
        apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.tsx `
        apps/owstats-desktop/scripts/matchWorkbenchVisualFixture.test.ts `
        apps/owstats-desktop/scripts/matchWorkbench.test.ts `
        apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx
git commit -m "style(matches): polish favorite editor and markers"
```

## Task 9: Register focused tests and run full verification

**Files:**

- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Register focused tests**

Replace the two script values with:

```json
"test:unit": "tsx scripts/rankingUtils.test.ts && tsx scripts/reviewNavigation.test.ts && tsx scripts/uiVisualContracts.test.ts && tsx scripts/uiVisualAcceptance.test.ts && tsx scripts/careerMmrUi.test.ts && tsx scripts/rankingDisplayNames.test.ts && tsx scripts/filterBarUi.test.ts && tsx scripts/matchesPageCache.test.ts && tsx scripts/reviewAchievements.test.ts && tsx scripts/matchListFormat.test.ts && tsx scripts/matchDetailHeaderFormat.test.ts && tsx scripts/arenaDetailFormat.test.ts && tsx scripts/playerHighlights.test.ts && tsx scripts/matchFavoriteState.test.ts",
"test:components": "tsx scripts/stab006ModuleContracts.test.ts && tsx scripts/visualSystem.test.ts && tsx scripts/pageShellComponents.test.tsx && tsx scripts/accountManagement.test.tsx && tsx scripts/accountOperationIsolation.test.ts && tsx scripts/accountScopedFrontendCaches.test.ts && tsx scripts/matchWorkbenchVisualFixture.test.ts && tsx scripts/matchFavoritesUi.test.tsx"
```

- [ ] **Step 2: Run all focused checks**

```powershell
cd apps\owstats-desktop
npx tsx scripts/filterBarUi.test.ts
npx tsx scripts/matchesPageCache.test.ts
npx tsx scripts/matchFavoriteState.test.ts
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/matchWorkbenchVisualFixture.test.ts

cd src-tauri
cargo test --test match_favorites
cargo test --test import_baseline
```

Expected: all pass.

- [ ] **Step 3: Run complete frontend verification**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: all resource, unit, component, legacy, text, TypeScript, and Vite build checks pass.

- [ ] **Step 4: Run complete backend verification**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: formatting and all integration tests pass.

- [ ] **Step 5: Run privacy and diff checks**

```powershell
cd D:\Codex\OW_data\owstats-local
rg -n "token|customerToken|authorization|credential" `
  apps/owstats-desktop/src/features/matches/MatchFavoriteEditor.tsx `
  apps/owstats-desktop/src/features/matches/useMatchFavoriteEditor.ts `
  apps/owstats-desktop/src-tauri/src/match_favorites.rs
git diff --check
git status --short
```

Expected:

- privacy search finds no sensitive field handling in favorite code,
- `git diff --check` exits 0,
- status contains only intended favorite feature files plus the user’s pre-existing unrelated files,
- no `.superpowers/` visual-companion artifacts are tracked.

- [ ] **Step 6: Run package verification only for release delivery**

If this implementation is being delivered as a release candidate:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

Expected: Tauri build succeeds. If `public/assets` reports `EPERM`, stop the preview/dev server and retry once.

- [ ] **Step 7: Commit verification registration**

```powershell
git add apps/owstats-desktop/package.json
git commit -m "test(matches): cover favorites and notes"
```

- [ ] **Step 8: Report delivery evidence**

Report:

- commit list,
- exact focused and full commands run,
- pass/fail totals,
- whether `npm run tauri build` ran,
- remaining unverified items,
- confirmation that no real credentials, real notes, app-data SQLite, or private player IDs were read.
