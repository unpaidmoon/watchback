# Phase 4 Windows Productization Design

## Status

This is the first confirmed design slice for Phase 4. The phase is expected to include more work than this document currently covers. Additional Phase 4 capabilities will be appended after user review.

## Goal

Phase 4 turns the current single-user Tauri desktop app into a Windows 11 installable product path: install, log in to NetEase Dashen through Chrome or Edge, save credentials locally without exposing secrets, sync the existing target season, and enter the already-built review and match analysis pages.

The phase also productizes the core analysis surface after Phase 3:

- Match details gain a Phase 3-style same-season average comparison.
- A new ranking module exposes hero win-rate and map win-rate rankings.
- The analytics module is simplified into a two-row dashboard: friend analysis above, map analysis below.
- Filtering belongs only to the match module. Analytics pages should show season-level summaries without their own filter controls.

## Explicit Non-Goals

- Do not add multi-account support.
- Do not add multi-player product flows.
- Do not expand the product toward multi-season management beyond the current season input and sync path.
- Do not port desktop functionality back into Astrbot command handlers.
- Do not replace the browser-assisted login flow with an embedded Tauri WebView.
- Do not remove the existing development workflow. `npm run dev`, backend tests, and helper scripts remain available for local debugging.

## Current Context

The active product is `apps/owstats-desktop`, a Tauri 2, React, TypeScript, Rust, and SQLite desktop app. Phase 2 implemented friend analytics. Phase 3 implemented personal season review and baseline comparison, including backend review aggregation and the `PersonalReviewPage` frontend.

The repository already contains browser-assisted Dashen credential extraction in `scripts/fetch_dashen_credentials.py`. The desktop sync backend already reads credentials from `data/.dashen_credentials.json`. The missing product link is the Tauri-first login experience: a user-facing button, progress events, safe credential storage, status refresh, and release validation.

## Recommended Approach

Use the existing Chrome or Edge CDP login technique as the product login path, but implement the installed desktop flow in the Tauri backend rather than requiring users to run Python scripts. Python scripts stay as development and diagnostic tools.

The recommended first implementation route is to port the required CDP credential extraction behavior to Rust:

- The Windows installer should not depend on a user-installed Python runtime.
- Rust-side commands can emit Tauri progress events directly.
- Credentials can be written through the same backend path that reports credential status.
- Error handling and redaction can reuse the desktop backend conventions.

If the Rust port proves unexpectedly large, a packaged helper executable is an acceptable fallback for the plan, but a raw Python script invocation is not the preferred installed-product design.

## User Flow

1. User installs and opens `OWStatsLocal` on Windows 11.
2. The first setup page shows credential status, local data status, and season sync controls.
3. User clicks `登录网易大神`.
4. Tauri backend starts a credential login task and opens Chrome or Edge with remote debugging enabled.
5. User logs in to NetEase Dashen in the browser and reaches the OW-linked account/activity page.
6. Backend detects `role_id`, requests the report token through the page signing helper, and receives `role_id/token`.
7. Backend writes credentials to local storage and emits a success progress event.
8. Frontend refreshes credential status and shows that credentials are saved. It never shows token, cookies, or customerToken.
9. User clicks `同步赛季`.
10. Existing Rust sync reads the saved credentials, imports match data into SQLite, and emits sync progress.
11. Once local data is readable, existing routes allow entering personal review, match list, match details, and friend analytics.

## Backend Design

Create a focused credentials module under `apps/owstats-desktop/src-tauri/src/credentials.rs`.

Responsibilities:

- Locate the local credential file.
- Read credential status without exposing secrets.
- Start and cancel browser-assisted login.
- Emit credential login progress events.
- Save credentials atomically.
- Redact token-like data from all user-facing errors.

Keep existing module boundaries:

- `sync.rs` continues to perform season sync and read existing credentials.
- `review.rs` continues to compute onboarding data health and review readiness.
- `commands.rs` exposes Tauri commands and delegates to modules.
- `redaction.rs` remains the shared place for token/cookie/customerToken sanitization behavior.

Proposed Tauri commands:

- `get_credential_status() -> CredentialStatus`
- `start_dashen_login() -> CredentialLoginResult`
- `cancel_dashen_login() -> CredentialLoginCancelResult`

Optional command if lightweight upstream validation is practical:

- `validate_credentials() -> CredentialValidationStatus`

`start_dashen_login` should run asynchronously and emit `credential-login-progress` events. It should not block the UI thread.

Suggested progress phases:

- `locating_browser`
- `opening_browser`
- `waiting_for_login`
- `role_id_found`
- `requesting_token`
- `saving_credentials`
- `saved`
- `failed`
- `cancelled`

## Credential Storage

The sync path currently reads `data/.dashen_credentials.json`. Phase 4 must make the desktop app use a writable local data directory for installed Windows builds while preserving the development path for `npm run dev`.

The stored JSON should contain only fields required for sync and diagnostics:

```json
{
  "role_id": 114514191,
  "token": "<secret>",
  "dts": null,
  "server": 1,
  "saved_at": "2026-06-27T00:00:00Z"
}
```

Frontend and logs may show:

- whether credentials exist
- whether required fields are present
- `role_id`
- `saved_at`
- non-sensitive file location or directory when useful for troubleshooting

Frontend and logs must not show:

- token
- cookie
- customerToken
- raw credential file contents
- browser profile contents

Credential writes should be atomic: write a temporary file in the same directory, flush it, then replace the credential file.

## Frontend Design

Update `OnboardingPage.tsx` from an external credential notice into a first-run product setup flow.

Credential section:

- Shows one of: `未登录`, `登录中`, `已保存`, `可能失效`, `保存失败`.
- Primary action is `登录网易大神`.
- When credentials exist, the primary action becomes `重新登录`.
- During login, show progress messages from `credential-login-progress`.
- Provide a cancel action while waiting for login.
- Never show or copy token values.

Season sync section:

- Keep the existing season number input, defaulting to the current project default of 22 until the product chooses another default.
- Disable sync when credentials are missing or incomplete.
- If sync fails with a credential-like failure, guide the user to log in again.
- Keep existing sync progress display.

Local data section:

- Keep existing SQLite status: data source, preset/open match count, detail count, last updated value, freshness risk, and warnings.
- Existing readable local data can still unlock review even when a fresh sync is not run in the current session.

Review entry:

- Enable `进入个人赛季复盘` when local data is readable.
- Keep existing routes and pages unchanged unless they need credential-aware error copy.

All user-facing frontend text remains Chinese.

## Match Module Enhancements

The match module remains the only place with filters. Existing match list filters should continue to drive match browsing and match-detail entry. Phase 4 should not add duplicate filtering controls to review, ranking, or analytics pages.

Match detail should add a Phase 3 comparison block that compares the selected match against same-season averages. The goal is to answer: "这局比我这个赛季的常态强在哪里、弱在哪里?"

Recommended comparison content:

- Core output: KDA, final blows, eliminations, damage, healing, deaths, and win/loss result where data is available.
- Hero context: if the selected match has a dominant hero or known hero usage, compare that hero's match performance against the player's same-season average for that hero when practical; otherwise fall back to all-hero season average.
- Map context: show the map and mode beside the comparison, but avoid turning match detail into a full map analytics page.
- Visual treatment: compact metric rows or tiles using `高于均值`, `低于均值`, and neutral states. Avoid large marketing-style cards.

The comparison should tolerate missing fields. If an upstream field is absent or unreliable, the UI should omit that metric rather than show a misleading zero.

## Ranking Module

Add a new top-level ranking module. It should be a local season ranking surface, not a global leaderboard and not a multi-player feature.

Ranking tabs:

- `英雄胜率`: list each hero with matches, wins, win rate, and supporting performance columns already available from local data. Include enough context to prevent tiny samples from looking authoritative.
- `地图胜率`: move the current review module's map adaptability capability here, preserving the original map adaptability behavior while giving it a clearer home.

Recommended hero ranking columns:

- Hero name.
- Matches.
- Wins.
- Win rate.
- Average KDA or death pressure metric if already derivable.
- Optional supporting columns such as average damage, healing, eliminations, or final blows when the local schema supports them reliably.

Recommended map ranking columns:

- Map name.
- Mode when available.
- Matches.
- Wins.
- Win rate.
- The existing map adaptability result or label currently shown in review.

Sorting defaults:

- Hero ranking defaults to win rate with a minimum-sample guard.
- Map ranking defaults to win rate with a minimum-sample guard.
- Users can sort by matches to inspect high-volume heroes or maps.

The ranking module should not introduce multi-season management. It uses the currently synced season data.

## Analytics Module Redesign

The analysis module should remove its own filters. Any filtering workflow remains in the match module. Analytics should present season-level summaries for quick reading.

Layout:

- Keep the existing friend analysis as the top row.
- Add map analysis as the bottom row.
- Both rows use four summary items and align visually: same density, same card rhythm, same typography scale, and same responsive behavior.
- On narrow screens, the eight items can stack while preserving row grouping: friend analysis first, map analysis second.

Friend analysis keeps its existing behavior and four-item structure.

Map analysis adds four season-level summary items with names that match the app's light, playful tone:

- `王牌地图`: highest win-rate map among maps that pass the minimum-sample guard.
- `翻车路段`: lowest win-rate map among maps that pass the minimum-sample guard.
- `常驻战场`: map with the most matches.
- `稀有来客`: map with the fewest matches among maps that have at least one match.

Minimum-sample handling:

- Win-rate best/worst should avoid over-celebrating or over-penalizing one-off maps. Use the same minimum-sample rule as the ranking module where possible.
- If no map reaches the minimum sample, show an explicit insufficient-sample state instead of a fake winner or loser.
- Most/least match counts can include all maps with at least one match.

The map analysis row should link into the new ranking module's `地图胜率` tab for deeper inspection.

## Error Handling

Expected failures and product responses:

- Browser not found: ask the user to install Chrome or Edge, then retry.
- DevTools port unavailable: explain that another automation browser may be running and ask the user to retry after closing it.
- Timeout before login: tell the user the login window expired and provide a retry button.
- `role_id` found but token request failed: ask the user to confirm the Dashen account is logged in and bound to the OW account.
- Credential save failed: show a non-sensitive local directory path and explain that the app could not write local data.
- Sync rejects saved credentials: show a redacted error and guide the user to `重新登录`.

All backend errors that may contain upstream payloads pass through redaction before reaching the UI.

## Windows Release Acceptance

Phase 4 release validation for this design slice:

- Windows 11 installer builds successfully.
- Installed app launches without the development server.
- First setup page displays an unauthenticated credential state on a clean profile.
- `登录网易大神` opens Chrome or Edge.
- After user login, the app saves credentials and refreshes status.
- Credential status does not expose token, cookies, or customerToken.
- Season sync reads the saved credentials and imports data.
- Sync progress and failures remain visible and redacted.
- Existing local data survives app restart.
- Personal review and match list can be opened after sync or when readable local data already exists.
- Match detail shows same-season average comparison when enough local data exists.
- The ranking module exposes hero win-rate and map win-rate tabs.
- The review module no longer owns map adaptability as its primary home; the behavior is preserved under ranking map win rate.
- Analytics has no independent filters and shows friend analysis above map analysis, with four summary items per row.
- `npm run dev` remains usable for development.
- `cargo test --tests`, `npm run build`, and Tauri build are part of the release checklist.

## Documentation Updates

Update documentation after implementation:

- README: Windows install, first login, season sync, and privacy notes.
- `docs/windows-release.md`: release checklist, smoke test steps, and troubleshooting.
- `AGENTS.md`: current Phase 4 shape, automatic login now belongs to the desktop onboarding flow, Python credential scripts remain diagnostic tools.

## Phase 4 Extension Points

The confirmed slice above is not the full Phase 4. Additional Phase 4 features can be added if they preserve the agreed product direction:

- Windows-first desktop stability.
- Single-user local app.
- No multi-account, multi-player, or Astrbot command expansion.
- No secret exposure.
- No removal of local development and debugging paths.

Candidate additions should be evaluated against the Windows release goal before entering the implementation plan.
