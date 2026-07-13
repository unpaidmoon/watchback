# Ranking Dynamic Seasons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the ranking page season selector reflect every preset season currently stored in the local database.

**Architecture:** Extend `RankingResponse` with a descending `availableSeasons` list computed by the ranking backend. Validate the requested season against that list, return rankings for the selected or latest available season, and render the response-owned list in `RankingPage`.

**Tech Stack:** Rust, rusqlite, serde, React, TypeScript, Tauri 2.

---

### Task 1: Pin the backend season contract

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_multi_season_pages.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src-tauri/src/ranking.rs`

- [ ] Add S15 and S20 preset rows to the existing multi-season ranking fixture and assert `available_seasons == [21, 20, 15]`.
- [ ] Assert an absent requested season falls back to S21 and only returns S21 aggregates.
- [ ] Run `cargo test --test phase8_multi_season_pages` and confirm the new assertions fail because `RankingResponse` does not yet expose dynamic seasons.
- [ ] Add `available_seasons: Vec<i64>` / `availableSeasons: number[]` to the aligned Rust and TypeScript DTOs.
- [ ] Query distinct preset seasons in descending order, validate the requested season against them, and use the newest available season as fallback.
- [ ] Re-run `cargo test --test phase8_multi_season_pages` and confirm it passes.

### Task 2: Render response-owned seasons

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
- Modify: `apps/owstats-desktop/src/pages/RankingPage.tsx`

- [ ] Add source-level assertions that the ranking page maps `ranking.availableSeasons`, contains no fixed `selectableSeasons`, and accepts positive integer URL seasons.
- [ ] Run `npx tsx scripts/phase8MultiSeasonPages.test.ts` and confirm the new assertions fail against the fixed `[23, 22, 21]` implementation.
- [ ] Replace the fixed selector with `ranking?.availableSeasons ?? []`, disable it when empty, and parse any positive integer season.
- [ ] When the backend returns a fallback season, synchronize component state and the ranking URL query without changing the current tab.
- [ ] Re-run `npx tsx scripts/phase8MultiSeasonPages.test.ts` and confirm it passes.

### Task 3: Verify the completed slice

**Files:**
- Verify only: all files above plus existing user changes.

- [ ] Run `cargo fmt --check` and `cargo test --test phase8_multi_season_pages` from `apps/owstats-desktop/src-tauri`.
- [ ] Run `npm test` and `npm run build` from `apps/owstats-desktop`.
- [ ] Run `git diff --check` and inspect `git status --short` to confirm this task only added the planned files beside pre-existing user changes.
- [ ] Do not stage or commit because the repository collaboration instructions prohibit doing so without an explicit request.
