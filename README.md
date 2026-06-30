# OWStatsLocal

OWStatsLocal 是一个 Windows 优先的守望先锋本地数据分析桌面应用，主项目位于 `apps/owstats-desktop`。

当前产品形态：

- Tauri 2 + React + TypeScript + Rust + SQLite。
- 单用户、本机数据路径。
- 桌面端登录网易大神，同步赛季数据到本地 SQLite。
- 不做多账号、多玩家，不再维护聊天机器人插件路径。
- 不提交 token、cookie、customerToken、浏览器 profile、原始玩家隐私数据。

## 功能

- 对局列表：按赛季、英雄、地图、胜负、模式、日期筛选，筛选状态写入本机 URL query。
- 对局详情：展示队友/对手数据，并提供同职责本赛季均值差值对比。
- 排行：英雄胜率、地图胜率聚合，支持当前 tab 内名称搜索。
- 分析：全赛季好友分析和地图分析。
- 复盘：个人赛季复盘。
- 登录同步：通过 Tauri 后端打开 Chrome/Edge 登录网易大神，并在本地保存同步凭证。

## 开发

后端验证：

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

前端构建：

```powershell
cd apps\owstats-desktop
npm install
npm run build
```

调试启动：

```powershell
cd apps\owstats-desktop
npm run tauri:dev
```

## 数据目录

运行时数据位于项目根目录的 `data/` 或系统应用数据目录，具体取决于开发/安装路径。以下内容不应提交：

- `data/.dashen_credentials.json`
- `data/chrome-dashen-profile/`
- `data/assets/`
- `data/ow_config_cache.json`

## 维护

长期维护流程见 [`docs/maintenance.md`](docs/maintenance.md)。发布前至少运行后端测试、前端测试、前端构建和 Tauri 打包。
