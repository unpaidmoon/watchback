use crate::db::OwDatabase;
use crate::importer::is_valid_match_detail_for;
use crate::models::SyncPageDiagnostic;
use anyhow::Result;
use rusqlite::params;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SeasonModeKey {
    pub match_kind: String,
    pub game_mode: String,
}

impl SeasonModeKey {
    pub fn new(match_kind: impl Into<String>, game_mode: impl Into<String>) -> Self {
        Self {
            match_kind: match_kind.into(),
            game_mode: game_mode.into(),
        }
    }
}

fn normalized_season_mode_key(match_kind: &str, game_mode: &str) -> Option<SeasonModeKey> {
    let game_mode = game_mode.trim();
    let (normalized_kind, canonical_mode) = match game_mode {
        "sport" | "SportPreset" | "SportOpen" => ("normal", "sport"),
        "leisure" | "LeisurePreset" | "Leisure6v6" => ("normal", "leisure"),
        mode if super::FIGHT_MODES.contains(&mode) => ("fight", mode),
        _ => return None,
    };
    let explicit_kind = match_kind.trim();
    if !explicit_kind.is_empty() && explicit_kind != normalized_kind {
        return None;
    }
    Some(SeasonModeKey::new(normalized_kind, canonical_mode))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModeSyncInventory {
    pub match_ids: HashSet<String>,
    pub detail_ids: HashSet<String>,
    pub latest_begin_ts: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonSyncInventory {
    pub match_ids: HashSet<String>,
    pub detail_ids: HashSet<String>,
    pub modes: HashMap<SeasonModeKey, ModeSyncInventory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickPageDecision {
    Empty,
    WatermarkReached,
    Continue,
    Unsafe,
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
                d.raw_detail_json,
                m.match_kind,
                m.game_mode,
                m.begin_ts
         FROM matches m
         LEFT JOIN match_details d ON d.match_id = m.match_id
         WHERE m.season = ?1",
    )?;
    let rows = statement.query_map(params![season], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;
    let mut inventory = SeasonSyncInventory::default();
    for row in rows {
        let (match_id, raw_detail_json, match_kind, game_mode, begin_ts) = row?;
        inventory.match_ids.insert(match_id.clone());
        let parsed_detail = raw_detail_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
        let has_valid_detail = parsed_detail
            .as_ref()
            .is_some_and(|detail| is_valid_match_detail_for(detail, &match_id));
        if has_valid_detail {
            inventory.detail_ids.insert(match_id.clone());
        }
        if let Some(mode_key) = normalized_season_mode_key(&match_kind, &game_mode) {
            let mode = inventory.modes.entry(mode_key).or_default();
            mode.match_ids.insert(match_id.clone());
            if has_valid_detail {
                mode.detail_ids.insert(match_id);
            }
            if begin_ts > 0 {
                mode.latest_begin_ts = Some(
                    mode.latest_begin_ts
                        .map_or(begin_ts, |latest| latest.max(begin_ts)),
                );
            }
        }
    }
    Ok(inventory)
}

pub fn classify_quick_page(
    raw_payload: &Value,
    known_match_ids: &HashSet<String>,
    watermark: Option<i64>,
) -> QuickPageDecision {
    if raw_payload.get("code").is_some_and(|code| {
        code.as_i64() != Some(0) && code.as_u64() != Some(0) && code.as_f64() != Some(0.0)
    }) {
        return QuickPageDecision::Unsafe;
    }
    let Some(raw_entries) =
        crate::season_fetch::extract_raw_match_entries_if_recognized(raw_payload)
    else {
        return QuickPageDecision::Unsafe;
    };
    if raw_entries.is_empty() {
        return QuickPageDecision::Empty;
    }
    let Some(watermark) = watermark else {
        return QuickPageDecision::Unsafe;
    };
    let mut should_continue = false;
    for entry in &raw_entries {
        let Some(match_id) = entry
            .get("matchId")
            .and_then(Value::as_str)
            .filter(|match_id| !match_id.trim().is_empty())
        else {
            return QuickPageDecision::Unsafe;
        };
        let Some(begin_ts) = entry
            .get("beginTs")
            .and_then(Value::as_i64)
            .filter(|begin_ts| *begin_ts > 0)
        else {
            return QuickPageDecision::Unsafe;
        };
        let begin_ts = crate::importer::normalize_timestamp(begin_ts);
        should_continue |= !known_match_ids.contains(match_id) || begin_ts > watermark;
    }
    if should_continue {
        QuickPageDecision::Continue
    } else {
        QuickPageDecision::WatermarkReached
    }
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

pub fn build_page_diagnostics(
    list_pages: &[Value],
    inventory: &SeasonSyncInventory,
) -> Vec<SyncPageDiagnostic> {
    let mut diagnostics = list_pages
        .iter()
        .map(|list_page| {
            let raw_entries = list_page
                .get("payload")
                .map(crate::season_fetch::extract_raw_match_entries)
                .unwrap_or_default();
            let valid_entries = list_page
                .get("payload")
                .map(crate::season_fetch::extract_match_entries)
                .unwrap_or_default();
            let mut seen = HashSet::new();
            let mut new_matches = 0;
            let mut missing_details = 0;
            let mut existing_complete_matches = 0;
            let mut begin_timestamps = raw_entries
                .iter()
                .filter_map(|entry| entry.get("beginTs").and_then(Value::as_i64))
                .collect::<Vec<_>>();
            for entry in &valid_entries {
                let Some(match_id) = entry
                    .get("matchId")
                    .and_then(Value::as_str)
                    .filter(|match_id| !match_id.trim().is_empty())
                else {
                    continue;
                };
                if !seen.insert(match_id) {
                    continue;
                }
                if !inventory.match_ids.contains(match_id) {
                    new_matches += 1;
                } else if !inventory.detail_ids.contains(match_id) {
                    missing_details += 1;
                } else {
                    existing_complete_matches += 1;
                }
            }
            begin_timestamps.sort_unstable();
            SyncPageDiagnostic {
                match_kind: list_page
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("normal")
                    .to_string(),
                game_mode: list_page
                    .get("gameMode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                page: list_page
                    .get("page")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                entries: raw_entries.len(),
                unique_valid_entries: seen.len(),
                new_matches,
                missing_details,
                existing_complete_matches,
                earliest_begin_ts: begin_timestamps.first().copied(),
                latest_begin_ts: begin_timestamps.last().copied(),
                stop_reason: None,
                watermark_reached: false,
            }
        })
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        (&left.match_kind, &left.game_mode, left.page).cmp(&(
            &right.match_kind,
            &right.game_mode,
            right.page,
        ))
    });
    diagnostics
}

pub(super) fn query_season_database_totals(db: &OwDatabase, season: i64) -> Result<(usize, usize)> {
    db.initialize()?;
    let conn = db.connect()?;
    let matches = conn.query_row(
        "SELECT COUNT(*) FROM matches WHERE season = ?1",
        params![season],
        |row| row.get::<_, i64>(0),
    )?;
    let details = crate::importer::count_valid_match_details_for_season(&conn, season)?;
    Ok((matches.max(0) as usize, details.max(0) as usize))
}
