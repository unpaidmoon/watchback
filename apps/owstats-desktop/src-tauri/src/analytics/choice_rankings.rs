use super::round2;
use crate::db::OwDatabase;
use crate::models::{ChoiceRankingEntry, ChoiceRankings};
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

const MIN_SAMPLE: i64 = 5;
const CONFIDENCE_LEVEL: f64 = 0.95;
const WILSON_Z: f64 = 1.959_963_984_540_054;

#[derive(Default)]
struct Counts {
    matches: i64,
    wins: i64,
}

#[derive(Default)]
struct StadiumLoadout {
    traits: HashSet<String>,
    mods: HashSet<String>,
}

pub(super) fn choice_rankings_for_season(
    db: &OwDatabase,
    season: Option<i64>,
) -> Result<ChoiceRankings> {
    let Some(season) = season else {
        return Ok(empty_rankings());
    };
    let conn = db.connect()?;
    let mut statement = conn.prepare(
        r#"
        SELECT m.match_id, m.bucket, m.result, d.raw_detail_json
        FROM matches m
        JOIN match_details d ON d.match_id=m.match_id
        WHERE m.season=?1 AND m.bucket IN ('preset', 'stadium')
        ORDER BY m.match_id
        "#,
    )?;
    let rows = statement.query_map(params![season], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut perks = HashMap::new();
    let mut traits = HashMap::new();
    let mut mods = HashMap::new();
    for row in rows {
        let (match_id, bucket, result, raw) = row?;
        let Ok(detail) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let won = result == "win";
        if bucket == "preset" {
            let current_bnet = conn
                .query_row(
                    "SELECT bnet_id FROM match_players
                     WHERE match_id=?1 AND is_current_player=1 LIMIT 1",
                    params![match_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(current_bnet) = current_bnet {
                add_once(
                    &mut perks,
                    extract_current_perks(&detail, &current_bnet),
                    won,
                );
            }
        } else {
            let loadout = extract_current_stadium_loadout(&detail);
            add_once(&mut traits, loadout.traits, won);
            add_once(&mut mods, loadout.mods, won);
        }
    }

    Ok(ChoiceRankings {
        min_sample: MIN_SAMPLE,
        confidence_level: CONFIDENCE_LEVEL,
        normal_perks: ranked(perks),
        stadium_traits: ranked(traits),
        stadium_mods: ranked(mods),
    })
}

fn empty_rankings() -> ChoiceRankings {
    ChoiceRankings {
        min_sample: MIN_SAMPLE,
        confidence_level: CONFIDENCE_LEVEL,
        normal_perks: Vec::new(),
        stadium_traits: Vec::new(),
        stadium_mods: Vec::new(),
    }
}

fn extract_current_perks(detail: &Value, current_bnet: &str) -> HashSet<String> {
    let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
    let players = data
        .pointer("/totalCount/teammateList")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .or_else(|| data.get("teammateList").and_then(Value::as_array));
    players
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("bnetId")
                    .and_then(value_string)
                    .is_some_and(|value| value == current_bnet)
            })
        })
        .and_then(|player| player.get("perks"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|perk| perk.get("guid").and_then(value_string))
        .collect()
}

fn extract_current_stadium_loadout(detail: &Value) -> StadiumLoadout {
    let player = detail
        .pointer("/payload/data/roundCountList")
        .and_then(Value::as_array)
        .and_then(|rounds| rounds.iter().rev().find(|round| round.is_object()))
        .and_then(|round| round.get("teammateList"))
        .and_then(Value::as_array)
        .and_then(|players| players.first());
    StadiumLoadout {
        traits: string_set(player.and_then(|item| item.get("traitGuids"))),
        mods: string_set(player.and_then(|item| item.get("modGuids"))),
    }
}

fn string_set(value: Option<&Value>) -> HashSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(value_string)
        .collect()
}

fn value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn add_once(target: &mut HashMap<String, Counts>, keys: HashSet<String>, won: bool) {
    for key in keys {
        let counts = target.entry(key).or_default();
        counts.matches += 1;
        counts.wins += i64::from(won);
    }
}

fn ranked(counts: HashMap<String, Counts>) -> Vec<ChoiceRankingEntry> {
    let mut entries = counts
        .into_iter()
        .filter(|(_, counts)| counts.matches >= MIN_SAMPLE)
        .map(|(key, counts)| ChoiceRankingEntry {
            key,
            matches: counts.matches,
            wins: counts.wins,
            win_rate: round2(counts.wins as f64 / counts.matches as f64),
        })
        .collect::<Vec<_>>();
    entries.sort_by(compare_entries);
    entries.truncate(3);
    entries
}

fn compare_entries(left: &ChoiceRankingEntry, right: &ChoiceRankingEntry) -> Ordering {
    wilson_lower(right.wins, right.matches)
        .partial_cmp(&wilson_lower(left.wins, left.matches))
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.matches.cmp(&left.matches))
        .then_with(|| {
            right
                .win_rate
                .partial_cmp(&left.win_rate)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| left.key.cmp(&right.key))
}

fn wilson_lower(wins: i64, matches: i64) -> f64 {
    if matches <= 0 {
        return 0.0;
    }
    let n = matches as f64;
    let rate = wins as f64 / n;
    let z2 = WILSON_Z * WILSON_Z;
    (rate + z2 / (2.0 * n)
        - WILSON_Z * ((rate * (1.0 - rate) + z2 / (4.0 * n)) / n).sqrt())
        / (1.0 + z2 / n)
}
