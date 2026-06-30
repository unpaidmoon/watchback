# Raw Match Fields

This is the compact field index for the Tauri/Rust app. The source fixture is:

```text
data/players/5479_s22_full_season.json
```

## Top-Level Shape

- `profile_card`: current player profile payload.
- `matches.entries[]`: season match summaries.
- `matches.details[]`: per-match detail payloads.
- `matches.detail_errors[]`: detail fetch failures; must not contain token-like values.
- `friendBnetIds`: friend identity source. Do not infer friends from repeated teammates.

## Match Summary Fields

- `matchId`: stable match id.
- `season`: season id.
- `gameMode`: mode classifier input.
- `mapGuid`, `mapName`: map identity.
- `beginTs`: match start timestamp.
- `heroGuid`, `heroName`: current player's hero when available.
- `matchRet`: result. Current importer treats `1` as win and `-1` or `2` as loss.
- `teamScore`, `opponentScore`: score fallback for result repair.

## Detail Fields

The detail payload contains player rows under:

- `teammateList`
- `enemyList`

Rows may include:

- `bnetId`, `playerName`
- `heroGuid`, `heroName`
- `kill`, `assist`, `death`
- `heroDamage`, `cure`, `resistDamage`
- `damageTaken`, `healingTaken`
- `finalHit`, `targetCompetingTime`
- `statMap`

`damageTaken` and `healingTaken` can arrive as JSON floats such as `17133.0`; import code must parse integers and floats.

## Mode Scope

Phase 2 expands preset/open modes into `match_players`:

- `LeisurePreset`
- `Leisure6v6`
- `SportPreset`
- `SportOpen`
- broad `leisure` / `sport` matches when classified as preset/open

Stadium/fight modes are excluded from phase-2 friend analytics.

## Projection Rules

`match_players` is rebuildable from `match_details`.

- Current player is matched by local/current profile bnet id.
- Friend rows are marked only by `friendBnetIds`.
- Enemy rows never enter friend boards.
- Raw detail JSON remains the source of truth for projection repairs.

## Sensitive Fields

Never display or log:

- `token`
- `customerToken`
- cookie values
- local credential cache content

Raw JSON display must redact token-like keys before rendering.
