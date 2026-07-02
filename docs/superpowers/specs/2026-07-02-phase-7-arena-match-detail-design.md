# Phase 7 Arena Match Detail Design

## Purpose

Phase 7 adapts OWStatsLocal's match detail experience for arena/fight data. The
first slice is intentionally limited to match detail. It does not build a full
arena statistics page and does not redesign the whole match workspace.

The source fixture is:

```text
data/players/5479_s22_full_season.json
```

Known fixture shape:

- Season 22 has 941 matches.
- 704 details are `match_kind = normal`.
- 237 details are `match_kind = fight`.
- Normal matches use top-level `teammateList` and `enemyList` detail data.
- Fight matches use `totalCount` and `roundCountList`; each selected view still
  has `teammateList`, `enemyList`, map, score, start time, and duration.

## Scope

In scope:

- Right-side match detail adaptation for arena/fight matches.
- Minimal left-list label change: arena/fight rows should show the concrete
  arena mode label (`角斗快速` or `角斗竞技`) where the normal row currently shows
  the mode/map subtitle, and should use the last arena round's map preview as
  the row background image.
- Existing match-list mode filters should include arena options: `角斗快速` and
  `角斗竞技`. Do not add a separate arena tab for Phase 7.
  - `LeisureFight` maps to `角斗快速`.
  - `SportFight` maps to `角斗竞技`.
- Arena detail DTO shape needed by the detail page.
- Competitive-rank display in default player rows for competitive preset/open
  matches and competitive arena matches when the match/round is identified as
  competitive and `rankInfo` is present.
- Unified default-row max highlighting for `killMax`, `heroDamageMax`,
  `cureMax`, and `resistDamageMax` across preset/open and arena modes.
- Asset discovery as the first execution step for arena abilities/traits,
  equipment, and competitive-rank icons.

Out of scope:

- Full arena statistics pages.
- Broad match workspace redesign.
- Broad normal preset/open match detail redesign. Phase 7 may still add the
  shared rank display and max-value highlighting agreed above.
- Reusing `match_players` for arena/fight analysis.

## Layout

Arena detail keeps the current rendered match-detail structure as the baseline:

```text
arena round selector
arena header/banner
current summary or round content
teammates
VS
enemies
```

The existing rendered page uses vertical teammate and enemy sections separated
by the `VS` divider. Do not redesign them as side-by-side columns or as one
mixed list.
Keep the existing section labels `队友` and `对手` for arena detail.

Note: an earlier main-thread discussion misread the `playerColumns` class name
as a left/right layout. The rendered product reference is authoritative here.

## Round Selector

Arena detail places the full-match/round selector at the very top of the detail
content, before the arena header/banner.

The selector may closely mimic the provided reference:

- angled active `角斗汇总` tab first
- score tabs for each round, formatted with spaces such as `1 - 0`
- Chinese round labels such as `回合-1`
- Do not show `角斗快速` or `角斗竞技` in the selector; mode labeling belongs in
  filters and list rows.
- Selected round tabs use result-sensitive active styling:
  - selected winning round: orange active tab
  - selected losing round: blue-gray active tab
- The selected `角斗汇总` tab is always orange and does not change color based on
  the full-match result.
- Unselected tabs keep the dark tab background. Their score text may use light
  result emphasis, such as orange text for winning rounds and blue-gray/white
  text for losing rounds, but the tab background should not become a full active
  color.

Behavior:

- The page opens on `角斗汇总`.
- Individual rounds are selected through this selector.
- `角斗汇总` uses `totalCount` player data. Individual rounds use that round's
  own `teammateList` and `enemyList` player data, including that round's
  equipment and ability/trait values.
- In `角斗汇总`, expanded player cards use final equipment and ability/trait
  values from `totalCount`.
- Switching between `角斗汇总` and a round collapses all expanded player cards.
- The selector is specific to arena/fight detail. Normal match detail keeps its
  current header flow.

Responsive constraints:

- All visible text is Chinese.
- Scores and round labels must not overflow.
- Arena has at most seven rounds. Design tab widths so `角斗汇总` plus up to
  seven round tabs fit in one row at normal desktop widths.
- In narrow windows where tabs cannot fit, use horizontal scrolling as the
  fallback rather than wrapping or a dropdown.
- First implementation does not require extra keyboard-navigation work beyond
  whatever the chosen button/control implementation naturally provides. The
  target interaction is desktop mouse use.
- Hover, focus, spacing, and color should integrate with the existing app visual
  system even when the selector shape follows the reference.

## Header Summary

Arena header/banner summary shows only:

- `角斗领域`
- score
- game duration
- match start time

When the selected view is `角斗汇总`, the score is the final match score.

When the selected view is an individual round, the score follows that round's
end-state cumulative score, for example `1-2` for round 3 if that is the round
tab score.

Header text differs by selected view:

- `角斗汇总`: title is `角斗领域`; fields are final score, total game duration,
  and match start time. It does not show a map name.
- Individual round: title is `地图：{mapName}`; fields are current round
  cumulative score, round duration, source game-mode label such as `攻防阵线`,
  and round start time.

Do not add a duplicate "currently selected" label to the banner.

Duration source:

- `角斗汇总`: prefer `totalCount.gameTimeSec`; if missing, fall back to summing
  round `gameTimeSec` values.
- Individual round: use that round's `gameTimeSec`.

Start-time source:

- `角斗汇总`: prefer the full-match entry `beginTs`; if missing, fall back to the
  first round `startTime`.
- Individual round: use that round's `startTime`.

Header background:

- `角斗汇总`: use a fixed arena-domain background image.
- Individual round: use the selected round's map preview image.
- Left match-list arena rows: use the last arena round's map preview image.
- Left match-list arena row score should prefer the full-match final score
  fields. If those are missing, fall back to the last round's cumulative score.
- The fixed arena-domain background should be discovered from existing assets
  first, especially under `assets/maps`. Do not generate a new image for the
  first implementation. If no suitable fixed asset exists, fall back to an
  existing default map/background or the last round's map preview.

Header result treatment:

- Both `角斗汇总` and individual round banners show a prominent win/loss state
  block similar to the reference.
- Winning arena round banners use the orange win treatment. Losing arena round
  banners use the same blue-gray treatment as the selected losing round tab.
  Reuse existing app win/loss color language where it matches these treatments.
- The `角斗汇总` banner uses the full-match result for its result block: orange
  for victory and blue-gray for defeat. This is independent from the `角斗汇总`
  selector tab, which remains orange when selected.
- `角斗汇总` uses the full-match `matchRet` for win/loss.
- The exact individual-round win/loss source is not fixed yet. Execution should
  audit whether round data has `matchRet`, whether score deltas are sufficient,
  or whether another field is the reliable source.

## Player Rows

Arena player lists preserve the source order for both teammates and enemies.
Do not put the current player first and do not sort by economy, output, or rank.

Arena detail should hide normal-mode `对局表现` and `同职责赛季均值对比`
modules. The existing scoring and role-comparison logic is designed for normal
preset/open matches and should not be implied to apply to arena/fight data in
Phase 7.

Arena detail should not show a raw JSON viewer. Debugging arena structure should
use development tools or separate read-only audit scripts, not the user-facing
detail UI.

Default player rows should reuse the existing match-detail player row pattern.
Do not redesign the default row around arena-only fields.

Default row rules:

- Keep the existing player, KDA, damage/mitigation/healing, and final-hit style
  already used by the match detail page.
- Highlight default-row kills, damage, healing, and mitigation/resist values
  when their source row has `killMax`, `heroDamageMax`, `cureMax`, or
  `resistDamageMax`. This applies to both preset/open and arena modes.
- Do not highlight `finalHit`; max highlighting follows only existing max
  fields in the source data.
- Prefer the player row's own name field when present; fall back to `nameMap`
  when needed. Do not use raw `bnetId` as the normal display name.
- Do not show a `我` marker for the current player in arena detail.
- Add competitive-rank display for competitive preset/open rows and competitive
  arena rows when the `gameMode` enum is identified as competitive and `rankInfo`
  is present. Examples include sport/ranked modes such as `SportPreset`,
  `SportOpen`, and `SportFight`, subject to execution-stage audit. In practice,
  `rankInfo` is expected to appear only for competitive modes. The display uses
  a rank icon plus a rank number/score beside the player name, not a separate
  table column. The exact `rankInfo` field for that number/score is not fixed in
  the design; execution should audit sample `rankInfo` values and rank icon
  assets before choosing the display field.
- Do not show S/A-style arena performance ratings in Phase 7. Arena performance
  scoring needs a separate algorithm and should not reuse normal-mode scoring.

Economy, abilities/traits, and equipment are shown only in the expanded player
card, never as default-row columns.

Expanded player row:

- Left: economy block, including current/end funds when available.
- Middle: four abilities/traits, shown as icons by default.
- Right: six equipment items, shown as icons by default.
- The expanded layout keeps fixed slots: four ability/trait slots and six
  equipment slots. If data has fewer items than the slot count, the empty slots
  stay reserved but do not show visible content. Do not show empty-state text
  such as `暂无装备` or `暂无异能` inside those slots.
- Ability/trait and equipment icons show name and explanation on hover using an
  in-app custom tooltip, not just the native browser `title`. The text content
  for those explanations will be filled during development. If an explanation is
  missing, the tooltip shows only the name. If the name is also missing during
  Phase 7 development, the tooltip may show the raw guid so missing mappings can
  be fixed. Execution should first check whether an existing tooltip pattern or
  style can be reused.
- In narrow windows, the expanded card stacks vertically as economy, then
  abilities/traits, then equipment. Do not force the three-column desktop layout
  into a cramped width.

Economy block:

- `角斗汇总`: show final funds, using `endWorth` when available.
- Individual round: show funds as `worth → endWorth`.
- If `worthMax` is true, highlight the funds value rather than adding a separate
  column.
- A funds/currency icon is not required in the first implementation. Add one
  only after asset discovery confirms a suitable asset.

Multiple player rows may be expanded at the same time. This supports comparison
across players. Expanded panels should remain compact enough for multi-open use.
No player row is expanded by default, including when returning to `角斗汇总`.
The entire player row toggles expansion, and a right-side arrow remains visible
as the explicit expand/collapse affordance.

## Assets

The first execution step for this feature should discover and map assets for:

- arena abilities/traits
- arena equipment
- arena competitive-rank icons

The design does not assume final asset paths yet.

If an arena trait, equipment item, or competitive-rank asset/label is missing,
the detail UI may show the raw guid during Phase 7 development so missing
mappings are easy to find and fix.

Before a user-facing release, a verification thread should explicitly decide
whether raw guid fallback is still acceptable or should be replaced by hidden
items or Chinese fallback labels.

## Data And Privacy

Arena data can include:

- `bnetId`
- `customerTokenMap`
- `nameMap`
- `roundCountList`
- `totalCount`
- `modGuids`
- `traitGuids`
- `aveStatGuids`
- `statMap`
- `worth`
- `endWorth`

Sensitive fields such as `customerTokenMap` and raw `bnetId` values must not be
printed, logged in reports, or displayed as raw identifiers.

The UI can use sensitive identifiers only for internal correlation when needed.

## Error Handling

If arena detail parsing fails, show a Chinese recoverable error banner such as
`角斗详情解析失败`, but keep any available base match information visible, such as
score, result, time, and map/banner data. Do not make the whole detail page fail
when only arena-specific parsing fails.

If one arena round fails to parse but other rounds are available, keep that
round's selector tab visible. Selecting the failed round should show a
round-level error state while leaving other rounds usable.

If `totalCount` is missing but `roundCountList` exists, `角斗汇总` should show a
summary-level error state while the individual round views remain usable. Do not
use the last round or summed rounds to fabricate a summary in Phase 7.

If `roundCountList` is missing but `totalCount` exists, show `角斗汇总` and a
Chinese warning that round data is missing. Do not make the whole arena detail
unavailable.

## Open Questions

- Which `rankInfo` field should be used for the rank number/score next to the
  rank icon.
- What source should determine individual-round win/loss: a round-level
  `matchRet`, score delta, or another field.
- Which labels should be used for `modGuids`, `traitGuids`, and `aveStatGuids`.
- How much of `statMap` should be surfaced in first version versus hidden behind
  a later specialized view.

## Verification Expectations

Implementation should verify:

- Normal match detail remains unchanged.
- Arena match detail opens on `角斗汇总`.
- Round tabs switch selected view and collapse expanded player rows.
- Teammate/enemy sections stay vertical with the existing `VS` separator.
- Arena rows do not expose raw `customerTokenMap`, raw `bnetId`, or local
  credential data.
- All user-facing text is Chinese.
