# Agent Notes

Short handoff for future Codex sessions. Keep this file small.

## Current Shape

Project root:

```text
D:\Codex\OW_data\owstats-local
```

The active product is the Tauri desktop app:

```text
apps/owstats-desktop/
```

Stack: Tauri 2, React, TypeScript, Rust, SQLite.

The old chatbot plugin path, external reference tree, Python helper scripts, and local raw player data are no longer part of the maintained codebase.

## Useful Context

Read these first only when relevant:

```text
apps/owstats-desktop/src-tauri/src/
apps/owstats-desktop/src-tauri/tests/
apps/owstats-desktop/src/
docs/windows-release.md
docs/superpowers/specs/2026-06-27-phase-4-windows-productization-design.md
```

Phase-4 product shape:

- Desktop onboarding owns NetEase Dashen login, credential status, and season sync entry.
- Installed-product login uses Tauri backend commands.
- Match module is the only filter entry.
- Ranking owns hero win-rate and map win-rate.
- Analysis is full-season by default: friend analysis above map analysis.

## Frontend Language

All user-facing frontend text must be Chinese. This includes navigation labels, page titles, empty/loading states, metric labels, evidence link labels, backend DTO labels shown by React, and fallback labels returned to the UI. Keep internal API names, TypeScript field names, Rust enum/string keys, and third-party protocol values in their original form unless they are displayed directly.

## Sensitive Data

Never print, commit, or expose:

- `OW_DASHEN_TOKEN`
- `customerToken`
- `data/.dashen_credentials.json`
- cookies or browser storage under `data/chrome-dashen-profile/`
- raw player privacy identifiers from local API dumps

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

## Token-Saving Workflow

- Start with `AGENTS.md`, then inspect only the files touched by the request.
- Prefer codebase-memory-mcp for code discovery when available.
- Use `rg` over broad file reads.
- For Tauri bugs, begin at `src-tauri/src/commands.rs`, then jump to the called module.
- For UI bugs, begin at the page/component under `apps/owstats-desktop/src/`.
- Do not include generated artifacts, build output, `node_modules`, `target`, browser profiles, or local data dumps in prompts.
