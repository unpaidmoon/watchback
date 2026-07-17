# OWStatsLocal Main Thread Log

This document is the durable ledger for the OWStatsLocal main-thread workflow.
Use it to recover decisions when a Codex session is compacted, restarted, or
split into execution and verification threads.

> **当前运行时数据政策（2026-07-18）**：本账本保留旧决定作历史记录；其中任何要求对运行时 raw JSON、错误文本或历史 SQLite/赛季 JSON 递归脱敏、遮盖或重写的条目，均已由 [`2026-07-18-remove-runtime-redaction-design.md`](2026-07-18-remove-runtime-redaction-design.md) 废止。当前以原始数据直通、禁止主动扩散凭据、以及保留发布资源和账号/event schema 防线为准。

## Main Thread Charter

The active main thread owns:

- product direction and task scope
- risk and priority decisions
- execution-thread prompt generation
- review of execution-thread evidence
- decision to accept, request fixes, or open a verification thread
- final task closeout and remaining-risk notes

The main thread should avoid collecting long command logs or trial-and-error
details. Execution threads bring back concise evidence instead.

## Project Constants

- Project root: `D:\Codex\OW_data\owstats-local`
- Active product: `apps/owstats-desktop/`
- Stack: Tauri 2, React, TypeScript, Rust, SQLite
- Desktop onboarding owns NetEase Dashen login, credential status, and season
  sync entry.
- Installed-product login uses Tauri backend commands.
- Match module is the only filter entry.
- Ranking owns hero win-rate and map win-rate.

## Hard Rules

- Confirm commands run from `D:\Codex\OW_data\owstats-local` or a deliberate
  subdirectory under it.
- All user-facing frontend text must be Chinese.
- 不得主动把 `OW_DASHEN_TOKEN`、`customerToken`、
  `data/.dashen_credentials.json`、cookie、`data/chrome-dashen-profile/`
  下的浏览器存储或本地 API dump 的玩家私密标识复制到日志、提交、发布包、共享配置、账号/event 表面或新的 DTO/payload。上游自然出现的运行时 raw JSON 与错误文本仍按 2026-07-18 设计原样通过。
- 正常对局的既有 raw JSON 查看面按存储内容原样返回；不得为显示重新加入运行时脱敏。战斗对局 raw JSON 为 `Null` 以及显示名/GUID 兜底仍是产品/schema 决策。
- Do not touch generated artifacts, build output, `node_modules`, `target`,
  browser profiles, or local data dumps.
- Keep narrow tasks narrow. Do not reopen product direction for region-scoped
  UI or bugfix work.

## Split Criteria

Use one session when the task is a small text, CSS, explanation, path lookup, or
single-error investigation.

Use main thread plus execution thread when the task changes a page region,
backend command, ranking or match display logic, or a small frontend/backend
contract.

Use main thread plus execution and verification threads when the task touches
Dashen login, credentials, tokens, cookies, raw JSON display, SQLite schema,
season sync, Tauri commands, release packaging, `data/`, or privacy-sensitive
fields.

## Execution Thread Prompt Template

```text
You are an OWStatsLocal execution thread.

Project root:
D:\Codex\OW_data\owstats-local

First confirm the current directory is the project root or a deliberate
subdirectory under it.

Background:
[why this task exists]

Concrete task:
[what to complete this time]

Allowed to modify:
- [specific files or directories]

Forbidden:
- data/
- data/.dashen_credentials.json
- data/chrome-dashen-profile/
- generated artifacts
- build output
- node_modules
- target
- unrelated docs or refactors

Hard requirements:
- All user-facing frontend text must be Chinese.
- Do not print, commit, or expose OW_DASHEN_TOKEN, customerToken, local
  credentials, cookies, browser storage, or raw player privacy identifiers.
- 既有正常对局 raw JSON 面按存储内容原样返回；不得主动把凭据复制到账号、DTO、事件、日志或原本不含它们的 payload。
- Keep the change local to the requested scope.

Verification:
- [task-specific commands]

Deliver an evidence report with:
1. Changed files
2. Why each file changed
3. Verification commands run
4. Verification results
5. Checks not run and why
6. Remaining risks
7. Whether a verification thread is recommended
```

## Verification Thread Prompt Template

```text
You are an OWStatsLocal verification thread. Review only; do not continue
development.

Project root:
D:\Codex\OW_data\owstats-local

First confirm the current directory is the project root or a deliberate
subdirectory under it.

Original task:
[paste original task]

Execution report:
[paste execution-thread report]

Review focus:
- Does the result satisfy the original task?
- Are there unrelated changes?
- Did the work expose OW_DASHEN_TOKEN, customerToken, local credentials,
  cookies, browser storage, or raw player privacy identifiers?
- Is all user-facing frontend text Chinese?
- 是否保持正常对局 raw JSON 原样返回、未主动把凭据复制到账号、DTO、事件、日志或新 payload？
- Were reasonable verification commands run?
- What risks remain before delivery?

Return findings first, ordered by severity, with file and line references where
possible. Do not fix code.
```

## Task Entry Template

```markdown
## YYYY-MM-DD Task Name

Goal:

Decision:

Execution thread:

Allowed scope:

Forbidden scope:

Acceptance criteria:

Execution result:

Verification:

Remaining risks:

Next step:
```

## 2026-07-02 Main Thread Established

Goal:
Establish the current Codex conversation as the OWStatsLocal main thread and
create durable recovery notes for future sessions.

Decision:
Use this session for direction, scope, execution-thread prompt generation,
report review, and final acceptance. Keep execution logs and large trial/error
details in separate execution threads when tasks are complex or risky.

Execution thread:
Not needed for this setup task.

Allowed scope:
- `AGENTS.md`
- `docs/superpowers/specs/main-thread-log.md`

Forbidden scope:
- `README.md` existing local changes
- `data/`
- app source code
- generated artifacts

Acceptance criteria:
- Future sessions can find the main-thread workflow from `AGENTS.md`.
- Durable prompt templates and task entry format exist in the project.
- Sensitive-data and frontend-language constraints remain explicit.

Execution result:
Created this ledger and linked it from `AGENTS.md`.

Verification:
Manual file inspection is sufficient for this documentation-only setup.

Remaining risks:
This is a process document; it does not enforce behavior automatically.

Next step:
Use the main thread to generate execution-thread prompts for the next nontrivial
OWStatsLocal task.

## 2026-07-02 Phase 7 Arena Match Detail

Goal:
Adapt the match detail experience for arena/fight data before building broader
arena analysis.

Design doc:
`docs/superpowers/specs/2026-07-02-phase-7-arena-match-detail-design.md`

Status:
Grilling in progress. No execution-thread prompt has been generated yet.

Summary decisions:
- Phase 7 starts with arena match detail, not a full arena statistics page.
- The left match list is not redesigned; arena rows only need the small mode
  subtitle to read `角斗领域`.
- The right-side detail panel owns the arena adaptation.
- Arena detail opens on `角斗汇总`, with round tabs at the top before the arena
  banner.
- Arena player rows are compact by default and expand to show economy,
  abilities/traits, and equipment.
- Multiple player rows may be expanded; switching summary/round collapses all.
- Teammates and enemies keep the existing vertical sections separated by `VS`.
- Normal preset/open match detail should remain functionally unchanged.

Next step:
Continue grilling Phase 7 details in the dedicated design doc, then produce an
execution-thread prompt only after the design is approved.

## 2026-07-02 Phase 7 Asset/Data Audit Review

Goal:
Review the first Phase 7 asset/data audit before allowing UI or DTO
implementation to start.

Decision:
Accept the audit findings as useful design input, but do not accept the current
work product as scope-clean. The execution report claimed only the audit report
changed, while `git status --short` also showed `assets/stadium/` and
`docs/superpowers/specs/2026-07-02-stadium-unmapped-manual-map.csv`.

Execution thread:
`docs/superpowers/specs/2026-07-02-phase-7-arena-asset-data-audit.md`

Verification:
A review-only thread found no critical issue and no sensitive value exposure,
but confirmed important scope issues:
- changed-files reporting is inaccurate
- `assets/stadium/` is a substantive asset import, not just audit output
- the manual unmapped CSV is also an unauthorized new artifact

Accepted technical inputs:
- `SportFight` is the main arena mode in the fixture.
- `modGuids` is the strongest six-slot equipment/mod candidate.
- `traitGuids` is the strongest four-slot ability/trait candidate.
- Arena rounds do not have `matchRet`; adjacent cumulative score deltas are the
  recommended first round-result source.
- Competitive rank display should use `rankName + rankSubTier`.
- Missing arena mappings must use Chinese fallback labels in UI; raw guid
  display is not acceptable for release-facing UI.

Next step:
Send a narrow correction prompt to the execution thread. It must amend the audit
report to list `assets/stadium/` and the CSV, explain their source and
necessity, report file counts and privacy checks, and recommend whether the
assets should be kept as a separate Phase 7 asset-import task. Do not start UI
or DTO implementation until this is resolved.

## 2026-07-02 Phase 7 Asset/Data Audit Correction Accepted

Goal:
Re-read the corrected Phase 7 asset/data audit and decide whether the correction
unblocks the next main-thread decision.

Decision:
Accept the corrected audit report as scope-transparent. It now explicitly lists
the audit report, `assets/stadium/`, and
`docs/superpowers/specs/2026-07-02-stadium-unmapped-manual-map.csv` as changed
artifacts and explains that the asset tree and CSV are asset-import candidates
created outside the original audit-only scope.

Verification:
- Current `assets/stadium/` file count is 1095 files, matching the corrected
  report.
- UTF-8 reads confirm Chinese text in the report is valid; PowerShell display
  may still render Chinese incorrectly.
- `git diff --check` reported only existing line-ending warnings for
  `AGENTS.md` and `README.md`.

Accepted constraints for later implementation:
- Release-facing UI must not display raw guid values.
- Missing `modGuids` and `traitGuids` mappings must use Chinese fallback labels
  or hidden items.
- `SportFight` rank icons need a separate arena resolver and must not silently
  reuse preset/open rank assets.

Next step:
Main thread must decide whether to keep `assets/stadium/` and the unmapped-id
CSV as a dedicated Phase 7 asset-import scope. After that decision, generate the
first UI/DTO execution-thread prompt.

## 2026-07-02 Phase 7 Asset Import Accepted

Goal:
Decide whether to keep the asset files and unmapped-id CSV created during the
Phase 7 audit correction flow.

Decision:
Keep `assets/stadium/` and
`docs/superpowers/specs/2026-07-02-stadium-unmapped-manual-map.csv` as a
dedicated Phase 7 asset-import preparation scope. They are accepted as useful
inputs for arena rank, mod, and trait resolution, but they are not treated as
part of the original audit-only task.

Acceptance constraints:
- Later implementation must add a dedicated arena resolver for
  `assets/stadium/manifest.json`.
- `SportFight` rank icons must not silently reuse preset/open rank assets.
- Release-facing UI must not show raw guid values.
- Missing arena mappings must use Chinese fallback labels or hidden slots.
- The unmapped CSV remains a mapping worksheet, not a user-facing source.

Next step:
Generate the first UI/DTO execution-thread prompt for Phase 7 arena match detail,
using the accepted audit findings and asset-import scope. Keep the first
implementation slice narrow.

## 2026-07-02 Phase 7 Arena Detail Slice 1 Executed

Goal:
Track the first narrow UI/DTO implementation slice for arena match detail.

Execution result:
Execution thread reports the first slice is complete:
- backend `ArenaMatchDetail` / `ArenaDetailSlice` DTOs
- fight detail parsing from `totalCount` and `roundCountList`
- safe arena names via `nameMap`, with no raw player id fallback
- fight `rawJson` returned as `null`
- frontend arena detail branch in full and compact detail views
- `角斗汇总` default selection and round switching
- `LeisureFight` / `SportFight` Chinese labels
- source-order teammate/enemy vertical sections with `VS`

Reported verification:
- targeted RED/GREEN Rust test for fight detail DTO
- `cargo fmt --check`
- `cargo test --tests`
- `npm test`
- `npm run build`

Main-thread spot check:
- Actual modified source files match the reported implementation files.
- UTF-8 reads confirm added Chinese strings are valid; PowerShell rendering may
  still show mojibake.
- `git diff --check` reports only line-ending warnings on existing working-tree
  files.

Remaining risks:
- No browser screenshot or Playwright pass has verified the selector layout.
- Privacy boundary and normal match-detail regression need review-only
  verification before acceptance.
- Arena equipment, trait, ability, tooltip, and rank icon resolver work remains
  intentionally out of scope.

Next step:
Open a verification thread for Slice 1. The thread must review only and focus on
privacy boundaries, Chinese UI, normal match-detail regression, and compact
arena interaction.

## 2026-07-02 Phase 7 Arena Detail Slice 1 Verification

Goal:
Review Slice 1 before accepting the first arena detail implementation.

Decision:
Do not accept Slice 1 yet. The verification thread found an important privacy
boundary issue that must be fixed before moving to the next feature slice.

Findings accepted by main thread:
- `arena_player_name(...)` filters inline `name`, but `nameMap` fallback is not
  passed through `looks_like_private_id`; a numeric/private-looking `nameMap`
  value could become user-facing text.
- Arena summary fallback logic is incomplete: duration should fall back from
  `totalCount.gameTimeSec` to the sum of round `gameTimeSec`, and start time
  should fall back to the first round `startTime`.
- Compact arena detail currently reuses the normal compact player row and shows
  the `威能` / `未记录威能` area even though arena mod/trait tooltips are out of
  scope for Slice 1.

Required fixes:
- Filter `nameMap` values with the same private-id guard before using them as
  display names; fallback to `未知玩家` when unsafe.
- Add regression coverage for private-looking `nameMap` values.
- Add summary fallback coverage for missing `totalCount.gameTimeSec` and missing
  match `beginTs`.
- Hide or bypass the normal compact perk strip for arena player rows in this
  slice.

Next step:
Send a narrow fix prompt to the execution thread. After fixes, run the same Rust
and frontend verification commands and request another review.

## 2026-07-02 Phase 7 Arena Detail Slice 1 Fixes

Goal:
Confirm the execution thread addressed Slice 1 verification findings without
expanding scope.

Decision:
Accept the targeted fixes at the main-thread spot-check level. The privacy
blocking issue is addressed in code and tests, but final Slice 1 acceptance
still needs a visual/browser pass because no screenshot verification has been
run yet.

Fixes accepted:
- `nameMap` fallback values are filtered with `looks_like_private_id` before
  becoming arena display names, with unsafe values falling back to `未知玩家`.
- Tests cover numeric/private-looking `nameMap` values and assert raw ids are
  not used as display names.
- `角斗汇总` duration falls back to summed round `gameTimeSec`; begin time falls
  back to the first round `startTime`.
- Compact arena player rows pass `showPerks={false}`, while normal compact rows
  retain the existing perk display by default.

Reported verification:
- `cargo fmt --check`
- `cargo test --tests`
- `npm test`
- `npm run build`
- `git diff --check`

Main-thread spot check:
- The three reported files contain the expected fixes.
- UTF-8 reads confirm Chinese strings are valid.
- `git diff --check` reports only line-ending warnings and no whitespace errors.

Remaining risk:
No Playwright or browser screenshot has verified the arena selector and compact
detail layout in a real viewport.

Next step:
Run a final visual/browser verification pass for Slice 1. If the selector and
compact arena detail are visually acceptable and no privacy regression is found,
accept Slice 1 and move to the next Phase 7 slice.

## 2026-07-02 Phase 7 Slice 1 List Entry Gap

Goal:
Resolve the user's desktop acceptance blocker after visual verification reported
Slice 1 acceptable.

Decision:
Do not fully accept Slice 1 yet. The detail implementation is usable through
mocked visual verification, but the real desktop acceptance path is blocked
because the match list was not adapted enough for arena/fight entries.

Observed gap:
- `list_matches(...)` still queries `WHERE m.bucket = 'preset'`.
- `MatchTimeline` still renders the generic match banner.
- `formatMatchBannerTitle(...)` still formats rows as `mapName \ modeLabel`.
- The Phase 7 design required arena/fight rows to show `角斗快速` or `角斗竞技`
  where the normal row shows the mode/map subtitle, and to use the last arena
  round map preview as the row background.

Next step:
Generate a narrow execution-thread prompt for the match-list entry slice. The
goal is to make arena/fight matches discoverable and selectable in the desktop
match list without redesigning the whole list or changing the arena detail
implementation.

## 2026-07-02 Phase 7 List Entry Execution Review

Goal:
Review the match-list entry execution report after the user found the desktop
acceptance path was still blocked.

Decision:
Do not accept this execution result. It appears to make arena/fight entries more
discoverable from the match list, but it also widened beyond the requested list
entry scope and changed accepted arena detail semantics.

Blocking issues:
- Arena detail summary now uses `last_arena_round(...)` / `summary_source` for
  score, map, and player rows. This conflicts with the accepted design that
  `角斗汇总` uses `totalCount` player data and final-match summary semantics.
- `apps/owstats-desktop/src/shared/arenaDetailFormat.ts` returns
  `VICTORY` / `DEFEAT` / `DRAW` for arena result sublabels, violating the hard
  rule that all new user-facing frontend text must be Chinese.
- The implementation went beyond the list-entry prompt by adding arena
  expansion UI, arena asset resolvers, rank icons, player highlight changes,
  package test-script changes, and several new tests. Those may be useful later
  but are not acceptable as part of the narrow list-entry fix without explicit
  scope approval.
- Generated validation artifacts remain in the worktree:
  `.playwright-cli/`, `test-results/`, and
  `apps/owstats-desktop/test-results/`.

What still looks directionally useful:
- `list_matches(...)` no longer only relies on `m.bucket = 'preset'` and now
  includes fight/game-mode conditions.
- The list projection has a path for last-round map preview and fight row
  display values.

Required correction:
Send a narrow correction prompt. Preserve the useful list-entry behavior, but
restore `角斗汇总` detail semantics to `totalCount`, replace new English arena UI
labels with Chinese, and either remove or explicitly quarantine unrelated
scope-expansion changes for a later slice.

## 2026-07-02 Phase 7 Arena Expansion Slice 1

Goal:
Review the first arena player expansion-card behavior slice.

Decision:
Accept the behavior slice at main-thread spot-check level. The implementation
keeps the work local to the full match detail page and does not enable compact
arena expansion.

Execution result:
- `MatchDetailReport` owns `expandedArenaPlayerKeys`.
- Multiple arena player rows can be expanded at the same time.
- Switching selected arena slice or match resets the expanded set.
- Full arena detail passes `allowExpansion={true}`.
- Normal match detail passes `allowExpansion={false}`.
- Compact detail remains unchanged.
- The expand affordance shows Chinese `展开` / `收起` text with an arrow icon.

Verification reported:
- `npx tsx scripts/arenaDetailFormat.test.ts`
- `npm test`
- `npm run build`
- `git diff --check`

Main-thread spot check:
- The diff only touches `MatchDetailPage.tsx`, `styles.css`, and
  `scripts/arenaDetailFormat.test.ts`.
- The previous per-row local `useState(false)` expansion state is removed.
- Source-level tests assert full arena detail enables expansion and compact
  detail does not expose `phase7ExpandIcon`.

Remaining risk:
This is source-level verification only; no browser click test or screenshot has
verified the full interaction. Also, all full arena rows currently show an
expand affordance when arena expansion is enabled, even if the row has sparse
expanded content.

Next step:
Proceed to Slice 2 for expansion content rules: verify summary uses `totalCount`
data, round views use round data, empty slots stay reserved without visible
empty text, missing mappings stay Chinese-only, and switching views continues to
collapse expanded rows.

## 2026-07-02 Phase 7 Arena Expansion Slice 2

Goal:
Review the arena expansion-card content-rule slice.

Decision:
Accept the slice at main-thread spot-check level. The implementation fixes the
highest-risk content issue by ensuring `角斗汇总` player rows and expansion fields
come from `totalCount`, while round views keep using their own round data.

Execution result:
- `arena_summary_player_rows(...)` now reads `totalCount` lists directly instead
  of preferring the last arena round.
- Tests assert summary `kills`, `damage`, economy fields, `modGuids`, and
  `traitGuids` come from `totalCount`.
- Tests assert round 3 keeps its own economy and guid values.
- Full arena detail passes `economyMode="final"` for `角斗汇总` and
  `economyMode="round"` for individual rounds.
- Summary economy displays `endWorth` first, then `worth`; round economy keeps
  the `worth -> endWorth` transition when both values differ.
- Fixed slots remain four `traitGuids` slots and six `modGuids` slots.
- Missing slot mappings use Chinese fallback labels and do not display raw guid
  text.

Reported verification:
- `cargo fmt --check`
- `cargo test --tests`
- `npx tsx scripts/arenaDetailFormat.test.ts`
- `npx tsx scripts/phase6Assets.test.ts`
- `npm test`
- `npm run build`
- `git diff --check`

Main-thread spot check:
- The relevant diffs are in `queries.rs`, `import_baseline.rs`,
  `MatchDetailPage.tsx`, and `arenaDetailFormat.test.ts`.
- The code-level checks match the requested content rules.

Remaining risk:
The slot and tooltip behavior is still verified by source-level assertions, not
by browser interaction or screenshot review. Tooltip positioning and compact
behavior remain out of scope.

Next step:
Plan the next slice around visual verification and tooltip/slot usability before
asset mapping is completed, so missing mappings can be inspected safely in the
UI with Chinese fallback labels.

## 2026-07-02 Phase 7 Arena Expansion Slice 3

Goal:
Review the arena expansion-card visual and tooltip usability slice.

Decision:
Accept the slice at main-thread spot-check level. The full arena detail
expansion card is now visually usable enough to support the next asset-mapping
work, while compact detail and normal detail remain without expansion entry.

Execution result:
- Expanded arena rows keep player identity and the expand/collapse control in
  separate header columns.
- Expanded content is organized as `经济` / `异能` / `装备` on desktop and stacks
  into one column below 760px.
- Multiple expanded rows use visible overflow and extra spacing.
- Slot groups keep four trait slots and six mod slots with empty visual
  placeholders instead of empty-state copy.
- Slot tooltips show Chinese fallback names and do not display raw guid text.
- Compact match detail and normal match detail still do not expose the arena
  expansion affordance.

Reported verification:
- `npx tsx scripts/arenaDetailFormat.test.ts`
- `npm test`
- `npm run build`
- `git diff --check`
- `npx playwright test scripts/arenaDetailVisualCheck.spec.mjs --reporter=line`

Main-thread spot check:
- The relevant visual diffs are in `MatchDetailPage.tsx`, `styles.css`,
  `arenaDetailFormat.test.ts`, and the new Playwright visual script.
- Screenshot outputs exist for summary expansion, tooltip description, round
  expansion, narrow expansion, normal detail without expansion, and compact
  detail without expansion.
- UTF-8 source text in the visual script reads correctly via Python even though
  PowerShell output can render the Chinese mock text incorrectly.

Remaining risk:
At 720px width, the expansion content stacks correctly, but the row metric values
above the expansion card can still truncate with ellipses. This is a polish item
for the next visual pass, not a blocker for using the card to continue mapping
assets. The visual check still runs in Vite + Chromium with a sanitized Tauri
mock, not the native Tauri WebView.

Next step:
Proceed to the asset/mapping-oriented slice using the now-visible slots. Keep
the same privacy rule: missing mappings may show only Chinese fallback text, not
raw guid values.

## 2026-07-03 Phase 7 Arena Compact Expansion Review

Goal:
Review the follow-up change that restores compact-panel row-click expansion and
changes arena summary player data back to the last-round projection.

Decision:
Accept. The compact-panel row-click expansion and the summary last-round
projection are intentional product-direction changes requested and personally
verified by the user. The full match-detail arena expansion card may be removed
for this direction; this is no longer treated as a blocking regression.

Accepted parts:
- Compact arena detail keeps expansion state in `MatchesPage.tsx`.
- Clicking the whole compact player row toggles the arena expansion content.
- There is no separate compact `展开/收起` button.
- `aria-expanded` and focus styling are preserved for compact arena rows.
- Switching compact arena slice or match clears the expanded compact rows.
- Backend summary player rows now use the last arena round when it has player
  lists, while damage taken and healing taken remain round-aggregate overrides.

Scope clarification:
- The user explicitly confirmed this change was their own suggestion and was
  personally accepted in desktop verification.
- Full-detail arena expansion removal is therefore an accepted scope change for
  this slice, not a required correction.
- Future prompts should not ask execution threads to restore full-detail
  expansion unless the product direction changes again.

Verification checked by main thread:
- `git diff --check` exits 0 with only LF/CRLF warnings.

## 2026-07-03 Phase 8 Command Center Season Sync Design

Goal:
Start Phase 8 and move the command center from a settings-like page toward a
season data sync and local multi-season status workspace.

Accepted product direction:
- Phase 7 arena asset calibration remains a tail item and does not block Phase
  8.
- Phase 8 priority is season data fetching and multi-season SQLite
  compatibility.
- Supported seasons are fixed to S21, S22, and S23.
- Do not add a global current-season state.
- Each page keeps its own season filter.
- Do not add a sync-all-seasons action.
- The command center owns login state, single-season sync, and local season
  health/status display.

Design doc:
- `docs/superpowers/specs/2026-07-03-phase-8-command-center-season-sync-design.md`

First implementation direction:
Build fixed S21/S22/S23 season cards on the command center, each with local data
counts, detail coverage, arena/normal split, failed detail count, latest sync
time, and a single-season sync button. Keep existing pages compatible with their
own season selection.

## 2026-07-03 Phase 8 Season JSON Dump Slice 1

Goal:
Review the local diagnostic entry for dumping a fetched season payload to
`data/players`.

Decision:
Accept the slice. It restores a practical full-season JSON dump path without
reimplementing the request protocol, while keeping the existing SQLite sync path
on the same fetch flow.

Execution result:
- Added `apps/owstats-desktop/src-tauri/src/bin/dump_season_json.rs`.
- Added exported APIs `dump_season_json` and `dump_season_json_with_base_url`.
- Refactored `sync_season_with_base_url` to reuse
  `fetch_season_parts_with_base_url`.
- Dump output path is `data/players/local_s{season}_full_season.json`.
- The `local_` filename is intentional to avoid putting local role ids in the
  filename.
- Fixture coverage includes normal list/detail, fight list/detail, and a detail
  failure.
- Dump payload recursively removes fields whose names include `token`, `cookie`,
  `authorization`, or `credential`.

Verification checked by main thread:
- `cargo fmt --check`
- `cargo test --test sync_client`
- `cargo build --bin dump_season_json`
- `cargo test --tests`
- `git diff --check` exits 0 with only LF/CRLF warnings.

Privacy notes:
- No real upstream fetch was run in this review.
- No local credential or browser profile file was read manually.
- The fixture asserts dumped JSON does not contain token/cookie/customerToken
  field names or dummy secret values.

Next step:
Proceed to the command-center season status cards, using this dump command only
as a local diagnostic path. Real dumped JSON files must remain uncommitted unless
the user explicitly requests otherwise.

## 2026-07-03 Phase 8 Command Center Season Cards Slice 2

Goal:
Review the command-center product slice that adds fixed S21/S22/S23 local season
status cards and single-season sync buttons.

Decision:
Accept the slice. The command center now exposes the fixed three-season local
status surface without adding sync-all, global current-season state, or UI access
to the diagnostic dump command.

Execution result:
- `OnboardingStatus` now includes `seasonStatuses`.
- Backend returns fixed S21, S22, and S23 season status rows.
- Each season status includes match/detail counts, normal/arena split, failed
  detail count, latest local timestamp, and detail coverage percent.
- Command center renders three fixed season cards.
- Each card has its own `同步 Sxx` action through the existing `syncSeason`
  command.
- While one season is syncing, the active season card shows progress and other
  season sync buttons are disabled.
- Existing login, logout, update check, credential status, profile summary, and
  data-health surfaces remain in place.

Verification checked by main thread:
- `cargo fmt --check`
- `cargo test --test phase8_command_center`
- `npx tsx scripts/phase6Shell.test.ts`
- `cargo test --tests`
- `cargo build --bin dump_season_json`
- `npm test`
- `npm run build`
- `git diff --check` exits 0 with only LF/CRLF warnings.

Privacy notes:
- No real upstream sync was run during review.
- No local credential or browser profile file was read manually.
- The command-center UI does not expose `dump_season_json` or any token-like
  value.

Remaining risk:
No Playwright or native Tauri visual screenshot was run for the new three-card
layout. Source tests and TypeScript build cover structure and compilation, but a
later visual pass should inspect narrow-width card wrapping and real-data text
lengths.

Next step:
Proceed to Phase 8 multi-season compatibility review across existing pages:
matches, review, ranking, and friend analytics should continue using their own
season filters without mixing S21/S22/S23 data.

## 2026-07-09 Phase 8 Multi-Season Page Compatibility Design

Goal:
Document the next development phase after fixed S21/S22/S23 command-center
sync: make existing pages work correctly when multiple seasons coexist in the
same SQLite database.

Accepted product direction:
- Keep S21, S22, and S23 as the fixed supported seasons.
- Do not add global current-season state.
- Do not add sync-all-seasons.
- Keep the command center as login, single-season sync, and local season health
  only; it must not drive page filters.
- Each product page owns its own season selection.
- Remove old S22-only assumptions from page defaults and tests.

Design doc:
- `docs/superpowers/specs/2026-07-09-phase-8-multi-season-page-compatibility-design.md`

First implementation direction:
Start with backend multi-season isolation tests before changing UI. Then adapt
personal review, ranking, friend analytics, and matches/detail behavior in
narrow slices, preserving Chinese-only visible UI text and privacy constraints.

## 2026-07-09 S23 Arena Map Preview Mapping

Goal:
Use S23 arena round data plus manual game-side confirmation to replace the
generic stadium preview with per-map previews where available.

Accepted mappings:
- `576460752303428278` -> `assets/maps/hanaoka.jpg` (`花冈`)
- `576460752303428219` -> `assets/maps/ilios2.png` (`伊利奥斯 - 废墟`)
- `576460752303428209` -> `assets/maps/powder-keg-mine.jpg` (`火药桶矿井`)
- `576460752303428192` -> `assets/maps/redwood-dam.jpg` (`红杉水坝`)
- `576460752303428277` -> `assets/maps/throne-of-anubis.jpg` (`阿努比斯王座`)
- `576460752303428337` -> `assets/maps/nepal2.png` (`尼泊尔 - 圣坛`)
- `576460752303428272` -> `assets/maps/esperanca2.png` (`埃斯佩兰萨`)
- `576460752303428437` -> `assets/maps/samoa2.png` (`萨摩亚`)
- `576460752303428190` -> `assets/maps/place-lacroix.jpg` (`拉克瓦广场`)
- `576460752303428183` -> `assets/maps/thames-district.jpg` (`泰晤士区`)
- `576460752303429014` -> `assets/maps/oasis-university.png` (`绿洲城 - 大学`)
- `576460752303428279` -> `assets/maps/colosseo.jpg` (`斗兽场`)
- `576460752303428469` -> `assets/maps/wuxing-university.jpg` (`五行大学 - 水学院`)
- `576460752303428266` -> `assets/maps/arena-victoriae.jpg` (`维多利亚竞技场`)
- `576460752303428488` -> `assets/maps/powder-keg-mine.jpg` (`火药桶矿井`)
- `576460752303428186` -> `assets/maps/gogadoro.jpg` (`高架车站`)

Deferred map GUIDs:
- `576460752303428212`
- `576460752303428683`
- `576460752303428223`

Implementation note:
The deferred GUIDs should continue to fall back to `query_tool.fightMapUrl`
until a later S23+ data sample confirms their map names and preview files.

## 2026-07-14 登录 BattleTag 自动识别与同步门禁

目标：
在网易大神浏览器登录流程中，从当前页面带“战网昵称”标签的文本自动识别 BattleTag，并安全地关联当前登录账号与单一同步目标；不恢复任意 BattleTag 输入或绑定入口。

已完成：
- 登录采集同时跟踪凭据角色和页面 BattleTag，只接受两次稳定、同源的完整观察；角色变化、缺失或冲突会使旧配对失效。
- 目标解析采用 `searchBnetAccount` 与 `queryCard` 两阶段校验，业务失败、身份不一致、过期请求和并发写入均失败关闭，旧目标不会被覆盖。
- `PlayerTarget` 记录来源账号关联；旧目标和其他账号目标不能直接解锁同步。
- Tauri 只暴露自动确认、切换确认、保留旧目标、清除和重新识别等安全动作；旧的任意绑定 command 与前端 API 已删除。
- 指挥台不再显示 BattleTag 输入框。多账号场景同时显示当前目标与检测到的登录账号，用户确认前禁用同步。
- 账号操作与赛季同步共用互斥门禁；同步在数据库、输出文件和上游请求前再次校验当前登录会话与目标。

自动化与烟测：
- `cargo fmt --check`：通过。
- 登录定向测试：78 项通过。
- `cargo test --tests`：通过；真实上游 live test 按设计忽略。
- `npm test`、`npm run build`：通过；仅有既有的大 chunk 警告。
- 合成 Tauri 浏览器烟测覆盖桌面与最小布局宽度、账号不一致、默认焦点、无手工输入、保留原目标和同步禁用状态。
- 产品入口扫描只在否定契约测试中命中旧命令名称；源码和 Tauri handler 无旧入口。
- `scripts/verify.ps1 -ChangedOnly` 在 Rust Clippy 前的敏感扫描、资源、前端、构建产物和格式检查均通过；Clippy 因 10 个早于本功能的生涯 MMR lint 退出 101，本功能未修改告警行。
- `git diff --check`：通过。

隐私与待验收：
- 全部自动化仅使用合成数据；未读取真实凭据、浏览器 profile、真实玩家数据或真实上游响应。
- 真实多账号切换、页面缺少 BattleTag、确认切换后的赛季同步需要用户在桌面端手工验收，只记录通过或失败，不记录账号值。
- `npm run tauri build` 已通过并生成 MSI 与 NSIS 安装包；尚未执行安装后烟测，不声称安装版运行验收已完成。
