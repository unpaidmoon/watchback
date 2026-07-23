use owstats_desktop_lib::{get_season_analysis, OwDatabase};
use rusqlite::params;
use serde_json::json;

fn seeded_db() -> OwDatabase {
    let dir = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(dir.keep().join("choice-rankings.db"));
    db.initialize().unwrap();
    db
}

fn insert_normal(
    db: &OwDatabase,
    id: &str,
    season: i64,
    result: &str,
    current_perks: &[&str],
    teammate_perks: &[&str],
) {
    let detail = json!({
        "payload": {"data": {"totalCount": {"teammateList": [
            {"bnetId": "current-player", "perks": current_perks.iter().map(|guid| json!({"guid": guid, "perkLevel": 1})).collect::<Vec<_>>()},
            {"bnetId": "teammate", "perks": teammate_perks.iter().map(|guid| json!({"guid": guid, "perkLevel": 1})).collect::<Vec<_>>()}
        ]}}}
    });
    insert_match(db, id, season, "preset", "normal", result, &detail);
    db.connect()
        .unwrap()
        .execute(
            "INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, kill, assist, death, hero_damage, cure,
                resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
                role_type, raw_json, updated_at
            ) VALUES (?1, ?2, 'teammate', 'current-player', '本人', 'hero', '英雄',
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, NULL, '{}', '2026-07-23T00:00:00Z')",
            params![id, season],
        )
        .unwrap();
}

fn insert_stadium(
    db: &OwDatabase,
    id: &str,
    season: i64,
    result: &str,
    final_trait: &str,
    final_mod: &str,
) {
    let detail = json!({
        "payload": {"data": {"roundCountList": [
            {"teammateList": [{"bnetId": "current-player", "traitGuids": ["round-only-trait"], "modGuids": ["round-only-mod"]}]},
            {"teammateList": [{"bnetId": "current-player", "traitGuids": [final_trait, final_trait], "modGuids": [final_mod, final_mod]}]}
        ]}}
    });
    insert_match(db, id, season, "stadium", "fight", result, &detail);
}

fn insert_match(
    db: &OwDatabase,
    id: &str,
    season: i64,
    bucket: &str,
    kind: &str,
    result: &str,
    detail: &serde_json::Value,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        "INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        ) VALUES (?1, ?2, ?3, '', ?4, ?5, 'map', '地图', 'hero', '英雄',
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, '{}', '2026-07-23T00:00:00Z')",
        params![id, season, bucket, kind, result],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
         VALUES (?1, ?2, ?3, '2026-07-23T00:00:00Z')",
        params![id, kind, detail.to_string()],
    )
    .unwrap();
}

#[test]
fn rankings_are_season_scoped_current_player_only_and_deduplicated() {
    let db = seeded_db();
    for index in 0..5 {
        insert_normal(
            &db,
            &format!("normal-{index}"),
            22,
            if index < 3 {
                "win"
            } else if index == 3 {
                "loss"
            } else {
                "draw"
            },
            &["my-perk", "my-perk"],
            &["teammate-perk"],
        );
    }
    insert_normal(&db, "old-season", 21, "win", &["old-perk"], &[]);

    let analysis = get_season_analysis(&db, Some(22)).unwrap();
    let entry = &analysis.choice_rankings.normal_perks[0];
    assert_eq!(
        (entry.key.as_str(), entry.matches, entry.wins),
        ("my-perk", 5, 3)
    );
    assert!(analysis
        .choice_rankings
        .normal_perks
        .iter()
        .all(|item| item.key != "teammate-perk"));
    assert!(analysis
        .choice_rankings
        .normal_perks
        .iter()
        .all(|item| item.key != "old-perk"));
}

#[test]
fn wilson_sorting_penalizes_small_samples_and_uses_stable_ties() {
    let db = seeded_db();
    for index in 0..10 {
        for key in ["eight-of-ten", "alpha", "beta"] {
            let wins = if key == "eight-of-ten" { 8 } else { 7 };
            insert_normal(
                &db,
                &format!("{key}-{index}"),
                22,
                if index < wins { "win" } else { "loss" },
                &[key],
                &[],
            );
        }
    }
    for index in 0..5 {
        insert_normal(
            &db,
            &format!("four-of-five-{index}"),
            22,
            if index < 4 { "win" } else { "loss" },
            &["four-of-five"],
            &[],
        );
    }

    let analysis = get_season_analysis(&db, Some(22)).unwrap();
    let keys = analysis
        .choice_rankings
        .normal_perks
        .iter()
        .map(|item| item.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["eight-of-ten", "alpha", "beta"]);
}

#[test]
fn stadium_uses_only_the_final_loadout_and_applies_minimum_sample() {
    let db = seeded_db();
    for index in 0..5 {
        insert_stadium(
            &db,
            &format!("stadium-{index}"),
            22,
            if index < 4 { "win" } else { "loss" },
            "final-trait",
            "final-mod",
        );
    }

    let rankings = get_season_analysis(&db, Some(22)).unwrap().choice_rankings;
    assert_eq!(rankings.min_sample, 5);
    assert_eq!(rankings.confidence_level, 0.95);
    assert_eq!(rankings.stadium_traits[0].matches, 5);
    assert_eq!(rankings.stadium_mods[0].matches, 5);
    assert!(rankings
        .stadium_traits
        .iter()
        .all(|item| item.key != "round-only-trait"));
    assert!(rankings
        .stadium_mods
        .iter()
        .all(|item| item.key != "round-only-mod"));
}
