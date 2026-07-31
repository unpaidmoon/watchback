# Watchback 第三方声明

本文件说明 Watchback 使用、参考或随发行包提供的第三方软件、服务、数据和素材，以及它们与 Watchback MIT 许可证之间的边界。

## 1. Watchback MIT 许可证的适用范围

Watchback 项目作者原创的源代码按照项目根目录 `LICENSE` 中的 MIT License 授权。

除非对应权利人另有明确授权，Watchback 的 MIT License **不适用于**：

- 第三方开源组件本身；
- 从其他项目复制、修改或派生的代码；
- 《守望先锋》相关名称、商标、角色、地图、图标、图片、文本、音频、游戏数据及其他素材；
- 网易、网易大神或其他上游服务提供的内容、接口响应、配置和资源；
- 运行时下载、缓存或从远端服务取得的数据及素材；
- 本文件列出的其他第三方内容。

本文件仅用于署名和说明授权边界，不会为任何第三方内容创设新的许可。

## 2. Overstats

Watchback 的网易大神上游接口、玩家资料与战绩查询链路实现参考了：

- 项目：AddOneSecondL/Overstats
- 地址：<https://github.com/AddOneSecondL/Overstats>

截至 2026 年 7 月 31 日核对时，Overstats 仓库的 GitHub 元数据和根目录未提供可检测的 `LICENSE` 文件；其 `THIRD_PARTY_NOTICES.md` 同时提到了一个根目录 MIT 许可证。由于这两项信息不一致，不能仅凭该说明推定 Overstats 全部源码已经按 MIT License 授权。

因此：

- 本节是来源署名，不代表 Overstats 源码已自动纳入 Watchback 的 MIT License；
- Watchback 的 MIT License 不覆盖权利归属尚未确认的 Overstats 代码、素材或数据；
- 在公开分发包含上游复制或派生代码的版本前，应取得可核验的上游许可证、固定到明确包含许可证的提交，或获得原作者的明确授权；
- 若 Watchback 仅参考了接口行为并进行了独立实现，也应保留本节署名和相应实现记录。

Overstats 自身还提示，其图片、字体、游戏素材、上游配置衍生数据和运行时下载资源不默认随其源码许可证授权。Watchback 对这些内容同样不作额外授权。

## 3. 《守望先锋》、暴雪、网易与网易大神

Watchback 是非官方第三方项目，与暴雪娱乐、网易及网易大神不存在隶属、赞助、认可或官方合作关系。

“Overwatch”“守望先锋”、相关角色和地图名称、标志、图像及其他游戏内容，可能是 Blizzard Entertainment, Inc. 或其关联方的商标、版权作品或其他受保护内容。网易及网易大神的名称、标志、服务内容和上游响应归其相应权利人所有。

Watchback 仓库和发行包中的部分游戏相关资源可能来自或参考以下来源：

- Blizzard 官方站点及其静态资源域名；
- 网易或网易大神相关服务及静态资源域名；
- [OverFast API](https://overfast-api.tekrop.fr/)；
- 仓库内 `assets/manifest.json`、`assets/stadium/manifest.json` 及其他资源清单记录的具体 `source_url`。

这些素材、数据和商标不因被 Watchback 使用或缓存而转为 MIT 授权。资源清单中的 URL 仅用于记录来源，不等于来源网站授予再分发许可。准备公开分发或商业使用时，应逐项确认是否允许复制、修改、缓存和再分发；无法确认授权的内容应替换为原创、明确许可或运行时按需获取的资源。

## 4. JavaScript 与 TypeScript 组件

Watchback 前端直接依赖的主要组件如下。实际锁定版本以 `apps/owstats-desktop/package-lock.json` 为准。

| 组件 | 许可证 | 项目地址 |
| --- | --- | --- |
| React、React DOM | MIT | <https://github.com/facebook/react> |
| React Router DOM | MIT | <https://github.com/remix-run/react-router> |
| Tauri JavaScript API 与 CLI | Apache-2.0 OR MIT | <https://github.com/tauri-apps/tauri> |
| Vite | MIT | <https://github.com/vitejs/vite> |
| `@vitejs/plugin-react` | MIT | <https://github.com/vitejs/vite-plugin-react> |
| TypeScript | Apache-2.0 | <https://github.com/microsoft/TypeScript> |
| Lucide React | ISC | <https://github.com/lucide-icons/lucide> |
| tsx | MIT | <https://github.com/privatenumber/tsx> |
| React 与 React DOM 类型定义 | MIT | <https://github.com/DefinitelyTyped/DefinitelyTyped> |

## 5. Rust 组件

Watchback 桌面运行时直接依赖的主要 Rust 组件如下。实际锁定版本和传递依赖以 `apps/owstats-desktop/src-tauri/Cargo.lock` 为准。

| 组件 | 许可证 |
| --- | --- |
| Tauri、Tauri Build、Tauri Opener Plugin | Apache-2.0 OR MIT |
| anyhow、base64、chrono、futures-util、image、reqwest、serde、serde_json、sha2、tempfile、thiserror | MIT OR Apache-2.0 |
| rusqlite、tokio、tokio-tungstenite | MIT |
| uuid、windows-sys、zeroize | Apache-2.0 OR MIT |
| rustix | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |

以上表格只列出直接依赖的主要组件，不是完整的传递依赖清单。每个组件仍受其自身许可证、版权声明和 NOTICE 文件约束。

## 6. 系统运行时与外部服务

Watchback 可能与以下系统组件或外部服务交互，但该交互不表示这些项目或服务向 Watchback 提供背书：

- Microsoft Edge WebView2；
- Google Chrome 或 Microsoft Edge；
- 网易大神及相关上游服务；
- 用户自行配置的 DeepSeek API 或兼容服务；
- GitHub Releases。

用户使用外部服务时，还需遵守相应服务的条款、隐私政策和适用规则。

## 7. 发行与合规维护

发布 Watchback 安装包前，维护者应：

1. 以 `package-lock.json` 和 `Cargo.lock` 为输入生成完整的依赖许可证清单；
2. 保留所有适用的版权声明、许可证全文和 NOTICE 文件；
3. 核对新增或替换素材的来源、作者、许可证及是否允许再分发；
4. 确认 Overstats 相关实现具有清晰、可核验的授权依据；
5. 确保安装包中实际包含的内容与本文件一致；
6. 在依赖、素材来源或授权条件变化时同步更新本文件。

若发现遗漏或权利归属错误，请按照 [SECURITY.md](SECURITY.md) 中的私密渠道联系维护者，或在不包含敏感信息和未公开漏洞细节的前提下提交普通 Issue。
