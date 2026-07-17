# Raw Match Fields

This is the compact field index for the Tauri/Rust app. The source fixture is:

```text
data/players/5479_s22_full_season.json
```

## Top-Level Shape

- `profile_card`: current player profile payload.
- `matches.entries[]`: season match summaries.
- `matches.details[]`: per-match detail payloads.
- `matches.detail_errors[]`: detail fetch failures; raw JSON and error text retain naturally occurring upstream content.
- `friendBnetIds`: friend identity source. Do not infer friends from repeated teammates.

## Field Dictionary Reference

The Overwatch config dictionary is stored in:

```text
docs/reference/ow_config_cache.json
```

It currently contains:

- `heroes`: hero GUID to Chinese hero name and role type.
- `maps`: map GUID to Chinese map name and mode label.
- `attrs`: stat/attribute GUID to Chinese display text and category.

Use this file as the local source of truth when translating raw GUID fields into product-facing Chinese labels.

For the full reference inventory, see:

```text
docs/reference/INDEX.md
```

That index documents `res/query_tool.json`, including `heroPerkList`, `perkList`, ranks, behavior icons, and the decimal raw perk GUID to hex dictionary ID conversion rule.

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
- `perks`: selected hero perk records. Raw rows use decimal perk GUIDs and `perkLevel`.
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

## 原始 JSON 与凭据边界

- 正常对局的 raw JSON 查看面直接返回数据库中存储的 entry 与 detail JSON，不执行递归脱敏、遮盖或重写；详情抓取错误的原始 JSON 与错误文本同样按既有错误面保留。
- 战斗/角斗对局的 raw JSON 继续为 `Null`；显示名、英雄和地图的 GUID 兜底继续遵循产品/schema 规则，不能借由原始 JSON 直通改变这些显示决策。
- 不主动把本地凭据缓存中的 `token`、`customerToken`、cookie 或 authorization 复制到账号索引、DTO、登录/后台事件、日志或原本不含它们的 payload。
