# 战报角斗好友协作与动态赛季实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让战报页动态展示所有本地已同步赛季，并让好友亲密度、高频组合和模型榜单纳入角斗竞技、角斗快速，其中角斗模型统计只取最后有效回合。

**Architecture:** 保持 `match_players` 的普通模式边界，在 `analytics/arena.rs` 中查询并解析 `match_details.raw_detail_json`，将角斗最后有效回合转换成现有好友聚合器的内部行后合并计算。`PersonalSeasonReview` 同时返回后端发现的 `availableSeasons`，前端使用该列表替换固定 S21/S22/S23，并以后端实际选中赛季修正 URL。

**Tech Stack:** Rust、rusqlite、serde_json、Tauri 2、React、TypeScript、tsx 脚本测试

---

## 文件边界

- Create: `apps/owstats-desktop/src-tauri/src/analytics/arena.rs`
  - 只负责角斗好友数据查询、最后有效回合解析、安全名称和内部行转换。
- Create: `apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs`
  - 使用完全合成数据覆盖动态赛季、两类角斗、最后回合、好友识别、去重和赛季隔离。
- Modify: `apps/owstats-desktop/src-tauri/src/analytics/mod.rs`
  - 合并普通与角斗好友行，并让高频组合接收角斗成员行；现有评分器不改公式。
- Modify: `apps/owstats-desktop/src-tauri/src/review/personal.rs`
  - 发现、验证并选择动态战报赛季。
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
  - 为 `PersonalSeasonReview` 增加 `available_seasons`。
- Modify: `apps/owstats-desktop/src/shared/types.ts`
  - 同步增加 `availableSeasons`。
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx`
  - 删除固定赛季列表，使用响应列表，并同步后端回退赛季与 URL。
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts`
  - 锁定动态赛季和正整数 URL 合同。
- Modify: `apps/owstats-desktop/scripts/reviewAchievements.test.ts`
  - 补齐新增 DTO 字段，保持成就 fixture 类型正确。

现有未提交的 `importer.rs`、`database/migrations.rs`、`import_baseline.rs` 和对局筛选文件不在本计划修改范围内。

### Task 1: 后端返回并校验动态战报赛季

**Files:**
- Create: `apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs:561`
- Modify: `apps/owstats-desktop/src-tauri/src/review/personal.rs:62-125`

- [ ] **Step 1: 写动态赛季失败测试**

创建测试文件，并写入最小数据库 helper 与以下测试。`seed_match` 必须插入完整 `matches` 必填列；用 `bucket` 参数分别制造普通和仅角斗赛季。

```rust
use owstats_desktop_lib::{get_personal_season_review, OwDatabase, PersonalReviewRequest};
use rusqlite::params;

fn test_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.keep().join("owstats.db"));
    db.initialize().unwrap();
    db
}

fn seed_match(db: &OwDatabase, id: &str, season: i64, bucket: &str, mode: &str, kind: &str) {
    db.connect()
        .unwrap()
        .execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
                hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
                deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'win', 'map-test', '测试地图', 'hero-self',
                '测试英雄', ?6, 1, 0, 10, 5, 2, 1000, 100, 50, 0, 600, '{}',
                '2026-07-16T00:00:00Z')
            "#,
            params![id, season, bucket, mode, kind, season * 1000],
        )
        .unwrap();
}

#[test]
fn personal_review_discovers_all_local_seasons_and_validates_requests() {
    let db = test_db();
    seed_match(&db, "s15-normal", 15, "preset", "SportPreset", "normal");
    seed_match(&db, "s20-arena", 20, "stadium", "LeisureFight", "fight");
    seed_match(&db, "s23-normal", 23, "preset", "SportPreset", "normal");

    let latest = get_personal_season_review(&db, PersonalReviewRequest { season: None }).unwrap();
    assert_eq!(latest.available_seasons, vec![23, 20, 15]);
    assert_eq!(latest.summary.season, Some(23));

    let older = get_personal_season_review(&db, PersonalReviewRequest { season: Some(15) }).unwrap();
    assert_eq!(older.summary.season, Some(15));

    let invalid = get_personal_season_review(&db, PersonalReviewRequest { season: Some(999) }).unwrap();
    assert_eq!(invalid.summary.season, Some(23));
}
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test review_arena_collaboration personal_review_discovers_all_local_seasons_and_validates_requests -- --exact
```

Expected: 编译失败，指出 `PersonalSeasonReview` 没有 `available_seasons` 字段。

- [ ] **Step 3: 增加 DTO 字段与动态选择逻辑**

在 `PersonalSeasonReview` 中紧跟 `summary` 增加：

```rust
pub available_seasons: Vec<i64>,
```

在 `review/personal.rs` 增加：

```rust
fn available_review_seasons(db: &OwDatabase) -> Result<Vec<i64>> {
    let conn = db.connect()?;
    let mut stmt = conn.prepare("SELECT DISTINCT season FROM matches ORDER BY season DESC")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

pub(super) fn selected_season(
    available_seasons: &[i64],
    requested: Option<i64>,
) -> Option<i64> {
    requested
        .filter(|season| available_seasons.contains(season))
        .or_else(|| available_seasons.first().copied())
}
```

把 `get_personal_season_review_impl` 开头改为先调用 `available_review_seasons`，再调用新的 `selected_season`：

```rust
let available_seasons = available_review_seasons(db)?;
let season = selected_season(&available_seasons, request.season);
```

在返回的 `PersonalSeasonReview` 中加入：

```rust
available_seasons,
```

删除旧的、直接信任 `requested` 且只查询 `bucket='preset'` 最新赛季的 `selected_season(db, requested)` 实现。

- [ ] **Step 4: 运行测试并确认 GREEN**

Run:

```powershell
cargo test --test review_arena_collaboration personal_review_discovers_all_local_seasons_and_validates_requests -- --exact
```

Expected: PASS；S20 即使只有角斗对局也出现在列表。

- [ ] **Step 5: 运行既有战报后端回归**

Run:

```powershell
cargo test --test phase3_review
cargo test --test phase8_multi_season_pages
```

Expected: PASS。若编译器指出测试中的 `PersonalSeasonReview` 字面量缺字段，只给该字面量补 `available_seasons`，不要改变断言语义。

- [ ] **Step 6: 提交 Task 1**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/src/review/personal.rs apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs
git commit -m "feat: discover dynamic review seasons"
```

### Task 2: 战报前端使用后端动态赛季

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts:482`
- Modify: `apps/owstats-desktop/src/pages/PersonalReviewPage.tsx:37,105-215,406`
- Modify: `apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts:1-45`
- Modify: `apps/owstats-desktop/scripts/reviewAchievements.test.ts:14`

- [ ] **Step 1: 写前端动态赛季失败测试**

在 `phase8MultiSeasonPages.test.ts` 从 `PersonalReviewPage` 导入 `parseReviewSeasonParam`，并加入：

```ts
assert.equal(reviewSource.includes("selectableSeasons"), false, "战报不得保留固定赛季列表");
assert.match(reviewSource, /review\?\.availableSeasons \?\? \[\]/, "战报赛季选项必须来自后端响应");
assert.match(reviewSource, /seasons=\{availableSeasons\}/, "战报应把动态赛季交给共享选择器");
assert.equal(parseReviewSeasonParam("15"), 15, "战报应接受本地旧赛季");
assert.equal(parseReviewSeasonParam("20"), 20, "战报应接受动态同步赛季");
assert.equal(parseReviewSeasonParam("0"), undefined, "战报应拒绝零");
assert.equal(parseReviewSeasonParam("-1"), undefined, "战报应拒绝负数");
assert.equal(parseReviewSeasonParam("15.5"), undefined, "战报应拒绝非整数");
assert.match(reviewSource, /backendSeason !== selectedSeason/, "后端回退到实际赛季时必须修正页面状态");
```

在 `reviewAchievements.test.ts` 的 `PersonalSeasonReview` fixture 顶层加入：

```ts
availableSeasons: [23, 20, 15],
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase8MultiSeasonPages.test.ts
```

Expected: FAIL，因为页面仍包含 `selectableSeasons`，且没有导出的 `parseReviewSeasonParam`。

- [ ] **Step 3: 同步 TypeScript DTO**

把 `PersonalSeasonReview` 定义改为：

```ts
export type PersonalSeasonReview = {
  summary: ReviewSummary;
  availableSeasons: number[];
  baseline: SeasonBaseline;
  roleSummaries: RoleRankSummary[];
  stadiumRoleSummaries: RoleRankSummary[];
  phase5Highlights: Phase5ReviewHighlights;
  topConclusions: ReviewConclusion[];
  personalBests: PersonalBest[];
  streaks: StreakSummary;
  heroPool: HeroPoolStability;
  maps: MapAdaptability;
};
```

- [ ] **Step 4: 替换固定列表并同步后端赛季**

删除：

```ts
const selectableSeasons = [23, 22, 21] as const;
```

在组件状态声明后增加：

```ts
const availableSeasons = review?.availableSeasons ?? [];
```

把请求成功后的赛季同步改为对默认与无效 URL 都生效：

```ts
const backendSeason = nextReview.summary.season;
loadedReviewSeasonRef.current = typeof backendSeason === "number" && backendSeason > 0 ? backendSeason : null;
if (typeof backendSeason === "number" && backendSeason > 0 && backendSeason !== selectedSeason) {
  setSelectedSeason(backendSeason);
  const nextParams = new URLSearchParams(params);
  nextParams.set("season", String(backendSeason));
  nextParams.set("view", view);
  setParams(nextParams, { replace: true });
}
```

将选择器属性改为：

```tsx
<PageSeasonSelect
  value={selectedSeason}
  seasons={availableSeasons}
  disabled={availableSeasons.length === 0}
  onChange={(season) => {
    if (season !== undefined) changeSeason(season);
  }}
/>
```

把页面底部解析函数导出并收紧为正整数：

```ts
export function parseReviewSeasonParam(value: string | null) {
  if (!value) return undefined;
  const season = Number(value);
  return Number.isInteger(season) && season > 0 ? season : undefined;
}
```

同时把组件中的调用从 `parseSeasonParam` 改为 `parseReviewSeasonParam`。

- [ ] **Step 5: 运行测试与类型构建并确认 GREEN**

Run:

```powershell
npx tsx scripts/phase8MultiSeasonPages.test.ts
npx tsx scripts/reviewAchievements.test.ts
npm run build
```

Expected: 全部 PASS，构建不再报告缺少 `availableSeasons`。

- [ ] **Step 6: 提交 Task 2**

```powershell
git add -- apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts apps/owstats-desktop/scripts/reviewAchievements.test.ts
git commit -m "feat: render dynamic review seasons"
```

### Task 3: 解析角斗最后有效回合并生成好友行

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/analytics/arena.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/analytics/mod.rs:12-20,90-123`
- Modify: `apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs`

- [ ] **Step 1: 扩展合成 fixture 并写三模式合并失败测试**

在测试文件增加：

```rust
fn seed_current_player(db: &OwDatabase) {
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO players (player_id,bnet_id,display_name,raw_json,updated_at) VALUES ('self','self','当前玩家','{}','2026-07-16T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute_batch(
        "INSERT INTO hero_dict (guid,name,updated_at) VALUES ('hero-self','当前英雄','2026-07-16T00:00:00Z');
         INSERT INTO hero_dict (guid,name,updated_at) VALUES ('hero-friend','好友英雄','2026-07-16T00:00:00Z');"
    ).unwrap();
}

fn seed_normal_friend_match(db: &OwDatabase, id: &str, season: i64) {
    seed_match(db, id, season, "preset", "SportPreset", "normal");
    let conn = db.connect().unwrap();
    for (bnet, name, current, friend, damage_taken, mitigation) in [
        ("self", "当前玩家", 1_i64, 0_i64, 0_i64, 0_i64),
        ("friend-1", "测试好友", 0_i64, 1_i64, 100_i64, 50_i64),
    ] {
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id,season,side,bnet_id,player_name,hero_guid,hero_name,
                is_current_player,is_friend,kill,assist,death,hero_damage,cure,
                resist_damage,healing_taken,damage_taken,final_hit,target_competing_time,
                role_type,raw_json,updated_at
            ) VALUES (?1,?2,'teammate',?3,?4,?5,?6,?7,?8,10,5,2,1000,100,?9,80,?10,1,30,'damage','{}','2026-07-16T00:00:00Z')
            "#,
            params![id, season, bnet, name, if current == 1 { "hero-self" } else { "hero-friend" }, if current == 1 { "当前英雄" } else { "好友英雄" }, current, friend, mitigation, damage_taken],
        ).unwrap();
    }
}

fn seed_arena_friend_match(db: &OwDatabase, id: &str, season: i64, mode: &str, result: &str) {
    seed_match(db, id, season, "stadium", mode, "fight");
    let detail = serde_json::json!({
        "match_id": id,
        "match_kind": "fight",
        "payload": {"data": {
            "totalCount": {"teammateList": [
                {"bnetId":"self","name":"当前玩家","friendBnetIds":["friend-1"],"heroGuid":"hero-self"},
                {"bnetId":"friend-1","name":"测试好友","heroGuid":"hero-friend","kill":999,"resistDamage":9999,"damageTaken":9999}
            ]},
            "roundCountList": [
                {"teammateList": [
                    {"bnetId":"self","name":"当前玩家","friendBnetIds":["friend-1"],"heroGuid":"hero-self"},
                    {"bnetId":"friend-1","name":"测试好友","heroGuid":"hero-friend","kill":1,"resistDamage":1,"damageTaken":1}
                ]},
                {"teammateList": [
                    {"bnetId":"self","name":"当前玩家","friendBnetIds":["friend-1"],"heroGuid":"hero-self"},
                    {"bnetId":"friend-1","name":"测试好友","heroGuid":"hero-friend","kill":10,"assist":5,"death":2,"heroDamage":1000,"cure":100,"resistDamage":70,"healingTaken":80,"damageTaken":200}
                ]}
            ]
        }}
    });
    let conn = db.connect().unwrap();
    conn.execute("UPDATE matches SET result=?2 WHERE match_id=?1", params![id, result]).unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id,match_kind,raw_detail_json,updated_at) VALUES (?1,'fight',?2,'2026-07-16T00:00:00Z')",
        params![id, detail.to_string()],
    ).unwrap();
}

#[test]
fn friend_collaboration_combines_normal_and_both_arena_modes_from_last_round() {
    let db = test_db();
    seed_current_player(&db);
    seed_normal_friend_match(&db, "normal-1", 20);
    seed_arena_friend_match(&db, "competitive-1", 20, "SportFight", "win");
    seed_arena_friend_match(&db, "competitive-2", 20, "SportFight", "loss");
    seed_arena_friend_match(&db, "quick-1", 20, "LeisureFight", "win");
    seed_arena_friend_match(&db, "quick-2", 20, "LeisureFight", "loss");

    let analytics = owstats_desktop_lib::get_friend_analytics(
        &db,
        owstats_desktop_lib::FriendAnalyticsFilters { season: Some(20), ..Default::default() },
    ).unwrap();
    let friend = analytics.common_friend_heroes.iter().find(|item| item.bnet_id == "friend-1").unwrap();
    assert_eq!(friend.matches, 5);
    assert_eq!(friend.intimacy_score, 25);
    let front = analytics.boards.iter().find(|board| board.key == "front_pressure").unwrap();
    let entry = front.friends.iter().find(|item| item.bnet_id == "friend-1").unwrap();
    assert_eq!(entry.avg_damage_taken, 180.0);
    assert_eq!(entry.avg_mitigation, 66.0);
}
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test --test review_arena_collaboration friend_collaboration_combines_normal_and_both_arena_modes_from_last_round -- --exact
```

Expected: FAIL；当前只统计 1 场普通对局，好友未达到五场模型榜单门槛。

- [ ] **Step 3: 创建角斗解析模块**

在 `analytics/mod.rs` 增加：

```rust
mod arena;
```

在 `analytics/arena.rs` 实现以下边界；函数签名必须保持一致，具体 SQL 参数继续使用 `ToSql` 绑定，不拼接用户值：

```rust
use super::{begin_bound, non_empty, FriendMatchRow, LineupRow};
use crate::db::OwDatabase;
use crate::importer::{int_field, string_field};
use crate::models::FriendAnalyticsFilters;
use anyhow::Result;
use rusqlite::{OptionalExtension, ToSql};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct ArenaAnalyticsRows {
    pub(super) friend_rows: Vec<FriendMatchRow>,
    pub(super) lineup_rows: Vec<LineupRow>,
}

pub(super) fn arena_analytics_rows(
    db: &OwDatabase,
    filters: &FriendAnalyticsFilters,
) -> Result<ArenaAnalyticsRows> {
    let conn = db.connect()?;
    let Some(current_bnet) = current_player_bnet(&conn)? else {
        return Ok(ArenaAnalyticsRows::default());
    };
    let hero_names = hero_names(&conn)?;
    let mut sql = String::from(
        "SELECT m.match_id,m.result,m.begin_ts,COALESCE(m.map_name,''),m.game_mode,d.raw_detail_json FROM matches m JOIN match_details d ON d.match_id=m.match_id WHERE m.bucket='stadium' AND m.game_mode IN ('SportFight','LeisureFight')",
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    append_match_filters(&mut sql, &mut owned, filters);
    sql.push_str(" ORDER BY m.begin_ts,m.match_id");
    let params = owned.iter().map(|value| value.as_ref() as &dyn ToSql).collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let matches = stmt.query_map(params.as_slice(), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?))
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    let mut output = ArenaAnalyticsRows::default();
    for (match_id, result, begin_ts, map_name, _mode, raw) in matches {
        let Ok(detail) = serde_json::from_str::<Value>(&raw) else { continue };
        let Some(round) = last_valid_round(&detail, &current_bnet) else { continue };
        append_round_rows(&mut output, &match_id, &result, begin_ts, &map_name, round, &current_bnet, &hero_names, filters);
    }
    Ok(output)
}
```

同一文件实现这些完整规则：

- `current_player_bnet`：先取 `players` 最新非空 `bnet_id`；没有时回退到 `match_players.is_current_player=1` 的最新非空值。
- `hero_names`：读取 `hero_dict(guid,name)` 到 `HashMap<String,String>`。
- `append_match_filters`：绑定 `season`、`map`、`result`、`mode`、`date_from`、`date_to`；`mode` 不是 `SportFight` 或 `LeisureFight` 时查询自然返回零角斗行。
- `last_valid_round`：从 `roundCountList` 反向查找对象，要求非空 `teammateList` 且其中存在 `bnetId == current_bnet`。
- `append_round_rows`：读取当前玩家的 `friendBnetIds`，与同回合队友按 `bnetId` 求交集并用 `HashSet` 去重；每个好友构造一条 `FriendMatchRow`，当前玩家与好友各构造一条 `LineupRow`。
- `hero` 筛选在解析后按好友最后回合英雄执行。
- `group_size = 1 + 本局去重后的好友数`。
- 好友统计字段分别使用 `kill`、`assist`、`death`、`heroDamage`、`cure`、`resistDamage`、`damageTaken`、`healingTaken`。
- 名称缺失、等于 ID 或只含长数字时分别使用“当前玩家”“未知好友”；英雄字典缺失时使用“未知英雄”，绝不把 GUID 当名称。

使用以下 helper 实现上述规则；`append_round_rows` 对每个已去重好友直接构造父模块既有的 `FriendMatchRow` 与 `LineupRow`，不得引入新的前端 DTO：

```rust
fn current_player_bnet(conn: &rusqlite::Connection) -> Result<Option<String>> {
    let profile = conn.query_row(
        "SELECT bnet_id FROM players WHERE TRIM(COALESCE(bnet_id,''))<>'' ORDER BY updated_at DESC LIMIT 1",
        [],
        |row| row.get(0),
    ).optional()?;
    if profile.is_some() {
        return Ok(profile);
    }
    conn.query_row(
        "SELECT bnet_id FROM match_players WHERE is_current_player=1 AND TRIM(bnet_id)<>'' ORDER BY updated_at DESC LIMIT 1",
        [],
        |row| row.get(0),
    ).optional().map_err(Into::into)
}

fn hero_names(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT guid,name FROM hero_dict WHERE TRIM(name)<>''")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>().map_err(Into::into)
}

fn append_match_filters(
    sql: &mut String,
    owned: &mut Vec<Box<dyn ToSql>>,
    filters: &FriendAnalyticsFilters,
) {
    if let Some(season) = filters.season {
        sql.push_str(" AND m.season=?");
        owned.push(Box::new(season));
    }
    if let Some(map) = non_empty(&filters.map) {
        sql.push_str(" AND (m.map_guid=? OR m.map_name=?)");
        owned.push(Box::new(map.clone()));
        owned.push(Box::new(map));
    }
    if let Some(result) = non_empty(&filters.result) {
        sql.push_str(" AND m.result=?");
        owned.push(Box::new(result));
    }
    if let Some(mode) = non_empty(&filters.mode) {
        sql.push_str(" AND m.game_mode=?");
        owned.push(Box::new(mode));
    }
    if let Some(from) = non_empty(&filters.date_from).and_then(|date| begin_bound(&date, false)) {
        sql.push_str(" AND m.begin_ts>=?");
        owned.push(Box::new(from));
    }
    if let Some(to) = non_empty(&filters.date_to).and_then(|date| begin_bound(&date, true)) {
        sql.push_str(" AND m.begin_ts<=?");
        owned.push(Box::new(to));
    }
}

fn last_valid_round<'a>(detail: &'a Value, current_bnet: &str) -> Option<&'a Value> {
    detail.pointer("/payload/data/roundCountList")
        .and_then(Value::as_array)?
        .iter()
        .rev()
        .find(|round| {
            round.get("teammateList")
                .and_then(Value::as_array)
                .is_some_and(|players| players.iter().any(|player| string_field(player, "bnetId").as_deref() == Some(current_bnet)))
        })
}

fn scalar_id(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn looks_like_private_id(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 8 && trimmed.chars().all(|character| character.is_ascii_digit())
}

fn safe_name(player: &Value, bnet_id: &str, fallback: &str) -> String {
    string_field(player, "name")
        .filter(|name| name != bnet_id && !looks_like_private_id(name))
        .unwrap_or_else(|| fallback.to_string())
}

fn hero_identity(player: &Value, hero_names: &HashMap<String, String>) -> (String, String) {
    let guid = string_field(player, "heroGuid").unwrap_or_default();
    let name = hero_names.get(&guid).cloned().unwrap_or_else(|| "未知英雄".to_string());
    (guid, name)
}

fn append_round_rows(
    output: &mut ArenaAnalyticsRows,
    match_id: &str,
    result: &str,
    begin_ts: i64,
    map_name: &str,
    round: &Value,
    current_bnet: &str,
    hero_names: &HashMap<String, String>,
    filters: &FriendAnalyticsFilters,
) {
    let Some(teammates) = round.get("teammateList").and_then(Value::as_array) else { return };
    let Some(current) = teammates.iter().find(|player| string_field(player, "bnetId").as_deref() == Some(current_bnet)) else { return };
    let friend_ids = current.get("friendBnetIds")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(scalar_id).collect::<HashSet<_>>())
        .unwrap_or_default();
    if friend_ids.is_empty() {
        return;
    }
    let mut friends = HashMap::<String, &Value>::new();
    for player in teammates {
        let Some(bnet_id) = string_field(player, "bnetId") else { continue };
        if bnet_id != current_bnet && friend_ids.contains(&bnet_id) {
            friends.entry(bnet_id).or_insert(player);
        }
    }
    let group_size = 1 + friends.len() as i64;
    let mut selected = friends.into_iter().filter_map(|(bnet_id, player)| {
        let (hero_guid, hero_name) = hero_identity(player, hero_names);
        if let Some(hero) = non_empty(&filters.hero) {
            if hero != hero_guid && hero != hero_name {
                return None;
            }
        }
        Some((bnet_id, player, hero_guid, hero_name))
    }).collect::<Vec<_>>();
    selected.sort_by(|left, right| left.0.cmp(&right.0));
    if selected.is_empty() {
        return;
    }
    let (current_hero_guid, current_hero_name) = hero_identity(current, hero_names);
    output.lineup_rows.push(LineupRow {
        match_id: match_id.to_string(), result: result.to_string(), begin_ts,
        bnet_id: current_bnet.to_string(), name: safe_name(current, current_bnet, "当前玩家"),
        hero_guid: current_hero_guid, hero_name: current_hero_name,
        is_current_player: true, is_friend: false,
    });
    for (bnet_id, player, hero_guid, hero_name) in selected {
        let name = safe_name(player, &bnet_id, "未知好友");
        output.friend_rows.push(FriendMatchRow {
            bnet_id: bnet_id.clone(), name: name.clone(), match_id: match_id.to_string(),
            result: result.to_string(), begin_ts, map_name: map_name.to_string(),
            hero_guid: hero_guid.clone(), hero_name: hero_name.clone(),
            kills: int_field(player, "kill"), assists: int_field(player, "assist"),
            deaths: int_field(player, "death"), damage: int_field(player, "heroDamage"),
            healing: int_field(player, "cure"), mitigation: int_field(player, "resistDamage"),
            damage_taken: int_field(player, "damageTaken"), healing_taken: int_field(player, "healingTaken"),
            group_size,
        });
        output.lineup_rows.push(LineupRow {
            match_id: match_id.to_string(), result: result.to_string(), begin_ts,
            bnet_id, name, hero_guid, hero_name, is_current_player: false, is_friend: true,
        });
    }
}
```

- [ ] **Step 4: 合并角斗好友行进入现有聚合器**

在 `get_friend_analytics` 完成普通投影修复后加入：

```rust
let arena_rows = arena::arena_analytics_rows(db, &filters)?;
rows.extend(arena_rows.friend_rows);
```

保留后续 `aggregate_rows`、`compute_intimacy_scores`、`build_boards` 和 `common_friend_heroes` 原样，使现有门槛与公式自动作用于合并数据。

- [ ] **Step 5: 运行目标测试并确认 GREEN**

Run:

```powershell
cargo test --test review_arena_collaboration friend_collaboration_combines_normal_and_both_arena_modes_from_last_round -- --exact
```

Expected: PASS；`avg_damage_taken=180`、`avg_mitigation=66` 证明读取的是最后回合，而不是 `totalCount` 或早期回合。

- [ ] **Step 6: 提交 Task 3**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/analytics/arena.rs apps/owstats-desktop/src-tauri/src/analytics/mod.rs apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs
git commit -m "feat: include arena rows in friend analytics"
```

### Task 4: 高频组合合并角斗成员

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/analytics/mod.rs:490-620`
- Modify: `apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs`

- [ ] **Step 1: 给目标测试增加高频组合断言**

在三模式合并测试末尾增加：

```rust
let combo = analytics.lineup_combinations.iter().find(|item| item.matches == 5).unwrap();
assert_eq!(combo.members.len(), 2);
assert!(combo.members.iter().any(|member| member.is_current_player && member.hero_name == "当前英雄"));
assert!(combo.members.iter().any(|member| !member.is_current_player && member.hero_name == "好友英雄"));
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cargo test --test review_arena_collaboration friend_collaboration_combines_normal_and_both_arena_modes_from_last_round -- --exact
```

Expected: FAIL；现有 `lineup_combinations` 只有 1 场普通样本，未达到 3 场门槛。

- [ ] **Step 3: 让组合聚合接收额外角斗行**

将函数签名改为：

```rust
fn lineup_combinations(
    db: &OwDatabase,
    filters: &FriendAnalyticsFilters,
    mut extra_rows: Vec<LineupRow>,
) -> Result<Vec<LineupCombination>> {
    let mut rows = lineup_rows(db, filters)?;
    rows.append(&mut extra_rows);
```

函数剩余分组、key、三场门槛、胜率和排序代码保持原样。在 `get_friend_analytics` 中先取出两类角斗行再调用：

```rust
let arena_rows = arena::arena_analytics_rows(db, &filters)?;
rows.extend(arena_rows.friend_rows);
let lineup_combinations = lineup_combinations(db, &filters, arena_rows.lineup_rows)?;
```

返回 DTO 时使用局部变量：

```rust
lineup_combinations,
```

- [ ] **Step 4: 运行目标测试并确认 GREEN**

Run:

```powershell
cargo test --test review_arena_collaboration friend_collaboration_combines_normal_and_both_arena_modes_from_last_round -- --exact
```

Expected: PASS；组合累计 5 场且两名成员英雄来自最后有效回合。

- [ ] **Step 5: 提交 Task 4**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/analytics/mod.rs apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs
git commit -m "feat: include arena matches in friend lineups"
```

### Task 5: 锁定角斗排除、去重、筛选与赛季隔离

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/analytics/arena.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs`

- [ ] **Step 1: 写排除与隔离失败测试**

增加三个测试：

```rust
#[test]
fn arena_friend_rows_require_current_player_friend_ids_and_valid_last_round() {
    let db = test_db();
    seed_current_player(&db);
    for index in 0..3 {
        seed_arena_friend_match(&db, &format!("valid-{index}"), 20, "SportFight", "win");
        seed_arena_friend_match(&db, &format!("other-season-{index}"), 15, "LeisureFight", "win");
    }
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE match_details SET raw_detail_json=json_remove(raw_detail_json,'$.payload.data.roundCountList[1].teammateList[0].friendBnetIds') WHERE match_id LIKE 'other-season-%'",
        [],
    ).unwrap();
    let analytics = owstats_desktop_lib::get_friend_analytics(
        &db,
        owstats_desktop_lib::FriendAnalyticsFilters { season: Some(20), ..Default::default() },
    ).unwrap();
    assert_eq!(analytics.common_friend_heroes.len(), 1);
    assert_eq!(analytics.common_friend_heroes[0].matches, 3);
}

#[test]
fn arena_friend_rows_deduplicate_same_friend_within_one_round() {
    let db = test_db();
    seed_current_player(&db);
    for index in 0..3 {
        seed_arena_friend_match(&db, &format!("duplicate-{index}"), 20, "SportFight", "win");
    }
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE match_details SET raw_detail_json=json_insert(raw_detail_json,'$.payload.data.roundCountList[1].teammateList[#]',json_extract(raw_detail_json,'$.payload.data.roundCountList[1].teammateList[1]')) WHERE match_id LIKE 'duplicate-%'",
        [],
    ).unwrap();
    let analytics = owstats_desktop_lib::get_friend_analytics(
        &db,
        owstats_desktop_lib::FriendAnalyticsFilters { season: Some(20), ..Default::default() },
    ).unwrap();
    assert_eq!(analytics.common_friend_heroes[0].matches, 3);
    assert_eq!(analytics.common_friend_heroes[0].intimacy_score, 15);
}

#[test]
fn arena_friend_rows_obey_mode_and_hero_filters() {
    let db = test_db();
    seed_current_player(&db);
    for index in 0..3 {
        seed_arena_friend_match(&db, &format!("competitive-{index}"), 20, "SportFight", "win");
        seed_arena_friend_match(&db, &format!("quick-{index}"), 20, "LeisureFight", "win");
    }
    let analytics = owstats_desktop_lib::get_friend_analytics(
        &db,
        owstats_desktop_lib::FriendAnalyticsFilters {
            season: Some(20), mode: Some("LeisureFight".into()), hero: Some("hero-friend".into()), ..Default::default()
        },
    ).unwrap();
    assert_eq!(analytics.common_friend_heroes[0].matches, 3);
}
```

- [ ] **Step 2: 运行测试并确认 RED 或已覆盖行为**

Run:

```powershell
cargo test --test review_arena_collaboration arena_friend_rows_ -- --nocapture
```

Expected: 新测试至少有一项失败，最常见是重复好友被重复计数或 hero/mode 过滤尚未完整执行。若某项立即通过，保留它作为回归合同，但必须确认其他新增断言确实 RED。

- [ ] **Step 3: 完成最小排除、去重和筛选实现**

在 `append_round_rows` 中先把好友收集到以 `bnetId` 为 key 的 `HashMap`，再生成行；`friendBnetIds` 缺失与空数组都返回零行。hero 过滤使用以下判断，不匹配则跳过该好友：

```rust
if let Some(hero) = non_empty(&filters.hero) {
    if hero_guid != hero && hero_name != hero {
        continue;
    }
}
```

SQL 中 mode 条件必须与已有 `m.game_mode = ?` 合并，且基础条件始终保留 `IN ('SportFight','LeisureFight')`。season 条件必须绑定到 `m.season`。

- [ ] **Step 4: 运行新增测试与好友分析既有回归**

Run:

```powershell
cargo test --test review_arena_collaboration
cargo test --test phase8_multi_season_pages
cargo test --test import_baseline friend_analytics_ -- --nocapture
```

Expected: PASS；普通好友分析测试不改变既有结果。

- [ ] **Step 5: 提交 Task 5**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/analytics/arena.rs apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs
git commit -m "test: harden arena friend analytics boundaries"
```

### Task 6: 全量验证与交付检查

**Files:**
- Verify only; do not add unrelated files.

- [ ] **Step 1: 格式化并检查 Rust**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt
cargo fmt --check
```

Expected: PASS。只格式化本计划实际修改的 Rust 文件；若 `cargo fmt` 触及无关用户文件，立即停止并只保留本计划文件的格式变化。

- [ ] **Step 2: 运行后端全量测试**

Run:

```powershell
cargo test --tests
```

Expected: PASS，无失败测试。

- [ ] **Step 3: 运行前端全量测试与构建**

Run:

```powershell
cd ..
npm test
npm run build
```

Expected: PASS；用户可见文本完整，TypeScript 无错误，Vite 构建成功。

- [ ] **Step 4: 检查 diff 与工作树边界**

Run:

```powershell
cd ..\..
git diff --check
git status --short
git diff --name-only HEAD~5..HEAD
```

Expected: `git diff --check` 无输出；本功能提交只包含本计划列出的文件。现有其他未提交文件保持原状，不暂存、不提交。

- [ ] **Step 5: 如格式化产生计划内未提交变化则提交**

```powershell
git add -- apps/owstats-desktop/src-tauri/src/analytics/arena.rs apps/owstats-desktop/src-tauri/src/analytics/mod.rs apps/owstats-desktop/src-tauri/src/review/personal.rs apps/owstats-desktop/src-tauri/src/models.rs apps/owstats-desktop/src-tauri/tests/review_arena_collaboration.rs apps/owstats-desktop/src/shared/types.ts apps/owstats-desktop/src/pages/PersonalReviewPage.tsx apps/owstats-desktop/scripts/phase8MultiSeasonPages.test.ts apps/owstats-desktop/scripts/reviewAchievements.test.ts
git diff --cached --check
git commit -m "chore: finalize review collaboration verification"
```

Expected: 只有确有计划内未提交变化时才创建提交；否则跳过本步。

## 计划自检

- 规格覆盖：动态赛季、无效 URL 回退、角斗竞技、角斗快速、亲密度、高频组合、最后回合模型、好友识别、去重、筛选、赛季隔离、隐私和普通模式回归均有对应任务。
- 范围：不修改指挥台、好友详情、SQLite schema、导入器或普通 `match_players` 语义。
- 类型一致：Rust 使用 `available_seasons`，serde/TypeScript 使用 `availableSeasons`；角斗解析统一返回 `ArenaAnalyticsRows { friend_rows, lineup_rows }`。
- TDD：每个行为改动都先增加失败测试并单独运行确认 RED，再写最小实现确认 GREEN。
