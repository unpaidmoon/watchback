# Match Party Grouping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在普通与角斗对局详情中按 `friendBnetIds` 稳定聚合同队玩家，以每队一根连续阵营色竖条展示，并删除无入口的独立详情页。

**Architecture:** Rust 查询层从原始玩家对象计算同阵营无向连通分量，只向 `PlayerRow` 暴露匿名局部 `partyGroup`，同时输出稳定分组顺序。React 的 `MatchTeamTable` 只消费该投影，将连续同组玩家包成语义分组；旧 `/matches/:matchId` 路由改为现有 `/matches?matchId=` 工作台深链。

**Tech Stack:** Rust、serde_json、Tauri 2、React 18、TypeScript、CSS、Node/tsx 测试。

**Repository constraint:** 当前工作树包含用户修改。执行时不得清理、覆盖、暂存或提交未授权文件；本计划以状态检查点替代提交步骤。

---

## File map

- Create `apps/owstats-desktop/src-tauri/src/queries/party_groups.rs`: 解析 `friendBnetIds`、求连通分量、分配匿名组号并稳定重排。
- Modify `apps/owstats-desktop/src-tauri/src/queries/mod.rs`: 注册并导入分组模块。
- Modify `apps/owstats-desktop/src-tauri/src/models.rs`: 为 `PlayerRow` 增加 `party_group: Option<usize>`。
- Modify `apps/owstats-desktop/src-tauri/src/queries/arena_detail.rs`: 普通与角斗玩家投影统一调用分组函数。
- Modify `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`: 补齐非详情用途 `PlayerRow` 的空组号。
- Modify `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`: 补齐测试构造器字段。
- Create `apps/owstats-desktop/src/features/matches/partyGroups.ts`: 将已排序玩家切成小队组与单排行。
- Create `apps/owstats-desktop/scripts/matchPartyGrouping.test.tsx`: 验证前端分组与渲染契约。
- Modify `apps/owstats-desktop/src/shared/types.ts`: 增加 `partyGroup`。
- Modify `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`: 渲染连续小队容器和单排玩家。
- Modify `apps/owstats-desktop/src/styles/features/matches.css`: 删除整表/逐行阵营条，新增每队一条连续竖条。
- Delete `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`: 移除无入口独立详情页。
- Modify `apps/owstats-desktop/src/App.tsx`: 删除独立详情 import 和路由。
- Modify `apps/owstats-desktop/src-tauri/src/review/personal.rs`: 将比赛证据深链改为 `/matches?matchId=`。
- Modify related scripts that still read or render `MatchDetailPage.tsx`: 删除旧页面专属断言，保留工作台详情覆盖。
- Modify `apps/owstats-desktop/package.json`: 将新测试加入 `test:components`。

### Task 1: Rust 小队分组核心

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/queries/party_groups.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/mod.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Test: `apps/owstats-desktop/src-tauri/src/queries/party_groups.rs`

- [ ] **Step 1: 写失败测试，固定无向传递关系、稳定顺序与异常回退**

在新模块底部先写单元测试，输入只使用合成 ID：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(id: &str) -> PlayerRow {
        PlayerRow {
            side: "teammate".into(),
            name: id.into(),
            bnet_id: id.into(),
            rank_label: None,
            rank_key: None,
            hero_guid: String::new(),
            hero_name: "测试英雄".into(),
            kills: 0,
            assists: 0,
            deaths: 0,
            damage: 0,
            healing: 0,
            mitigation: 0,
            final_hits: 0,
            target_time_sec: 0,
            healing_taken: 0,
            damage_taken: 0,
            worth: None,
            end_worth: None,
            worth_max: false,
            mod_guids: Vec::new(),
            trait_guids: Vec::new(),
            has_source_max_flags: false,
            kill_max: false,
            hero_damage_max: false,
            cure_max: false,
            resist_damage_max: false,
            perks: Vec::new(),
            hero_stats: Vec::new(),
            party_group: None,
        }
    }

    #[test]
    fn groups_unilateral_transitive_links_and_preserves_first_seen_order() {
        let source = vec![
            json!({"bnetId":"solo-a","friendBnetIds":[]}),
            json!({"bnetId":"party-a","friendBnetIds":["party-b"]}),
            json!({"bnetId":"solo-b","friendBnetIds":["enemy-x", "missing-x"]}),
            json!({"bnetId":"party-c","friendBnetIds":[]}),
            json!({"bnetId":"party-b","friendBnetIds":["party-c", "party-c"]}),
        ];
        let rows = source.iter().map(|item| row(item["bnetId"].as_str().unwrap())).collect();

        let grouped = assign_party_groups(&source, rows);

        assert_eq!(
            grouped.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
            vec!["solo-a", "party-a", "party-c", "party-b", "solo-b"]
        );
        assert_eq!(
            grouped.iter().map(|row| row.party_group).collect::<Vec<_>>(),
            vec![None, Some(0), Some(0), Some(0), None]
        );
    }

    #[test]
    fn keeps_multiple_parties_separate_and_solos_unmarked() {
        let source = vec![
            json!({"bnetId":"a","friendBnetIds":["b"]}),
            json!({"bnetId":"c","friendBnetIds":["d"]}),
            json!({"bnetId":"solo"}),
            json!({"bnetId":"b","friendBnetIds":[]}),
            json!({"bnetId":"d","friendBnetIds":[]}),
        ];
        let rows = source.iter().map(|item| row(item["bnetId"].as_str().unwrap())).collect();
        let grouped = assign_party_groups(&source, rows);
        assert_eq!(
            grouped.iter().map(|row| (row.name.as_str(), row.party_group)).collect::<Vec<_>>(),
            vec![("a", Some(0)), ("b", Some(0)), ("c", Some(1)), ("d", Some(1)), ("solo", None)]
        );
    }
}
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test queries::party_groups::tests --lib
```

Expected: FAIL，原因是 `party_groups` 模块、`party_group` 字段或 `assign_party_groups` 尚不存在。

- [ ] **Step 3: 添加 DTO 字段与最小分组实现**

在 `PlayerRow` 增加：

```rust
#[serde(default)]
pub party_group: Option<usize>,
```

在 `queries/mod.rs` 注册：

```rust
mod party_groups;
use party_groups::*;
```

在 `party_groups.rs` 实现：

```rust
use super::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub(super) fn assign_party_groups(source: &[Value], rows: Vec<PlayerRow>) -> Vec<PlayerRow> {
    let mut first_index = HashMap::new();
    for (index, item) in source.iter().enumerate() {
        if let Some(id) = string_field(item, "bnetId").filter(|id| !id.is_empty()) {
            first_index.entry(id).or_insert(index);
        }
    }

    let mut edges = vec![HashSet::<usize>::new(); source.len()];
    for (index, item) in source.iter().enumerate() {
        let Some(friends) = item.get("friendBnetIds").and_then(Value::as_array) else {
            continue;
        };
        for friend in friends.iter().filter_map(value_to_string) {
            let Some(&other) = first_index.get(&friend) else { continue };
            if other == index { continue; }
            edges[index].insert(other);
            edges[other].insert(index);
        }
    }

    let mut component_for = vec![None; source.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for start in 0..source.len() {
        if component_for[start].is_some() || edges[start].is_empty() { continue; }
        let component_id = components.len();
        let mut queue = VecDeque::from([start]);
        let mut members = Vec::new();
        component_for[start] = Some(component_id);
        while let Some(index) = queue.pop_front() {
            members.push(index);
            for &next in &edges[index] {
                if component_for[next].is_none() {
                    component_for[next] = Some(component_id);
                    queue.push_back(next);
                }
            }
        }
        members.sort_unstable();
        components.push(members);
    }

    let mut rows = rows.into_iter().map(Some).collect::<Vec<_>>();
    let mut emitted = HashSet::new();
    let mut result = Vec::with_capacity(rows.len());
    for index in 0..rows.len() {
        if rows[index].is_none() { continue; }
        if let Some(component_id) = component_for.get(index).copied().flatten() {
            if emitted.insert(component_id) {
                for &member in &components[component_id] {
                    if let Some(mut row) = rows.get_mut(member).and_then(Option::take) {
                        row.party_group = Some(component_id);
                        result.push(row);
                    }
                }
            }
        } else if let Some(row) = rows[index].take() {
            result.push(row);
        }
    }
    result.extend(rows.into_iter().flatten());
    result
}

fn party_id(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
```

同时将循环中的 `filter_map(value_to_string)` 写为 `filter_map(party_id)`。该本地转换只接受非空字符串与数字，不记录原值。

- [ ] **Step 4: 运行测试并确认 GREEN**

Run:

```powershell
cargo fmt
cargo test queries::party_groups::tests --lib
```

Expected: 两个分组测试 PASS。

- [ ] **Step 5: 检查工作树，不提交**

Run:

```powershell
git status --short
git diff --check
```

Expected: 只有既有用户修改、本任务规范/计划及 Task 1 预期文件；不执行 `git add` 或 `git commit`。

### Task 2: 普通与角斗详情接入匿名分组

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/queries/arena_detail.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/queries/normal_detail.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/import_baseline.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/ai_roast_snapshot.rs`

- [ ] **Step 1: 写失败的详情投影测试**

在 `import_baseline.rs` 的合成详情 fixture 中加入一场普通对局和一个角斗回合，使用 `solo-a, party-a, solo-b, party-b` 的源顺序，并令 `party-a.friendBnetIds = ["party-b"]`。断言普通、角斗总览和对应回合均返回：

```rust
assert_eq!(
    detail.teammates.iter().map(|p| (p.name.as_str(), p.party_group)).collect::<Vec<_>>(),
    vec![
        ("单排甲", None),
        ("组队甲", Some(0)),
        ("组队乙", Some(0)),
        ("单排乙", None),
    ]
);
```

敌方 fixture 再加入一个二人组，断言其组号在敌方独立从 `Some(0)` 开始，证明不跨阵营串组。

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test import_baseline party_groups -- --nocapture
```

Expected: FAIL，实际顺序仍是源顺序或 `party_group` 均为 `None`。

- [ ] **Step 3: 让所有玩家构造器先显式回退为空组**

在 `normal_detail.rs` 的奖项专用 `PlayerRow`、`arena_detail.rs` 的普通/角斗 `PlayerRow`、`ai_roast_snapshot.rs` 测试构造器中加入：

```rust
party_group: None,
```

- [ ] **Step 4: 在原始玩家数组与投影行仍一一对应时调用分组**

把 `player_rows` 和 `arena_player_rows` 的收尾改为先取得 `items` 与 `rows`，再调用：

```rust
assign_party_groups(items, rows)
```

`arena_summary_player_rows` 必须先按未重排的 `rows.iter_mut().zip(items)` 写入跨回合承伤/受疗聚合，再调用 `assign_party_groups(items, rows)`；不得先排序再 `zip`，否则统计会写到错误玩家。

- [ ] **Step 5: 运行定向后端测试**

Run:

```powershell
cargo fmt
cargo test --test import_baseline party_groups -- --nocapture
cargo test queries::party_groups::tests --lib
```

Expected: PASS；输出不得含合成 ID 之外的玩家数据。

### Task 3: 前端连续小队容器与竖条

**Files:**
- Create: `apps/owstats-desktop/src/features/matches/partyGroups.ts`
- Create: `apps/owstats-desktop/scripts/matchPartyGrouping.test.tsx`
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`
- Modify: `apps/owstats-desktop/src/styles/features/matches.css`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: 写失败的纯函数和组件结构测试**

`partyGroups.ts` 预期导出：

```ts
export type PlayerPartyBlock = {
  partyGroup: number | null;
  players: PlayerRow[];
};

export function playerPartyBlocks(players: PlayerRow[]): PlayerPartyBlock[];
```

测试输入依次为 `partyGroup: 0, 0, null, 1, 1`，断言输出为五名玩家组成的三个 block，并渲染 `MatchTeamTable` 后断言：

```ts
assert.equal(findAll(tree, (node) => hasClass(node, "matchTeamPartyGroup")).length, 2);
assert.equal(findAll(tree, (node) => hasClass(node, "matchTeamPartyRail")).length, 2);
assert.equal(findAll(tree, (node) => hasClass(node, "matchTeamSoloPlayer")).length, 1);
```

同时用源码/CSS 契约断言：

```ts
assert.doesNotMatch(tableSource, /matchTeamTableMarker/);
assert.match(styleSource, /\.matchTeamPartyRail\s*\{[^}]*background:\s*var\(--data-secondary\)/s);
assert.match(styleSource, /\.matchTeamTable\.enemy \.matchTeamPartyRail\s*\{[^}]*background:\s*var\(--status-danger\)/s);
assert.doesNotMatch(styleSource, /\.matchTeamTable \.phase6CompactPlayerRow[\s\S]*border-left:\s*3px/s);
```

- [ ] **Step 2: 注册测试并确认 RED**

在 `test:components` 中追加：

```json
"tsx scripts/matchPartyGrouping.test.tsx"
```

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchPartyGrouping.test.tsx
```

Expected: FAIL，原因是 helper、DTO 字段或小队 DOM 尚不存在。

- [ ] **Step 3: 实现连续 block helper**

```ts
import type { PlayerRow } from "../../shared/types";

export type PlayerPartyBlock = {
  partyGroup: number | null;
  players: PlayerRow[];
};

export function playerPartyBlocks(players: PlayerRow[]): PlayerPartyBlock[] {
  const blocks: PlayerPartyBlock[] = [];
  for (const player of players) {
    const partyGroup = player.partyGroup ?? null;
    const previous = blocks.at(-1);
    if (partyGroup !== null && previous?.partyGroup === partyGroup) {
      previous.players.push(player);
    } else {
      blocks.push({ partyGroup, players: [player] });
    }
  }
  return blocks;
}
```

在 TypeScript `PlayerRow` 增加：

```ts
partyGroup: number | null;
```

更新所有合成 `PlayerRow` fixture，显式使用 `partyGroup: null`，小队测试 fixture 使用具体数字。

- [ ] **Step 4: 重构 `MatchTeamTable` 渲染层**

删除表级：

```tsx
<span className={`matchTeamTableMarker ${tone}`} aria-hidden="true" />
```

将 `players.map` 改为 `playerPartyBlocks(players).map`。非空组渲染：

```tsx
<div
  className="matchTeamPartyGroup"
  aria-label={`${block.players.length} 人小队`}
  key={`party-${block.partyGroup}`}
>
  <span className="matchTeamPartyRail" aria-hidden="true" />
  {renderPlayers(block.players)}
</div>
```

单排用：

```tsx
<div className="matchTeamSoloPlayer">{renderPlayers(block.players)}</div>
```

把现有单行 JSX 提取为局部 `renderPlayer`，继续使用整张表中的原始绝对 index 生成 `compactPlayerKey` 和高亮 key，不能因 block 内 index 重置而改变展开状态或 React key。

- [ ] **Step 5: 实现方案 A 样式**

删除 `.matchTeamTableMarker`、`.matchTeamTableMarker.enemy` 和玩家行阵营色 `border-left`。新增：

```css
.matchTeamPartyGroup {
  position: relative;
  display: grid;
  gap: var(--space-2);
}

.matchTeamPartyRail {
  position: absolute;
  left: calc(var(--space-2) * -1);
  top: 1px;
  bottom: 1px;
  width: 4px;
  border-radius: 999px;
  background: var(--data-secondary);
}

.matchTeamTable.enemy .matchTeamPartyRail {
  background: var(--status-danger);
}

.matchTeamPartyGroup + .matchTeamPartyGroup,
.matchTeamPartyGroup + .matchTeamSoloPlayer,
.matchTeamSoloPlayer + .matchTeamPartyGroup {
  margin-top: var(--space-2);
}
```

玩家行保留 `border: 1px solid var(--border-subtle)` 中性边框。竖条放在包含展开内容的 block 外层，使展开区域仍属于同一连续竖条。

- [ ] **Step 6: 运行前端定向测试**

Run:

```powershell
npx tsx scripts/matchPartyGrouping.test.tsx
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/arenaDetailFormat.test.ts
npx tsx scripts/arenaPlayerRoundSummary.test.tsx
```

Expected: PASS；如旧契约仍要求表级/逐行竖条，只更新为“小队级竖条”契约，不削弱其他详情断言。

### Task 4: 删除独立详情页并修复深链

**Files:**
- Delete: `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
- Modify: `apps/owstats-desktop/src/App.tsx`
- Modify: `apps/owstats-desktop/src-tauri/src/review/personal.rs`
- Modify: `apps/owstats-desktop/src/shared/qaFeedback.ts`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`
- Modify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`
- Modify: `apps/owstats-desktop/scripts/matchFavoritesUi.test.tsx`
- Modify: `apps/owstats-desktop/scripts/matchesPageCache.test.ts`
- Modify: `apps/owstats-desktop/scripts/aiMatchRoastUi.test.tsx`
- Modify: `apps/owstats-desktop/scripts/phase6QaFeedback.test.ts`
- Test: `apps/owstats-desktop/src-tauri/tests/phase8_command_center.rs` or existing review evidence test location

- [ ] **Step 1: 先写失败的路由清理与证据深链测试**

前端契约增加：

```ts
assert.doesNotMatch(appSource, /MatchDetailPage/);
assert.doesNotMatch(appSource, /path="\/matches\/:matchId"/);
assert.equal(existsSync(new URL("../src/pages/MatchDetailPage.tsx", import.meta.url)), false);
```

Rust 复盘证据测试断言：

```rust
assert_eq!(match_evidence(&row).route, "/matches?matchId=synthetic-match");
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchWorkbench.test.ts
cd src-tauri
cargo test match_evidence --lib --tests
```

Expected: FAIL，旧路由与旧证据链接仍存在。

- [ ] **Step 3: 删除页面、路由和旧 QA 映射**

从 `App.tsx` 删除 `MatchDetailPage` import 与：

```tsx
<Route path="/matches/:matchId" element={<MatchDetailPage />} />
```

删除 `MatchDetailPage.tsx`。删除 `qaFeedback.ts` 中只匹配 `/matches/` 子路径的详情标签；`/matches` 继续使用工作台标签。

- [ ] **Step 4: 将证据链接改为工作台已支持的查询参数**

在 `review/personal.rs`：

```rust
route: format!("/matches?matchId={}", row.match_id),
```

保持 `match_id` DTO 字段不变，不新增前端路由。

- [ ] **Step 5: 清理旧页面测试引用**

从列出的脚本中删除 `readFileSync("../src/pages/MatchDetailPage.tsx")`、`MatchDetailReport` import/render 和独立页面专属断言。相同能力必须继续由以下组件测试覆盖：

- `CompactMatchDetail`
- `MatchDetailPane`
- `MatchDetailHeader`
- `MatchTeamTable`
- `AiMatchRoast`
- `MatchFavoriteEditor`

不得删除工作台缓存、收藏持久化、角斗回合和 AI 锐评本身的断言。

- [ ] **Step 6: 运行路由与受影响组件测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchWorkbench.test.ts
npx tsx scripts/matchFavoritesUi.test.tsx
npx tsx scripts/aiMatchRoastUi.test.tsx
npx tsx scripts/arenaDetailFormat.test.ts
npx tsx scripts/matchesPageCache.test.ts
npx tsx scripts/phase6QaFeedback.test.ts
cd src-tauri
cargo test match_evidence --lib --tests
```

Expected: PASS，且 `rg -n "MatchDetailPage|/matches/:matchId" apps/owstats-desktop/src apps/owstats-desktop/scripts` 无运行时代码或测试命中。

### Task 5: 全量验证与隐私边界检查

**Files:**
- Verify all files changed by Tasks 1–4

- [ ] **Step 1: 后端格式与测试**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: PASS。

- [ ] **Step 2: 前端测试与构建**

Run:

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: PASS；Vite 产物验证无绝对开发机路径。

- [ ] **Step 3: 检查目标语义和敏感字段扩散**

Run:

```powershell
rg -n "friendBnetIds" apps/owstats-desktop/src apps/owstats-desktop/src-tauri/src/models.rs
rg -n "MatchDetailPage|/matches/:matchId|matchTeamTableMarker" apps/owstats-desktop/src apps/owstats-desktop/scripts
rg -n "D:\\\\Codex|/@fs/|customerToken|authorization|cookie|credential" apps/owstats-desktop/src apps/owstats-desktop/src-tauri/src/queries/party_groups.rs
```

Expected:

- 前端与 `PlayerRow` DTO 不包含 `friendBnetIds`。
- 旧独立详情页、旧路由和旧表级竖条无命中。
- 新代码没有绝对路径或凭据字段。

- [ ] **Step 4: 最终差异检查**

Run:

```powershell
cd D:\Codex\OW_data\owstats-local
git diff --check
git status --short
git diff -- apps/owstats-desktop/src-tauri/src/queries apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src apps/owstats-desktop/scripts apps/owstats-desktop/package.json docs/superpowers/specs/2026-07-23-match-party-grouping-design.md docs/superpowers/plans/2026-07-23-match-party-grouping.md
```

Expected: 无空白错误；仅报告并保留用户既有修改与本任务预期文件。不得暂存或提交。
