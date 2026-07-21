use owstats_desktop_lib::{list_matches, refresh_match_evaluations, MatchFilters, OwDatabase};
use rusqlite::params;
use serde_json::json;

fn seed_stadium_match(db: &OwDatabase, match_id: &str, game_mode: &str, result: &str) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        ) VALUES (?1, 22, 'stadium', ?2, 'fight', ?3, 'map-a', '地图 A',
            'hero-current', '英雄 A', 1800000000, 2, 1, 12, 8, 7, 9000, 6000, 5000,
            0, 600, '{}', '2026-07-20T00:00:00Z')
        "#,
        params![match_id, game_mode, result],
    )
    .unwrap();

    let raw_detail = json!({
        "payload": { "data": {
            "roundCountList": [
                {
                    "mapGuid": "map-a",
                    "teamScore": 2,
                    "opponentScore": 1,
                    "gameTimeSec": 600,
                    "teammateList": [
                        {
                            "bnetId": "current",
                            "name": "当前玩家",
                            "heroGuid": "hero-current",
                            "kill": 12,
                            "assist": 8,
                            "death": 7,
                            "heroDamage": 9000,
                            "cure": 6000,
                            "resistDamage": 5000
                        },
                        {
                            "bnetId": "ally",
                            "name": "队友",
                            "heroGuid": "hero-ally",
                            "kill": 4,
                            "assist": 3,
                            "death": 2,
                            "heroDamage": 3000,
                            "cure": 1000,
                            "resistDamage": 500
                        }
                    ],
                    "enemyList": [
                        {
                            "bnetId": "enemy",
                            "kill": 8,
                            "assist": 2,
                            "death": 5,
                            "heroDamage": 7000,
                            "cure": 2000,
                            "resistDamage": 1000
                        }
                    ]
                },
                { "mapGuid": "map-empty", "teammateList": [], "enemyList": [] }
            ]
        }}
    })
    .to_string();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'fight', ?2, '2026-07-20T00:00:00Z')",
        params![match_id, raw_detail],
    )
    .unwrap();
}

#[test]
fn stadium_competitive_and_quick_show_evaluation_and_final_valid_round_awards() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();
    seed_stadium_match(&db, "stadium-competitive", "SportFight", "win");
    seed_stadium_match(&db, "stadium-quick", "LeisureFight", "loss");

    refresh_match_evaluations(&db).unwrap();
    let matches = list_matches(&db, MatchFilters::default()).unwrap();

    for match_id in ["stadium-competitive", "stadium-quick"] {
        let item = matches
            .iter()
            .find(|item| item.match_id == match_id)
            .expect("stadium match in list");
        assert!(item.evaluation.is_some(), "{match_id} evaluation");
        assert_eq!(
            item.team_awards
                .iter()
                .map(|award| award.code.as_str())
                .collect::<Vec<_>>(),
            vec!["kill", "damage", "healing", "mitigation", "death"]
        );
        assert_eq!(item.kills, 12);
        assert_eq!(item.damage, 9000);
    }
}
