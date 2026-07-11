# STAB-006 渐进式模块化审计与设计

- 日期：2026-07-11
- 状态：依据已批准总设计自主确认
- 前置：STAB-003、STAB-004、STAB-005 已验证
- 原则：保持 Tauri command、DTO、页面行为、固定 S21/S22/S23 赛季边界和中文兜底不变；纯重构提交不混入功能变化。

## 1. 审计结论

项目具备产品可用性，但多个文件同时承担边界、解析、SQL、计算和 UI 呈现职责，超过总设计建议的约 600 行评估阈值：

| 区域 | 现状 | 主要混合职责 | 首选切分 |
| --- | ---: | --- | --- |
| `src/styles.css` | 5667 行 | 全局变量、应用壳、页面、组件、响应式、遗留 phase 样式 | tokens/base、shell、features、shared components |
| `queries.rs` | 1311 行 | SQL 查询、DTO 映射、普通详情、角斗 JSON 解析、字典、隐私 fallback | match list、normal detail、arena detail、lookup/filter helpers |
| `review.rs` | 1086 行 | 赛季战报、角色段位、成就/结论、指挥台状态 | personal review、command-center status、shared aggregates |
| `analytics.rs` | 973 行 | 好友筛选 SQL、聚合、亲密度、榜单、阵容 | repository rows、friend metrics、friend boards |
| `sync.rs` | 948 行 | 凭据读写、HTTP、重试、节流、payload、落盘、导入 | legacy facade、client/retry、fetch orchestration、persist |
| `MatchesPage.tsx` | 728 行 | 路由 query、请求状态、筛选、列表、普通/角斗紧凑详情 | page controller、filter state、compact normal detail、compact arena detail |
| `OnboardingPage.tsx` | 554 行 | 当前账号、同步、资料、设置、健康状态、展示组件 | command-center data hook、sync actions、profile panel、season health |

已有可复用前端片段（`features/matches/FilterBar.tsx`、`MatchTimeline.tsx`、`shared/` 纯函数）和 Rust 集成测试。它们适合作为切分锚点，但现有前端测试多数是源码字符串断言，不能单独证明拆分后的真实行为。

## 2. 不做的事

- 不重写页面、不换框架、不引入全局状态管理或 ORM。
- 不改变数据库 schema、同步协议、身份模型、season 选择或 `match_id` 语义。
- 不删除所有旧字符串测试；只在被拆分区域旁新增行为/契约测试，逐步降低字符串断言依赖。
- 不把 STAB-007 的验证脚本、CI 或依赖升级夹带进本阶段。

## 3. 目标文件结构

### 3.1 Rust

采用“对外 façade 稳定、内部按职责拆分”。`lib.rs` 对外 re-export 和 `commands.rs` 调用保持稳定。

```text
src-tauri/src/
├─ queries/
│  ├─ mod.rs              # list/filter/detail façade
│  ├─ match_list.rs       # 列表 SQL 与筛选参数
│  ├─ normal_detail.rs    # 普通对局 DTO 映射
│  ├─ arena_detail.rs     # 角斗解析、中文隐私 fallback
│  └─ lookups.rs          # 字典、distinct options、通用字段解析
├─ review/
│  ├─ mod.rs              # get_personal_season_review / get_onboarding_status
│  ├─ personal.rs         # 战报聚合、结论、证据
│  ├─ roles.rs            # 段位与角色汇总
│  └─ command_center.rs   # 三赛季状态与资料健康
├─ analytics/
│  ├─ mod.rs
│  ├─ repository.rs       # 行级 SQL 和筛选
│  ├─ metrics.rs          # 聚合与亲密度
│  └─ boards.rs           # 榜单、阵容和展示 DTO
└─ sync/
   ├─ mod.rs              # 为兼容现有测试/CLI 的 façade
   ├─ client.rs           # headers、retry、节流、状态错误
   ├─ fetch.rs            # 分页和详情抓取编排
   ├─ payload.rs          # payload 合成与去重
   └─ persist.rs          # STAB-004 的安全原子落盘与导入
```

STAB-005 新增的数据库迁移、备份与 `SyncCoordinator` 保持独立，不被挪入业务模块。

### 3.2 前端

```text
src/
├─ app/
│  ├─ AppShell.tsx
│  └─ routes.tsx
├─ features/
│  ├─ matches/
│  │  ├─ MatchesPage.tsx          # 页面控制器，不含详情 JSX
│  │  ├─ matchFilters.ts          # URL/filter 纯函数
│  │  ├─ CompactMatchDetail.tsx
│  │  └─ CompactArenaDetail.tsx
│  └─ command-center/
│     ├─ OnboardingPage.tsx        # 页面组合
│     ├─ useCommandCenter.ts       # 查询、刷新、同步状态
│     ├─ ProfilePanel.tsx
│     └─ SeasonHealthPanel.tsx
└─ styles/
   ├─ tokens.css
   ├─ base.css
   ├─ shell.css
   ├─ shared.css
   └─ features/{matches,command-center,review,ranking,friends}.css
```

入口 `main.tsx` 仍只导入一个聚合 `styles.css`；该文件改为 import 各模块，避免一次性修改所有组件 className。`App.tsx` 的路由路径、导航和可访问性属性不变。

## 4. 切分顺序

1. 先创建目录和无行为变化的 façade/模块边界。
2. CSS 只移动原有规则并以 CSS bundle/关键选择器验证；不改视觉设计。
3. Rust 每次只迁移一个 cohesive 查询/计算域，并用现有 fixture 比较完整 DTO 序列化。
4. 前端先提取纯函数，再提取无副作用展示组件，最后将页面压缩为状态编排。
5. 每次迁移后运行该域测试和全量回归，提交保持可独立回退。

## 5. 风险控制

- 跨模块数据结构仍使用现有 `models.rs` / `shared/types.ts`，不在重构中改 DTO 字段。
- 角斗详情的中文 fallback、raw GUID 隐私保护和按 season 逻辑必须有契约测试；禁止以“移动文件”丢失保护。
- HTTP 限速、429 重试、脱敏和 STAB-005 同步锁由 `sync` façade 调用，不允许在新模块复制实现。
- 页面 query 参数、缓存、selected match 和同步按钮的禁用行为有前端行为测试后才迁移 JSX。
- 如果一项切分无法在不改变可观察行为的情况下独立验证，记录在进度表作为有依据的延后项，而不是强行重写。

## 6. 验收

- 目标大文件均已按上述职责拆分，或在 `docs/stabilization-progress.md` 说明延期理由和剩余风险。
- 旧 public Rust API、Tauri commands、TypeScript DTO、路由和用户可见中文文本保持稳定。
- `rg` 不显示被迁移的旧实现重复副本；模块依赖方向从页面/commands 指向 feature/domain，不反向依赖。
- 前端新增真实组件/纯函数测试，Rust 新增 DTO/查询契约测试；现有字符串测试可保留但不作为唯一证据。
- `cargo fmt --check`、`cargo test --tests`、`npm test`、`npm run build`、`git diff --check` 全部通过。
