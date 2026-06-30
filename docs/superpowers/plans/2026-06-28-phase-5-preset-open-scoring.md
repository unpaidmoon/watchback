# Phase 5 Preset/Open Scoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add cached preset/open match evaluation, list tags, season highlights, friend common heroes, and lineup combinations to the Tauri desktop app.

**Architecture:** The backend owns scoring and aggregation. SQLite stores per-match evaluation cache rows, import computes them opportunistically, and frontend views render DTO fields without reimplementing scoring rules.

**Tech Stack:** Tauri 2, Rust, rusqlite, serde_json, React, TypeScript, SQLite.

---

## Tasks

- [ ] Add Phase 5 backend tests covering scoring matrix, KDA, death edge cases, evidence priority, match-nature tags, fallback behavior, cache versioning, friend common heroes, lineup combinations, import cache rows, and primary tag filtering.
- [ ] Add `match_evaluation.rs` with rule versioning, per-match evaluator, cache upsert/recompute helpers, and JSON serialization for evidence/reasons.
- [ ] Extend SQLite initialization with `match_evaluations` and supporting indexes.
- [ ] Call evaluation recompute after import/projection rebuild without allowing scoring failures to abort import.
- [ ] Extend models and queries with `evaluation`, `primaryTag` filtering, stale-cache refresh command, and match detail evaluation summary.
- [ ] Extend review analytics with Phase 5 highlights.
- [ ] Extend friend analytics with common friend heroes and current-player-plus-friend lineup combinations.
- [ ] Wire frontend DTOs, filter UI, match list/detail labels, streak separators, review highlights, and friend analysis modules.
- [ ] Run `cargo fmt --check`, `cargo test --tests`, and `npm run build`; fix failures.

## Acceptance Checks

- preset/open imports generate cached scoring rows.
- Unscored, draw, and unknown-result matches hide scoring labels.
- Match list shows grade, one primary tag, and at most two evidence tags.
- Primary tag filter is single-select and limited to `带飞局`, `尽力局`, `躺赢局`, `背锅局`.
- Streak labels appear only for 3+ win/loss list segments.
- Season review shows positive Phase 5 highlights.
- Friend common heroes and lineup combinations use only identified friends and hide under-sampled data.
