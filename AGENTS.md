# Agent Notes

Short handoff for future Codex sessions. Keep this file small.

## Current Shape

Project root:

```text
D:\Workspace\OW数据\astrbot_plugin_owstats777
```

The active product is the Tauri desktop app:

```text
apps/owstats-desktop/
```

Stack: Tauri 2, React, TypeScript, Rust, SQLite.

Old PySide6 MVP and static HTML report code have been removed after phase 2. Do not look for `ow_local_app/`, `scripts/season_report/`, or `data/reports/`.

## Useful Context

Read these first only when relevant:

```text
docs/raw-match-fields.md
docs/player-data-fetch.md
apps/owstats-desktop/src-tauri/src/
apps/owstats-desktop/src/
```

Default regression fixture:

```text
data/players/5479_s22_full_season.json
```

Known S22 counts:

- Total matches: 941
- Preset/open matches: 704
- Stadium/fight matches: 237

Phase-2 friend analysis is implemented in Rust backend analytics plus React pages. `match_players` is a rebuildable projection from `match_details`, not the source of truth.

Phase-4 product shape:

- Desktop onboarding owns NetEase Dashen login, credential status, and season sync entry.
- Installed-product login should use Tauri backend commands; Python credential scripts are diagnostics only.
- Match module is the only filter entry. Analysis and review must not add duplicate filters.
- Ranking owns hero win-rate and map win-rate. Old review map adaptability belongs under ranking's map tab.
- Analysis is full-season by default: friend analysis above map analysis.

## Frontend Language

All user-facing frontend text must be Chinese. This includes navigation labels, page titles, empty/loading states, metric labels, evidence link labels, backend DTO labels shown by React, and fallback labels returned to the UI. Keep internal API names, TypeScript field names, Rust enum/string keys, and third-party protocol values in their original form unless they are displayed directly.

## Keep

These are still useful for future development:

```text
apps/owstats-desktop/
data/players/5479_s22_full_season.json
data/ow_config_cache.json
docs/raw-match-fields.md
docs/player-data-fetch.md
scripts/fetch_dashen_credentials.py
scripts/fetch_player_data.py
scripts/download_ow_assets.py
Overstats/
```

`Overstats/` remains the protocol/resource reference. Avoid reading the whole directory; search targeted symbols first.

## Sensitive Data

Never print, commit, or expose:

- `OW_DASHEN_TOKEN`
- `customerToken`
- `data/.dashen_credentials.json`
- cookies or browser storage under `data/chrome-dashen-profile/`

Raw JSON viewers must redact token-like fields before display.

## Verification

Rust/Tauri backend:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Frontend:

```powershell
cd apps\owstats-desktop
npm install
npm run build
```

Python helper scripts:

```powershell
python -m py_compile scripts/fetch_player_data.py scripts/download_ow_assets.py scripts/fetch_dashen_credentials.py
python -m unittest discover -s tests -v
```

## Token-Saving Workflow

- Start with `AGENTS.md`, then inspect only the files touched by the request.
- Prefer `codebase-memory-mcp` for code discovery when available: `get_architecture`, `search_graph`, `trace_path`, `get_code_snippet`, `query_graph`, `search_code`, and `detect_changes`.
- Use `detect_changes` before larger edits or reviews to map git diff impact.
- Prefer `rg` over broad file reads.
- For Tauri bugs, begin at `src-tauri/src/commands.rs`, then jump to the called module.
- For UI bugs, begin at the page/component under `apps/owstats-desktop/src/`.
- Fall back to `rg` for string literals, config values, docs, generated files, or when graph results are insufficient.
- Do not read `Overstats/` broadly; use it only for exact API/resource references.
- Do not include generated artifacts, build output, `node_modules`, `target`, browser profiles, or local data dumps in prompts.
