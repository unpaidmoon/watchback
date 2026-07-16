use owstats_desktop_lib::{
    load_season_sync_inventory, select_detail_entries, OwDatabase, SeasonSyncInventory,
};
use rusqlite::params;
use serde_json::json;

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
         VALUES ('s23-complete', 'normal', '{}', 'now')",
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
         VALUES ('s22-complete', 'normal', '{}', 'now')",
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
