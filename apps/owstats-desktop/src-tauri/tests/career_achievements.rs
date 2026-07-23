use chrono::{Local, TimeZone};
use owstats_desktop_lib::{get_career_achievements, OwDatabase};
use rusqlite::params;

#[allow(clippy::too_many_arguments)]
fn seed_match(
    db: &OwDatabase,
    id: &str,
    season: i64,
    result: &str,
    begin_ts: i64,
    kills: i64,
    assists: i64,
    deaths: i64,
    damage: i64,
    duration_sec: i64,
    rank_name: &str,
    detail: &str,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"INSERT INTO matches (
          match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
          hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
          deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        ) VALUES (?1, ?2, 'preset', 'SportPreset', 'normal', ?3, 'map', '地图',
          'hero', '英雄', ?4, 1, 0, ?5, ?6, ?7, ?8, 0, 0, 0, ?9, ?10, 'synthetic')"#,
        params![
            id,
            season,
            result,
            begin_ts,
            kills,
            assists,
            deaths,
            damage,
            duration_sec,
            serde_json::json!({"rankInfo": {"rankName": rank_name}}).to_string()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'normal', ?2, 'synthetic')",
        params![id, detail],
    ).unwrap();
}

fn seed_player(
    db: &OwDatabase,
    match_id: &str,
    bnet: &str,
    current: bool,
    friend: bool,
    damage_taken: i64,
) {
    db.connect().unwrap().execute(
        r#"INSERT INTO match_players (
          match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
          is_current_player, is_friend, kill, assist, death, hero_damage, damage_taken, raw_json, updated_at
        ) VALUES (?1, 22, 'teammate', ?2, '合成玩家', 'hero', '英雄', ?3, ?4,
          (SELECT kills FROM matches WHERE match_id=?1),
          (SELECT assists FROM matches WHERE match_id=?1),
          (SELECT deaths FROM matches WHERE match_id=?1),
          (SELECT damage FROM matches WHERE match_id=?1), ?5, '{}', 'synthetic')"#,
        params![match_id, bnet, i64::from(current), i64::from(friend), damage_taken],
    ).unwrap();
}

fn seed_evaluation(
    db: &OwDatabase,
    match_id: &str,
    grade: &str,
    primary_tag: &str,
    evidence_tags: &[&str],
) {
    db.connect()
        .unwrap()
        .execute(
            r#"INSERT INTO match_evaluations (
              match_id, mode, rule_version, grade, primary_tag, evidence_tags_json,
              reasons_json, confidence, computed_at
            ) VALUES (?1, 'SportPreset', 1, ?2, ?3, ?4, '[]', 1.0, 'synthetic')"#,
            params![
                match_id,
                grade,
                primary_tag,
                serde_json::to_string(evidence_tags).unwrap()
            ],
        )
        .unwrap();
}

fn set_current_hero(db: &OwDatabase, match_id: &str, hero: &str) {
    db.connect()
        .unwrap()
        .execute(
            "UPDATE match_players SET hero_guid = ?2, hero_name = ?2 WHERE match_id = ?1 AND is_current_player = 1",
            params![match_id, hero],
        )
        .unwrap();
}

fn local_day(year: i32, month: u32, day: u32) -> i64 {
    local_time(year, month, day, 12, 0)
}

fn local_time(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
    Local
        .with_ymd_and_hms(year, month, day, hour, minute, 0)
        .single()
        .unwrap()
        .timestamp()
}

fn seed_stadium_match(db: &OwDatabase, id: &str, begin_ts: i64, total_worth: i64, detail: &str) {
    seed_match(db, id, 23, "win", begin_ts, 1, 1, 1, 1, 600, "Gold", detail);
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE matches SET bucket='stadium', game_mode='SportFight', total_worth=?2 WHERE match_id=?1",
        params![id, total_worth],
    )
    .unwrap();
    seed_player(db, id, "current-player", true, false, 1);
}

#[test]
fn career_achievement_contract_is_stable() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("career-achievements.sqlite3"));
    db.initialize().unwrap();

    let achievements = get_career_achievements(&db).unwrap();
    let keys = achievements
        .iter()
        .map(|achievement| achievement.key.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        keys,
        [
            "win-streak",
            "s-grade",
            "carry",
            "signature",
            "hero-focus",
            "switching",
            "glass-cannon",
            "damage-machine",
            "bloodbath",
            "comeback",
            "godlike-run",
            "iron-wall",
            "rank-proof",
            "battle-hardened",
            "brothers-in-arms",
            "best-teammate",
            "unwavering",
            "wealth-tycoon",
            "stadium-ascendant",
        ]
    );
    assert!(achievements.iter().all(|achievement| {
        achievement.tiers.len() == 3
            && achievement
                .tiers
                .iter()
                .all(|tier| tier.unlocked_at.is_none())
    }));
    assert!(!keys.contains(&"peak"));
    assert!(!keys.contains(&"win-rate"));
    let achievement = |key: &str| achievements.iter().find(|item| item.key == key).unwrap();
    assert_eq!(
        achievement("win-streak")
            .tiers
            .iter()
            .map(|tier| tier.requirement.as_str())
            .collect::<Vec<_>>(),
        [
            "单赛季最长连胜达到 5 局",
            "单赛季最长连胜达到 10 局",
            "单赛季最长连胜达到 15 局"
        ]
    );
    assert_eq!(
        achievement("comeback").tiers[0].requirement,
        "完成 1 场角斗领域的让三追四"
    );
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
        achievement("wealth-tycoon").detail,
        "角斗竞技最后一回合资金"
    );
    assert_eq!(
        achievement("wealth-tycoon")
            .tiers
            .iter()
            .map(|tier| tier.requirement.as_str())
            .collect::<Vec<_>>(),
        [
            "最后一回合结束时资金达到 80,000",
            "最后一回合结束时资金达到 90,000",
            "最后一回合结束时资金达到 100,000",
        ]
    );
    assert_eq!(achievement("stadium-ascendant").title, "登峰造极");
    assert_eq!(
        achievement("stadium-ascendant").detail,
        "角斗竞技任意职责最高段位"
    );
    assert_eq!(
        achievement("stadium-ascendant")
            .tiers
            .iter()
            .map(|tier| tier.requirement.as_str())
            .collect::<Vec<_>>(),
        ["达到专家", "达到全明星", "达到传奇"]
    );
}

#[test]
fn unwavering_deduplicates_local_days_resets_gaps_and_crosses_seasons() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("unwavering.sqlite3"));
    db.initialize().unwrap();
    for day in 1..=9 {
        let id = format!("first-{day}");
        seed_match(
            &db,
            &id,
            22,
            "win",
            local_day(2026, 6, day),
            1,
            1,
            1,
            1,
            600,
            "Gold",
            "{}",
        );
    }
    seed_match(
        &db,
        "same-day",
        22,
        "win",
        local_day(2026, 6, 9) + 60,
        1,
        1,
        1,
        1,
        600,
        "Gold",
        "{}",
    );
    for offset in 0..30 {
        let id = format!("second-{offset}");
        seed_match(
            &db,
            &id,
            if offset < 15 { 22 } else { 23 },
            "win",
            local_day(2026, 6, 11) + offset * 86_400,
            1,
            1,
            1,
            1,
            600,
            "Gold",
            "{}",
        );
    }
    let achievements = get_career_achievements(&db).unwrap();
    let achievement = achievements
        .iter()
        .find(|item| item.key == "unwavering")
        .unwrap();
    assert_eq!(achievement.current_level, 3);
    assert_eq!(
        achievement
            .tiers
            .iter()
            .map(|tier| tier.unlocked_at)
            .collect::<Vec<_>>(),
        [9, 19, 29].map(|offset| Some(local_day(2026, 6, 11) + offset * 86_400))
    );
}

#[test]
fn unwavering_counts_matches_across_local_midnight_as_adjacent_days() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("unwavering-midnight.sqlite3"));
    db.initialize().unwrap();
    seed_match(
        &db,
        "before-midnight",
        22,
        "win",
        local_time(2026, 7, 1, 23, 59),
        1,
        1,
        1,
        1,
        600,
        "Gold",
        "{}",
    );
    for day in 2..=10 {
        let id = format!("midnight-day-{day}");
        seed_match(
            &db,
            &id,
            22,
            "win",
            if day == 2 {
                local_time(2026, 7, day, 0, 1)
            } else {
                local_day(2026, 7, day)
            },
            1,
            1,
            1,
            1,
            600,
            "Gold",
            "{}",
        );
    }
    let achievements = get_career_achievements(&db).unwrap();
    assert_eq!(
        achievements
            .iter()
            .find(|item| item.key == "unwavering")
            .unwrap()
            .tiers[0]
            .unlocked_at,
        Some(local_day(2026, 7, 10))
    );
}

#[test]
fn stadium_wealth_uses_only_last_round_current_player_and_never_falls_back() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("stadium-wealth.sqlite3"));
    db.initialize().unwrap();
    seed_stadium_match(
        &db,
        "previous-only",
        100,
        0,
        r#"{"payload":{"data":{"roundCountList":[{"teammateList":[{"bnetId":"current-player","endWorth":120000}]},{"teammateList":[{"bnetId":"current-player","endWorth":79999}]}]}}}"#,
    );
    seed_stadium_match(
        &db,
        "teammate-only",
        200,
        0,
        r#"{"data":{"roundCountList":[{"teammateList":[{"bnetId":"current-player","endWorth":79999},{"bnetId":"teammate","endWorth":120000}]}]}}"#,
    );
    seed_stadium_match(&db, "missing-detail", 300, 120000, "{}");
    seed_stadium_match(
        &db,
        "snake-case-worth",
        310,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":"current-player","end_worth":100000}]}]}"#,
    );
    seed_stadium_match(
        &db,
        "total-count-only",
        320,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":"current-player","totalCount":120000}]}]}"#,
    );
    seed_stadium_match(
        &db,
        "wrong-bucket",
        330,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":"current-player","endWorth":100000}]}]}"#,
    );
    db.connect()
        .unwrap()
        .execute(
            "UPDATE matches SET bucket='preset' WHERE match_id='wrong-bucket'",
            [],
        )
        .unwrap();
    seed_stadium_match(
        &db,
        "wrong-mode",
        340,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":"current-player","endWorth":100000}]}]}"#,
    );
    db.connect()
        .unwrap()
        .execute(
            "UPDATE matches SET game_mode='SportPreset' WHERE match_id='wrong-mode'",
            [],
        )
        .unwrap();
    let achievements = get_career_achievements(&db).unwrap();
    assert_eq!(
        achievements
            .iter()
            .find(|item| item.key == "wealth-tycoon")
            .unwrap()
            .current_level,
        0
    );

    seed_stadium_match(
        &db,
        "current-rich",
        400,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":"current-player","endWorth":100000}]}]}"#,
    );
    let achievements = get_career_achievements(&db).unwrap();
    let achievement = achievements
        .iter()
        .find(|item| item.key == "wealth-tycoon")
        .unwrap();
    assert_eq!(achievement.current_level, 3);
    assert!(achievement
        .tiers
        .iter()
        .all(|tier| tier.unlocked_at == Some(400)));
}

#[test]
fn stadium_numeric_bnet_id_drives_wealth_and_trimmed_rank() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("stadium-numeric-bnet.sqlite3"));
    db.initialize().unwrap();
    seed_stadium_match(
        &db,
        "numeric-bnet",
        500,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":12345,"endWorth":100000,"rankInfo":{"rankName":" Grandmaster "}}]}]}"#,
    );
    db.connect()
        .unwrap()
        .execute(
            "UPDATE match_players SET bnet_id='12345' WHERE match_id='numeric-bnet' AND is_current_player=1",
            [],
        )
        .unwrap();

    let achievements = get_career_achievements(&db).unwrap();
    assert_eq!(
        achievements
            .iter()
            .find(|item| item.key == "wealth-tycoon")
            .unwrap()
            .current_level,
        3
    );
    assert_eq!(
        achievements
            .iter()
            .find(|item| item.key == "stadium-ascendant")
            .unwrap()
            .current_level,
        3
    );
}

#[test]
fn stadium_rank_uses_last_round_current_player_across_roles_only() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("stadium-rank.sqlite3"));
    db.initialize().unwrap();
    seed_match(
        &db,
        "normal-gm",
        23,
        "win",
        50,
        1,
        1,
        1,
        1,
        600,
        "Grandmaster",
        "{}",
    );
    seed_player(&db, "normal-gm", "current-player", true, false, 1);
    seed_stadium_match(
        &db,
        "teammate-gm",
        60,
        0,
        r#"{"data":{"roundCountList":[{"teammateList":[{"bnetId":"current-player"},{"bnetId":"other","rankInfo":{"rankName":"Grandmaster"}}]}]}}"#,
    );
    seed_stadium_match(
        &db,
        "missing",
        70,
        0,
        r#"{"data":{"roundCountList":[{"teammateList":[{"bnetId":"current-player"}]}]}}"#,
    );
    seed_stadium_match(
        &db,
        "previous-gm-last-low",
        80,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":"current-player","rankInfo":{"rankName":"Grandmaster"}}]},{"teammateList":[{"bnetId":"current-player","rankInfo":{"rankName":"Gold"}}]}]}"#,
    );
    seed_stadium_match(
        &db,
        "previous-gm-last-missing",
        90,
        0,
        r#"{"roundCountList":[{"teammateList":[{"bnetId":"current-player","rankInfo":{"rankName":"Grandmaster"}}]},{"teammateList":[{"bnetId":"current-player"}]}]}"#,
    );
    let achievements = get_career_achievements(&db).unwrap();
    assert_eq!(
        achievements
            .iter()
            .find(|item| item.key == "stadium-ascendant")
            .unwrap()
            .current_level,
        0
    );
    for (index, rank) in ["Diamond", "Master", "Grandmaster"].into_iter().enumerate() {
        let detail = match index {
            0 => serde_json::json!({"roundCountList":[{"teammateList":[{"bnetId":"current-player","roleType":index,"rankInfo":{"rank_name":rank}}]}]}),
            _ => serde_json::json!({"payload":{"data":{"roundCountList":[{"teammateList":[{"bnetId":"current-player","roleType":index,"rankInfo":{"rankName":rank}}]}]}}}),
        }
        .to_string();
        seed_stadium_match(
            &db,
            &format!("rank-{index}"),
            100 + index as i64,
            0,
            &detail,
        );
    }
    let achievements = get_career_achievements(&db).unwrap();
    let achievement = achievements
        .iter()
        .find(|item| item.key == "stadium-ascendant")
        .unwrap();
    assert_eq!(achievement.current_level, 3);
    assert_eq!(
        achievement
            .tiers
            .iter()
            .map(|tier| tier.unlocked_at)
            .collect::<Vec<_>>(),
        [Some(100), Some(101), Some(102)]
    );
}

#[test]
fn career_achievements_skip_player_metrics_when_current_player_projection_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(
        temp.path()
            .join("career-achievements-missing-player.sqlite3"),
    );
    db.initialize().unwrap();
    seed_match(
        &db,
        "missing-current-player",
        22,
        "win",
        100,
        50,
        50,
        0,
        99_000,
        600,
        "Champion",
        "{}",
    );

    let achievements =
        get_career_achievements(&db).expect("缺少当前玩家投影的历史对局不应导致整个成就页失败");
    let achievement = |key: &str| achievements.iter().find(|item| item.key == key).unwrap();

    assert_eq!(achievement("hero-focus").current_level, 0);
    assert_eq!(achievement("switching").current_level, 0);
    assert_eq!(achievement("glass-cannon").current_level, 0);
}

#[test]
fn career_fact_projection_drives_single_match_rules_and_uses_current_player_damage_taken() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("career-facts.sqlite3"));
    db.initialize().unwrap();
    seed_match(
        &db,
        "later",
        23,
        "win",
        200,
        25,
        31,
        0,
        25_000,
        600,
        "Master",
        r#"{"rounds":[{"teamScore":0,"opponentScore":3}]}"#,
    );
    seed_player(&db, "later", "current", true, false, 8_000);
    seed_player(&db, "later", "friend-a", false, true, 99_000);
    seed_player(&db, "later", "friend-b", false, true, 99_000);
    seed_match(
        &db, "short", 22, "win", 100, 50, 50, 0, 99_000, 299, "Champion", "{}",
    );
    seed_player(&db, "short", "current", true, false, 20_000);

    let achievements = get_career_achievements(&db).unwrap();
    let achievement = |key: &str| achievements.iter().find(|item| item.key == key).unwrap();
    assert_eq!(achievement("glass-cannon").current_level, 3);
    assert_eq!(achievement("glass-cannon").tiers[2].unlocked_at, Some(200));
    assert_eq!(achievement("damage-machine").current_level, 3);
    assert_eq!(
        achievement("iron-wall").current_level,
        0,
        "299 秒对局不得参与每十分钟规则"
    );
    assert_eq!(achievement("bloodbath").current_level, 3);
    assert_eq!(achievement("godlike-run").current_level, 3);
    assert_eq!(achievement("best-teammate").current_level, 3);
    assert_eq!(achievement("comeback").tiers[0].unlocked_at, Some(200));
    assert_eq!(achievement("rank-proof").current_level, 3);
    assert_eq!(
        achievement("brothers-in-arms").current_level,
        0,
        "同场多名好友只能计一场"
    );
}

#[test]
fn revised_career_rules_use_final_hero_share_and_stable_signature_tag() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("revised-career-rules.sqlite3"));
    db.initialize().unwrap();

    for index in 0..15 {
        let id = format!("career-{index:02}");
        seed_match(
            &db,
            &id,
            22,
            "win",
            1_000 + index,
            5,
            5,
            1,
            5_000,
            600,
            "Gold",
            "{}",
        );
        seed_player(&db, &id, "current", true, false, 5_000);
        set_current_hero(&db, &id, if index < 8 { "hero-a" } else { "hero-b" });
        seed_evaluation(
            &db,
            &id,
            if index < 10 { "S" } else { "A" },
            if index < 10 { "带飞局" } else { "普通局" },
            if index < 10 {
                &["KDA最高"]
            } else {
                &["击杀最多"]
            },
        );
    }

    let achievements = get_career_achievements(&db).unwrap();
    let achievement = |key: &str| achievements.iter().find(|item| item.key == key).unwrap();

    assert_eq!(achievement("win-streak").current_level, 3);
    assert_eq!(achievement("s-grade").current_level, 1);
    assert_eq!(achievement("carry").current_level, 1);
    assert_eq!(achievement("signature").current_level, 1);
    assert_eq!(achievement("signature").detail, "KDA最高");
    assert_eq!(achievement("hero-focus").current_level, 2);
    assert!(achievement("hero-focus")
        .tiers
        .iter()
        .all(|tier| tier.unlocked_at.is_none()));
}
