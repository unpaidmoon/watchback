# Phase 5 Match Evaluation and Team Analysis Design

## Status

This spec is the current Phase 5 product direction for preset/open match evaluation, match-list tags, season review highlights, and grouped-play friend analysis.

The detailed scoring rules are maintained in:

- `docs/phase-5-scoring-design.md`
- `docs/phase-5-scoring-field-notes.md`

Those two documents are authoritative for threshold details, tag wording, cache fields, and confirmed product decisions.

## Goal

Phase 5 turns the match module from a historical record browser into a lightweight review surface. Each preset/open match should quickly communicate:

1. A grade and one readable judgment tag.
2. Up to two compact evidence tags.
3. Whether the player has notable streaks across adjacent matches.
4. Which friends and hero combinations appear most often in grouped play.

The product goal is positive reinforcement and user delight. The scoring system should prefer reasonable positive interpretations when the data supports more than one explanation. Negative-facing tags require stronger evidence.

## Scope

Phase 5 covers only preset/open mode data.

Out of scope for this phase:

- stadium/fight scoring.
- Global ranking or public comparison.
- Full five-player lineup analysis as the default display.
- AI-generated long-form review text.
- Public 0-100 score display.
- Credential, sync, or multi-account changes.

preset/open use the same first-version scoring rules. The scoring cache keeps mode information so future versions can split rules by mode.

## Current Project Context

The active desktop project is `apps/owstats-desktop`, using Tauri 2, React, TypeScript, Rust, and SQLite.

Relevant existing areas:

- Match list: `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- Match detail: `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
- Timeline cards: `apps/owstats-desktop/src/features/matches/MatchTimeline.tsx`
- Frontend DTOs: `apps/owstats-desktop/src/shared/types.ts`
- Backend queries: `apps/owstats-desktop/src-tauri/src/queries.rs`
- Backend analytics: `apps/owstats-desktop/src-tauri/src/analytics.rs`
- Local schema/import: `apps/owstats-desktop/src-tauri/src/db.rs` and importer modules

Raw data references:

- `docs/raw-match-fields.md`
- `data/players/5479_s22_full_season.json`

## Product Architecture

Add a backend-owned evaluation layer. The frontend renders cached evaluation output but does not derive core scoring decisions.

Recommended backend units:

- `match_evaluation.rs`: per-match grade, primary tag, evidence tags, confidence, and internal reasons.
- `team_patterns.rs`: grouped-play friend common heroes and lineup combinations.
- Existing query/import modules call these units and expose results through DTOs and Tauri commands.

Recommended frontend ownership:

- Match list displays grade, primary tag, and up to two evidence tags.
- Match list supports single-select filtering for core primary tags.
- Match detail displays labels only in the first version, not full scoring reasons.
- Personal review consumes cached scoring tags for positive season highlights.
- Friend analysis displays representative friend heroes and common grouped lineups.

## Match Evaluation Rules

Each evaluated match has:

- one grade: `S`, `A`, `B`, or `C`
- one primary tag
- zero or more evidence tags, with at most two displayed
- confidence
- internal reasons for testing and debugging

Primary tags:

| Condition | Tag |
| --- | --- |
| Win and strong personal data | `带飞局` |
| Win and weak personal data | `躺赢局` |
| Loss and strong personal data | `尽力局` |
| Loss and weak personal data | `背锅局` |
| Win and middle-state data | `顺风局` |
| Loss and middle-state data | `惜败局` |

Result-only fallbacks:

- Win with insufficient detail: `胜利局`
- Loss with insufficient detail: `失败局`
- Draw or unknown result: no scoring label

Strong personal data:

- First among teammates in any core metric.

Weak personal data:

- Not first among teammates in any core metric.
- Unique highest death count among teammates.

Core metrics:

- `heroDamage`
- `kill`
- `cure`
- `resistDamage`
- KDA = `(kill + assist) / max(death, 1)`

Death count of `0` uses denominator `1` for KDA.

## Evidence Tags

Personal honor evidence tags:

- `KDA最高`
- `击杀最多`
- `伤害最高`
- `治疗最高`
- `格挡最多`

These compare only against teammates, not all players in the match.

Match-nature evidence tags:

- `翻盘局`: win where the player's team total kills are lower than enemy team total kills.
- `伯仲局`: team total kill difference is 2 or less, and team total damage difference is within 10%.

Display priority:

1. `KDA最高`
2. `击杀最多`
3. `伤害最高`
4. `治疗最高`
5. `格挡最多`
6. `翻盘局`
7. `伯仲局`

`伯仲局` is low priority and should only appear when there is room after stronger tags.

## Grade Mapping

| Condition | Grade |
| --- | --- |
| `带飞局` or `尽力局` with at least 2 core evidence tags | `S` |
| `带飞局` or `尽力局` with 1 core evidence tag | `A` |
| `顺风局` or `惜败局` | `B` |
| `躺赢局` or `背锅局` | `C` |
| `胜利局` or `失败局` fallback | `B` |

The first version does not show a public 0-100 score.

## Streak Labels

Streaks are list separators/status labels, not single-match tags.

Rules:

- `连胜`: 3 or more consecutive wins.
- `连败`: 3 or more consecutive losses.
- Text is plain result text such as `3连胜` or `5连败`.
- In reverse-chronological lists, place the streak label at the top of the streak segment. It means the matches below form this streak.

## Match List Filtering

The first version supports single-select filtering for core primary tags only:

- `带飞局`
- `尽力局`
- `躺赢局`
- `背锅局`

Display-only labels are not filter options:

- `顺风局`
- `惜败局`
- `胜利局`
- `失败局`

Evidence tags are also display-only and not included in the first-version filter.

## SQLite Cache

Evaluation is cached in SQLite. It is not only computed transiently at read time.

Rules:

- Compute cached scoring during import.
- Scoring failure must not fail the whole import.
- Matches with scoring failure are treated as unevaluated.
- Unevaluated matches hide the scoring area in the UI.
- Cached rows include a scoring rule version.
- Opening the match list detects stale rule versions and recomputes them in the background.

First-version cache fields:

- `match_id`
- `mode`
- `rule_version`
- `grade`
- `primary_tag`
- `evidence_tags_json`
- `reasons_json`
- `confidence`
- `computed_at`

## Friend Common Heroes

Friend common hero analysis only includes preset/open matches and only uses friends identified by `friendBnetIds`.

Display rules:

- Sort friends by grouped match count.
- Show at most 5 friends.
- A friend needs at least 3 grouped matches to appear.
- Show one representative hero per friend.
- Representative hero is chosen by match count.
- If tied, choose the most recently used hero.
- Win rate is supporting context, not the primary sort key.

Example:

```text
好友A：安娜 18场 61%胜率
```

## Lineup Combinations

Lineup analysis only uses current player plus one or more identified friends.

Rules:

- Do not default to full five-player lineups.
- Prefer combinations that include multiple identified friends when available.
- Sort by match count.
- Show win rate as supporting context.
- A combination needs at least 3 matches to enter the ranked list.
- If no combination reaches 3 matches, hide the lineup module.

Examples:

```text
你：源氏 + 好友A：安娜，12场，58%胜率
你：温斯顿 + 好友A：安娜 + 好友B：卢西奥，5场，60%胜率
```

## Season Review Integration

Personal season review consumes cached scoring output and focuses on positive highlights.

Display:

- `带飞局` count
- `尽力局` count
- `S` grade match count
- longest win streak
- `最常高光`

`最常高光` only counts personal honor evidence tags:

- `KDA最高`
- `击杀最多`
- `伤害最高`
- `治疗最高`
- `格挡最多`

Do not foreground `躺赢局` or `背锅局` in the season review by default.

## Error Handling

- Missing details: use result-only fallback when possible, with lower confidence.
- Missing teammate rows: skip teammate ranking, evidence tags, and lineup evidence.
- Missing friend identity: hide friend grouped-play modules; do not infer friends.
- Unknown hero name: fall back to hero GUID.
- Scoring failure: keep match visible and hide scoring area.
- Sensitive fields such as tokens, cookies, and credential cache content must never appear in UI or logs.

## Acceptance Criteria

Phase 5 is accepted when:

- preset/open import generates cached evaluation rows.
- Match list shows grade, one primary tag, and at most two evidence tags.
- Match list hides scoring for unevaluated matches.
- Draw or unknown result shows no scoring label.
- Primary tag filter is single-select and includes only `带飞局`, `尽力局`, `躺赢局`, and `背锅局`.
- Streak labels appear for 3+ win/loss segments in the match list.
- Friend common hero module shows at most 5 qualified friends, one hero per friend.
- Lineup module shows current-player-plus-friend combinations with at least 3 matches.
- Season review shows positive scoring highlights.
- Scoring failures do not block import.
- Rule-version mismatch triggers background recomputation from the match list.
- Backend tests cover the scoring matrix, KDA zero-death handling, evidence priority, `翻盘局`, `伯仲局`, detail-missing fallback, cache versioning, friend common heroes, and lineup aggregation.

## Design Self-Review

This document is aligned with `docs/phase-5-scoring-design.md` and removes older draft assumptions that are no longer true:

- `背锅局` is now a valid primary tag.
- `躺赢局` is displayed directly.
- SQLite caching is required.
- Details do not show full scoring reasons in the first version.
- Streak labels are list segment labels, not single-match tags.
- Friend common hero and lineup analysis are restricted to preset/open.

All first-version scoring decisions captured here are resolved.
