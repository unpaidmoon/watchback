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
    for (guid, name) in [
        ("synthetic-self-hero", "当前英雄"),
        ("synthetic-friend-hero", "好友英雄"),
    ] {
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
            "好友英雄",
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
    assert_eq!(friend.representative_hero_name, "好友英雄");

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
    assert_eq!(friend_member.hero_name, "好友英雄");

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
