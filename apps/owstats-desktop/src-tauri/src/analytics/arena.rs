use super::FriendMatchRow;
use crate::db::OwDatabase;
use crate::models::FriendAnalyticsFilters;
use anyhow::Result;
use rusqlite::ToSql;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(super) struct ArenaAnalyticsRows {
    pub(super) friend_rows: Vec<FriendMatchRow>,
}

struct ArenaMatchSource {
    match_id: String,
    result: String,
    begin_ts: i64,
    map_name: String,
    raw_detail_json: String,
}

pub(super) fn arena_analytics_rows(
    db: &OwDatabase,
    filters: &FriendAnalyticsFilters,
) -> Result<ArenaAnalyticsRows> {
    let conn = db.connect()?;
    let Some(current_bnet_id) = current_player_bnet_id(&conn)? else {
        return Ok(ArenaAnalyticsRows {
            friend_rows: Vec::new(),
        });
    };
    let hero_names = hero_names(&conn)?;
    let mut sql = String::from(
        r#"
        SELECT m.match_id, m.result, m.begin_ts,
               COALESCE(NULLIF(TRIM(m.map_name), ''), '未知地图'),
               d.raw_detail_json
        FROM matches m
        JOIN match_details d ON d.match_id = m.match_id
        WHERE m.bucket = 'stadium'
          AND m.game_mode IN ('SportFight', 'LeisureFight')
        "#,
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(season) = filters.season {
        sql.push_str(" AND m.season = ?");
        owned.push(Box::new(season));
    }
    sql.push_str(" ORDER BY m.begin_ts DESC, m.match_id");
    let params = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let sources = stmt
        .query_map(params.as_slice(), |row| {
            Ok(ArenaMatchSource {
                match_id: row.get(0)?,
                result: row.get(1)?,
                begin_ts: row.get(2)?,
                map_name: row.get(3)?,
                raw_detail_json: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut friend_rows = Vec::new();
    for source in sources {
        let Ok(detail) = serde_json::from_str::<Value>(&source.raw_detail_json) else {
            continue;
        };
        friend_rows.extend(friend_rows_for_match(
            &source,
            &detail,
            &current_bnet_id,
            &hero_names,
            filters,
        ));
    }
    Ok(ArenaAnalyticsRows { friend_rows })
}

fn current_player_bnet_id(conn: &rusqlite::Connection) -> Result<Option<String>> {
    let from_players = conn.query_row(
        r#"
        SELECT TRIM(bnet_id)
        FROM players
        WHERE bnet_id IS NOT NULL AND TRIM(bnet_id) <> ''
        ORDER BY updated_at DESC, player_id DESC
        LIMIT 1
        "#,
        [],
        |row| row.get::<_, String>(0),
    );
    match from_players {
        Ok(value) => return Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => {}
        Err(error) => return Err(error.into()),
    }

    let from_projection = conn.query_row(
        r#"
        SELECT TRIM(p.bnet_id)
        FROM match_players p
        JOIN matches m ON m.match_id = p.match_id
        WHERE p.is_current_player = 1 AND TRIM(p.bnet_id) <> ''
        ORDER BY m.begin_ts DESC, p.updated_at DESC
        LIMIT 1
        "#,
        [],
        |row| row.get::<_, String>(0),
    );
    match from_projection {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn hero_names(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT guid, name FROM hero_dict")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(Into::into)
}

fn friend_rows_for_match(
    source: &ArenaMatchSource,
    detail: &Value,
    current_bnet_id: &str,
    hero_names: &HashMap<String, String>,
    filters: &FriendAnalyticsFilters,
) -> Vec<FriendMatchRow> {
    let Some(data) = detail.pointer("/payload/data") else {
        return Vec::new();
    };
    let Some(round) = last_valid_round(data, current_bnet_id) else {
        return Vec::new();
    };
    let Some(teammates) = round.get("teammateList").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(current) = teammates
        .iter()
        .find(|player| scalar_field(player, "bnetId").as_deref() == Some(current_bnet_id))
    else {
        return Vec::new();
    };
    let friend_ids = scalar_set(current.get("friendBnetIds"));
    if friend_ids.is_empty() {
        return Vec::new();
    }
    let recognized = teammates
        .iter()
        .filter_map(|player| scalar_field(player, "bnetId"))
        .filter(|bnet_id| friend_ids.contains(bnet_id) && bnet_id != current_bnet_id)
        .collect::<HashSet<_>>();
    if recognized.is_empty() {
        return Vec::new();
    }
    let group_size = 1 + recognized.len() as i64;
    let name_map = data
        .get("nameMap")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let hero_filter = filters
        .hero
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut emitted = HashSet::new();
    teammates
        .iter()
        .filter_map(|player| {
            let bnet_id = scalar_field(player, "bnetId")?;
            if !recognized.contains(&bnet_id) || !emitted.insert(bnet_id.clone()) {
                return None;
            }
            let hero_guid = scalar_field(player, "heroGuid").unwrap_or_default();
            let hero_name = hero_names
                .get(&hero_guid)
                .cloned()
                .unwrap_or_else(|| "未知英雄".to_string());
            if hero_filter.is_some_and(|value| value != hero_guid && value != hero_name) {
                return None;
            }
            Some(FriendMatchRow {
                bnet_id: bnet_id.clone(),
                name: safe_player_name(player, &bnet_id, &name_map, "未知好友"),
                match_id: source.match_id.clone(),
                result: source.result.clone(),
                begin_ts: source.begin_ts,
                map_name: source.map_name.clone(),
                hero_guid,
                hero_name,
                kills: integer_field(player, "kill"),
                assists: integer_field(player, "assist"),
                deaths: integer_field(player, "death"),
                damage: integer_field(player, "heroDamage"),
                healing: integer_field(player, "cure"),
                mitigation: integer_field(player, "resistDamage"),
                damage_taken: integer_field(player, "damageTaken"),
                healing_taken: integer_field(player, "healingTaken"),
                group_size,
            })
        })
        .collect()
}

fn last_valid_round<'a>(data: &'a Value, current_bnet_id: &str) -> Option<&'a Value> {
    data.get("roundCountList")?
        .as_array()?
        .iter()
        .rev()
        .find(|round| {
            round.is_object()
                && round
                    .get("teammateList")
                    .and_then(Value::as_array)
                    .is_some_and(|players| {
                        !players.is_empty()
                            && players.iter().any(|player| {
                                scalar_field(player, "bnetId").as_deref() == Some(current_bnet_id)
                            })
                    })
        })
}

fn scalar_set(value: Option<&Value>) -> HashSet<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(scalar_value).collect(),
        Some(value) => scalar_value(value).into_iter().collect(),
        None => HashSet::new(),
    }
}

fn scalar_field(value: &Value, key: &str) -> Option<String> {
    scalar_value(value.get(key)?)
}

fn scalar_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .or_else(|| value.as_u64().map(|number| number.to_string()))
}

fn integer_field(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_f64().map(|number| number.round() as i64))
                .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
        })
        .unwrap_or_default()
}

fn safe_player_name(
    player: &Value,
    bnet_id: &str,
    name_map: &serde_json::Map<String, Value>,
    fallback: &str,
) -> String {
    scalar_field(player, "name")
        .or_else(|| name_map.get(bnet_id).and_then(scalar_value))
        .filter(|name| name != bnet_id && !looks_like_long_numeric_id(name))
        .unwrap_or_else(|| fallback.to_string())
}

fn looks_like_long_numeric_id(value: &str) -> bool {
    value.len() >= 4 && value.chars().all(|item| item.is_ascii_digit())
}
