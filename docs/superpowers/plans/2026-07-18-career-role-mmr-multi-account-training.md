# 生涯三职责 MMR 多账号训练与影子验证 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用全部本地账号和动态赛季训练三职责分层表现模型，验证职责 × 模式组合，生成隐私安全的最终训练报告和只读影子 MMR 对比，不自动激活生产表现修正。

**Architecture:** 保留既有胜负主干和账号独立 MMR，在训练侧增加多账号只读投影、角斗最终回合输入、职责核心 + 训练组偏移逻辑模型、账号/赛季/英雄防泄漏验证与分阶段 bootstrap。职责竞技与 `LeisurePreset` 共享职责队列参数，开放竞技与 `Leisure6v6` 共享开放队列参数，但五种实际表现模式分别验证。训练产物是不可变的聚合模型配置、15 格验收矩阵和两个账号各自的内存影子重放摘要；生产激活必须等用户查看最终报告后另行实施。

**Tech Stack:** Rust 2021、rusqlite、serde/serde_json、现有 `career_mmr` 模块、SQLite、PowerShell、Cargo integration tests

---

## 范围与执行前提

- 设计来源：`docs/superpowers/specs/2026-07-18-career-role-mmr-multi-account-training-design.md`。
- 执行前使用 `superpowers:using-git-worktrees` 创建隔离 worktree；不要在当前脏工作树直接实现。
- 本计划只实现训练、验证、模型产物和只读影子重放。不得自动切换 `mmr_model_versions.is_active`，不得修改正式 MMR。
- 所有真实数据运行只输出聚合结果，不输出账号目录名、BattleTag、玩家 ID、比赛 ID 或原始 JSON。
- 每一任务的提交只包含列出的文件，不暂存 worktree 中无关文件。

## 文件结构

新增模块：

- `career_mmr/training_sources.rs`：多账号只读训练源、聚合投影和账号分组。
- `career_mmr/performance_model.rs`：职责核心 + 训练组偏移的受约束逻辑模型。
- `career_mmr/training_report.rs`：模型产物、15 格矩阵、聚合报告和隐私检查。
- `career_mmr/shadow.rs`：不写数据库的账号独立影子历史重放。
- `src/bin/train_career_mmr.rs`：分阶段训练 CLI 和进度输出。

新增测试：

- `tests/career_mmr_multi_account_training.rs`
- `tests/career_mmr_hierarchical_model.rs`
- `tests/career_mmr_training_report.rs`
- `tests/career_mmr_shadow.rs`

## Task 1：建立六种实际模式、三个训练组和差异化胜负权重

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs:14-85`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs:160-290`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs:232-247`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/replay.rs:395-401`
- Test: `apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_mmr_rating.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_mmr_replay.rs`

- [ ] **Step 1: 写实际模式、训练组和权重映射失败测试**

```rust
#[test]
fn maps_all_training_and_outcome_mode_families() {
    assert_eq!(normalize_mode_family("SportPreset", None), Some(ModeFamily::RoleCompetitive));
    assert_eq!(normalize_mode_family("LeisurePreset", None), Some(ModeFamily::RoleQuick));
    assert_eq!(normalize_mode_family("SportOpen", None), Some(ModeFamily::OpenCompetitive));
    assert_eq!(normalize_mode_family("Leisure6v6", None), Some(ModeFamily::OpenQuick));
    assert_eq!(normalize_mode_family("SportFight", None), Some(ModeFamily::StadiumCompetitive));
    assert_eq!(normalize_mode_family("LeisureFight", None), Some(ModeFamily::StadiumQuick));
    assert_eq!(normalize_mode_family("QuickFight", None), Some(ModeFamily::StadiumQuick));
}

#[test]
fn pairs_competitive_and_quick_modes_into_shared_training_cohorts() {
    assert_eq!(training_cohort(ModeFamily::RoleCompetitive), TrainingCohort::RoleQueue);
    assert_eq!(training_cohort(ModeFamily::RoleQuick), TrainingCohort::RoleQueue);
    assert_eq!(training_cohort(ModeFamily::OpenCompetitive), TrainingCohort::OpenQueue);
    assert_eq!(training_cohort(ModeFamily::OpenQuick), TrainingCohort::OpenQueue);
    assert_eq!(training_cohort(ModeFamily::StadiumCompetitive), TrainingCohort::StadiumCompetitive);
    assert_eq!(training_cohort(ModeFamily::StadiumQuick), TrainingCohort::OutcomeOnly);
}
```

断言职责/开放竞技权重为 1.0、角斗竞技为 0.5、职责式/开放式快速为 0.3、角斗快速为
0.1；竞技上限仍为 ±30，三类快速上限仍为 ±10。

- [ ] **Step 2: 运行测试确认失败**

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test career_mmr_inputs maps_all_training_and_outcome_mode_families
cargo test --test career_mmr_rating every_mode_caps_both_directions_and_applies_share_before_clamping
```

Expected: FAIL，实际快速模式尚未拆分、训练组不存在或新权重不匹配。

- [ ] **Step 3: 实现统一模式 API**

```rust
pub enum ModeFamily {
    RoleCompetitive,
    RoleQuick,
    OpenCompetitive,
    OpenQuick,
    StadiumCompetitive,
    StadiumQuick,
}

pub enum TrainingCohort {
    RoleQueue,
    OpenQueue,
    StadiumCompetitive,
    OutcomeOnly,
}

impl ModeFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RoleCompetitive => "role_competitive",
            Self::RoleQuick => "role_quick",
            Self::OpenCompetitive => "open_competitive",
            Self::OpenQuick => "open_quick",
            Self::StadiumCompetitive => "stadium_competitive",
            Self::StadiumQuick => "stadium_quick",
        }
    }

    pub fn trains_performance(self) -> bool {
        !matches!(self, Self::StadiumQuick)
    }

    pub fn is_quick(self) -> bool {
        matches!(self, Self::RoleQuick | Self::OpenQuick | Self::StadiumQuick)
    }
}
```

`Role`、`ModeFamily` 和 `TrainingCohort` 统一派生 `Serialize`、`Deserialize`、`PartialOrd`、`Ord`；Task 5 删除
`performance_forward.rs` 中重复的 `Role` 枚举，所有模型、报告和重放只使用
`input::Role`/`input::ModeFamily`/`input::TrainingCohort`，避免同名类型漂移。

`normalize_mode_family` 明确将 `LeisurePreset` 和兼容值 `leisure` 映射到 `RoleQuick`，将
`Leisure6v6` 映射到 `OpenQuick`，并增加 `sportfight`、`leisurefight`、`quickfight`。
角斗提取 SQL 同时读取 `m.game_mode`，不再硬编码竞技模式。

- [ ] **Step 4: 统一权重、上限和持久化名称**

```rust
fn mode_weight(mode: ModeFamily) -> f64 {
    match mode {
        ModeFamily::RoleCompetitive | ModeFamily::OpenCompetitive => 1.0,
        ModeFamily::StadiumCompetitive => 0.5,
        ModeFamily::RoleQuick | ModeFamily::OpenQuick => 0.3,
        ModeFamily::StadiumQuick => 0.1,
    }
}

fn mode_cap(mode: ModeFamily) -> f64 {
    if mode.is_quick() { 10.0 } else { 30.0 }
}
```

删除 `replay.rs` 的重复模式名称 match，改用 `input.mode_family.as_str()`。

- [ ] **Step 5: 跑模式回归并提交**

```powershell
cargo test --test career_mmr_inputs
cargo test --test career_mmr_rating
cargo test --test career_mmr_replay
git add apps/owstats-desktop/src-tauri/src/career_mmr/input.rs apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs apps/owstats-desktop/src-tauri/src/career_mmr/replay.rs apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs apps/owstats-desktop/src-tauri/tests/career_mmr_rating.rs apps/owstats-desktop/src-tauri/tests/career_mmr_replay.rs
git commit -m "feat: split stadium competitive and quick mmr modes"
```

Expected: PASS；六种实际模式名称稳定，配对模式共享训练组但保留不同 `modeWeight`，角斗
快速事件写为 `stadium_quick`。

## Task 2：把角斗最终回合纳入训练投影

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/input.rs:160-444`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training.rs:10-412`
- Test: `apps/owstats-desktop/src-tauri/tests/career_mmr_training_projection.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs`

- [ ] **Step 1: 写角斗最终快照失败测试**

构造两个回合的 `SportFight`，第一回合 `finalHit=99`，最后回合 `finalHit=7`，整场 600 秒：

```rust
let projection = build_training_projection(&conn)?;
let row = projection.rows.iter().find(|row| row.mode_family == ModeFamily::StadiumCompetitive).unwrap();
assert_eq!(row.final_hit, Some(7.0));
assert_eq!(row.duration_sec, 600.0);
assert!(row.allow_performance);
assert_eq!(row.role, "support");
```

再构造 `LeisureFight`，断言 `StadiumQuick`、`outcome_only=true`、`allow_performance=false`。

- [ ] **Step 2: 运行测试确认当前整体排除角斗**

```powershell
cargo test --test career_mmr_training_projection stadium_final_snapshot_is_the_only_performance_source
```

Expected: FAIL；当前 `WHERE m.bucket<>'stadium'` 不返回角斗训练行。

- [ ] **Step 3: 提取可复用最终回合快照**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct StadiumFinalSnapshot {
    pub role: Role,
    pub hero_guid: String,
    pub statistics: Value,
}

pub fn stadium_final_snapshot(
    rounds: &[Value],
    current_player_key: &str,
    context: &ExtractionContext,
) -> Option<StadiumFinalSnapshot> {
    let round = last_structurally_valid_stadium_round(rounds)?;
    let player = current_player_row(round, current_player_key)?;
    let hero_guid = scalar_text(player.get("heroGuid"))?;
    let role = context.hero_roles.get(&hero_guid).copied()?;
    Some(StadiumFinalSnapshot {
        role,
        hero_guid,
        statistics: stadium_performance_features(player),
    })
}
```

`extract_stadium_match_with_context` 和训练投影共同调用。旧 shape 审计只作诊断，不再用 `Ambiguous` 关闭 `SportFight` 表现。
这里的快照严格等于排序后的最后有效回合；不得累加、平均或用倒数第二回合替代。

- [ ] **Step 4: 在训练投影加入角斗分支**

角斗训练行使用最终快照英雄和统计，`duration_sec`/`hero_duration_sec` 都使用整场时长；`SportFight` 可训练，`LeisureFight/QuickFight` outcome-only。`TrainingAudit` 新增：

```rust
pub stadium_competitive_candidates: usize,
pub stadium_quick_outcome_only_rows: usize,
pub stadium_performance_eligible_rows: usize,
```

- [ ] **Step 5: 验证并提交**

```powershell
cargo test --test career_mmr_inputs stadium_uses_only_last_valid_round_role_and_performance
cargo test --test career_mmr_training_projection
git add apps/owstats-desktop/src-tauri/src/career_mmr/input.rs apps/owstats-desktop/src-tauri/src/career_mmr/training.rs apps/owstats-desktop/src-tauri/tests/career_mmr_inputs.rs apps/owstats-desktop/src-tauri/tests/career_mmr_training_projection.rs
git commit -m "feat: train from stadium final snapshots"
```

Expected: PASS；前面回合的 99 不进入训练行，角斗竞技可训练，角斗快速只参与胜负。

## Task 3：实现多账号只读训练源和跨账号防泄漏分组

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/training_sources.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_multi_account_training.rs`

- [ ] **Step 1: 写两个账号联合投影失败测试**

创建两个 registry 账号数据库和一个 registry 外备份库；正式库各有独立比赛并共享一场真实比赛。断言：

```rust
let sources = discover_training_sources(&shared_paths)?;
assert_eq!(sources.len(), 2);
let projection = build_multi_account_projection(&sources)?;
assert_eq!(projection.audit.account_datasets, 2);
let shared = projection.rows.iter().filter(|r| r.source_match_group == shared_group).collect::<Vec<_>>();
assert_eq!(shared.len(), 2);
assert_ne!(shared[0].account_group, shared[1].account_group);
assert_eq!(shared[0].match_group, shared[1].match_group);
assert_eq!(projection.rows.len(), expected_current_player_segments);
```

fixture 同时放入高统计队友和对手，断言他们不会成为额外 `TrainingRow`；他们只影响
`team_strength`、`opponent_strength` 和 `available_team_healing` 等临时上下文字段。

- [ ] **Step 2: 运行测试确认 API 不存在**

```powershell
cargo test --test career_mmr_multi_account_training
```

Expected: FAIL。

- [ ] **Step 3: 增加完全只读的 registry 数据源发现**

```rust
pub struct TrainingSourceSpec {
    pub account_group: String,
    database_path: PathBuf,
}

pub fn discover_training_sources(shared: &SharedAppPaths) -> Result<Vec<TrainingSourceSpec>> {
    let registry = read_registry(&shared.registry())?;
    registry.accounts.iter().enumerate().map(|(index, entry)| {
        let paths = shared.account(&entry.account_id);
        read_manifest(&paths.manifest(), &entry.account_id)?;
        verify_account_path_boundary(shared, &paths)?;
        Ok(TrainingSourceSpec {
            account_group: format!("dataset-{:03}", index + 1),
            database_path: paths.database(),
        })
    }).collect()
}
```

该入口只调用只读的 `read_registry`/`read_manifest`，不得调用可能执行恢复、迁移或写索引的
`AccountContextManager::load`。`database_path` 保持私有，不实现 `Serialize`/`Debug`，错误不
拼接路径。只枚举 registry 账号，不扫描 backups。

- [ ] **Step 4: 增加账号分组和聚合器**

`TrainingRow` 增加：

```rust
pub account_group: String,       // dataset-001，不是账号 ID
pub source_match_group: String,  // 仅内存稳定分组键
```

`training_sources.rs` 核心入口：

```rust
pub fn build_multi_account_projection(sources: &[TrainingSourceSpec]) -> Result<TrainingProjection> {
    let mut combined = TrainingProjection::default();
    for source in sources {
        let conn = open_query_only(&source.database_path)?;
        validate_training_schema(&conn)?;
        combined.merge(build_training_projection_for_account(&conn, &source.account_group)?);
    }
    combined.dedupe_account_rows();
    Ok(combined)
}
```

`TrainingProjection`/`TrainingAudit` 派生或实现 `Default` 和 `merge`。去重键为
`(account_group, source_match_group, role, hero_group, begin_ts)`；不同账号同场保留两条，但
共享 `match_group`。

- [ ] **Step 5: 验证只读、去重、同折并提交**

```powershell
cargo test --test career_mmr_multi_account_training
git add apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs apps/owstats-desktop/src-tauri/src/career_mmr/training.rs apps/owstats-desktop/src-tauri/src/career_mmr/training_sources.rs apps/owstats-desktop/src-tauri/tests/career_mmr_multi_account_training.rs
git commit -m "feat: aggregate privacy-safe multi-account mmr training rows"
```

Expected: PASS；备份库未读取，数据库 query-only，跨账号同场不跨折。

## Task 4：扩展折内稳健基线和资源转化特征

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/training.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/performance.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_forward.rs`
- Test: `apps/owstats-desktop/src-tauri/tests/career_mmr_performance_matrix.rs`

- [ ] **Step 1: 写基线回退和未来泄漏失败测试**

```rust
assert_eq!(row.baselines[&FeatureName::Damage], BaselineLevel::HeroModeSeason);
assert_eq!(fallback.baselines[&FeatureName::Damage], BaselineLevel::HeroEnvironment);
assert_eq!(last.baselines[&FeatureName::Damage], BaselineLevel::RoleEnvironment);
```

在 validation-only 加入极端值后，训练行的中位数和 MAD 必须不变。

- [ ] **Step 2: 写资源债务五方案测试**

构造两条同英雄同模式样本：受到治疗相同，但一条击杀/助攻/生存转化高，一条低。断言仅 `ResourceConversionDebt` 对低转化样本产生更高债务；多英雄且不能归属的整场 `healingTaken` 被排除。

- [ ] **Step 3: 运行测试确认失败**

```powershell
cargo test --test career_mmr_performance_matrix
```

Expected: FAIL；当前缺少环境回退和 team healing 输入。

- [ ] **Step 4: 扩展投影和基线枚举**

`TrainingRow` 增加：

```rust
pub available_team_healing: Option<f64>,
pub environment: TrainingEnvironment, // Ordinary | Stadium
pub base_delta: Option<f64>,           // 来自同模型基线事件，仅用于 MMR 限幅评估
```

训练投影从 active `career_mmr_events` 同时读取 `base_delta`；事件不存在时使用初始 sigma、
当前 `expectedWinRate`、mode、role share 和 data quality 调用 `outcome_delta_for_expected` 生成
确定性回退。所有 cap 命中率和平均调整必须以 MMR 单位计算，不能把概率差冒充 MMR。

`BaselineLevel` 改为：

```rust
HeroModeSeason,
HeroMode,
HeroEnvironment,
RoleMode,
RoleEnvironment,
Unavailable,
```

基线键包含 environment，普通与角斗不得直接共用原始基线。训练折拟合后才能转换验证折。
`MatrixRow` 额外保存非敏感的 `damage_baseline_median_per_ten: Option<f64>`，供输出伤害转化
计算；该中位数仍只能来自训练折。

- [ ] **Step 5: 实现预期受到治疗和五种互斥策略**

在折内实现 `HealingExpectationModel`：样本少于 100 时回退到同基线桶中位数；否则对标准化
预测量执行 8 次 Huber IRLS，每次解带 `lambda=0.1` 的 ridge normal equation。模型和回退
中位数都只能由训练折生成。预测与债务计算为：

```rust
let expected = healing_model.predict(&[
    duration_per_ten,
    standardized_damage_taken,
    standardized_death,
    standardized_available_team_healing,
]);
let excess = actual_healing_taken - expected;
let resource_debt = excess.max(0.0) * (1.0 - normalized_role_conversion).max(0.0);
```

`DirectNegative` 使用原始每 10 分钟暴露，`HeroNormalized` 使用稳健 Z，`ExcessHealing` 使用正超额，`ResourceConversionDebt` 使用上式，`None` 不生成 debt。多英雄普通比赛不复制或线性分摊整场 `healingTaken`。

同时在 `FeatureValues` 增加职责派生特征，并在完成原始稳健标准化后计算：

```rust
pub frontline_conversion: Option<f64>,
pub survival: Option<f64>,
pub abnormal_damage_taken: Option<f64>,
```

公式固定为：

```text
tank.frontline_conversion =
  positive(damageTakenZ)
  × (0.5 + 0.5 × clamp(-deathZ / 3, 0, 1))
  × meanPositive(finalZ, nonFinalZ, assistZ, objectiveZ)

support.survival = clamp(-deathZ, -3, 3)

abnormal_damage_taken = positive(damageTakenZ) × positive(deathZ)
```

输出的 `damage_conversion` 依赖正在比较的最后一击倍率，不存入通用 `FeatureValues`；Task 5
在 `role_vector(row, multiplier)` 中按下式即时生成，再用训练折分布稳健缩放：

```text
(finalBlows × selectedFinalMultiplier + nonFinalEliminations)
/ max(damagePer10, 0.25 × heroDamageMedian)
```

测试必须证明单独提高伤害/治疗不会自动提高转化特征；末日铁拳、路霸等重装只与自己的
英雄基线比较。

- [ ] **Step 6: 运行回归并提交**

```powershell
cargo test --test career_mmr_performance_matrix
cargo test --test career_mmr_training_projection
git add apps/owstats-desktop/src-tauri/src/career_mmr/training.rs apps/owstats-desktop/src-tauri/src/career_mmr/performance.rs apps/owstats-desktop/src-tauri/src/career_mmr/performance_forward.rs apps/owstats-desktop/src-tauri/tests/career_mmr_performance_matrix.rs
git commit -m "feat: build fold-local role and environment baselines"
```

Expected: PASS；折内基线无未来泄漏，角斗/普通隔离，资源债务只惩罚低转化超额资源。

## Task 5：实现职责核心 + 训练组偏移受约束逻辑模型

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_model.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_forward.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_hierarchical_model.rs`

- [ ] **Step 1: 写分层模型行为失败测试**

```rust
assert!(model.core_weight("final") > model.core_weight("nonfinal"));
assert!(model.core_weight("nonfinal") > 0.0);
assert!(model.core_weight("death") < 0.0);
assert!(model.cohort_offset(TrainingCohort::OpenQueue).is_none()); // 训练组少于 150 行
assert!(candidate_metrics.brier < baseline_metrics.brier);
```

另证明 `RoleCompetitive` 与 `RoleQuick` 使用同一 `RoleQueue` 偏移，`OpenCompetitive` 与
`OpenQuick` 使用同一 `OpenQueue` 偏移；再用相反数据证明 `RoleQueue` 和
`StadiumCompetitive` 可产生不同偏移，但共享同一职责核心。

- [ ] **Step 2: 运行测试确认模型模块不存在**

```powershell
cargo test --test career_mmr_hierarchical_model
```

Expected: FAIL。

- [ ] **Step 3: 定义不可变模型类型**

先删除 `performance_forward.rs` 自己定义的 `Role`，统一导入 `input::Role`。`ModelVariant` 保持
在表现模块中，避免在输入层引入模型概念。

```rust
pub struct FitConfig {
    pub lambda_core: f64,
    pub lambda_cohort: f64,
    pub learning_rate: f64,
    pub steps: usize,
    pub min_cohort_offset_rows: usize,
    pub final_multiplier: f64,
}

pub struct HierarchicalModel {
    pub role: Role,
    pub strategy: HealingTakenStrategy,
    pub variant: ModelVariant,
    pub feature_names: Vec<String>,
    pub core_weights: Vec<f64>,
    pub cohort_offsets: BTreeMap<TrainingCohort, Vec<f64>>,
    pub cohort_samples: BTreeMap<TrainingCohort, usize>,
}
```

- [ ] **Step 4: 实现带赛前胜率 offset 的逻辑损失**

```rust
pub fn predict_probability(model: &HierarchicalModel, row: &MatrixRow) -> f64 {
    let offset = logit(row.expected_win_rate.clamp(1e-6, 1.0 - 1e-6));
    let weights = model.effective_weights(training_cohort(row.mode_family));
    sigmoid(offset + dot(&weights, &row.feature_values()))
}
```

优化目标为加权交叉熵，加 `lambda_core * ||beta||²` 和 `lambda_cohort * ||delta_cohort||²`；样本权重为 `data_quality × season_balance_weight`。胜负主干的 `modeWeight` 不作为训练损失样本权重，避免把“对 MMR 的影响幅度”和“模型应学习多少数据”混为一谈。每步投影约束：

```text
final > 0
nonfinal > 0
final >= final_multiplier × nonfinal
death < 0
debt <= 0（启用时）
```

少于 150 行的训练组偏移强制为 0；各训练组有效权重也必须满足方向约束。竞技与快速配对
模式共享拟合参数，但稳健归一化仍优先使用各自实际模式的基线。

- [ ] **Step 5: 替换残差线性拟合并运行测试**

`performance_forward.rs` 保留特征向量和 metrics，删除线性残差 `fit/predict`，改用分层逻辑模型。候选仍比较 Core/RoleFull、五个最后一击倍率和五种 `healingTaken` 策略。
RoleFull 特征必须使用 Task 4 的职责派生值：重装加入 frontline conversion、减伤和异常承伤；
输出即时计算 damage conversion；支援加入治疗、伤害、survival 和异常承伤。原始伤害、治疗、
减伤仍保留为可消融候选，不能跳过简单模型比较。

```powershell
cargo test --test career_mmr_hierarchical_model
cargo test --test career_mmr_performance_forward
```

Expected: PASS；模型使用 logit offset，配对模式共享训练组参数，稀疏训练组偏移为 0，
方向和倍率约束生效。

- [ ] **Step 6: 提交**

```powershell
git add apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs apps/owstats-desktop/src-tauri/src/career_mmr/performance_model.rs apps/owstats-desktop/src-tauri/src/career_mmr/performance_forward.rs apps/owstats-desktop/src-tauri/tests/career_mmr_hierarchical_model.rs apps/owstats-desktop/src-tauri/tests/career_mmr_performance_forward.rs
git commit -m "feat: fit hierarchical constrained mmr performance models"
```

## Task 6：实现 15 格实际模式验证、逐账号留一和正式门槛

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_forward.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/performance_stability.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_performance_forward.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_performance_cli.rs`

- [ ] **Step 1: 写嵌套选择和 15 格失败测试**

构造训练期偏好复杂模型、未来期偏好简单模型的数据，断言外层验证不能使用完整数据选候选。再断言：

```rust
for role in [Role::Tank, Role::Damage, Role::Support] {
    for mode in [
        ModeFamily::RoleCompetitive,
        ModeFamily::RoleQuick,
        ModeFamily::OpenCompetitive,
        ModeFamily::OpenQuick,
        ModeFamily::StadiumCompetitive,
    ] {
        assert!(matrix.contains_key(&(role, mode)));
    }
}
assert!(!matrix.keys().any(|(_, mode)| *mode == ModeFamily::StadiumQuick));
```

再断言 `RoleCompetitive` 与 `RoleQuick` 的模型产物引用同一 `RoleQueue` 参数版本，
`OpenCompetitive` 与 `OpenQuick` 引用同一 `OpenQueue` 参数版本；四个实际模式的验证样本、
指标、拒绝原因和 `CellDecision` 均独立，不能用配对模式结果代替。

- [ ] **Step 2: 增加逐账号留一和格状态**

```rust
pub struct AccountLooReport {
    pub candidate_groups: usize,
    pub evaluated_groups: usize,
    pub coverage_limited_groups: usize,
    pub mean_brier_delta: Option<f64>,
    pub worst_brier_delta: Option<f64>,
}

pub enum CellDecision { Eligible, Shadow, OutcomeOnly }
```

覆盖不足记录 `account_coverage_limited`，不能伪装成通过。
现有逐赛季留一、逐英雄留一同步升级为“职责 × 实际模式”粒度，并确保每次留一都重新拟合
英雄基线、healing expectation 和模型权重；不得复用全量矩阵。测试分别构造只在被留出
赛季/英雄出现的极端值，证明训练参数不受其影响。

- [ ] **Step 3: 修正嵌套候选选择**

每个外层 forward fold 在训练区间内再建内部时间折，只用内部验证选择 variant、倍率、策略和正则强度。内部折不足时只允许最简单 Core，并记录 `selection_degraded=true`。

统一所有报告的 Brier delta 符号为：

```text
brier_delta = candidate_brier - baseline_brier
```

因此小于 0 才表示改善；修正当前 forward 与 bootstrap 使用相反符号的问题。

- [ ] **Step 4: 实现精确门槛**

```rust
if improved_folds < 4 { reject("improved_folds_below_4"); }
if bootstrap.brier_delta_ci95_high.unwrap_or(f64::INFINITY) >= 0.0 {
    reject("bootstrap_brier_ci_not_below_zero");
}
if candidate_ece > baseline_ece + 0.005 { reject("ece_worse_than_0_005"); }
if critical_direction_stability < 0.90 { reject("critical_direction_stability_below_90_percent"); }
if critical_ci_crosses_zero { reject("critical_coefficient_ci_crosses_zero"); }
if p95_single_feature_share > 0.35 { reject("p95_single_feature_contribution_above_35_percent"); }
if average_adjustment.abs() > 0.10 { reject("average_adjustment_above_0_10_mmr"); }
if upper_cap_rate > 0.05 || lower_cap_rate > 0.05 { reject("cap_hit_rate_above_5_percent"); }
```

职责核心少于 500 行直接 Shadow；训练组偏移少于 150 行收缩为 0，但每个实际模式格仍须
独立验证。资源债务启用时 CI 上界必须小于 0。
`average_adjustment`、cap rate 和 P95 使用 `TrainingRow.base_delta` 经过正式限幅函数后得到的
MMR 调整，不使用 `predicted_probability - expectedWinRate`。

- [ ] **Step 5: 增加阶段配置和进度事件**

```rust
pub enum TrainingStage { Screen100, Candidate1000, Final10000 }

pub struct ProgressEvent {
    pub stage: TrainingStage,
    pub completed: usize,
    pub total: usize,
    pub elapsed_ms: u128,
    pub eta_ms: Option<u128>,
}
```

每完成固定批次输出聚合进度；事件不得携带账号、比赛或英雄原始标识。

- [ ] **Step 6: 增加反事实测试**

固定击杀、助攻、死亡和目标贡献，仅提高伤害或治疗，表现调整不得持续上升；提高最后一击且其他条件不变时方向必须为正。

- [ ] **Step 7: 运行验证并提交**

```powershell
cargo test --test career_mmr_performance_forward
cargo test --test career_mmr_performance_cli
git add apps/owstats-desktop/src-tauri/src/career_mmr/performance_forward.rs apps/owstats-desktop/src-tauri/src/career_mmr/performance_stability.rs apps/owstats-desktop/src-tauri/tests/career_mmr_performance_forward.rs apps/owstats-desktop/src-tauri/tests/career_mmr_performance_cli.rs
git commit -m "feat: validate mmr performance by role mode and account"
```

Expected: PASS；15 格齐全，共享训练组但不共享验证结论，角斗快速不在矩阵、逐账号覆盖
不足明确记录。

## Task 7：生成不可变模型产物和隐私安全最终报告

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/training_report.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_training_report.rs`

- [ ] **Step 1: 写报告契约失败测试**

```rust
assert_eq!(report.dataset.account_datasets, 2);
assert_eq!(report.cells.len(), 15);
assert_eq!(report.stadium_quick.decision, CellDecision::OutcomeOnly);
assert!(report.cells.iter().all(|cell| !cell.rejected_reasons.is_empty() || cell.eligible));
assert_private_keys_absent(&serde_json::to_value(&report)?);
```

序列化文本不得包含测试 BattleTag、UUID、match id、数据库路径或 `raw_json`。

- [ ] **Step 2: 定义模型产物和每格报告**

```rust
#[derive(Serialize, Deserialize)]
pub struct PerformanceModelArtifact {
    pub schema_version: u32,
    pub algorithm: String,
    pub feature_version: String,
    pub seed: u64,
    pub roles: BTreeMap<Role, HierarchicalModelArtifact>,
    pub cells: Vec<CellArtifact>,
    pub normalization: NormalizationArtifact,
    pub caps: PerformanceCaps,
    pub config_hash: String,
}

pub struct CellReport {
    pub role: Role,
    pub mode: ModeFamily,
    pub training_cohort: TrainingCohort,
    pub shared_parameter_version: String,
    pub training_rows: usize,
    pub validation_rows: usize,
    pub seasons: usize,
    pub heroes: usize,
    pub decision: CellDecision,
    pub eligible: bool,
    pub rejected_reasons: Vec<String>,
    pub metrics: CellMetrics,
}
```

`config_hash` 只覆盖配置、权重和聚合样本桶，不哈希或输出账号/比赛 ID。

- [ ] **Step 3: 实现递归隐私检查**

拒绝键名包含：`token`、`customerToken`、`authorization`、`cookie`、`credential`、`battleTag`、`accountId`、`playerId`、`bnetId`、`matchId`、`rawJson`、`databasePath`。错误不回显值。

- [ ] **Step 4: 运行报告测试并提交**

```powershell
cargo test --test career_mmr_training_report
git add apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs apps/owstats-desktop/src-tauri/src/career_mmr/training_report.rs apps/owstats-desktop/src-tauri/tests/career_mmr_training_report.rs
git commit -m "feat: produce privacy-safe mmr training artifacts"
```

Expected: PASS；报告有 15 格和角斗快速单列，配对模式能追溯同一参数版本但结果分别展示，
不含私密标识。

## Task 8：实现表现限幅和只读影子历史重放

**Files:**

- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs`
- Create: `apps/owstats-desktop/src-tauri/src/career_mmr/shadow.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs`
- Create: `apps/owstats-desktop/src-tauri/tests/career_mmr_shadow.rs`

- [ ] **Step 1: 写限幅和不反转失败测试**

```rust
assert_eq!(bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::RoleCompetitive), 2.0);
assert_eq!(bounded_performance_adjustment(-20.0, 99.0, Role::Damage, ModeFamily::RoleCompetitive), 2.0);
assert_eq!(bounded_performance_adjustment(20.0, 99.0, Role::Support, ModeFamily::RoleCompetitive), 2.0);
assert_eq!(bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::RoleQuick), 1.0);
assert_eq!(bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::OpenQuick), 1.0);
assert_eq!(bounded_performance_adjustment(20.0, 99.0, Role::Damage, ModeFamily::StadiumQuick), 0.0);
assert!(20.0 + bounded_performance_adjustment(20.0, -99.0, Role::Damage, ModeFamily::RoleCompetitive) > 0.0);
assert!(-20.0 + bounded_performance_adjustment(-20.0, 99.0, Role::Damage, ModeFamily::RoleCompetitive) < 0.0);
```

- [ ] **Step 2: 实现纯限幅函数**

```rust
pub fn bounded_performance_adjustment(
    base_delta: f64,
    requested: f64,
    role: Role,
    mode: ModeFamily,
) -> f64 {
    if mode == ModeFamily::StadiumQuick || base_delta == 0.0 { return 0.0; }
    let absolute = match (role, mode.is_quick()) {
        (_, true) => 1.0,
        (Role::Support, false) => 2.0,
        _ => 3.0,
    };
    requested
        .clamp(-0.1 * base_delta.abs(), 0.1 * base_delta.abs())
        .clamp(-absolute, absolute)
}
```

调用方用正表现生成正 requested、负表现生成负 requested；10% 上限保证胜负方向不反转。

- [ ] **Step 3: 实现按预期胜率更新和不写数据库的影子重放**

先把 `rating.rs::update_rating` 的状态推进主体抽成
`update_rating_for_expected(state, input, expected, performance_adjustment)`；原 `update_rating` 仍
负责从双方强度求 expected，并传入 0 调用该函数，确保正式基线行为不变。

`simulate_shadow_replay(rows_for_account, artifact)` 使用账号自己的 `TrainingRow`：按
`(match_group, role)` 聚合英雄片段并计算 role share，按时间稳定排序；按账号自身三职责状态
重放；只对 Eligible 格计算表现；将 logit effect 按该格训练集 P95 归一到 `[-1,1]`；应用
限幅；返回聚合摘要。函数只接收内存投影，不再打开或写 SQLite。

```rust
pub struct ShadowAccountSummary {
    pub account_group: String,
    pub roles: Vec<ShadowRoleSummary>,
    pub cap_hits: BTreeMap<CellKey, CapHitSummary>,
}
```

写入最终报告时把 `account_group` 转换为“数据集1/数据集2”。

- [ ] **Step 4: 用源数据库摘要验证只读和状态隔离**

在多账号投影前后比较只读连接的 `total_changes()` 和 MMR 表聚合摘要，必须完全一致；随后
只向 shadow 传内存行。两个账号不得共享 `RoleRatingState`，多职责片段的 role share 之和
不得超过 1.0。

- [ ] **Step 5: 运行 shadow 测试并提交**

```powershell
cargo test --test career_mmr_shadow
cargo test --test career_mmr_rating
git add apps/owstats-desktop/src-tauri/src/career_mmr/mod.rs apps/owstats-desktop/src-tauri/src/career_mmr/rating.rs apps/owstats-desktop/src-tauri/src/career_mmr/shadow.rs apps/owstats-desktop/src-tauri/tests/career_mmr_shadow.rs apps/owstats-desktop/src-tauri/tests/career_mmr_rating.rs
git commit -m "feat: simulate bounded career mmr performance shadows"
```

Expected: PASS；角斗快速修正恒为 0，所有模式不越界，数据库未改变。

## Task 9：实现分阶段训练 CLI、进度和聚合输出

**Files:**

- Create: `apps/owstats-desktop/src-tauri/src/bin/train_career_mmr.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/bin/evaluate_career_mmr_performance.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/career_mmr_performance_cli.rs`

- [ ] **Step 1: 写 CLI 契约失败测试**

CLI 语法：

```text
train_career_mmr --app-data-dir <目录> --stage screen|candidate|final
                 --report <report.json> --artifact <model.json>
                 [--promote-from <上一阶段report.json>] [--seed N]
```

临时 app data 测试断言：screen=100、candidate=1,000、final=10,000；candidate/final 未传
`--promote-from` 时拒绝运行；新阶段只计算上一阶段标记为可晋级的候选；stderr 有阶段/完成
数/ETA；输出不含 fixture BattleTag、UUID、比赛 ID 或路径；缺目标账号库时退出非 0 且错误
不打印路径。

- [ ] **Step 2: 运行测试确认二进制不存在**

```powershell
cargo test --test career_mmr_performance_cli
```

Expected: FAIL，`CARGO_BIN_EXE_train_career_mmr` 未定义。

- [ ] **Step 3: 实现严格参数解析和串联**

未知/重复选项、缺值、无效阶段和不可写报告目录返回中文错误。路径只用于 I/O，不进入报告或 stderr。核心流程：

```rust
let shared = SharedAppPaths::from_app_data_dir(args.app_data_dir);
let sources = discover_training_sources(&shared)?;
let projection = build_multi_account_projection(&sources)?;
let promoted = load_promoted_candidates(args.promote_from.as_deref(), args.stage)?;
let evaluation = evaluate_stability(
    &projection.rows,
    stage.config(promoted),
    progress_to_stderr,
);
let artifact = build_model_artifact(&projection.audit, &evaluation, args.seed)?;
let mut report = build_training_report(&projection.audit, &evaluation, &artifact)?;
report.shadow = simulate_all_accounts(&projection.rows, &artifact)?;
write_json_atomic(args.artifact, &artifact)?;
write_json_atomic(args.report, &report)?;
```

CLI 不调用 `AccountContextManager::load`，避免账号恢复/迁移写入。写文件使用同目录临时文件
再 rename，失败不留下半份正式报告。固定 seed 默认 `0x5eed`。

- [ ] **Step 4: 保持旧单库评估器兼容**

`evaluate_career_mmr_performance` 继续接受一个只读 SQLite，用新模型生成单数据集研究报告；`account_loo` 明确标为覆盖不足，不作为多账号正式入口。

- [ ] **Step 5: 运行 CLI 测试并提交**

```powershell
cargo test --test career_mmr_performance_cli
cargo build --bin train_career_mmr
git add apps/owstats-desktop/src-tauri/src/bin/train_career_mmr.rs apps/owstats-desktop/src-tauri/src/bin/evaluate_career_mmr_performance.rs apps/owstats-desktop/src-tauri/tests/career_mmr_performance_cli.rs
git commit -m "feat: add staged multi-account mmr training cli"
```

Expected: PASS；screen fixture 完成，错误和进度不泄露路径或标识。

## Task 10：全量自动验证和真实数据分阶段训练

**Files:**

- Create after successful real run: `docs/reports/2026-07-18-career-mmr-training-report.md`
- Keep local only: app-data diagnostics `career-mmr-*.json`

- [ ] **Step 1: 运行后端定向测试**

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test career_mmr_inputs
cargo test --test career_mmr_training_projection
cargo test --test career_mmr_multi_account_training
cargo test --test career_mmr_performance_matrix
cargo test --test career_mmr_hierarchical_model
cargo test --test career_mmr_performance_forward
cargo test --test career_mmr_training_report
cargo test --test career_mmr_shadow
cargo test --test career_mmr_performance_cli
```

Expected: 全部 PASS。

- [ ] **Step 2: 运行完整后端验证**

```powershell
cargo fmt --check
cargo test --tests
cargo build --bins
```

Expected: 全部退出 0。

- [ ] **Step 3: 对真实账号执行只读 100 次筛选**

```powershell
cargo run --release --bin train_career_mmr -- --app-data-dir $env:APPDATA\local.owstats.desktop --stage screen --report $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-screen.json --artifact $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-screen-model.json
```

只报告聚合进度和失败组合。确认实际纳入账号、场次和动态赛季；若与 2026-07-18 基线不同，以新报告为准并解释差异。

- [ ] **Step 4: 只让 screen 入围候选执行 1,000 次**

```powershell
cargo run --release --bin train_career_mmr -- --app-data-dir $env:APPDATA\local.owstats.desktop --stage candidate --promote-from $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-screen.json --report $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-candidate.json --artifact $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-candidate-model.json
```

无候选通过 screen 时停止，不浪费后续计算，并保留拒绝原因。

- [ ] **Step 5: 只让 candidate 入围候选执行 10,000 次**

```powershell
cargo run --release --bin train_career_mmr -- --app-data-dir $env:APPDATA\local.owstats.desktop --stage final --promote-from $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-candidate.json --report $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-final.json --artifact $env:APPDATA\local.owstats.desktop\diagnostics\career-mmr-final-model.json
```

每 60 秒内至少提供一次有效进度更新。中断时保留上一阶段完整产物，不把部分 final 报告视为完成。

- [ ] **Step 6: 生成玩家可读最终报告**

从最高已完成阶段的聚合 JSON 生成 Markdown：有 final 用 final；无 final 但有 candidate 用
candidate；全部未晋级则用 screen。报告包含数据范围、权重和 CI、五种 healing 方案、
forward/season/hero/account/bootstrap、完整 15 格、配对模式共享参数但分开验证、角斗快速仅
胜负、每格状态/失败原因、
新旧 MMR 聚合影子差异和上线结论。不得写入账号、玩家、比赛标识、路径或原始 JSON。

- [ ] **Step 7: 运行前端、发布和差异验证**

```powershell
cd apps\owstats-desktop
npm test
npm run build
npm run tauri build
cd ..\..
git diff --check
git status --short
```

Expected: 全部退出 0；真实 JSON 模型和诊断报告留在 app data，不进入 Git。

- [ ] **Step 8: 提交最终聚合报告和文档**

```powershell
git add docs/reports/2026-07-18-career-mmr-training-report.md docs/superpowers/specs/2026-07-18-career-role-mmr-multi-account-training-design.md docs/superpowers/plans/2026-07-18-career-role-mmr-multi-account-training.md
git commit -m "docs: report multi-account career mmr training results"
```

提交前再次确认没有 raw JSON、账号值、玩家 ID、比赛 ID 或凭据。

## Task 11：用户验收门禁

**Files:** None

- [ ] **Step 1: 向用户交付最终训练结果**

报告实际命令、耗时、通过/失败矩阵、未验证项、是否接触真实隐私数据，以及正式数据库未被修改的证据。

- [ ] **Step 2: 停止，不自动激活**

即使某些格满足 Eligible，也不把候选模型写成 active，不重写正式 `career_mmr_events`。等待用户查看最终报告并明确确认后，再单独设计和实施“模型激活、迁移、回滚与 UI 状态”计划。
