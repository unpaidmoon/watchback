use owstats_desktop_lib::{get_match_detail, list_matches, MatchFilters, OwDatabase};
use rusqlite::params;

fn seeded_role_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.keep().join("owstats.db");
    let db = OwDatabase::new(path);
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    for (match_id, role, kills, deaths, damage, healing) in [
        ("match-tank-1", "tank", 10, 6, 5000, 1000),
        ("match-tank-2", "tank", 20, 4, 7000, 1200),
        ("match-dps-1", "dps", 30, 8, 9000, 0),
        ("match-no-role", "", 11, 3, 3000, 300),
    ] {
        conn.execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
                hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
                deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
            )
            VALUES (?1, 22, 'preset', 'SportPreset', 'normal', 'win', 'map-a', 'Map A',
                'hero-a', 'Hero A', 1800000000, 2, 1, ?3, 0, ?4, ?5, ?6, 0, 0, 600, '{}',
                '2026-06-27T00:00:00Z')
            "#,
            params![match_id, role, kills, deaths, damage, healing],
        )
        .unwrap();
        let raw_detail = format!(
            r#"{{"payload":{{"data":{{"teammateList":[{{"bnetId":"1","name":"Me","heroGuid":"hero-a","kill":{kills},"death":{deaths},"heroDamage":{damage},"cure":{healing},"roleType":"{role}"}}],"enemyList":[]}}}}}}"#
        );
        conn.execute(
            "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'normal', ?2, '2026-06-27T00:00:00Z')",
            params![match_id, raw_detail],
        )
        .unwrap();
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, kill, assist, death, hero_damage, cure,
                resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
                role_type, raw_json, updated_at
            )
            VALUES (?1, 22, 'teammate', '1', 'Me', 'hero-a', 'Hero A',
                1, 0, ?3, 0, ?4, ?5, ?6, 0, 0, 0, 0, 0, NULLIF(?2, ''), '{}',
                '2026-06-27T00:00:00Z')
            "#,
            params![match_id, role, kills, deaths, damage, healing],
        )
        .unwrap();
    }
    db
}

fn seeded_awards_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.keep().join("owstats.db");
    let db = OwDatabase::new(path);
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES ('award-match', 22, 'preset', 'SportPreset', 'normal', 'win', 'map-a', 'Map A',
            'hero-a', 'Hero A', 1800000000, 2, 1, 30, 6, 9, 12000, 8000, 9000, 0, 600, '{}',
            '2026-06-27T00:00:00Z')
        "#,
        [],
    )
    .unwrap();
    conn.execute(
        r#"
        INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
        VALUES ('award-match', 'normal', '{"payload":{"data":{"teammateList":[
            {"bnetId":"1","name":"Me","heroGuid":"hero-a","kill":30,"assist":6,"death":9,"heroDamage":12000,"cure":8000,"resistDamage":9000},
            {"bnetId":"2","name":"Ally","heroGuid":"hero-b","kill":10,"assist":8,"death":3,"heroDamage":7000,"cure":2000,"resistDamage":1000}
        ],"enemyList":[]}}}', '2026-06-27T00:00:00Z')
        "#,
        [],
    )
    .unwrap();
    for (bnet_id, name, is_current, kill, death, damage, healing, mitigation) in [
        ("1", "Me", 1, 30, 9, 12000, 8000, 9000),
        ("2", "Ally", 0, 10, 3, 7000, 2000, 1000),
    ] {
        conn.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, kill, assist, death, hero_damage, cure,
                resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
                role_type, raw_json, updated_at
            )
            VALUES ('award-match', 22, 'teammate', ?1, ?2, 'hero-a', 'Hero A',
                ?3, 0, ?4, 0, ?5, ?6, ?7, ?8, 0, 0, 0, 0, 'dps', '{}',
                '2026-06-27T00:00:00Z')
            "#,
            params![bnet_id, name, is_current, kill, death, damage, healing, mitigation],
        )
        .unwrap();
    }
    db
}

#[test]
fn match_detail_returns_same_role_current_values_and_averages() {
    let db = seeded_role_db();
    let detail = get_match_detail(&db, "match-tank-2").unwrap();
    let comparison = detail.same_role_comparison.unwrap();

    assert_eq!(detail.map_guid.as_deref(), Some("map-a"));
    assert_eq!(detail.map_name.as_deref(), Some("Map A"));
    assert_eq!(detail.hero_guid.as_deref(), Some("hero-a"));
    assert_eq!(detail.hero_name.as_deref(), Some("Hero A"));
    assert_eq!(detail.result.as_deref(), Some("win"));
    assert_eq!(detail.result_label.as_deref(), Some("\u{80dc}\u{5229}"));
    assert_eq!(detail.game_mode.as_deref(), Some("SportPreset"));
    assert_eq!(
        detail.mode_label.as_deref(),
        Some("\u{7ade}\u{6280}\u{9884}\u{8bbe}")
    );
    assert_eq!(detail.team_score, Some(2));
    assert_eq!(detail.opponent_score, Some(1));
    assert_eq!(detail.duration_sec, Some(600));
    assert_eq!(detail.begin_ts, Some(1800000000));
    assert!(detail.begin_label.is_some());
    assert_eq!(comparison.raw_role.as_deref(), Some("tank"));
    assert_eq!(comparison.role_label.as_deref(), Some("\u{5766}\u{514b}"));
    assert_eq!(comparison.current.kills, Some(20.0));
    assert_eq!(comparison.average.kills, Some(15.0));
    assert_eq!(comparison.current.deaths, Some(4.0));
    assert_eq!(comparison.average.deaths, Some(5.0));
}

#[test]
fn match_detail_hides_average_when_role_is_missing() {
    let db = seeded_role_db();
    let detail = get_match_detail(&db, "match-no-role").unwrap();
    assert!(detail.same_role_comparison.is_none());
}

#[test]
fn match_list_and_detail_return_current_player_performance_awards() {
    let db = seeded_awards_db();
    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    let awards = &matches[0].team_awards;
    assert_eq!(
        awards
            .iter()
            .map(|award| award.code.as_str())
            .collect::<Vec<_>>(),
        vec!["kill", "damage", "healing", "mitigation", "death"]
    );
    assert_eq!(awards[0].tone, "gold");
    assert_eq!(awards[4].tone, "danger");

    let detail = get_match_detail(&db, "award-match").unwrap();
    assert_eq!(detail.team_awards, matches[0].team_awards);
}
