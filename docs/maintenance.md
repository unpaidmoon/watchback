# Maintenance Guide

## Daily Development

Work in small branches and small commits. Each branch should have one purpose, such as fixing text, changing release configuration, adding a database migration, implementing feature code, or updating tests.

Keep commits focused on one reason to change. If two changes would need different rollback decisions, split them into separate commits.

## Required Checks

Backend:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Frontend:

```powershell
cd apps\owstats-desktop
npm run test
npm run build
```

Release:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

## Commit Discipline

- Do not mix unrelated changes in one commit. Keep product behavior, release configuration, database migrations, text repair, and test-only changes separate when they can be reviewed or rolled back independently.
- Do not commit credentials, tokens, cookies, browser profiles, raw player dumps, local SQLite data, dependency folders, or generated build output.
- Check `git status --short` before every commit and before release. Only intentional source and documentation changes should be listed.
- If a change modifies the SQLite schema, add or update migration tests. At minimum, cover initializing the database twice so migrations stay idempotent.

## Rollback

If a release fails after installation, identify the smallest commit that introduced the failure and revert that commit first.

For code or UI regressions, prefer reverting the focused commit instead of mixing rollback with new fixes. For database migrations, preserve existing local data: migrations should be forward-compatible, idempotent, and safe to run on databases created by older releases. If a schema change cannot be safely rolled back, ship a new forward migration that restores application behavior without deleting user data.
