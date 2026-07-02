# Agent Notes

Short handoff for future Codex sessions. Keep this file small.

## Current Shape

Before reading, editing, running commands, or starting a dev server, confirm the
working directory is this project root or a deliberate subdirectory under it.
as the active project. If a command is run from outside the root, use
`git -C D:\Codex\OW_data\owstats-local ...` or `cd D:\Codex\OW_data\owstats-local`
first.

The active product is the Tauri desktop app:

```text
apps/owstats-desktop/
```

Stack: Tauri 2, React, TypeScript, Rust, SQLite.

## Useful Context

Read these first only when relevant:

```text
apps/owstats-desktop/src-tauri/src/
apps/owstats-desktop/src-tauri/tests/
apps/owstats-desktop/src/
docs/windows-release.md
docs/superpowers/specs/
docs/superpowers/specs/main-thread-log.md
```

## Main Thread Workflow

This project uses a main-thread workflow for complex Codex work.

- The current main-thread session owns direction, scope, risk decisions, task
  prompts for execution threads, and final acceptance.
- Execution threads should receive narrow prompts with allowed paths,
  forbidden paths, verification commands, and a required evidence report.
- Verification threads are used for risky changes and should review only; they
  should not continue development.
- Record durable decisions and handoff prompts in
  `docs/superpowers/specs/main-thread-log.md`.

product shape:

- Desktop onboarding owns NetEase Dashen login, credential status, and season sync entry.
- Installed-product login uses Tauri backend commands.
- Match module is the only filter entry.
- Ranking owns hero win-rate and map win-rate.

Data fixture:

- `data/players/5479_s22_full_season.json` is the original anonymized full-season JSON data file.

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

- Start with this `AGENTS.md`, confirm `D:\Codex\OW_data\owstats-local` is the
  active project root, then inspect only the files touched by the request.
- Prefer codebase-memory-mcp for code discovery when available.
- Use `rg` over broad file reads.
- For Tauri bugs, begin at `src-tauri/src/commands.rs`, then jump to the called module.
- For UI bugs, begin at the page/component under `apps/owstats-desktop/src/`.
- Do not include generated artifacts, build output, `node_modules`, `target`, browser profiles, or local data dumps in prompts.
