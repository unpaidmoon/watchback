# Phase 5 Scoring Field Notes

This note records which raw and projected fields support the confirmed Phase 5 scoring design. The product rules are defined in `docs/phase-5-scoring-design.md`; this file explains how those rules map to available data.

## Source Data

- Fixture source: `data/players/5479_s22_full_season.json`
- Summary source: `matches.entries[]`
- Detail source: `matches.details[]`
- Projection source: `match_players`, rebuilt from `match_details`
- Friend identity source: `friendBnetIds`
- Current analysis scope: preset/open modes only

Stadium/fight modes are out of scope until preset/open scoring is stable.

Raw match detail JSON remains the source of truth for projection repairs. Friend identity must only come from `friendBnetIds`; repeated teammates are not enough to infer friendship.

## Fields Useful For Scoring

### Match Identity And Scope

- `matchId`: stable key for cached scoring rows.
- `season`: season-scoped grouping for streaks and future baselines.
- `gameMode`: mode classifier input; cached scoring keeps mode for future preset/open split.
- `mapGuid`, `mapName`: map context for future analysis.
- `beginTs`: sort key for match list order, streaks, and recent friend hero tie-breaks.

### Result And Scoreboard

- `matchRet`: raw result input.
- `teamScore`, `opponentScore`: fallback context when result repair or close-match context is needed.

Scoring should use normalized result labels from the importer. Current importer behavior treats `1` as win and `-1` or `2` as loss, but match evaluation should not duplicate raw `matchRet` interpretation in the frontend.

### Current Player Hero

- `heroGuid`, `heroName`: current player's hero when available.

Hero identity is useful for labels, friend lineup combinations, and future same-hero baselines. First-version primary judgment does not require same-hero or personal-history baseline.

### Player Detail Rows

Detail rows under `teammateList` and `enemyList` may include:

- `bnetId`, `playerName`
- `heroGuid`, `heroName`
- `kill`, `assist`, `death`
- `heroDamage`, `cure`, `resistDamage`
- `damageTaken`, `healingTaken`
- `finalHit`, `targetCompetingTime`
- `statMap`

`damageTaken` and `healingTaken` can arrive as JSON floats. Use normalized projection values where possible, or parse integer and float forms safely in importer/projection code.

## Confirmed Scoring Decisions

The UI displays grade plus tags, not a public 0-100 score. Numeric internals can be added later for trends or threshold tuning.

Each evaluated match has:

- one primary judgment tag
- zero or more evidence tags
- at most two displayed evidence tags
- a cached confidence value
- internal reasons saved for debugging and future UI expansion

Detail pages display labels only in the first version. They do not show full scoring reasons.

The product goal is user delight and positive reinforcement. When multiple interpretations are reasonable, prefer the more positive explanation. Negative-facing tags require stronger evidence.

## Primary Judgment Matrix

| Condition | Primary tag |
| --- | --- |
| Win plus strong personal data | `带飞局` |
| Win plus weak personal data | `躺赢局` |
| Loss plus strong personal data | `尽力局` |
| Loss plus weak personal data | `背锅局` |
| Win with neither strong nor weak data | `顺风局` |
| Loss with neither strong nor weak data | `惜败局` |

Result-only fallbacks:

- win: `胜利局`
- loss: `失败局`
- draw or unknown: no scoring label

## Strong And Weak Data Rules

First-version personal data quality is based on teammate-relative ranking.

Core metrics:

- `heroDamage`
- `kill`
- `cure`
- `resistDamage`
- KDA = `(kill + assist) / max(death, 1)`

If `death` is `0`, calculate KDA as if death were `1`.

Strong personal data:

- The current player is first among teammates in any core metric.

Weak personal data:

- The current player is not first among teammates in any core metric.
- The current player has the unique highest death count among teammates.

This intentionally makes positive labels easier to trigger and negative labels less frequent.

`targetCompetingTime`, low deaths, `finalHit`, and deeper `statMap` entries can enrich future explanations but are not first-version primary triggers.

## Evidence Tags

Personal honor evidence tags:

- `KDA最高`: highest KDA among teammates.
- `击杀最多`: highest `kill` among teammates.
- `伤害最高`: highest `heroDamage` among teammates.
- `治疗最高`: highest `cure` among teammates.
- `格挡最多`: highest `resistDamage` among teammates.

These compare only against teammates, not all players in the match.

Match-nature evidence tags:

- `翻盘局`: match is won while friendly total kills are lower than enemy total kills.
- `伯仲局`: friendly/enemy total kill difference is 2 or less, and total damage difference is within 10%.

Display priority:

1. `KDA最高`
2. `击杀最多`
3. `伤害最高`
4. `治疗最高`
5. `格挡最多`
6. `翻盘局`
7. `伯仲局`

`伯仲局` is low priority and should only appear when there is room after stronger evidence tags.

## Grade Mapping

| Condition | Grade |
| --- | --- |
| `带飞局` or `尽力局` with at least 2 core evidence tags | `S` |
| `带飞局` or `尽力局` with 1 core evidence tag | `A` |
| `顺风局` or `惜败局` | `B` |
| `躺赢局` or `背锅局` | `C` |
| `胜利局` or `失败局` fallback | `B` |

## Streak Inputs

Streaks use:

- ordered `beginTs`
- normalized result
- season-scoped preset/open match list

`连胜` and `连败` start at 3 consecutive matches. They are list segment labels, not single-match tags. In reverse-chronological lists, place the streak label at the top of the streak segment and use plain text such as `3连胜` or `5连败`.

## Friend Common Hero Inputs

Friend common hero analysis uses:

- friend rows marked from `friendBnetIds`
- friend `bnetId` and `playerName`
- friend `heroGuid` and `heroName`
- normalized match result
- `beginTs`

Display rules:

- only preset/open
- sort friends by grouped match count
- show at most 5 friends
- require at least 3 grouped matches
- show one representative hero per friend
- choose representative hero by match count
- if tied, choose the most recently used hero
- show win rate as supporting context only

Example:

```text
好友A：安娜 18场 61%胜率
```

## Lineup Combination Inputs

Lineup combinations use teammate rows plus friend identity.

First-version scope:

- current player plus one or more identified friends
- no default full five-player lineup
- only preset/open

Aggregation:

- key by stable hero GUIDs and friend IDs
- sort by match count
- show win rate as supporting context
- require at least 3 matches to enter the ranked list
- hide the module if no combination reaches 3 matches

Examples:

```text
你：源氏 + 好友A：安娜，12场，58%胜率
你：温斯顿 + 好友A：安娜 + 好友B：卢西奥，5场，60%胜率
```

## Cache Inputs

Scoring is cached in SQLite during import. Scoring failure must not fail the whole import.

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

When the match list opens, stale rule versions are detected and recomputed in the background.

## Season Review Inputs

Season review consumes cached scoring output.

Positive highlights:

- `带飞局` count
- `尽力局` count
- `S` grade match count
- longest win streak
- `最常高光`

`最常高光` only counts:

- `KDA最高`
- `击杀最多`
- `伤害最高`
- `治疗最高`
- `格挡最多`

Do not count `翻盘局` or `伯仲局` as `最常高光`. Do not foreground `躺赢局` or `背锅局` in season review by default.

## Data Quality And Fallbacks

- Missing details or impossible teammate ranking: use result-only fallback when result is win/loss.
- Draw or unknown result: no scoring label.
- Missing teammate rows: skip teammate-relative evidence and lineup evidence.
- Missing friend identity: hide grouped-play modules and do not infer friends.
- Missing hero names: use GUID fallbacks.
- Scoring failure: keep the match visible and hide scoring area.
- Sensitive fields such as token, cookie, customerToken, and credential cache content must never be displayed or logged.
