use crate::db::OwDatabase;
use crate::importer::is_valid_match_detail_for;
use anyhow::Result;
use rusqlite::params;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonSyncInventory {
    pub match_ids: HashSet<String>,
    pub detail_ids: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetailSelection {
    pub entries: Vec<Value>,
    pub new_matches: usize,
    pub missing_details: usize,
}

pub fn load_season_sync_inventory(db: &OwDatabase, season: i64) -> Result<SeasonSyncInventory> {
    db.initialize()?;
    let conn = db.connect()?;
    let mut statement = conn.prepare(
        "SELECT m.match_id,
                d.raw_detail_json
         FROM matches m
         LEFT JOIN match_details d ON d.match_id = m.match_id
         WHERE m.season = ?1",
    )?;
    let rows = statement.query_map(params![season], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    let mut inventory = SeasonSyncInventory::default();
    for row in rows {
        let (match_id, raw_detail_json) = row?;
        inventory.match_ids.insert(match_id.clone());
        let parsed_detail = raw_detail_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
        if parsed_detail
            .as_ref()
            .is_some_and(|detail| is_valid_match_detail_for(detail, &match_id))
        {
            inventory.detail_ids.insert(match_id);
        }
    }
    Ok(inventory)
}

pub fn select_detail_entries(
    entries: &[Value],
    inventory: &SeasonSyncInventory,
) -> DetailSelection {
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    let mut new_matches = 0;
    let mut missing_details = 0;

    for entry in entries {
        let Some(match_id) = entry.get("matchId").and_then(Value::as_str) else {
            continue;
        };
        if match_id.is_empty()
            || inventory.detail_ids.contains(match_id)
            || !seen.insert(match_id.to_string())
        {
            continue;
        }
        if inventory.match_ids.contains(match_id) {
            missing_details += 1;
        } else {
            new_matches += 1;
        }
        selected.push(entry.clone());
    }

    DetailSelection {
        entries: selected,
        new_matches,
        missing_details,
    }
}
