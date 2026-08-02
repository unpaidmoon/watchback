# `codex/project-improvements` 合并主分支完整实施方案

更新日期：2026-08-03

实施分支：`codex/project-improvements`

实施位置：功能分支独立工作树

目标位置：主分支工作树

## 1. 目标

在不丢失主分支“对局列表动态刷新”能力、不恢复已删除的 BattleTag 重新绑定功能、不弱化依赖安全门禁的前提下，使 `codex/project-improvements` 达到可安全快进合并到 `main` 的状态。

最终结果必须同时满足：

- 登录发布前能够严格证明搜索结果、资料卡、BattleTag 与登录 `roleId` 属于同一账号；
- React Router 临时安全例外无法被动态导入、CommonJS 引入或构建脚本绕过；
- 主分支提交 `e966a91` 的对局时间线动态刷新完整保留；
- 用户可见的 BattleTag 重新绑定、切换、保留、重试和清除功能不会重新出现；
- 两份含义不同的实施方案不被错误拼接或覆盖；
- 合并后的前端、Rust、安全审计、开发版和 tracked-only 验证全部通过；
- 主分支最终使用 `--ff-only` 合并，不生成未经验证的新结果。

## 2. 当前基线

| 项目 | 当前值 |
| --- | --- |
| 功能分支 HEAD | `d0afbf5` |
| 主分支 HEAD | `e966a91` |
| 共同基点 | `e660203` |
| 主分支独有提交 | `e966a91 对局列表动态刷新` |
| 功能分支独有提交 | `140b406`、`1354c9a`、`d0afbf5` |
| 功能分支工作树 | 当前干净 |
| 主分支工作树 | 当前干净 |

`git merge-tree --write-tree main HEAD` 已确认直接合并存在两个未解决冲突：

1. `apps/owstats-desktop/src-tauri/src/commands.rs` 内容冲突；
2. `project-improvement-implementation-plan.md` add/add 冲突。

以下文件两侧都有修改，但 Git 当前能够自动合并，仍需人工复核语义：

- `apps/owstats-desktop/scripts/matchesPageCache.test.ts`；
- `apps/owstats-desktop/src-tauri/src/lib.rs`；
- `apps/owstats-desktop/src/shared/api.ts`；
- `apps/owstats-desktop/src/shared/types.ts`。

## 3. 当前发布阻断项

### 3.1 身份验证对字段缺失处理过于宽松

当前 `account_sync_identity.rs` 使用 `Option::is_some_and` 判断 `roleId` 不一致。这意味着字段缺失时条件为 false，流程可能继续成功。资料卡没有任何可识别 BattleTag 时，当前校验函数也会返回成功。

这不满足现有方案中的要求：登录账号发布前必须证明搜索结果、资料卡、BattleTag 和 `roleId` 一致。

必须改为 fail-closed：必要身份字段缺失、格式无效、存在冲突或值不一致时均拒绝发布。

### 3.2 React Router RSC 门禁覆盖不完整

当前门禁只扫描 `src`，并主要覆盖静态 `from` 导入。以下形式可能绕过：

```ts
await import("react-router/rsc");
require("react-router/dom/server");
```

构建脚本、Vite 配置或其他可执行 TypeScript/JavaScript 文件中的服务端入口也可能不在扫描范围内。

### 3.3 主分支功能尚未集成验证

功能分支自己的验证已通过，但它没有包含主分支新增的对局列表动态刷新。因此，现有绿色结果不能证明合并后的组合行为正确。

## 4. 不变量与非目标

### 4.1 必须保持的不变量

- BattleTag 只在登录流程中捕获，不恢复独立重新绑定入口；
- `customerToken` 只作为账号级内部同步身份，不进入前端状态、IPC 响应或日志；
- 持久化同步身份必须携带并匹配来源 `roleId`；
- 对局动态刷新事件必须保持账号隔离和赛季隔离；
- 当前选中的旧对局不能因首页动态刷新被错误清除；
- React Router 继续精确固定为 `7.18.2`，直至官方稳定修复版本可用；
- 不使用 `npm audit fix --force`、nightly、experimental 或未发布版本；
- 不读取、复制或记录真实凭据和玩家隐私数据。

### 4.2 本次不做

- 不迁移 React Router 8；
- 不重构整个账号系统或路由系统；
- 不改变对局动态刷新产品行为；
- 不修改用户数据库或账号目录格式；
- 不删除兼容性的 `player_target.json`；
- 不把无关功能追加到本次冲突解决提交。

## 5. 实施阶段

## 阶段 A：实施前快照与防漂移检查

在功能分支工作树执行：

```powershell
# 在功能分支工作树根目录执行
git status --short --branch
git rev-parse HEAD
git rev-parse main
git merge-base main HEAD
git log --oneline HEAD..main
git log --oneline main..HEAD
git merge-tree --write-tree main HEAD
```

记录当前三个基线 SHA。如果 `main` 不再是 `e966a91`，必须重新运行合并预演并更新本方案中的冲突清单，不能沿用旧判断。

如果任一工作树不干净，停止实施，不覆盖或混入用户修改。

## 阶段 B：先建立身份验证失败契约

修改生产代码前，增加本地 HTTP 合约测试，至少覆盖：

1. 搜索结果和资料卡都包含一致的 BattleTag、`roleId` 时成功；
2. 搜索结果缺少 `roleId` 时拒绝；
3. 搜索结果 `roleId` 不一致时拒绝；
4. 搜索结果缺少可验证 BattleTag 时拒绝；
5. 搜索结果 BattleTag 不一致时拒绝；
6. 资料卡缺少 `roleId` 时拒绝；
7. 资料卡 `roleId` 不一致时拒绝；
8. 资料卡缺少可验证 BattleTag 时拒绝；
9. 资料卡包含冲突 BattleTag 时拒绝；
10. 搜索或资料卡业务码非 0 时拒绝；
11. 401/403 分类为认证拒绝；
12. 超时、5xx 和连接失败分类为临时失败；
13. 任一失败路径都不能发布账号或覆盖已有同步身份。

测试应使用合成 BattleTag、role ID 和 token，不接触真实服务。

预期先得到红灯，证明测试能捕获当前宽松行为。

## 阶段 C：收紧账号同步身份验证

### C.1 搜索结果校验

搜索结果只有在以下条件全部满足时才可继续：

- 业务响应明确成功；
- `data` 是对象；
- `roleId`/`role_id` 存在、可解析，并且等于登录凭据 `role_id`；
- `customerToken` 存在且非空；
- 返回的 BattleTag 存在、格式有效，并与登录捕获值规范化后一致。

不得用“字段存在时才比较”的逻辑表达必要字段。

### C.2 资料卡校验

资料卡只有在以下条件全部满足时才可继续：

- 业务响应明确成功；
- `data` 是对象；
- `roleId`/`role_id` 存在、可解析，并与登录凭据一致；
- 至少存在一个格式有效的 BattleTag 字段；
- 所有可识别 BattleTag 值均与登录捕获值一致；
- 多个身份字段互相冲突时拒绝。

### C.3 减少两条验证路径漂移

`verify_credentials_with_base_url` 与 `resolve_with_base_url` 当前包含相似的搜索、role 校验、资料卡查询和身份校验。应抽取共享的内部验证流程，例如：

```text
resolve_verified_identity(credentials, battleTag, baseUrl)
```

共享流程返回验证后的内部身份或类型化失败；调用方只负责把失败映射为：

- `Authenticated`；
- `Rejected`；
- `TransientFailure`；
- 登录发布后的安全错误文本。

不得为复用而重新引入用户可操作的目标状态机。

## 阶段 D：补强 React Router 安全门禁

### D.1 集中安全例外元数据

新增一个被审计脚本与契约测试共同读取的机器可读策略文件，建议：

```text
apps/owstats-desktop/scripts/dependencySecurityPolicy.json
```

至少记录：

- 精确允许版本 `7.18.2`；
- 允许的生产依赖公告 URL；
- 例外到期日期 `2026-09-02`；
- 适用原因和解除条件的稳定标识。

`auditDependencies.mjs` 和 `routerSecurityPolicy.test.ts` 不再分别硬编码版本与公告集合。

### D.2 扩大扫描范围

递归扫描应用中的可执行 JavaScript/TypeScript 文件与构建配置，包括：

- `src/**/*.{js,jsx,ts,tsx,mjs,cjs}`；
- `scripts/**/*.{js,jsx,ts,tsx,mjs,cjs}`；
- `vite.config.*`；
- 其他实际参与构建或启动的根级配置文件。

排除：

- `node_modules`；
- `dist`；
- `public`；
- `src-tauri/target`；
- 纯测试夹具中专门用于验证检测器的字符串，但检测器测试必须单独执行。

### D.3 覆盖全部入口形式

检测器必须识别：

- `import ... from "模块"`；
- `export ... from "模块"`；
- `import("模块")`；
- `require("模块")`；
- 已知 RSC/服务端 Router API 标识符。

至少为每种形式建立一条“应拒绝”测试，并为普通 `react-router-dom` SPA 导入建立“应允许”测试。

### D.4 审计门禁保持收紧

继续实际执行：

```powershell
npm audit --omit=dev --json
```

允许逻辑必须满足：

- 无漏洞时通过；
- 只有当前已记录公告时受控通过；
- 出现新包、新公告、公告集合变化或 JSON 无法解析时失败；
- 不把非零退出码永久吞掉；
- 例外到期后必须失败并要求人工复查。

## 阶段 E：消除方案文档 add/add 冲突

当前功能分支与主分支都新增了 `project-improvement-implementation-plan.md`，但内容和用途不同。不得简单选择 `ours`、`theirs` 或把两份全文拼接。

在合并 `main` 前：

1. 将功能分支当前以 BattleTag 清理为主题的文档改名为：

   ```text
   battletag-rebinding-removal-implementation-plan.md
   ```

2. 保留文档历史和实施记录；
3. 原路径留给主分支的全项目改进方案；
4. 检查仓库引用并更新指向新文件名的链接。

这样可以在语义上消除 add/add 冲突，并避免以后再次覆盖不同方案。

该重命名单独提交，便于审查和回滚。

## 阶段 F：把主分支合入功能分支

确保阶段 B 至 E 已分别提交且工作树干净，然后执行：

```powershell
# 在功能分支工作树根目录执行
git merge --no-ff main
```

### F.1 `commands.rs` 冲突处理原则

最终文件必须同时包含：

- 主分支新增的 `MatchTimelineRevision` DTO；
- `match_timeline_revision_for`；
- `get_match_timeline_revision` Tauri 命令；
- 当前反馈地址等主分支有效修改；
- 功能分支的 `account_sync_identity` 登录发布与同步前校验；
- 功能分支对旧 BattleTag 重绑定命令的删除。

最终文件不得恢复：

- `get_player_target_status`；
- `get_login_target_status`；
- `confirm_login_target_switch`；
- `keep_current_player_target`；
- `retry_login_target_binding`；
- `clear_player_target`；
- `LoginTargetCoordinator` 或旧的用户态恢复状态机。

删除冲突标记后，使用 `rg` 明确验证保留项和禁止项。

### F.2 自动合并文件语义复核

#### `src-tauri/src/lib.rs`

必须同时：

- 注册 `get_match_timeline_revision`；
- 注册内部同步身份依赖所需模块；
- 不注册旧 BattleTag 重绑定命令。

#### `src/shared/api.ts`

必须保留 `getMatchTimelineRevision`，同时不得恢复旧登录目标/玩家目标 IPC adapter。

#### `src/shared/types.ts`

必须保留 `MatchTimelineRevision`，同时不得恢复 `LoginTargetStatus`、`PlayerTargetStatus` 等旧状态 DTO。

#### `matchesPageCache.test.ts`

必须保留主分支动态刷新测试，同时保留功能分支减少脆弱源码正则断言的测试结构改进。不能为了消除冲突而删除任一侧的行为断言。

## 阶段 G：合并后静态边界检查

执行：

```powershell
rg -n "get_match_timeline_revision|MatchTimelineRevision|shouldProbeMatchTimelineRevision" apps/owstats-desktop
rg -n "get_player_target_status|get_login_target_status|confirm_login_target_switch|keep_current_player_target|retry_login_target_binding|clear_player_target|LoginTargetCoordinator|TargetRecoveryPanel" apps/owstats-desktop
rg -n "^(<<<<<<<|=======|>>>>>>>)" . -g "!node_modules/**" -g "!target/**"
git diff --check
```

验收：

- 第一组保留项均存在且连接完整；
- 第二组遗留产品接口在生产代码中无匹配；
- 无冲突标记；
- 无空白错误。

## 阶段 H：自动化验证

### H.1 定向测试

```powershell
Set-Location apps\owstats-desktop
npx tsx scripts\accountSyncIdentity.test.ts
npx tsx scripts\routerSecurityPolicy.test.ts
npx tsx scripts\matchesPageCache.test.ts
npx tsx scripts\liveMatchTimeline.test.ts
node scripts\auditDependencies.mjs
npm ls react-router react-router-dom
```

Rust 定向测试应覆盖：

```powershell
Set-Location src-tauri
cargo test account_sync_identity
cargo test match_timeline_revision
cargo test --test phase8_real_fetch
cargo test --test phase4_login_redesign
```

### H.2 完整工作树验证

```powershell
# 回到功能分支工作树根目录
.\scripts\verify.ps1 -ChangedOnly
```

必须通过：

- 敏感信息与绝对路径扫描；
- 验证流程契约；
- 生产依赖安全审计；
- 前端全部测试和生产构建；
- Rust fmt、Clippy 和全量测试；
- `git diff --check`。

## 阶段 I：开发版人工回归

启动合并后的开发版：

```powershell
Set-Location apps\owstats-desktop
npm run tauri:dev
```

### I.1 对局动态刷新

- 打开对局列表后触发或模拟数据库 revision 变化；
- 页面能够探测新 revision；
- 新对局正确并入列表；
- 当前选中的旧对局仍可查看；
- 当前筛选、赛季、分页游标和滚动位置不被无故重置；
- 不同账号的事件不能修改当前账号列表；
- “全部赛季”能正确汇总多赛季失效通知。

### I.2 登录与同步身份

- 添加有效账号后直接显示登录时捕获的 BattleTag；
- 不出现重新绑定、切换、保留、重试或清除目标入口；
- 身份一致时能够同步；
- 身份缺失或不一致时在发布前失败；
- 401/403 与临时网络错误展示正确分类；
- 切换账号后只使用当前账号的内部同步身份。

### I.3 Router 与通用回归

- 所有一级 Hash 路由可进入；
- 查询参数、前进、后退和刷新保持正确；
- 页面标题焦点恢复正常；
- 开发者控制台没有新增 Router 或路由匹配警告；
- 不使用或记录真实凭据、token 和隐私数据。

## 阶段 J：提交合并结果并执行 tracked-only 验证

确认合并冲突完全解决后提交：

```powershell
git status --short
git diff --check
git add -- <明确列出的冲突解决文件>
git commit
```

不得用未经审查的 `git add -A` 混入临时文件或用户修改。

提交后运行：

```powershell
.\scripts\verify.ps1
```

完整验证必须证明：

- `git archive HEAD` 包含所有必要文件；
- `npm ci` 能从锁文件重建；
- 安全策略文件、审计脚本和 Router 契约均被跟踪；
- 前端测试与构建在纯净 checkout 通过；
- Rust 测试在纯净 checkout 通过；
- 不依赖现有 `node_modules`、未跟踪文件或本机绝对路径。

## 阶段 K：最终合并到主分支

### K.1 合并前门禁

只有以下条件全部满足才能继续：

- 功能分支工作树干净；
- 主分支工作树干净；
- `main` 仍是已验证时使用的同一个提交；
- 功能分支完整验证通过；
- tracked-only 验证通过；
- 开发版人工回归通过；
- `git merge-base --is-ancestor main codex/project-improvements` 返回成功。

### K.2 快进合并

```powershell
# 在主分支工作树根目录执行
git status --short --branch
git merge --ff-only codex/project-improvements
```

使用 `--ff-only` 的原因是：被测试的功能分支 HEAD 已经包含主分支。如果 Git 不能快进，说明主分支又发生变化，应停止并重新集成、重新验证，而不是临时生成一个未经测试的合并提交。

合并后检查：

```powershell
git status --short --branch
git log -5 --oneline
git rev-parse HEAD
```

本方案默认不自动 push。推送远端必须由用户单独授权。

## 6. 建议提交拆分

建议保留以下可审查、可回滚的提交边界：

1. `fix: fail closed on incomplete account identity`
2. `test: harden React Router security boundary`
3. `docs: separate BattleTag removal plan`
4. `merge: integrate latest main into project improvements`

如果冲突解决只产生机械整合，放入合并提交；如果需要修改动态刷新或身份业务逻辑，应使用独立修复提交并重新执行完整验证。

## 7. 回滚方案

### 7.1 合并主分支前

每阶段独立提交。某阶段失败时，只回滚该阶段提交，不使用 `git reset --hard`，不覆盖工作树中的其他修改。

### 7.2 功能分支已经合入主分支但尚未发布

使用常规 `git revert` 回滚对应提交或合并提交；不要改写已共享历史。

### 7.3 已发布

- 停止继续发布受影响构建；
- 回滚应用提交，不修改用户账号目录或数据库；
- 保留兼容性的内部同步身份文件；
- 重新运行完整验证并生成新构建；
- 对身份验证失败优先修复自动解析，不恢复用户可见的重新绑定状态机。

## 8. 风险矩阵

| 风险 | 可能性 | 影响 | 控制措施 |
| --- | ---: | ---: | --- |
| 冲突解决时丢失对局动态刷新命令 | 中 | 高 | 明确保留符号清单、定向测试、人工动态刷新回归 |
| 冲突解决时恢复旧 BattleTag 状态机 | 中 | 高 | 禁止符号扫描、前端/Rust 契约测试 |
| 严格身份验证与真实上游字段不兼容 | 中 | 高 | 合成 HTTP 合约、脱敏开发版验证、失败分类清晰、可回滚提交 |
| RSC 门禁出现漏检 | 中 | 高 | 扫描构建范围、覆盖静态/动态/CommonJS 形式、检测器单元测试 |
| RSC 门禁误报自身测试字符串 | 中 | 低 | 将检测函数与扫描入口分离，明确排除检测器夹具并测试允许项 |
| 两份方案文档再次相互覆盖 | 高 | 中 | 按主题重命名，原路径保留主分支全项目方案 |
| 主分支在验证后继续前进 | 中 | 高 | 记录 SHA，最终只允许 `--ff-only`，变化后重新集成验证 |
| 纯净环境缺少新策略文件 | 低 | 高 | 提交后运行 tracked-only checkout |

## 9. 完成定义

- [x] 主分支与功能分支 SHA 已重新确认；
- [x] 身份字段缺失和不一致测试先失败后通过；
- [x] 搜索结果必须具备一致 BattleTag 与 `roleId`；
- [x] 资料卡必须具备一致 BattleTag 与 `roleId`；
- [x] 失败路径不会发布账号或覆盖内部同步身份；
- [x] Router 门禁覆盖静态导入、动态导入和 `require`；
- [x] Router 门禁扫描源码、脚本和构建配置；
- [x] 安全例外元数据集中且带到期日期；
- [x] BattleTag 方案文档已按主题重命名；
- [x] 最新 `main` 已合入功能分支；
- [x] `commands.rs` 冲突正确解决；
- [x] 对局动态刷新命令、DTO、API 和测试完整保留；
- [x] 旧 BattleTag 用户态入口、API、命令和模型保持删除；
- [x] 自动合并文件完成语义复核；
- [x] 无冲突标记和 diff 空白错误；
- [x] 身份、Router、对局动态刷新定向测试通过；
- [x] `verify.ps1 -ChangedOnly` 通过；
- [x] Tauri 开发版人工回归通过；
- [ ] 提交后的完整 tracked-only 验证通过；
- [ ] 两个工作树均干净；
- [ ] 主分支通过 `git merge --ff-only` 合并；
- [ ] 合并后 HEAD 与已验证功能分支 HEAD 一致。

## 10. 实施记录

```text
实施日期：2026-08-03
实施人员：Codex（子任务并行审计与修复）
实施前功能分支 HEAD：d0afbf5
实施前 main HEAD：e966a91
共同基点：e660203

身份验证修复提交：36a6f31
Router 门禁修复提交：c75dc4e
文档重命名提交：03d3c38
主分支集成提交：b0e4851
最终功能分支 HEAD：待最终文档提交后记录
最终 main HEAD：待快进合并后记录

身份定向测试：通过（Rust 单元测试与 phase8_real_fetch）
Router 安全门禁：通过（策略、检测器及真实 npm audit 受控例外）
对局动态刷新定向测试：通过（前端缓存、实时刷新及 Rust revision 测试）
npm run check：通过
cargo fmt：通过
cargo clippy：通过
cargo test：通过
verify -ChangedOnly：通过
开发版人工回归：Tauri 开发版成功启动；路由、前进后退、刷新及查询参数通过浏览器自动化冒烟验证。真实账号登录因无测试凭据未执行。
tracked-only checkout：待最终完整验证

冲突解决说明：`commands.rs` 保留主分支对局时间线 revision 命令与测试，同时保留功能分支账号身份同步实现，继续删除旧 BattleTag 重绑定命令。
遗留问题：React Router 上游高危公告仍由限时策略例外管理；不得引入其 RSC/服务端 API。
回滚提交：不适用；如需回滚，应按上述独立提交使用 `git revert`。
下一次 React Router 例外复查日期：2026-09-02（到期后门禁自动失败）
```

## 11. 合并后的影响

完成本方案后：

- 用户继续只在登录时识别 BattleTag，不会看到额外绑定流程；
- 登录身份不完整时会更早失败，避免把错误账号或不完整身份发布到本地；
- 对局列表获得主分支最新的动态刷新能力；
- 新增 Router RSC/服务端能力会被自动门禁阻止；
- 后续 React Router 升级需要显式更新集中策略和测试；
- 主分支历史包含一个已完整验证的组合结果，后续开发不需要在两个分支差异上重复踩坑。
