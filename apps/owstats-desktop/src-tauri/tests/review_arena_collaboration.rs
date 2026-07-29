#![allow(clippy::too_many_arguments)]

use owstats_desktop_lib::{
    get_friend_analytics, get_personal_season_review, FriendAnalyticsFilters, OwDatabase,
    PersonalReviewRequest,
};
use rusqlite::params;
use serde_json::json;

fn seed_match(db: &OwDatabase, match_id: &str, season: i64, bucket: &str, game_mode: &str) {
    let match_kind = if bucket == "stadium" {
        "fight"
    } else {
        "normal"
    };
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'win', 'synthetic-map', '测试地图',
            'synthetic-hero', '测试英雄', ?6, 2, 1, 10, 5, 2, 5000, 1000, 500,
            0, 600, '{}', '2026-07-16T00:00:00Z')
        "#,
        params![match_id, season, bucket, game_mode, match_kind, season * 100],
    )
    .unwrap();
}

fn seed_current_player(db: &OwDatabase) {
    db.connect()
        .unwrap()
        .execute(
            "INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at) VALUES ('boundary-self', 'boundary-self', '当前玩家', '{}', '2026-07-16T00:00:00Z')",
            [],
        )
        .unwrap();
}

fn seed_boundary_arena_match(
    db: &OwDatabase,
    match_id: &str,
    season: i64,
    mode: &str,
    result: &str,
    map_guid: &str,
    map_name: &str,
    begin_ts: i64,
    friend_id: &str,
    friend_hero: &str,
) {
    seed_match(db, match_id, season, "stadium", mode);
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE matches SET result=?2, map_guid=?3, map_name=?4, begin_ts=?5 WHERE match_id=?1",
        params![match_id, result, map_guid, map_name, begin_ts],
    )
    .unwrap();
    let detail = json!({
        "payload": {
            "data": {
                "roundCountList": [{
                    "teammateList": [
                        {
                            "bnetId": "boundary-self",
                            "name": "当前玩家",
                            "heroGuid": "boundary-self-hero",
                            "friendBnetIds": [friend_id]
                        },
                        {
                            "bnetId": friend_id,
                            "name": "边界好友",
                            "heroGuid": friend_hero,
                            "kill": 3,
                            "heroDamage": 300
                        },
                        {
                            "bnetId": friend_id,
                            "name": "重复好友不应重复计数",
                            "heroGuid": friend_hero,
                            "kill": 99,
                            "heroDamage": 9900
                        }
                    ],
                    "enemyList": [{
                        "bnetId": friend_id,
                        "name": "敌方同 ID 不应计数",
                        "heroGuid": "enemy-hero"
                    }]
                }]
            }
        }
    });
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'fight', ?2, '2026-07-16T00:00:00Z')",
        params![match_id, detail.to_string()],
    )
    .unwrap();
}

fn arena_friend_rows(db: &OwDatabase, filters: FriendAnalyticsFilters) -> i64 {
    get_friend_analytics(db, filters)
        .unwrap()
        .summary
        .friend_rows
}

#[test]
fn personal_review_discovers_all_local_seasons_and_validates_requests() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("review-arena-collaboration.sqlite3"));
    db.initialize().unwrap();

    seed_match(&db, "synthetic-s15", 15, "preset", "SportPreset");
    seed_match(&db, "synthetic-s20", 20, "stadium", "LeisureFight");
    seed_match(&db, "synthetic-s23", 23, "preset", "SportPreset");

    let latest = get_personal_season_review(&db, PersonalReviewRequest { season: None }).unwrap();
    assert_eq!(latest.available_seasons, vec![23, 20, 15]);
    assert_eq!(latest.summary.season, Some(23));

    let requested =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(15) }).unwrap();
    assert_eq!(requested.available_seasons, vec![23, 20, 15]);
    assert_eq!(requested.summary.season, Some(15));

    let stadium_only =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(20) }).unwrap();
    assert_eq!(stadium_only.available_seasons, vec![23, 20, 15]);
    assert_eq!(stadium_only.summary.season, Some(20));

    let missing =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(999) }).unwrap();
    assert_eq!(missing.available_seasons, vec![23, 20, 15]);
    assert_eq!(missing.summary.season, Some(23));
}

#[test]
fn personal_review_preserves_requested_season_when_no_local_seasons_exist() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("empty-review.sqlite3"));
    db.initialize().unwrap();

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    assert!(review.available_seasons.is_empty());
    assert_eq!(review.summary.season, Some(22));
}

#[test]
fn personal_review_excludes_non_positive_seasons_and_falls_back_to_latest_valid_season() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("review-invalid-season.sqlite3"));
    db.initialize().unwrap();

    seed_match(&db, "synthetic-s0", 0, "preset", "SportPreset");
    seed_match(&db, "synthetic-s20", 20, "stadium", "SportFight");

    let latest = get_personal_season_review(&db, PersonalReviewRequest { season: None }).unwrap();
    assert_eq!(latest.available_seasons, vec![20]);
    assert_eq!(latest.summary.season, Some(20));

    let requested_zero =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(0) }).unwrap();
    assert_eq!(requested_zero.available_seasons, vec![20]);
    assert_eq!(requested_zero.summary.season, Some(20));
}

#[test]
fn lineup_display_sanitizes_private_ids_and_raw_hero_guids_across_normal_and_arena() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-private-lineup.sqlite3"));
    db.initialize().unwrap();
    let current_id = "123e4567-e89b-12d3-a456-426614174000";
    let friend_id = "12345678901234567890123456789012";
    let current_hero_guid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let friend_hero_guid = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at) VALUES ('private-self', ?1, ?1, '{}', '2026-07-16T00:00:00Z')",
        [current_id],
    )
    .unwrap();
    for guid in [current_hero_guid, friend_hero_guid] {
        conn.execute(
            "INSERT INTO hero_dict (guid, name, updated_at) VALUES (?1, ?1, '2026-07-16T00:00:00Z')",
            [guid],
        )
        .unwrap();
    }
    drop(conn);

    seed_match(&db, "private-normal", 23, "preset", "SportPreset");
    let conn = db.connect().unwrap();
    for (bnet_id, hero_guid, is_current_player, is_friend) in [
        (current_id, current_hero_guid, 1, 0),
        (friend_id, friend_hero_guid, 0, 1),
    ] {
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, raw_json, updated_at
            ) VALUES (
                'private-normal', 23, 'teammate', ?1, ?1, ?2, ?2,
                ?3, ?4, '{}', '2026-07-16T00:00:00Z'
            )
            "#,
            params![bnet_id, hero_guid, is_current_player, is_friend],
        )
        .unwrap();
    }
    drop(conn);

    for index in 0..2 {
        let match_id = format!("private-arena-{index}");
        seed_match(&db, &match_id, 23, "stadium", "SportFight");
        let detail = json!({
            "payload": {
                "data": {
                    "roundCountList": [{
                        "teammateList": [
                            {
                                "bnetId": current_id,
                                "name": current_id,
                                "heroGuid": current_hero_guid,
                                "friendBnetIds": [friend_id]
                            },
                            {
                                "bnetId": friend_id,
                                "name": friend_id,
                                "heroGuid": friend_hero_guid
                            }
                        ]
                    }]
                }
            }
        });
        db.connect()
            .unwrap()
            .execute(
                "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'fight', ?2, '2026-07-16T00:00:00Z')",
                params![match_id, detail.to_string()],
            )
            .unwrap();
    }

    let analytics = get_friend_analytics(
        &db,
        FriendAnalyticsFilters {
            season: Some(23),
            ..FriendAnalyticsFilters::default()
        },
    )
    .unwrap();
    assert_eq!(analytics.lineup_combinations.len(), 1);
    let lineup = &analytics.lineup_combinations[0];
    assert_eq!(lineup.matches, 3);
    let current = lineup
        .members
        .iter()
        .find(|member| member.is_current_player)
        .unwrap();
    assert_eq!(current.bnet_id.as_deref(), Some(current_id));
    assert_eq!(current.hero_guid, current_hero_guid);
    assert_eq!(current.name, "当前玩家");
    assert_eq!(current.hero_name, "未知英雄");
    let friend = lineup
        .members
        .iter()
        .find(|member| !member.is_current_player)
        .unwrap();
    assert_eq!(friend.bnet_id.as_deref(), Some(friend_id));
    assert_eq!(friend.hero_guid, friend_hero_guid);
    assert_eq!(friend.name, "未知好友");
    assert_eq!(friend.hero_name, "未知英雄");
    for member in &lineup.members {
        assert_ne!(member.name, member.bnet_id.as_deref().unwrap());
        assert_ne!(member.hero_name, member.hero_guid);
    }
}

#[test]
fn friend_analytics_merges_normal_and_last_valid_arena_rounds() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-friend-arena.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();

    conn.execute(
        "INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at) VALUES ('synthetic-self', 'synthetic-self', '当前玩家', '{}', '2026-07-16T00:00:00Z')",
        [],
    )
    .unwrap();
    {
        let (guid, name) = ("synthetic-self-hero", "当前英雄");
        conn.execute(
            "INSERT INTO hero_dict (guid, name, updated_at) VALUES (?1, ?2, '2026-07-16T00:00:00Z')",
            params![guid, name],
        )
        .unwrap();
    }

    seed_match(&db, "synthetic-normal-friend", 23, "preset", "SportPreset");
    for (
        bnet_id,
        name,
        hero_guid,
        hero_name,
        is_current_player,
        is_friend,
        damage_taken,
        mitigation,
    ) in [
        (
            "synthetic-self",
            "当前玩家",
            "synthetic-self-hero",
            "当前英雄",
            1,
            0,
            0,
            0,
        ),
        (
            "20260716",
            "可信好友",
            "synthetic-friend-hero",
            "可信英雄",
            0,
            1,
            100,
            50,
        ),
    ] {
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, kill, assist, death, hero_damage, cure,
                resist_damage, healing_taken, damage_taken, raw_json, updated_at
            ) VALUES (
                'synthetic-normal-friend', 23, 'teammate', ?1, ?2, ?3, ?4,
                ?5, ?6, 10, 5, 2, 5000, 1000, ?7, 40, ?8, '{}',
                '2026-07-16T00:00:00Z'
            )
            "#,
            params![
                bnet_id,
                name,
                hero_guid,
                hero_name,
                is_current_player,
                is_friend,
                mitigation,
                damage_taken
            ],
        )
        .unwrap();
    }

    for (index, mode) in ["SportFight", "SportFight", "LeisureFight", "LeisureFight"]
        .into_iter()
        .enumerate()
    {
        let match_id = format!("synthetic-arena-{index}");
        seed_match(&db, &match_id, 23, "stadium", mode);
        if index % 2 == 1 {
            conn.execute(
                "UPDATE matches SET result='loss' WHERE match_id=?1",
                [&match_id],
            )
            .unwrap();
        }
        let friend_id = if index % 2 == 0 {
            json!("20260716")
        } else {
            json!(20260716)
        };
        let friend_bnet_id = if index % 2 == 0 {
            json!("20260716")
        } else {
            json!(20260716)
        };
        let detail = json!({
            "payload": {
                "data": {
                    "nameMap": {
                        "synthetic-self": "当前玩家"
                    },
                    "totalCount": {
                        "teammateList": [
                            {"bnetId": "synthetic-self", "heroGuid": "synthetic-self-hero", "friendBnetIds": [friend_id]},
                            {"bnetId": friend_bnet_id, "heroGuid": "synthetic-friend-hero", "damageTaken": 900, "resistDamage": 800}
                        ]
                    },
                    "roundCountList": [
                        {
                            "teammateList": [
                                {"bnetId": "synthetic-self", "heroGuid": "synthetic-self-hero", "friendBnetIds": [friend_id]},
                                {"bnetId": friend_bnet_id, "heroGuid": "synthetic-friend-hero", "kill": 99, "assist": 98, "death": 97, "heroDamage": 9600, "cure": 9500, "resistDamage": 940, "damageTaken": 930, "healingTaken": 920}
                            ]
                        },
                        {
                            "teammateList": [
                                {"bnetId": "synthetic-self", "heroGuid": "synthetic-self-hero", "friendBnetIds": [friend_id]},
                                {"bnetId": friend_bnet_id, "heroGuid": "synthetic-friend-hero", "kill": 10, "assist": 5, "death": 2, "heroDamage": 5000, "cure": 1000, "resistDamage": 70, "damageTaken": 200, "healingTaken": 40}
                            ]
                        },
                        {"teammateList": []}
                    ]
                }
            }
        });
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'fight', ?2, '2026-07-16T00:00:00Z')",
            params![match_id, detail.to_string()],
        )
        .unwrap();
    }
    drop(conn);

    let analytics = get_friend_analytics(
        &db,
        FriendAnalyticsFilters {
            season: Some(23),
            ..FriendAnalyticsFilters::default()
        },
    )
    .unwrap();

    assert_eq!(analytics.summary.preset_matches, 1);
    let friend = analytics
        .common_friend_heroes
        .iter()
        .find(|item| item.bnet_id == "20260716")
        .unwrap();
    assert_eq!(friend.name, "可信好友");
    assert_eq!(friend.matches, 5);
    assert_eq!(friend.intimacy_score, 25);
    assert_eq!(friend.representative_hero_name, "可信英雄");

    let lineup = analytics
        .lineup_combinations
        .iter()
        .find(|item| item.matches == 5)
        .unwrap();
    assert_eq!(lineup.members.len(), 2);
    assert_eq!(lineup.wins, 3);
    assert_eq!(lineup.win_rate, 0.6);
    let current = lineup
        .members
        .iter()
        .find(|member| member.is_current_player)
        .unwrap();
    assert_eq!(current.bnet_id.as_deref(), Some("synthetic-self"));
    assert_eq!(current.hero_name, "当前英雄");
    let friend_member = lineup
        .members
        .iter()
        .find(|member| !member.is_current_player)
        .unwrap();
    assert_eq!(friend_member.bnet_id.as_deref(), Some("20260716"));
    assert_eq!(friend_member.name, "可信好友");
    assert_eq!(friend_member.hero_name, "可信英雄");

    for _ in 0..20 {
        let repeated = get_friend_analytics(
            &db,
            FriendAnalyticsFilters {
                season: Some(23),
                ..FriendAnalyticsFilters::default()
            },
        )
        .unwrap();
        let repeated_lineup = repeated
            .lineup_combinations
            .iter()
            .find(|item| item.matches == 5)
            .unwrap();
        let repeated_friend = repeated_lineup
            .members
            .iter()
            .find(|member| !member.is_current_player)
            .unwrap();
        assert_eq!(repeated_friend.name, "可信好友");
        assert_eq!(repeated_friend.hero_name, "可信英雄");
    }

    let front_pressure = analytics
        .boards
        .iter()
        .find(|board| board.key == "front_pressure")
        .unwrap();
    let model_entry = front_pressure
        .friends
        .iter()
        .find(|item| item.bnet_id == "20260716")
        .unwrap();
    assert_eq!(model_entry.name, "可信好友");
    assert_eq!(model_entry.same_team_matches, 5);
    assert_eq!(model_entry.avg_damage_taken, 180.0);
    assert_eq!(model_entry.avg_mitigation, 66.0);
}

#[test]
fn arena_lineup_key_is_stable_when_friend_order_changes_between_matches() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-stable-arena-lineup.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at) VALUES ('synthetic-self', 'synthetic-self', '当前玩家', '{}', '2026-07-16T00:00:00Z')",
        [],
    )
    .unwrap();
    for (guid, name) in [
        ("synthetic-self-hero", "当前英雄"),
        ("synthetic-friend-a-hero", "好友A英雄"),
        ("synthetic-friend-b-hero", "好友B英雄"),
    ] {
        conn.execute(
            "INSERT INTO hero_dict (guid, name, updated_at) VALUES (?1, ?2, '2026-07-16T00:00:00Z')",
            params![guid, name],
        )
        .unwrap();
    }

    for index in 0..3 {
        let match_id = format!("synthetic-stable-arena-{index}");
        seed_match(&db, &match_id, 23, "stadium", "SportFight");
        let friends = if index % 2 == 0 {
            vec![
                json!({"bnetId": "friend-a", "name": "好友A", "heroGuid": "synthetic-friend-a-hero"}),
                json!({"bnetId": "friend-b", "name": "好友B", "heroGuid": "synthetic-friend-b-hero"}),
            ]
        } else {
            vec![
                json!({"bnetId": "friend-b", "name": "好友B", "heroGuid": "synthetic-friend-b-hero"}),
                json!({"bnetId": "friend-a", "name": "好友A", "heroGuid": "synthetic-friend-a-hero"}),
            ]
        };
        let mut teammates = vec![json!({
            "bnetId": "synthetic-self",
            "name": "当前玩家",
            "heroGuid": "synthetic-self-hero",
            "friendBnetIds": ["friend-a", "friend-b"]
        })];
        teammates.extend(friends);
        let detail = json!({
            "payload": {
                "data": {
                    "roundCountList": [{"teammateList": teammates}]
                }
            }
        });
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'fight', ?2, '2026-07-16T00:00:00Z')",
            params![match_id, detail.to_string()],
        )
        .unwrap();
    }
    drop(conn);

    let analytics = get_friend_analytics(
        &db,
        FriendAnalyticsFilters {
            season: Some(23),
            ..FriendAnalyticsFilters::default()
        },
    )
    .unwrap();

    assert_eq!(analytics.lineup_combinations.len(), 1);
    let lineup = &analytics.lineup_combinations[0];
    assert_eq!(lineup.matches, 3);
    assert_eq!(lineup.members.len(), 3);
    assert!(lineup.members[0].is_current_player);
    assert_eq!(lineup.members[0].bnet_id.as_deref(), Some("synthetic-self"));
    assert_eq!(lineup.members[1].bnet_id.as_deref(), Some("friend-a"));
    assert_eq!(lineup.members[2].bnet_id.as_deref(), Some("friend-b"));
}

#[test]
fn arena_identity_fallback_prefers_latest_projection_update() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-identity-fallback.sqlite3"));
    db.initialize().unwrap();

    seed_match(&db, "synthetic-identity-old", 23, "preset", "SportPreset");
    seed_match(&db, "synthetic-identity-new", 23, "preset", "SportPreset");
    seed_match(&db, "synthetic-identity-arena", 23, "stadium", "SportFight");
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE matches SET begin_ts=9000 WHERE match_id='synthetic-identity-old'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE matches SET begin_ts=1000 WHERE match_id='synthetic-identity-new'",
        [],
    )
    .unwrap();
    for (match_id, bnet_id, updated_at) in [
        (
            "synthetic-identity-old",
            "synthetic-self-old",
            "2026-07-15T00:00:00Z",
        ),
        (
            "synthetic-identity-new",
            "synthetic-self-new",
            "2026-07-16T00:00:00Z",
        ),
    ] {
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, raw_json, updated_at
            ) VALUES (?1, 23, 'teammate', ?2, '当前玩家', 'synthetic-self-hero',
                '当前英雄', 1, 0, '{}', ?3)
            "#,
            params![match_id, bnet_id, updated_at],
        )
        .unwrap();
    }
    let detail = json!({
        "payload": {
            "data": {
                "roundCountList": [{
                    "teammateList": [
                        {
                            "bnetId": "synthetic-self-new",
                            "heroGuid": "synthetic-self-hero",
                            "friendBnetIds": ["synthetic-friend"]
                        },
                        {
                            "bnetId": "synthetic-friend",
                            "name": "合成好友",
                            "heroGuid": "synthetic-friend-hero",
                            "damageTaken": 200,
                            "resistDamage": 70
                        }
                    ]
                }]
            }
        }
    });
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES ('synthetic-identity-arena', 'fight', ?1, '2026-07-16T00:00:00Z')",
        [detail.to_string()],
    )
    .unwrap();
    drop(conn);

    let analytics = get_friend_analytics(
        &db,
        FriendAnalyticsFilters {
            season: Some(23),
            ..FriendAnalyticsFilters::default()
        },
    )
    .unwrap();

    assert_eq!(analytics.summary.friend_rows, 1);
}

#[test]
fn arena_private_id_shaped_names_use_unknown_friend_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-private-name.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at) VALUES ('synthetic-self', 'synthetic-self', '当前玩家', '{}', '2026-07-16T00:00:00Z')",
        [],
    )
    .unwrap();

    let private_names = [
        "0123456789abcdef0123456789abcdef",
        "01234567-89ab-cdef-0123-456789abcdef",
        "01234567890123456789012345678901",
    ];
    for (index, private_name) in private_names.into_iter().enumerate() {
        let match_id = format!("synthetic-private-arena-{index}");
        seed_match(&db, &match_id, 23, "stadium", "SportFight");
        let detail = json!({
            "payload": {
                "data": {
                    "roundCountList": [{
                        "teammateList": [
                            {
                                "bnetId": "synthetic-self",
                                "heroGuid": "synthetic-self-hero",
                                "friendBnetIds": ["synthetic-private-friend"]
                            },
                            {
                                "bnetId": "synthetic-private-friend",
                                "name": private_name,
                                "heroGuid": "synthetic-friend-hero"
                            }
                        ]
                    }]
                }
            }
        });
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'fight', ?2, '2026-07-16T00:00:00Z')",
            params![match_id, detail.to_string()],
        )
        .unwrap();
    }
    drop(conn);

    let analytics = get_friend_analytics(
        &db,
        FriendAnalyticsFilters {
            season: Some(23),
            ..FriendAnalyticsFilters::default()
        },
    )
    .unwrap();

    let friend = analytics
        .common_friend_heroes
        .iter()
        .find(|item| item.bnet_id == "synthetic-private-friend")
        .unwrap();
    assert_eq!(friend.name, "未知好友");
    for visible_name in analytics
        .common_friend_heroes
        .iter()
        .map(|item| item.name.as_str())
        .chain(
            analytics
                .boards
                .iter()
                .flat_map(|board| board.friends.iter())
                .map(|item| item.name.as_str()),
        )
    {
        assert!(!private_names.contains(&visible_name));
    }
}

#[test]
fn raw_hero_guid_never_becomes_representative_hero_name() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-hero-guid-fallback.sqlite3"));
    db.initialize().unwrap();
    seed_match(&db, "synthetic-guid-normal", 23, "preset", "SportPreset");
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at) VALUES ('synthetic-self', 'synthetic-self', '当前玩家', '{}', '2026-07-16T00:00:00Z')",
        [],
    )
    .unwrap();
    for (bnet_id, is_current_player, is_friend, hero_guid, hero_name) in [
        ("synthetic-self", 1, 0, "synthetic-self-hero", "当前英雄"),
        (
            "synthetic-guid-friend",
            0,
            1,
            "synthetic-raw-hero-guid",
            "synthetic-raw-hero-guid",
        ),
    ] {
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, raw_json, updated_at
            ) VALUES ('synthetic-guid-normal', 23, 'teammate', ?1, '可信好友', ?2,
                ?3, ?4, ?5, '{}', '2026-07-16T00:00:00Z')
            "#,
            params![bnet_id, hero_guid, hero_name, is_current_player, is_friend],
        )
        .unwrap();
    }
    for index in 0..2 {
        let match_id = format!("synthetic-guid-arena-{index}");
        seed_match(&db, &match_id, 23, "stadium", "SportFight");
        let detail = json!({
            "payload": {
                "data": {
                    "roundCountList": [{
                        "teammateList": [
                            {
                                "bnetId": "synthetic-self",
                                "heroGuid": "synthetic-self-hero",
                                "friendBnetIds": ["synthetic-guid-friend"]
                            },
                            {
                                "bnetId": "synthetic-guid-friend",
                                "name": "可信好友",
                                "heroGuid": "synthetic-raw-hero-guid"
                            }
                        ]
                    }]
                }
            }
        });
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'fight', ?2, '2026-07-16T00:00:00Z')",
            params![match_id, detail.to_string()],
        )
        .unwrap();
    }
    drop(conn);

    let analytics = get_friend_analytics(
        &db,
        FriendAnalyticsFilters {
            season: Some(23),
            ..FriendAnalyticsFilters::default()
        },
    )
    .unwrap();
    let friend = analytics
        .common_friend_heroes
        .iter()
        .find(|item| item.bnet_id == "synthetic-guid-friend")
        .unwrap();
    assert_eq!(friend.representative_hero_name, "未知英雄");
    assert_ne!(
        friend.representative_hero_name,
        friend.representative_hero_guid
    );
    assert!(!friend
        .representative_hero_name
        .contains("synthetic-raw-hero-guid"));
}

#[test]
fn arena_friend_rows_require_identity_friend_ids_valid_round_and_selected_season() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-arena-boundaries.sqlite3"));
    db.initialize().unwrap();
    seed_current_player(&db);

    for index in 0..3 {
        seed_boundary_arena_match(
            &db,
            &format!("selected-season-{index}"),
            20,
            "SportFight",
            "win",
            "boundary-map",
            "边界地图",
            1_789_000_000 + index,
            "boundary-friend",
            "boundary-hero",
        );
        seed_boundary_arena_match(
            &db,
            &format!("other-season-{index}"),
            15,
            "LeisureFight",
            "win",
            "boundary-map",
            "边界地图",
            1_789_000_100 + index,
            "boundary-friend",
            "boundary-hero",
        );
    }
    seed_boundary_arena_match(
        &db,
        "missing-friend-ids",
        20,
        "SportFight",
        "win",
        "boundary-map",
        "边界地图",
        1_789_000_200,
        "boundary-friend",
        "boundary-hero",
    );
    seed_boundary_arena_match(
        &db,
        "empty-friend-ids",
        20,
        "SportFight",
        "win",
        "boundary-map",
        "边界地图",
        1_789_000_202,
        "boundary-friend",
        "boundary-hero",
    );
    seed_boundary_arena_match(
        &db,
        "unsupported-arena-mode",
        20,
        "UnknownFight",
        "win",
        "boundary-map",
        "边界地图",
        1_789_000_203,
        "boundary-friend",
        "boundary-hero",
    );
    seed_boundary_arena_match(
        &db,
        "invalid-last-round",
        20,
        "SportFight",
        "win",
        "boundary-map",
        "边界地图",
        1_789_000_201,
        "boundary-friend",
        "boundary-hero",
    );
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE match_details SET raw_detail_json=json_remove(raw_detail_json, '$.payload.data.roundCountList[0].teammateList[0].friendBnetIds') WHERE match_id='missing-friend-ids'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE match_details SET raw_detail_json=json_set(raw_detail_json, '$.payload.data.roundCountList[0].teammateList[0].friendBnetIds', json('[]')) WHERE match_id='empty-friend-ids'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE match_details SET raw_detail_json=?1 WHERE match_id='invalid-last-round'",
        [json!({
            "payload": {
                "data": {
                    "roundCountList": [{"teammateList": [{"bnetId": "someone-else"}]}]
                }
            }
        })
        .to_string()],
    )
    .unwrap();
    drop(conn);

    let analytics = get_friend_analytics(
        &db,
        FriendAnalyticsFilters {
            season: Some(20),
            ..FriendAnalyticsFilters::default()
        },
    )
    .unwrap();
    assert_eq!(analytics.summary.friend_rows, 3);
    let friend = analytics
        .common_friend_heroes
        .iter()
        .find(|item| item.bnet_id == "boundary-friend")
        .unwrap();
    assert_eq!(friend.matches, 3);
    assert_eq!(friend.intimacy_score, 15);
    assert_eq!(analytics.lineup_combinations.len(), 1);
    assert_eq!(analytics.lineup_combinations[0].matches, 3);

    let no_identity = OwDatabase::new(temp.path().join("synthetic-no-identity.sqlite3"));
    no_identity.initialize().unwrap();
    seed_boundary_arena_match(
        &no_identity,
        "no-identity",
        20,
        "SportFight",
        "win",
        "boundary-map",
        "边界地图",
        1_789_000_000,
        "boundary-friend",
        "boundary-hero",
    );
    assert_eq!(
        arena_friend_rows(
            &no_identity,
            FriendAnalyticsFilters {
                season: Some(20),
                ..FriendAnalyticsFilters::default()
            }
        ),
        0
    );
}

#[test]
fn arena_friend_rows_obey_match_and_last_round_hero_filters() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("synthetic-arena-filters.sqlite3"));
    db.initialize().unwrap();
    seed_current_player(&db);
    let conn = db.connect().unwrap();
    for (guid, name) in [("competitive-hero", "竞技英雄"), ("quick-hero", "快速英雄")] {
        conn.execute(
            "INSERT INTO hero_dict (guid, name, updated_at) VALUES (?1, ?2, '2026-07-16T00:00:00Z')",
            params![guid, name],
        )
        .unwrap();
    }
    drop(conn);

    for index in 0..3 {
        seed_boundary_arena_match(
            &db,
            &format!("competitive-filter-{index}"),
            20,
            "SportFight",
            "win",
            "competitive-map-guid",
            "竞技地图",
            1_789_000_000 + index,
            "boundary-friend",
            "competitive-hero",
        );
        seed_boundary_arena_match(
            &db,
            &format!("quick-filter-{index}"),
            20,
            "LeisureFight",
            "loss",
            "quick-map-guid",
            "快速地图",
            1_789_000_100 + index,
            "boundary-friend",
            "quick-hero",
        );
    }

    for filters in [
        FriendAnalyticsFilters {
            season: Some(20),
            mode: Some("LeisureFight".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            hero: Some("quick-hero".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            hero: Some("快速英雄".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            map: Some("quick-map-guid".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            map: Some("快速地图".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            result: Some("loss".into()),
            ..FriendAnalyticsFilters::default()
        },
    ] {
        assert_eq!(arena_friend_rows(&db, filters), 3);
    }

    for filters in [
        FriendAnalyticsFilters {
            season: Some(20),
            mode: Some("SportPreset".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            hero: Some("missing-hero".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            map: Some("missing-map".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            result: Some("draw".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            date_from: Some("2099-01-01".into()),
            ..FriendAnalyticsFilters::default()
        },
        FriendAnalyticsFilters {
            season: Some(20),
            date_to: Some("2000-01-01".into()),
            ..FriendAnalyticsFilters::default()
        },
    ] {
        let analytics = get_friend_analytics(&db, filters).unwrap();
        assert_eq!(analytics.summary.friend_rows, 0);
        assert!(analytics.lineup_combinations.is_empty());
    }
}
