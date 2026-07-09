use owstats_desktop_lib::{get_onboarding_status, OnboardingStatusRequest, OwDatabase};
use rusqlite::params;

fn status_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("owstats.db");
    let db = OwDatabase::new(&path);
    db.initialize().unwrap();
    std::mem::forget(temp);
    db
}

fn seed_match(
    db: &OwDatabase,
    season: i64,
    match_id: &str,
    bucket: &str,
    match_kind: &str,
    updated_at: &str,
    with_detail: bool,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'win', 'map-a', '地图A',
            'hero-a', '英雄A', ?6, 1, 0, 1, 1, 1, 1000, 500, 100, 0, 600, '{}', ?7)
        "#,
        params![
            match_id,
            season,
            bucket,
            if match_kind == "fight" { "SportFight" } else { "SportPreset" },
            match_kind,
            season * 1000,
            updated_at
        ],
    )
    .unwrap();
    if with_detail {
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, ?2, '{}', ?3)",
            params![match_id, match_kind, updated_at],
        )
        .unwrap();
    }
}

fn seed_error(db: &OwDatabase, season: i64, match_id: &str) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO sync_errors (season, match_id, error_type, message, raw_json, created_at)
        VALUES (?1, ?2, 'DetailFetchError', '详情获取失败', '{}', '2026-07-03T00:00:00Z')
        "#,
        params![season, match_id],
    )
    .unwrap();
}

#[test]
fn command_center_status_returns_fixed_empty_seasons() {
    let db = status_db();

    let status = get_onboarding_status(&db, OnboardingStatusRequest { season: None }).unwrap();

    let seasons = status
        .season_statuses
        .iter()
        .map(|item| item.season)
        .collect::<Vec<_>>();
    assert_eq!(seasons, vec![21, 22, 23]);
    assert!(status.season_statuses.iter().all(|item| {
        item.match_count == 0
            && item.detail_count == 0
            && item.normal_match_count == 0
            && item.arena_match_count == 0
            && item.failed_detail_count == 0
            && item.coverage_percent == 0
            && item.last_synced_or_imported_at.is_none()
    }));
}

#[test]
fn command_center_status_does_not_fabricate_missing_seasons() {
    let db = status_db();
    seed_match(
        &db,
        22,
        "s22-normal",
        "preset",
        "normal",
        "2026-07-03T01:00:00Z",
        true,
    );
    seed_match(
        &db,
        22,
        "s22-arena",
        "stadium",
        "fight",
        "2026-07-03T02:00:00Z",
        false,
    );
    seed_error(&db, 22, "s22-arena");

    let status = get_onboarding_status(&db, OnboardingStatusRequest { season: Some(22) }).unwrap();
    let s21 = status
        .season_statuses
        .iter()
        .find(|item| item.season == 21)
        .unwrap();
    let s22 = status
        .season_statuses
        .iter()
        .find(|item| item.season == 22)
        .unwrap();
    let s23 = status
        .season_statuses
        .iter()
        .find(|item| item.season == 23)
        .unwrap();

    assert_eq!(s21.match_count, 0);
    assert_eq!(s23.match_count, 0);
    assert_eq!(s22.match_count, 2);
    assert_eq!(s22.detail_count, 1);
    assert_eq!(s22.normal_match_count, 1);
    assert_eq!(s22.arena_match_count, 1);
    assert_eq!(s22.failed_detail_count, 1);
    assert_eq!(s22.coverage_percent, 50);
    assert_eq!(
        s22.last_synced_or_imported_at.as_deref(),
        Some("2026-07-03T02:00:00Z")
    );
}

#[test]
fn command_center_status_ignores_stale_detail_errors_when_details_exist() {
    let db = status_db();
    seed_match(
        &db,
        23,
        "s23-normal",
        "preset",
        "normal",
        "2026-07-09T01:00:00Z",
        true,
    );
    seed_match(
        &db,
        23,
        "s23-arena",
        "stadium",
        "fight",
        "2026-07-09T02:00:00Z",
        true,
    );
    seed_error(&db, 23, "s23-normal");

    let status = get_onboarding_status(&db, OnboardingStatusRequest { season: Some(23) }).unwrap();
    let s23 = status
        .season_statuses
        .iter()
        .find(|item| item.season == 23)
        .unwrap();

    assert_eq!(s23.match_count, 2);
    assert_eq!(s23.detail_count, 2);
    assert_eq!(s23.failed_detail_count, 0);
    assert_eq!(s23.coverage_percent, 100);
}

#[test]
fn command_center_status_keeps_multiple_seasons_isolated() {
    let db = status_db();
    seed_match(
        &db,
        21,
        "s21-normal",
        "preset",
        "normal",
        "2026-07-01T01:00:00Z",
        false,
    );
    seed_match(
        &db,
        22,
        "s22-normal",
        "preset",
        "normal",
        "2026-07-02T01:00:00Z",
        true,
    );
    seed_match(
        &db,
        22,
        "s22-arena",
        "stadium",
        "fight",
        "2026-07-02T02:00:00Z",
        true,
    );
    seed_match(
        &db,
        23,
        "s23-arena",
        "stadium",
        "fight",
        "2026-07-03T01:00:00Z",
        false,
    );
    seed_error(&db, 21, "s21-normal");
    seed_error(&db, 23, "s23-arena");

    let status = get_onboarding_status(&db, OnboardingStatusRequest { season: Some(22) }).unwrap();
    let by_season = |season| {
        status
            .season_statuses
            .iter()
            .find(|item| item.season == season)
            .unwrap()
    };

    assert_eq!(by_season(21).match_count, 1);
    assert_eq!(by_season(21).detail_count, 0);
    assert_eq!(by_season(21).normal_match_count, 1);
    assert_eq!(by_season(21).arena_match_count, 0);
    assert_eq!(by_season(21).failed_detail_count, 1);
    assert_eq!(by_season(22).match_count, 2);
    assert_eq!(by_season(22).detail_count, 2);
    assert_eq!(by_season(22).normal_match_count, 1);
    assert_eq!(by_season(22).arena_match_count, 1);
    assert_eq!(by_season(22).failed_detail_count, 0);
    assert_eq!(by_season(23).match_count, 1);
    assert_eq!(by_season(23).detail_count, 0);
    assert_eq!(by_season(23).normal_match_count, 0);
    assert_eq!(by_season(23).arena_match_count, 1);
    assert_eq!(by_season(23).failed_detail_count, 1);
}
