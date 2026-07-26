use serde_json::Value;
use std::collections::BTreeSet;

const SEASONS: [i64; 5] = [15, 20, 21, 22, 23];
const MODES: [&str; 6] = [
    "LeisurePreset",
    "Leisure6v6",
    "SportPreset",
    "SportOpen",
    "SportFight",
    "LeisureFight",
];

fn fixture(source: &str) -> Value {
    serde_json::from_str(source).expect("player-query fixture must be valid JSON")
}

fn samples(value: &Value) -> &[Value] {
    value["samples"]
        .as_array()
        .expect("matrix fixture must contain samples")
}

fn sample_modes(value: &Value) -> BTreeSet<&str> {
    samples(value)
        .iter()
        .map(|sample| sample["mode"].as_str().expect("sample must name its mode"))
        .collect()
}

#[test]
fn search_and_card_fixtures_define_identity_and_all_supported_seasons() {
    let search = fixture(include_str!(
        "fixtures/player_query/search_bnet_account.json"
    ));
    assert_eq!(search["fixtureKind"], "synthetic-contract");
    assert_eq!(search["code"], 0);
    assert!(search["data"]["battleTag"].as_str().unwrap().contains('#'));
    assert!(search["data"]["accountKey"].as_str().is_some());

    let cards = fixture(include_str!("fixtures/player_query/card_seasons.json"));
    let seasons: Vec<_> = samples(&cards)
        .iter()
        .map(|sample| sample["season"].as_i64().unwrap())
        .collect();
    assert_eq!(seasons, SEASONS);

    for sample in samples(&cards) {
        let data = &sample["response"]["data"];
        assert!(data["icon"].as_str().is_some());
        assert!(data["gameTime"].is_number() || data["gameTime"].as_str().is_some());
        // Upstream level is the endorsement level shown as “赞赏等级”, not account level.
        assert!(data["level"].as_u64().is_some());
        // The text title remains optional. The query UI intentionally ignores titleIcon.
        assert!(data["title"].as_str().is_some());
    }
}

#[test]
fn count_info_matrix_covers_six_exact_modes_and_rank_and_hero_fields() {
    let counts = fixture(include_str!("fixtures/player_query/count_info_modes.json"));
    assert_eq!(sample_modes(&counts), MODES.into_iter().collect());

    for sample in samples(&counts) {
        let data = &sample["response"]["data"];
        let ranks = data["roleRanks"].as_array().unwrap();
        let heroes = data["heroList"].as_array().unwrap();
        for rank in ranks {
            for field in ["role", "rank", "division", "wins", "games", "winRate"] {
                assert!(!rank[field].is_null(), "rank field {field} is contractual");
            }
        }
        for hero in heroes {
            for field in [
                "heroGuid",
                "heroName",
                "gameTimeSec",
                "wins",
                "games",
                "winRate",
            ] {
                assert!(!hero[field].is_null(), "hero field {field} is contractual");
            }
        }
    }
}

#[test]
fn match_page_matrix_covers_first_and_terminal_pages_for_every_mode() {
    let pages = fixture(include_str!("fixtures/player_query/match_pages_modes.json"));
    assert_eq!(pages["pageSize"], 12);
    assert_eq!(sample_modes(&pages), MODES.into_iter().collect());

    for sample in samples(&pages) {
        let first = sample["firstPage"]["data"]["matchList"].as_array().unwrap();
        let last = sample["lastPage"]["data"]["matchList"].as_array().unwrap();
        assert!(!first.is_empty());
        assert!(first.len() <= 12);
        assert!(last.len() < 12, "raw short page establishes endReached");

        for row in first {
            for field in ["matchId", "matchRet", "heroGuid", "heroName", "beginTs"] {
                assert!(!row[field].is_null(), "list field {field} is contractual");
            }
            for field in ["kill", "assist", "death"] {
                assert!(row.get(field).is_some(), "K/A/D presence is contractual");
            }
        }

        let is_arena = sample["kind"] == "arena";
        for row in first {
            if is_arena {
                assert!(row.get("mapGuid").is_none());
                assert!(row.get("mapName").is_none());
            } else {
                assert!(row.get("mapGuid").is_some());
                assert!(row.get("mapName").is_some());
            }
        }
    }
}

#[test]
fn normal_and_arena_details_lock_distinct_shapes() {
    let normal = fixture(include_str!("fixtures/player_query/normal_detail.json"));
    let normal_data = &normal["data"];
    for field in [
        "matchId",
        "matchRet",
        "gameTimeSec",
        "mapGuid",
        "mapName",
        "teammateList",
        "enemyList",
        "heroList",
    ] {
        assert!(!normal_data[field].is_null(), "normal detail needs {field}");
    }

    let arena = fixture(include_str!("fixtures/player_query/arena_detail.json"));
    let arena_data = &arena["data"];
    assert!(arena_data["totalCount"].is_object());
    let rounds = arena_data["roundCountList"].as_array().unwrap();
    assert!(rounds.len() > 1);
    assert!(rounds.iter().all(|round| round["mapGuid"].is_string()));
    assert!(rounds.iter().all(|round| round["gameTimeSec"].is_u64()));
    let hero = &arena_data["heroList"][0];
    assert!(hero["equipment"].is_array());
    assert!(hero["powers"].is_array());
}

#[test]
fn committed_fixtures_are_synthetic_and_contain_no_sensitive_fields() {
    let fixtures = [
        include_str!("fixtures/player_query/search_bnet_account.json"),
        include_str!("fixtures/player_query/card_seasons.json"),
        include_str!("fixtures/player_query/count_info_modes.json"),
        include_str!("fixtures/player_query/match_pages_modes.json"),
        include_str!("fixtures/player_query/normal_detail.json"),
        include_str!("fixtures/player_query/arena_detail.json"),
    ];
    let forbidden_keys = [
        "authorization",
        "cookie",
        "customertoken",
        "bigdatatoken",
        "traceid",
        "roleid",
    ];

    fn scan(value: &Value, forbidden_keys: &[&str]) {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    let normalized = key.to_ascii_lowercase().replace(['_', '-'], "");
                    assert!(
                        !forbidden_keys.contains(&normalized.as_str()),
                        "fixture contains forbidden field: {key}"
                    );
                    scan(child, forbidden_keys);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| scan(item, forbidden_keys)),
            Value::String(text) => {
                assert!(!text.starts_with("Bearer "));
                assert!(!text.contains("ContractPlayer#1001") || text == "ContractPlayer#1001");
            }
            _ => {}
        }
    }

    for source in fixtures {
        let value = fixture(source);
        assert_eq!(value["fixtureKind"], "synthetic-contract");
        scan(&value, &forbidden_keys);
    }
}
