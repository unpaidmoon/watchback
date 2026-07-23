# Review Choice Win-Rate Rankings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three season-scoped top-three choice rankings to the personal review page for normal perks, stadium traits, and stadium mods, ranked by the 95% Wilson lower bound.

**Architecture:** Extend the existing `SeasonAnalysis` response and calculate rankings in a focused Rust analytics module by parsing season-filtered `match_details.raw_detail_json` at query time. Render one responsive three-column React panel before map analysis, resolving names and icons through the existing packaged perk and stadium asset maps.

**Tech Stack:** Rust, rusqlite, serde_json, Tauri 2, React, TypeScript, CSS, Node/tsx contract tests.

---

## File structure

- Create `apps/owstats-desktop/src-tauri/src/analytics/choice_rankings.rs`: season query, final-loadout extraction, per-match de-duplication, aggregation, Wilson scoring, deterministic sorting.
- Modify `apps/owstats-desktop/src-tauri/src/analytics/mod.rs`: register the focused module and attach its result to `SeasonAnalysis`.
- Modify `apps/owstats-desktop/src-tauri/src/models.rs`: add serialized ranking DTOs.
- Modify `apps/owstats-desktop/src/shared/types.ts`: mirror the Rust DTOs.
- Create `apps/owstats-desktop/src-tauri/tests/review_choice_rankings.rs`: synthetic database/details coverage for the complete statistical contract.
- Create `apps/owstats-desktop/src/features/review/ChoiceRankingsPanel.tsx`: isolated panel, columns, rows, fallbacks, loading and empty states.
- Modify `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`: insert the panel immediately before `MapAnalysisModules`.
- Modify `apps/owstats-desktop/src/styles/shared.css`: responsive flat-panel styling following `design.md`.
- Create `apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts`: source/type/resource/UI contract checks.
- Modify `apps/owstats-desktop/package.json`: include the new frontend contract test.
- Modify `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`: add safe synthetic ranking data for visual rendering.

### Task 1: Add ranking DTOs and the statistical core

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/analytics/choice_rankings.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/analytics/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/review_choice_rankings.rs`

- [ ] **Step 1: Write failing Wilson and aggregation tests**

Create `review_choice_rankings.rs` with synthetic helpers that insert `matches` and `match_details`, then add tests with these assertions:

```rust
#[test]
fn wilson_ranking_penalizes_small_samples_and_is_deterministic() {
    let db = seeded_choice_db(&[
        normal_series(22, "perk-five-of-five", 5, 5),
        normal_series(22, "perk-eight-of-ten", 10, 8),
        normal_series(22, "perk-alpha", 10, 7),
        normal_series(22, "perk-beta", 10, 7),
    ]);

    let rankings = choice_rankings_for_season(&db, Some(22)).unwrap();
    let keys = rankings.normal_perks.iter().map(|row| row.key.as_str()).collect::<Vec<_>>();

    assert_eq!(keys, vec!["perk-eight-of-ten", "perk-alpha", "perk-beta"]);
    assert_eq!(rankings.min_sample, 5);
    assert_eq!(rankings.confidence_level, 0.95);
}

#[test]
fn aggregation_is_season_scoped_current_player_only_and_deduplicated_per_match() {
    let db = seeded_mixed_choice_db();
    let rankings = choice_rankings_for_season(&db, Some(22)).unwrap();

    assert_eq!(entry(&rankings.normal_perks, "my-perk").matches, 5);
    assert_eq!(entry(&rankings.normal_perks, "my-perk").wins, 3);
    assert!(rankings.normal_perks.iter().all(|row| row.key != "teammate-perk"));
    assert!(rankings.normal_perks.iter().all(|row| row.key != "season-21-perk"));
}

#[test]
fn stadium_uses_final_loadout_instead_of_round_snapshots() {
    let db = seeded_stadium_choice_db();
    let rankings = choice_rankings_for_season(&db, Some(22)).unwrap();

    assert_eq!(entry(&rankings.stadium_traits, "final-trait").matches, 5);
    assert_eq!(entry(&rankings.stadium_mods, "final-mod").matches, 5);
    assert!(rankings.stadium_traits.iter().all(|row| row.key != "round-only-trait"));
    assert!(rankings.stadium_mods.iter().all(|row| row.key != "round-only-mod"));
}
```

The fixture JSON must use only synthetic IDs and must include wins, losses, a draw, duplicate GUIDs in one final array, teammate/opponent rows, round-only choices, another season, and one malformed detail.

- [ ] **Step 2: Run the targeted test and verify RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test review_choice_rankings
```

Expected: compilation fails because `ChoiceRankings`, `ChoiceRankingEntry`, and `choice_rankings_for_season` do not exist.

- [ ] **Step 3: Add the DTOs**

Add to `models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceRankings {
    pub min_sample: i64,
    pub confidence_level: f64,
    pub normal_perks: Vec<ChoiceRankingEntry>,
    pub stadium_traits: Vec<ChoiceRankingEntry>,
    pub stadium_mods: Vec<ChoiceRankingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceRankingEntry {
    pub key: String,
    pub matches: i64,
    pub wins: i64,
    pub win_rate: f64,
}
```

- [ ] **Step 4: Implement the minimal statistical core**

In `choice_rankings.rs`, define:

```rust
use crate::db::OwDatabase;
use crate::models::{ChoiceRankingEntry, ChoiceRankings};
use anyhow::Result;
use rusqlite::params;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const MIN_SAMPLE: i64 = 5;
const WILSON_Z: f64 = 1.959_963_984_540_054;

#[derive(Default)]
struct ChoiceCounts {
    matches: i64,
    wins: i64,
}

pub(crate) fn choice_rankings_for_season(
    db: &OwDatabase,
    season: Option<i64>,
) -> Result<ChoiceRankings> {
    let selected = season.unwrap_or_default();
    let conn = db.connect()?;
    let mut stmt = conn.prepare(
        "SELECT m.bucket, m.result, d.raw_detail_json
         FROM matches m
         JOIN match_details d ON d.match_id=m.match_id
         WHERE m.season=?1 AND m.bucket IN ('preset','stadium')",
    )?;
    let rows = stmt.query_map(params![selected], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut perks = HashMap::new();
    let mut traits = HashMap::new();
    let mut mods = HashMap::new();
    for row in rows {
        let (bucket, result, raw) = row?;
        let Ok(detail) = serde_json::from_str::<Value>(&raw) else { continue };
        let won = result == "win";
        match bucket.as_str() {
            "preset" => add_once(&mut perks, extract_current_perks(&detail), won),
            "stadium" => {
                let final_loadout = extract_current_stadium_loadout(&detail);
                add_once(&mut traits, final_loadout.traits, won);
                add_once(&mut mods, final_loadout.mods, won);
            }
            _ => {}
        }
    }

    Ok(ChoiceRankings {
        min_sample: MIN_SAMPLE,
        confidence_level: 0.95,
        normal_perks: ranked(perks),
        stadium_traits: ranked(traits),
        stadium_mods: ranked(mods),
    })
}
```

Implement `extract_current_perks` by locating `/payload/data/totalCount/teammateList` (falling back to `/payload/data/teammateList`), matching the `bnetId` recorded as `is_current_player=1` in `match_players`, and collecting its `perks[*].guid`. Implement `extract_current_stadium_loadout` by selecting the last valid `roundCountList` item and then the first `teammateList` player, matching the existing `fight_list_projection` current-player convention. Both helpers return empty sets when required nodes or arrays are missing. `add_once` collects into a `HashSet<String>` before incrementing. `ranked` filters `matches >= 5`, sorts by Wilson lower bound, matches, actual rate, and key, then truncates to three. Use the existing analytics `round2` convention for `win_rate`.

- [ ] **Step 5: Run the targeted test and verify GREEN**

Run `cargo test --test review_choice_rankings`.

Expected: all choice-ranking tests pass with no real data access.

- [ ] **Step 6: Commit the statistical core**

```powershell
git add apps/owstats-desktop/src-tauri/src/analytics/choice_rankings.rs apps/owstats-desktop/src-tauri/src/analytics/mod.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/tests/review_choice_rankings.rs
git commit -m "feat: calculate season choice rankings"
```

### Task 2: Attach rankings to the season-analysis contract

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/analytics/mod.rs`
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Test: `apps/owstats-desktop/src-tauri/tests/review_choice_rankings.rs`
- Test: `apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts`

- [ ] **Step 1: Write failing backend and frontend contract assertions**

Add:

```rust
#[test]
fn season_analysis_exposes_choice_rankings_for_the_requested_season() {
    let db = seeded_mixed_choice_db();
    let analysis = get_season_analysis(&db, Some(22)).unwrap();
    assert_eq!(analysis.choice_rankings.normal_perks[0].key, "my-perk");
}
```

Create the TypeScript contract test with:

```ts
import assert from "node:assert/strict";
import fs from "node:fs";

const types = fs.readFileSync(new URL("../src/shared/types.ts", import.meta.url), "utf8");
assert.match(types, /choiceRankings: ChoiceRankings/);
assert.match(types, /normalPerks: ChoiceRankingEntry\\[\\]/);
assert.match(types, /stadiumTraits: ChoiceRankingEntry\\[\\]/);
assert.match(types, /stadiumMods: ChoiceRankingEntry\\[\\]/);
```

- [ ] **Step 2: Verify RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test review_choice_rankings season_analysis_exposes
cd ..
npx tsx scripts/reviewChoiceRankings.test.ts
```

Expected: both fail because `SeasonAnalysis.choice_rankings` and the TypeScript types are absent.

- [ ] **Step 3: Wire the contract**

Add `pub choice_rankings: ChoiceRankings` to Rust `SeasonAnalysis`, initialize it with `choice_rankings_for_season(db, selected)?`, and mirror these exact TypeScript fields:

```ts
export type ChoiceRankingEntry = {
  key: string;
  matches: number;
  wins: number;
  winRate: number;
};

export type ChoiceRankings = {
  minSample: number;
  confidenceLevel: number;
  normalPerks: ChoiceRankingEntry[];
  stadiumTraits: ChoiceRankingEntry[];
  stadiumMods: ChoiceRankingEntry[];
};
```

- [ ] **Step 4: Verify GREEN**

Run the two commands from Step 2. Expected: both pass.

- [ ] **Step 5: Commit the API contract**

```powershell
git add apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/analytics/mod.rs apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src-tauri/tests/review_choice_rankings.rs apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts
git commit -m "feat: expose choice rankings in season analysis"
```

### Task 3: Add the review panel with existing resource resolution

**Files:**
- Create: `apps/owstats-desktop/src/features/review/ChoiceRankingsPanel.tsx`
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
- Modify: `apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts`

- [ ] **Step 1: Add failing UI/resource contract assertions**

Extend the test to assert:

```ts
const review = fs.readFileSync(new URL("../src/pages/PersonalReviewPage.tsx", import.meta.url), "utf8");
const panel = fs.readFileSync(new URL("../src/features/review/ChoiceRankingsPanel.tsx", import.meta.url), "utf8");
assert.match(review, /<ChoiceRankingsPanel[\\s\\S]*?<MapAnalysisModules/);
assert.match(panel, /普通对局 · 威能/);
assert.match(panel, /角斗 · 异能/);
assert.match(panel, /角斗 · 装备/);
assert.match(panel, /至少选取 \\{rankings\\.minSample\\} 场 · 按 95% 置信统计排序/);
assert.doesNotMatch(panel, /置信下限/);
assert.match(panel, /resolvePerkInfo/);
assert.match(panel, /resolveArenaModTraitInfo/);
assert.match(panel, /样本不足，至少需要/);
```

- [ ] **Step 2: Verify RED**

Run `npx tsx scripts/reviewChoiceRankings.test.ts`.

Expected: failure because the panel does not exist.

- [ ] **Step 3: Implement the panel**

Build `ChoiceRankingsPanel` with props `{ rankings, loading, error }`. Resolve ordinary perks with the existing `resolvePerkInfo(entry.key)` and render `未知威能` when it returns `null`. Resolve both stadium categories with the existing `resolveArenaModTraitInfo(entry.key)` and use the raw `entry.key` when it returns `null`, preserving the stadium GUID exception. Render one `phase6ChoiceRankingsPanel` section, three `phase6ChoiceRankingColumn` children, and rows containing rank, fixed icon slot, display name, `formatPercent(entry.winRate)`, and `${entry.matches} 场`. A column with no rows renders `样本不足，至少需要 ${rankings.minSample} 场选取记录`; a null ranking outside loading/error renders `暂无可统计数据`.

- [ ] **Step 4: Insert it before map analysis**

In `PersonalReviewPage.tsx`, import the panel and render:

```tsx
<ChoiceRankingsPanel
  rankings={analysis?.choiceRankings ?? null}
  loading={analysisLoading}
  error={analysisError}
/>
<MapAnalysisModules
  analysis={analysis}
  loading={analysisLoading}
  error={analysisError}
  season={titleSeason}
/>
```

- [ ] **Step 5: Verify GREEN**

Run `npx tsx scripts/reviewChoiceRankings.test.ts`.

Expected: all contract assertions pass.

- [ ] **Step 6: Commit the component**

```powershell
git add apps/owstats-desktop/src/features/review/ChoiceRankingsPanel.tsx apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts
git commit -m "feat: render review choice rankings"
```

### Task 4: Style and visually fixture the selected layout

**Files:**
- Modify: `apps/owstats-desktop/src/styles/shared.css`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`
- Modify: `apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Add failing style and fixture assertions**

Assert the stylesheet contains the panel, three-column grid, separators, numeric alignment, fixed icon slot, and a `max-width: 900px` responsive rule. Assert the visual fixture has all three ranking arrays populated with synthetic GUIDs and no credential-like keys.

- [ ] **Step 2: Verify RED**

Run `npx tsx scripts/reviewChoiceRankings.test.ts`.

Expected: failure on missing CSS/fixture contracts.

- [ ] **Step 3: Add minimal design-system-compliant CSS**

Use:

```css
.phase6ChoiceRankingsPanel {
  padding: 24px;
  border: 1px solid var(--glass-border);
  border-radius: 18px;
  background: var(--glass-primary);
}

.phase6ChoiceRankingsGrid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 16px;
  border-top: 1px solid var(--glass-border);
}

.phase6ChoiceRankingColumn + .phase6ChoiceRankingColumn {
  border-left: 1px solid var(--glass-border);
}

@media (max-width: 900px) {
  .phase6ChoiceRankingsGrid { grid-template-columns: 1fr; }
  .phase6ChoiceRankingColumn + .phase6ChoiceRankingColumn {
    border-left: 0;
    border-top: 1px solid var(--glass-border);
  }
}
```

Add row rules using the existing 4/8/12/16/24 scale: `min-height: 44px`, `gap: 8px`, a `32px` fixed icon slot, `font-variant-numeric: tabular-nums` on values, and `var(--text-primary)` / `var(--text-secondary)` text. Do not add a shadow, brand color, font, or nested card surface.

- [ ] **Step 4: Populate the synthetic visual fixture and register the test**

Add three synthetic entries per category to `SeasonAnalysis.choiceRankings`. Add `tsx scripts/reviewChoiceRankings.test.ts` to `test:unit` in `package.json`.

- [ ] **Step 5: Verify GREEN**

Run:

```powershell
npx tsx scripts/reviewChoiceRankings.test.ts
npm run test:unit
```

Expected: both commands pass.

- [ ] **Step 6: Commit the visual integration**

```powershell
git add apps/owstats-desktop/src/styles/shared.css apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts apps/owstats-desktop/scripts/reviewChoiceRankings.test.ts apps/owstats-desktop/package.json
git commit -m "style: add choice ranking panel layout"
```

### Task 5: Full verification and handoff

**Files:**
- Review all files changed in Tasks 1–4.

- [ ] **Step 1: Run Rust formatting and tests**

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: exit code 0 and zero failed tests.

- [ ] **Step 2: Run frontend tests and build**

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: exit code 0 for both commands.

- [ ] **Step 3: Run repository hygiene checks**

```powershell
cd ..\..
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. Inspect `git status --short` and confirm feature files are the only changes owned by this implementation; preserve all pre-existing user changes.

- [ ] **Step 4: Review privacy boundaries**

Confirm the diff contains no token, cookie, authorization, credential, browser profile, real user dump, local SQLite, absolute `D:\Codex` runtime dependency, or `/@fs/` path. Confirm all tests and visual fixtures use synthetic identifiers.

- [ ] **Step 5: Optional release build**

Run `npm run tauri build` only when release-level verification is requested or practical. If omitted, report it explicitly as unverified.

- [ ] **Step 6: Final commit if verification caused tracked fixes**

Stage only feature-owned files and commit those fixes with:

```powershell
git commit -m "test: verify review choice rankings"
```

Do not create an empty commit.
