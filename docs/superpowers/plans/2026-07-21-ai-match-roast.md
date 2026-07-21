# AI 对局锐评 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在详情完整的所有支持对局中提供一次性、永久保存、匿名且有事实依据的 DeepSeek AI 锐评，并在指挥台安全配置用户自己的 API Key。

**Architecture:** Rust 后端负责凭据、数据查询、标签补算、事实/关系选择、隐私最小化、DeepSeek 调用、结构校验和 SQLite 永久保存；React 只负责配置状态、生成状态和结果展示。普通与角斗复用同一锐评 DTO 和组件，角斗只在汇总视图出现；应用级协调器保证同时只有一个生成请求。

**Tech Stack:** Tauri 2、Rust、rusqlite、reqwest、Windows Credential Manager、React 18、TypeScript、SQLite、DeepSeek Chat Completions JSON Output。

---

## 开始前约束

- 当前工作树长期包含用户改动。执行前先运行 `git status --short`，不得清理、覆盖、暂存或提交无关文件。
- `commands.rs`、`database/migrations.rs`、`OnboardingPage.tsx` 和详情 UI 当前可能与用户改动重叠；执行时逐文件复核差异。
- 使用合成测试数据，不读取凭据文件，不把真实玩家数据、SQLite、账号 ID 或路径写入测试输出。
- 实施阶段先使用 `superpowers:using-git-worktrees` 建立隔离工作区；若用户当前未提交改动是实现前置条件，先停下协调，不从脏工作树复制隐私数据。

## 文件结构

**新增后端文件**

- `apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs`：领域入口、公共类型与模块导出。
- `apps/owstats-desktop/src-tauri/src/ai_roast/store.rs`：每场一条永久锐评的 SQLite 读写。
- `apps/owstats-desktop/src-tauri/src/ai_roast/material.rs`：普通对局事实、职责规则、关系候选、同职责均值和连胜连败。
- `apps/owstats-desktop/src-tauri/src/ai_roast/arena.rs`：角斗回合走势、让三追四和最终资金排名。
- `apps/owstats-desktop/src-tauri/src/ai_roast/client.rs`：DeepSeek 模型列表验证和 Chat Completions 调用。
- `apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs`：封号、正文、句数、事实编号和未知英雄/数字校验。
- `apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs`：应用级单请求互斥与原账号/原对局绑定。
- `apps/owstats-desktop/src-tauri/src/deepseek_key.rs`：Windows Credential Manager 封装；仅暴露状态、验证后保存和后端读取。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs`：迁移、永久性、级联删除和详情装配。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_material.rs`：标签路由、职责差异、关系证据、连续状态和隐私边界。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_arena.rs`：回合走势和 `endWorth` 完整性。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs`：合成 HTTP 服务、错误映射、超时和结构校验。
- `apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx`：紧凑生成入口、加载轮播、错误和永久结果。
- `apps/owstats-desktop/src/features/command-center/DeepSeekSettings.tsx`：指挥台 Key 配置区。
- `apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx`：静态/组件契约与位置验证。

**修改现有文件**

- `apps/owstats-desktop/src-tauri/Cargo.toml`：启用 Windows Credential Manager 所需 `windows-sys` feature。
- `apps/owstats-desktop/src-tauri/src/database/migrations.rs`：新增幂等迁移。
- `apps/owstats-desktop/src-tauri/src/models.rs`：锐评、Key 状态和生成结果 DTO。
- `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`：详情装配永久锐评。
- `apps/owstats-desktop/src-tauri/src/commands.rs`、`src/lib.rs`：Tauri 命令与 managed coordinator。
- `apps/owstats-desktop/src/shared/types.ts`、`src/shared/api.ts`：前端 DTO 与 invoke 封装。
- `apps/owstats-desktop/src/pages/OnboardingPage.tsx`：放置 Key 设置区。
- `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`、`src/features/matches/CompactMatchDetail.tsx`：在收藏上方放置锐评。
- `apps/owstats-desktop/src/styles/features/matches.css` 和指挥台对应样式：遵循 `design.md`。
- `apps/owstats-desktop/package.json`：把新增 UI 测试加入 `test:components`。

### Task 1: SQLite 永久锐评与详情 DTO

**Files:** `database/migrations.rs`, `models.rs`, `ai_roast/store.rs`, `queries/normal_detail.rs`, `tests/ai_roast_store.rs`

- [ ] **Step 1: 写迁移失败测试**

在 `ai_roast_store.rs` 构造旧版本库，断言升级后存在一对一、级联删除的表：

```rust
assert_eq!(version, 11);
assert_eq!(table_count(&conn, "match_ai_roasts")?, 1);
assert_eq!(foreign_key_cascade(&conn, "match_ai_roasts", "matches")?, 1);
```

- [ ] **Step 2: 运行失败测试**

Run: `cargo test --test ai_roast_store upgrades_v10_to_v11_idempotently -- --exact`
Expected: FAIL，当前 schema version 为 10 且表不存在。

- [ ] **Step 3: 增加 v11 幂等迁移**

```sql
CREATE TABLE IF NOT EXISTS match_ai_roasts (
    match_id TEXT PRIMARY KEY REFERENCES matches(match_id) ON DELETE CASCADE,
    award TEXT NOT NULL CHECK(length(award) BETWEEN 1 AND 32),
    commentary TEXT NOT NULL CHECK(length(commentary) BETWEEN 1 AND 720),
    used_fact_ids_json TEXT NOT NULL,
    model TEXT NOT NULL,
    evaluation_rule_version INTEGER NOT NULL,
    material_version INTEGER NOT NULL,
    prompt_version INTEGER NOT NULL,
    created_at TEXT NOT NULL
);
```

把 `CURRENT_SCHEMA_VERSION` 提升到 11，并保持重复 `initialize()` 安全。

- [ ] **Step 4: 添加永久读写类型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiMatchRoastView {
    pub award: String,
    pub commentary: String,
}
```

`store.rs` 提供 `roast_for_match(conn, match_id)` 和只插入不存在记录的 `save_roast_once`；冲突时返回已保存结果，绝不覆盖。

- [ ] **Step 5: 装配详情并验证生命周期**

给 `MatchDetailView` 增加 `ai_roast: Option<AiMatchRoastView>`。测试普通、角斗详情可读；模型/提示词版本变化不删除结果；删除 match 后级联删除；原始 JSON 不包含锐评正文。

- [ ] **Step 6: 运行测试并提交**

Run: `cargo test --test ai_roast_store`
Expected: PASS。

```powershell
git add apps/owstats-desktop/src-tauri/src/database/migrations.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/ai_roast apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs
git commit -m "feat: persist match AI roasts"
```

### Task 2: DeepSeek API Key 安全配置

**Files:** `Cargo.toml`, `deepseek_key.rs`, `models.rs`, `commands.rs`, `lib.rs`, `tests/ai_roast_client.rs`

- [ ] **Step 1: 写凭据边界测试**

定义可注入存储接口，测试状态 DTO 只有状态：

```rust
assert_eq!(status.state, "ready");
assert!(!serde_json::to_string(&status)?.contains("sk-synthetic"));
```

测试新 Key 验证失败时，旧 Key 保持不变。

- [ ] **Step 2: 启用 Windows API 并实现存储**

在 `windows-sys` 增加 `Win32_Security_Credentials`。Credential target 固定为 `OWStatsLocal/DeepSeekApiKey`；使用 `CredReadW`/`CredWriteW`/`CredFree`，不实现删除命令，不写日志。

- [ ] **Step 3: 暴露最小命令**

```rust
#[tauri::command]
pub fn get_deepseek_key_status(state: State<'_, DeepSeekKeyManager>)
    -> Result<DeepSeekKeyStatus, String>;

#[tauri::command]
pub async fn save_deepseek_key(
    state: State<'_, DeepSeekKeyManager>,
    api_key: String,
) -> Result<DeepSeekKeyStatus, String>;
```

保存流程为 trim/非空检查 → `GET /models` 验证 → 验证成功后覆盖；失败时旧值不变。返回仅 `ready|missing` 和中文 message。

- [ ] **Step 4: 使用合成 HTTP 服务验证认证**

复用项目 `TcpListener` 测试模式，断言 Authorization 为合成值但测试输出不打印 header；覆盖 200、401、402、429、500、503 和网络失败。

- [ ] **Step 5: 运行测试并提交**

Run: `cargo test --test ai_roast_client key_`
Expected: PASS。

```powershell
git add apps/owstats-desktop/src-tauri/Cargo.toml apps/owstats-desktop/src-tauri/src/deepseek_key.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs
git commit -m "feat: configure DeepSeek credentials securely"
```

### Task 3: 普通对局素材选择器

**Files:** `ai_roast/material.rs`, `ai_roast/mod.rs`, `tests/ai_roast_material.rs`

- [ ] **Step 1: 写合成场景测试**

至少覆盖带飞局找短板、躺赢局找大爹、尽力局找主锅、背锅局自嘲、顺风/惜败/平局中性回退。每个关系候选必须有两条支持事实且至少一条为排名/对位差。

```rust
assert_eq!(material.evaluation.primary_tag, "尽力局");
assert_eq!(material.relationship.as_ref().unwrap().kind, "scapegoat");
assert!(material.selected_facts.len() >= 3 && material.selected_facts.len() <= 4);
assert!(material.private_field_scan().is_empty());
```

- [ ] **Step 2: 实现本地输入类型和版本**

```rust
pub const MATERIAL_VERSION: i64 = 1;
pub struct RoastFact { pub id: String, pub text: String, pub strength: i32 }
pub struct RoastRelationship {
    pub kind: RelationshipKind,
    pub subject: String,
    pub facts: Vec<RoastFact>,
    pub counter_facts: Vec<RoastFact>,
    pub confidence: Confidence,
}
```

序列化 DTO 不包含 `match_id`、名字、BattleTag、`bnet_id`、raw JSON、备注、路径或账号字段。

- [ ] **Step 3: 实现职责规则和少量画像**

先使用坦克/输出/支援通用规则，再用 GUID 白名单修正偏输出、偏治疗和混合型支援。关系排序以队内/全场排名、同职责差、团队占比和组合指标为主，固定绝对值只做无效数据判断。

- [ ] **Step 4: 复用标签和同职责均值**

若 `match_evaluations` 缺失，只补算当前 match；仍不足则使用胜利/失败/平局中性路由。复用现有同职责平均的击杀、死亡、伤害、治疗，不计算当前英雄均值。

- [ ] **Step 5: 实现连续状态**

按同账号、同赛季、`begin_ts, match_id` 稳定排序，计算当前局之前的连续段以及本局延续/终结事件；平局中断。只输出长度和事件，不输出历史 ID。

- [ ] **Step 6: 实现动态事实去重**

候选按标签相关性、证据强度、反差和可理解性排序；同一指标的排名/占比/差值只保留最强一条，最终3～4条，不凑弱事实。

- [ ] **Step 7: 运行测试和隐私扫描并提交**

Run: `cargo test --test ai_roast_material`
Expected: PASS，所有 JSON 快照不含禁止字段。

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/material.rs apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs apps/owstats-desktop/src-tauri/tests/ai_roast_material.rs
git commit -m "feat: build grounded AI roast material"
```

### Task 4: 角斗整场素材

**Files:** `ai_roast/arena.rs`, `tests/ai_roast_arena.rs`

- [ ] **Step 1: 写回合序列失败测试**

覆盖让三追四、领先后被追平并输决胜、普通连胜回合、平局/缺失回合，以及只在所有同队玩家 `endWorth` 完整时生成排名。

```rust
assert!(facts.iter().any(|f| f.text.contains("让三追四")));
assert!(!incomplete_end_worth_facts.iter().any(|f| f.id.contains("funds-rank")));
```

- [ ] **Step 2: 实现可靠回合事实**

使用现有 `ArenaDetailSlice.result` 和累计比分推导回合胜负序列；不比较双方资金总和，不生成经济领先/落后因果。

- [ ] **Step 3: 实现最终资金排名**

只比较汇总/最终有效切片中同队全部玩家的 `end_worth`。任何一人缺失则不产生资金排名；不回退 `worth`。

- [ ] **Step 4: 运行测试并提交**

Run: `cargo test --test ai_roast_arena`
Expected: PASS。

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/arena.rs apps/owstats-desktop/src-tauri/tests/ai_roast_arena.rs
git commit -m "feat: add arena roast turning points"
```

### Task 5: DeepSeek 请求和输出校验

**Files:** `ai_roast/client.rs`, `ai_roast/validator.rs`, `tests/ai_roast_client.rs`

- [ ] **Step 1: 写请求契约测试**

断言固定 `deepseek-v4-flash`、`thinking.disabled`、`stream=false`、JSON Output、`max_tokens=500`，prompt 包含允许事实 JSON 和禁止新增事实规则；不含完整阵容和隐私字段。

- [ ] **Step 2: 实现固定请求**

```rust
const MODEL: &str = "deepseek-v4-flash";
const PROMPT_VERSION: i64 = 1;
const MAX_TOKENS: u32 = 500;
```

使用60秒 reqwest timeout；不记录 request/response body，不保存 usage。

- [ ] **Step 3: 写校验失败测试**

覆盖封号空/超过16汉字、正文非2～3句/超过180汉字、未知 fact ID、未知英雄名、输入未出现的数字、空 content、`finish_reason=length|content_filter|insufficient_system_resource`。

- [ ] **Step 4: 实现严格输出类型**

```rust
#[derive(Deserialize)]
struct ModelRoastOutput {
    award: String,
    commentary: String,
    used_fact_ids: Vec<String>,
}
```

校验通过后才转换为 `AiMatchRoastView`。空/非法 JSON在同一次用户操作自动重试一次，其他错误不自动重试。

- [ ] **Step 5: 映射中文产品错误并测试**

401 → Key 无效；402 → 余额不足；429 → 请求过多；500/503 → 服务繁忙；timeout → “锐评生成超时，请重试”。错误不包含 URL、header、body、ID 或路径。

- [ ] **Step 6: 运行测试并提交**

Run: `cargo test --test ai_roast_client`
Expected: PASS。

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/client.rs apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs
git commit -m "feat: generate and validate DeepSeek roasts"
```

### Task 6: 生成协调器与 Tauri 命令

**Files:** `ai_roast/coordinator.rs`, `commands.rs`, `lib.rs`, `tests/ai_roast_client.rs`

- [ ] **Step 1: 写并发和账号切换测试**

第一个请求挂起时第二个返回 `busy`；释放/错误/超时后可再次获取。切换账号或 match 后，第一个结果仍写入捕获的原数据库。

- [ ] **Step 2: 实现独立协调器**

```rust
#[derive(Default, Clone)]
pub struct AiRoastCoordinator(Arc<tokio::sync::Mutex<Option<ActiveRoast>>>);
```

不要复用或占用 `SyncCoordinator`。RAII guard 在成功、错误和 panic unwind 路径释放 active state。

- [ ] **Step 3: 实现命令**

```rust
#[tauri::command]
pub async fn generate_match_ai_roast(
    accounts: State<'_, AccountContextManager>,
    keys: State<'_, DeepSeekKeyManager>,
    coordinator: State<'_, AiRoastCoordinator>,
    match_id: String,
) -> Result<AiRoastGenerationResult, String>;
```

命令先捕获 account snapshot/database，再检查已有永久结果、Key、完整详情、active request，构造素材、调用、校验并 `save_roast_once`。

- [ ] **Step 4: 注册 managed state 和命令**

在 `lib.rs` manage `DeepSeekKeyManager`、`AiRoastCoordinator`，注册三个命令：status、save key、generate roast。

- [ ] **Step 5: 运行后端测试并提交**

Run: `cargo test --tests ai_roast`
Expected: PASS。

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs apps/owstats-desktop/src-tauri/src/commands.rs apps/owstats-desktop/src-tauri/src/lib.rs apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs
git commit -m "feat: orchestrate match AI roast generation"
```

### Task 7: 前端 DTO、API 与锐评组件

**Files:** `shared/types.ts`, `shared/api.ts`, `features/matches/AiMatchRoast.tsx`, `scripts/aiMatchRoastUi.test.tsx`

- [ ] **Step 1: 写 UI 契约失败测试**

覆盖紧凑初态、未配置提示、六条加载文案、busy、可重试/不可重试错误、成功时无“本场封号”和“AI 生成”标签、没有换一个按钮。

- [ ] **Step 2: 添加前端类型和 API**

```ts
export type AiMatchRoastView = { award: string; commentary: string };
export type DeepSeekKeyStatus = { state: "ready" | "missing"; message: string };
export const generateMatchAiRoast = (matchId: string) =>
  invoke<AiMatchRoastView>("generate_match_ai_roast", { matchId });
```

- [ ] **Step 3: 实现组件状态机**

`AiMatchRoast` 接收 `matchId`、`initialRoast`、`detailReady`。未生成仅渲染紧凑按钮；加载每2秒轮播固定六句，`aria-live` 保持“正在生成 AI 锐评”；成功展开大字号 `award` 和正文。

- [ ] **Step 4: 防止过期响应污染当前对局**

使用 request token/match ID guard；切换对局不取消后端请求，但旧响应不得写进新对局 React state。返回原对局时通过详情 DTO 读取永久结果。

- [ ] **Step 5: 运行组件测试并提交**

Run: `npx tsx scripts/aiMatchRoastUi.test.tsx`
Expected: PASS。

```powershell
git add apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/shared/api.ts apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx
git commit -m "feat: add AI roast detail component"
```

### Task 8: 指挥台 Key 配置

**Files:** `features/command-center/DeepSeekSettings.tsx`, `pages/OnboardingPage.tsx`, `styles/*`, `scripts/aiMatchRoastUi.test.tsx`

- [ ] **Step 1: 写指挥台失败测试**

断言存在独立“AI 锐评”区、常驻匿名数据说明、状态仅为“已配置并验证/未配置”、无删除按钮、无模型选择、无 Key 片段。

- [ ] **Step 2: 实现设置组件**

密码输入只保存在组件临时 state；保存调用后立即清空。验证期间禁用重复提交；失败保留后端旧 Key，不把输入复制到错误、日志或状态 DTO。

- [ ] **Step 3: 放入指挥台**

在 `OnboardingPage` 的应用工具区域增加单层低对比表面，遵循 `design.md` 的44px控件、12px圆角和现有间距，不引入新品牌色。

- [ ] **Step 4: 运行测试并提交**

Run: `npx tsx scripts/aiMatchRoastUi.test.tsx`
Expected: PASS。

```powershell
git add apps/owstats-desktop/src/features/command-center/DeepSeekSettings.tsx apps/owstats-desktop/src/pages/OnboardingPage.tsx apps/owstats-desktop/src/styles apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx
git commit -m "feat: configure DeepSeek from command center"
```

### Task 9: 普通/角斗详情集成与视觉验收

**Files:** `MatchDetailPage.tsx`, `CompactMatchDetail.tsx`, `styles/features/matches.css`, `package.json`, `scripts/aiMatchRoastUi.test.tsx`

- [ ] **Step 1: 写位置契约测试**

断言普通完整详情和紧凑详情中 `AiMatchRoast` 位于 `MatchFavoriteEditor` 前；角斗仅 `selected.key === "summary"` 渲染，两者顺序相同；列表卡片和筛选无 AI 标记。

- [ ] **Step 2: 集成组件**

普通与角斗汇总传入同一个整场 `detail.aiRoast`；单回合不渲染。详情缺失显示“对局详情完整后可生成锐评”。

- [ ] **Step 3: 添加视觉样式**

未生成是紧凑操作栏；成功后主标题字号大于正文但不新增独立标签。正文连续排版，不显示事实清单、技术元数据或分享/复制按钮。支持 `1440×900`、`1920×1080`、`2540×1344` 和 `prefers-reduced-motion`。

- [ ] **Step 4: 接入测试脚本**

把 `tsx scripts/aiMatchRoastUi.test.tsx` 加入 `test:components`，不删除现有脚本。

- [ ] **Step 5: 运行前端测试和构建并提交**

Run: `npm test`
Expected: PASS。

Run: `npm run build`
Expected: PASS，无绝对开发路径进入 dist。

```powershell
git add apps/owstats-desktop/src/pages/MatchDetailPage.tsx apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/package.json apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx
git commit -m "feat: integrate AI roasts into match details"
```

### Task 10: 全量验证与隐私/发布检查

**Files:** all files from Tasks 1–9, `docs/superpowers/specs/2026-07-20-ai-match-roast-design.md`

- [ ] **Step 1: 后端格式和测试**

Run: `cargo fmt --check`
Expected: PASS。

Run: `cargo test --tests`
Expected: PASS。

- [ ] **Step 2: 前端测试和构建**

Run: `npm test`
Expected: PASS。

Run: `npm run build`
Expected: PASS。

- [ ] **Step 3: 静态隐私扫描**

Run:

```powershell
rg -n "customerToken|authorization|cookie|credential|bnetId|playerName|rawJson|matchId" apps/owstats-desktop/src-tauri/src/ai_roast apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx
```

Expected: 只有明确的本地内部 match key 或禁止字段测试断言；DeepSeek serialized request 类型不包含真实身份、凭据或 raw JSON 字段。

- [ ] **Step 4: 发布构建**

Run: `npm run tauri build`
Expected: PASS；安装包不包含 Key、账号数据、SQLite、浏览器 profile、真实数据 dump 或开发机绝对路径。

- [ ] **Step 5: 视觉烟测**

使用合成 fixture 检查三种分辨率、普通详情、紧凑详情、角斗汇总、角斗单回合、未配置、加载轮播、错误、成功永久结果。不得使用真实玩家数据截图。

- [ ] **Step 6: 最终差异检查**

Run: `git diff --check`
Expected: PASS。

Run: `git status --short`
Expected: 仅包含本计划预期文件和执行前已记录的用户改动；报告未验证项及是否接触真实隐私数据。
