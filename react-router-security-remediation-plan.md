# React Router 安全告警稳定处置实施方案

更新日期：2026-08-03

适用分支：`codex/project-improvements`

关联公告：`GHSA-qwww-vcr4-c8h2`

## 1. 目标

在没有可用的官方稳定修复版本时，以可验证、可回滚的方式控制 React Router 供应链风险：

- 不为了清除审计数字而降级到包含更多已知漏洞的版本；
- 将当前依赖精确固定，避免未评估的自动漂移；
- 自动证明项目没有使用本公告涉及的 RSC/服务端 Router 能力；
- 每次仓库验证都运行真实的生产依赖审计；
- 只允许已评估的单一公告，任何新增公告立即阻断；
- 官方稳定修复发布后，以独立升级任务解除临时例外。

## 2. 事实基线

实施前状态：

- `react-router-dom` 使用开放范围 `^7.18.2`；
- 实际解析为 `react-router-dom@7.18.2` 和 `react-router@7.18.2`；
- `npm audit` 报告 2 个 high 条目，均来自 `GHSA-qwww-vcr4-c8h2` 的同一依赖链；
- 源码使用 `HashRouter`、`Routes`、`Route`、`Navigate`、`NavLink`、`useLocation`、`useNavigate`、`useSearchParams` 和 `useNavigationType`；
- 源码不使用 RSC、SSR、服务端 action 或 React Router 服务端入口；
- npm Registry 的 `latest` 仍为 `7.18.2`，公告标注的 `8.3.0` 尚不存在。

基线 `npm run check` 已通过。

## 3. 原降级方案为何撤销

原计划拟精确固定 `7.11.0`。实际试装后发现它虽然避开 `GHSA-qwww-vcr4-c8h2`，但重新命中多条旧公告，包括开放重定向、XSS、DoS、SSR/RSC 反序列化等问题；生产依赖审计仍报告 2 个 high。

因此，`7.11.0` 不满足“审计无 high”验收标准，也比 `7.18.2` 丢失更多安全修复。该试装已撤销，不进入最终变更。

禁止使用以下做法：

- `npm audit fix --force`；
- nightly、experimental、Git 提交或未发布版本；
- 永久忽略 `npm audit` 退出码；
- 通过删除锁文件或测试掩盖告警。

## 4. 最终处置设计

### 4.1 精确固定版本

将依赖从：

```json
"react-router-dom": "^7.18.2"
```

改为：

```json
"react-router-dom": "7.18.2"
```

锁文件必须仅解析 `react-router-dom@7.18.2` 和 `react-router@7.18.2`。

### 4.2 安全策略契约

新增 `scripts/routerSecurityPolicy.test.ts`，自动检查：

- manifest 中只能是精确版本 `7.18.2`；
- lockfile 顶层声明和两个 Router 包必须一致；
- `src` 中禁止引入 React Router RSC/服务端入口及关键 API；
- 未来升级或引入 RSC 时必须显式修改测试和安全政策，不能静默越过例外边界。

该测试加入 `test:resources`，因此 `npm test`、`npm run check`、仓库验证和 CI 都会执行。

### 4.3 可收紧的审计门禁

新增 `scripts/auditDependencies.mjs`：

1. 实际执行 `npm audit --omit=dev --json`；
2. 解析完整漏洞集合，不忽略命令结果；
3. 只允许 `react-router`/`react-router-dom` 依赖链上的 `GHSA-qwww-vcr4-c8h2`；
4. 无漏洞时正常通过；
5. 新增任何包、公告或公告集合变化时立即失败，要求重新评估。

`scripts/verify.ps1` 在当前工作树和 tracked-only checkout 中都运行该门禁。

### 4.4 安全政策同步

更新 `SECURITY.md`，记录：

- 当前精确版本；
- 公告为何在当前 Tauri SPA 中不可利用；
- 为什么拒绝降级；
- 自动控制措施；
- 例外失效条件与复查日期。

## 5. 验证方案

### 5.1 依赖与审计

```powershell
Set-Location apps\owstats-desktop
npm ls react-router react-router-dom
node scripts\auditDependencies.mjs
npx tsx scripts\routerSecurityPolicy.test.ts
```

预期：仅解析 7.18.2；审计只命中已记录公告并由严格例外门禁通过；安全策略契约通过。

### 5.2 前端回归

```powershell
npm run check
```

必须覆盖测试、类型检查、Vite 生产构建和构建产物路径扫描。

### 5.3 Rust 与仓库验证

```powershell
Set-Location src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

Set-Location ..\..\..
.\scripts\verify.ps1 -ChangedOnly
```

提交后运行完整 tracked-only 验证：

```powershell
.\scripts\verify.ps1
```

### 5.4 开发版人工回归

```powershell
Set-Location apps\owstats-desktop
npm run tauri:dev
```

检查启动重定向、所有一级导航、玩家查询子路由、查询参数、前进/后退、未知路由、焦点恢复以及 WebView 刷新后的 Hash 路由保持。不得使用真实凭据或记录隐私数据。

## 6. 验收标准

- [x] 拒绝不安全的 `7.11.0` 降级；
- [x] `react-router-dom` 精确固定为 `7.18.2`；
- [x] lockfile 仅解析一组 7.18.2 Router 包；
- [x] 新增版本/RSC 安全策略契约；
- [x] 新增只允许单一已评估公告的生产依赖审计门禁；
- [x] 当前工作树与 tracked-only checkout 都接入审计门禁；
- [x] `SECURITY.md` 与实际处置一致；
- [x] `npm run check` 在最终变更上通过；
- [x] Rust fmt、Clippy、全目标测试通过；
- [x] `verify.ps1 -ChangedOnly` 通过；
- [x] 独立提交完成；
- [x] 提交后的完整 tracked-only 验证通过；
- [x] 开发版路由人工回归完成或记录环境限制。

## 7. 回滚方案

本次变更只涉及前端依赖声明、锁文件、验证脚本、测试和文档，不修改用户数据、Rust 数据库或账号目录。

若门禁实现导致误判：

1. 不删除审计步骤；
2. 保存实际 `npm audit --json` 结果并确认公告身份；
3. 修正解析逻辑和相应测试；
4. 重新执行完整验证；
5. 只有在确认策略边界正确后才能提交。

## 8. 解除临时例外

满足以下条件后启动独立升级：

- npm Registry 已发布官方稳定修复版本；
- 该版本不再命中公告；
- Node、React、React DOM、Vite 与 Tauri 构建链要求可满足；
- 官方迁移说明可用。

升级时必须更新精确版本契约、删除对本公告的临时允许、运行本文全部自动化验证和开发版路由回归。

## 9. 实施记录

| 项目 | 结果 |
| --- | --- |
| 实施日期 | 2026-08-03 |
| 实施分支 | `codex/project-improvements` |
| 实施前版本 | `^7.18.2`，解析为 7.18.2 |
| 7.11.0 试装 | 撤销；命中多条历史公告，仍有 2 high |
| 最终版本 | 精确固定 7.18.2 |
| 风险边界 | 仅允许 `GHSA-qwww-vcr4-c8h2`，且禁止 RSC/服务端 Router API |
| 下次复查 | 2026-09-02 之前，或任何 Router/React/路由架构变更时立即复查 |
| 实施提交 | 本次独立安全处置提交 |
| 最终自动验证 | `npm run check`、`verify.ps1 -ChangedOnly` 及提交后的完整 tracked-only checkout 全部通过 |
| 开发版路由回归 | Tauri 开发版成功编译并启动；浏览器烟测覆盖全部一级导航、查询参数、前进/后退、刷新、未知路由和焦点。浏览器环境中的 Tauri `invoke` 错误为预期，未读取真实账号数据 |
