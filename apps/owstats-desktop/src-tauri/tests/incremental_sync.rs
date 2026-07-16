use owstats_desktop_lib::{
    import_incremental_payload_value_embedded, import_payload_value_embedded,
    load_season_sync_inventory, select_detail_entries, OwDatabase, SeasonSyncInventory,
};
use rusqlite::params;
use serde_json::json;
use std::collections::HashSet;

#[test]
fn inventory_is_scoped_to_the_requested_season() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
            begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
            mitigation, total_worth, duration_sec, raw_entry_json, updated_at
         ) VALUES (?1, ?2, 'preset', 'sport', 'normal', 'win', 'map', 'hero',
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, '{}', 'now')",
        params!["s23-complete", 23],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
         VALUES ('s23-complete', 'normal',
            '{\"match_id\":\"s23-complete\",\"match_kind\":\"normal\",\"payload\":{\"code\":0,\"data\":{}}}',
            'now')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
            begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
            mitigation, total_worth, duration_sec, raw_entry_json, updated_at
         ) VALUES (?1, ?2, 'preset', 'sport', 'normal', 'win', 'map', 'hero',
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, '{}', 'now')",
        params!["s22-complete", 22],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
         VALUES ('s22-complete', 'normal',
            '{\"match_id\":\"s22-complete\",\"match_kind\":\"normal\",\"payload\":{\"code\":0,\"data\":{}}}',
            'now')",
        [],
    )
    .unwrap();

    let inventory = load_season_sync_inventory(&db, 23).unwrap();

    assert_eq!(
        inventory.match_ids,
        ["s23-complete".to_string()].into_iter().collect()
    );
    assert_eq!(
        inventory.detail_ids,
        ["s23-complete".to_string()].into_iter().collect()
    );
}

#[test]
fn inventory_treats_only_valid_wrapped_details_as_complete() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for match_id in [
        "no-detail",
        "valid-normal",
        "valid-fight",
        "empty-object",
        "damaged-json",
        "failed-payload",
        "missing-kind",
        "unknown-kind",
        "mismatched-id",
    ] {
        conn.execute(
            "INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid,
                begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing,
                mitigation, total_worth, duration_sec, raw_entry_json, updated_at
             ) VALUES (?1, 23, 'preset', 'sport', 'normal', 'win', 'map', 'hero',
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, '{}', 'now')",
            params![match_id],
        )
        .unwrap();
    }
    for (match_id, match_kind, raw_detail_json) in [
        (
            "valid-normal",
            "normal",
            r#"{"match_id":"valid-normal","match_kind":"normal","payload":{"code":0,"data":{}}}"#,
        ),
        (
            "valid-fight",
            "fight",
            r#"{"match_id":"valid-fight","match_kind":"fight","payload":{"data":{}}}"#,
        ),
        ("empty-object", "normal", "{}"),
        ("damaged-json", "normal", "{not-json"),
        (
            "failed-payload",
            "normal",
            r#"{"match_id":"failed-payload","match_kind":"normal","payload":{"code":500,"data":{}}}"#,
        ),
        (
            "missing-kind",
            "normal",
            r#"{"match_id":"missing-kind","payload":{"code":0,"data":{}}}"#,
        ),
        (
            "unknown-kind",
            "custom",
            r#"{"match_id":"unknown-kind","match_kind":"custom","payload":{"code":0,"data":{}}}"#,
        ),
        (
            "mismatched-id",
            "normal",
            r#"{"match_id":"different-id","match_kind":"normal","payload":{"code":0,"data":{}}}"#,
        ),
    ] {
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
             VALUES (?1, ?2, ?3, 'now')",
            params![match_id, match_kind, raw_detail_json],
        )
        .unwrap();
    }

    let inventory = load_season_sync_inventory(&db, 23).unwrap();

    assert_eq!(
        inventory.match_ids,
        [
            "no-detail",
            "valid-normal",
            "valid-fight",
            "empty-object",
            "damaged-json",
            "failed-payload",
            "missing-kind",
            "unknown-kind",
            "mismatched-id",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    );
    assert_eq!(
        inventory.detail_ids,
        ["valid-normal", "valid-fight"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );
}

#[test]
fn detail_selector_returns_only_new_or_missing_details() {
    let mut inventory = SeasonSyncInventory::default();
    inventory
        .match_ids
        .extend(["complete".to_string(), "missing".to_string()]);
    inventory.detail_ids.insert("complete".to_string());
    let entries = vec![
        json!({"matchId":"complete","beginTs":3}),
        json!({"matchId":"missing","beginTs":2}),
        json!({"matchId":"new","beginTs":1}),
        json!({"matchId":"new","beginTs":0}),
        json!({"matchId":""}),
        json!({"beginTs":0}),
    ];

    let selection = select_detail_entries(&entries, &inventory);
    let ids = selection
        .entries
        .iter()
        .filter_map(|entry| entry["matchId"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["missing", "new"]);
    assert_eq!(selection.new_matches, 1);
    assert_eq!(selection.missing_details, 1);
}

fn entry(match_id: &str, game_mode: &str, begin_ts: i64) -> serde_json::Value {
    json!({
        "matchId": match_id,
        "gameMode": game_mode,
        "matchRet": 1,
        "beginTs": begin_ts
    })
}

fn normal_detail(match_id: &str, duration: i64, player_id: &str) -> serde_json::Value {
    json!({
        "match_id": match_id,
        "match_kind": "normal",
        "payload": {
            "code": 0,
            "data": {
                "gameTimeSec": duration,
                "totalCount": {
                    "teammateList": [{
                        "bnetId": player_id,
                        "name": format!("玩家-{player_id}"),
                        "heroGuid": "hero"
                    }],
                    "enemyList": []
                }
            }
        }
    })
}

fn fight_detail(match_id: &str, duration: i64, player_id: &str) -> serde_json::Value {
    json!({
        "match_id": match_id,
        "match_kind": "fight",
        "payload": {
            "code": 0,
            "data": {
                "gameTimeSec": duration,
                "totalCount": {
                    "teammateList": [{
                        "bnetId": player_id,
                        "name": format!("角斗玩家-{player_id}")
                    }],
                    "enemyList": []
                }
            }
        }
    })
}

fn payload(
    season: i64,
    entries: Vec<serde_json::Value>,
    details: Vec<serde_json::Value>,
    detail_errors: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "season": season,
        "matches": {
            "entries": entries,
            "details": details,
            "detail_errors": detail_errors
        }
    })
}

#[test]
fn incremental_import_preserves_unrefreshed_derivations_and_replaces_refreshed_index() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    import_payload_value_embedded(
        &db,
        &payload(
            23,
            vec![
                entry("first", "SportPreset", 1),
                entry("second", "SportPreset", 2),
            ],
            vec![
                normal_detail("first", 101, "first-old"),
                normal_detail("second", 202, "second-old"),
            ],
            vec![],
        ),
    )
    .unwrap();

    let summary = import_incremental_payload_value_embedded(
        &db,
        &payload(
            23,
            vec![
                entry("first", "QuickFight", 11),
                entry("second", "SportPreset", 22),
            ],
            vec![
                json!({
                    "match_id": "first",
                    "match_kind": "normal",
                    "payload": {}
                }),
                normal_detail("second", 303, "second-new"),
            ],
            vec![],
        ),
        ["first".to_string(), "second".to_string()]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let conn = db.connect().unwrap();
    let first: (String, String, i64, i64, String) = conn
        .query_row(
            "SELECT match_kind, bucket, duration_sec, begin_ts,
                    (SELECT group_concat(bnet_id, ',') FROM match_players WHERE match_id='first')
             FROM matches WHERE match_id='first'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    let second: (i64, String) = conn
        .query_row(
            "SELECT duration_sec,
                    (SELECT group_concat(bnet_id, ',') FROM match_players WHERE match_id='second')
             FROM matches WHERE match_id='second'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(
        first,
        (
            "normal".to_string(),
            "preset".to_string(),
            101,
            11,
            "first-old".to_string()
        )
    );
    assert_eq!(second, (303, "second-new".to_string()));
    assert_eq!(summary.details_seen, 1);
}

#[test]
fn incremental_import_reconciles_only_attempted_detail_errors() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    let initial_errors = ["unattempted", "recovered", "failed"]
        .into_iter()
        .map(|match_id| {
            json!({
                "match_id": match_id,
                "error_type": "detail_fetch",
                "message": format!("old-{match_id}")
            })
        })
        .collect();
    import_payload_value_embedded(
        &db,
        &payload(
            23,
            vec![
                entry("unattempted", "SportPreset", 1),
                entry("recovered", "SportPreset", 2),
                entry("failed", "SportPreset", 3),
            ],
            vec![],
            initial_errors,
        ),
    )
    .unwrap();

    import_incremental_payload_value_embedded(
        &db,
        &payload(
            23,
            vec![],
            vec![normal_detail("recovered", 10, "player")],
            vec![json!({
                "match_id": "failed",
                "error_type": "detail_fetch",
                "message": "new-failed"
            })],
        ),
        ["recovered".to_string(), "failed".to_string()]
            .into_iter()
            .collect(),
    )
    .unwrap();

    let conn = db.connect().unwrap();
    let rows = {
        let mut statement = conn
            .prepare("SELECT match_id, message FROM sync_errors WHERE season=23 ORDER BY match_id")
            .unwrap();
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(
        rows,
        vec![
            ("failed".to_string(), "new-failed".to_string()),
            ("unattempted".to_string(), "old-unattempted".to_string())
        ]
    );
}

#[test]
fn incremental_import_writes_season_totals_from_sqlite() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    let initial_entries = (0..10)
        .map(|index| entry(&format!("old-{index}"), "SportPreset", index))
        .collect::<Vec<_>>();
    let initial_details = (0..10)
        .map(|index| normal_detail(&format!("old-{index}"), 100 + index, "player"))
        .collect::<Vec<_>>();
    import_payload_value_embedded(&db, &payload(23, initial_entries, initial_details, vec![]))
        .unwrap();

    import_incremental_payload_value_embedded(
        &db,
        &payload(
            23,
            vec![entry("new", "SportPreset", 20)],
            vec![normal_detail("new", 120, "player")],
            vec![],
        ),
        HashSet::from(["new".to_string()]),
    )
    .unwrap();

    let conn = db.connect().unwrap();
    let totals: (i64, i64) = conn
        .query_row(
            "SELECT matches_count, details_count FROM seasons WHERE season=23",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(totals, (11, 11));
}

#[test]
fn incremental_import_rejects_malformed_detail_identity_and_kind() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    import_payload_value_embedded(
        &db,
        &payload(
            23,
            vec![
                entry("missing-kind", "SportPreset", 1),
                entry("empty-kind", "SportPreset", 2),
                entry("unknown-kind", "SportPreset", 3),
                entry("valid-normal", "SportPreset", 4),
                entry("valid-fight", "QuickFight", 5),
            ],
            vec![
                normal_detail("missing-kind", 10, "missing-kind-old"),
                normal_detail("empty-kind", 20, "empty-kind-old"),
                normal_detail("unknown-kind", 30, "unknown-kind-old"),
                normal_detail("valid-normal", 40, "normal-old"),
                fight_detail("valid-fight", 50, "fight-old"),
            ],
            vec![],
        ),
    )
    .unwrap();

    let summary = import_incremental_payload_value_embedded(
        &db,
        &payload(
            23,
            vec![],
            vec![
                json!({
                    "match_kind": "normal",
                    "payload": {"code": 0, "data": {}}
                }),
                json!({
                    "match_id": "   ",
                    "match_kind": "fight",
                    "payload": {"code": 0, "data": {}}
                }),
                json!({
                    "match_id": "missing-kind",
                    "payload": {"code": 0, "data": {}}
                }),
                json!({
                    "match_id": "empty-kind",
                    "match_kind": "   ",
                    "payload": {"code": 0, "data": {}}
                }),
                json!({
                    "match_id": "unknown-kind",
                    "match_kind": "custom",
                    "payload": {"code": 0, "data": {}}
                }),
                normal_detail("valid-normal", 60, "normal-new"),
                fight_detail("valid-fight", 70, "fight-new"),
            ],
            vec![],
        ),
        [
            "missing-kind",
            "empty-kind",
            "unknown-kind",
            "valid-normal",
            "valid-fight",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
    )
    .unwrap();

    let conn = db.connect().unwrap();
    let preserved_kinds = {
        let mut statement = conn
            .prepare(
                "SELECT match_id, match_kind
                 FROM match_details
                 WHERE match_id IN ('missing-kind', 'empty-kind', 'unknown-kind')
                 ORDER BY match_id",
            )
            .unwrap();
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    let player_ids = |match_id: &str| -> Vec<String> {
        let mut statement = conn
            .prepare("SELECT bnet_id FROM match_players WHERE match_id=? ORDER BY bnet_id")
            .unwrap();
        statement
            .query_map([match_id], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    let fight_players = {
        let mut statement = conn
            .prepare(
                "SELECT bnet_id FROM arena_match_players
                 WHERE match_id='valid-fight' ORDER BY bnet_id",
            )
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<String>>>()
            .unwrap()
    };

    assert_eq!(summary.details_seen, 2);
    assert_eq!(
        preserved_kinds,
        vec![
            ("empty-kind".to_string(), "normal".to_string()),
            ("missing-kind".to_string(), "normal".to_string()),
            ("unknown-kind".to_string(), "normal".to_string()),
        ]
    );
    assert_eq!(player_ids("missing-kind"), vec!["missing-kind-old"]);
    assert_eq!(player_ids("empty-kind"), vec!["empty-kind-old"]);
    assert_eq!(player_ids("unknown-kind"), vec!["unknown-kind-old"]);
    assert_eq!(player_ids("valid-normal"), vec!["normal-new"]);
    assert_eq!(fight_players, vec!["fight-new"]);
}
