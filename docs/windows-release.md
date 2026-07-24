# Windows Release Checklist

## Build

- `cd apps\owstats-desktop`
- `npm run test:text`
- `npm run test`
- `npm run build`
- `cd src-tauri`
- `cargo fmt --check`
- `cargo test --tests`
- `cd ..`
- `npm run tauri build` on a Windows release machine with the full Tauri toolchain.

## Release Configuration

- `src-tauri\tauri.conf.json` has `bundle.active` set to `true`.
- `src-tauri\tauri.conf.json` has `bundle.targets` set to `nsis`; Windows 正式发布只生成和发布 NSIS 安装程序，不发布 MSI。
- `src-tauri\tauri.conf.json` has `bundle.icon` set to `["icons/icon.ico"]`.
- `src-tauri\icons\icon.ico` exists.
- `app.security.csp` is constrained to `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' asset: https://asset.localhost; style-src 'self' 'unsafe-inline'; script-src 'self'`.
- Generated release artifacts appear under `src-tauri\target\release\bundle`.
- NSIS 必须在部署程序文件后、授予普通用户 `data` 权限前调用非 GUI 的受信任迁移 helper；旧 AppData 仅能作为只读迁移源。
- helper 必须拒绝重解析点、目录类型错误、未规范化的迁移源或目标，以及已发布 `data` 的覆盖；任一检查、复制或校验失败均 fail closed 并中止安装，不能读取后回退到 AppData。
- 安装器只允许为精确的 `<安装目录>\data` 设置受保护 DACL，使当前交互用户可写；不得放宽安装根或程序文件权限，并须以该用户令牌实际验证写入权限。
- 覆盖升级必须保留 `data`；默认卸载必须保留 `data`，只有用户明确选择后才能通过同一受边界校验的受信任 helper 删除它。

## Smoke Test

- 使用合成数据验收：全新安装、旧 AppData 迁移、选择新安装路径、覆盖升级、默认保留卸载和显式删除；不得使用真实凭据、真实用户数据、浏览器 profile 或本地 SQLite。
- 在默认路径、`Program Files`、非 C 盘和包含中文或空格的路径安装；普通用户能写入 `data`，但不能修改程序文件。
- 全新安装不创建业务 AppData；旧版升级仅复制旧 AppData 并保留原目录；静态 assets 可在安装版中访问且不进入 `data`。
- Clean Windows profile starts unauthenticated.
- "登录网易大神" opens Chrome or Edge.
- Saved credential status shows role_id and saved time without token, cookie, customerToken, or raw credential JSON.
- Season sync imports matches into local SQLite.
- Match filters cover season, hero, map, result, mode, date range, last 7 days, last 30 days, and clear/all.
- Match list restores filters from readable query params: `season`, `hero`, `map`, `result`, `mode`, `startDate`, `endDate`.
- Match detail same-role comparison hides missing metric cards and hides the whole block if no average is available.
- Ranking opens on "英雄胜率" / "地图胜率"; search filters the loaded list without backend calls.
- Review has no old map adaptability section.
- Analysis has no filters and shows friend analysis above map analysis.

## Troubleshooting

- Browser not found: install Chrome or Edge, then retry.
- Login timeout or token request failure: confirm the Dashen account is logged in and bound to the Overwatch account.
- Credential save failure: inspect the installation directory's `data` permissions; do not broaden permissions on program files.
- Sync credential rejection: use "重新登录" and retry sync.

Never attach or publish `data/.dashen_credentials.json`, token values, cookies, customerToken, or browser profile contents.
