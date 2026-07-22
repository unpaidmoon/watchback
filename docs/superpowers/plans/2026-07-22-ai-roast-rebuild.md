# AI 锐评叙事重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将在途的五区块 AI 锐评改造成“一个有趣主标题 + 一篇连续损友复盘”，让 DeepSeek 基于完整计分板和关键任务归属选择唯一主线，同时保持本地证据校验、历史兼容和重新生成安全性。

**Architecture:** 复用现有 V2 完整快照、轻量知识库、证据目录、有限修正与 SQLite 报告列；新增关键任务上下文，将报告 schema 升级为包含后台叙事计划、主标题和正文的 V3。DeepSeek 在一次思考模式请求中完成综合判断与写作，Rust 校验叙事计划及正文，存储层用 untagged payload 兼容 V2 五区块，前端对 V3 只展示标题与正文。

**Tech Stack:** Rust、Tauri 2、rusqlite、reqwest、serde/serde_json、React 18、TypeScript、SQLite、DeepSeek Chat Completion API。

---

## 执行前提与工作树边界

当前工作树已经包含未提交的 V2 AI 锐评重构。执行前必须运行 `git status --short`，把这些文件视为本计划基线；不得清理、回退或覆盖。每个任务只暂存列出的 AI 锐评文件，绝不暂存 `AGENTS.md`、本地 SQLite、真实数据、凭据或无关改动。

本计划不创建干净 worktree，因为干净分支不包含当前 V2 在途实现。若执行者必须隔离工作，应先把当前在途变更带入隔离环境并逐文件核对，不能从 `HEAD` 重新实现旧计划。

所有测试使用合成数据。真实 API 验收只能通过既有 `validate_ai_roast_live` 工具在临时数据库副本上显式运行，并且只输出匿名计数、耗时和字符数。

## 文件结构

### 新建

- `apps/owstats-desktop/src-tauri/src/ai_roast/task.rs`：从静态知识和本场双方英雄生成关键任务、威胁、主要承担者与辅助承担者上下文。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_task.rs`：任务筛选、第一责任人优先和隐私边界测试。

### 修改

- `apps/owstats-desktop/runtime-config/ai-roast-knowledge.json`：版本升至 2，增加通用任务解释和高价值对局任务映射。
- `apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs`：注册 `task` 模块。
- `apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs`：解析和校验任务知识。
- `apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs`：输入版本升至 3，加入 `taskKnowledge`。
- `apps/owstats-desktop/src-tauri/src/ai_roast/report.rs`：定义 V3 叙事计划和 V2/V3 兼容 payload，扩展任务知识证据目录。
- `apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs`：从五区块校验迁移为叙事计划、标题和正文校验。
- `apps/owstats-desktop/src-tauri/src/ai_roast/client.rs`：提示词版本升至 3，改为唯一主线与口语化损友写作，更新有限修正。
- `apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs`：为快照附加任务上下文，保存 V3 payload。
- `apps/owstats-desktop/src-tauri/src/ai_roast/store.rs`：保存 V3、读取 V2/V3、汇总后台证据引用。
- `apps/owstats-desktop/src-tauri/src/models.rs`：保留旧五区块 DTO，仅将其用于历史记录。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs`：V3 schema 和兼容 payload 测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`：V3 输入字段与任务上下文测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_knowledge.rs`：知识版本和任务配置完整性测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs`：新提示词、请求结构和有限修正测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs`：关键任务、胜负立场和叙事主线场景。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs`：V2 五区块与 V3 叙事 payload 兼容测试。
- `apps/owstats-desktop/src-tauri/tests/ai_roast_coordinator.rs`：新生成与重新生成持久化测试。
- `apps/owstats-desktop/src-tauri/src/bin/validate_ai_roast_live.rs`：真实验收条件从五区块改为 V3 计划、标题和正文。
- `apps/owstats-desktop/src/shared/types.ts`：把五区块类型标记为历史 DTO；用户成功态继续只消费 `award` 与 `commentary`。
- `apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx`：V3 使用标题加正文，V2 继续兼容旧展示。
- `apps/owstats-desktop/src/features/command-center/DeepSeekSettings.tsx`：将模型数据披露改为与匿名完整计分板一致的真实说明。
- `apps/owstats-desktop/src/styles/features/matches.css`：增加连续叙事样式，旧五区块样式只服务历史记录。
- `apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx`：新成功态、旧记录兼容和视觉结构断言。

### 不修改数据库 schema

在途 SQLite 迁移已经提供 `report_json`、`report_schema_version`、`input_schema_version` 和 `knowledge_version`。V3 后台计划、标题与正文全部可存入现有列；本计划不得为了改 JSON 形状再增加一组重复列或重写历史记录。

## Task 1: 定义 V3 叙事报告与 V2 兼容 payload

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/report.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs`

- [ ] **Step 1: 写 V3 schema 和 V2 兼容失败测试**

在 `ai_roast_report.rs` 增加：

```rust
#[test]
fn narrative_report_has_hidden_plan_title_and_commentary() {
    let report = AiRoastNarrative {
        analysis: NarrativeAnalysis {
            match_thesis: claim("我方没有处理关键空中威胁"),
            current_player: claim("当前玩家有贡献但转化不足"),
            key_actors: vec![KeyActorClaim {
                subject: "我方士兵：76".into(),
                impact: ActorImpact::Negative,
                claim: claim("主要对空任务没有形成结果"),
            }],
            supporting_factors: Vec::new(),
        },
        title: "法老之鹰在天上续了年卡".into(),
        commentary: "我方并非没有输出，但最该处理的威胁一直活得很舒服。".into(),
    };
    let value = serde_json::to_value(&report).unwrap();
    assert!(value["analysis"]["matchThesis"].is_object());
    assert_eq!(value["title"], "法老之鹰在天上续了年卡");
    assert!(value.get("decisiveFactors").is_none());
}

#[test]
fn stored_payload_reads_legacy_five_section_json() {
    let json = serde_json::to_string(&synthetic_report()).unwrap();
    let payload: StoredRoastPayload = serde_json::from_str(&json).unwrap();
    assert!(matches!(payload, StoredRoastPayload::Legacy(_)));
}
```

并增加测试 helper：

```rust
fn claim(judgment: &str) -> RoastClaim {
    RoastClaim {
        judgment: judgment.into(),
        claim_type: ClaimType::StatisticalInterpretation,
        evidence_refs: vec!["我方士兵：76.stats.damage".into()],
    }
}
```

- [ ] **Step 2: 运行测试确认缺少 V3 类型而失败**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test ai_roast_report narrative_report_has_hidden_plan_title_and_commentary
```

Expected: FAIL，提示 `AiRoastNarrative`、`NarrativeAnalysis` 或 `StoredRoastPayload` 不存在。

- [ ] **Step 3: 在 `report.rs` 增加完整 V3 类型并保留旧类型**

将版本改为 `3`，在现有 V2 `AiRoastReport` 类型之前加入：

```rust
pub const REPORT_SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiRoastNarrative {
    pub analysis: NarrativeAnalysis,
    pub title: String,
    pub commentary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NarrativeAnalysis {
    pub match_thesis: RoastClaim,
    pub current_player: RoastClaim,
    pub key_actors: Vec<KeyActorClaim>,
    pub supporting_factors: Vec<RoastClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoastClaim {
    pub judgment: String,
    pub claim_type: ClaimType,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyActorClaim {
    pub subject: String,
    pub impact: ActorImpact,
    #[serde(flatten)]
    pub claim: RoastClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorImpact {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StoredRoastPayload {
    Narrative(AiRoastNarrative),
    Legacy(AiRoastReport),
}

impl StoredRoastPayload {
    pub fn evidence_refs(&self) -> Vec<String> {
        match self {
            Self::Narrative(report) => std::iter::once(&report.analysis.match_thesis)
                .chain(std::iter::once(&report.analysis.current_player))
                .chain(report.analysis.key_actors.iter().map(|item| &item.claim))
                .chain(report.analysis.supporting_factors.iter())
                .flat_map(|claim| claim.evidence_refs.iter().cloned())
                .collect(),
            Self::Legacy(report) => report
                .decisive_factors
                .iter()
                .chain(std::iter::once(&report.current_player))
                .chain(report.team_and_matchups.iter().map(|item| &item.insight))
                .flat_map(|item| item.evidence_refs.iter().cloned())
                .collect(),
        }
    }
}
```

- [ ] **Step 4: 运行报告测试**

Run: `cargo test --test ai_roast_report`

Expected: PASS。

- [ ] **Step 5: 提交报告类型**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/report.rs apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs
git commit -m "feat: define narrative AI roast report"
```

## Task 2: 增加关键任务知识与第一责任人筛选

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/ai_roast/task.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/ai_roast_task.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs`
- Modify: `apps/owstats-desktop/runtime-config/ai-roast-knowledge.json`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_knowledge.rs`

- [ ] **Step 1: 写对空任务第一责任人失败测试**

新建 `ai_roast_task.rs`，使用合成玩家构造：

```rust
#[test]
fn anti_air_assigns_primary_responder_before_secondary_helpers() {
    let knowledge = embedded_knowledge().unwrap();
    let team = vec![
        player("我方士兵：76", "士兵：76", "damage"),
        player("我方D.Va", "D.Va", "tank"),
        player("我方安娜", "安娜", "support"),
    ];
    let opponent = vec![player("敌方法老之鹰", "法老之鹰", "damage")];
    let context = task_knowledge_for_players(&knowledge, &team, &opponent);
    let anti_air = context.assignments.get("anti_air").unwrap();
    assert_eq!(anti_air.threat_subjects, vec!["敌方法老之鹰"]);
    assert_eq!(anti_air.primary_responsible_subjects, vec!["我方士兵：76"]);
    assert_eq!(
        anti_air.secondary_responsible_subjects,
        vec!["我方D.Va", "我方安娜"]
    );
}
```

`player` helper 只填写匿名 subject、hero、role 和默认统计，不使用真实 ID。

- [ ] **Step 2: 运行测试确认模块不存在**

Run: `cargo test --test ai_roast_task`

Expected: FAIL，提示 `ai_roast::task` 不存在。

- [ ] **Step 3: 在 `task.rs` 定义任务配置和运行时上下文**

```rust
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use super::{knowledge::RoastKnowledge, snapshot::RoastPlayer};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchupTaskProfile {
    pub label: String,
    pub threat_heroes: Vec<String>,
    pub primary_responders: Vec<String>,
    pub secondary_responders: Vec<String>,
    pub evaluation_fields: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UniversalTaskProfile {
    pub label: String,
    pub explanation: String,
    pub evaluation_fields: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskKnowledgeContext {
    pub universal: BTreeMap<String, UniversalTaskProfile>,
    pub assignments: BTreeMap<String, TaskAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskAssignment {
    pub label: String,
    pub threat_subjects: Vec<String>,
    pub primary_responsible_subjects: Vec<String>,
    pub secondary_responsible_subjects: Vec<String>,
    pub evaluation_fields: Vec<String>,
    pub limitations: Vec<String>,
}

pub fn task_knowledge_for_players(
    knowledge: &RoastKnowledge,
    team: &[RoastPlayer],
    opponent: &[RoastPlayer],
) -> TaskKnowledgeContext {
    let assignments = knowledge
        .matchup_tasks
        .iter()
        .filter_map(|(key, profile)| {
            let threats = opponent.iter().filter(|row| profile.threat_heroes.contains(&row.hero)).map(|row| row.subject.clone()).collect::<Vec<_>>();
            let primary = team.iter().filter(|row| profile.primary_responders.contains(&row.hero)).map(|row| row.subject.clone()).collect::<Vec<_>>();
            if threats.is_empty() || primary.is_empty() { return None; }
            let secondary = team.iter().filter(|row| profile.secondary_responders.contains(&row.hero)).map(|row| row.subject.clone()).collect::<Vec<_>>();
            Some((key.clone(), TaskAssignment {
                label: profile.label.clone(),
                threat_subjects: threats,
                primary_responsible_subjects: primary,
                secondary_responsible_subjects: secondary,
                evaluation_fields: profile.evaluation_fields.clone(),
                limitations: profile.limitations.clone(),
            }))
        })
        .collect();
    TaskKnowledgeContext { universal: knowledge.universal_tasks.clone(), assignments }
}
```

- [ ] **Step 4: 扩展知识根类型并校验配置**

在 `RoastKnowledge` 增加：

```rust
pub universal_tasks: BTreeMap<String, UniversalTaskProfile>,
pub matchup_tasks: BTreeMap<String, MatchupTaskProfile>,
```

将知识版本断言改为 `knowledge.version == 2`，并校验每个 matchup task 的 `threat_heroes` 与 `primary_responders` 非空、三个英雄列表无重复、`evaluation_fields` 非空。

- [ ] **Step 5: 在 JSON 中加入最小可审查任务集**

将顶层 `version` 改为 `2`，加入：

```json
"universalTasks": {
  "frontline_value": {"label":"前排承压与推进收益","explanation":"高承伤需要结合阵亡、减伤、击杀参与和队伍结果解释。","evaluationFields":["damageTaken","deaths","mitigation","killParticipationRate"],"limitations":["最终计分板不能证明具体推进过程。"]},
  "damage_conversion": {"label":"输出到击杀的转化","explanation":"高伤害需要结合击杀、最后一击、参与率和存活判断。","evaluationFields":["damage","kills","finalHits","killParticipationRate","deaths"],"limitations":["不能推断伤害具体打给了谁。"]},
  "backline_survival": {"label":"后排生存与保护","explanation":"支援高阵亡可能压缩全队输出窗口，但不能仅凭死亡断言保护者具体失误。","evaluationFields":["deaths","healing","assists","damageTaken"],"limitations":["不能推断单次死亡来源或站位。"]},
  "resource_conversion": {"label":"资源占用与产出","explanation":"高治疗承接或高承伤需要与击杀、最后一击、参与和存活共同解释。","evaluationFields":["healingReceived","damageTaken","kills","finalHits","deaths"],"limitations":["资源数据不等于具体治疗或保护目标。"]}
},
"matchupTasks": {
  "anti_air": {"label":"对空与空中威胁处理","threatHeroes":["法老之鹰","回声"],"primaryResponders":["士兵：76","卡西迪","艾什","黑百合","索杰恩"],"secondaryResponders":["D.Va","安娜","巴蒂斯特","伊拉锐"],"evaluationFields":["kills","finalHits","killParticipationRate","damage","deaths"],"limitations":["可以判断主要对空任务是否形成结果，不能断言具体攻击目标或英雄切换。"]}
}
```

- [ ] **Step 6: 注册模块并运行知识与任务测试**

在 `mod.rs` 增加 `pub mod task;`。

Run:

```powershell
cargo test --test ai_roast_knowledge
cargo test --test ai_roast_task
```

Expected: PASS。

- [ ] **Step 7: 提交任务知识**

```powershell
git add apps/owstats-desktop/runtime-config/ai-roast-knowledge.json apps/owstats-desktop/src-tauri/src/ai_roast/mod.rs apps/owstats-desktop/src-tauri/src/ai_roast/knowledge.rs apps/owstats-desktop/src-tauri/src/ai_roast/task.rs apps/owstats-desktop/src-tauri/tests/ai_roast_knowledge.rs apps/owstats-desktop/src-tauri/tests/ai_roast_task.rs
git commit -m "feat: add AI roast task ownership knowledge"
```

## Task 3: 将任务上下文接入 V3 快照和证据目录

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs`

- [ ] **Step 1: 写 `taskKnowledge` 序列化和证据失败测试**

```rust
#[test]
fn v3_snapshot_serializes_task_knowledge() {
    let snapshot = synthetic_snapshot_with_anti_air();
    let value = serde_json::to_value(snapshot).unwrap();
    assert_eq!(value["schemaVersion"], 3);
    assert_eq!(value["taskKnowledge"]["assignments"]["anti_air"]["primaryResponsibleSubjects"][0], "我方士兵：76");
}

#[test]
fn task_assignments_are_available_as_game_knowledge_evidence() {
    let catalog = evidence_catalog(&synthetic_snapshot_with_anti_air());
    let entry = catalog.get("taskKnowledge.assignments.anti_air.primaryResponsibleSubjects.0").unwrap();
    assert_eq!(entry.semantic, EvidenceSemantic::GameKnowledge);
}
```

- [ ] **Step 2: 运行定向测试确认失败**

Run: `cargo test --test ai_roast_snapshot v3_snapshot_serializes_task_knowledge`

Expected: FAIL，快照仍为版本 2 或没有 `taskKnowledge`。

- [ ] **Step 3: 扩展快照**

在 `RoastSnapshot` 增加：

```rust
pub task_knowledge: TaskKnowledgeContext,
```

将 `INPUT_SCHEMA_VERSION` 改为 `3`，并在 `build_roast_snapshot` 返回值中初始化：

```rust
task_knowledge: TaskKnowledgeContext::default(),
```

- [ ] **Step 4: 协调器在匿名主体建立后附加任务上下文**

在 `generate_captured` 中改为：

```rust
let mut snapshot = build_roast_snapshot(&conn, &request.match_id, &detail, context)
    .map_err(|_| MATERIAL_MESSAGE)?;
snapshot.task_knowledge = task_knowledge_for_players(
    &knowledge,
    &snapshot.scoreboard.team,
    &snapshot.scoreboard.opponent,
);
```

- [ ] **Step 5: 把任务上下文加入证据目录**

在 `evidence_catalog` 中调用现有 `add_object`：

```rust
add_object(
    &mut catalog,
    "taskKnowledge",
    &snapshot.task_knowledge,
    EvidenceSemantic::GameKnowledge,
);
```

- [ ] **Step 6: 运行快照、报告和协调器测试**

Run:

```powershell
cargo test --test ai_roast_snapshot
cargo test --test ai_roast_report
cargo test --test ai_roast_coordinator
```

Expected: PASS。

- [ ] **Step 7: 提交 V3 输入**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/snapshot.rs apps/owstats-desktop/src-tauri/src/ai_roast/report.rs apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs
git commit -m "feat: attach task context to AI roast snapshot"
```

## Task 4: 将校验器迁移为叙事计划与连续正文校验

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs`

- [ ] **Step 1: 写结构、推断和口语锐度测试**

增加以下断言：

```rust
#[test]
fn narrative_requires_one_thesis_and_at_most_two_supporting_factors() {
    let mut report = narrative("我方士兵：76");
    report.analysis.supporting_factors = vec![claim(), claim(), claim()];
    assert_eq!(validate_narrative_output(&serde_json::to_string(&report).unwrap(), &snapshot()), Err(RoastValidationError::InvalidSectionCount));
}

#[test]
fn game_slang_is_allowed_but_identity_attacks_are_not() {
    let mut report = narrative("我方士兵：76");
    report.commentary = "这个士兵：76到底在忙什么？对空任务交了张白卷，这把头锅戴好。".into();
    assert!(validate(&report, &snapshot()).is_ok());
    report.commentary = "这个玩家就是蠢货。".into();
    assert_eq!(validate(&report, &snapshot()), Err(RoastValidationError::Unfriendly));
}

#[test]
fn anti_air_blame_requires_task_knowledge_and_combat_evidence() {
    let mut report = narrative("我方士兵：76");
    report.analysis.match_thesis.claim_type = ClaimType::GameContextInference;
    report.analysis.match_thesis.evidence_refs = vec![
        "taskKnowledge.assignments.anti_air.primaryResponsibleSubjects.0".into(),
        "敌方法老之鹰.stats.deaths".into(),
        "敌方法老之鹰.stats.kills".into(),
        "我方士兵：76.stats.finalHits".into(),
    ];
    assert!(validate(&report, &snapshot_with_anti_air()).is_ok());
}
```

- [ ] **Step 2: 运行测试确认旧五区块入口失败**

Run: `cargo test --test ai_roast_report narrative_requires_one_thesis_and_at_most_two_supporting_factors`

Expected: FAIL，`validate_narrative_output` 不存在或旧 schema 拒绝新字段。

- [ ] **Step 3: 新增 `validate_narrative_output` 并复用证据校验**

入口必须：

```rust
pub fn validate_narrative_output(
    input: &str,
    snapshot: &RoastSnapshot,
) -> Result<AiRoastNarrative, RoastValidationError> {
    let report: AiRoastNarrative = serde_json::from_str(normalize_json_input(input))
        .map_err(|_| RoastValidationError::InvalidJson)?;
    if report.title.trim().is_empty() || report.commentary.trim().is_empty() {
        return Err(RoastValidationError::EmptyField);
    }
    if report.title.chars().count() > 36 || report.commentary.chars().count() > 650 {
        return Err(RoastValidationError::TextTooLong);
    }
    if report.analysis.supporting_factors.len() > 2 || report.analysis.key_actors.len() > 3 {
        return Err(RoastValidationError::InvalidSectionCount);
    }
    if report.analysis.key_actors.iter().filter(|actor| actor.impact == ActorImpact::Negative).count() > 2 {
        return Err(RoastValidationError::InvalidSectionCount);
    }
    validate_narrative(&report, snapshot)?;
    Ok(report)
}
```

将旧 `validate_insight` 提炼为接收 `RoastClaim` 的 `validate_claim`，继续检查空引用、重复引用、未知路径、数字、比较、知识推断和不支持的具体因果。

- [ ] **Step 4: 增加主线和正文一致性规则**

实现以下确定性规则：

```rust
fn validate_thesis(
    claim: &RoastClaim,
    catalog: &EvidenceCatalog,
    snapshot: &RoastSnapshot,
) -> Result<(), RoastValidationError> {
    let dimensions = claim.evidence_refs.iter().map(|path| {
        if path.starts_with("taskKnowledge.") || path.starts_with("gameKnowledge.") { "knowledge" }
        else if path.starts_with("roleMatchups.") { "role_matchup" }
        else { path.split('.').next().unwrap_or("unknown") }
    }).collect::<HashSet<_>>();
    if claim.evidence_refs.len() < 2 || dimensions.len() < 2 {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    validate_claim(claim, catalog, snapshot)
}
```

正文出现某个匿名 subject 时，该 subject 必须等于当前玩家、出现在 `keyActors`，或被 `matchThesis`/`supportingFactors` 的证据路径引用。正文中的数字和比较词使用所有后台 claim 引用的证据合集复核。

- [ ] **Step 5: 保留口语表达，阻止无法证明的具体过程**

保留现有真正的人身攻击词 `菜狗、废物、蠢货、闭嘴、去死`；不要加入 `坐牢、头锅、战犯、失职、背锅、问题`。将不可证明过程检查扩展为：

```rust
fn contains_unsupported_process_claim(text: &str) -> bool {
    ["整局没打过", "从来没打", "拒绝换英雄", "故意送", "故意摆烂", "没交大", "不开麦"]
        .iter()
        .any(|word| text.contains(word))
}
```

“这个士兵：76干嘛不打法老之鹰”作为口语化反问不单独拦截，但必须由后台 `game_context_inference` 计划和任务证据支撑。

- [ ] **Step 6: 运行报告和场景测试**

Run:

```powershell
cargo test --test ai_roast_report
cargo test --test ai_roast_scenarios
```

Expected: PASS。

- [ ] **Step 7: 提交叙事校验器**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/validator.rs apps/owstats-desktop/src-tauri/tests/ai_roast_report.rs apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs
git commit -m "feat: validate focused AI roast narratives"
```

## Task 5: 重写单次请求提示词和有限修正

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/client.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs`

- [ ] **Step 1: 写提示词契约失败测试**

```rust
#[test]
fn prompt_demands_one_story_title_and_continuous_commentary() {
    let prompt = system_prompt();
    assert!(prompt.contains("如果只能告诉朋友一件事"));
    assert!(prompt.contains("一个有趣的主标题"));
    assert!(prompt.contains("一篇连续正文"));
    assert!(prompt.contains("第一责任人"));
    assert!(prompt.contains("胜利"));
    assert!(prompt.contains("失败"));
    assert!(!prompt.contains("固定为五区块"));
    assert!(!prompt.contains("模仿") && !prompt.contains("其他模型"));
}

#[test]
fn request_uses_narrative_schema_and_version_three() {
    let request = request_json(&snapshot());
    assert_eq!(PROMPT_VERSION, 3);
    let text = request.to_string();
    assert!(text.contains("matchThesis"));
    assert!(text.contains("supportingFactors"));
    assert!(!text.contains("decisiveFactors"));
}
```

- [ ] **Step 2: 运行测试确认旧提示词失败**

Run: `cargo test --test ai_roast_client prompt_demands_one_story_title_and_continuous_commentary`

Expected: FAIL，旧提示词仍要求五区块。

- [ ] **Step 3: 将生成返回类型和校验入口改为 V3**

`DeepSeekRoastClient::generate` 与内部方法返回 `AiRoastNarrative`，调用 `validate_narrative_output`。将 `PROMPT_VERSION` 改为 `3`。

- [ ] **Step 4: 用明确的单主线提示词替换旧系统提示词**

新提示词必须包含以下完整骨架和行为要求：

```text
你是《守望先锋》赛后损友。先理解比赛，再说人话；不写五区块报告，不引用、扮演或自称任何外部产品人格。

写作前在 analysis 中完成：判断比赛形状；提出候选解释并寻找反证；识别最关键任务和第一责任人；确定唯一 matchThesis、当前玩家定位、关键人物和最多两个辅助因素。职责总差只是候选证据，不能自动成为答案。

胜利时进入庆功视角：尽量找到当前玩家真实亮点，并表扬真正带飞者；如果当前玩家被抬赢，用善意拆台而不是虚构贡献。失败时先说出当前玩家为什么坐牢，再依据证据点出最多两名主要责任人；当前玩家自己是主要问题时必须直说。

正文可以使用坐牢、头锅、战犯、付费陪玩、交白卷等游戏圈表达，也可以使用反问和夸张；只嘲讽本局表现，不攻击人格、智力、身份或现实生活。后台判断必须谨慎，前台不要写“从数据推断、可能、一定程度”等公关分析腔。

如果锐评换到多数其他对局仍成立，重写。如果只是在朗读职责差值，继续检查转化、资源、存活、关键威胁和任务归属。如果标题不能概括正文唯一主线，重写。

只返回 JSON：{"analysis":{"matchThesis":{"judgment":"","claimType":"statistical_interpretation","evidenceRefs":[""]},"currentPlayer":{"judgment":"","claimType":"statistical_interpretation","evidenceRefs":[""]},"keyActors":[],"supportingFactors":[]},"title":"","commentary":""}
```

同时保留数字、比较、身份、凭据、录像边界和证据路径规则。正文目标三百五十至五百汉字，硬上限六百五十字符；标题硬上限三十六字符。

- [ ] **Step 5: 重写用户消息**

用户消息明确说明快照已包含 `taskKnowledge`，并要求 `game_context_inference` 同时引用任务/游戏知识和战斗统计。删除当前玩家目标时间必须占据固定栏目、每条六十至九十字、禁止失职/背锅等旧规则。

- [ ] **Step 6: 更新有限修正分类与定位**

`validation_location` 只返回 `analysis.matchThesis`、`analysis.currentPlayer`、`analysis.keyActors[n]`、`analysis.supportingFactors[n]`、`title` 或 `commentary`。长度失败要求压缩标题或连续正文；证据失败要求修正对应 claim；主线证据不足时才允许更换 `matchThesis`。

- [ ] **Step 7: 运行客户端测试**

Run: `cargo test --test ai_roast_client`

Expected: PASS；请求仍保持 `temperature: 0.8`、thinking enabled、high reasoning effort、JSON object 和最多一次修正。

- [ ] **Step 8: 提交提示词**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/client.rs apps/owstats-desktop/src-tauri/tests/ai_roast_client.rs
git commit -m "feat: generate focused AI roast narratives"
```

## Task 6: 保存 V3 payload 并兼容 V2 五区块

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/store.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_coordinator.rs`

- [ ] **Step 1: 写 V3 保存与 V2 读取失败测试**

```rust
#[test]
fn narrative_payload_persists_title_body_and_hidden_analysis() {
    let conn = database();
    let record = narrative_record("match-a");
    let view = save_roast_once(&conn, &record).unwrap();
    assert_eq!(view.award, "法老之鹰在天上续了年卡");
    assert_eq!(view.commentary, "连续正文");
    assert!(view.report.is_none());
    let json: String = conn.query_row("SELECT report_json FROM match_ai_roasts WHERE match_id='match-a'", [], |row| row.get(0)).unwrap();
    assert!(json.contains("matchThesis"));
}

#[test]
fn legacy_five_section_payload_still_returns_legacy_report() {
    seed_legacy_v2_roast(&conn, "match-old");
    let view = roast_for_match(&conn, "match-old").unwrap().unwrap();
    assert!(view.report.is_some());
}
```

- [ ] **Step 2: 运行存储测试确认类型不匹配**

Run: `cargo test --test ai_roast_store narrative_payload_persists_title_body_and_hidden_analysis`

Expected: FAIL，`AiRoastRecord.report` 仍只接受旧 `AiRoastReport`。

- [ ] **Step 3: 把存储记录改为兼容 payload**

```rust
pub struct AiRoastRecord {
    pub match_id: String,
    pub award: String,
    pub commentary: String,
    pub report: Option<StoredRoastPayload>,
    pub used_fact_ids: Vec<String>,
    pub model: String,
    pub evaluation_rule_version: i64,
    pub material_version: i64,
    pub prompt_version: i64,
    pub report_schema_version: Option<i64>,
    pub input_schema_version: Option<i64>,
    pub knowledge_version: Option<i64>,
    pub created_at: String,
}
```

`serialized_values` 调用 `payload.evidence_refs()`，排序去重后写 `used_fact_ids_json`。

- [ ] **Step 4: 读取时只把 V2 暴露为旧前端报告**

```rust
let report = report_json
    .map(|json| serde_json::from_str::<StoredRoastPayload>(&json).map_err(|_| anyhow!("结构化锐评记录无效")))
    .transpose()?;
let legacy_report = match report {
    Some(StoredRoastPayload::Legacy(report)) => Some(report),
    Some(StoredRoastPayload::Narrative(_)) | None => None,
};
Ok(Some(AiMatchRoastView { award, commentary, report: legacy_report }))
```

- [ ] **Step 5: 协调器保存 V3 标题、正文和版本**

```rust
let record = AiRoastRecord {
    match_id: request.match_id,
    award: output.title.clone(),
    commentary: output.commentary.clone(),
    report: Some(StoredRoastPayload::Narrative(output)),
    used_fact_ids: Vec::new(),
    model: MODEL.to_string(),
    evaluation_rule_version: 0,
    material_version: INPUT_SCHEMA_VERSION,
    prompt_version: PROMPT_VERSION,
    report_schema_version: Some(REPORT_SCHEMA_VERSION),
    input_schema_version: Some(INPUT_SCHEMA_VERSION),
    knowledge_version: Some(knowledge_version),
    created_at: Utc::now().to_rfc3339(),
};
```

- [ ] **Step 6: 运行存储与协调器测试**

Run:

```powershell
cargo test --test ai_roast_store
cargo test --test ai_roast_coordinator
```

Expected: PASS；重新生成仍用事务式单条 UPSERT，失败时不覆盖旧结果。

- [ ] **Step 7: 提交存储协调**

```powershell
git add apps/owstats-desktop/src-tauri/src/ai_roast/store.rs apps/owstats-desktop/src-tauri/src/ai_roast/coordinator.rs apps/owstats-desktop/src-tauri/tests/ai_roast_store.rs apps/owstats-desktop/src-tauri/tests/ai_roast_coordinator.rs
git commit -m "feat: persist narrative AI roasts compatibly"
```

## Task 7: 将新成功态改为主标题加连续正文

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx`
- Modify: `apps/owstats-desktop/src/features/command-center/DeepSeekSettings.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx`

- [ ] **Step 1: 先把 UI 契约改成 V3 成功态并确认失败**

在测试中新增 V3 view：

```tsx
const narrativeMarkup = renderToStaticMarkup(
  createElement(AiMatchRoast, {
    matchId: 'match-narrative',
    initialRoast: {
      award: '法老之鹰在天上续了年卡',
      commentary: '我方不是没有输出，问题是最该处理的威胁一直活得很舒服。',
      report: null,
    },
    detailReady: true,
  }),
);
assert.match(narrativeMarkup, /class="aiMatchRoastNarrative"/);
assert.match(narrativeMarkup, /<h3[^>]*>法老之鹰在天上续了年卡<\/h3>/);
assert.match(narrativeMarkup, /<p[^>]*>我方不是没有输出/);
for (const heading of ['本场判词', '胜负手', '你这局', '队友与对位', '终审锐评']) {
  assert.doesNotMatch(narrativeMarkup, new RegExp(heading));
}
```

保留原有 `savedMarkup` 作为 V2 历史兼容断言。

- [ ] **Step 2: 运行前端契约测试确认失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/aiMatchRoastUi.test.tsx
```

Expected: FAIL，当前 fallback 仍使用 `aiMatchRoastLegacy`。

- [ ] **Step 3: 将无 report 的结果渲染为正式连续叙事**

在 `AiMatchRoast.tsx` 将 fallback 改为：

```tsx
function RoastContent({ roast }: { roast: AiMatchRoastView }) {
  if (!roast.report) {
    return (
      <article className="aiMatchRoastNarrative">
        <h3>{roast.award}</h3>
        <p>{roast.commentary}</p>
      </article>
    );
  }
  return <LegacyRoastReport report={roast.report} />;
}
```

将旧 `RoastReport` 重命名为 `LegacyRoastReport`，只接收 `AiRoastReport`；新生成、旧标题正文记录都使用连续叙事样式，V2 五区块 JSON 才进入历史渲染分支。

- [ ] **Step 4: 调整 TypeScript 命名说明兼容边界**

把五区块类型重命名为 `LegacyAiRoastReport`，并在 `AiMatchRoastView` 使用：

```ts
export type AiMatchRoastView = {
  award: string;
  commentary: string;
  report: LegacyAiRoastReport | null;
};
```

内部 evidence 与 claim type 仅为历史 DTO，不新增 V3 后台 analysis 到前端类型。

- [ ] **Step 5: 增加连续叙事样式**

```css
.aiMatchRoastNarrative {
  min-width: 0;
  display: grid;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-6);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-control);
  background: var(--surface-secondary);
}

.aiMatchRoastNarrative h3 {
  margin: 0;
  color: var(--text-primary);
  font-size: 20px;
  line-height: 1.35;
  font-weight: 700;
}

.aiMatchRoastNarrative p {
  margin: 0;
  color: var(--text-secondary);
  font-size: 14px;
  line-height: 1.75;
  white-space: pre-line;
}
```

在窄屏媒体查询中将叙事 padding 改为 `var(--space-4)`。不得增加彩色标题、额外卡片、阴影或分区线。

- [ ] **Step 6: 修正 DeepSeek 数据披露**

将 `DeepSeekSettings.tsx` 的披露改为与实际请求一致：

```tsx
<p className="deepSeekSettingsDisclosure">
  仅发送匿名英雄阵容、计分板、派生指标与回合趋势；不会发送玩家名称、玩家 ID、笔记、原始 JSON、对局 ID 或凭据。
</p>
```

同步更新 UI 测试：必须出现“匿名英雄阵容”和“计分板”，不得继续声称“不发送完整阵容”。

- [ ] **Step 7: 运行 UI 测试和构建**

Run:

```powershell
npx tsx scripts/aiMatchRoastUi.test.tsx
npm test
npm run build
```

Expected: PASS。

- [ ] **Step 8: 提交 UI**

```powershell
git add apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx apps/owstats-desktop/src/features/command-center/DeepSeekSettings.tsx apps/owstats-desktop/src/styles/features/matches.css apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx
git commit -m "feat: present AI roast as one narrative"
```

## Task 8: 扩展理解场景和私有验收工具

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/bin/validate_ai_roast_live.rs`

- [ ] **Step 1: 增加胜负立场和非机械主线场景**

增加合成 case table，至少包含：

```rust
struct NarrativeCase {
    name: &'static str,
    result: &'static str,
    expected_thesis_subjects: &'static [&'static str],
    max_negative_actors: usize,
}

let cases = [
    NarrativeCase { name: "win_user_contributes_and_teammate_carries", result: "win", expected_thesis_subjects: &["当前玩家", "我方大爹"], max_negative_actors: 0 },
    NarrativeCase { name: "win_user_is_carried", result: "win", expected_thesis_subjects: &["我方大爹"], max_negative_actors: 0 },
    NarrativeCase { name: "loss_primary_anti_air_fails", result: "loss", expected_thesis_subjects: &["我方士兵：76", "敌方法老之鹰"], max_negative_actors: 2 },
    NarrativeCase { name: "loss_backline_collapse_explains_low_damage", result: "loss", expected_thesis_subjects: &["我方支援"], max_negative_actors: 2 },
    NarrativeCase { name: "close_match_has_no_unique_war_criminal", result: "loss", expected_thesis_subjects: &["职责转化"], max_negative_actors: 1 },
];
```

测试关注后台 plan 的证据合法性、责任人数和唯一主线，不断言模型必须逐字输出某个笑话。

- [ ] **Step 2: 更新真实验收条件**

将工具中的五区块数量检查替换为：

```rust
ensure!(!narrative.title.trim().is_empty(), "invalid_title");
ensure!(!narrative.commentary.trim().is_empty(), "invalid_commentary");
ensure!(narrative.analysis.supporting_factors.len() <= 2, "too_many_supporting_factors");
ensure!(narrative.analysis.key_actors.iter().filter(|actor| actor.impact == ActorImpact::Negative).count() <= 2, "too_many_negative_actors");
ensure!((250..=650).contains(&narrative.commentary.chars().count()), "invalid_commentary_length");
```

继续只输出 `case`、`stage`、`status`、`elapsed_ms`、`report_chars` 和汇总计数；不得打印标题、正文、后台计划、ID、英雄、路径、payload 或凭据。

- [ ] **Step 3: 运行离线场景和工具编译测试**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test ai_roast_scenarios
cargo test --bin validate_ai_roast_live
cargo run --bin validate_ai_roast_live -- --help
```

Expected: PASS；帮助文本不展开本机路径。

- [ ] **Step 4: 提交场景与工具更新**

```powershell
git add apps/owstats-desktop/src-tauri/tests/ai_roast_scenarios.rs apps/owstats-desktop/src-tauri/src/bin/validate_ai_roast_live.rs
git commit -m "test: cover focused AI roast understanding"
```

## Task 9: 全量验证与交付检查

**Files:**
- Verify all AI roast files; only modify scoped files if a verification failure exposes a defect.

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

- [ ] **Step 3: 新旧契约和禁用风格静态扫描**

Run:

```powershell
rg -n "固定为五区块|decisiveFactors|teamAndMatchups|closingRoast|本场判词|终审锐评" apps/owstats-desktop/src-tauri/src/ai_roast/client.rs apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx apps/owstats-desktop/src/shared/types.ts
```

Expected: 客户端新提示词和 V3 成功态无五区块；仅允许历史 DTO/兼容渲染中出现旧字段名。系统提示不得引用或扮演任何外部产品人格。

- [ ] **Step 4: 路径与凭据静态扫描**

Run:

```powershell
rg -n "D:\\Codex|/@fs/|customerToken|authorization|cookie|browser profile" apps/owstats-desktop/src-tauri/src/ai_roast apps/owstats-desktop/runtime-config/ai-roast-knowledge.json apps/owstats-desktop/src/features/matches/AiMatchRoast.tsx
```

Expected: 不出现开发机绝对路径、资源逃逸或新增凭据字段；测试用禁止词常量需人工区分。

- [ ] **Step 5: 运行发布构建**

Run:

```powershell
cd apps\owstats-desktop
npm run tauri build
```

Expected: PASS。

- [ ] **Step 6: 可选的显式真实 API 验收**

只有操作者明确选择运行时，才从 Windows roaming application data 推导 app data 目录并传给 `validate_ai_roast_live`。工具必须使用临时数据库副本，唯一可报告结果为匿名通过数、失败阶段、耗时范围和字符数范围。

- [ ] **Step 7: 最终 diff 与工作树检查**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` 无输出；状态中只剩用户原有未提交改动或本计划预期文件，没有本地 SQLite、真实数据 dump、凭据、浏览器 profile、临时副本或模型正文。

- [ ] **Step 8: 最终报告**

列出实际运行的命令、通过/失败结果、未验证项和是否运行真实 API 工具。若没有运行真实 API，明确写“未接触真实凭据或真实对局数据”；若显式运行，只报告匿名统计并声明没有读取、打印或提交正文、ID、路径、payload 或凭据。
