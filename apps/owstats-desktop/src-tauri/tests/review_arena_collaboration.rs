use owstats_desktop_lib::{get_personal_season_review, OwDatabase, PersonalReviewRequest};
use rusqlite::params;

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
