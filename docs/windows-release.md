# Windows Release Checklist

## Build

- `cd apps\owstats-desktop\src-tauri`
- `cargo fmt --check`
- `cargo test --tests`
- `cd ..`
- `npm run build`
- `npm run tauri build` on a Windows release machine with the full Tauri toolchain.

## Smoke Test

- Clean Windows profile starts unauthenticated.
- “登录网易大神” opens Chrome or Edge.
- Saved credential status shows role_id and saved time without token, cookie, customerToken, or raw credential JSON.
- Season sync imports matches into local SQLite.
- Match filters cover season, hero, map, result, mode, date range, last 7 days, last 30 days, and clear/all.
- Match list restores filters from readable query params: `season`, `hero`, `map`, `result`, `mode`, `startDate`, `endDate`.
- Match detail same-role comparison hides missing metric cards and hides the whole block if no average is available.
- Ranking opens on “英雄胜率”; “地图胜率” search filters the loaded list without backend calls.
- Review has no old map adaptability section.
- Analysis has no filters and shows friend analysis above map analysis.

## Troubleshooting

- Browser not found: install Chrome or Edge, then retry.
- Login timeout or token request failure: confirm the Dashen account is logged in and bound to the Overwatch account.
- Credential save failure: inspect the non-sensitive app data directory permissions.
- Sync credential rejection: use “重新登录” and retry sync.

Never attach or publish `data/.dashen_credentials.json`, token values, cookies, customerToken, or browser profile contents.
