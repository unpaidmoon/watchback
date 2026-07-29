use anyhow::Result;
use owstats_desktop_lib::{list_matches_page, MatchFilters, OwDatabase};
use rusqlite::params;

#[test]
fn match_pages_are_stable_across_equal_timestamps() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("matches.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    for (match_id, begin_ts) in [("m1", 20), ("m2", 20), ("m3", 30)] {
        conn.execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result,
                map_guid, hero_guid, begin_ts, team_score, opponent_score,
                kills, assists, deaths, damage, healing, mitigation,
                total_worth, duration_sec, raw_entry_json, updated_at
            ) VALUES (?1, 23, 'preset', 'SportPreset', 'normal', 'win',
                      'map', 'hero', ?2, 1, 0, 1, 2, 3, 4, 5, 6,
                      0, 600, '{}', '2026-07-30T00:00:00Z')
            "#,
            params![match_id, begin_ts],
        )?;
    }
    drop(conn);

    let first = list_matches_page(&db, MatchFilters::default(), None, 2)?;
    assert_eq!(
        first.matches.iter().map(|item| item.match_id.as_str()).collect::<Vec<_>>(),
        ["m3", "m1"]
    );
    assert!(first.next_cursor.is_some());
    assert_eq!(first.total, Some(3));

    let second = list_matches_page(&db, MatchFilters::default(), first.next_cursor, 2)?;
    assert_eq!(
        second.matches.iter().map(|item| item.match_id.as_str()).collect::<Vec<_>>(),
        ["m2"]
    );
    assert!(second.next_cursor.is_none());
    assert_eq!(second.total, None);
    Ok(())
}
