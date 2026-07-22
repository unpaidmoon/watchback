#![recursion_limit = "256"]

use owstats_desktop_lib::{
    get_friend_analytics, get_match_detail, get_match_favorite, import_payload_value,
    list_filter_options, list_matches, save_match_favorite, search_match_players,
    sync_import_parts, FriendAnalyticsFilters, MatchFilters, OwDatabase,
};
use serde_json::{json, Value};

fn import_synthetic_season(db: &OwDatabase) {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    import_payload_value(db, &season_payload(), temp.path()).unwrap();
}

#[test]
fn repeated_import_preserves_local_match_favorite_and_note() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));

    import_synthetic_season(&db);
    save_match_favorite(&db, "match-win", "保留这条本地备注").unwrap();
    import_synthetic_season(&db);

    let favorite = get_match_favorite(&db, "match-win").unwrap().unwrap();
    assert_eq!(favorite.note, "保留这条本地备注");
}

#[test]
fn legacy_normal_map_uses_builtin_name_in_list_detail_and_filters() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = json!({
        "season": 21,
        "matches": {
            "entries": [match_entry(
                "legacy-lijiang",
                1,
                "LeisurePreset",
                "576460752303425207",
                "hero-current",
                1700000000000_i64
            )],
            "details": [match_detail("legacy-lijiang", "normal", "576460752303425207", 600)],
            "detail_errors": []
        }
    });

    import_payload_value(&db, &payload, temp.path()).unwrap();

    let row = list_matches(
        &db,
        MatchFilters {
            season: Some(22),
            ..MatchFilters::default()
        },
    )
    .unwrap()
    .into_iter()
    .find(|item| item.match_id == "legacy-lijiang")
    .unwrap();
    assert_eq!(row.map_name, "漓江塔");

    let filters = list_filter_options(&db).unwrap();
    assert!(filters
        .maps
        .iter()
        .any(|option| option.id == "576460752303425207" && option.label == "漓江塔"));

    let detail = get_match_detail(&db, "legacy-lijiang").unwrap();
    assert_eq!(detail.map_name.as_deref(), Some("漓江塔"));
}

fn lookup_payload() -> String {
    serde_json::to_string(&json!({
        "heroes": {
            "hero-current": {"name": "安娜"},
            "hero-friend": {"name": "莱因哈特"},
            "hero-enemy": {"name": "源氏"}
        },
        "maps": {
            "map-a": {"name": "漓江塔"},
            "map-b": {"name": "国王大道"}
        }
    }))
    .unwrap()
}

fn season_payload() -> Value {
    json!({
        "season": 22,
        "profile_card": {
            "data": {
                "bnetId": 1000,
                "name": "Current#1000",
                "customerToken": "customer-secret"
            }
        },
        "matches": {
            "entries": [
                match_entry("match-win", 1, "LeisurePreset", "map-a", "hero-current", 1700000000000_i64),
                match_entry("match-loss", -1, "SportPreset", "map-b", "hero-current", 1700003600000_i64),
                match_entry("match-stadium", 1, "StadiumFight", "map-a", "hero-current", 1700007200000_i64)
            ],
            "details": [
                match_detail("match-win", "normal", "map-a", 600),
                match_detail("match-loss", "normal", "map-b", 650),
                match_detail("match-stadium", "fight", "map-a", 500)
            ],
            "detail_errors": [
                {
                    "match_id": "bad-detail",
                    "error_type": "DetailFetchError",
                    "message": "request failed token=entry-secret customerToken=detail-secret"
                }
            ]
        }
    })
}

fn match_entry(
    match_id: &str,
    match_ret: i64,
    game_mode: &str,
    map_guid: &str,
    hero_guid: &str,
    begin_ts: i64,
) -> Value {
    json!({
        "matchId": match_id,
        "matchRet": match_ret,
        "mapGuid": map_guid,
        "heroGuid": hero_guid,
        "beginTs": begin_ts,
        "gameMode": game_mode,
        "_dashenSeason": 22,
        "teamScore": 2,
        "opponentScore": if match_ret == 1 { 1 } else { 2 },
        "kill": if match_ret == 1 { 12 } else { 4 },
        "assist": 8,
        "death": if match_ret == 1 { 3 } else { 9 },
        "heroDamage": if match_ret == 1 { 9000 } else { 3000 },
        "cure": if match_ret == 1 { 8000 } else { 2000 },
        "resistDamage": 500,
        "totalWorth": 100,
        "token": "entry-secret"
    })
}

fn match_detail(match_id: &str, match_kind: &str, map_guid: &str, duration: i64) -> Value {
    json!({
        "match_id": match_id,
        "match_kind": match_kind,
        "payload": {
            "data": {
                "matchRet": if match_id == "match-loss" { -1 } else { 1 },
                "mapGuid": map_guid,
                "gameTimeSec": duration,
                "teammateList": [
                    {
                        "bnetId": 1000,
                        "name": "Current#1000",
                        "heroGuid": "hero-current",
                        "roleType": "healer",
                        "friendBnetIds": [2000],
                        "kill": 5,
                        "assist": 7,
                        "death": 2,
                        "heroDamage": 3000,
                        "cure": 7000,
                        "resistDamage": 100,
                        "healingTaken": 900,
                        "damageTaken": 1200,
                        "finalHit": 1,
                        "targetCompetingTime": 30,
                        "customerToken": "detail-secret"
                    },
                    {
                        "bnetId": 2000,
                        "name": "Friend#2000",
                        "heroGuid": "hero-friend",
                        "roleType": "tank",
                        "kill": 4,
                        "assist": 3,
                        "death": 3,
                        "heroDamage": 2500,
                        "cure": 0,
                        "resistDamage": 9000,
                        "healingTaken": 100,
                        "damageTaken": 10000
                    }
                ],
                "enemyList": [
                    {
                        "bnetId": 3000,
                        "name": "Enemy#3000",
                        "heroGuid": "hero-enemy",
                        "roleType": "dps",
                        "kill": 6,
                        "death": 4,
                        "heroDamage": 5000
                    }
                ]
            }
        }
    })
}

#[test]
fn imports_synthetic_season_and_counts_preset_matches() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    assert_eq!(matches.len(), 3);
    assert!(matches.iter().any(|item| item.match_id == "match-stadium"));
    assert!(matches
        .iter()
        .filter(|item| item.match_id != "match-stadium")
        .all(|item| item.bucket == "preset"));
}

#[test]
fn list_matches_includes_fight_rows_and_uses_last_round_map_preview() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let mut arena_entry = match_entry(
        "match-arena",
        1,
        "SportFight",
        "map-a",
        "hero-current",
        1700003600000_i64,
    );
    arena_entry["teamScore"] = json!(9);
    arena_entry["opponentScore"] = json!(8);
    arena_entry["kill"] = json!(99);
    arena_entry["assist"] = json!(88);
    arena_entry["death"] = json!(77);
    arena_entry["heroDamage"] = json!(99999);
    arena_entry["cure"] = json!(88888);
    arena_entry["resistDamage"] = json!(77777);
    let payload = json!({
        "season": 22,
        "profile_card": { "data": { "bnetId": 1000, "name": "Current#1000" } },
        "matches": {
            "entries": [
                match_entry("match-normal", 1, "LeisurePreset", "map-a", "hero-current", 1700000000000_i64),
                arena_entry
            ],
            "details": [
                match_detail("match-normal", "normal", "map-a", 600),
                {
                    "match_id": "match-arena",
                    "match_kind": "fight",
                    "payload": {
                        "data": {
                            "matchRet": 1,
                            "totalCount": {"mapGuid": "map-a", "teamScore": 9, "opponentScore": 8, "teammateList": [], "enemyList": []},
                            "roundCountList": [
                                {"mapGuid": "map-a", "teamScore": 1, "opponentScore": 0, "teammateList": [], "enemyList": []},
                                {
                                    "mapGuid": "576460752303428183",
                                    "teamScore": 2,
                                    "opponentScore": 0,
                                    "gameTimeSec": 222,
                                    "teammateList": [
                                        {
                                            "bnetId": 1000,
                                            "name": "Current#1000",
                                            "heroGuid": "hero-current",
                                            "kill": 7,
                                            "assist": 6,
                                            "death": 5,
                                            "heroDamage": 7000,
                                            "cure": 6000,
                                            "resistDamage": 5000
                                        }
                                    ],
                                    "enemyList": []
                                }
                            ]
                        }
                    }
                }
            ]
        }
    });

    import_payload_value(&db, &payload, temp.path()).unwrap();
    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    let arena = matches
        .iter()
        .find(|item| item.match_id == "match-arena")
        .expect("fight row in list");

    assert_eq!(arena.bucket, "stadium");
    assert_eq!(arena.mode_label, "角斗竞技");
    assert_eq!(arena.map_guid, "map-a");
    assert_eq!(
        arena.map_preview_guid.as_deref(),
        Some("576460752303428183")
    );
    assert_eq!(arena.map_name, "泰晤士区");
    assert_eq!(arena.team_score, 2);
    assert_eq!(arena.opponent_score, 0);
    assert_eq!(arena.kills, 7);
    assert_eq!(arena.assists, 6);
    assert_eq!(arena.deaths, 5);
    assert_eq!(arena.damage, 7000);
    assert_eq!(arena.healing, 6000);
    assert_eq!(arena.mitigation, 5000);
    assert_eq!(arena.duration_sec, 222);

    let filtered = list_matches(
        &db,
        MatchFilters {
            mode: Some("SportFight".to_string()),
            ..MatchFilters::default()
        },
    )
    .unwrap();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].match_id, "match-arena");
}

#[test]
fn import_prefers_total_count_player_rows_for_full_stat_maps() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = json!({
        "season": 22,
        "profile_card": { "data": { "bnetId": 1000, "name": "Current#1000" } },
        "matches": {
            "entries": [
                match_entry("match-total-count", 1, "LeisurePreset", "map-a", "hero-current", 1700000000000_i64)
            ],
            "details": [
                {
                    "match_id": "match-total-count",
                    "match_kind": "normal",
                    "payload": {
                        "data": {
                            "matchRet": 1,
                            "mapGuid": "map-a",
                            "gameTimeSec": 600,
                            "teammateList": [
                                {
                                    "bnetId": 1000,
                                    "name": "Current#1000",
                                    "heroGuid": "hero-current",
                                    "kill": 5,
                                    "finalHit": 1,
                                    "targetCompetingTime": 30
                                }
                            ],
                            "enemyList": [],
                            "totalCount": {
                                "teammateList": [
                                    {
                                        "bnetId": 1000,
                                        "name": "Current#1000",
                                        "heroGuid": "hero-current",
                                        "kill": 25,
                                        "finalHit": 12,
                                        "targetCompetingTime": 417.4,
                                        "statMap": {
                                            "603482350067646507": 12,
                                            "603482350067648534": 417.4
                                        }
                                    }
                                ],
                                "enemyList": []
                            }
                        }
                    }
                }
            ]
        }
    });

    import_payload_value(&db, &payload, temp.path()).unwrap();
    let detail = get_match_detail(&db, "match-total-count").unwrap();
    let player = detail.teammates.first().expect("teammate row");

    assert_eq!(player.kills, 25);
    assert_eq!(player.final_hits, 12);
    assert_eq!(player.target_time_sec, 417);
    assert_eq!(player.hero_stats.len(), 2);
}

#[test]
fn fight_detail_exposes_arena_summary_rounds_and_safe_names() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = json!({
        "season": 22,
        "profile_card": { "data": { "bnetId": 1000, "name": "Current#1000" } },
        "matches": {
            "entries": [
                match_entry("match-arena", 1, "SportFight", "map-b", "hero-current", 1700000000000_i64)
            ],
            "details": [
                {
                    "match_id": "match-arena",
                    "match_kind": "fight",
                    "payload": {
                        "data": {
                            "matchRet": 1,
                            "nameMap": {
                                "1000": "我方一号",
                                "2000": "我方二号",
                                "3000": "对手一号",
                                "4000": "4000"
                            },
                            "totalCount": {
                                "mapGuid": "map-a",
                                "gameTimeSec": 901,
                                "startTime": 1700000010,
                                "teamScore": 2,
                                "opponentScore": 1,
                                "teammateList": [
                                    {
                                        "bnetId": 1000,
                                        "heroGuid": "hero-current",
                                        "kill": 8,
                                        "assist": 4,
                                        "death": 1,
                                        "heroDamage": 3200,
                                        "cure": 100,
                                        "resistDamage": 80,
                                        "worth": 17000,
                                        "endWorth": 17250,
                                        "worthMax": true,
                                        "modGuids": ["mod-a", "mod-b"],
                                        "traitGuids": ["trait-a"],
                                        "killMax": true,
                                        "heroDamageMax": true,
                                        "cureMax": false,
                                        "resistDamageMax": false,
                                        "rankInfo": {"rankName": "Master", "rankSubTier": 2}
                                    },
                                    {"bnetId": 2000, "heroGuid": "hero-friend", "kill": 3}
                                ],
                                "enemyList": [
                                    {"bnetId": 3000, "heroGuid": "hero-enemy", "kill": 5},
                                    {"bnetId": 4000, "heroGuid": "hero-enemy", "kill": 2}
                                ]
                            },
                            "roundCountList": [
                                {
                                    "mapGuid": "map-a",
                                    "gameTimeSec": 301,
                                    "startTime": 1700000010,
                                    "teamScore": 1,
                                    "opponentScore": 0,
                                    "teammateList": [{"bnetId": 2000, "heroGuid": "hero-friend", "kill": 2}],
                                    "enemyList": [{"bnetId": 3000, "heroGuid": "hero-enemy", "kill": 1}]
                                },
                                {
                                    "mapGuid": "576460752303428272",
                                    "gameTimeSec": 310,
                                    "startTime": 1700000330,
                                    "teamScore": 1,
                                    "opponentScore": 1,
                                    "teammateList": [
                                        {
                                            "bnetId": 1000,
                                            "heroGuid": "hero-current",
                                            "kill": 4,
                                            "damageTaken": 100,
                                            "healingTaken": 200
                                        }
                                    ],
                                    "enemyList": [{"bnetId": 4000, "heroGuid": "hero-enemy", "kill": 3}]
                                },
                                {
                                    "mapGuid": "map-a",
                                    "gameTimeSec": 290,
                                    "startTime": 1700000650,
                                    "teamScore": 2,
                                    "opponentScore": 1,
                                    "teammateList": [
                                        {
                                            "bnetId": 1000,
                                            "heroGuid": "hero-current",
                                            "kill": 22,
                                            "assist": 5,
                                            "death": 2,
                                            "heroDamage": 9000,
                                            "cure": 1200,
                                            "resistDamage": 700,
                                            "finalHit": 6,
                                            "damageTaken": 300,
                                            "healingTaken": 400,
                                            "worth": 900,
                                            "endWorth": 1500,
                                            "worthMax": true,
                                            "modGuids": ["mod-last"],
                                            "traitGuids": ["trait-last"],
                                            "killMax": true,
                                            "heroDamageMax": true,
                                            "cureMax": false,
                                            "resistDamageMax": true,
                                            "rankInfo": {"rankName": "Master", "rankSubTier": 2}
                                        },
                                        {
                                            "bnetId": 2000,
                                            "heroGuid": "hero-friend",
                                            "kill": 7,
                                            "damageTaken": 50,
                                            "healingTaken": 60
                                        }
                                    ],
                                    "enemyList": [
                                        {
                                            "bnetId": 3000,
                                            "heroGuid": "hero-enemy",
                                            "kill": 2,
                                            "damageTaken": 30,
                                            "healingTaken": 40
                                        },
                                        {
                                            "bnetId": 4000,
                                            "heroGuid": "hero-enemy",
                                            "kill": 1,
                                            "damageTaken": 70,
                                            "healingTaken": 80
                                        }
                                    ]
                                }
                            ]
                        }
                    }
                }
            ]
        }
    });

    import_payload_value(&db, &payload, temp.path()).unwrap();
    let detail = get_match_detail(&db, "match-arena").unwrap();
    assert!(detail.raw_json.is_null());
    let arena = detail.arena_detail.expect("arena detail");

    assert_eq!(detail.mode_label.as_deref(), Some("角斗竞技"));
    assert_eq!(arena.summary.label, "角斗汇总");
    assert_eq!(arena.summary.team_score, Some(2));
    assert_eq!(arena.summary.opponent_score, Some(1));
    assert_eq!(arena.summary.duration_sec, Some(901));
    assert_eq!(arena.summary.begin_ts, Some(1700000000));
    assert_eq!(arena.summary.map_guid.as_deref(), Some("map-a"));
    assert_eq!(arena.summary.map_name.as_deref(), Some("漓江塔"));
    assert_eq!(arena.summary.teammates[0].name, "我方一号");
    assert_eq!(arena.summary.teammates[1].name, "我方二号");
    assert_eq!(arena.summary.enemies[0].name, "对手一号");
    assert_eq!(arena.summary.enemies[1].name, "未知玩家");
    assert_eq!(
        arena.summary.teammates[0].rank_label.as_deref(),
        Some("大师 2")
    );
    assert_eq!(arena.summary.teammates[0].kills, 22);
    assert_eq!(arena.summary.teammates[0].assists, 5);
    assert_eq!(arena.summary.teammates[0].deaths, 2);
    assert_eq!(arena.summary.teammates[0].damage, 9000);
    assert_eq!(arena.summary.teammates[0].healing, 1200);
    assert_eq!(arena.summary.teammates[0].mitigation, 700);
    assert_eq!(arena.summary.teammates[0].final_hits, 6);
    assert_eq!(arena.summary.teammates[0].damage_taken, 400);
    assert_eq!(arena.summary.teammates[0].healing_taken, 600);
    assert_eq!(arena.summary.teammates[1].damage_taken, 50);
    assert_eq!(arena.summary.teammates[1].healing_taken, 60);
    assert_eq!(arena.summary.enemies[0].damage_taken, 30);
    assert_eq!(arena.summary.enemies[0].healing_taken, 40);
    assert_eq!(arena.summary.enemies[1].damage_taken, 70);
    assert_eq!(arena.summary.enemies[1].healing_taken, 80);
    assert_eq!(arena.summary.teammates[0].worth, Some(900));
    assert_eq!(arena.summary.teammates[0].end_worth, Some(1500));
    assert!(arena.summary.teammates[0].worth_max);
    assert_eq!(arena.summary.teammates[0].mod_guids, vec!["mod-last"]);
    assert_eq!(arena.summary.teammates[0].trait_guids, vec!["trait-last"]);
    assert!(arena.summary.teammates[0].kill_max);
    assert!(arena.summary.teammates[0].hero_damage_max);
    assert!(arena.summary.teammates[0].resist_damage_max);
    assert!(arena
        .summary
        .teammates
        .iter()
        .chain(arena.summary.enemies.iter())
        .all(|player| !["1000", "2000", "3000", "4000"].contains(&player.name.as_str())));
    assert_eq!(arena.rounds.len(), 3);
    assert_eq!(arena.rounds[0].label, "回合 1");
    assert_eq!(arena.rounds[0].result.as_deref(), Some("win"));
    assert_eq!(arena.rounds[1].result.as_deref(), Some("loss"));
    assert_eq!(arena.rounds[1].map_name.as_deref(), Some("埃斯佩兰萨"));
    assert_eq!(arena.rounds[2].result.as_deref(), Some("win"));
    assert_eq!(arena.rounds[0].teammates[0].name, "我方二号");
    assert_eq!(arena.rounds[1].enemies[0].name, "未知玩家");
    assert_eq!(arena.rounds[2].teammates[0].kills, 22);
    assert_eq!(arena.rounds[2].teammates[0].damage, 9000);
    assert_eq!(arena.rounds[2].teammates[0].worth, Some(900));
    assert_eq!(arena.rounds[2].teammates[0].end_worth, Some(1500));
    assert!(arena.rounds[2].teammates[0].worth_max);
    assert_eq!(arena.rounds[2].teammates[0].mod_guids, vec!["mod-last"]);
    assert_eq!(arena.rounds[2].teammates[0].trait_guids, vec!["trait-last"]);
}

#[test]
fn fight_summary_falls_back_to_round_duration_sum_and_first_round_start() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = json!({
        "season": 22,
        "profile_card": { "data": { "bnetId": 1000, "name": "Current#1000" } },
        "matches": {
            "entries": [
                {
                    "matchId": "match-arena-fallback",
                    "matchRet": 1,
                    "mapGuid": "map-a",
                    "heroGuid": "hero-current",
                    "gameMode": "SportFight",
                    "_dashenSeason": 22,
                    "teamScore": 2,
                    "opponentScore": 0
                }
            ],
            "details": [
                {
                    "match_id": "match-arena-fallback",
                    "match_kind": "fight",
                    "payload": {
                        "data": {
                            "matchRet": 1,
                            "nameMap": {},
                            "totalCount": {
                                "mapGuid": "map-a",
                                "teamScore": 2,
                                "opponentScore": 0,
                                "teammateList": [],
                                "enemyList": []
                            },
                            "roundCountList": [
                                {
                                    "mapGuid": "map-a",
                                    "gameTimeSec": 111,
                                    "startTime": 1700000100,
                                    "teamScore": 1,
                                    "opponentScore": 0,
                                    "teammateList": [],
                                    "enemyList": []
                                },
                                {
                                    "mapGuid": "map-b",
                                    "gameTimeSec": 222,
                                    "startTime": 1700000300,
                                    "teamScore": 2,
                                    "opponentScore": 0,
                                    "teammateList": [],
                                    "enemyList": []
                                }
                            ]
                        }
                    }
                }
            ]
        }
    });

    import_payload_value(&db, &payload, temp.path()).unwrap();
    let detail = get_match_detail(&db, "match-arena-fallback").unwrap();
    let arena = detail.arena_detail.expect("arena detail");

    assert_eq!(arena.summary.duration_sec, Some(333));
    assert_eq!(arena.summary.begin_ts, Some(1700000100));
}

#[test]
fn database_initialize_sets_schema_version_idempotently() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));

    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute("DROP TABLE match_ai_roasts", []).unwrap();
    conn.pragma_update(None, "user_version", 9_i64).unwrap();
    drop(conn);

    db.initialize().unwrap();
    db.initialize().unwrap();

    let conn = db.connect().unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();

    assert_eq!(version, 12);
    let roast_table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='match_ai_roasts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(roast_table_count, 1);
}

#[test]
fn database_migration_backfills_umbrella_normal_match_modes() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    let cases = [
        ("quick-preset", "leisure", "IT_UNRANKED", "LeisurePreset"),
        ("quick-6v6", "leisure", "IT_RULESET", "Leisure6v6"),
        ("ranked-preset", "sport", "IT_RANKED", "SportPreset"),
        ("ranked-open", "sport", "IT_RULESET", "SportOpen"),
    ];
    for (index, (match_id, game_mode, instance_type, _)) in cases.iter().enumerate() {
        conn.execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
                hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
                deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
            ) VALUES (?1, 21, 'preset', ?2, 'normal', 'win', 'map', '地图', 'hero', '英雄',
                ?3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 60, ?4, '2026-07-16T00:00:00Z')
            "#,
            rusqlite::params![
                match_id,
                game_mode,
                index as i64,
                json!({"gameMode": game_mode, "instanceType": instance_type}).to_string()
            ],
        )
        .unwrap();
    }
    conn.pragma_update(None, "user_version", 6_i64).unwrap();
    drop(conn);

    db.initialize().unwrap();
    db.initialize().unwrap();

    let conn = db.connect().unwrap();
    for (match_id, _, _, expected_mode) in cases {
        let stored_mode: String = conn
            .query_row(
                "SELECT game_mode FROM matches WHERE match_id = ?1",
                [match_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_mode, expected_mode, "unexpected mode for {match_id}");
    }
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 12);
}

#[test]
fn database_migration_backfills_legacy_builtin_map_names() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute("DELETE FROM map_dict", []).unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (
            'legacy-map', 21, 'preset', 'LeisurePreset', 'normal', 'win',
            '576460752303425207', NULL, 'hero-current', '安娜', 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, '{}', '2026-07-12T00:00:00Z'
        )
        "#,
        [],
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 4_i64).unwrap();
    drop(conn);

    db.initialize().unwrap();

    let conn = db.connect().unwrap();
    let map_name: String = conn
        .query_row(
            "SELECT map_name FROM matches WHERE match_id = 'legacy-map'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let dictionary_name: String = conn
        .query_row(
            "SELECT name FROM map_dict WHERE guid = '576460752303425207'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(map_name, "漓江塔");
    assert_eq!(dictionary_name, "漓江塔");
}

#[test]
fn database_migrates_legacy_match_players_role_type() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let conn = db.connect().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE match_players (
            match_id TEXT NOT NULL,
            season INTEGER NOT NULL,
            side TEXT NOT NULL,
            bnet_id TEXT NOT NULL,
            player_name TEXT NOT NULL,
            hero_guid TEXT NOT NULL,
            hero_name TEXT,
            is_current_player INTEGER NOT NULL DEFAULT 0,
            is_friend INTEGER NOT NULL DEFAULT 0,
            kill INTEGER NOT NULL DEFAULT 0,
            assist INTEGER NOT NULL DEFAULT 0,
            death INTEGER NOT NULL DEFAULT 0,
            hero_damage INTEGER NOT NULL DEFAULT 0,
            cure INTEGER NOT NULL DEFAULT 0,
            resist_damage INTEGER NOT NULL DEFAULT 0,
            healing_taken INTEGER NOT NULL DEFAULT 0,
            damage_taken INTEGER NOT NULL DEFAULT 0,
            final_hit INTEGER NOT NULL DEFAULT 0,
            target_competing_time INTEGER NOT NULL DEFAULT 0,
            raw_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (match_id, side, bnet_id)
        );
        PRAGMA user_version = 1;
        "#,
    )
    .unwrap();
    drop(conn);

    db.initialize().unwrap();

    let conn = db.connect().unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let role_type_columns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('match_players') WHERE name='role_type'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let role_index_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_index_list('match_players') WHERE name='idx_match_players_role'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 12);
    assert_eq!(role_type_columns, 1);
    assert_eq!(role_index_count, 1);
}

#[test]
fn database_migrates_v2_arena_player_index_idempotently() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let conn = db.connect().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE matches (
            match_id TEXT PRIMARY KEY,
            season INTEGER NOT NULL,
            bucket TEXT NOT NULL,
            game_mode TEXT NOT NULL,
            match_kind TEXT NOT NULL,
            result TEXT NOT NULL,
            map_guid TEXT NOT NULL,
            map_name TEXT,
            hero_guid TEXT NOT NULL,
            hero_name TEXT,
            begin_ts INTEGER NOT NULL,
            team_score INTEGER NOT NULL,
            opponent_score INTEGER NOT NULL,
            kills INTEGER NOT NULL,
            assists INTEGER NOT NULL,
            deaths INTEGER NOT NULL,
            damage INTEGER NOT NULL,
            healing INTEGER NOT NULL,
            mitigation INTEGER NOT NULL,
            total_worth INTEGER NOT NULL,
            duration_sec INTEGER NOT NULL,
            raw_entry_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE match_players (
            match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
            season INTEGER NOT NULL,
            side TEXT NOT NULL,
            bnet_id TEXT NOT NULL,
            player_name TEXT NOT NULL,
            hero_guid TEXT NOT NULL,
            hero_name TEXT,
            is_current_player INTEGER NOT NULL DEFAULT 0,
            is_friend INTEGER NOT NULL DEFAULT 0,
            kill INTEGER NOT NULL DEFAULT 0,
            assist INTEGER NOT NULL DEFAULT 0,
            death INTEGER NOT NULL DEFAULT 0,
            hero_damage INTEGER NOT NULL DEFAULT 0,
            cure INTEGER NOT NULL DEFAULT 0,
            resist_damage INTEGER NOT NULL DEFAULT 0,
            healing_taken INTEGER NOT NULL DEFAULT 0,
            damage_taken INTEGER NOT NULL DEFAULT 0,
            final_hit INTEGER NOT NULL DEFAULT 0,
            target_competing_time INTEGER NOT NULL DEFAULT 0,
            role_type TEXT,
            raw_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (match_id, side, bnet_id)
        );

        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (
            'legacy-v2-match', 22, 'stadium', 'SportFight', 'fight', 'win', 'map-a', NULL,
            'hero-a', NULL, 1700000000, 2, 1, 1, 2,
            3, 4, 5, 6, 7, 8, '{}', '2026-07-11T00:00:00Z'
        );

        PRAGMA user_version = 2;
        "#,
    )
    .unwrap();
    drop(conn);

    db.initialize().unwrap();
    db.initialize().unwrap();

    let conn = db.connect().unwrap();
    let version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let arena_table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='arena_match_players'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let arena_columns: String = conn
        .query_row(
            "SELECT group_concat(name, ',') FROM pragma_table_info('arena_match_players') ORDER BY cid",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let primary_key_columns: String = conn
        .query_row(
            "SELECT group_concat(name, ',') FROM pragma_table_info('arena_match_players') WHERE pk > 0 ORDER BY pk",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let foreign_key_definition = conn
        .query_row(
            "SELECT \"table\" || ',' || \"from\" || ',' || on_delete FROM pragma_foreign_key_list('arena_match_players')",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    let bnet_index_columns: String = conn
        .query_row(
            "SELECT group_concat(name, ',') FROM pragma_index_info('idx_arena_match_players_bnet') ORDER BY seqno",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let name_index_columns: String = conn
        .query_row(
            "SELECT group_concat(name, ',') FROM pragma_index_info('idx_arena_match_players_name') ORDER BY seqno",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let explicit_indexes: String = conn
        .query_row(
            "SELECT group_concat(name, ',') FROM (SELECT name FROM pragma_index_list('arena_match_players') WHERE origin='c' ORDER BY name)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let preserved_matches: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM matches WHERE match_id='legacy-v2-match'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(version, 12);
    assert_eq!(arena_table_count, 1);
    assert_eq!(
        arena_columns,
        "match_id,season,side,bnet_id,player_name,updated_at"
    );
    assert_eq!(primary_key_columns, "match_id,side,bnet_id");
    assert_eq!(foreign_key_definition, "matches,match_id,CASCADE");
    assert_eq!(
        explicit_indexes,
        "idx_arena_match_players_bnet,idx_arena_match_players_name"
    );
    assert_eq!(bnet_index_columns, "bnet_id,season");
    assert_eq!(name_index_columns, "player_name,season");
    assert_eq!(preserved_matches, 1);
}

#[test]
fn arena_player_projection_indexes_total_and_round_players_safely_and_idempotently() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = json!({
        "season": 22,
        "matches": {
            "entries": [
                {
                    "matchId": "arena-projection",
                    "matchRet": 1,
                    "mapGuid": "map-a",
                    "heroGuid": "hero-current",
                    "gameMode": "SportFight",
                    "_dashenSeason": 22
                }
            ],
            "details": [
                {
                    "match_id": "arena-projection",
                    "match_kind": "fight",
                    "payload": {
                        "data": {
                            "nameMap": {
                                "9400": "回合玩家",
                                "9500": "95001234"
                            },
                            "totalCount": {
                                "teammateList": [
                                    {"bnetId": 9100, "name": "总计玩家"},
                                    {"bnetId": 9200, "name": "92001234"}
                                ],
                                "enemyList": [
                                    {
                                        "bnetId": 9300,
                                        "name": "0f8fad5b-d9cb-469f-a165-70867728950e"
                                    }
                                ]
                            },
                            "roundCountList": [
                                {
                                    "teammateList": [
                                        {"bnetId": 9100, "name": "回合更新玩家"},
                                        {"bnetId": 9400}
                                    ],
                                    "enemyList": [
                                        {"bnetId": 9500}
                                    ]
                                }
                            ]
                        }
                    }
                }
            ]
        }
    });

    import_payload_value(&db, &payload, temp.path()).unwrap();
    import_payload_value(&db, &payload, temp.path()).unwrap();

    let conn = db.connect().unwrap();
    let rows = {
        let mut statement = conn
            .prepare(
                "SELECT side, bnet_id, player_name FROM arena_match_players \
                 WHERE season = 22 ORDER BY bnet_id",
            )
            .unwrap();
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    let marker: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key='arena_player_index_v1:22'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows[0],
        ("teammate".into(), "9100".into(), "回合更新玩家".into())
    );
    assert_eq!(
        rows[1],
        ("teammate".into(), "9200".into(), "未知玩家".into())
    );
    assert_eq!(rows[2], ("enemy".into(), "9300".into(), "未知玩家".into()));
    assert_eq!(
        rows[3],
        ("teammate".into(), "9400".into(), "回合玩家".into())
    );
    assert_eq!(rows[4], ("enemy".into(), "9500".into(), "未知玩家".into()));
    assert_eq!(marker, "complete");
}

#[test]
fn arena_player_rebuild_only_replaces_its_season_and_marks_empty_projection_complete() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = |season: i64, match_id: &str, players: Value| {
        json!({
            "season": season,
            "matches": {
                "entries": [
                    {
                        "matchId": match_id,
                        "matchRet": 1,
                        "mapGuid": "map-a",
                        "heroGuid": "hero-current",
                        "gameMode": "SportFight",
                        "_dashenSeason": season
                    }
                ],
                "details": [
                    {
                        "match_id": match_id,
                        "match_kind": "fight",
                        "payload": {
                            "data": {
                                "totalCount": {
                                    "teammateList": players,
                                    "enemyList": []
                                },
                                "roundCountList": []
                            }
                        }
                    }
                ]
            }
        })
    };

    import_payload_value(
        &db,
        &payload(
            21,
            "arena-s21",
            json!([{"bnetId": 2100, "name": "S21 玩家"}]),
        ),
        temp.path(),
    )
    .unwrap();
    import_payload_value(
        &db,
        &payload(
            22,
            "arena-s22",
            json!([{"bnetId": 2200, "name": "S22 玩家"}]),
        ),
        temp.path(),
    )
    .unwrap();
    import_payload_value(&db, &payload(22, "arena-s22", json!([])), temp.path()).unwrap();

    let conn = db.connect().unwrap();
    let s21_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM arena_match_players WHERE season = 21",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let s22_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM arena_match_players WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let complete_markers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM app_settings \
             WHERE key IN ('arena_player_index_v1:21', 'arena_player_index_v1:22') \
               AND value = 'complete'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(s21_rows, 1);
    assert_eq!(s22_rows, 0);
    assert_eq!(complete_markers, 2);
}

#[test]
fn arena_player_searches_by_name_and_bnet_without_returning_private_ids_and_filters_matches() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = json!({
        "season": 22,
        "matches": {
            "entries": [{
                "matchId": "arena-query",
                "matchRet": 1,
                "mapGuid": "map-a",
                "heroGuid": "hero-current",
                "gameMode": "SportFight",
                "beginTs": 1700007200000_i64,
                "_dashenSeason": 22
            }],
            "details": [{
                "match_id": "arena-query",
                "match_kind": "fight",
                "payload": {"data": {"totalCount": {
                    "teammateList": [{"bnetId": 22001234, "name": "稳定玩家名"}],
                    "enemyList": []
                }, "roundCountList": []}}
            }]
        }
    });
    import_payload_value(&db, &payload, temp.path()).unwrap();

    let by_name = search_match_players(&db, "稳定玩家").unwrap();
    let by_bnet = search_match_players(&db, "22001234").unwrap();
    let filtered = list_matches(
        &db,
        MatchFilters {
            season: Some(22),
            player_name: Some("稳定玩家名".into()),
            ..MatchFilters::default()
        },
    )
    .unwrap();

    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].player_name, "稳定玩家名");
    assert_eq!(by_bnet.len(), 1);
    assert_eq!(by_bnet[0].player_name, "稳定玩家名");
    assert!(!serde_json::to_string(&by_bnet)
        .unwrap()
        .contains("22001234"));
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].match_id, "arena-query");
}

#[test]
fn arena_player_backfill_detects_each_unmarked_fight_season() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = |season: i64, match_id: &str, bnet_id: i64, name: &str| {
        json!({
            "season": season,
            "matches": {
                "entries": [{
                    "matchId": match_id,
                    "matchRet": 1,
                    "mapGuid": "map-a",
                    "heroGuid": "hero-current",
                    "gameMode": "SportFight",
                    "_dashenSeason": season
                }],
                "details": [{
                    "match_id": match_id,
                    "match_kind": "fight",
                    "payload": {"data": {"totalCount": {
                        "teammateList": [{"bnetId": bnet_id, "name": name}],
                        "enemyList": []
                    }, "roundCountList": []}}
                }]
            }
        })
    };
    import_payload_value(
        &db,
        &payload(21, "arena-s21", 2100, "S21 玩家"),
        temp.path(),
    )
    .unwrap();
    import_payload_value(
        &db,
        &payload(22, "arena-s22", 2200, "S22 玩家"),
        temp.path(),
    )
    .unwrap();
    let conn = db.connect().unwrap();
    conn.execute("DELETE FROM arena_match_players WHERE season = 22", [])
        .unwrap();
    conn.execute(
        "DELETE FROM app_settings WHERE key = 'arena_player_index_v1:22'",
        [],
    )
    .unwrap();
    drop(conn);

    let matches = list_matches(
        &db,
        MatchFilters {
            season: Some(22),
            player_name: Some("S22 玩家".into()),
            ..MatchFilters::default()
        },
    )
    .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].match_id, "arena-s22");
}

#[test]
fn arena_player_complete_marker_prevents_rebuilding_zero_player_season() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = json!({
        "season": 22,
        "matches": {
            "entries": [{
                "matchId": "arena-empty",
                "matchRet": 1,
                "mapGuid": "map-a",
                "heroGuid": "hero-current",
                "gameMode": "SportFight",
                "_dashenSeason": 22
            }],
            "details": [{
                "match_id": "arena-empty",
                "match_kind": "fight",
                "payload": {"data": {"totalCount": {
                    "teammateList": [], "enemyList": []
                }, "roundCountList": []}}
            }]
        }
    });
    import_payload_value(&db, &payload, temp.path()).unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "UPDATE app_settings SET updated_at = 'sentinel' WHERE key = 'arena_player_index_v1:22'",
        [],
    )
    .unwrap();
    drop(conn);

    search_match_players(&db, "nobody").unwrap();
    list_matches(&db, MatchFilters::default()).unwrap();

    let updated_at: String = db
        .connect()
        .unwrap()
        .query_row(
            "SELECT updated_at FROM app_settings WHERE key = 'arena_player_index_v1:22'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(updated_at, "sentinel");
}

#[test]
fn filters_matches_by_season_result_mode_hero_map_and_date_range() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let first = list_matches(&db, MatchFilters::default())
        .unwrap()
        .into_iter()
        .find(|item| item.result == "win")
        .unwrap();
    let filtered = list_matches(
        &db,
        MatchFilters {
            season: Some(first.season),
            hero: Some(first.hero_guid.clone()),
            map: Some(first.map_guid.clone()),
            result: Some(first.result.clone()),
            mode: Some(first.game_mode.clone()),
            primary_tag: None,
            player_name: None,
            date_from: Some(first.begin_label[0..10].to_string()),
            date_to: Some(first.begin_label[0..10].to_string()),
            favorite_only: None,
        },
    )
    .unwrap();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].match_id, first.match_id);
}

#[test]
fn detail_exposes_teammates_enemies_and_unmodified_raw_json() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let detail = get_match_detail(&db, "match-win").unwrap();
    let raw = serde_json::to_string(&detail.raw_json).unwrap();

    assert_eq!(detail.match_id, "match-win");
    assert_eq!(detail.teammates.len(), 2);
    assert_eq!(detail.enemies.len(), 1);
    assert_eq!(detail.teammates[0].hero_name, "安娜");
    assert!(raw.contains("detail-secret"));
    assert!(raw.contains("entry-secret"));
    assert!(raw.contains("customerToken"));
    assert!(raw.contains("token\""));
}

#[test]
fn filter_options_are_populated_from_imported_matches() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let options = list_filter_options(&db).unwrap();

    assert_eq!(options.seasons, vec![22]);
    assert!(options
        .heroes
        .iter()
        .any(|option| option.id == "hero-current"));
    assert!(options.maps.iter().any(|option| option.id == "map-a"));
    assert!(options.results.contains(&"win".to_string()));
    assert!(options.modes.contains(&"LeisurePreset".to_string()));
}

#[test]
fn filter_options_expose_only_the_six_supported_match_modes() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for (index, game_mode) in ["leisure", "sport"].iter().enumerate() {
        conn.execute(
            "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name, hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, 22, 'preset', ?2, 'normal', 'win', 'map', '地图', 'hero', '英雄', ?3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 60, '{}', '2026-07-12T00:00:00Z')",
            rusqlite::params![format!("legacy-mode-{index}"), game_mode, index as i64],
        )
        .unwrap();
    }

    let options = list_filter_options(&db).unwrap();

    assert_eq!(
        options.modes,
        vec![
            "Leisure6v6",
            "LeisurePreset",
            "SportPreset",
            "SportOpen",
            "SportFight",
            "LeisureFight",
        ]
    );
}

#[test]
fn imports_umbrella_normal_modes_as_supported_match_modes() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let cases = [
        ("quick-preset", "leisure", "IT_UNRANKED", "LeisurePreset"),
        ("quick-6v6", "leisure", "IT_RULESET", "Leisure6v6"),
        ("ranked-preset", "sport", "IT_RANKED", "SportPreset"),
        ("ranked-open", "sport", "IT_RULESET", "SportOpen"),
    ];
    let entries = cases
        .iter()
        .enumerate()
        .map(|(index, (match_id, game_mode, instance_type, _))| {
            json!({
                "matchId": match_id,
                "matchRet": 1,
                "mapGuid": "map-a",
                "heroGuid": "hero-current",
                "beginTs": 1_700_000_000_000_i64 + index as i64,
                "gameMode": game_mode,
                "instanceType": instance_type
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "season": 21,
        "matches": {
            "entries": entries,
            "details": [],
            "detail_errors": []
        }
    });

    import_payload_value(&db, &payload, temp.path()).unwrap();

    let conn = db.connect().unwrap();
    for (match_id, _, _, expected_mode) in cases {
        let stored_mode: String = conn
            .query_row(
                "SELECT game_mode FROM matches WHERE match_id = ?1",
                [match_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_mode, expected_mode, "unexpected mode for {match_id}");
    }
}

#[test]
fn imports_negative_match_ret_as_loss() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let matches = list_matches(
        &db,
        MatchFilters {
            result: Some("loss".to_string()),
            ..MatchFilters::default()
        },
    )
    .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].result, "loss");
    assert_eq!(matches[0].result_label, "失败");
}

#[test]
fn import_preserves_natural_raw_json_for_entries_details_errors_and_profile() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_payload_value(
        &db,
        &json!({
            "season": 22,
            "profile_card": {"data": {
                "bnetId": "synthetic-profile",
                "name": "Synthetic#1000",
                "customerToken": "synthetic-profile-token"
            }},
            "matches": {
                "entries": [{
                    "matchId": "raw-storage-match",
                    "gameMode": "SportPreset",
                    "beginTs": 1_700_000_000_000_i64,
                    "token": "synthetic-entry-token"
                }],
                "details": [{
                    "match_id": "raw-storage-match",
                    "match_kind": "normal",
                    "payload": {"code": 0, "data": {"cookie": "synthetic-detail-cookie"}}
                }],
                "detail_errors": [{
                    "match_id": "raw-storage-error",
                    "error_type": "DetailFetchError",
                    "message": "synthetic failure",
                    "authorization": "synthetic-error-authorization"
                }]
            }
        }),
        temp.path(),
    )
    .unwrap();

    let conn = db.connect().unwrap();
    let entry: String = conn
        .query_row(
            "SELECT raw_entry_json FROM matches WHERE match_id='raw-storage-match'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let detail: String = conn
        .query_row(
            "SELECT raw_detail_json FROM match_details WHERE match_id='raw-storage-match'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let error: String = conn
        .query_row(
            "SELECT raw_json FROM sync_errors WHERE match_id='raw-storage-error'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let profile: String = conn
        .query_row(
            "SELECT raw_json FROM players WHERE player_id='synthetic-profile'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(entry.contains("synthetic-entry-token"));
    assert!(detail.contains("synthetic-detail-cookie"));
    assert!(error.contains("synthetic-error-authorization"));
    assert!(profile.contains("synthetic-profile-token"));
}

#[test]
fn import_rebuilds_match_players_for_preset_matches_only() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);
    let conn = db.connect().unwrap();

    let player_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))
        .unwrap();
    let stadium_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM match_players WHERE match_id IN (SELECT match_id FROM matches WHERE bucket='stadium')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let friend_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM match_players WHERE side='teammate' AND is_friend=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let projection_raw: String = conn
        .query_row(
            "SELECT raw_json FROM match_players WHERE raw_json LIKE '%customerToken%' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();

    assert_eq!(player_rows, 6);
    assert_eq!(stadium_rows, 0);
    assert_eq!(friend_rows, 2);
    assert!(projection_raw.contains("customerToken"));
}

#[test]
fn import_persists_profile_card_for_command_center() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_payload_value(&db, &season_payload(), temp.path()).unwrap();
    let conn = db.connect().unwrap();

    let raw_json: String = conn
        .query_row(
            "SELECT raw_json FROM players WHERE bnet_id = '1000'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert!(raw_json.contains("\"name\":\"Current#1000\""));
    assert!(raw_json.contains("customerToken"));
    assert!(raw_json.contains("customer-secret"));
}

#[test]
fn friend_analytics_backfills_empty_match_players_from_existing_details() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);
    let conn = db.connect().unwrap();
    conn.execute("DELETE FROM match_players", []).unwrap();
    drop(conn);

    let analytics = get_friend_analytics(&db, FriendAnalyticsFilters::default()).unwrap();

    assert!(analytics.summary.friend_rows > 0);
    let conn = db.connect().unwrap();
    let player_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))
        .unwrap();
    assert!(player_rows > 0);
}

#[allow(clippy::too_many_arguments)] // Test fixture maps directly to the persisted player columns.
fn seed_friend_row(
    db: &OwDatabase,
    match_id: &str,
    bnet_id: &str,
    name: &str,
    begin_ts: i64,
    kills: i64,
    deaths: i64,
    resist_damage: i64,
    healing_taken: i64,
    damage_taken: i64,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT OR IGNORE INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'preset', 'SportPreset', 'normal', 'win', 'map-a', '漓江塔',
            'hero-current', '安娜', ?2, 2, 1, 1, 1, 1, 1000, 1000, 100, 0, 600, '{}',
            '2026-06-27T00:00:00Z')
        "#,
        rusqlite::params![match_id, begin_ts],
    )
    .unwrap();
    conn.execute(
        r#"
        INSERT INTO match_players (
            match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
            is_current_player, is_friend, kill, assist, death, hero_damage, cure,
            resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
            raw_json, updated_at
        )
        VALUES (?1, 22, 'teammate', ?2, ?3, 'hero-friend', '莱因哈特',
            0, 1, ?4, 0, ?5, 1000, 0, ?6, ?7, ?8, 0, 0, '{}',
            '2026-06-27T00:00:00Z')
        "#,
        rusqlite::params![
            match_id,
            bnet_id,
            name,
            kills,
            deaths,
            resist_damage,
            healing_taken,
            damage_taken
        ],
    )
    .unwrap();
}

#[test]
fn friend_analytics_uses_block_for_front_pressure_and_adds_healing_conversion_board() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();

    let friends = [
        ("front", "格挡前排#1", 2, 3, 9000, 100, 10000),
        ("care", "重点照顾#1", 1, 8, 100, 9000, 3000),
        ("low", "省资源#1", 2, 1, 100, 100, 2000),
        ("convert", "转化击杀#1", 8, 2, 200, 5000, 2000),
        ("middle", "普通好友#1", 2, 3, 200, 1000, 2500),
    ];
    for (friend_index, (bnet_id, name, kills, deaths, mitigation, healing_taken, damage_taken)) in
        friends.iter().enumerate()
    {
        for match_index in 0..5 {
            seed_friend_row(
                &db,
                &format!("m-{friend_index}-{match_index}"),
                bnet_id,
                name,
                (friend_index * 10 + match_index) as i64,
                *kills,
                *deaths,
                *mitigation,
                *healing_taken,
                *damage_taken,
            );
        }
    }

    let analytics = get_friend_analytics(&db, FriendAnalyticsFilters::default()).unwrap();
    let keys = analytics
        .boards
        .iter()
        .map(|board| board.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            "front_pressure",
            "low_resource_partner",
            "priority_care",
            "healing_conversion"
        ]
    );
    let front = analytics
        .boards
        .iter()
        .find(|board| board.key == "front_pressure")
        .unwrap();
    assert_eq!(front.friends[0].bnet_id, "front");
    assert!(front.description.contains("格挡"));
    let conversion = analytics
        .boards
        .iter()
        .find(|board| board.key == "healing_conversion")
        .unwrap();
    assert_eq!(conversion.friends[0].bnet_id, "convert");
    assert!(conversion.label.contains("转化"));
}

#[test]
fn friend_boards_use_review_names_and_return_only_two_friends() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();

    let friends = [
        ("front-a", "Front A", 3, 3, 9000, 100, 9000),
        ("front-b", "Front B", 3, 3, 9000, 100, 9000),
        ("front-c", "Front C", 3, 3, 9000, 100, 9000),
        ("low", "Low", 2, 1, 100, 100, 1000),
        ("care", "Care", 1, 8, 100, 9000, 3000),
        ("middle", "Middle", 2, 3, 200, 1000, 2500),
    ];
    for (friend_index, (bnet_id, name, kills, deaths, mitigation, healing_taken, damage_taken)) in
        friends.iter().enumerate()
    {
        for match_index in 0..5 {
            seed_friend_row(
                &db,
                &format!("review-board-{friend_index}-{match_index}"),
                bnet_id,
                name,
                (friend_index * 10 + match_index) as i64,
                *kills,
                *deaths,
                *mitigation,
                *healing_taken,
                *damage_taken,
            );
        }
    }

    let analytics = get_friend_analytics(&db, FriendAnalyticsFilters::default()).unwrap();
    let front = analytics
        .boards
        .iter()
        .find(|board| board.key == "front_pressure")
        .unwrap();
    assert_eq!(front.friends.len(), 2);
    assert!(analytics
        .boards
        .iter()
        .any(|board| board.key == "low_resource_partner" && board.label == "在世华佗"));
    assert!(analytics
        .boards
        .iter()
        .any(|board| board.key == "priority_care" && board.label == "资源黑洞"));
}

#[test]
fn sync_import_parts_is_idempotent_and_preserves_detail_errors() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let project_root = temp.path();
    let cached_player = json!({
        "input_bnet": "Tester#1000",
        "resolved_bnet": "Tester#1000",
        "profile_card": {
            "data": {
                "bnetId": 1000,
                "name": "Tester#1000",
                "customerToken": "customer-secret"
            }
        }
    });
    let entries = vec![json!({
        "matchId": "sync-match-1",
        "matchRet": 1,
        "mapGuid": "map-a",
        "heroGuid": "hero-current",
        "beginTs": 1700000000000_i64,
        "gameMode": "LeisurePreset",
        "_dashenSeason": 22,
        "authorization": "entry-auth-secret"
    })];
    let details = vec![json!({
        "match_id": "sync-match-1",
        "match_kind": "normal",
        "payload": {
            "data": {
                "matchRet": 1,
                "customerToken": "detail-customer-secret",
                "mapGuid": "map-a",
                "gameTimeSec": 600,
                "teammateList": [
                    {"bnetId": 1000, "name": "Tester#1000", "heroGuid": "hero-current", "friendBnetIds": [2000], "kill": 1},
                    {"bnetId": 2000, "name": "Friend#2000", "heroGuid": "hero-friend", "kill": 2, "healingTaken": 100, "damageTaken": 200}
                ],
                "enemyList": [
                    {"bnetId": 3000, "name": "Enemy#3000", "heroGuid": "hero-enemy", "kill": 3}
                ]
            }
        }
    })];
    let detail_errors = vec![json!({
        "match_id": "sync-match-2",
        "error_type": "DetailFetchError",
        "message": "request failed token=token-secret customerToken=customer-secret cookie=session-secret"
    })];

    let first = sync_import_parts(
        &db,
        project_root,
        22,
        &cached_player,
        entries.clone(),
        details.clone(),
        detail_errors.clone(),
        1,
    )
    .unwrap();
    let second = sync_import_parts(
        &db,
        project_root,
        22,
        &cached_player,
        entries.clone(),
        details.clone(),
        detail_errors,
        1,
    )
    .unwrap();
    let conn = db.connect().unwrap();
    let matches_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM matches", [], |row| row.get(0))
        .unwrap();
    let details_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_details", [], |row| row.get(0))
        .unwrap();
    let player_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))
        .unwrap();
    let error_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sync_errors WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let errors_text: String = conn
        .query_row(
            "SELECT raw_json FROM sync_errors ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let entry_text: String = conn
        .query_row(
            "SELECT raw_entry_json FROM matches WHERE match_id = 'sync-match-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let detail_text: String = conn
        .query_row(
            "SELECT raw_detail_json FROM match_details WHERE match_id = 'sync-match-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let error_message: String = conn
        .query_row(
            "SELECT message FROM sync_errors ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(first.failed_details, 1);
    assert_eq!(second.failed_details, 1);
    assert_eq!(matches_count, 1);
    assert_eq!(details_count, 1);
    assert_eq!(player_rows, 3);
    assert_eq!(error_rows, 1);
    assert!(errors_text.contains("token-secret"));
    assert!(errors_text.contains("customer-secret"));
    assert!(errors_text.contains("session-secret"));
    assert!(entry_text.contains("entry-auth-secret"));
    assert!(detail_text.contains("detail-customer-secret"));
    assert!(error_message.contains("token-secret"));
    assert!(first.latest_errors.join(" ").contains("token-secret"));

    drop(conn);
    let recovered = sync_import_parts(
        &db,
        project_root,
        22,
        &cached_player,
        entries,
        details,
        vec![],
        1,
    )
    .unwrap();
    let conn = db.connect().unwrap();
    let remaining_errors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sync_errors WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(recovered.failed_details, 0);
    assert_eq!(remaining_errors, 0);
}

#[test]
fn interrupted_import_rolls_back_and_a_later_retry_remains_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let payload = season_payload();
    import_payload_value(&db, &payload, temp.path()).unwrap();
    let conn = db.connect().unwrap();
    let before_matches: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM matches WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let before_errors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sync_errors WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_stab005_import BEFORE UPDATE ON matches BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
    )
    .unwrap();
    drop(conn);

    assert!(import_payload_value(&db, &payload, temp.path()).is_err());
    let conn = db.connect().unwrap();
    let failed_matches: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM matches WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let failed_errors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sync_errors WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(failed_matches, before_matches);
    assert_eq!(failed_errors, before_errors);
    conn.execute("DROP TRIGGER fail_stab005_import", [])
        .unwrap();
    drop(conn);

    import_payload_value(&db, &payload, temp.path()).unwrap();
    let conn = db.connect().unwrap();
    let recovered_matches: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM matches WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let recovered_errors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sync_errors WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    import_payload_value(&db, &payload, temp.path()).unwrap();
    let retried_matches: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM matches WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let retried_errors: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sync_errors WHERE season = 22",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(recovered_matches, before_matches);
    assert_eq!(recovered_errors, before_errors);
    assert_eq!(retried_matches, recovered_matches);
    assert_eq!(retried_errors, recovered_errors);
}
