use crate::db::OwDatabase;
use crate::models::{RankingResponse, RankingRow};
use anyhow::Result;
use rusqlite::ToSql;

pub fn get_rankings(db: &OwDatabase, season: Option<i64>) -> Result<RankingResponse> {
    db.initialize()?;
    let selected = season.or_else(|| latest_season(db).ok().flatten());
    Ok(RankingResponse {
        season: selected,
        heroes: ranking_rows(db, selected, "hero_guid", "COALESCE(hero_name, hero_guid)")?,
        maps: map_rows_with_labels(db, selected)?,
    })
}

fn latest_season(db: &OwDatabase) -> Result<Option<i64>> {
    let conn = db.connect()?;
    let season = conn.query_row(
        "SELECT season FROM matches WHERE bucket='preset' ORDER BY season DESC LIMIT 1",
        [],
        |row| row.get(0),
    );
    match season {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn map_rows_with_labels(db: &OwDatabase, season: Option<i64>) -> Result<Vec<RankingRow>> {
    let mut rows = ranking_rows(db, season, "map_guid", "COALESCE(map_name, map_guid)")?;
    for row in &mut rows {
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
               SUM(CASE WHEN result='loss' THEN 1 ELSE 0 END) AS losses,
               SUM(duration_sec) AS play_time_sec
        FROM matches
        WHERE bucket='preset'
        "#
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(season) = season {
        sql.push_str(" AND season=?");
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
        let play_time_sec: i64 = row.get(5)?;
        Ok(RankingRow {
            key: row.get(0)?,
            name: row.get(1)?,
            matches,
            wins,
            losses: row.get(4)?,
            win_rate: round2(ratio(wins, matches)),
            play_time_hours: round1(play_time_sec as f64 / 3600.0),
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
