# 0.3.0-beta.1 发布依赖安全复核

复核日期：2026-07-29

## 已修复

- `postcss` 从 `8.5.15` 升级到 `8.5.25`，消除 `GHSA-r28c-9q8g-f849`。
- `react-router-dom` 从 `6.30.4` 升级到 `7.18.2`，消除旧版客户端跳转与 SSR hydration 公告。

## 临时例外

`npm audit --omit=dev` 仍报告 `GHSA-qwww-vcr4-c8h2`。公告明确说明它只影响 unstable RSC API；Watchback 是 Tauri 桌面端，只使用客户端 `HashRouter`，没有 RSC、服务端 action 或 React Router 服务端运行时，因此当前产品路径不可达。

上游修复位于 `react-router 8.3.0`，但该版本要求 React 19.2.7，且没有对应的 `react-router-dom 8.3.0` 发布包。为避免在发布前引入无关的 React 主版本迁移，本次保留 `react-router-dom 7.18.2`。

参考：https://github.com/advisories/GHSA-qwww-vcr4-c8h2

## 解除条件

`react-router-dom` 发布兼容的修复版本后升级，并重新运行：

```powershell
npm audit --omit=dev
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```
