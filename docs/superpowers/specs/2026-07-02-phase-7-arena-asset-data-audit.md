# Phase 7 Arena Asset/Data Audit

Scope: audit only. No UI, DTO, Rust business logic, or React business logic was changed.

## 1. Changed Files

- Created this report: `docs/superpowers/specs/2026-07-02-phase-7-arena-asset-data-audit.md`.
- Created arena asset candidate tree: `assets/stadium/`.
- Created manual unmapped-id worksheet: `docs/superpowers/specs/2026-07-02-stadium-unmapped-manual-map.csv`.

## Scope Deviation / Asset Import Candidate

The original prompt for this thread was audit-only. During follow-up work, this thread also created asset-import candidate files:

- `assets/stadium/` was created by this audit thread after the main audit identified that `SportFight` rank icons and arena `modTrait` icons were missing from the product asset tree.
- `docs/superpowers/specs/2026-07-02-stadium-unmapped-manual-map.csv` was created by this audit thread as a manual mapping worksheet for remaining `modGuids` and `traitGuids` ids not covered by `modTrait`.

Asset sources and counts:

- `assets/stadium/ranks/fight/`: 14 PNG files, made from 7 downloaded `rankListFight` icon URLs and 7 copied `*-pure.png` references from `docs/reference/res/rank_flat/`.
- `assets/stadium/mod-trait/`: 1080 downloaded PNG files from `docs/reference/res/query_tool.json` -> `modTrait` source URLs.
- `assets/stadium/manifest.json`: 1 JSON manifest generated from `docs/reference/res/query_tool.json` and fixture coverage probes.
- `assets/stadium/` total: 1095 files.
- Manual worksheet CSV: 26 rows, one per currently unmapped `modGuids` / `traitGuids` id, with empty manual mapping columns for later human completion.

Why this exceeds the original audit-only prompt:

- The original scope was to audit data/assets and document findings, not import new assets or create follow-up worksheets.
- The asset files and CSV were created as practical preparation for Phase 7 implementation after the audit found that existing preset/open competitive rank assets cannot be reused for `SportFight`, and that `modGuids` / `traitGuids` need a dedicated arena mapping source.

Recommended main-thread handling:

- Treat `assets/stadium/` and the CSV as a separate Phase 7 asset-import preparation candidate, not as part of the original audit-only deliverable.
- Either keep them and review/stage them under a dedicated asset-import scope, or split/clean them before accepting the audit report alone.
- Do not start UI/DTO implementation from these files until the main thread explicitly accepts the asset-import scope and resolver strategy.

## 2. Commands Run

- `Get-Location`
- `git status --short`
- `Get-Content -Encoding UTF8 docs/superpowers/specs/2026-07-02-phase-7-arena-match-detail-design.md`
- `rg --files ...`
- `rg -n ... apps/owstats-desktop/src apps/owstats-desktop/src-tauri/src apps/owstats-desktop/src-tauri/tests`
- Targeted `Get-Content -Encoding UTF8` reads for:
  - `apps/owstats-desktop/src/shared/assets.ts`
  - `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
  - `apps/owstats-desktop/src/pages/MatchesPage.tsx`
  - `apps/owstats-desktop/src/features/matches/MatchTimeline.tsx`
  - `apps/owstats-desktop/src/features/matches/FilterBar.tsx`
  - `apps/owstats-desktop/src/shared/rankingUtils.ts`
  - `apps/owstats-desktop/src/shared/playerHighlights.ts`
  - `apps/owstats-desktop/src/shared/types.ts`
  - `apps/owstats-desktop/src-tauri/src/models.rs`
  - `apps/owstats-desktop/src-tauri/src/importer.rs`
  - `apps/owstats-desktop/src-tauri/src/queries.rs`
- Aggregate-only Python fixture probes via PowerShell here-string and `python -c`.
- Aggregate-only Python asset manifest probe via PowerShell here-string and `python -c`.

One attempted Bash-style heredoc command failed under PowerShell before reading the fixture. It was replaced by the PowerShell-safe `python -c` form.

## 3. Asset Findings

Actual asset root is repo-level `assets/`, not `apps/owstats-desktop/src-tauri/assets/`. `apps/owstats-desktop/src/shared/assets.ts` imports:

- `assets/manifest.json`
- `assets/ranks/manifest.json`

Current manifest counts:

- Heroes: 52 entries under `assets/heroes/*.png`.
- Maps: 57 entries under `assets/maps/*.jpg`.
- Perk icons: 208 entries under `assets/perks/*.png`.
- Rank entries: 10 base rank entries under `assets/ranks/*.png`.
- Open queue wide rank assets: 7 files under `assets/ranks/open-wide/*.png`.

Map/background patterns:

- `assets/maps/{map-key}.jpg`
- `mapAssetUrl()` / `mapAssetUrls()` in `apps/owstats-desktop/src/shared/assets.ts`
- `MapPreview` in `apps/owstats-desktop/src/shared/phase6Components.tsx`

Fixed arena-domain background candidate:

- Best first candidate: `assets/maps/arena-victoriae.jpg`, because the key and name are explicitly arena-themed.
- Other elimination-style fallback candidates already present: `castillo`, `ecopoint-antarctica`, `black-forest`, `necropolis`.
- If the first implementation cannot confidently map a fixed arena background, fall back to the selected/last round map preview through `MapPreview`.

Hero/map/rank/icon reference points:

- Hero/map assets: `apps/owstats-desktop/src/shared/assets.ts`, `HeroAvatar`, `MapPreview`.
- Rank assets: `rankAssetUrl(rankKey)` in `apps/owstats-desktop/src/shared/assets.ts`; base rank keys are `unranked`, `bronze`, `silver`, `gold`, `platinum`, `diamond`, `master`, `grandmaster`, `champion`, `top500`.
- Perk icon assets: `resolvePerkInfo()` and `perkAssetUrls()` in `apps/owstats-desktop/src/shared/assets.ts`.

Arena-specific asset coverage:

- Arena equipment/mods: assets were added under `assets/stadium/mod-trait/` from the local reference config `docs/reference/res/query_tool.json` -> `modTrait`. Data uses `modGuids`; the field has up to 6 values per player, so it is the strongest equipment-slot candidate, not proof that equipment data is absent.
- Arena abilities/traits: assets were added under `assets/stadium/mod-trait/` from the same `modTrait` reference config. Data uses `traitGuids`; the field has up to 4 values per player, matching the planned four ability/trait slots.
- Arena equipment: no fields named `equipment`, `equipmentList`, `equips`, `item`, `goods`, `weapon`, `bag`, or `inventory` were found in the fight subtree. Treat `modGuids` as the equipment/mod candidate until label/icon mapping confirms semantics.
- Competitive rank icons: `SportFight` / arena competitive rank icons were added under `assets/stadium/ranks/fight/` from `rankListFight`. These are separate from preset/open competitive assets and should not reuse `assets/ranks/`.
- Asset manifest: `assets/stadium/manifest.json` records source URLs, local paths, labels/descriptions, and fixture coverage.

Recommended first implementation fallback:

- Equipment/mod icons: `modGuids` does not resolve through the existing perk or hero-attribute mappings, but mostly resolves through `assets/stadium/manifest.json` -> `modTrait`. Coverage against the audited fixture is 372 of 382 unique ids; 10 ids are still missing from the local reference config.
- Ability/trait icons: `traitGuids` does not resolve through the existing perk or hero-attribute mappings, but mostly resolves through `assets/stadium/manifest.json` -> `modTrait`. Coverage against the audited fixture is 340 of 356 unique ids; 16 ids are still missing from the local reference config.
- Arena stat labels: `aveStatGuids` fully resolves through the existing hero-attribute mapping, but it is a stat-label source, not an equipment icon source.
- Rank icon: use `assets/stadium/ranks/fight/`; do not reuse current preset/open competitive rank icon assets for `SportFight`.

## 4. Existing Component/Style Reuse Points

Normal match detail page:

- `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
- `MatchDetailReport` owns the header/banner, `DetailEvaluation`, `SameRoleComparison`, and vertical `PlayerTable` sections.
- Teammate/enemy rendering already uses `队友`, `VS`, `对手` in a vertical structure.
- Player row pattern is `PlayerTable` -> `PlayerCard`, with `HeroAvatar`, player name, hero name, K/A/D, damage/healing/mitigation, damage taken/healing taken.

Embedded/list-side detail:

- `apps/owstats-desktop/src/pages/MatchesPage.tsx`
- `CompactMatchDetail`, `CompactTeam`, and `CompactPlayerRow` provide the current compact right-pane player row pattern.
- `CompactPlayerRow` already resolves perk info and uses `phase6PerkTooltip` / `phase6PerkTooltipPanel`.

Tooltip reuse:

- Existing custom tooltip style is in `MatchesPage.tsx` via `phase6PerkTooltip` and `phase6PerkTooltipPanel`.
- This is the closest reusable pattern for arena ability/trait/equipment hover cards.

Win/loss styling:

- Detail header uses `phase6DetailHero ${detail.result}` in `MatchDetailPage.tsx`.
- Compact detail uses `phase6CompactDetail ${detail.result}` in `MatchesPage.tsx`.
- Timeline rows use `phase6MatchRowReal ${match.result}` in `MatchTimeline.tsx`.

Max-value highlight styling:

- Current highlight helper: `apps/owstats-desktop/src/shared/playerHighlights.ts`.
- Current CSS hook: `phase6StatHighlight`.
- Current implementation highlights computed max values for `kills`, `assists`, `damage`, `healing`, and `mitigation`; Phase 7 should align this with source flags `killMax`, `heroDamageMax`, `cureMax`, and `resistDamageMax`.

Match list subtitle/background/filter points:

- List rendering: `apps/owstats-desktop/src/features/matches/MatchTimeline.tsx`.
- Banner title/score formatting: `apps/owstats-desktop/src/shared/matchListFormat.ts`.
- Filter UI: `apps/owstats-desktop/src/features/matches/FilterBar.tsx`.
- Frontend mode labels: `apps/owstats-desktop/src/shared/rankingUtils.ts`.
- Backend mode labels: `apps/owstats-desktop/src-tauri/src/queries.rs`.
- Current labels do not include `LeisureFight` or `SportFight`; Phase 7 should add `角斗快速` and `角斗竞技`.

## 5. Redacted Fight Data Shape Findings

Fixture aggregate:

- Entries: 941.
- Details: 941.
- `match_kind = normal`: 704.
- `match_kind = fight`: 237.
- Fight modes: `LeisureFight` 6, `SportFight` 231.

Fight top-level `payload.data` fields present in all 237 fight details:

- `totalCount`
- `roundCountList`
- `matchRet`
- `customerTokenMap`
- `nameMap`
- current-player id field present but not printed

`totalCount` fields:

- Present in all fight details: `gameTimeSec`, `startTime`, `teamScore`, `opponentScore`, `teammateList`, `enemyList`, `mapGuid`.
- Not present at `totalCount` level: `score`, `matchRet`, `rankInfo`, `worth`, `endWorth`, `worthMax`.
- `teammateList` / `enemyList`: present as arrays in all 237 fight details.

`totalCount` player rows:

- Rows audited: 2373.
- Present on every row: `heroGuid`, current-player/player id field, `worth`, `endWorth`, `worthMax`, `modGuids`, `traitGuids`, `aveStatGuids`, `statMap`, `killMax`, `heroDamageMax`, `cureMax`, `resistDamageMax`.
- `rankInfo`: present on 2302 of 2373 rows, matching competitive arena coverage.
- `name`: not present on rows; display names must be resolved through `nameMap` or existing fallback logic.
- No `perks`, `equipment`, `equipmentList`, `equips`, `item`, `goods`, `weapon`, `bag`, or `inventory` fields found.
- `modGuids`, `traitGuids`, and `aveStatGuids` are arrays of string ids.
- `statMap` is an object of numeric values.
- `modGuids` has up to 6 values per player and does not resolve through existing perk or hero-attribute mappings; this is the strongest current candidate for the planned six equipment/mod slots.
- `traitGuids` has up to 4 values per player and does not resolve through existing perk or hero-attribute mappings; this matches the planned four ability/trait slots.
- `aveStatGuids` has up to 4 values per player and does resolve through existing hero-attribute mappings; use it for stat labels, not equipment.

`roundCountList`:

- Present in all 237 fight details.
- Round counts per match range from 1 to 7.
- Round count histogram: 1 round: 1 match; 2 rounds: 6; 3 rounds: 58; 4 rounds: 46; 5 rounds: 54; 6 rounds: 37; 7 rounds: 35.

Round fields:

- Present on every audited round: `gameTimeSec`, `startTime`, `teamScore`, `opponentScore`, `teammateList`, `enemyList`, `mapGuid`.
- Not present on rounds: `score`, `matchRet`, `rankInfo`, `map`.
- Round player rows audited: 11061.
- Round player rows contain the same arena economy/mod/trait/stat fields as `totalCount`.
- `endWorth` is present on most, but not all, round player rows.

## 6. Recommended `rankInfo` Display Field

Competitive-like modes with `rankInfo`:

- `SportPreset`: entry-level `rankInfo` present on all 56 audited entries.
- `SportOpen`: entry-level `rankInfo` present on all 127 audited entries.
- `SportFight`: entry-level `rankInfo` present on all 231 audited entries, and player-row rankInfo is present on nearly all competitive arena player rows.

Non-competitive-like modes without `rankInfo`:

- `LeisurePreset`
- `Leisure6v6`
- `LeisureFight`

Audited `rankInfo` keys:

- `rankName` / `rank_name`
- `rankSubTier` / `rank_sub_tier`
- `rankScore`
- `ts`

Recommendation:

- Display `rankName + rankSubTier` beside the player name, for example `大师 2` after translating `rankName` to Chinese.
- Use `rankName` to choose the rank tier family after arena rank assets are added. The existing base rank keys are useful as names, but the current icon files should not be reused for `SportFight`.
- Keep `rankScore` available as secondary tooltip/detail text, not the default inline label. It appears numeric and useful, but it is less recognizable than tier + division for the default row.

Rank icon mapping:

- Normalize English `rankName` to tier keys: `Bronze`, `Silver`, `Gold`, `Platinum`, `Diamond`, `Master`, `Grandmaster`, `Champion`, `Top 500`.
- Add a separate arena-rank asset mapping before UI implementation. Do not route `SportFight` rows to the existing preset/open rank images.
- Fall back to a Chinese text label if arena-rank icon assets are absent.

## 7. Recommended Individual-Round Win/Loss Source

Round-level `matchRet` is not present.

Recommended source:

- Use cumulative `teamScore` / `opponentScore` deltas between adjacent rounds.
- For round N, compare round N score against previous round score. If `teamScore` increased, that round is a win; if `opponentScore` increased, that round is a loss.
- For round 1, compare against `0-0`.

Fallback:

- If both scores are unchanged or malformed, treat the round result as unknown and use neutral styling. Do not infer from full-match `payload.data.matchRet` for individual rounds.

Full-match result source:

- Use `payload.data.matchRet` or the entry-level imported result for `角斗汇总`.

## 8. Missing Mappings/Assets And First-Implementation Fallback

Downloaded asset inventory:

- `assets/stadium/ranks/fight/`: 7 downloaded `rankListFight` icons plus 7 local `*-pure.png` copies from `docs/reference/res/rank_flat/`.
- `assets/stadium/mod-trait/`: 1080 downloaded `modTrait` icons, grouped by `power`, `survival`, `skill`, `weapon`, `gadget`, and `uncategorized`.
- `assets/stadium/manifest.json`: source URLs, local paths, Chinese names/descriptions, category, level, hero fields, and fixture coverage.

Still missing or incomplete:

- Dedicated frontend/backend resolver for `modGuids` and `traitGuids` using `assets/stadium/manifest.json`.
- `modGuids`: 372 of 382 unique ids in the audited fixture resolve through `modTrait`; 10 ids remain unmapped.
- `traitGuids`: 340 of 356 unique ids in the audited fixture resolve through `modTrait`; 16 ids remain unmapped.
- `aveStatGuids` is covered by the existing hero-attribute mapping and should stay a stat-label source, not an equipment/trait icon source.

Recommended development fallback:

- For `modGuids` and `traitGuids`, use fixed slots after adding the dedicated arena mapping backed by `assets/stadium/manifest.json`.
- Release-facing UI must not display raw guid values.
- If a `modGuids` lookup misses, show only a Chinese fallback such as `待补充异能`.
- If a `traitGuids` lookup misses, show only a Chinese fallback such as `待补充特质`.
- Raw guid values may exist only in development audit material, manifest coverage, or manual mapping worksheets; they must not enter the default user interface.
- For final release review, prefer hidden/missing-label fallback over raw guid display.

## 9. Privacy Checks Performed

- Did not read forbidden credential/browser-profile paths.
- Did not print `OW_DASHEN_TOKEN`, `customerToken`, `customerTokenMap` values, cookies, browser storage, or local credentials.
- Did not print raw player ids.
- Fixture probes output aggregate counts, field names, enum values, boolean/presence counts, and anonymized shapes.
- The written report intentionally omits raw player identifiers and raw fixture examples.
- Checked `assets/stadium/manifest.json` source URLs for query strings: 1087 source URLs, 0 URLs with query parameters, and therefore no token/auth/signature/query keys were present in those URLs.
- Checked `docs/superpowers/specs/2026-07-02-stadium-unmapped-manual-map.csv`: 26 rows; only `field`, `decimalId`, `hexId`, and `usageCount` are populated. Manual mapping columns are blank.
- Did not read forbidden credential/browser-profile paths during this correction.

## 10. Checks Not Run And Why

- `npm install`, `npm run build`, `cargo fmt --check`, and `cargo test --tests` were not run because this correction only amends audit documentation and does not change application source code.
- No dev server was started because no UI implementation was requested.
- No screenshot verification was run because no UI changed.

## 11. Remaining Risks

- `modGuids` and `traitGuids` now have a partial Chinese label/explanation source in `assets/stadium/manifest.json`; the remaining unmapped ids need a newer config or another source.
- Equipment is not exposed under a literal `equipment` field; `modGuids` is the likely six-slot equipment/mod source, but this still needs semantic confirmation in UI copy.
- Existing `get_match_detail` currently reads player rows through `totalCount`; round selection will need new DTO shape to read a selected round's own lists.
- Existing `player_rows()` falls back to an id field when `name` is missing; arena detail must resolve `nameMap` instead and avoid displaying raw ids.
- Current `playerHighlights.ts` computes maxima from values; Phase 7 wants source-flag-driven highlighting for `killMax`, `heroDamageMax`, `cureMax`, and `resistDamageMax`.
- `LeisureFight` and `SportFight` mode labels are missing from both frontend and backend label maps.
- `SportFight` rank icon assets are present under `assets/stadium/ranks/fight/`; the implementation still needs a separate resolver so preset/open competitive rank assets are not reused.

## 12. Verification Thread Recommendation

Recommend a verification thread before UI implementation if Phase 7 will display arena ability/trait/equipment tooltips.

Reason: the largest remaining risk is mapping coverage and privacy behavior, not layout. A review-only thread should verify that no raw player ids or token-like fields can reach the UI, and that any raw guid fallback is limited to development-only behavior or replaced with Chinese fallback labels before release review.
