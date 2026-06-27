use owstats_desktop_lib::{get_season_analysis, ranking, OwDatabase};
use rusqlite::params;

fn seeded_ranking_db() -> OwDatabase {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.keep().join("owstats.db");
    let db = OwDatabase::new(path);
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    let mut idx = 0_i64;
    let mut insert = |hero: &str, map: &str, result: &str, duration: i64, begin_ts: i64| {
        idx += 1;
        conn.execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
                hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
                deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
            )
            VALUES (?1, 22, 'preset', 'SportPreset', 'normal', ?2, ?3, ?4, ?5, ?5,
                ?6, 1, 0, 1, 0, 1, 100, 10, 0, 0, ?7, '{}', '2026-06-27T00:00:00Z')
            "#,
            params![
                format!("match-{idx}"),
                result,
                format!("map-{map}"),
                map,
                hero,
                begin_ts,
                duration
            ],
        )
        .unwrap();
    };
    for i in 0..7 {
        insert("Tracer", "King's Row", "win", 900, 1000 + i);
    }
    for i in 0..3 {
        insert("Tracer", "King's Row", "loss", 300, 1100 + i);
    }
    for i in 0..5 {
        insert("Soldier", "Numbani", "loss", 600, 1200 + i);
    }
    for i in 0..6 {
        insert(
            "Dva",
            "Dorado",
            if i < 3 { "win" } else { "loss" },
            400,
            1300 + i,
        );
    }
    insert("Ana", "Ilios", "win", 200, 900);
    db
}

#[test]
fn hero_ranking_returns_backend_aggregates_and_default_sort() {
    let db = seeded_ranking_db();
    let ranking = ranking::get_rankings(&db, None).unwrap();
    let first = &ranking.heroes[0];

    assert_eq!(first.name, "Ana");
    assert_eq!(first.matches, 1);
    assert_eq!(first.wins, 1);
    assert_eq!(first.losses, 0);
    assert_eq!(first.win_rate, 1.0);
}

#[test]
fn map_ranking_uses_win_rate_matches_playtime_tiebreaks() {
    let db = seeded_ranking_db();
    let ranking = ranking::get_rankings(&db, None).unwrap();
    assert_eq!(ranking.maps[0].name, "Ilios");
    assert_eq!(ranking.maps[1].name, "King's Row");
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
