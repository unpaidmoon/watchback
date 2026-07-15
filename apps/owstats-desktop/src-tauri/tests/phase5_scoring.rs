use owstats_desktop_lib::{
    get_friend_analytics, import_payload_value, list_matches, refresh_match_evaluations,
    FriendAnalyticsFilters, MatchFilters, OwDatabase,
};
use rusqlite::params;
use serde_json::{json, Value};

fn temp_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    OwDatabase::new(temp.keep().join("phase5.sqlite3"))
}

#[derive(Clone)]
struct PlayerSeed {
    bnet_id: &'static str,
    name: &'static str,
    hero_guid: &'static str,
    hero_name: &'static str,
    is_current: bool,
    is_friend: bool,
    kill: i64,
    assist: i64,
    death: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
}

#[allow(clippy::too_many_arguments)] // Test fixture mirrors all independent score inputs.
fn player(
    bnet_id: &'static str,
    name: &'static str,
    hero_guid: &'static str,
    is_current: bool,
    is_friend: bool,
    kill: i64,
    assist: i64,
    death: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
) -> PlayerSeed {
    PlayerSeed {
        bnet_id,
        name,
        hero_guid,
        hero_name: hero_guid,
        is_current,
        is_friend,
        kill,
        assist,
        death,
        damage,
        healing,
        mitigation,
    }
}

fn seed_match(
    db: &OwDatabase,
    match_id: &str,
    result: &str,
    begin_ts: i64,
    current: PlayerSeed,
    teammates: Vec<PlayerSeed>,
    enemies: Vec<PlayerSeed>,
) {
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'preset', 'SportPreset', 'normal', ?2, 'map-a', '漓江塔',
            ?3, ?4, ?5, 2, 1, ?6, ?7, ?8, ?9, ?10, ?11, 0, 600, '{}',
            '2026-06-28T00:00:00Z')
        "#,
        params![
            match_id,
            result,
            current.hero_guid,
            current.hero_name,
            begin_ts,
            current.kill,
            current.assist,
            current.death,
            current.damage,
            current.healing,
            current.mitigation
        ],
    )
    .unwrap();
    for item in std::iter::once(current.clone())
        .chain(teammates)
        .chain(enemies.clone())
    {
        let side = if enemies.iter().any(|enemy| enemy.bnet_id == item.bnet_id) {
            "enemy"
        } else {
            "teammate"
        };
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, kill, assist, death, hero_damage, cure,
                resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
                raw_json, updated_at
            )
            VALUES (?1, 22, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, 0, 0, 0, 0, '{}', '2026-06-28T00:00:00Z')
            "#,
            params![
                match_id,
                side,
                item.bnet_id,
                item.name,
                item.hero_guid,
                item.hero_name,
                if item.is_current { 1 } else { 0 },
                if item.is_friend { 1 } else { 0 },
                item.kill,
                item.assist,
                item.death,
                item.damage,
                item.healing,
                item.mitigation
            ],
        )
        .unwrap();
    }
}

fn standard_enemies() -> Vec<PlayerSeed> {
    vec![
        player(
            "e1", "Enemy 1", "enemy-a", false, false, 10, 0, 4, 6000, 0, 0,
        ),
        player(
            "e2", "Enemy 2", "enemy-b", false, false, 9, 0, 5, 5000, 0, 0,
        ),
    ]
}

fn standard_teammates() -> Vec<PlayerSeed> {
    vec![
        player(
            "t1", "Team 1", "ally-a", false, false, 8, 4, 4, 7000, 1000, 300,
        ),
        player(
            "t2", "Team 2", "ally-b", false, false, 7, 3, 5, 6000, 2000, 200,
        ),
    ]
}

#[test]
fn scoring_matrix_and_grade_rules_use_teammate_rankings() {
    let db = temp_db();
    seed_match(
        &db,
        "carry",
        "win",
        10,
        player(
            "me", "Me", "hero-me", true, false, 20, 10, 2, 12000, 3000, 500,
        ),
        standard_teammates(),
        standard_enemies(),
    );
    seed_match(
        &db,
        "try-hard",
        "loss",
        9,
        player(
            "me", "Me", "hero-me", true, false, 20, 10, 2, 12000, 3000, 500,
        ),
        standard_teammates(),
        standard_enemies(),
    );
    seed_match(
        &db,
        "free-win",
        "win",
        8,
        player("me", "Me", "hero-me", true, false, 1, 0, 9, 100, 0, 0),
        standard_teammates(),
        standard_enemies(),
    );
    seed_match(
        &db,
        "blame",
        "loss",
        7,
        player("me", "Me", "hero-me", true, false, 1, 0, 9, 100, 0, 0),
        standard_teammates(),
        standard_enemies(),
    );
    seed_match(
        &db,
        "smooth-win",
        "win",
        6,
        player("me", "Me", "hero-me", true, false, 6, 2, 3, 5000, 500, 100),
        standard_teammates(),
        standard_enemies(),
    );
    seed_match(
        &db,
        "close-loss",
        "loss",
        5,
        player("me", "Me", "hero-me", true, false, 6, 2, 3, 5000, 500, 100),
        standard_teammates(),
        standard_enemies(),
    );

    refresh_match_evaluations(&db).unwrap();
    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    let tag = |id: &str| {
        matches
            .iter()
            .find(|item| item.match_id == id)
            .unwrap()
            .evaluation
            .as_ref()
            .unwrap()
            .clone()
    };

    assert_eq!(tag("carry").primary_tag, "带飞局");
    assert_eq!(tag("carry").grade, "S");
    assert_eq!(tag("try-hard").primary_tag, "尽力局");
    assert_eq!(tag("free-win").primary_tag, "躺赢局");
    assert_eq!(tag("free-win").grade, "C");
    assert_eq!(tag("blame").primary_tag, "背锅局");
    assert_eq!(tag("smooth-win").primary_tag, "顺风局");
    assert_eq!(tag("smooth-win").grade, "B");
    assert_eq!(tag("close-loss").primary_tag, "惜败局");
}

#[test]
fn kda_zero_deaths_unique_highest_deaths_and_evidence_priority_are_handled() {
    let db = temp_db();
    seed_match(
        &db,
        "kda-zero",
        "win",
        1,
        player(
            "me", "Me", "hero-me", true, false, 5, 12, 0, 7000, 5000, 2000,
        ),
        vec![
            player(
                "t1", "Team 1", "ally-a", false, false, 8, 2, 2, 6000, 1000, 100,
            ),
            player(
                "t2", "Team 2", "ally-b", false, false, 7, 2, 3, 5000, 1000, 100,
            ),
        ],
        standard_enemies(),
    );
    seed_match(
        &db,
        "death-tied",
        "loss",
        2,
        player("me", "Me", "hero-me", true, false, 1, 0, 9, 100, 0, 0),
        vec![
            player(
                "t1", "Team 1", "ally-a", false, false, 8, 2, 9, 6000, 1000, 100,
            ),
            player(
                "t2", "Team 2", "ally-b", false, false, 7, 2, 3, 5000, 1000, 100,
            ),
        ],
        standard_enemies(),
    );

    refresh_match_evaluations(&db).unwrap();
    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    let kda = matches
        .iter()
        .find(|item| item.match_id == "kda-zero")
        .unwrap()
        .evaluation
        .as_ref()
        .unwrap();
    assert_eq!(kda.evidence_tags, vec!["KDA最高", "伤害最高"]);
    assert_eq!(kda.grade, "S");

    let tied = matches
        .iter()
        .find(|item| item.match_id == "death-tied")
        .unwrap()
        .evaluation
        .as_ref()
        .unwrap();
    assert_eq!(tied.primary_tag, "惜败局");
}

#[test]
fn comeback_close_match_fallback_unknown_result_and_primary_tag_filter_work() {
    let db = temp_db();
    seed_match(
        &db,
        "comeback",
        "win",
        1,
        player("me", "Me", "hero-me", true, false, 1, 0, 2, 6000, 0, 0),
        vec![player(
            "t1", "Team 1", "ally-a", false, false, 2, 0, 3, 5000, 0, 0,
        )],
        vec![player(
            "e1", "Enemy 1", "enemy-a", false, false, 20, 0, 4, 10000, 0, 0,
        )],
    );
    seed_match(
        &db,
        "close",
        "loss",
        2,
        player("me", "Me", "hero-me", true, false, 4, 0, 4, 4500, 0, 0),
        vec![player(
            "t1", "Team 1", "ally-a", false, false, 5, 1, 3, 5000, 0, 0,
        )],
        vec![player(
            "e1", "Enemy 1", "enemy-a", false, false, 7, 0, 4, 10500, 0, 0,
        )],
    );
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES ('missing-detail', 22, 'preset', 'SportPreset', 'normal', 'win', 'map-a', '漓江塔',
            'hero-me', 'hero-me', 3, 2, 1, 1, 1, 1, 100, 0, 0, 0, 600, '{}',
            '2026-06-28T00:00:00Z'),
            ('unknown', 22, 'preset', 'SportPreset', 'normal', 'draw', 'map-a', '漓江塔',
            'hero-me', 'hero-me', 4, 1, 1, 1, 1, 1, 100, 0, 0, 0, 600, '{}',
            '2026-06-28T00:00:00Z')
        "#,
        [],
    )
    .unwrap();
    drop(conn);

    refresh_match_evaluations(&db).unwrap();
    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    let evaluation = |id: &str| {
        matches
            .iter()
            .find(|item| item.match_id == id)
            .unwrap()
            .evaluation
            .clone()
    };

    assert!(evaluation("comeback")
        .unwrap()
        .evidence_tags
        .contains(&"翻盘局".to_string()));
    assert!(evaluation("close")
        .unwrap()
        .evidence_tags
        .contains(&"伯仲局".to_string()));
    assert_eq!(evaluation("missing-detail").unwrap().primary_tag, "胜利局");
    assert!(evaluation("unknown").is_none());

    let filtered = list_matches(
        &db,
        MatchFilters {
            primary_tag: Some("带飞局".to_string()),
            ..MatchFilters::default()
        },
    )
    .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].match_id, "comeback");
}

#[test]
fn stale_cache_versions_are_refreshed() {
    let db = temp_db();
    seed_match(
        &db,
        "versioned",
        "win",
        1,
        player(
            "me", "Me", "hero-me", true, false, 20, 10, 2, 12000, 3000, 500,
        ),
        standard_teammates(),
        standard_enemies(),
    );
    refresh_match_evaluations(&db).unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE match_evaluations SET rule_version=0, primary_tag='旧标签' WHERE match_id='versioned'",
        [],
    )
    .unwrap();
    drop(conn);

    refresh_match_evaluations(&db).unwrap();
    let row = list_matches(&db, MatchFilters::default())
        .unwrap()
        .remove(0);
    assert_eq!(row.evaluation.unwrap().primary_tag, "带飞局");
}

#[test]
fn import_generates_evaluation_cache_without_blocking_match_import() {
    let db = temp_db();
    let root = tempfile::tempdir().unwrap();
    import_payload_value(&db, &import_payload(), root.path()).unwrap();

    let conn = db.connect().unwrap();
    let cache_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_evaluations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(cache_rows, 1);

    let row = list_matches(&db, MatchFilters::default())
        .unwrap()
        .remove(0);
    assert_eq!(row.evaluation.unwrap().primary_tag, "带飞局");
}

fn import_payload() -> Value {
    json!({
        "season": 22,
        "profile_card": {"data": {"bnetId": "me"}},
        "matches": {
            "entries": [{
                "matchId": "imported",
                "matchRet": 1,
                "mapGuid": "map-a",
                "heroGuid": "hero-me",
                "beginTs": 1700000000000_i64,
                "gameMode": "SportPreset",
                "_dashenSeason": 22,
                "teamScore": 2,
                "opponentScore": 1,
                "kill": 20,
                "assist": 10,
                "death": 2,
                "heroDamage": 12000,
                "cure": 3000,
                "resistDamage": 500
            }],
            "details": [{
                "match_id": "imported",
                "match_kind": "normal",
                "payload": {"data": {
                    "teammateList": [
                        {"bnetId": "me", "name": "Me", "heroGuid": "hero-me", "kill": 20, "assist": 10, "death": 2, "heroDamage": 12000, "cure": 3000, "resistDamage": 500},
                        {"bnetId": "t1", "name": "Team 1", "heroGuid": "ally-a", "kill": 8, "assist": 4, "death": 4, "heroDamage": 7000, "cure": 1000, "resistDamage": 300}
                    ],
                    "enemyList": [
                        {"bnetId": "e1", "name": "Enemy 1", "heroGuid": "enemy-a", "kill": 10, "death": 4, "heroDamage": 6000}
                    ]
                }}
            }],
            "detail_errors": []
        }
    })
}

#[test]
fn friend_common_heroes_and_lineup_combinations_follow_phase5_rules() {
    let db = temp_db();
    for index in 0..3 {
        seed_match(
            &db,
            &format!("friend-a-{index}"),
            "win",
            index,
            player("me", "Me", "hero-me", true, false, 10, 3, 2, 6000, 0, 0),
            vec![
                player(
                    "friend-a", "Friend A", "ana", false, true, 4, 8, 2, 2000, 6000, 0,
                ),
                player(
                    "friend-b", "Friend B", "rein", false, true, 5, 2, 4, 3000, 0, 9000,
                ),
            ],
            standard_enemies(),
        );
    }
    for index in 0..2 {
        seed_match(
            &db,
            &format!("friend-a-alt-{index}"),
            "loss",
            10 + index,
            player("me", "Me", "hero-me", true, false, 8, 3, 3, 5000, 0, 0),
            vec![player(
                "friend-a", "Friend A", "bap", false, true, 5, 8, 2, 2500, 5000, 0,
            )],
            standard_enemies(),
        );
    }

    let analytics = get_friend_analytics(&db, FriendAnalyticsFilters::default()).unwrap();
    assert_eq!(analytics.common_friend_heroes.len(), 2);
    assert_eq!(analytics.common_friend_heroes[0].bnet_id, "friend-a");
    assert_eq!(
        analytics.common_friend_heroes[0].representative_hero_name,
        "未知英雄"
    );
    assert_eq!(analytics.common_friend_heroes[0].matches, 5);
    assert_eq!(analytics.lineup_combinations.len(), 1);
    assert_eq!(analytics.lineup_combinations[0].matches, 3);
    assert!(analytics.lineup_combinations[0]
        .members
        .iter()
        .any(|member| member.bnet_id.as_deref() == Some("friend-b")));
}
