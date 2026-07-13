use crate::db::OwDatabase;
use crate::models::{RankingResponse, RankingRow};
use anyhow::Result;
use rusqlite::ToSql;
use std::collections::HashMap;

pub fn get_rankings(db: &OwDatabase, season: Option<i64>) -> Result<RankingResponse> {
    db.initialize()?;
    let available_seasons = available_seasons(db)?;
    let selected = season
        .filter(|requested| available_seasons.contains(requested))
        .or_else(|| available_seasons.first().copied());
    Ok(RankingResponse {
        season: selected,
        available_seasons,
        heroes: ranking_rows(db, selected, "hero_guid", "COALESCE(hero_name, hero_guid)")?,
        maps: map_rows_with_labels(db, selected)?,
    })
}

fn available_seasons(db: &OwDatabase) -> Result<Vec<i64>> {
    let conn = db.connect()?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT season FROM matches WHERE bucket='preset' ORDER BY season DESC",
    )?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn map_rows_with_labels(db: &OwDatabase, season: Option<i64>) -> Result<Vec<RankingRow>> {
    let mut rows = ranking_rows(db, season, "map_guid", "COALESCE(map_name, map_guid)")?;
    let best_heroes = best_heroes_by_map(db, season)?;
    for row in &mut rows {
        if let Some(best_hero) = best_heroes.get(&row.key) {
            row.best_hero_guid = Some(best_hero.guid.clone());
            row.best_hero_name = Some(best_hero.name.clone());
        }
        row.adaptability_label = Some(if row.matches >= 5 {
            "样本达标".to_string()
        } else {
            "样本较少".to_string()
        });
    }
    Ok(rows)
}

fn ranking_rows(
    db: &OwDatabase,
    season: Option<i64>,
    key_expr: &str,
    name_expr: &str,
) -> Result<Vec<RankingRow>> {
    let conn = db.connect()?;
    let mut sql = format!(
        r#"
        SELECT {key_expr}, {name_expr},
               COUNT(*) AS matches,
               SUM(CASE WHEN result='win' THEN 1 ELSE 0 END) AS wins,
               SUM(duration_sec) AS play_time_sec,
               SUM(kills) AS kills,
               SUM(assists) AS assists,
               SUM(deaths) AS deaths,
               AVG(CASE e.grade
                    WHEN 'S' THEN 4.0
                    WHEN 'A' THEN 3.0
                    WHEN 'B' THEN 2.0
                    WHEN 'C' THEN 1.0
                    ELSE NULL
               END) AS rating_score
        FROM matches
        LEFT JOIN match_evaluations e ON e.match_id = matches.match_id
        WHERE matches.bucket='preset'
        "#
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(season) = season {
        sql.push_str(" AND matches.season=?");
        owned.push(Box::new(season));
    }
    sql.push_str(&format!(" GROUP BY {key_expr}, {name_expr}"));
    let params = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        let matches: i64 = row.get(2)?;
        let wins: i64 = row.get(3)?;
        let play_time_sec: i64 = row.get(4)?;
        let kills: i64 = row.get(5)?;
        let assists: i64 = row.get(6)?;
        let deaths: i64 = row.get(7)?;
        let rating_score: Option<f64> = row.get(8)?;
        let kda = kda(kills, assists, deaths);
        Ok(RankingRow {
            key: row.get(0)?,
            name: row.get(1)?,
            matches,
            win_rate: round2(ratio(wins, matches)),
            play_time_hours: round1(play_time_sec as f64 / 3600.0),
            kda,
            rating: rating_from_score(rating_score),
            best_hero_guid: None,
            best_hero_name: None,
            adaptability_label: None,
        })
    })?;
    let mut rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    rows.sort_by(|a, b| {
        b.win_rate
            .partial_cmp(&a.win_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.matches.cmp(&a.matches))
            .then_with(|| {
                b.play_time_hours
                    .partial_cmp(&a.play_time_hours)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(rows)
}

#[derive(Debug, Clone)]
struct BestHero {
    guid: String,
    name: String,
}

fn best_heroes_by_map(db: &OwDatabase, season: Option<i64>) -> Result<HashMap<String, BestHero>> {
    let conn = db.connect()?;
    let mut sql = r#"
        SELECT map_guid,
               hero_guid,
               COALESCE(hero_name, hero_guid) AS hero_name,
               COUNT(*) AS matches,
               SUM(CASE WHEN result='win' THEN 1 ELSE 0 END) AS wins,
               SUM(duration_sec) AS play_time_sec
        FROM matches
        WHERE bucket='preset'
        "#
    .to_string();
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(season) = season {
        sql.push_str(" AND season=?");
        owned.push(Box::new(season));
    }
    sql.push_str(
        r#"
        GROUP BY map_guid, hero_guid, COALESCE(hero_name, hero_guid)
        ORDER BY map_guid ASC,
                 (CAST(wins AS REAL) + 2.5) / (matches + 5.0) DESC,
                 CAST(wins AS REAL) / CASE WHEN matches <= 0 THEN 1 ELSE matches END DESC,
                 matches DESC,
                 play_time_sec DESC,
                 hero_name ASC
        "#,
    );
    let params = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            BestHero {
                guid: row.get(1)?,
                name: row.get(2)?,
            },
        ))
    })?;
    let mut best_heroes = HashMap::new();
    for row in rows {
        let (map_guid, hero) = row?;
        best_heroes.entry(map_guid).or_insert(hero);
    }
    Ok(best_heroes)
}

fn kda(kills: i64, assists: i64, deaths: i64) -> f64 {
    round2((kills + assists) as f64 / deaths.max(1) as f64)
}

fn rating_from_score(score: Option<f64>) -> Option<String> {
    let score = score?;
    let grade = if score >= 3.5 {
        "S"
    } else if score >= 2.5 {
        "A"
    } else if score >= 1.5 {
        "B"
    } else {
        "C"
    };
    Some(grade.to_string())
}

fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        value as f64 / total as f64
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
