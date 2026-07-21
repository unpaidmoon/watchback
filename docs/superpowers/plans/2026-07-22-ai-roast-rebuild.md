# AI 锐评重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 AI 锐评从“三至四条本地结论的两句改写”重构为基于双方完整计分板、轻量英雄与地图知识、可核验证据和五区块短报告的赛后分析，并用本机已有 DeepSeek API Key 与本地真实对局完成多次端到端验证。

**Architecture:** Rust 新增独立的输入快照、知识筛选和报告 schema 模块；普通与角斗共用一个完整输入，角斗只追加最后有效回合、回合胜负和最终资金。DeepSeek V4 Flash 在思考模式下输出结构化五区块报告，Rust 通过扁平证据目录校验主体、数字、比较和推断边界，再以 SQLite v12 前向迁移保存新报告，同时保留旧标题正文兼容读取。

**Tech Stack:** Rust、Tauri 2、rusqlite、reqwest、serde/serde_json、React 18、TypeScript、SQLite、DeepSeek Chat Completion API。

---

## 执行前提与工作树边界

当前工作树已经包含一批未提交的 AI 锐评输出恢复、重新生成、查询和 UI 改动。执行前必须先运行 `git status --short`，逐项确认这些改动属于用户正在进行的工作；不得清理、覆盖或把无关文件带入本计划的提交。

本计划以当前工作树中的重新生成与有限修正能力为基线。若执行时使用独立 worktree，必须先通过 `superpowers:using-git-worktrees` 创建隔离环境，并确保这些在途改动已经由用户提交或以可审查补丁带入，不能从 `03ad530` 的干净分支直接假定它们存在。

## 文件结构

### 新建

- `apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs`：统一输入 schema、双方计分板、派生指标、职责对位、赛季基准、连续状态和角斗扩展。
- `apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs`：解析、校验并筛选本场涉及的英雄类型、单英雄覆盖和地图画像。
- `apps/owstats-desktop/src-tauri/src/ai_roast/report.rs`：五区块输出类型、claim type、证据目录和值语义。
- `apps/owstats-desktop/runtime-config/ai-roast-knowledge.json`：受版本控制并通过 `include_str!` 编入 Rust 二进制的轻量知识库。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`：普通/角斗输入、派生指标和隐私边界测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_knowledge.rs`：英雄覆盖、分类数量、地图匹配和本场知识筛选测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs`：五区块 schema、证据引用、比较断言、数字和攻击性内容测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs`：英雄特性、目标时间、对空职责、职责对位和角斗走势合成场景测试。
- `apps/owstats-desktop/src-tauri/src/bin/validate_ai_roast_live.rs`：显式运行的真实 API 与本地数据验收工具，只输出阶段、匿名计数、耗时和结果大小。

### 修改

- `apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs`：注册新模块。
- `apps/owstats-desktop/src-tauri/src/ai_roast/client.rs`：V2 请求、思考模式参数、五区块提示词、一次有限修正。
- `apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs`：迁移为报告校验入口；保留必要的通用内容安全函数。
- `apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs`：构造统一快照、知识上下文和证据目录，调用新客户端并保存新报告。
- `apps/owstats-desktop/src-tauri/src/ai_roast/store.rs`：新报告与旧记录兼容读写、原子重新生成。
- `apps/owstats-desktop/src-tauri/src/database/migrations.rs`：SQLite v12 幂等迁移。
- `apps/owstats-desktop/src-tauri/src/models.rs`：Rust 前端 DTO 增加可选结构化报告。
- `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`：详情查询继续返回兼容锐评 DTO。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs`：V2 请求和提示词契约。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_coordinator.rs`：统一普通/角斗生成与有限修正。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs`：v11 到 v12、旧记录、新报告和替换测试。
- `apps/owstats-desktop/src/shared/types.ts`：五区块 TypeScript 类型。
- `apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx`：结构化报告与旧记录双态展示。
- `apps/owstats-desktop/src/styles/features/matches.css`：连续表面、分隔线和响应式五区块样式。
- `apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx`：结构、可访问性和旧记录兼容测试。

### 删除

- `apps/owstats-desktop/src-tauri/src/ai_roast/material.rs`：新协调链路、测试和调用者全部迁移后删除旧三至四事实选择器。
- `apps/owstats-desktop/src-tauri/src/ai_roast/arena.rs`：角斗迁入统一快照后删除极简事实包。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_material.rs`：由快照和场景测试取代。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_arena.rs`：由统一角斗快照测试取代。

## Task 1: 定义 V2 输入、报告和证据类型

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs`
- Create: `apps/owstats-desktop/src-tauri/src/ai_roast/report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs`

- [ ] **Step 1: 写 V2 schema 序列化失败测试**

在两个新测试文件中固定顶层字段、camelCase JSON 和五区块输出。最小断言如下：

```rust
#[test]
fn v2_snapshot_serializes_complete_scoreboard_shape() {
    let value = serde_json::to_value(synthetic_snapshot()).unwrap();
    assert_eq!(value["schemaVersion"], 2);
    assert!(value["scoreboard"]["team"].is_array());
    assert!(value["scoreboard"]["opponent"].is_array());
    assert!(value.get("selectedFacts").is_none());
    assert!(value.get("anonymousRelation").is_none());
}

#[test]
fn report_requires_five_section_contract() {
    let report = synthetic_report();
    assert!(!report.verdict.title.is_empty());
    assert!((2..=3).contains(&report.decisive_factors.len()));
    assert!((1..=3).contains(&report.team_and_matchups.len()));
    assert!(!report.closing_roast.is_empty());
}
```

- [ ] **Step 2: 运行测试并确认因模块和类型不存在而失败**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test ai_roast_snapshot --test ai_roast_report
```

Expected: FAIL，错误包含无法找到 `ai_roast::snapshot` 或 `ai_roast::report`。

- [ ] **Step 3: 创建输入类型**

在 `snapshot.rs` 定义以下公开类型；所有可选上游字段使用 `Option`，不得用零值代替缺失：

```rust
pub const INPUT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastSnapshot {
    pub schema_version: i64,
    pub match_kind: RoastMatchKind,
    #[serde(rename = "match")]
    pub match_context: RoastMatchContext,
    pub current_player: String,
    pub scoreboard: RoastScoreboard,
    pub role_matchups: Vec<RoleMatchup>,
    pub current_player_baseline: Option<CurrentPlayerBaseline>,
    pub streak: Option<RoastStreak>,
    pub game_knowledge: KnowledgeContext,
    pub arena: Option<ArenaContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoastMatchKind { Standard, Arena }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoastPlayer {
    pub subject: String,
    pub is_current_player: bool,
    pub hero: String,
    pub role: Option<String>,
    pub stats: PlayerStats,
    pub derived: DerivedMetrics,
}
```

同时定义设计文档中的 `RoastMatchContext`、`RoastScoreboard`、`PlayerStats`、`DerivedMetrics`、`RoleMatchup`、`CurrentPlayerBaseline`、`RoastStreak`、`ArenaContext` 和 `ArenaRoundResult`，字段名与设计 JSON 完全一致。

- [ ] **Step 4: 创建报告与证据类型**

在 `report.rs` 定义：

```rust
pub const REPORT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiRoastReport {
    pub verdict: RoastVerdict,
    pub decisive_factors: Vec<RoastInsight>,
    pub current_player: RoastInsight,
    pub team_and_matchups: Vec<TeamInsight>,
    pub closing_roast: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    DirectFact,
    StatisticalInterpretation,
    GameContextInference,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvidenceValue {
    Integer(i64),
    Decimal(f64),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSemantic {
    RawStat,
    Rank,
    Share,
    Delta,
    RoundResult,
    FinalWorth,
    GameKnowledge,
}
```

`RoastInsight` 包含 `title`、`commentary`、`claim_type`、`evidence_refs`；`TeamInsight` 额外包含 `kind`，取值仅允许 `positive_player`、`negative_player`、`role_matchup`、`arena_trend`。

- [ ] **Step 5: 注册模块并让类型测试通过**

在 `mod.rs` 加入：

```rust
pub mod knowledge;
pub mod report;
pub mod snapshot;
```

先创建最小 `knowledge.rs`，仅定义可序列化的空 `KnowledgeContext`，完整加载器在 Task 4 实现。

Run:

```powershell
cargo test --test ai_roast_snapshot --test ai_roast_report
```

Expected: PASS。

- [ ] **Step 6: 提交 schema 基础**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs apps/owstats-desktop/src-tauri/src/ai_roast/report.rs apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs
git commit -m "feat: define AI roast report schema"
```

## Task 2: 构造普通对局完整快照

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`

- [ ] **Step 1: 写双方计分板与自然主体命名测试**

用合成 `MatchDetailView` 构造我方安娜、士兵：76、温斯顿和敌方法老之鹰、艾什、天使；断言：

```rust
let snapshot = build_roast_snapshot(&conn, "synthetic-match", &detail, knowledge()).unwrap();
assert_eq!(snapshot.scoreboard.team.len(), 3);
assert_eq!(snapshot.scoreboard.opponent.len(), 3);
assert_eq!(snapshot.current_player, "我方安娜");
assert!(snapshot.scoreboard.team.iter().any(|row| row.subject == "我方士兵：76"));
assert!(snapshot.scoreboard.opponent.iter().any(|row| row.subject == "敌方法老之鹰"));
assert!(!serde_json::to_string(&snapshot).unwrap().contains("SyntheticPlayer#"));
assert!(!serde_json::to_string(&snapshot).unwrap().contains("synthetic-account"));
```

再加入同队同英雄测试，期望名称稳定为“我方安娜一号”“我方安娜二号”。

- [ ] **Step 2: 运行测试确认 `build_roast_snapshot` 不存在**

Run: `cargo test --test ai_roast_snapshot ordinary_ -- --nocapture`

Expected: FAIL，无法找到 `build_roast_snapshot`。

- [ ] **Step 3: 实现玩家字段映射和缺失值边界**

新增：

```rust
pub fn build_roast_snapshot(
    conn: &Connection,
    match_id: &str,
    detail: &MatchDetailView,
    game_knowledge: KnowledgeContext,
) -> Result<RoastSnapshot>;
```

只从 `MatchDetailView.teammates` 和 `enemies` 复制设计允许的字段。显式排除 `name`、`bnet_id`、`hero_guid`、`rank_key`、`raw_json`、`perks` 和 `hero_stats`。`rank_label` 首版也不发送，避免不稳定文本进入归责。

- [ ] **Step 4: 写派生指标失败测试**

断言：

```rust
let soldier = player(&snapshot, "我方士兵：76");
assert_eq!(soldier.derived.team_damage_rank, Some(1));
assert_eq!(soldier.derived.match_damage_rank, Some(2));
assert_eq!(soldier.derived.team_damage_share, Some(0.40));
assert_eq!(soldier.derived.damage_per_death, Some(2_500.0));
assert_eq!(soldier.derived.kill_participation_rate, Some(0.75));
```

团队总伤害为零时，`team_damage_share` 必须为 `None`；阵亡为零时，`damage_per_death` 为 `None`，不能除零或用无穷值。

- [ ] **Step 5: 实现客观派生指标**

仅计算：

- 各原始指标的队内和全场竞赛排名。
- 击杀参与率：`(kills + assists) / max(team_kills, 1)`，并限制在 `0.0..=1.0`；上游数据使分子超过团队击杀时省略该指标而不是截断。
- 团队伤害、治疗、阵亡、承伤和受到治疗占比。
- 每次阵亡对应伤害、治疗和击杀参与；零阵亡时省略。

不要继续计算一个会直接决定主锅的综合分数。

- [ ] **Step 6: 写并实现职责对位测试**

按 `tank`、`damage`、`support` 汇总双方可靠角色，输出 raw totals 和 `team - opponent` deltas。无法映射职责的玩家仍留在计分板，但不进入 `role_matchups`。

Run: `cargo test --test ai_roast_snapshot ordinary_ derived_ role_matchup_`

Expected: PASS。

- [ ] **Step 7: 接入赛季基准与连续状态**

复用旧 `material.rs` 中同职责赛季均值和 streak 查询，但把结果改为纯客观 `CurrentPlayerBaseline` 与 `RoastStreak`。测试必须覆盖跨赛季不混读、平局中断连续状态和没有历史样本时返回 `None`。

Run: `cargo test --test ai_roast_snapshot`

Expected: PASS。

- [ ] **Step 8: 提交普通快照**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs
git commit -m "feat: build complete AI roast snapshots"
```

## Task 3: 将角斗改为普通快照的超集

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`

- [ ] **Step 1: 写角斗最终回合测试**

构造三个回合，第二回合字段残缺，第三回合结构完整；断言第三回合双方完整玩家进入基础 `scoreboard`，而不是进入另一套 `candidateFacts`：

```rust
assert_eq!(snapshot.match_kind, RoastMatchKind::Arena);
assert_eq!(snapshot.scoreboard.team[0].stats.kills, Some(9));
assert!(serde_json::to_value(&snapshot).unwrap().get("candidateFacts").is_none());
```

- [ ] **Step 2: 写回合胜负与最终资金测试**

断言仅发送：

```rust
assert_eq!(arena.round_results.len(), 7);
assert_eq!(arena.round_results[0].score_after, "0:1");
assert_eq!(arena.final_worth["我方安娜"], 7050);
```

序列化后不得出现 `modGuids`、`traitGuids`、`worthHistory`、`roundScoreboard` 或逐回合玩家数组。

- [ ] **Step 3: 运行测试确认角斗仍走旧极简实现**

Run: `cargo test --test ai_roast_snapshot arena_`

Expected: FAIL，`arena` 超集尚未构造。

- [ ] **Step 4: 实现最后有效回合选择**

使用 `ArenaMatchDetail.rounds` 中最后一个同时满足以下条件的回合：

- 我方与敌方玩家列表均非空。
- 双方比分存在且非负。
- 当前玩家能够在我方列表中唯一识别。
- 玩家统计结构可用于基础快照。

无法找到时返回既有“详情尚未完整同步”错误，不回退到不完整汇总。

- [ ] **Step 5: 实现角斗最小扩展**

`round_results` 只保存回合序号、本回合胜负和结束比分；`final_worth` 是按中文主体索引的映射，只收录最后有效回合中存在可靠 `end_worth` 的玩家。资金缺失时省略该主体，不得从 `worth` 回退；并在测试中固定此规则。

- [ ] **Step 6: 运行角斗与全部快照测试**

Run: `cargo test --test ai_roast_snapshot`

Expected: PASS。

- [ ] **Step 7: 提交统一角斗快照**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs
git commit -m "feat: add arena context to AI roast snapshots"
```

## Task 4: 落实精简英雄分类与特殊地图知识库

**Files:**
- Create: `apps/owstats-desktop/runtime-config/ai-roast-knowledge.json`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_knowledge.rs`

- [ ] **Step 1: 写知识库覆盖与分类数量测试**

测试从 `ow-lookups.json` 读取当前 52 个英雄名称，并断言每个名称在知识库中恰好出现一次；记录允许 `primaryArchetype: null`，表示已审查但不确定。断言每个职责最多三种主要分类、每个英雄最多一个辅助分类：

```rust
assert_eq!(knowledge.heroes.len(), lookup_hero_names().len());
assert!(knowledge.heroes.values().all(|hero| hero.secondary_archetypes.len() <= 1));
assert!(knowledge.archetypes_by_role("damage").len() <= 3);
```

- [ ] **Step 2: 写特殊英雄与地图测试**

至少固定：

```rust
assert!(knowledge.hero("禅雅塔").unwrap().overrides.contains_key("healing"));
assert!(knowledge.hero("天使").unwrap().overrides.contains_key("damage"));
assert!(knowledge.hero("生命之梭").unwrap().limitations.iter().any(|v| v.contains("生命之握")));
assert!(knowledge.map("阿特利斯").unwrap().traits.contains(&"交火距离偏短".into()));
assert!(knowledge.map("伊利奥斯").unwrap().traits.contains(&"拉扯快".into()));
```

- [ ] **Step 3: 运行测试确认知识文件不存在**

Run: `cargo test --test ai_roast_knowledge`

Expected: FAIL，无法读取或编译 `ai-roast-knowledge.json`。

- [ ] **Step 4: 建立版本 1 知识 schema**

文件顶层固定为：

```json
{
  "version": 1,
  "archetypes": {},
  "heroes": {},
  "maps": {}
}
```

英雄主要类型只允许：

- 坦克：`狗坦`、`地面坦`、`消耗坦`。
- 输出：`枪位`、`狗位`、`阵地输出`。
- 支援：`治疗型`、`进攻型`、`功能型`。

类型只写 `description` 和 `statExpectations`。单英雄只写 `primaryArchetype`、可选 `secondaryArchetypes`、`overrides`、`limitations` 和 `reviewSources`。

- [ ] **Step 5: 逐个审查并填入全部英雄**

以 `runtime-config/ow-lookups.json` 的 52 个名称为闭集。优先参考暴雪官方英雄页和当前补丁说明；社区资料只能帮助定位问题，不能单独成为硬规则。对埃姆雷、安燃、飞天猫、弗蕾娅、骇灾、金驭、瑞稀、死怨、探奇、无漾、西拉、斩仇等新英雄，只有在官方资料足够清楚时填写分类或覆盖，否则显式写 `primaryArchetype: null`、空覆盖和已查阅来源。不得猜测。

需要重点人工复核的既有英雄至少包括：D.Va、安娜、巴蒂斯特、布丽吉塔、禅雅塔、法老之鹰、黑百合、回声、卡西迪、狂鼠、拉玛刹、莱因哈特、猎空、卢西奥、路霸、美、末日铁拳、莫伊拉、破坏球、生命之梭、士兵：76、死神、索杰恩、天使、温斯顿、雾子、西格玛、伊拉锐、源氏、渣客女王、秩序之光、朱诺。

- [ ] **Step 6: 只录入确认度高的地图画像**

首批包含用户已确认的阿特利斯和伊利奥斯。伊利奥斯能识别到废墟、灯塔或深井时按子地图记录；只能识别“大地图伊利奥斯”时仅发送通用短图和拉扯快倾向，不发送具体视野或地形结论。其他地图没有明确且影响数据解释的特性时不写。

- [ ] **Step 7: 实现编译期加载与本场筛选**

```rust
const RAW_KNOWLEDGE: &str = include_str!("../../../runtime-config/ai-roast-knowledge.json");

pub fn embedded_knowledge() -> Result<RoastKnowledge>;

pub fn knowledge_for_match(
    knowledge: &RoastKnowledge,
    heroes: impl Iterator<Item = String>,
    map_name: Option<&str>,
) -> KnowledgeContext;
```

`KnowledgeContext` 只保留本场涉及的类型、英雄和一条匹配地图画像。不得把完整知识库发送给 DeepSeek。
`reviewSources` 只用于本地维护审查，不进入 `KnowledgeContext` 或模型请求。

- [ ] **Step 8: 运行知识、资源和干净编译测试**

Run:

```powershell
cargo test --test ai_roast_knowledge
cd ..
npm run test:resources
npm run build
```

Expected: 全部 PASS，Vite/Rust 不依赖开发机绝对路径。

- [ ] **Step 9: 提交知识库**

```powershell
git add apps/owstats-desktop/runtime-config/ai-roast-knowledge.json apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs apps/owstats-desktop/src-tauri/tests/ai_roast_knowledge.rs
git commit -m "feat: add AI roast game knowledge"
```

## Task 5: 构建扁平证据目录与报告校验器

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs`

- [ ] **Step 1: 写证据目录测试**

从合成快照建立以下稳定引用：

```text
我方士兵：76.stats.damage
我方士兵：76.derived.teamDamageRank
roleMatchups.damage.deltas.finalHits
arena.roundResults.4.result
arena.finalWorth.我方安娜
gameKnowledge.heroes.禅雅塔.overrides.healing
```

测试断言每个引用映射到 `EvidenceValue` 和 `EvidenceSemantic`，且目录不包含名字、ID、GUID、路径或 raw JSON。

- [ ] **Step 2: 实现 `EvidenceCatalog::from_snapshot`**

```rust
#[derive(Debug, Clone)]
pub struct EvidenceEntry {
    pub value: EvidenceValue,
    pub semantic: EvidenceSemantic,
}

pub type EvidenceCatalog = BTreeMap<String, EvidenceEntry>;

pub fn evidence_catalog(snapshot: &RoastSnapshot) -> EvidenceCatalog;
```

路径只由中文主体和固定 schema 字段生成，不接受模型自定义路径。

- [ ] **Step 3: 写结构与引用失败测试**

覆盖：缺区块、胜负手少于两条或多于三条、队友与对位超过三条、重复引用、不存在引用、错误主体、空标题、单区块和总长度超限。

Run: `cargo test --test ai_roast_report report_rejects_`

Expected: FAIL，V2 校验入口尚未实现。

- [ ] **Step 4: 实现 V2 报告校验入口**

```rust
pub fn validate_report_output(
    input: &str,
    snapshot: &RoastSnapshot,
) -> Result<AiRoastReport, RoastValidationError>;
```

长度上限固定为：标题 24 个汉字、总评 100 个汉字、每条 commentary 160 个汉字、closing 100 个汉字、全报告用户可见文本合计 600 个汉字。

- [ ] **Step 5: 写并实现数字校验**

正文出现的阿拉伯数字、中文数词、百分比和比分必须能在该条 `evidence_refs` 对应值中找到。允许百分比以 `0.47` 对应“47%”，允许整数与带千分位表示等价。不存在证据的数字返回 `UnsupportedNumber`。

- [ ] **Step 6: 写并实现比较语义校验**

比较词使用有限映射：

- “最高、最低、第一、垫底”必须引用 `Rank` 且值与断言一致。
- “领先、落后、高于、低于”必须引用 `Delta`、`Rank` 或双方直接 raw stat。
- “让三追四、连赢、决胜回合”必须引用 `RoundResult`。
- “最有钱、资金第一”必须引用 `FinalWorth` 排名证据。

无法可靠解析的复杂句式不尝试猜测，返回可修正校验错误，让模型换成可验证表达。

- [ ] **Step 7: 保留安全边界并删除旧表达压制**

保留身份 ID、攻击人格、仇恨和凭据关键词检查。删除旧的“所有数字禁止、恰好两句、至少两个 fact ID、必须关系事实、万能比喻黑名单”。`direct_fact` 不得只引用 `GameKnowledge`；`game_context_inference` 必须同时包含至少一条统计证据和一条知识证据。

- [ ] **Step 8: 运行报告测试**

Run: `cargo test --test ai_roast_report`

Expected: PASS。

- [ ] **Step 9: 提交证据校验器**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/report.rs apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs
git commit -m "feat: validate grounded AI roast reports"
```

## Task 6: 重写 DeepSeek 请求和冷面损友提示词

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/client.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs`

- [ ] **Step 1: 写请求参数契约测试**

解析合成服务捕获的请求并断言：

```rust
assert_eq!(request["model"], "deepseek-v4-flash");
assert_eq!(request["thinking"]["type"], "enabled");
assert_eq!(request["reasoning_effort"], "high");
assert_eq!(request["temperature"], 0.8);
assert_eq!(request["max_tokens"], 4000);
assert_eq!(request["response_format"]["type"], "json_object");
assert_eq!(request["stream"], false);
```

- [ ] **Step 2: 写完整输入和旧字段缺失测试**

断言 user message 中有 `scoreboard.team`、`scoreboard.opponent`、`roleMatchups`、`gameKnowledge`，且没有 `selectedFacts`、`anonymousRelation`、`usedFactIds`。

- [ ] **Step 3: 写提示词风格契约测试**

系统提示词必须包含“事实优先”“冷面幽默”“不写客服或公关稿”“不推测故意送、摆烂或主观意图”“五区块”“每个分析条目附 evidenceRefs”；不得出现“你是 Grok”或要求模仿模型身份。

- [ ] **Step 4: 运行客户端测试确认旧请求失败**

Run: `cargo test --test ai_roast_client request_v2_ prompt_v2_`

Expected: FAIL，仍发送 V1 fact payload 和非思考请求。

- [ ] **Step 5: 替换请求 DTO 与参数**

`DeepSeekRoastClient::generate` 改为：

```rust
pub async fn generate(
    &self,
    api_key: &str,
    snapshot: &RoastSnapshot,
) -> Result<AiRoastReport, DeepSeekRoastError>;
```

请求只包含 system message 和完整 V2 snapshot user message。保留官方 `response_format: json_object`。

- [ ] **Step 6: 写入最终系统提示词**

将设计文档第 7、8、9 节转换为提示词，明确分析顺序、职责解释、目标时间、推断边界、五区块 JSON schema和冷面损友语气。加入三组正反例：高伤害低转化、目标时间机会成本、枪位面对空中威胁；正例不得成为运行时输出模板。

- [ ] **Step 7: 实现一次有限修正**

首次校验失败后只追加匿名错误位置和错误类型，例如：

```text
上一次报告的 teamAndMatchups[1] 引用了不存在的证据字段。
请只修正无效引用或对应结论，保持其他区块语义不变，并重新返回完整 JSON。
```

不得回传第一次正文、对局 payload、ID、路径或凭据。主请求加一次修正后仍失败即返回 `InvalidOutput`；不继续五次自动请求。

- [ ] **Step 8: 运行全部客户端测试**

Run: `cargo test --test ai_roast_client`

Expected: PASS，包括思考模式、JSON、一次修正、超时、429、余额和内容过滤。

- [ ] **Step 9: 提交客户端重构**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/client.rs apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs
git commit -m "feat: generate five-part AI roast reports"
```

## Task 7: SQLite v12 与旧锐评兼容

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/database/migrations.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/store.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs`

- [ ] **Step 1: 写 v11 到 v12 升级库失败测试**

创建 v11 数据库和一条旧锐评，运行两次初始化后断言：

```rust
assert_eq!(user_version(&conn), 12);
assert!(columns(&conn, "match_ai_roasts").contains("report_json"));
assert!(columns(&conn, "match_ai_roasts").contains("report_schema_version"));
assert!(columns(&conn, "match_ai_roasts").contains("input_schema_version"));
assert!(columns(&conn, "match_ai_roasts").contains("knowledge_version"));
assert_eq!(roast_for_match(&conn, "legacy")?.unwrap().report, None);
```

- [ ] **Step 2: 运行存储测试确认 schema 仍为 v11**

Run: `cargo test --test ai_roast_store migrates_v11_to_v12_`

Expected: FAIL，`CURRENT_SCHEMA_VERSION` 仍为 11。

- [ ] **Step 3: 实现幂等 v12 迁移**

将 schema 版本提升到 12，使用 `add_column_if_missing` 增加可空列：

```sql
report_json TEXT
report_schema_version INTEGER
input_schema_version INTEGER
knowledge_version INTEGER
```

不得更新、删除或回填旧记录。旧 `award`、`commentary`、`used_fact_ids_json` 和版本字段保留。

- [ ] **Step 4: 扩展 Rust DTO**

```rust
pub struct AiMatchRoastView {
    pub award: String,
    pub commentary: String,
    pub report: Option<AiRoastReport>,
}
```

新报告的 `award` 和 `commentary` 分别冗余保存 `report.verdict.title` 与 `report.verdict.summary`，供旧前端和降级读取；`report` 为新 UI 的完整数据。

- [ ] **Step 5: 实现新旧读写和原子替换**

`AiRoastRecord` 增加 report 与三个版本字段。新生成保存 `report_json`；`used_fact_ids_json` 改存报告中去重排序后的全部 `evidence_refs`，保持旧列约束有效。重新生成使用单条 UPSERT 原子替换所有新旧列；失败时不修改原记录。

旧非空列继续按兼容规则填写：`evaluation_rule_version = 0`，`material_version = INPUT_SCHEMA_VERSION`，`prompt_version = 2`。这些值只用于旧 schema 兼容；新读取逻辑以显式的报告、输入和知识版本列为准。

- [ ] **Step 6: 运行存储测试**

Run: `cargo test --test ai_roast_store`

Expected: PASS，覆盖旧记录展示、新报告 round-trip、重新生成、级联删除、恶意 JSON 拒绝和重复迁移。

- [ ] **Step 7: 提交迁移与存储**

```powershell
git add apps/owstats-desktop/src-tauri/src/database/migrations.rs apps/owstats-desktop/src-tauri/src/ai_roast/store.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs
git commit -m "feat: store structured AI roast reports"
```

## Task 8: 协调器切换到统一快照并移除旧选择器

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_coordinator.rs`
- Delete: `apps/owstats-desktop/src-tauri/src/ai_roast/material.rs`
- Delete: `apps/owstats-desktop/src-tauri/src/ai_roast/arena.rs`
- Delete: `apps/owstats-desktop/src-tauri/tests/ai_roast_material.rs`
- Delete: `apps/owstats-desktop/src-tauri/tests/ai_roast_arena.rs`

- [ ] **Step 1: 写普通和角斗统一协调测试**

合成服务捕获请求并断言普通与角斗都包含完整 `scoreboard`，角斗额外含 `arena.roundResults`。测试现有缓存直接返回、`regenerate=true` 覆盖、生成失败保留旧结果。

- [ ] **Step 2: 写知识和证据进入协调链路测试**

断言协调器只选择本场英雄和地图知识，客户端返回的引用由同一 snapshot 目录校验，保存记录包含 `REPORT_SCHEMA_VERSION`、`INPUT_SCHEMA_VERSION` 和知识版本。

- [ ] **Step 3: 运行测试确认协调器仍构造旧素材**

Run: `cargo test --test ai_roast_coordinator`

Expected: FAIL，捕获请求仍为 `selectedFacts` 或 `candidateFacts`。

- [ ] **Step 4: 替换 `PreparedMaterial`**

协调器顺序固定为：

```text
读取并验证详情
→ 读取内置知识库
→ 按本场英雄和地图筛选知识
→ build_roast_snapshot
→ DeepSeekRoastClient::generate
→ validate_report_output
→ 保存新报告
```

继续保留应用级生成互斥、请求时账号与数据库捕获、API Key 后端读取、现有用户可见错误映射和不记录 payload 的边界。

- [ ] **Step 5: 删除旧关系与事实选择器**

确认 `rg "RoastMaterial|ArenaRoastMaterial|selected_facts|candidate_facts|anonymous_relation" apps/owstats-desktop/src-tauri/src apps/owstats-desktop/src-tauri/tests` 只剩设计无关文本后，删除旧模块与旧测试，并从 `mod.rs` 移除注册。

- [ ] **Step 6: 运行 AI 锐评全部后端测试**

Run:

```powershell
cargo test --test ai_roast_snapshot --test ai_roast_knowledge --test ai_roast_report --test ai_roast_client --test ai_roast_coordinator --test ai_roast_store
```

Expected: PASS。

- [ ] **Step 7: 提交协调链路**

```powershell
git add -A apps/owstats-desktop/src-tauri/src/ai_roast apps/owstats-desktop/src-tauri/tests/ai_roast_*.rs
git commit -m "refactor: use complete match context for AI roasts"
```

提交前用 `git diff --cached --name-only` 确认没有暂存本计划以外的用户改动。

## Task 9: 前端展示五区块并兼容旧记录

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx`
- Modify: `apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx`

- [ ] **Step 1: 写 TypeScript 类型与结构渲染失败测试**

定义合成报告并断言静态 HTML 包含：本场判词、胜负手、你这局、队友与对位、终审锐评；不得显示 `evidenceRefs` 或 `claimType`。

```tsx
const report: AiRoastReport = {
  verdict: { title: "输出刷卡前排停机", summary: "合成总评。" },
  decisiveFactors: [factor1, factor2],
  currentPlayer: current,
  teamAndMatchups: [matchup],
  closingRoast: "合成收尾。",
};
```

- [ ] **Step 2: 运行 UI 合约确认仍只渲染标题正文**

Run: `npx tsx scripts/aiMatchRoastUi.test.tsx`

Expected: FAIL，缺少五区块标题。

- [ ] **Step 3: 扩展前端类型**

```ts
export type AiRoastClaimType =
  | "direct_fact"
  | "statistical_interpretation"
  | "game_context_inference";

export type AiRoastInsight = {
  title: string;
  commentary: string;
  claimType: AiRoastClaimType;
  evidenceRefs: string[];
};

export type AiRoastReport = {
  verdict: { title: string; summary: string };
  decisiveFactors: AiRoastInsight[];
  currentPlayer: AiRoastInsight;
  teamAndMatchups: Array<AiRoastInsight & { type: string }>;
  closingRoast: string;
};

export type AiMatchRoastView = {
  award: string;
  commentary: string;
  report: AiRoastReport | null;
};
```

- [ ] **Step 4: 实现双态渲染**

`report === null` 时保持旧标题正文和重新生成按钮。新报告使用语义化 `<section>`、`<h3>`、`<h4>` 和列表；胜负手、队友与对位按数组顺序渲染，不显示内部证据路径。

- [ ] **Step 5: 保留重新生成错误行为**

重新生成中继续显示旧报告；失败时旧报告不消失，行内显示错误；成功后原子切换新报告。屏幕阅读器状态继续使用稳定的“正在生成 AI 锐评”。

- [ ] **Step 6: 运行 UI 合约测试**

Run: `npx tsx scripts/aiMatchRoastUi.test.tsx`

Expected: PASS。

- [ ] **Step 7: 提交前端结构**

```powershell
git add apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx
git commit -m "feat: render structured AI roast reports"
```

## Task 10: 完成符合 design.md 的报告样式

**Files:**
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx`

- [ ] **Step 1: 写视觉合约失败测试**

断言：

- `.aiMatchRoastReport` 是单一连续容器。
- `.aiMatchRoastSection + .aiMatchRoastSection` 使用 `1px` 分隔线。
- 条目没有独立白底、阴影或大圆角卡片。
- 标题、正文和辅助文字只使用既有黑灰 token。
- 按钮仍为至少 44px。

- [ ] **Step 2: 运行合约测试确认新类尚无样式**

Run: `npx tsx scripts/aiMatchRoastUi.test.tsx`

Expected: FAIL，缺少 `.aiMatchRoastReport` 等规则。

- [ ] **Step 3: 实现连续表面样式**

使用现有 `--space-*`、`--text-*`、`--border-subtle`、`--surface-secondary` 和 `--radius-control`。区块间距采用 16/24px，正文不小于 14px；只让本场判词主标题明显加粗，其他标题使用 18px 常规字重。

- [ ] **Step 4: 加入响应式与 reduced motion**

宽屏保持横向留白；窄屏区块自然单列。报告本身不创建内层滚动，不增加动画；现有 loading 过渡继续遵守 `prefers-reduced-motion`。

- [ ] **Step 5: 运行 UI 测试和构建**

Run:

```powershell
npx tsx scripts/aiMatchRoastUi.test.tsx
npm run build
```

Expected: PASS。

- [ ] **Step 6: 使用纯合成 fixture 做三档视口验收**

在 `2540x1344`、`1920x1080`、`1440x900` 检查：无水平滚动、无嵌套滚动、长中文可换行、重新生成按钮可见、旧记录布局稳定。截图不得使用真实本地对局。

- [ ] **Step 7: 提交样式**

```powershell
git add apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx
git commit -m "style: present AI roast reports as short sections"
```

## Task 11: 建立关键游戏理解合成场景

**Files:**
- Create: `apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs`

- [ ] **Step 1: 为六类关键场景建立纯合成快照**

固定以下 fixture：

1. 我方天使面对敌方卡西迪与艾什，生存环境推断必须为 `game_context_inference`。
2. 我方士兵：76是主要枪位，敌方法老之鹰高影响；允许写“对空任务没有形成结果”，禁止写“整局没打过法老之鹰”。
3. 禅雅塔高伤害低治疗，不得仅因治疗排名判定失职。
4. 天使和生命之梭低伤害不进入负面证据。
5. 当前玩家目标时间全队最高、战斗数据略低，评价必须引用目标时间；战斗数据全面垫底时目标时间不能单独免责。
6. 阿特利斯或伊利奥斯短图中狗位的高参与得到地图上下文支持，但地图本身不能直接判定表现好坏。

- [ ] **Step 2: 写允许与禁止措辞测试**

对每个 fixture 构造合法报告与非法报告，调用 `validate_report_output`。非法报告必须因 claim type、证据缺失或无依据绝对断言失败，而不是因普通幽默词汇失败。

- [ ] **Step 3: 加入角斗场景**

覆盖让三追四、决胜回合、最终资金第一但表现普通。允许“最终资金与表现形成反差”，禁止“因为最有钱所以完成翻盘”。

- [ ] **Step 4: 运行场景与全部 AI 测试**

Run:

```powershell
cargo test --test ai_roast_scenarios
cargo test --tests ai_roast
```

Expected: PASS。

- [ ] **Step 5: 提交场景回归**

```powershell
git add apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs
git commit -m "test: cover AI roast game understanding"
```

## Task 12: 使用本机 API Key 和本地真实数据做多次端到端验证

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/bin/validate_ai_roast_live.rs`
- Modify: `apps/owstats-desktop/src-tauri/Cargo.toml` only if an explicit bin entry is required by the existing manifest layout

- [ ] **Step 1: 写真实验收工具的只读源库边界**

工具接收唯一参数 `app_data_dir`，通过 `SharedAppPaths` 和 `AccountContextManager::active_snapshot()` 定位活动账号数据库。使用 rusqlite backup API 将源库复制到 `tempfile::TempDir`，之后所有迁移、查询、生成和锐评写入都只针对临时副本。

禁止直接复制 WAL 文件，禁止修改源数据库，禁止输出输入参数或解析后的路径。

- [ ] **Step 2: 通过现有凭据管理器读取 Key**

只调用：

```rust
let keys = DeepSeekKeyManager::new()?;
let status = keys.status()?;
ensure!(status.state == "ready", "live validation requires a configured key");
```

生成过程继续由协调器内部调用 `backend_api_key()`。工具自身不得读取、打印、计算摘要、复制或序列化 API Key。

- [ ] **Step 3: 在临时副本中选择四次调用**

不打印 match ID，仅在进程内选择：

1. 一场详情完整的普通对局。
2. 另一场英雄阵容不同的普通对局。
3. 一场详情完整的角斗对局。
4. 对第一场普通对局执行一次 `regenerate=true`。

候选不足时只输出类似 `eligible_standard=1`、`eligible_arena=0` 的匿名计数后失败，不输出对局 ID、赛季、时间、路径或玩家内容。

- [ ] **Step 4: 每次调用执行结构和内容验收**

每次必须满足：

- 返回 `report: Some`。
- 胜负手 2 至 3 条。
- 队友与对位 1 至 3 条。
- 所有 evidence refs 再次通过本地校验。
- 用户可见文本 350 至 600 个汉字；真实数据不足导致合法短报告时允许低于 350，但不得缺本场判词、你这局或终审锐评。
- 不包含输入中排除的身份、GUID、路径或凭据标记。

第四次调用不要求一定改变责任判断；记录标题或收尾是否变化作为非阻断观察值。

- [ ] **Step 5: 限制诊断输出**

唯一允许的 stdout 形态：

```text
eligible_standard=2
eligible_arena=1
case=1 stage=generate status=pass elapsed_ms=1234 report_chars=482
case=2 stage=generate status=pass elapsed_ms=1234 report_chars=451
case=3 stage=generate status=pass elapsed_ms=1234 report_chars=509
case=4 stage=regenerate status=pass elapsed_ms=1234 report_chars=476 variation=true
summary passed=4 failed=0
```

错误只输出固定阶段和分类，不输出 HTTP body、模型正文、请求 payload、ID、玩家、路径或行号。

- [ ] **Step 6: 先运行无网络编译和帮助检查**

Run:

```powershell
cargo test --bin validate_ai_roast_live
cargo run --bin validate_ai_roast_live -- --help
```

Expected: PASS；帮助只说明参数，不展开本机路径。

- [ ] **Step 7: 显式运行四次真实 DeepSeek 验证**

PowerShell 中从 Windows roaming application data 与 Tauri identifier 推导 `app_data_dir`，变量值不得打印、写入提交、日志文件或最终报告：

```powershell
$owstatsAppData = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'local.owstats.desktop'
cargo run --bin validate_ai_roast_live -- $owstatsAppData
```

Expected: `summary passed=4 failed=0`。该命令会使用 Windows Credential Manager 中已经配置的 Key，并读取本地真实数据的临时数据库副本；不会读取或输出 Key 内容，不修改源数据库。

- [ ] **Step 8: 在进程内执行真实结果质量门槛**

验收工具在内存中对四份报告执行额外质量检查，只输出通过计数：每个分析条目都有证据、五区块没有重复正文、没有设计中列出的公关套话、标题与终审锐评不是纯统计复述、重新生成结果均通过相同事实校验。检查代码不得打印、保存或返回真实正文；结束后让 `TempDir` 自动清理临时数据库，不另存请求或响应。

- [ ] **Step 9: 提交验收工具**

```powershell
git add apps/owstats-desktop/src-tauri/src/bin/validate_ai_roast_live.rs apps/owstats-desktop/src-tauri/Cargo.toml
git commit -m "test: add private AI roast live validation"
```

若 `Cargo.toml` 不需要改动，不要暂存它。

## Task 13: 全量验证、干净 checkout 与安装版烟测

**Files:**
- Verify only unless failures require scoped fixes

- [ ] **Step 1: Rust 格式和全量测试**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: PASS。

- [ ] **Step 2: 前端全量测试和构建**

Run:

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: PASS。

- [ ] **Step 3: 路径、资源和隐私静态扫描**

Run:

```powershell
rg -n "D:\\Codex|/@fs/|customerToken|authorization|cookie|browser profile" apps/owstats-desktop/src-tauri/src/ai_roast apps/owstats-desktop/runtime-config/ai-roast-knowledge.json apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx
```

Expected: 不出现开发机绝对路径、资源逃逸或新凭据字段；代码中的禁止词测试常量需要人工区分，不得误判为运行时泄漏。

- [ ] **Step 4: 干净 checkout 构建**

在经验证位于独立临时目录的 clean worktree 中检出当前分支，运行：

```powershell
cd apps\owstats-desktop
npm ci
npm test
npm run build
```

Expected: PASS，知识 JSON 通过跟踪文件和 `include_str!` 可用，不依赖当前开发目录的未跟踪文件。

- [ ] **Step 5: Tauri 发布构建**

Run:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

Expected: PASS。

- [ ] **Step 6: 安装版烟测**

安装构建产物后，用合成数据检查五区块 UI、旧记录兼容、未配置 Key、生成中和错误状态；真实 API 烟测沿用 Task 12 的临时副本工具，不把真实内容用于截图。

- [ ] **Step 7: 最终 diff 与工作树检查**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` 无输出；`git status --short` 只包含本计划预期文件或用户原有未提交改动，没有本地 SQLite、真实数据 dump、凭据、浏览器 profile、临时副本或生成正文。

- [ ] **Step 8: 最终报告**

交付说明必须列出实际执行命令、结果、未验证项、四次真实调用的匿名通过计数与耗时范围，并明确：使用过本机已配置 Key 和本地真实数据的临时副本，但没有读取、打印、提交或在回复中输出 Key、payload、玩家标识、路径或真实锐评正文。
