use chrono::{Local, NaiveDateTime, TimeZone};
use owstats_desktop_lib::{list_matches, search_match_players, MatchFilters, OwDatabase};
use rusqlite::params;

#[test]
fn match_filters_include_start_and_end_dates() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for (match_id, begin_ts) in [
        ("match-0619", ts("2027-09-19 12:00:00")),
        ("match-0620-morning", ts("2027-09-20 08:00:00")),
        ("match-0621-night", ts("2027-09-21 23:59:59")),
        ("match-0622", ts("2027-09-22 00:00:00")),
    ] {
        conn.execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
                hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
                deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
            )
            VALUES (?1, 22, 'preset', 'SportPreset', 'normal', 'win', 'map-a', 'Map A',
                'hero-a', 'Hero A', ?2, 1, 0, 1, 0, 1, 100, 10, 0, 0, 600, '{}',
                '2026-06-27T00:00:00Z')
            "#,
            params![match_id, begin_ts],
        )
        .unwrap();
    }

    let rows = list_matches(
        &db,
        MatchFilters {
            date_from: Some("2027-09-20".to_string()),
            date_to: Some("2027-09-21".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let ids = rows.into_iter().map(|row| row.match_id).collect::<Vec<_>>();

    assert_eq!(ids, vec!["match-0621-night", "match-0620-morning"]);
}

#[test]
fn match_filters_include_exact_player_name_from_all_match_players() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    insert_match(&conn, "match-older", ts("2027-09-20 08:00:00"));
    insert_match(&conn, "match-newer", ts("2027-09-21 08:00:00"));
    insert_match(&conn, "match-similar-id", ts("2027-09-22 08:00:00"));
    insert_player(
        &conn,
        "match-older",
        "teammate",
        "bnet-a",
        "是糖三分毒#5811",
    );
    insert_player(&conn, "match-newer", "enemy", "bnet-b", "是糖三分毒#5811");
    insert_player(
        &conn,
        "match-similar-id",
        "teammate",
        "bnet-c",
        "是糖三分毒",
    );

    let rows = list_matches(
        &db,
        MatchFilters {
            player_name: Some("是糖三分毒#5811".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let ids = rows.into_iter().map(|row| row.match_id).collect::<Vec<_>>();

    assert_eq!(ids, vec!["match-newer", "match-older"]);
}

#[test]
fn player_name_search_finds_candidates_by_partial_display_name() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    insert_match(&conn, "match-a", ts("2027-09-20 08:00:00"));
    insert_match(&conn, "match-b", ts("2027-09-21 08:00:00"));
    insert_match(&conn, "match-c", ts("2027-09-22 08:00:00"));
    insert_player(&conn, "match-a", "teammate", "bnet-a", "是糖三分毒#5811");
    insert_player(&conn, "match-b", "enemy", "bnet-b", "是糖三分毒#5811");
    insert_player(&conn, "match-c", "enemy", "bnet-c", "糖分不够#1234");

    let candidates = search_match_players(&db, "糖三分").unwrap();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].player_name, "是糖三分毒#5811");
    assert_eq!(candidates[0].matches, 2);
    assert_eq!(
        candidates[0].latest_begin_ts,
        Some(ts("2027-09-21 08:00:00"))
    );
}

fn ts(value: &str) -> i64 {
    let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap();
    Local
        .from_local_datetime(&naive)
        .single()
        .unwrap()
        .timestamp()
}

fn insert_match(conn: &rusqlite::Connection, match_id: &str, begin_ts: i64) {
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'preset', 'SportPreset', 'normal', 'win', 'map-a', 'Map A',
            'hero-a', 'Hero A', ?2, 1, 0, 1, 0, 1, 100, 10, 0, 0, 600, '{}',
            '2026-06-27T00:00:00Z')
        "#,
        params![match_id, begin_ts],
    )
    .unwrap();
}

fn insert_player(
    conn: &rusqlite::Connection,
    match_id: &str,
    side: &str,
    bnet_id: &str,
    player_name: &str,
) {
    conn.execute(
        r#"
        INSERT INTO match_players (
            match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
            raw_json, updated_at
        )
        VALUES (?1, 22, ?2, ?3, ?4, 'hero-a', 'Hero A', '{}', '2026-06-27T00:00:00Z')
        "#,
        params![match_id, side, bnet_id, player_name],
    )
    .unwrap();
}
