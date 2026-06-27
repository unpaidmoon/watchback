# Phase 5 Match Evaluation and Team Analysis Design

## Status

This is a short draft for Phase 5. It captures the intended product direction after the competitor research on 王者营地 and should be expanded after product review.

Phase 5 focuses on three confirmed directions:

- Per-match scoring and readable match tags.
- Match-list tag display and tag-oriented browsing.
- Friend/team analysis for grouped play, including friend hero habits and common lineup combinations.

## Goal

Phase 5 turns the match module from a historical record browser into a practical review surface. Each match should answer three questions quickly:

1. How did I perform in this match?
2. Why does the system think that?
3. What patterns appear when I play with specific friends or lineups?

The product should keep the current local-first SQLite model and add explanation-oriented data contracts on top of existing match summaries, match details, and friend projections.

## Explicit Non-Goals

- No global leaderboard or public comparison.
- No direct copy of 王者营地 scoring thresholds.
- No blame-heavy wording such as public "背锅" labels in the first version.
- No machine-learning model requirement for Phase 5; rule-based scoring is enough.
- No social sharing surface in the first version.
- No multi-account or cross-player product flow.
- No change to credential sync ownership.

## Current Context

The desktop product is `apps/owstats-desktop`, using Tauri 2, React, TypeScript, Rust, and SQLite.

Relevant current boundaries:

- Match list lives in `apps/owstats-desktop/src/pages/MatchesPage.tsx`.
- Match detail lives in `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`.
- Match timeline cards live in `apps/owstats-desktop/src/features/matches/MatchTimeline.tsx`.
- Frontend DTOs live in `apps/owstats-desktop/src/shared/types.ts`.
- Backend match queries live in `apps/owstats-desktop/src-tauri/src/queries.rs`.
- Friend analytics live in `apps/owstats-desktop/src-tauri/src/analytics.rs` and related React pages.
- `match_players` is a rebuildable projection from raw match details and already supports friend analysis.

Existing match data can already support a first scoring pass:

- Match result, mode, map, hero, begin time, scores, duration.
- Current-player kills, assists, deaths, damage, healing, mitigation.
- Detail rows for teammates and enemies.
- Friend rows via `friendBnetIds`.
- Existing season review fields such as streaks, personal bests, hero pool, and evidence links.

## Product Architecture

Add a backend-owned evaluation layer. The frontend displays evaluations but does not derive core score, tags, or friend-lineup aggregates.

Recommended backend units:

- `match_evaluation.rs`: per-match score, tag, and reason generation.
- `team_patterns.rs`: grouped-play friend hero usage and common lineup combinations.
- Existing `queries.rs` and `analytics.rs` call these modules and expose DTOs through Tauri commands.

Recommended frontend ownership:

- Match list shows compact tags on each match card.
- Match detail shows a match evaluation panel above the raw JSON section.
- Friend analysis gets a grouped-play section for friend hero habits and lineup combinations.

## Per-Match Evaluation

Each preset/open match receives a compact evaluation.

Backend DTO shape:

```text
MatchEvaluation {
  matchId
  score
  gradeLabel
  primaryTag
  tags[]
  reasons[]
  evidence[]
  confidence
}
```

Score:

- Use a 0-100 internal score.
- Display can be numeric or grade-like, for example `86` or `A`.
- Score is relative to local season context, not absolute global strength.

Core inputs:

- Win/loss/draw.
- KDA-like contribution.
- Deaths relative to same season and similar role/hero when available.
- Damage, healing, and mitigation relative to local baselines.
- Duration-adjusted values for short or long matches.
- Team-relative rank when full detail rows are available.
- Streak context from adjacent matches.

Initial tags:

- `带飞局`: win plus top-tier contribution.
- `尽力局`: loss or close loss plus high contribution.
- `躺赢局`: win plus low personal contribution relative to team/season.
- `状态低迷`: low contribution or high deaths relative to context.
- `实力局`: above-average but not dominant performance.
- `伯仲局`: close score or close team performance.
- `连胜中`: current match is part of a win streak.
- `连败中`: current match is part of a loss streak.
- `个人最佳`: this match sets or ties a tracked personal best.

Tone rules:

- Labels should be useful rather than punitive.
- `状态低迷` is preferred over `背锅局`.
- Every primary tag must have at least one reason string that explains the evidence.

## Match List Tag Browsing

Match list becomes the main browsing entry for evaluations.

Card additions:

- Primary tag chip.
- Up to two secondary chips.
- Compact score or grade.
- One short reason, for example `伤害高于赛季均值 38%`.

Filters:

- Add tag filter after the existing season, hero, map, result, mode, and date filters.
- Tag filter uses stable internal tag keys in route query parameters.
- Invalid tag query parameters are ignored.

Sorting:

- Keep default recent-first sorting.
- Do not add manual score sorting in the first version.

Empty states:

- If evaluation has not been computed, show the normal match list without fake tags.
- If a selected tag has no matches, show a short empty state with a clear filter action.

## Match Detail Evaluation Panel

Match detail should explain the evaluation before showing raw data.

Panel content:

- Score/grade.
- Primary tag and secondary tags.
- Three to five evidence rows.
- Current-player metrics versus local baseline.
- Team rank if match details are available.
- Link-like evidence anchors can point to existing player tables or raw detail sections.

Behavior:

- If detail data is missing, still show summary-based evaluation with lower confidence.
- If a metric cannot be normalized reliably, omit that evidence row.
- Raw JSON remains available for diagnostics but should not be the main review content.

## Friend Grouped-Play Analysis

Extend the existing friend analytics from "which friends correlate with outcomes" into "what happens when I play with this friend or lineup."

Friend hero habits:

- For each friend, show their most common heroes when grouped with the current player.
- Include matches, win rate, and recent trend if available.
- Support friend detail pages first; broad dashboard cards can come later.

Example display:

```text
好友 A 常用英雄
安娜: 18 场, 61.1% 胜率
朱诺: 9 场, 55.6% 胜率
雾子: 6 场, 50.0% 胜率
```

Common lineup combinations:

- Aggregate lineups from teammate detail rows in matches that include one or more known friends.
- Start with hero-name combinations, not rigid role-lock assumptions.
- Normalize lineup keys by sorted hero identifiers so the same set produces one aggregate.
- Track the current player's hero plus known friends' heroes as the first version.
- If full teammate rows are reliable, optionally include all five teammate heroes in a separate "完整阵容" section.

Lineup DTO shape:

```text
FriendLineupCombo {
  key
  label
  friendIds[]
  heroes[]
  matches
  wins
  winRate
  lastPlayedAt
  sampleLabel
  evidenceMatchId
}
```

Lineup examples:

- `你: 温斯顿 + 好友A: 安娜`
- `你: 源氏 + 好友A: 安娜 + 好友B: 卢西奥`
- `完整阵容: 温斯顿 / 猎空 / 黑影 / 安娜 / 卢西奥`

Minimum sample:

- Friend hero habits can show at 2+ matches with an explicit small-sample label.
- Lineup combinations should require at least 3 matches for ranking.
- Combos below the threshold can appear only in recent history, not in "best lineup" ranking.

## Data Contracts

Backend remains authoritative for:

- Evaluation score.
- Primary and secondary tags.
- Explanation reasons.
- Evaluation confidence.
- Friend hero aggregates.
- Friend lineup aggregates.
- Win-rate and sample labels.

Frontend may compute only presentation formatting:

- Percent formatting.
- Chip ordering if backend already provides priority.
- Simple signed deltas when backend provides both current and baseline values.

Evaluation data can be computed on demand in Phase 5. Persistent storage is optional for the first implementation, but the design should keep a future cache table possible:

```text
match_evaluations(match_id, version, score, primary_tag, tags_json, reasons_json, confidence, computed_at)
```

Versioning matters because scoring rules will change.

## Error Handling

Expected partial-data cases:

- Missing match details: return lower-confidence summary evaluation.
- Missing teammate rows: omit team-rank evidence.
- Missing friend identity data: hide grouped-play analysis and keep existing friend boards.
- Small sample: show the aggregate with a small-sample label or hide it from ranking sections.
- Unknown hero/map names: use existing GUID fallback rules.

No evaluation failure should block the match list. If evaluation fails for a match, the UI should still show the base match card.

## Acceptance Criteria

Phase 5 is accepted when:

- Match list can display score/grade and tags for local preset/open matches.
- Match list supports tag filtering without breaking existing filters.
- Each displayed primary tag has at least one visible reason.
- Match detail shows a readable evaluation panel above raw JSON.
- Missing detail data degrades to lower-confidence summary evaluation.
- Existing personal review and friend analytics still load.
- Friend detail shows common heroes used by that friend when grouped with the current player.
- Friend analysis exposes common lineup combinations with match count, win rate, and evidence match.
- Small samples are labeled rather than presented as strong conclusions.
- Backend tests cover tag assignment, score normalization, streak tags, friend hero habits, and lineup aggregation.
- Frontend build passes.

## Open Product Questions

These are intentionally left for user review before implementation planning:

- Whether the first UI should display numeric score, grade, or both.
- Whether `躺赢局` should be shown as-is or softened to a more neutral phrase.
- Whether friend lineup combinations should include only known friends or all teammates when full rows are available.
- Whether tag filters should be single-select or multi-select.
- Whether score/tag computation should be cached in SQLite in the first implementation.

## Design Self-Review

Coverage:

- The design covers the requested points 1, 2, and 5.
- The friend analysis supplement includes friend common heroes and common lineup combinations.
- The design uses existing match, detail, and friend projection data instead of adding external dependencies.

Conflict check:

- No requirement changes credential sync, multi-account behavior, or local-first storage.
- Backend remains authoritative for analytics; frontend is presentation-focused.
- Rule-based scoring is compatible with later AI summary work.

Placeholder check:

- The document has no implementation placeholders.
- The open questions are explicit product decisions for later review, not missing design sections.

Scope check:

- Phase 5 is broad but coherent because all work centers on local match interpretation and grouped-play analysis.
- The later implementation plan should split this into independent tasks: backend evaluation engine, match list DTO/UI, match detail panel, friend hero habits, lineup aggregation, tests, and docs.
