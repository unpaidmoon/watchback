use owstats_desktop_lib::{
    get_personal_season_review, get_season_analysis, list_matches, ranking, MatchFilters,
    OwDatabase, PersonalReviewRequest,
};
use rusqlite::params;

fn test_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.keep().join("owstats.db");
    let db = OwDatabase::new(path);
    db.initialize().unwrap();
    db
}

#[derive(Clone, Copy)]
struct MatchSeed<'a> {
    season: i64,
    id: &'a str,
    result: &'a str,
    map_guid: &'a str,
    map_name: &'a str,
    hero_guid: &'a str,
    hero_name: &'a str,
    begin_ts: i64,
    grade: &'a str,
    friend_bnet: &'a str,
    friend_name: &'a str,
}

fn seed_match(db: &OwDatabase, seed: MatchSeed<'_>) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, ?2, 'preset', 'SportPreset', 'normal', ?3, ?4, ?5, ?6, ?7, ?8,
            CASE WHEN ?3 = 'win' THEN 2 ELSE 1 END,
            CASE WHEN ?3 = 'win' THEN 1 ELSE 2 END,
            20, 5, 4, 12000, 1500, 400, 0, 900, '{}', '2026-07-09T00:00:00Z')
        "#,
        params![
            seed.id,
            seed.season,
            seed.result,
            seed.map_guid,
            seed.map_name,
            seed.hero_guid,
            seed.hero_name,
            seed.begin_ts,
        ],
    )
    .unwrap();
    conn.execute(
        r#"
        INSERT INTO match_evaluations (
            match_id, mode, rule_version, grade, primary_tag, evidence_tags_json,
            reasons_json, confidence, computed_at
        )
        VALUES (?1, 'SportPreset', 1, ?2, '测试标签', '[]', '[]', 1.0, '2026-07-09T00:00:00Z')
        "#,
        params![seed.id, seed.grade],
    )
    .unwrap();
    seed_player(db, seed, "self", "当前玩家", 1, 0);
    seed_player(db, seed, seed.friend_bnet, seed.friend_name, 0, 1);
}

fn seed_player(
    db: &OwDatabase,
    seed: MatchSeed<'_>,
    bnet_id: &str,
    player_name: &str,
    is_current_player: i64,
    is_friend: i64,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO match_players (
            match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
            is_current_player, is_friend, kill, assist, death, hero_damage, cure,
            resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
            role_type, raw_json, updated_at
        )
        VALUES (?1, ?2, 'teammate', ?3, ?4, ?5, ?6, ?7, ?8,
            20, 5, 4, 12000, 1500, 400, 1000, 2000, 1, 90, 'damage', '{}',
            '2026-07-09T00:00:00Z')
        "#,
        params![
            seed.id,
            seed.season,
            bnet_id,
            player_name,
            seed.hero_guid,
            seed.hero_name,
            is_current_player,
            is_friend,
        ],
    )
    .unwrap();
}

fn seed_two_seasons(db: &OwDatabase) {
    for index in 0..5 {
        seed_match(
            db,
            MatchSeed {
                season: 21,
                id: Box::leak(format!("s21-match-{index}").into_boxed_str()),
                result: "win",
                map_guid: "map-ilios",
                map_name: "Ilios",
                hero_guid: "hero-ana",
                hero_name: "Ana",
                begin_ts: 21_000 + index,
                grade: "S",
                friend_bnet: "s21-friend",
                friend_name: "S21 Friend",
            },
        );
        seed_match(
            db,
            MatchSeed {
                season: 22,
                id: Box::leak(format!("s22-match-{index}").into_boxed_str()),
                result: "loss",
                map_guid: "map-dorado",
                map_name: "Dorado",
                hero_guid: "hero-tracer",
                hero_name: "Tracer",
                begin_ts: 22_000 + index,
                grade: "C",
                friend_bnet: "s22-friend",
                friend_name: "S22 Friend",
            },
        );
    }
}

#[test]
fn page_queries_keep_selected_seasons_isolated() {
    let db = test_db();
    seed_two_seasons(&db);

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(21) }).unwrap();
    assert_eq!(review.summary.season, Some(21));
    assert_eq!(review.summary.total_matches, 5);
    assert_eq!(review.summary.wins, 5);
    assert_eq!(review.summary.losses, 0);

    let rankings = ranking::get_rankings(&db, Some(21)).unwrap();
    assert_eq!(rankings.season, Some(21));
    assert_eq!(rankings.heroes.len(), 1);
    assert_eq!(rankings.heroes[0].name, "Ana");
    assert_eq!(rankings.heroes[0].matches, 5);
    assert_eq!(rankings.heroes[0].rating.as_deref(), Some("S"));
    assert_eq!(rankings.maps.len(), 1);
    assert_eq!(rankings.maps[0].name, "Ilios");
    assert_eq!(rankings.maps[0].best_hero_name.as_deref(), Some("Ana"));

    let analysis = get_season_analysis(&db, Some(21)).unwrap();
    assert_eq!(analysis.season, Some(21));
    assert_eq!(analysis.friend.summary.season, Some(21));
    assert_eq!(analysis.friend.summary.preset_matches, 5);
    assert_eq!(analysis.friend.summary.eligible_friends, 1);
    assert_eq!(analysis.friend.common_friend_heroes[0].name, "S21 Friend");
    assert_eq!(analysis.map_cards.ace_map.unwrap().name, "Ilios");
}

#[test]
fn match_list_filters_keep_s21_and_s22_isolated() {
    let db = test_db();
    seed_two_seasons(&db);

    let s21_rows = list_matches(
        &db,
        MatchFilters {
            season: Some(21),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s21_rows.len(), 5);
    assert!(s21_rows.iter().all(|row| row.season == 21));
    assert!(s21_rows.iter().all(|row| row.map_name == "Ilios"));

    let s22_rows = list_matches(
        &db,
        MatchFilters {
            season: Some(22),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s22_rows.len(), 5);
    assert!(s22_rows.iter().all(|row| row.season == 22));
    assert!(s22_rows.iter().all(|row| row.map_name == "Dorado"));
}

#[test]
fn ranking_returns_dynamic_seasons_and_falls_back_from_missing_season() {
    let db = test_db();
    seed_two_seasons(&db);
    for (season, id) in [(15, "s15-match"), (20, "s20-match")] {
        seed_match(
            &db,
            MatchSeed {
                season,
                id,
                result: "win",
                map_guid: "map-legacy",
                map_name: "Legacy Map",
                hero_guid: "hero-legacy",
                hero_name: "Legacy Hero",
                begin_ts: season * 1_000,
                grade: "A",
                friend_bnet: "legacy-friend",
                friend_name: "Legacy Friend",
            },
        );
    }

    let ranking = ranking::get_rankings(&db, Some(99)).unwrap();
    let response = serde_json::to_value(&ranking).unwrap();

    assert_eq!(
        response["availableSeasons"],
        serde_json::json!([22, 21, 20, 15])
    );
    assert_eq!(ranking.season, Some(22));
    assert_eq!(ranking.heroes.len(), 1);
    assert_eq!(ranking.heroes[0].name, "Tracer");
}

#[test]
fn match_id_is_globally_unique_for_detail_lookup() {
    let db = test_db();
    seed_match(
        &db,
        MatchSeed {
            season: 21,
            id: "same-match-id",
            result: "win",
            map_guid: "map-ilios",
            map_name: "Ilios",
            hero_guid: "hero-ana",
            hero_name: "Ana",
            begin_ts: 21_000,
            grade: "S",
            friend_bnet: "s21-friend",
            friend_name: "S21 Friend",
        },
    );

    let conn = db.connect().unwrap();
    let duplicate = conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES ('same-match-id', 22, 'preset', 'SportPreset', 'normal', 'loss',
            'map-dorado', 'Dorado', 'hero-tracer', 'Tracer', 22000, 1, 2,
            10, 2, 6, 5000, 500, 100, 0, 800, '{}', '2026-07-09T00:00:00Z')
        "#,
        [],
    );

    assert!(duplicate.is_err());
}
