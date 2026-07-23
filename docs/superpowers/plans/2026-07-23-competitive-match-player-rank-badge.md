# 竞技对局玩家段位徽标 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在预设竞技和开放竞技的两种对局详情中，于玩家名称右侧显示标准竞技段位图标与分段数字，并在段位缺失或未知时保持为空。

**Architecture:** 后端在现有普通玩家 DTO 构造点复用段位名称规范化规则，直接从玩家 `rankInfo` 生成 `rankKey` 与 `rankLabel`。前端新增“严格匹配、失败返回空”的标准段位资源解析函数，并由两种详情渲染器显式传入 `competitive | arena | none` 段位变体，以选择标准资源、角斗资源或完全不展示。

**Tech Stack:** Rust、serde_json、React 18、TypeScript、Node `assert`、Tauri 2

---

## 文件结构

- 修改 `apps/owstats-desktop/src-tauri/src/queries/arena_detail.rs`：集中维护标准/角斗共用的段位字段解析，并在普通玩家 DTO 构造时填充段位。
- 修改 `apps/owstats-desktop/src/shared/assets.ts`：增加不会回退到 `unranked` 的严格标准段位资源解析函数。
- 修改 `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`：根据显式段位变体选择标准资源、角斗资源或不展示。
- 修改 `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`：仅为预设竞技、开放竞技和角斗详情传入对应段位变体。
- 修改 `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`：独立详情的玩家表与玩家卡传递对局类型并选择资源。
- 修改 `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`：验证源码契约和严格空值行为。
- 修改 `apps/owstats-desktop/scripts/matchWorkbench.test.ts`：用合成 DTO 验证预设、开放、角斗和无段位的最终 HTML。

### Task 1: 后端普通竞技段位映射

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/queries/arena_detail.rs`

- [ ] **Step 1: 写入失败测试**

在 `arena_detail.rs` 尾部增加单元测试模块，以合成玩家对象验证 camelCase、snake_case 和未知段位：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_player_rows_map_competitive_rank_info() {
        let players = json!([{
            "name": "合成玩家",
            "heroGuid": "synthetic-hero",
            "rankInfo": { "rankName": "Diamond", "rankSubTier": 4 }
        }]);
        let rows = player_rows(Some(&players), "teammate", &HashMap::new());
        assert_eq!(rows[0].rank_key.as_deref(), Some("diamond"));
        assert_eq!(rows[0].rank_label.as_deref(), Some("钻石 4"));
    }

    #[test]
    fn normal_player_rows_support_snake_case_and_leave_unknown_rank_empty() {
        let known = json!([{
            "name": "合成玩家",
            "heroGuid": "synthetic-hero",
            "rankInfo": { "rank_name": "Master", "rank_sub_tier": 2 }
        }]);
        let unknown = json!([{
            "name": "合成玩家",
            "heroGuid": "synthetic-hero",
            "rankInfo": { "rankName": "UnknownTier", "rankSubTier": 1 }
        }]);
        let known_rows = player_rows(Some(&known), "enemy", &HashMap::new());
        let unknown_rows = player_rows(Some(&unknown), "enemy", &HashMap::new());
        assert_eq!(known_rows[0].rank_key.as_deref(), Some("master"));
        assert_eq!(known_rows[0].rank_label.as_deref(), Some("大师 2"));
        assert_eq!(unknown_rows[0].rank_key, None);
        assert_eq!(unknown_rows[0].rank_label, None);
    }
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test normal_player_rows_
```

Expected: 两个新增测试失败，实际 `rank_key` 与 `rank_label` 为 `None`。

- [ ] **Step 3: 写入最小后端实现**

将 `player_rows` 的固定空值替换为现有解析函数：

```rust
rank_label: competitive_rank_label(item),
rank_key: competitive_rank_key(item),
```

把当前 `arena_rank_label` 与 `arena_rank_key` 的解析主体提取为普通和角斗均可调用的函数：

```rust
pub(super) fn competitive_rank_label(item: &Value) -> Option<String> {
    let rank = item.get("rankInfo")?;
    let raw_name = string_field(rank, "rankName")
        .or_else(|| string_field(rank, "rank_name"))?;
    let name = rank_name_label(&raw_name)?;
    let sub_tier = optional_i64(rank, "rankSubTier")
        .or_else(|| optional_i64(rank, "rank_sub_tier"));
    Some(match sub_tier {
        Some(value) if value > 0 => format!("{name} {value}"),
        _ => name.to_string(),
    })
}

pub(super) fn competitive_rank_key(item: &Value) -> Option<String> {
    let rank = item.get("rankInfo")?;
    let raw_name = string_field(rank, "rankName")
        .or_else(|| string_field(rank, "rank_name"))?;
    Some(normalize_rank_key(&raw_name)?.to_string())
}
```

让 `arena_rank_label` 与 `arena_rank_key` 委托给这两个函数，保持现有角斗行为：

```rust
pub(super) fn arena_rank_label(item: &Value) -> Option<String> {
    competitive_rank_label(item)
}

pub(super) fn arena_rank_key(item: &Value) -> Option<String> {
    competitive_rank_key(item)
}
```

- [ ] **Step 4: 运行后端定向测试**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test normal_player_rows_
```

Expected: 新增测试全部通过。

### Task 2: 严格标准段位资源解析

**Files:**
- Modify: `apps/owstats-desktop/src/shared/assets.ts`
- Modify: `apps/owstats-desktop/scripts/arenaDetailFormat.test.ts`

- [ ] **Step 1: 写入失败测试**

在测试中导入并断言严格资源解析行为：

```ts
import { competitiveRankAssetUrl } from "../src/shared/assets";

assert.equal(competitiveRankAssetUrl("Diamond"), "/assets/ranks/diamond.png");
assert.equal(competitiveRankAssetUrl(null), null);
assert.equal(competitiveRankAssetUrl("UnknownTier"), null);
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaDetailFormat.test.ts
```

Expected: FAIL，提示 `competitiveRankAssetUrl` 尚未导出。

- [ ] **Step 3: 写入最小前端资源实现**

在 `rankAssetUrl` 附近增加严格解析函数，不改变其他页面依赖的既有回退行为：

```ts
export function competitiveRankAssetUrl(rankKey: string | null | undefined): string | null {
  const key = normalizeAssetKey(rankKey);
  if (!key) return null;
  const file = rankFiles.get(key);
  return file ? `${rankAssetRoot}/${file}` : null;
}
```

- [ ] **Step 4: 运行测试并确认通过**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/arenaDetailFormat.test.ts
```

Expected: PASS。

### Task 3: 紧凑详情选择正确段位资源

**Files:**
- Modify: `apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx`
- Modify: `apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: 写入失败渲染测试**

在 `matchWorkbench.test.ts` 中构造带标准段位的预设竞技与开放竞技 DTO，并检查标准资源和分段数字：

```ts
const rankedPlayer = {
  ...anonymousPlayer,
  rankLabel: "钻石 4",
  rankKey: "diamond",
};

for (const [gameMode, modeLabel] of [
  ["SportPreset", "预设竞技"],
  ["SportOpen", "开放竞技"],
] as const) {
  const markup = renderToStaticMarkup(
    createElement(CompactMatchDetail, {
      detail: {
        ...normalDetail,
        matchId: `synthetic-${gameMode}`,
        gameMode,
        modeLabel,
        teammates: [rankedPlayer],
      },
      loading: false,
      error: null,
      matchId: `synthetic-${gameMode}`,
      ...navigationProps,
    }),
  );
  assert.match(markup, /\/assets\/ranks\/diamond\.png/);
  assert.match(markup, />4<\/small>/);
}
```

同时增加无段位不渲染标准或未定级资源的断言：

```ts
assert.doesNotMatch(normalMarkup, /\/assets\/ranks\/unranked\.png/);
```

再用带有 `rankedPlayer` 的快速模式 DTO 断言不渲染任何段位资源：

```ts
assert.doesNotMatch(quickMarkup, /\/assets\/ranks\//);
assert.doesNotMatch(quickMarkup, /\/assets\/stadium\//);
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchWorkbench.test.ts
```

Expected: FAIL，普通竞技仍调用角斗资源解析，HTML 中没有标准段位图标。

- [ ] **Step 3: 写入最小紧凑详情实现**

给 `MatchTeamTableProps` 增加：

```ts
rankVariant?: "competitive" | "arena";
```

组件默认不传 `rankVariant`，并在玩家行传递该值。玩家行选择资源：

```ts
const rankIcon =
  rankVariant === "arena"
    ? arenaRankAssetUrl(player.rankKey)
    : rankVariant === "competitive"
      ? competitiveRankAssetUrl(player.rankKey)
      : null;
```

`CompactMatchDetail` 根据普通对局模式生成变体：

```ts
const rankVariant =
  detail.gameMode === "SportPreset" || detail.gameMode === "SportOpen"
    ? "competitive"
    : undefined;
```

两处普通详情调用传入 `rankVariant={rankVariant}`；四处角斗详情调用显式传入：

```tsx
<MatchTeamTable
  rankVariant="arena"
  title="队友数据"
  tone="team"
  players={selected.teammates}
  ...
/>
```

- [ ] **Step 4: 运行紧凑详情测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchWorkbench.test.ts
```

Expected: 预设、开放、角斗与空段位断言全部通过。

### Task 4: 独立详情选择正确段位资源

**Files:**
- Modify: `apps/owstats-desktop/src/pages/MatchDetailPage.tsx`
- Modify: `apps/owstats-desktop/scripts/matchWorkbench.test.ts`

- [ ] **Step 1: 写入失败渲染测试**

用同一 `rankedPlayer` 渲染 `MatchDetailReport`，分别验证 `SportPreset` 和 `SportOpen`：

```ts
const standaloneRankedMarkup = renderToStaticMarkup(
  createElement(MatchDetailReport, {
    detail: {
      ...normalDetail,
      matchId: "synthetic-ranked-standalone",
      gameMode: "SportPreset",
      modeLabel: "预设竞技",
      teammates: [rankedPlayer],
    },
    loading: false,
    error: null,
    matchId: "synthetic-ranked-standalone",
    favoriteController,
  }),
);
assert.match(standaloneRankedMarkup, /\/assets\/ranks\/diamond\.png/);
assert.match(standaloneRankedMarkup, />4<\/span>/);
```

保留并加强角斗详情已有断言，确认其仍包含角斗资源路径且不包含标准段位路径。

- [ ] **Step 2: 运行测试并确认按预期失败**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchWorkbench.test.ts
```

Expected: FAIL，独立普通详情仍调用角斗资源解析。

- [ ] **Step 3: 写入最小独立详情实现**

给 `PlayerTable` 和 `PlayerCard` 增加 `rankVariant?: "competitive" | "arena"` 参数。由详情模式显式生成段位变体，并选择资源：

```ts
const rankIcon =
  rankVariant === "arena"
    ? arenaRankAssetUrl(player.rankKey)
    : rankVariant === "competitive"
      ? competitiveRankAssetUrl(player.rankKey)
      : null;
```

详情级变体遵循：

```ts
const rankVariant = detail.arenaDetail
  ? "arena"
  : detail.gameMode === "SportPreset" || detail.gameMode === "SportOpen"
    ? "competitive"
    : undefined;
```

所有 `PlayerTable` 调用传入该 `rankVariant`，避免从段位键猜测对局类型，并确保快速模式保持无徽标。

- [ ] **Step 4: 运行详情渲染测试**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/matchWorkbench.test.ts
```

Expected: 两种详情、三种竞技模式和空段位断言全部通过。

### Task 5: 回归验证与工作树检查

**Files:**
- Verify only: all files listed above

- [ ] **Step 1: 格式化 Rust**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt
cargo fmt --check
```

Expected: `cargo fmt --check` 退出码为 0。

- [ ] **Step 2: 运行完整后端测试**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --tests
```

Expected: 全部通过。

- [ ] **Step 3: 运行完整前端测试**

Run:

```powershell
cd apps\owstats-desktop
npm test
```

Expected: 全部通过。

- [ ] **Step 4: 构建前端**

Run:

```powershell
cd apps\owstats-desktop
npm run build
```

Expected: TypeScript 与 Vite 构建成功，资源路径验证通过。

- [ ] **Step 5: 检查补丁和工作树**

Run:

```powershell
git diff --check
git status --short
git diff -- apps/owstats-desktop/src-tauri/src/queries/arena_detail.rs apps/owstats-desktop/src/shared/assets.ts apps/owstats-desktop/src/features/matches/MatchTeamTable.tsx apps/owstats-desktop/src/features/matches/CompactMatchDetail.tsx apps/owstats-desktop/src/pages/MatchDetailPage.tsx apps/owstats-desktop/scripts/arenaDetailFormat.test.ts apps/owstats-desktop/scripts/matchWorkbench.test.ts docs/superpowers/specs/2026-07-23-competitive-match-player-rank-badge-design.md docs/superpowers/plans/2026-07-23-competitive-match-player-rank-badge.md
```

Expected: `git diff --check` 无输出；状态中只新增或修改本计划列出的任务文件以及进入任务前已存在的用户改动。

> 项目协作说明禁止未经明确要求暂存或提交，因此本计划不包含 `git add` 或 `git commit`。
