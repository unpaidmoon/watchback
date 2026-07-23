# 连续游玩与角斗竞技成就 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有成就墙中新增“风雨无阻”“财富大亨”“登峰造极”三组三级成就，并使用合成测试证明连续自然日、角斗最后一回合资金及任意职责角斗段位的判定正确。

**Architecture:** 继续由 Rust `review::achievements` 聚合全部职业生涯成就，不新增 SQLite schema。扩展每场对局事实投影：普通对局只提供游玩日期，角斗对局额外从详情 JSON 最后一回合的当前玩家条目读取 `endWorth` 与 `rankInfo.rankName`；通用前端成就墙按新 key 自动渲染，资源清单与视觉 fixture 显式登记新 key。

**Tech Stack:** Rust、rusqlite、serde_json、chrono、Tauri 2、React、TypeScript、Vite、PNG RGBA 资源

---

## 文件结构

- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`
  - 新增三组成就定义。
  - 把当前玩家 `bnet_id`、角斗最后一回合 `endWorth` 和角斗段位投影到 `CareerMatchFact`。
  - 计算连续本地自然日及两类角斗成就的首次解锁时间。
- Modify: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`
  - 扩展合成 fixture，覆盖新 key、自然日去重与断档、最后一回合限定、当前玩家限定、角斗段位阈值和缺失数据。
- Create: `assets/achievements/token-style/{unwavering,wealth-tycoon,stadium-ascendant}-token-tier-{1,2,3}.png`
  - 九枚正式透明徽章。
- Modify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`
  - 把三个新 key 加入正式资源契约。
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`
  - 把三个新 key 加入视觉验收成就 fixture。
- Modify: `apps/owstats-desktop/runtime-config/assets-inventory.json`
  - 由 `npm run prepare:assets` 自动刷新，不手工维护。

不修改 `AchievementWall.tsx`、`assets.ts` 或 TypeScript DTO：现有 `CareerAchievement` 契约和 `achievementBadgeUrl(key, level)` 已能承载新增项。

### Task 1: 固定三组成就的公开契约

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`

- [ ] **Step 1: 扩展空数据库契约测试**

在 `career_achievement_contract_is_stable` 的 key 数组末尾加入：

```rust
"unwavering",
"wealth-tycoon",
"stadium-ascendant",
```

并增加精确中文字段断言：

```rust
assert_eq!(achievement("unwavering").title, "风雨无阻");
assert_eq!(achievement("unwavering").detail, "连续自然日完成可统计对局");
assert_eq!(
    achievement("unwavering")
        .tiers
        .iter()
        .map(|tier| tier.requirement.as_str())
        .collect::<Vec<_>>(),
    ["连续游玩 10 天", "连续游玩 20 天", "连续游玩 30 天"]
);
assert_eq!(achievement("wealth-tycoon").title, "财富大亨");
assert_eq!(
    achievement("wealth-tycoon")
        .tiers
        .iter()
        .map(|tier| tier.requirement.as_str())
        .collect::<Vec<_>>(),
    [
        "角斗竞技最后一回合结束时资金达到 80,000",
        "角斗竞技最后一回合结束时资金达到 90,000",
        "角斗竞技最后一回合结束时资金达到 100,000",
    ]
);
assert_eq!(achievement("stadium-ascendant").title, "登峰造极");
assert_eq!(
    achievement("stadium-ascendant")
        .tiers
        .iter()
        .map(|tier| tier.requirement.as_str())
        .collect::<Vec<_>>(),
    [
        "角斗竞技任意职责段位达到专家",
        "角斗竞技任意职责段位达到全明星",
        "角斗竞技任意职责段位达到传奇",
    ]
);
```

- [ ] **Step 2: 运行契约测试并确认失败**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test career_achievements career_achievement_contract_is_stable -- --exact
```

Expected: FAIL，新 key 不存在或 key 数组数量不一致。

- [ ] **Step 3: 在 `DEFINITIONS` 末尾加入三组定义**

把数组长度从 `16` 改为 `19`，并追加：

```rust
AchievementDefinition {
    key: "unwavering",
    title: "风雨无阻",
    detail: "连续自然日完成可统计对局",
    requirements: ["连续游玩 10 天", "连续游玩 20 天", "连续游玩 30 天"],
},
AchievementDefinition {
    key: "wealth-tycoon",
    title: "财富大亨",
    detail: "角斗竞技最后一回合资金",
    requirements: [
        "角斗竞技最后一回合结束时资金达到 80,000",
        "角斗竞技最后一回合结束时资金达到 90,000",
        "角斗竞技最后一回合结束时资金达到 100,000",
    ],
},
AchievementDefinition {
    key: "stadium-ascendant",
    title: "登峰造极",
    detail: "角斗竞技任意职责最高段位",
    requirements: [
        "角斗竞技任意职责段位达到专家",
        "角斗竞技任意职责段位达到全明星",
        "角斗竞技任意职责段位达到传奇",
    ],
},
```

- [ ] **Step 4: 运行契约测试并确认通过**

Run:

```powershell
cargo test --test career_achievements career_achievement_contract_is_stable -- --exact
```

Expected: PASS，空数据库返回 19 个稳定 key，每项三级均未解锁。

- [ ] **Step 5: 提交公开契约**

```powershell
git add apps/owstats-desktop/src-tauri/src/review/achievements.rs apps/owstats-desktop/src-tauri/tests/career_achievements.rs
git commit -m "feat: define continuity and stadium achievements"
```

### Task 2: 实现连续自然日事实与解锁规则

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`

- [ ] **Step 1: 新增合成日期辅助函数和连续游玩测试**

在测试文件导入：

```rust
use chrono::{Local, TimeZone};
```

新增：

```rust
fn local_ts(year: i32, month: u32, day: u32, hour: u32) -> i64 {
    Local
        .with_ymd_and_hms(year, month, day, hour, 0, 0)
        .single()
        .unwrap()
        .timestamp()
}
```

新增测试。测试需用现有 `seed_match` 和 `seed_player` 创建：

- 2026-01-01 同一天两场，证明一天只计一次。
- 2026-01-02 至 2026-01-10 每天一场，首次在第 10 个不同自然日解锁 I。
- 2026-01-12 至 2026-01-31 连续 20 天，证明 2026-01-11 的断档会重置，并在 2026-01-31 解锁 II。
- 2026-02-02 至 2026-03-03 连续 30 天，证明 III 在最后一天解锁。

断言：

```rust
let item = achievement("unwavering");
assert_eq!(item.current_level, 3);
assert_eq!(item.tiers[0].unlocked_at, Some(local_ts(2026, 1, 10, 12)));
assert_eq!(item.tiers[1].unlocked_at, Some(local_ts(2026, 1, 31, 12)));
assert_eq!(item.tiers[2].unlocked_at, Some(local_ts(2026, 3, 3, 12)));
```

另增一条跨赛季不断档断言：相邻自然日即使 `season` 不同也应继续累计，因为该成就是职业生涯规则。

- [ ] **Step 2: 运行新测试并确认失败**

Run:

```powershell
cargo test --test career_achievements continuity_achievement_uses_distinct_local_days_and_resets_after_gap -- --exact
```

Expected: FAIL，`unwavering.current_level` 仍为 0。

- [ ] **Step 3: 实现本地自然日转换与连续段计算**

在 `achievements.rs` 导入：

```rust
use chrono::{Local, NaiveDate, TimeZone};
```

新增纯辅助函数：

```rust
fn local_match_day(begin_ts: i64) -> Option<NaiveDate> {
    Local.timestamp_opt(begin_ts, 0).single().map(|value| value.date_naive())
}

fn consecutive_day_unlock_dates(
    facts: &[CareerMatchFact],
    thresholds: [i64; 3],
) -> [Option<i64>; 3] {
    let mut dates = [None; 3];
    let mut previous_day = None;
    let mut streak = 0_i64;

    for fact in facts {
        let Some(day) = local_match_day(fact.begin_ts) else {
            continue;
        };
        if previous_day == Some(day) {
            continue;
        }
        streak = match previous_day {
            Some(previous) if day.signed_duration_since(previous).num_days() == 1 => streak + 1,
            _ => 1,
        };
        previous_day = Some(day);
        for (index, threshold) in thresholds.into_iter().enumerate() {
            if streak >= threshold && dates[index].is_none() {
                dates[index] = Some(fact.begin_ts);
            }
        }
    }
    dates
}
```

在 `evaluate_new_achievements` 中加入：

```rust
set_dates(
    achievements,
    "unwavering",
    consecutive_day_unlock_dates(facts, [10, 20, 30]),
);
```

`load_career_facts` 已按 `m.begin_ts, m.match_id` 升序返回，辅助函数不得自行使用当前时间，也不得按赛季拆分。

- [ ] **Step 4: 运行连续日期测试**

Run:

```powershell
cargo test --test career_achievements continuity_achievement_uses_distinct_local_days_and_resets_after_gap -- --exact
```

Expected: PASS。

- [ ] **Step 5: 运行完整成就测试并提交**

Run:

```powershell
cargo test --test career_achievements
```

Expected: PASS。

```powershell
git add apps/owstats-desktop/src-tauri/src/review/achievements.rs apps/owstats-desktop/src-tauri/tests/career_achievements.rs
git commit -m "feat: calculate consecutive play achievements"
```

### Task 3: 投影角斗最后一回合的当前玩家事实

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/tests/career_achievements.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/review/achievements.rs`

- [ ] **Step 1: 新增角斗 fixture 辅助函数**

新增 `seed_stadium_match`，插入：

```rust
bucket = "stadium"
game_mode = "SportFight"
match_kind = "arena"
raw_entry_json = {"roleType": role}
```

同时插入当前玩家 `match_players` 行和如下合成详情：

```rust
serde_json::json!({
    "payload": {
        "data": {
            "roundCountList": rounds
        }
    }
})
```

每个 round 的当前玩家条目使用：

```rust
{
    "bnetId": "synthetic-current",
    "endWorth": end_worth,
    "rankInfo": {
        "rankName": rank_name,
        "rankSubTier": rank_sub_tier
    }
}
```

所有值必须为合成值，不读取真实凭据、真实 SQLite 或用户数据。

- [ ] **Step 2: 新增最后一回合与当前玩家限定测试**

创建三场角斗：

1. 前一回合当前玩家 `endWorth = 120_000`，最后一回合 `79_999`，不得解锁财富成就。
2. 最后一回合队友 `endWorth = 120_000`、当前玩家 `79_999`，不得借用队友资金。
3. 最后一回合当前玩家 `endWorth = 100_000`，应同时解锁三级，解锁时间均为该场 `begin_ts`。

再创建一场列表字段 `matches.total_worth = 120_000`、但详情没有 `roundCountList` 的角斗，断言它不解锁，证明不使用列表汇总值兜底。

断言：

```rust
let item = achievement("wealth-tycoon");
assert_eq!(item.current_level, 3);
assert_eq!(
    item.tiers
        .iter()
        .map(|tier| tier.unlocked_at)
        .collect::<Vec<_>>(),
    [Some(400), Some(400), Some(400)]
);
```

- [ ] **Step 3: 新增角斗段位阈值测试**

按时间顺序创建三个不同 `roleType` 的角斗：

- `tank`，最后一回合当前玩家 `rankName = "Diamond"`、`rankSubTier = 5`，解锁专家。
- `dps`，最后一回合当前玩家 `rankName = "Master"`、`rankSubTier = 5`，解锁全明星。
- `support`，最后一回合当前玩家 `rankName = "Grandmaster"`、`rankSubTier = 5`，解锁传奇。

另加入干扰数据：

- 普通竞技 `rankName = "Grandmaster"` 不得解锁角斗段位成就。
- 角斗最后一回合仅队友为 `Grandmaster` 不得解锁。
- 最后一回合缺少当前玩家、`rankInfo` 或 `endWorth` 时保持未命中且不报错。

断言三个等级的解锁时间分别等于对应角斗的 `begin_ts`。

- [ ] **Step 4: 运行新测试并确认失败**

Run:

```powershell
cargo test --test career_achievements stadium_wealth_uses_current_players_last_round_end_worth -- --exact
cargo test --test career_achievements stadium_ascendant_accepts_any_role_and_stadium_ranks_only -- --exact
```

Expected: 两条均 FAIL，新事实尚未投影。

- [ ] **Step 5: 扩展 `CareerMatchFact` 与 SQL 投影**

新增字段：

```rust
is_stadium: bool,
stadium_end_worth: Option<i64>,
stadium_rank_level: i64,
```

把 SQL 投影固定为以下顺序。继续只选择 `is_current_player = 1` 的投影行，不新增凭据或账号字段到返回 DTO：

```sql
SELECT m.match_id, m.season, m.result, m.begin_ts, m.bucket, m.game_mode,
       player.hero_guid, player.kill, player.assist, player.death,
       player.hero_damage, m.duration_sec, m.raw_entry_json,
       player.damage_taken, player.bnet_id,
       EXISTS(SELECT 1 FROM match_players friend
              WHERE friend.match_id = m.match_id AND friend.is_friend = 1),
       detail.raw_detail_json, evaluation.grade, evaluation.primary_tag,
       evaluation.evidence_tags_json
FROM matches m
LEFT JOIN match_players player ON player.rowid = (
    SELECT current_player.rowid FROM match_players current_player
    WHERE current_player.match_id = m.match_id
      AND current_player.is_current_player = 1
    ORDER BY current_player.side, current_player.bnet_id LIMIT 1
)
LEFT JOIN match_details detail ON detail.match_id = m.match_id
LEFT JOIN match_evaluations evaluation ON evaluation.match_id = m.match_id
ORDER BY m.begin_ts, m.match_id
```

在 mapper 中：

```rust
let raw_entry: String = row.get(12)?;
let raw_detail: Option<String> = row.get(16)?;
let bucket: String = row.get(4)?;
let game_mode: String = row.get(5)?;
let current_bnet: Option<String> = row.get(14)?;
let is_stadium = bucket == "stadium" && game_mode == "SportFight";
let (stadium_end_worth, stadium_rank_level) = if is_stadium {
    stadium_last_round_fact(raw_detail.as_deref(), current_bnet.as_deref())
} else {
    (None, 0)
};
```

构造 `CareerMatchFact` 时使用固定索引：`hero = 6`、`kills = 7`、`assists = 8`、`deaths = 9`、`damage = 10`、`duration_sec = 11`、`damage_taken = 13`、`has_friend = 15`、`grade = 17`、`primary_tag = 18`、`evidence_tags_json = 19`。

- [ ] **Step 6: 实现详情解析辅助函数**

新增：

```rust
fn stadium_last_round_fact(
    raw_detail: Option<&str>,
    current_bnet: Option<&str>,
) -> (Option<i64>, i64) {
    let Some(current_bnet) = current_bnet.filter(|value| !value.trim().is_empty()) else {
        return (None, 0);
    };
    let detail = raw_detail
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .unwrap_or(Value::Null);
    let data = detail
        .pointer("/payload/data")
        .or_else(|| detail.get("data"))
        .unwrap_or(&detail);
    let Some(player) = data
        .get("roundCountList")
        .and_then(Value::as_array)
        .and_then(|rounds| rounds.last())
        .and_then(|round| round.get("teammateList"))
        .and_then(Value::as_array)
        .and_then(|players| {
            players.iter().find(|player| {
                player
                    .get("bnetId")
                    .and_then(value_as_string)
                    .as_deref()
                    == Some(current_bnet)
            })
        })
    else {
        return (None, 0);
    };
    (
        player.get("endWorth").and_then(Value::as_i64),
        stadium_rank_level_from_player(player),
    )
}

fn value_as_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.as_i64().map(|item| item.to_string()))
        .or_else(|| value.as_u64().map(|item| item.to_string()))
}

fn stadium_rank_level_from_player(player: &Value) -> i64 {
    let name = player
        .get("rankInfo")
        .and_then(|rank| rank.get("rankName").or_else(|| rank.get("rank_name")))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match name.as_str() {
        "bronze" => 1,
        "silver" => 2,
        "gold" => 3,
        "platinum" => 4,
        "diamond" => 5,
        "master" => 6,
        "grandmaster" => 7,
        _ => 0,
    }
}
```

不要读取 `totalCount` 或 `matches.total_worth` 作为资金兜底；不要递归搜索任意 `endWorth`，避免误取前一回合或队友值。`Diamond/Master/Grandmaster` 分别是角斗中文段位“专家/全明星/传奇”的稳定上游 key。

- [ ] **Step 7: 实现两组角斗规则**

在 `evaluate_new_achievements` 中加入：

```rust
set_dates(
    achievements,
    "wealth-tycoon",
    [80_000, 90_000, 100_000].map(|threshold| {
        first_match_unlock(facts, |fact| {
            fact.is_stadium
                && fact
                    .stadium_end_worth
                    .is_some_and(|value| value >= threshold)
        })
    }),
);
set_dates(
    achievements,
    "stadium-ascendant",
    [5, 6, 7].map(|threshold| {
        first_match_unlock(facts, |fact| {
            fact.is_stadium && fact.stadium_rank_level >= threshold
        })
    }),
);
```

- [ ] **Step 8: 运行角斗测试与全部后端成就测试**

Run:

```powershell
cargo test --test career_achievements stadium_wealth_uses_current_players_last_round_end_worth -- --exact
cargo test --test career_achievements stadium_ascendant_accepts_any_role_and_stadium_ranks_only -- --exact
cargo test --test career_achievements
```

Expected: 全部 PASS。

- [ ] **Step 9: 格式化、检查并提交**

Run:

```powershell
cargo fmt --check
```

Expected: PASS；如失败，先运行 `cargo fmt`，再重新运行 `cargo fmt --check`。

```powershell
git add apps/owstats-desktop/src-tauri/src/review/achievements.rs apps/owstats-desktop/src-tauri/tests/career_achievements.rs
git commit -m "feat: evaluate stadium wealth and rank achievements"
```

### Task 4: 逐组设计并验收三组成就徽章

**Files:**
- Create temporarily: `assets/achievements/token-style/{key}-token-group.png`
- Create temporarily: `assets/achievements/token-style/{key}-token-preview.png`
- Create temporarily: `tmp/achievement-transparent/{key}/tier-{1,2,3}-key.png`
- Create: `assets/achievements/token-style/{key}-token-tier-{1,2,3}.png`

- [ ] **Step 1: 生成并审核“风雨无阻”三联预览**

使用 `imagegen`，一次只生成 `unwavering`：

```text
Create a horizontal set of three fantasy challenge token achievement badges for a competitive game UI.

Achievement theme: unwavering commitment shown through consecutive days of play
Visual symbol: a firm abstract footprint encircled by a continuous sun-path arc, no text

Style reference: elegant MOBA challenge token badges, ornate metallic frames, collectible rank medallions, consistent shape language, premium game achievement UI.

Three tiers from left to right:
Tier 1 is restrained bronze with warm orange accents and a clear simple frame.
Tier 2 is refined silver-blue with moon-white accents and stronger material detail.
Tier 3 is prestigious gold with sunlit accents, richer ornament, and the most ceremonial frame.

Use an open-frame construction with visible gaps between rings, leaves, ornaments, and the central symbol. Do not place a solid dark disc behind the badge.
No text, no letters, no numbers, no logos, no calendars, no clock markings, no characters, no weapons, no screenshots.
Centered badges, consistent scale and lighting, high detail. This image is a design preview only.
```

把预览交给用户确认；未确认前不得生成正式 tier 资源。

- [ ] **Step 2: 生成并审核“财富大亨”三联预览**

只生成 `wealth-tycoon`：

```text
Achievement theme: finishing the final stadium round with exceptional wealth and resource control
Visual symbol: an open abstract treasury arch lifting one faceted gemstone, no text or currency marks
```

其余结构沿用工作流模板；三级分别采用青铜翠绿、银蓝晶光、金色琥珀。明确禁止货币符号、数字、筹码、赌场元素、钱袋文字、具体游戏道具和暗色底盘。把预览交给用户确认。

- [ ] **Step 3: 生成并审核“登峰造极”三联预览**

只生成 `stadium-ascendant`：

```text
Achievement theme: ascending through stadium competitive ranks to the summit
Visual symbol: three rising open steps or mountain facets converging beneath a summit star, no text
```

其余结构沿用工作流模板；三级分别采用青铜岩色、银蓝冰峰、金色晨光。明确禁止段位文字、数字、职责图标、排行榜、官方段位徽标、角色和武器。把预览交给用户确认。

- [ ] **Step 4: 为每个已确认设计逐级生成纯色键图**

每个 key 的 I、II、III 必须分别生成，不能从三联图裁切。使用已确认图作为设计参考，并使用工作流的正式单枚 prompt：

```text
Place the badge on a completely uniform solid #ff00ff chroma-key background.
Use an open-frame construction: all gaps between rings, branches, leaves, spikes, ornaments, and the central symbol must expose the background directly.
One front-facing badge only, centered with even empty space around it.
No text, letters, numbers, logo, watermark, shadow, glow, rays, particles, dark disc, gradient, or texture.
```

每组源图分别保存为对应目录中的三个明确文件：

```text
tmp/achievement-transparent/unwavering/tier-1-key.png
tmp/achievement-transparent/unwavering/tier-2-key.png
tmp/achievement-transparent/unwavering/tier-3-key.png
tmp/achievement-transparent/wealth-tycoon/tier-1-key.png
tmp/achievement-transparent/wealth-tycoon/tier-2-key.png
tmp/achievement-transparent/wealth-tycoon/tier-3-key.png
tmp/achievement-transparent/stadium-ascendant/tier-1-key.png
tmp/achievement-transparent/stadium-ascendant/tier-2-key.png
tmp/achievement-transparent/stadium-ascendant/tier-3-key.png
```

- [ ] **Step 5: 转换为正式透明 PNG**

Run:

```powershell
python scripts/process_achievement_token_transparency.py --group unwavering --input-dir tmp/achievement-transparent/unwavering
python scripts/process_achievement_token_transparency.py --group wealth-tycoon --input-dir tmp/achievement-transparent/wealth-tycoon
python scripts/process_achievement_token_transparency.py --group stadium-ascendant --input-dir tmp/achievement-transparent/stadium-ascendant
```

Expected: 每组输出三个 `512×512` RGBA PNG，四角 alpha 为 0，主体不超过 `480×480`。

- [ ] **Step 6: 制作深浅双背景预览并人工验收**

逐枚确认：

- 内外开放区域均透明，无暗色圆盘。
- 深色与浅色背景均无洋红残边、绿色污染、灰黑毛边。
- 缩小到成就墙尺寸后核心符号可辨。
- I/II/III 保持同一核心符号，且青铜、银蓝、金色递进明确。
- 正式 PNG 不包含阴影、外发光、放射线或粒子。

任何一项失败都重新生成对应 tier，不用调色或遮盖掩盖污染。

- [ ] **Step 7: 只提交九枚正式资源**

`group`、`preview` 和 `tmp` 中间文件不进入前端资源提交。

```powershell
git add assets/achievements/token-style/unwavering-token-tier-1.png assets/achievements/token-style/unwavering-token-tier-2.png assets/achievements/token-style/unwavering-token-tier-3.png
git add assets/achievements/token-style/wealth-tycoon-token-tier-1.png assets/achievements/token-style/wealth-tycoon-token-tier-2.png assets/achievements/token-style/wealth-tycoon-token-tier-3.png
git add assets/achievements/token-style/stadium-ascendant-token-tier-1.png assets/achievements/token-style/stadium-ascendant-token-tier-2.png assets/achievements/token-style/stadium-ascendant-token-tier-3.png
git commit -m "feat: add continuity and stadium achievement badges"
```

### Task 5: 接入打包清单与视觉验收 fixture

**Files:**
- Modify: `apps/owstats-desktop/scripts/phase6Assets.test.ts`
- Modify: `apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts`
- Modify: `apps/owstats-desktop/runtime-config/assets-inventory.json`

- [ ] **Step 1: 先扩展资源测试 key 列表**

在 `careerAchievementKeys` 末尾加入：

```typescript
"unwavering",
"wealth-tycoon",
"stadium-ascendant",
```

- [ ] **Step 2: 运行资源测试并确认失败**

Run:

```powershell
cd apps\owstats-desktop
npm run test:resources
```

Expected: FAIL，运行时资源清单尚未包含九枚新徽章。

- [ ] **Step 3: 扩展视觉 fixture**

在 `uiVisualAcceptance.fixture.ts` 的 `fixtureAchievementKeys` 末尾加入相同三个 key。保持现有通用 mock 数据结构，不为三个 key 新建专用 React 分支。

- [ ] **Step 4: 刷新正式资源**

Run:

```powershell
npm run prepare:assets
```

Expected: `runtime-config/assets-inventory.json` 新增 `unwavering`、`wealth-tycoon`、`stadium-ascendant` 各三级共九条 `achievements/token-style/` PNG 路径，没有开发机绝对路径或 `tmp` 路径。

- [ ] **Step 5: 运行资源与前端测试**

Run:

```powershell
npm run test:resources
npm test
```

Expected: 全部 PASS。

- [ ] **Step 6: 提交打包接入**

```powershell
git add apps/owstats-desktop/scripts/phase6Assets.test.ts apps/owstats-desktop/scripts/uiVisualAcceptance.fixture.ts apps/owstats-desktop/runtime-config/assets-inventory.json
git commit -m "test: package new achievement badges"
```

### Task 6: 完整回归与交付检查

**Files:**
- Verify only

- [ ] **Step 1: 运行后端格式与测试**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
```

Expected: 全部 PASS。

- [ ] **Step 2: 运行前端测试与构建**

Run:

```powershell
cd apps\owstats-desktop
npm test
npm run build
```

Expected: 全部 PASS，构建产物中可解析三个新 key 的九枚徽章。

- [ ] **Step 3: 运行发布构建**

Run:

```powershell
npm run tauri build
```

Expected: PASS，安装包资源不依赖 `D:\Codex\...`、`/@fs/`、`tmp` 或未跟踪本机文件。

- [ ] **Step 4: 检查差异卫生与隐私边界**

Run:

```powershell
cd ..\..
git diff --check
git status --short
git diff --cached --name-only
```

Expected:

- `git diff --check` 无输出。
- 只包含本计划明确列出的代码、测试、九枚正式 PNG、资源清单和 fixture 变更。
- 不包含真实 SQLite、浏览器 profile、凭据、token、cookie、authorization、真实数据 dump、`tmp` 中间图或 `.codebase-memory/`。

- [ ] **Step 5: 报告交付结果**

交付说明必须列出：

- 实际执行的命令及通过/失败结果。
- 未执行的验证项及原因。
- 三组徽章的人工深浅背景验收结论。
- 是否接触真实隐私数据；正常执行应明确为“否，仅使用合成测试数据”。
