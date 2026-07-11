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

## Portable Development Inputs

- `apps/owstats-desktop/runtime-config/ow-lookups.json` 是前后端共用的正式最小字典；桌面运行和构建不得读取被忽略的 `docs/reference/`。
- `apps/owstats-desktop/runtime-config/assets-inventory.json` 是资源构建白名单。资源变更后显式运行 `node scripts/prepareAssets.mjs --refresh-inventory` 并审查差异；普通 test/build 不调用 Git。
- `npm test`、`npm run build` 和 `npm run dev` 会先确定性重建 `public/assets`。生成目录不提交，资源 URL 固定为 `/assets/...`。
- Tauri 桌面运行路径统一由 `AppPaths` 从 `app_data_dir` 派生。数据库、设置、凭据、浏览器 profile、赛季 JSON、诊断和备份不得从仓库根推导。
- `CARGO_MANIFEST_DIR` 只允许显式开发 CLI 和测试使用；桌面 commands 不得依赖仓库布局。
- `npm run build` 的 postbuild 会扫描 `dist`，发现 `/@fs/`、开发机盘符路径或 `docs/reference` 时失败。

更新资源或路径代码后，除常规检查外还应从 `git archive HEAD` 创建 tracked-only 临时副本，执行前端测试、构建、Rust 测试和 `npm run tauri:dev -- --no-watch` 启动烟测。不要把真实 app-data、凭据、浏览器 profile 或玩家数据复制到临时副本。

## Commit Discipline

- Do not mix unrelated changes in one commit. Keep product behavior, release configuration, database migrations, text repair, and test-only changes separate when they can be reviewed or rolled back independently.
- Do not commit credentials, tokens, cookies, browser profiles, raw player dumps, local SQLite data, dependency folders, or generated build output.
- Check `git status --short` before every commit and before release. Only intentional source and documentation changes should be listed.
- If a change modifies the SQLite schema, add or update migration tests. At minimum, cover initializing the database twice so migrations stay idempotent.

## Privacy Migration

- 桌面程序初始化数据库时会执行一次 `privacy_sanitization_v1`：仅处理 app data 下的 SQLite raw 字段和 `seasons/` JSON，并先写入 `privacy-backups/<timestamp>/` 备份。
- 迁移会递归删除 token、cookie、authorization、credential 和 storage 类键；失败时不会写完成 marker，可在修复环境后安全重试。
- 不要手动删除 `privacy-backups` 以掩盖问题；它可能是恢复旧数据的唯一副本。凭据、浏览器 profile 和旧 `.dashen_player_target.json` 不属于该迁移的读取或备份范围。

## Rollback

If a release fails after installation, identify the smallest commit that introduced the failure and revert that commit first.

For code or UI regressions, prefer reverting the focused commit instead of mixing rollback with new fixes. For database migrations, preserve existing local data: migrations should be forward-compatible, idempotent, and safe to run on databases created by older releases. If a schema change cannot be safely rolled back, ship a new forward migration that restores application behavior without deleting user data.
