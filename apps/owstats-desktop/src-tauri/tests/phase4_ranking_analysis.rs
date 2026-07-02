use owstats_desktop_lib::{get_season_analysis, ranking, OwDatabase};
use rusqlite::params;

fn seeded_ranking_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.keep().join("owstats.db");
    let db = OwDatabase::new(path);
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    let mut idx = 0_i64;
    let mut insert = |hero: &str,
                      map: &str,
                      result: &str,
                      duration: i64,
                      begin_ts: i64,
                      kills: i64,
                      assists: i64,
                      deaths: i64,
                      grade: &str| {
        idx += 1;
        let match_id = format!("match-{idx}");
        conn.execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
                hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
                deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
            )
            VALUES (?1, 22, 'preset', 'SportPreset', 'normal', ?2, ?3, ?4, ?5, ?5,
                ?6, 1, 0, ?8, ?9, ?10, 100, 10, 0, 0, ?7, '{}', '2026-06-27T00:00:00Z')
            "#,
            params![
                match_id,
                result,
                format!("map-{map}"),
                map,
                hero,
                begin_ts,
                duration,
                kills,
                assists,
                deaths
            ],
        )
        .unwrap();
        conn.execute(
            r#"
            INSERT INTO match_evaluations (
                match_id, mode, rule_version, grade, primary_tag, evidence_tags_json,
                reasons_json, confidence, computed_at
            )
            VALUES (?1, 'SportPreset', 1, ?2, '测试标签', '[]', '[]', 1.0, '2026-06-27T00:00:00Z')
            "#,
            params![match_id, grade],
        )
        .unwrap();
    };
    for i in 0..7 {
        insert("Tracer", "King's Row", "win", 900, 1000 + i, 20, 5, 5, "S");
    }
    for i in 0..3 {
        insert(
            "Tracer",
            "King's Row",
            "loss",
            300,
            1100 + i,
            10,
            0,
            10,
            "B",
        );
    }
    insert("Widow", "King's Row", "win", 120, 1150, 12, 0, 2, "S");
    for i in 0..5 {
        insert("Soldier", "Numbani", "loss", 600, 1200 + i, 5, 5, 5, "C");
    }
    for i in 0..6 {
        insert(
            "Dva",
            "Dorado",
            if i < 3 { "win" } else { "loss" },
            400,
            1300 + i,
            4,
            2,
            4,
            "B",
        );
    }
    insert("Ana", "Ilios", "win", 200, 900, 8, 2, 1, "S");
    db
}

#[test]
fn hero_ranking_returns_backend_aggregates_and_default_sort() {
    let db = seeded_ranking_db();
    let ranking = ranking::get_rankings(&db, None).unwrap();
    let first = ranking.heroes.iter().find(|row| row.name == "Ana").unwrap();

    assert_eq!(first.name, "Ana");
    assert_eq!(first.matches, 1);
    assert_eq!(first.win_rate, 1.0);
    assert_eq!(first.kda, 10.0);
    assert_eq!(first.rating.as_deref(), Some("S"));
}

#[test]
fn map_ranking_uses_win_rate_matches_playtime_tiebreaks() {
    let db = seeded_ranking_db();
    let ranking = ranking::get_rankings(&db, None).unwrap();
    assert_eq!(ranking.maps[0].name, "Ilios");
    assert_eq!(ranking.maps[0].best_hero_name.as_deref(), Some("Ana"));
    assert_eq!(ranking.maps[0].kda, 10.0);
    assert_eq!(ranking.maps[0].rating.as_deref(), Some("S"));
    assert_eq!(ranking.maps[1].name, "King's Row");
    assert_eq!(ranking.maps[1].best_hero_name.as_deref(), Some("Tracer"));
    assert_eq!(ranking.maps[1].rating.as_deref(), Some("A"));
}

#[test]
fn map_analysis_uses_min_sample_and_tiebreaks() {
    let db = seeded_ranking_db();
    let analysis = get_season_analysis(&db, None).unwrap();

    assert_eq!(analysis.map_cards.ace_map.unwrap().name, "King's Row");
    assert_eq!(analysis.map_cards.throw_zone.unwrap().name, "Numbani");
    assert_eq!(analysis.map_cards.home_field.unwrap().name, "King's Row");
    assert_eq!(analysis.map_cards.rare_guest.unwrap().name, "Ilios");
    assert_eq!(analysis.map_cards.min_sample, 5);
}
