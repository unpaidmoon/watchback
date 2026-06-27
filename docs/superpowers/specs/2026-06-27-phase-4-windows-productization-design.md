# Phase 4 Windows Productization Design

## Status

This design is the confirmed Phase 4 scope for the Windows-first productization pass. It replaces earlier partial notes and is ready to drive an implementation plan after the self-review in this document.

## Goal

Phase 4 turns the current single-user Tauri desktop app into a Windows 11 installable product path: install, log in to NetEase Dashen through Chrome or Edge, save credentials locally without exposing secrets, sync the current season, and use product-ready match, ranking, review, and analysis surfaces.

Phase 4 also consolidates the post-Phase-3 product surface:

- Match detail gains same-role, same-season average comparison.
- A new top-level ranking module owns hero win-rate and map win-rate rankings.
- The review module keeps personal season review, but removes its old map adaptability area.
- The analysis module becomes an unfiltered season-level dashboard: friend analysis above map analysis.
- The match module is the only filtering entry point.

## Explicit Non-Goals

- No multi-account support.
- No multi-player product flows.
- No Astrbot command or callback expansion.
- No Astrbot backflow for desktop-only functionality.
- No embedded Tauri WebView login.
- No duplicate analytics filters outside the match module.
- No ranking secondary detail pages for heroes or maps in Phase 4.
- No manual sorting controls in ranking tables.
- No removal of the development workflow. `npm run dev`, backend tests, and diagnostic Python scripts remain available.

## Current Context

The product app is `apps/owstats-desktop`, using Tauri 2, React, TypeScript, Rust, and SQLite.

Relevant current boundaries:

- Frontend routes live in `apps/owstats-desktop/src/App.tsx`.
- Frontend API wrappers and DTOs live in `apps/owstats-desktop/src/shared/api.ts` and `apps/owstats-desktop/src/shared/types.ts`.
- Match browsing and details live in `apps/owstats-desktop/src/pages/MatchesPage.tsx`, `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`, and `apps/owstats-desktop/src/features/matches/FilterBar.tsx`.
- Friend analysis currently lives in `apps/owstats-desktop/src/pages/FriendAnalyticsPage.tsx` and still has its own filters.
- Personal review currently lives in `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx` and still renders map adaptability from `PersonalSeasonReview.maps`.
- Backend commands are exposed from `apps/owstats-desktop/src-tauri/src/commands.rs`.
- Match, analytics, review, import, and schema logic live in `queries.rs`, `analytics.rs`, `review.rs`, `importer.rs`, and `db.rs`.
- Browser-assisted Dashen credential extraction already exists in `scripts/fetch_dashen_credentials.py` and remains a development diagnostic path.

## Product Architecture

The installed Windows app is Tauri-first. Browser-assisted Dashen login is exposed as a desktop button and implemented in the Tauri backend so the installer does not depend on a user-installed Python runtime.

The Python credential script remains available for development and troubleshooting, but installed-product login must run through Tauri commands and events.

Data stays local:

- Credentials are stored locally and never displayed.
- SQLite remains the source of product views after sync/import.
- Ranking, analysis, review, and match detail all aggregate from the local SQLite data for the currently synced season.

## User Flow

1. User installs and opens `OWStatsLocal` on Windows 11.
2. The setup page shows credential status, local data status, and season sync controls.
3. User clicks `登录网易大神`.
4. Tauri starts a credential login task and opens Chrome or Edge with remote debugging enabled.
5. User logs in to NetEase Dashen in the browser.
6. Backend detects `role_id`, requests the report token through the page signing helper, and receives `role_id/token`.
7. Backend writes credentials to local storage and emits success.
8. Frontend refreshes credential status and shows that credentials are saved. It never shows token, cookies, customerToken, or raw credential content.
9. User clicks `同步赛季`.
10. Existing Rust sync reads saved credentials, imports match data into SQLite, and emits sync progress.
11. After local data is readable, the app allows review, match list, match detail, ranking, and analysis entry.

## Credential Login Design

Create `apps/owstats-desktop/src-tauri/src/credentials.rs`.

Responsibilities:

- Resolve the credential file path for installed and development builds.
- Read credential status without exposing secrets.
- Start and cancel browser-assisted login.
- Locate Chrome or Edge on Windows.
- Open the selected browser with a temporary remote-debugging profile.
- Poll CDP for the Dashen login page and extract `role_id/token`.
- Save credentials atomically.
- Emit redacted progress and error events.

Tauri commands:

- `get_credential_status() -> CredentialStatus`
- `start_dashen_login() -> CredentialLoginResult`
- `cancel_dashen_login() -> CredentialLoginCancelResult`

Progress event:

- Event name: `credential-login-progress`
- Phases: `locating_browser`, `opening_browser`, `waiting_for_login`, `role_id_found`, `requesting_token`, `saving_credentials`, `saved`, `failed`, `cancelled`

Engineering defaults:

- Prefer Chrome, then Edge when both are installed.
- Use a bounded login timeout of 10 minutes.
- Cancellation stops polling and leaves any already saved valid credentials untouched.
- Errors returned to UI always pass through `redaction.rs`.
- The login task must not block the UI thread.

## Credential Storage

Installed Windows builds store credentials in the app data directory. Development builds may continue reading and writing the existing repository-local diagnostic path when explicitly run through scripts.

Stored JSON contains only sync-required fields and non-secret diagnostics:

```json
{
  "role_id": 114514191,
  "token": "<secret>",
  "dts": null,
  "server": 1,
  "saved_at": "2026-06-27T00:00:00Z"
}
```

UI and logs may show:

- credential existence
- required-field completeness
- `role_id`
- `saved_at`
- non-sensitive directory path for troubleshooting

UI and logs must not show:

- token
- cookie
- customerToken
- raw credential file contents
- browser profile contents

Writes are atomic: write a temporary file in the same directory, flush it, then replace the credential file.

## Onboarding and Sync UI

`OnboardingPage.tsx` becomes the first-run product setup flow.

Credential section:

- States: not logged in, logging in, saved, possibly invalid, save failed.
- Primary action: `登录网易大神`.
- If credentials already exist, primary action becomes `重新登录`.
- Show progress messages from `credential-login-progress`.
- Provide cancel while waiting for login.
- Never show or copy secret values.

Season sync section:

- Keep season input. Default remains the current project default, season 22, until changed by a later product decision.
- Disable sync when credentials are missing or incomplete.
- Credential-like sync failure tells the user to log in again.
- Existing sync progress remains visible and redacted.

Local data section:

- Keep SQLite status: data source, season, preset/open match count, detail count, last synced/imported time, freshness risk, and warnings.
- Existing readable local data can unlock the app even if no fresh sync ran in the current session.

All user-facing frontend text remains Chinese.

## Match Module

The match module is the only filtering entry point.

Filter dimensions:

- Season
- Hero
- Map
- Result
- Mode
- Date range

Filter behavior:

- Non-date filter changes immediately refresh the match list.
- Date range supports start date and end date.
- Date shortcuts: last 7 days and last 30 days.
- Date clear/all removes date limits.
- Date inputs refresh after a valid selection or after edit completion, not on every partial keystroke.

Existing date fields `dateFrom` and `dateTo` remain the frontend DTO names. Backend `MatchFilters.date_from` and `date_to` continue to receive ISO date strings in `YYYY-MM-DD` format.

## Match Detail Same-Role Comparison

Match detail appends a same-role season average comparison below existing detail content. It does not replace any existing match-detail feature.

Role source:

- Prefer raw upstream `roleType`.
- Confirmed raw values: `tank`, `dps`, `healer`.
- Frontend labels: `坦克`, `输出`, `支援`.
- Diagnostics may show the raw English value.
- Do not rewrite `healer` to `support` in storage.

Storage/import requirement:

- Preserve `roleType` from upstream payloads for current-player rows.
- Add local schema support for raw role values where needed. The engineering default is `match_players.role_type TEXT` because same-role comparisons are player-row level, while match summary can continue to show hero and match fields.

Comparison metrics:

- Kills
- Deaths
- Damage
- Healing

Backend data contract:

- Extend `MatchDetailView` with `sameRoleComparison`.
- Return current match values and same-role season average values explicitly for kills, deaths, damage, and healing.
- Return raw role and display role label.
- Return `null` average for a metric when the backend cannot compute a reliable same-role average.
- Do not require or expose sample count in the primary UI contract.

Frontend behavior:

- Frontend computes displayed deltas from backend current values and average values.
- Display one row of four compact metric cards.
- Each card shows only the signed delta versus same-role average.
- Cards do not repeat the current match raw value.
- If one metric lacks an average, hide only that card.
- If all four metrics lack averages, hide the entire comparison block.
- Use signed numbers and red/green color cues.
- Death color semantics are inverted: fewer deaths than average is good, more deaths than average is bad.

## Ranking Module

Add a top-level navigation entry: `排行`.

Route defaults:

- Default route opens hero win rate.
- Tabs: `英雄胜率` and `地图胜率`.
- Switching tabs keeps the search text and applies it to the active tab.

Page behavior:

- Ranking page uses normal full-page scrolling.
- Do not implement an internally scrolling table region.
- Name search filters the already loaded list in real time on the frontend.
- Search does not issue backend queries per keystroke.
- No manual sorting controls.
- No hero or map secondary detail pages in Phase 4.

Hero win-rate columns:

- Hero name
- Matches
- Wins
- Losses
- Win rate
- Play time

Map win-rate columns:

- Map name
- Matches
- Wins
- Losses
- Win rate
- Play time

Backend data contract:

- Add ranking DTOs that explicitly return `matches`, `wins`, `losses`, `win_rate`, and `play_time_hours`.
- Frontend only displays these aggregates.
- Frontend must not derive wins, losses, match count, win rate, or play time.
- Win rate display uses one decimal place.
- Play time display uses decimal hours with one fractional digit.

Default sorting:

1. Win rate descending.
2. Matches descending.
3. Play time descending.
4. Name ascending as a stable final tie-breaker.

Map adaptability migration:

- Move the existing map adaptability capability out of review and into ranking's `地图胜率` tab.
- Preserve the capability as a secondary indicator or supporting content on the map ranking list.
- Do not create a map detail subpage.
- Remove the old map adaptability section from `PersonalReviewPage.tsx` directly. Do not leave a placeholder, redirect block, or "moved to ranking" card.

Ranking scope:

- Ranking is local season ranking, not a global leaderboard.
- Ranking uses the currently synced season.
- Ranking does not introduce multi-season management.

## Analysis Module

The analysis module removes its own filters. Filtering stays only in the match module.

Scope:

- Uses the current synced season's full local data.
- Does not split by mode.
- Does not filter by hero, map, result, or date.
- Friend analysis and map analysis read from the same full-season dataset.

Top row:

- Keep the existing friend analysis four items.
- Keep original names, content, and behavior.
- Remove the analysis page `FilterBar`.
- Existing friend detail behavior can remain if it is already part of the friend analysis card behavior.

Bottom row:

- Add four map analysis cards:
  - `王牌地图`: highest win-rate map with at least 5 matches.
  - `翻车路段`: lowest win-rate map with at least 5 matches.
  - `常驻战场`: map with the most matches.
  - `稀有来客`: map with the fewest matches among maps with at least one match.

Card display:

- Map name
- Win rate, formatted like ranking
- Match count

Tie-breaks:

- `常驻战场`: if maps tie on match count, choose the most recently played map.
- `稀有来客`: if maps tie on match count, choose the earliest played map.

Insufficient samples:

- `王牌地图` and `翻车路段` require at least 5 matches.
- If no map reaches 5 matches, show an insufficient-sample state for those cards rather than a fake winner or loser.
- `常驻战场` and `稀有来客` can use any map with at least one match.

Navigation:

- All four map analysis cards link to ranking's `地图胜率` tab.

## Data Contracts and Formatting

Backend remains authoritative for aggregation:

- Ranking aggregates wins, losses, matches, win rate, and play time.
- Analysis aggregates map card candidates and tie-break results.
- Match detail returns current values and averages; frontend computes only deltas.

Frontend formatting:

- Win rate: one decimal percentage, e.g. `52.4%`.
- Play time: one decimal hour value, e.g. `3.5 小时`.
- Signed deltas: include `+` for positive values and `-` for negative values.

Missing data:

- Missing single metric average hides only that metric card.
- Missing all comparison metric averages hides the whole comparison block.
- Missing map-analysis eligible winner/loser shows an insufficient-sample state.
- Ranking empty state explains that local season data is missing or has no qualifying rows.

## Error Handling

Expected failures and product responses:

- Browser not found: ask the user to install Chrome or Edge, then retry.
- DevTools port unavailable: explain that another automation browser may be running and ask the user to retry after closing it.
- Timeout before login: tell the user the login window expired and provide retry.
- `role_id` found but token request failed: ask the user to confirm the Dashen account is logged in and bound to the OW account.
- Credential save failed: show a non-sensitive local directory path and explain that the app could not write local data.
- Sync rejects saved credentials: show a redacted error and guide the user to `重新登录`.

All backend errors that may contain upstream payloads pass through redaction before reaching the UI.

## Windows Release Acceptance

Phase 4 is accepted when:

- Windows 11 installer builds successfully.
- Installed app launches without the development server.
- First setup page displays an unauthenticated credential state on a clean profile.
- `登录网易大神` opens Chrome or Edge.
- After user login, the app saves credentials and refreshes status.
- Credential status does not expose token, cookies, or customerToken.
- Season sync reads saved credentials and imports data.
- Sync progress and failures remain visible and redacted.
- Existing local data survives app restart.
- Match list supports season, hero, map, result, mode, start date, end date, last 7 days, last 30 days, and all/clear filters.
- Match detail shows same-role season deltas under the existing detail content when same-role averages exist.
- The delta block hides unavailable metric cards and hides itself when all four metrics are unavailable.
- Ranking appears as a top-level navigation item.
- Ranking defaults to `英雄胜率`.
- Hero ranking shows matches, wins, losses, win rate, and play time.
- Map ranking shows matches, wins, losses, win rate, and play time.
- Ranking uses backend-provided aggregates and default sorting only.
- Ranking search filters the loaded tab list in real time and survives tab switches.
- Review no longer renders the old map adaptability area.
- Map adaptability behavior is available under ranking's map win-rate tab.
- Analysis has no filters.
- Analysis shows existing friend analysis above the four new map analysis cards.
- Map analysis cards display map name, win rate, and match count, and link to ranking's map tab.
- `npm run dev` remains usable.
- Backend tests, frontend build, and Tauri build are part of the release checklist.

## Documentation Updates After Implementation

- README: Windows install, first login, season sync, ranking, analysis, and privacy notes.
- `docs/windows-release.md`: release checklist, smoke test steps, and troubleshooting.
- `AGENTS.md`: Phase 4 shape, desktop onboarding login, ranking ownership, and Python scripts as diagnostics.

## Design Self-Review

Coverage:

- All confirmed points are represented as explicit product behavior, data contract, or acceptance criteria.
- Same-role comparison is appended below existing match detail, uses `roleType`, returns backend current/average values, and computes frontend deltas.
- Ranking is a top-level module with the confirmed tabs, columns, formatting, sorting, search, and scroll behavior.
- Review map adaptability is removed from review and moved into ranking without a placeholder.
- Analysis removes filters, keeps friend analysis unchanged, and adds the four map analysis cards with confirmed sample and tie-break rules.
- Match filters are the only filters and include the confirmed date range behavior.

Conflict check:

- No requirement introduces multi-account, multi-player, Astrbot backflow, or embedded WebView login.
- Ranking and analysis both use local current-season data and do not introduce multi-season management.
- Frontend derives only the match-detail deltas; backend remains authoritative for ranking and analysis aggregates.

Placeholder check:

- The design contains no unfinished placeholder, deferred decision, or open question.

Scope check:

- The scope is large but coherent for one Phase 4 plan because the subsystems share the same desktop data model and release target. The implementation plan should still break the work into independently testable tasks: credential login, match detail comparison, ranking, analysis redesign, review cleanup, docs, and release validation.
