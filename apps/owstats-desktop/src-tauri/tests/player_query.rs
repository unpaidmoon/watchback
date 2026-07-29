use owstats_desktop_lib::player_query::{mapper, models::*};
use serde_json::json;

#[test]
fn mapper_preserves_order_deduplicates_and_uses_raw_count_for_end() {
    let mut items = Vec::new();
    for index in 0..12 {
        items.push(json!({"matchId":format!("m-{index}"),"matchRet":1,"teamScore":3,"opponentScore":2,"mapGuid":"map","mapName":"地图","heroGuid":"hero","heroName":"英雄","kill":2,"assist":3,"death":4,"beginTs":1767225600000i64}));
    }
    items[11]["matchId"] = "m-0".into();
    let (rows, raw) = mapper::matches(
        &json!({"code":0,"data":{"matchList":items}}),
        MatchKind::Normal,
    )
    .unwrap();
    assert_eq!(raw, 12);
    assert_eq!(rows.len(), 11);
    assert_eq!(rows[0].match_id, "m-0");
    assert_eq!(rows[0].map_name.as_deref(), Some("地图"));
    assert!(rows[0]
        .played_at
        .as_deref()
        .unwrap()
        .starts_with("2026-01-01"));
}

#[test]
fn arena_rows_drop_map_and_missing_fields_degrade_independently() {
    let value = json!({"code":0,"data":{"matchList":[{"matchId":"arena","matchRet":2,"teamScore":null,"opponentScore":null,"mapGuid":"must-drop","mapName":"must-drop","heroName":"英雄","assist":5,"beginTs":1767225600000i64}]}});
    let (rows, raw) = mapper::matches(&value, MatchKind::Arena).unwrap();
    assert_eq!(raw, 1);
    let row = &rows[0];
    assert_eq!(row.result, "loss");
    assert_eq!(row.score, "-");
    assert!(row.map_name.is_none());
    assert_eq!(row.kills, None);
    assert_eq!(row.assists, Some(5));
}

#[test]
fn real_negative_match_result_is_a_loss_and_legacy_two_remains_compatible() {
    for match_ret in [-1, 2] {
        let value =
            json!({"code":0,"data":[{"matchId":format!("loss-{match_ret}"),"matchRet":match_ret}]});
        let (rows, _) = mapper::matches(&value, MatchKind::Normal).unwrap();
        assert_eq!(rows[0].result, "loss");
    }
}

#[test]
fn detail_mapper_does_not_expose_raw_response_or_tokens() {
    let raw = json!({"code":0,"data":{"matchId":"detail","matchRet":1,"teamScore":3,"opponentScore":1,"mapName":"地图","gameTimeSec":120,"customerToken":"must-not-leak","teammateList":[{"playerName":"队友","heroGuid":"hero","kill":1,"assist":2,"death":3}],"enemyList":[],"heroList":[]}});
    let mapped = mapper::detail(raw, MatchKind::Normal).unwrap();
    let text = serde_json::to_string(&mapped).unwrap();
    assert_eq!(mapped.match_id, "detail");
    assert_eq!(mapped.result.as_deref(), Some("win"));
    assert!(mapped.raw_json.is_null());
    assert!(mapped.arena_detail.is_none());
    assert!(!text.contains("customerToken"));
    assert!(!text.contains("must-not-leak"));
}

#[test]
fn normal_detail_maps_perks_and_fractional_incoming_stats() {
    let raw = json!({"code":0,"data":{
        "matchId":"normal-fields",
        "matchRet":1,
        "teammateList":[{
            "name":"Player#1234",
            "damageTaken":5100.4,
            "healingTaken":1932.4,
            "perks":[{"guid":"perk-a","perkLevel":2}]
        }],
        "enemyList":[]
    }});
    let mapped = mapper::detail(raw, MatchKind::Normal).unwrap();
    assert_eq!(mapped.teammates[0].damage_taken, 5100);
    assert_eq!(mapped.teammates[0].healing_taken, 1932);
    assert_eq!(mapped.teammates[0].perks[0].guid, "perk-a");
    assert_eq!(mapped.teammates[0].perks[0].perk_level, 2);
}

#[test]
fn query_detail_assigns_party_groups_from_friend_links() {
    let raw = json!({"code":0,"data":{
        "matchId":"party-detail",
        "matchRet":1,
        "teammateList":[
            {"bnetId":101,"name":"Solo#1","friendBnetIds":[]},
            {"bnetId":201,"name":"PartyA#1","friendBnetIds":[202]},
            {"bnetId":301,"name":"Solo#2","friendBnetIds":[]},
            {"bnetId":202,"name":"PartyB#1","friendBnetIds":[]}
        ],
        "enemyList":[]
    }});
    let mapped = mapper::detail(raw, MatchKind::Normal).unwrap();
    assert_eq!(
        mapped
            .teammates
            .iter()
            .map(|player| (player.name.as_str(), player.party_group))
            .collect::<Vec<_>>(),
        vec![
            ("Solo#1", None),
            ("PartyA#1", Some(0)),
            ("PartyB#1", Some(0)),
            ("Solo#2", None),
        ]
    );
}

#[test]
fn profile_card_parses_fractional_hours_as_seconds() {
    let value = json!({"code":0,"data":{"gameTime":"1043.35"}});
    let card = mapper::card(&value, "Player#1234".into(), None).unwrap();
    assert_eq!(card.game_time_seconds, Some(3_756_060));
}

#[test]
fn detail_equipment_and_powers_are_reduced_to_stable_keys() {
    let raw = json!({"code":0,"data":{"matchId":"arena","totalCount":{"matchRet":1},"heroList":[{"heroGuid":"hero","equipment":[{"id":"eq-1","secret":"drop"}],"powers":[{"guid":"power-1","unknown":{"raw":true}}]}]}});
    let mapped = mapper::detail(raw, MatchKind::Arena).unwrap();
    let arena = mapped.arena_detail.as_ref().expect("arena detail contract");
    assert!(arena.summary.teammates.is_empty());
    assert!(arena.summary.enemies.is_empty());
    assert!(!serde_json::to_string(&mapped).unwrap().contains("secret"));
}

#[test]
fn arena_detail_uses_the_final_valid_round_for_summary_and_header() {
    let raw = json!({"code":0,"data":{
        "matchId":"arena-final-round",
        "matchRet":1,
        "nameMap":{"595945364":"Target#1234","545622836":"Enemy#5678"},
        "totalCount":{
            "matchRet":null,
            "teamScore":99,
            "opponentScore":88,
            "gameTimeSec":9999,
            "teammateList":[{"bnetId":"player#1","kill":99,"assist":88,"death":77}],
            "enemyList":[]
        },
        "roundCountList":[
            {
                "round":1,
                "teamScore":0,
                "opponentScore":1,
                "mapGuid":"first-map",
                "mapName":"First Map",
                "gameTimeSec":100,
                "teammateList":[{"bnetId":595945364,"kill":1,"assist":2,"death":3,"damageTaken":100.4,"healingTaken":50.4}],
                "enemyList":[]
            },
            {
                "round":2,
                "teamScore":3,
                "opponentScore":2,
                "mapGuid":"576460752303428278",
                "gameTimeSec":240,
                "teammateList":[{
                    "bnetId":595945364,
                    "heroGuid":"207165582859043119",
                    "kill":7,
                    "assist":8,
                    "death":2,
                    "damageTaken":12872.665,
                    "healingTaken":4114.6504,
                    "rankInfo":{"rankName":"Platinum","rankSubTier":5},
                    "modGuids":["920986123797269810"],
                    "traitGuids":["920986123797276688"],
                    "statMap":{"603482350067646495":7}
                }],
                "enemyList":[{"bnetId":545622836,"kill":4,"assist":5,"death":6}]
            },
            {
                "round":3,
                "matchRet":null,
                "mapGuid":"empty-map",
                "teammateList":[],
                "enemyList":[]
            }
        ]
    }});

    let mapped = mapper::detail(raw, MatchKind::Arena).unwrap();
    let summary = &mapped.arena_detail.as_ref().unwrap().summary;

    assert_eq!(mapped.result.as_deref(), Some("win"));
    assert_eq!(mapped.map_guid.as_deref(), Some("576460752303428278"));
    assert_eq!(mapped.map_name.as_deref(), Some("花冈"));
    assert_eq!(mapped.team_score, Some(3));
    assert_eq!(mapped.opponent_score, Some(2));
    assert_eq!(mapped.duration_sec, Some(240));
    assert_eq!(mapped.teammates[0].name, "Target#1234");
    assert_eq!(mapped.teammates[0].bnet_id, "595945364");
    assert_eq!(mapped.teammates[0].kills, 7);
    assert_eq!(mapped.teammates[0].damage_taken, 12873);
    assert_eq!(mapped.teammates[0].healing_taken, 4115);
    assert_eq!(
        mapped.teammates[0].rank_label.as_deref(),
        Some("Platinum 5")
    );
    assert_eq!(mapped.teammates[0].mod_guids, vec!["920986123797269810"]);
    assert_eq!(mapped.teammates[0].trait_guids, vec!["920986123797276688"]);
    assert_eq!(mapped.teammates[0].hero_stats.len(), 1);
    assert_eq!(summary.map_guid.as_deref(), Some("576460752303428278"));
    assert_eq!(summary.map_name.as_deref(), Some("花冈"));
    assert_eq!(summary.team_score, Some(3));
    assert_eq!(summary.teammates[0].kills, 7);
    assert_eq!(summary.teammates[0].damage_taken, 12973);
    assert_eq!(summary.teammates[0].healing_taken, 4165);
    let rounds = &mapped.arena_detail.as_ref().unwrap().rounds;
    assert_eq!(rounds[0].result.as_deref(), Some("loss"));
    assert_eq!(rounds[1].result.as_deref(), Some("win"));
    assert_eq!(
        mapped.arena_detail.as_ref().unwrap().rounds.len(),
        3,
        "the detail still exposes every round while the summary uses the final valid one"
    );
}

#[test]
fn serialized_query_details_match_the_shared_frontend_contract() {
    let normal = mapper::detail(
        serde_json::from_str(include_str!("fixtures/player_query/normal_detail.json")).unwrap(),
        MatchKind::Normal,
    )
    .unwrap();
    let arena = mapper::detail(
        serde_json::from_str(include_str!("fixtures/player_query/arena_detail.json")).unwrap(),
        MatchKind::Arena,
    )
    .unwrap();
    for serialized in [
        serde_json::to_value(normal).unwrap(),
        serde_json::to_value(arena.clone()).unwrap(),
    ] {
        for key in [
            "matchId",
            "gameMode",
            "modeLabel",
            "result",
            "resultLabel",
            "mapGuid",
            "mapName",
            "heroGuid",
            "heroName",
            "beginTs",
            "beginLabel",
            "teamScore",
            "opponentScore",
            "durationSec",
            "teammates",
            "enemies",
            "arenaDetail",
            "evaluation",
            "teamAwards",
            "sameRoleComparison",
            "favorite",
            "aiRoast",
        ] {
            assert!(
                serialized.get(key).is_some(),
                "query detail misses frontend key {key}"
            );
        }
        assert!(serialized["teammates"].is_array());
        assert!(serialized["enemies"].is_array());
        assert!(serialized["teamAwards"].is_array());
        assert!(
            serialized.get("rawJson").is_none(),
            "query details must not expose the upstream payload to the WebView"
        );
    }
    let arena = serde_json::to_value(arena).unwrap();
    assert_eq!(arena["arenaDetail"]["summary"]["key"], "summary");
    assert_eq!(arena["arenaDetail"]["rounds"].as_array().unwrap().len(), 3);
    for slice in std::iter::once(&arena["arenaDetail"]["summary"])
        .chain(arena["arenaDetail"]["rounds"].as_array().unwrap().iter())
    {
        assert!(slice["teammates"].is_array());
        assert!(slice["enemies"].is_array());
    }
}

#[test]
fn all_supported_seasons_and_modes_have_exact_routes() {
    assert_eq!(Season::S23.upstream(), None);
    assert_eq!(Season::S15.upstream(), Some(15));
    let modes = [
        QueryMode::LeisurePreset,
        QueryMode::Leisure6v6,
        QueryMode::SportPreset,
        QueryMode::SportOpen,
        QueryMode::SportFight,
        QueryMode::LeisureFight,
    ];
    assert_eq!(
        modes.map(QueryMode::upstream),
        [
            "LeisurePreset",
            "Leisure6v6",
            "SportPreset",
            "SportOpen",
            "SportFight",
            "LeisureFight"
        ]
    );
    assert_eq!(
        modes.map(QueryMode::kind),
        [
            MatchKind::Normal,
            MatchKind::Normal,
            MatchKind::Normal,
            MatchKind::Normal,
            MatchKind::Arena,
            MatchKind::Arena
        ]
    );
}

#[test]
fn zero_duration_heroes_are_not_exposed() {
    let value = json!({"code":0,"data":{"heroList":[{"heroGuid":"zero","heroName":"零","gameTimeSec":0},{"heroGuid":"used","heroName":"常用","gameTimeSec":60,"wins":1,"games":2,"winRate":50}]}});
    let rows = mapper::heroes(&value).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].hero_guid, "used");
}

#[test]
fn real_count_shape_maps_competitive_ranks_and_mode_heroes() {
    let value = json!({"code":0,"data":{
        "guideCountData":[{"roleType":"dps","matchSum":27,"winRate":"59.26","kda":"3.11","lastRankInfo":{"rankName":"Gold","rankSubTier":4}}],
        "presetsHeroUseSummaryList":[{"heroGuid":"zero","gameTime":0},{"heroGuid":"used","gameTime":60,"winSum":1,"matchSum":2,"winRate":"50.00"}]
    }});
    let ranks = mapper::ranks(&value).unwrap();
    assert_eq!(ranks.len(), 1);
    assert_eq!(ranks[0].role, "dps");
    assert_eq!(ranks[0].rank.as_deref(), Some("Gold"));
    assert_eq!(ranks[0].division, Some(4));
    assert_eq!(ranks[0].games, Some(27));
    assert_eq!(ranks[0].win_rate, Some(0.5926));
    assert_eq!(ranks[0].kda, Some(3.11));

    let heroes = mapper::heroes_for_mode(&value, QueryMode::LeisurePreset).unwrap();
    assert_eq!(heroes.len(), 1);
    assert_eq!(heroes[0].hero_guid, "used");
    assert_eq!(heroes[0].game_time_seconds, 60);
    assert_eq!(heroes[0].wins, Some(1));
    assert_eq!(heroes[0].games, Some(2));
    assert_eq!(heroes[0].win_rate, Some(0.5));
}

#[test]
fn normal_grouped_list_preserves_all_rows_without_exact_mode_filter() {
    let value = json!({"code":0,"data":[
        {"matchId":"wanted","gameMode":"LeisurePreset","matchRet":1,"mapGuid":"map-a","heroGuid":"hero-a","kill":2,"assist":3,"death":4,"beginTs":1767225600000i64},
        {"matchId":"other","gameMode":"Leisure6v6","matchRet":2}
    ]});
    let (rows, raw) = mapper::matches(&value, MatchKind::Normal).unwrap();
    assert_eq!(raw, 2);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].match_id, "wanted");
    assert_eq!(rows[0].hero_key.as_deref(), Some("hero-a"));
    assert_eq!(rows[0].map_key.as_deref(), Some("map-a"));
}

#[test]
fn query_modes_separate_exact_values_from_grouped_upstream_requests() {
    assert_eq!(QueryMode::LeisurePreset.list_upstream(), "leisure");
    assert_eq!(QueryMode::Leisure6v6.list_upstream(), "leisure");
    assert_eq!(QueryMode::SportPreset.list_upstream(), "sport");
    assert_eq!(QueryMode::SportOpen.list_upstream(), "sport");
    assert_eq!(QueryMode::SportFight.list_upstream(), "SportFight");
    assert_eq!(QueryMode::LeisureFight.list_upstream(), "LeisureFight");
    assert_eq!(QueryMode::LeisurePreset.list_scope_key(), "normal:leisure");
    assert_eq!(QueryMode::Leisure6v6.list_scope_key(), "normal:leisure");
    assert_eq!(QueryMode::SportPreset.list_scope_key(), "normal:sport");
    assert_eq!(QueryMode::SportOpen.list_scope_key(), "normal:sport");
    assert_eq!(QueryMode::SportFight.list_scope_key(), "arena:SportFight");
    assert_eq!(
        QueryMode::LeisureFight.list_scope_key(),
        "arena:LeisureFight"
    );
}

#[test]
fn count_requests_remain_exact_for_preset_open_and_stadium() {
    assert_eq!(QueryMode::SportPreset.count_upstream(), "sport");
    assert_eq!(QueryMode::SportOpen.count_upstream(), "sport");
    assert_eq!(QueryMode::SportFight.count_upstream(), "fight");
}

#[test]
fn sport_count_splits_three_role_queue_rows_from_single_open_row() {
    let value = json!({"code":0,"data":{"guideCountData":[
        {"roleType":"tank","matchSum":10,"winRate":"50.00","lastRankInfo":{"rankName":"Gold","rankSubTier":2}},
        {"roleType":"dps","matchSum":11,"winRate":"54.55","lastRankInfo":{"rankName":"Platinum","rankSubTier":4}},
        {"roleType":"healer","matchSum":12,"winRate":"58.33","lastRankInfo":{"rankName":"Diamond","rankSubTier":1}},
        {"roleType":"open","matchSum":7,"winRate":"42.86","lastRankInfo":{"rankName":"Gold","rankSubTier":5}}
    ]}});
    assert_eq!(mapper::competitive_ranks(&value).unwrap().len(), 3);
    let open = mapper::open_competitive_rank(&value).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].role, "open");
}

#[test]
fn fight_count_uses_authoritative_role_type_count_data() {
    let value = json!({"code":0,"data":{"roleTypeCountData":[
        {"roleType":"tank","matchSum":20,"wins":11,"winRate":"55.00","kda":"3.25","lastRankInfo":{"rankName":"Master","rankSubTier":2}},
        {"roleType":"dps","matchSum":9,"winRate":"44.44","kda":"2.10","lastRankInfo":{"rankName":"Diamond","rankSubTier":4}},
        {"roleType":"healer","matchSum":12,"winRate":"58.33","kda":"4.00","lastRankInfo":{"rankName":"Platinum","rankSubTier":1}}
    ],"heroUseSummaryList":[]}});
    let stadium = mapper::stadium_rank(&value).unwrap();
    assert_eq!(stadium.len(), 3);
    let tank = stadium.iter().find(|row| row.role == "tank").unwrap();
    assert_eq!(tank.rank.as_deref(), Some("Master"));
    assert_eq!(tank.division, Some(2));
    assert_eq!(tank.games, Some(20));
    assert_eq!(tank.wins, Some(11));
    assert_eq!(tank.win_rate, Some(0.55));
    assert_eq!(tank.kda, Some(3.25));
}

#[test]
fn fight_hero_endpoints_use_their_real_independent_shapes() {
    let sport = json!({"code":0,"data":{"roleTypeCountData":[],"heroUseSummaryList":[
        {"heroGuid":"sport-hero","gameTime":120,"winSum":3,"matchSum":5,"winRate":"60"}
    ]}});
    let leisure = json!({"code":0,"data":{"roleDataList":[],"heroDataList":[
        {"heroGuid":"leisure-hero","gameTime":90,"winSum":2,"matchSum":4,"winRate":"50"}
    ]}});
    let sport_rows = mapper::heroes_for_mode(&sport, QueryMode::SportFight).unwrap();
    let leisure_rows = mapper::heroes_for_mode(&leisure, QueryMode::LeisureFight).unwrap();
    assert_eq!(sport_rows[0].hero_guid, "sport-hero");
    assert_eq!(sport_rows[0].game_time_seconds, 120);
    assert_eq!(leisure_rows[0].hero_guid, "leisure-hero");
    assert_eq!(leisure_rows[0].win_rate, Some(0.5));
}
