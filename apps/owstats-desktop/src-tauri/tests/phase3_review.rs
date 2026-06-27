use owstats_desktop_lib::{
    get_onboarding_status, get_personal_season_review, OnboardingStatusRequest, OwDatabase,
    PersonalReviewRequest,
};

fn seed_match(
    db: &OwDatabase,
    id: &str,
    result: &str,
    map: &str,
    hero: &str,
    begin_ts: i64,
    kills: i64,
    deaths: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
    duration_sec: i64,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'preset', 'SportPreset', 'normal', ?2, ?3, ?3, ?4, ?4, ?5,
            CASE WHEN ?2 = 'win' THEN 2 ELSE 1 END,
            CASE WHEN ?2 = 'win' THEN 1 ELSE 2 END,
            ?6, 3, ?7, ?8, ?9, ?10, 0, ?11, '{}', '2026-06-27T00:00:00Z')
        "#,
        rusqlite::params![
            id,
            result,
            map,
            hero,
            begin_ts,
            kills,
            deaths,
            damage,
            healing,
            mitigation,
            duration_sec
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'normal', '{}', '2026-06-27T00:00:00Z')",
        [id],
    )
    .unwrap();
}

#[test]
fn personal_review_exposes_baseline_exclusions_without_hiding_matches() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();

    seed_match(
        &db, "normal-1", "win", "Ilios", "Ana", 100, 12, 4, 10_000, 8_000, 600, 620,
    );
    seed_match(
        &db, "normal-2", "loss", "Ilios", "Ana", 200, 11, 5, 9_000, 7_000, 500, 640,
    );
    seed_match(
        &db, "normal-3", "win", "Dorado", "Tracer", 300, 18, 3, 13_000, 500, 300, 700,
    );
    seed_match(
        &db, "short", "loss", "Dorado", "Tracer", 400, 2, 1, 900, 100, 20, 20,
    );
    seed_match(
        &db, "missing", "loss", "Circuit", "Cassidy", 500, 0, 0, 0, 0, 0, 650,
    );

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    assert_eq!(review.summary.total_matches, 5);
    assert_eq!(review.summary.baseline_matches, 3);
    assert_eq!(review.baseline.excluded_total, 2);
    assert!(review
        .baseline
        .exclusion_reasons
        .iter()
        .any(|item| item.reason == "short_match" && item.count == 1));
    assert!(review
        .baseline
        .exclusion_reasons
        .iter()
        .any(|item| item.reason == "missing_key_stats" && item.count == 1));
    assert_eq!(review.baseline.short_match_threshold_sec, 60);
    assert!(review
        .top_conclusions
        .iter()
        .all(|item| item.evidence.kind != "none"));
    assert!((5..=8).contains(&review.top_conclusions.len()));
}

#[test]
fn personal_review_keeps_loss_streak_out_of_top_summary_and_uses_positive_bests() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();

    seed_match(
        &db, "loss-1", "loss", "Ilios", "Ana", 100, 20, 5, 16_000, 8_000, 700, 620,
    );
    seed_match(
        &db, "loss-2", "loss", "Ilios", "Ana", 200, 10, 7, 9_000, 7_000, 500, 640,
    );
    seed_match(
        &db, "loss-3", "loss", "Ilios", "Ana", 300, 8, 8, 8_000, 6_000, 400, 660,
    );
    seed_match(
        &db, "win-1", "win", "Dorado", "Tracer", 400, 22, 2, 15_000, 300, 100, 700,
    );
    seed_match(
        &db, "win-2", "win", "Dorado", "Tracer", 500, 18, 3, 13_000, 500, 100, 680,
    );
    seed_match(
        &db, "win-3", "win", "Circuit", "Cassidy", 600, 12, 4, 10_000, 400, 12_000, 690,
    );

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    assert_eq!(review.streaks.longest_loss_streak, 3);
    assert!(review
        .top_conclusions
        .iter()
        .all(|item| item.kind != "loss_streak"));
    assert!(review
        .personal_bests
        .iter()
        .any(|item| item.metric == "damage" && item.match_id == "loss-1" && item.result == "loss"));
    assert!(review
        .personal_bests
        .iter()
        .all(|item| item.metric != "mitigation" && item.metric != "healing_taken"));
    assert!(review.hero_pool.label.contains("活跃") || review.hero_pool.label.contains("适应"));
}

#[test]
fn onboarding_status_allows_existing_readable_local_data() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();
    seed_match(
        &db, "normal-1", "win", "Ilios", "Ana", 100, 12, 4, 10_000, 8_000, 600, 620,
    );

    let status = get_onboarding_status(&db, OnboardingStatusRequest { season: Some(22) }).unwrap();

    assert!(status.can_enter_review);
    assert_eq!(status.selected_season, Some(22));
    assert_eq!(status.local_data.preset_matches, 1);
    assert!(status.local_data.data_source.contains("本地"));
    assert!(!status.credential_status.exposes_secret);
}
