#![allow(clippy::too_many_arguments)] // Review fixtures deliberately expose each match dimension.

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

fn seed_ranked_match_entry(
    db: &OwDatabase,
    id: &str,
    game_mode: &str,
    role_type: Option<&str>,
    result: &str,
    begin_ts: i64,
    rank_score: i64,
    rank_sub_tier: i64,
    kills: i64,
    assists: i64,
    deaths: i64,
) {
    let conn = db.connect().unwrap();
    let mut raw_entry = serde_json::json!({
        "gameMode": game_mode,
        "rankInfo": {
            "rankName": "Diamond",
            "rankScore": rank_score,
            "rankSubTier": rank_sub_tier
        }
    });
    if let Some(role_type) = role_type {
        raw_entry["roleType"] = serde_json::json!(role_type);
    }
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'preset', ?2, 'normal', ?3, 'Ilios', 'Ilios', 'hero', 'Hero', ?4,
            CASE WHEN ?3 = 'win' THEN 2 ELSE 1 END,
            CASE WHEN ?3 = 'win' THEN 1 ELSE 2 END,
            ?5, ?6, ?7, 1000, 100, 100, 0, 620, ?8, '2026-06-27T00:00:00Z')
        "#,
        rusqlite::params![
            id,
            game_mode,
            result,
            begin_ts,
            kills,
            assists,
            deaths,
            raw_entry.to_string()
        ],
    )
    .unwrap();
}

fn seed_ranked_match_entry_with_instance_type(
    db: &OwDatabase,
    id: &str,
    game_mode: &str,
    instance_type: &str,
    role_type: Option<&str>,
    result: &str,
    begin_ts: i64,
    rank_score: i64,
    rank_sub_tier: i64,
) {
    let conn = db.connect().unwrap();
    let mut raw_entry = serde_json::json!({
        "gameMode": game_mode,
        "instanceType": instance_type,
        "rankInfo": {
            "rankName": "Diamond",
            "rankScore": rank_score,
            "rankSubTier": rank_sub_tier
        }
    });
    if let Some(role_type) = role_type {
        raw_entry["roleType"] = serde_json::json!(role_type);
    }
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'preset', ?2, 'normal', ?3, 'Ilios', 'Ilios', 'hero', 'Hero', ?4,
            CASE WHEN ?3 = 'win' THEN 2 ELSE 1 END,
            CASE WHEN ?3 = 'win' THEN 1 ELSE 2 END,
            10, 5, 5, 1000, 100, 100, 0, 620, ?5, '2026-06-27T00:00:00Z')
        "#,
        rusqlite::params![id, game_mode, result, begin_ts, raw_entry.to_string()],
    )
    .unwrap();
}

fn seed_current_player_role(
    db: &OwDatabase,
    match_id: &str,
    role_type: &str,
    rank_score: i64,
    rank_sub_tier: i64,
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
        SELECT
            match_id, season, 'teammate', 'self', 'Self', hero_guid, hero_name,
            1, 0, kills, assists, deaths, damage, healing,
            mitigation, 0, 0, 0, 0, ?2, ?3, updated_at
        FROM matches
        WHERE match_id = ?1
        "#,
        rusqlite::params![
            match_id,
            role_type,
            serde_json::json!({
                "roleType": role_type,
                "rankInfo": {
                    "rankScore": rank_score,
                    "rankSubTier": rank_sub_tier
                }
            })
            .to_string()
        ],
    )
    .unwrap();
}

fn seed_stadium_competitive_match(
    db: &OwDatabase,
    id: &str,
    hero_guid: &str,
    result: &str,
    begin_ts: i64,
    rank_name: &str,
    rank_sub_tier: i64,
    kills: i64,
    assists: i64,
    deaths: i64,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'stadium', 'SportFight', 'fight', ?2, 'map', 'Map', 'hero', 'Hero', ?3,
            2, 1, 0, 0, 0, 0, 0, 0, 0, 600, ?4, '2026-07-14T00:00:00Z')
        "#,
        rusqlite::params![id, result, begin_ts, "{}"],
    )
    .unwrap();
    conn.execute(
        r#"
        INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
        VALUES (?1, 'fight', ?2, '2026-07-14T00:00:00Z')
        "#,
        rusqlite::params![
            id,
            serde_json::json!({
                "payload": { "data": { "totalCount": { "teammateList": [{
                    "bnetId": 9001,
                    "heroGuid": hero_guid,
                    "kill": kills,
                    "assist": assists,
                    "death": deaths,
                    "rankInfo": { "rankName": rank_name, "rankSubTier": rank_sub_tier }
                }] } } }
            })
            .to_string()
        ],
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

#[test]
fn personal_review_exposes_four_role_rank_summaries() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();
    seed_match(
        &db,
        "tank-1",
        "win",
        "Ilios",
        "Reinhardt",
        100,
        16,
        4,
        9_000,
        100,
        8_000,
        620,
    );
    seed_match(
        &db,
        "tank-2",
        "loss",
        "Ilios",
        "Reinhardt",
        200,
        8,
        6,
        6_000,
        100,
        6_000,
        620,
    );
    seed_match(
        &db, "dps-1", "win", "Dorado", "Tracer", 300, 22, 3, 14_000, 0, 200, 620,
    );

    seed_current_player_role(&db, "tank-1", "tank", 305, 5);
    seed_current_player_role(&db, "tank-2", "tank", 306, 4);
    seed_current_player_role(&db, "dps-1", "dps", 710, 2);

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    assert_eq!(review.role_summaries.len(), 4);
    let tank = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "tank")
        .unwrap();
    assert_eq!(tank.matches, 2);
    assert_eq!(tank.wins, 1);
    assert_eq!(tank.rank_level, 4);
    assert_eq!(tank.rank_sub_tier, 4);
    assert_eq!(tank.rank_key, "platinum");
    assert_eq!(tank.kda, 3.0);

    let dps = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "dps")
        .unwrap();
    assert_eq!(dps.rank_key, "champion");

    let support = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "healer")
        .unwrap();
    assert_eq!(support.matches, 0);
    assert_eq!(support.rank_key, "unranked");
}

#[test]
fn personal_review_uses_match_entry_rank_and_role_when_player_role_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();

    seed_ranked_match_entry(
        &db,
        "preset-tank",
        "SportPreset",
        Some("tank"),
        "win",
        100,
        496,
        4,
        10,
        5,
        5,
    );
    seed_ranked_match_entry(
        &db,
        "preset-tank-no-rank",
        "SportPreset",
        Some("tank"),
        "loss",
        300,
        0,
        0,
        6,
        2,
        3,
    );
    seed_ranked_match_entry(
        &db,
        "open-latest",
        "SportOpen",
        Some("dps"),
        "loss",
        200,
        595,
        5,
        20,
        2,
        4,
    );

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    let tank = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "tank")
        .unwrap();
    assert_eq!(tank.matches, 2);
    assert_eq!(tank.rank_key, "diamond");
    assert_eq!(tank.rank_sub_tier, 4);

    let open = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "open")
        .unwrap();
    assert_eq!(open.matches, 1);
    assert_eq!(open.rank_key, "master");
    assert_eq!(open.rank_sub_tier, 5);
    assert_eq!(open.kda, 5.5);
}

#[test]
fn role_rank_summary_counts_only_matching_competitive_mode() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();

    seed_ranked_match_entry(
        &db,
        "preset-tank",
        "SportPreset",
        Some("tank"),
        "win",
        100,
        496,
        4,
        10,
        1,
        5,
    );
    seed_ranked_match_entry(
        &db,
        "open-tank",
        "SportOpen",
        Some("tank"),
        "loss",
        200,
        595,
        5,
        20,
        2,
        4,
    );
    seed_ranked_match_entry(
        &db,
        "stadium-no-role",
        "SportFight",
        None,
        "win",
        300,
        698,
        2,
        30,
        3,
        3,
    );
    seed_ranked_match_entry(
        &db,
        "leisure-tank",
        "LeisurePreset",
        Some("tank"),
        "loss",
        400,
        798,
        1,
        40,
        4,
        4,
    );

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    let tank = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "tank")
        .unwrap();
    assert_eq!(tank.matches, 1);
    assert_eq!(tank.wins, 1);
    assert_eq!(tank.rank_key, "diamond");

    let open = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "open")
        .unwrap();
    assert_eq!(open.matches, 1);
    assert_eq!(open.wins, 0);
    assert_eq!(open.rank_key, "master");
}

#[test]
fn personal_review_counts_only_lowercase_competitive_roles_by_entry_role_type() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();

    seed_ranked_match_entry(
        &db,
        "s23-leisure-dps",
        "leisure",
        Some("dps"),
        "win",
        100,
        0,
        0,
        18,
        4,
        2,
    );
    seed_ranked_match_entry_with_instance_type(
        &db,
        "s23-sport-tank",
        "sport",
        "IT_RANKED",
        Some("tank"),
        "loss",
        200,
        598,
        2,
    );

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    let tank = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "tank")
        .unwrap();
    assert_eq!(tank.matches, 1);
    assert_eq!(tank.rank_key, "master");
    assert_eq!(tank.rank_sub_tier, 2);

    let dps = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "dps")
        .unwrap();
    assert_eq!(dps.matches, 0);
    assert_eq!(dps.wins, 0);
    assert_eq!(dps.rank_key, "unranked");
    assert_eq!(dps.kda, 0.0);
}

#[test]
fn personal_review_keeps_lowercase_sport_open_out_of_role_queue() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();

    seed_ranked_match_entry_with_instance_type(
        &db,
        "s21-sport-open",
        "sport",
        "IT_RULESET",
        Some("tank"),
        "win",
        100,
        595,
        3,
    );
    seed_ranked_match_entry_with_instance_type(
        &db,
        "s21-sport-tank",
        "sport",
        "IT_RANKED",
        Some("tank"),
        "loss",
        200,
        496,
        4,
    );

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    let open = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "open")
        .unwrap();
    assert_eq!(open.matches, 1);
    assert_eq!(open.wins, 1);
    assert_eq!(open.rank_key, "master");

    let tank = review
        .role_summaries
        .iter()
        .find(|item| item.role_type == "tank")
        .unwrap();
    assert_eq!(tank.matches, 1);
    assert_eq!(tank.wins, 0);
    assert_eq!(tank.rank_key, "diamond");
}

#[test]
fn personal_review_exposes_stadium_competitive_role_ranks_separately() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("phase3.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at) VALUES ('current', '9001', 'Current', '{}', '2026-07-14T00:00:00Z')",
        [],
    )
    .unwrap();

    seed_stadium_competitive_match(
        &db,
        "stadium-tank",
        "207165582859042823",
        "win",
        100,
        "Master",
        2,
        8,
        2,
        2,
    );
    seed_stadium_competitive_match(
        &db,
        "stadium-dps",
        "207165582859042818",
        "loss",
        200,
        "Diamond",
        4,
        12,
        1,
        3,
    );

    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();

    assert_eq!(review.stadium_role_summaries.len(), 3);
    let tank = review
        .stadium_role_summaries
        .iter()
        .find(|item| item.role_type == "tank")
        .unwrap();
    assert_eq!(tank.matches, 1);
    assert_eq!(tank.wins, 1);
    assert_eq!(tank.kda, 5.0);
    assert_eq!(tank.rank_key, "master");
    assert_eq!(tank.rank_sub_tier, 2);
    let support = review
        .stadium_role_summaries
        .iter()
        .find(|item| item.role_type == "healer")
        .unwrap();
    assert_eq!(support.matches, 0);
    assert_eq!(support.rank_key, "unranked");
}
