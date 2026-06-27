use owstats_desktop_lib::{get_match_detail, OwDatabase};
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

#[test]
fn match_detail_returns_same_role_current_values_and_averages() {
    let db = seeded_role_db();
    let detail = get_match_detail(&db, "match-tank-2").unwrap();
    let comparison = detail.same_role_comparison.unwrap();

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
