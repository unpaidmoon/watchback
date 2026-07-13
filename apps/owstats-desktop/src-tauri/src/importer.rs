use crate::db::OwDatabase;
use crate::match_evaluation::refresh_match_evaluations_conn;
use crate::models::ImportSummary;
use crate::redaction::{redact_sensitive_json, redact_sensitive_text, sanitize_persisted_json};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const PRESET_MODES: &[&str] = &[
    "LeisurePreset",
    "Leisure6v6",
    "SportPreset",
    "SportOpen",
    "leisure",
    "sport",
];

pub fn import_full_season_payload(
    db: &OwDatabase,
    path: impl AsRef<Path>,
) -> Result<ImportSummary> {
    db.initialize()?;
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read season payload {}", path.display()))?;
    let payload: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse season payload {}", path.display()))?;
    import_payload_value_embedded(db, &payload)
}

pub fn import_payload_value_embedded(db: &OwDatabase, payload: &Value) -> Result<ImportSummary> {
    import_payload_value_with_lookups(db, payload, load_embedded_lookup()?)
}

pub fn import_payload_value(
    db: &OwDatabase,
    payload: &Value,
    project_root: &Path,
) -> Result<ImportSummary> {
    import_payload_value_with_lookups(db, payload, load_lookup(project_root))
}

fn import_payload_value_with_lookups(
    db: &OwDatabase,
    payload: &Value,
    lookups: Lookups,
) -> Result<ImportSummary> {
    let sanitized_payload = sanitize_persisted_json(payload);
    let payload = &sanitized_payload;
    db.initialize()?;
    let conn = db.connect()?;
    let season = payload.get("season").and_then(Value::as_i64).unwrap_or(0);
    let matches_block = payload.get("matches").unwrap_or(&Value::Null);
    let entries = matches_block
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let details = matches_block
        .get("details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let errors = matches_block
        .get("detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let now = Utc::now().to_rfc3339();
    let details_by_id = details_by_match_id(&details);

    let tx = conn.unchecked_transaction()?;
    upsert_dictionary(&tx, "hero_dict", &lookups.heroes, &now)?;
    upsert_dictionary(&tx, "map_dict", &lookups.maps, &now)?;
    upsert_profile_card(&tx, payload, &now)?;

    for entry in &entries {
        let match_id = string_field(entry, "matchId")
            .or_else(|| string_field(entry, "match_id"))
            .unwrap_or_default();
        if match_id.is_empty() {
            continue;
        }
        let detail = details_by_id.get(&match_id);
        let mode = string_field(entry, "gameMode").unwrap_or_default();
        let match_kind = detail
            .and_then(|item| string_field(item, "match_kind"))
            .unwrap_or_default();
        let map_guid = string_field(entry, "mapGuid").unwrap_or_default();
        let hero_guid = string_field(entry, "heroGuid").unwrap_or_default();
        let map_name = lookups.maps.get(&map_guid).cloned();
        let hero_name = lookups.heroes.get(&hero_guid).cloned();
        tx.execute(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
                hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
                deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(match_id) DO UPDATE SET
                season=excluded.season,
                bucket=excluded.bucket,
                game_mode=excluded.game_mode,
                match_kind=excluded.match_kind,
                result=excluded.result,
                map_guid=excluded.map_guid,
                map_name=excluded.map_name,
                hero_guid=excluded.hero_guid,
                hero_name=excluded.hero_name,
                begin_ts=excluded.begin_ts,
                team_score=excluded.team_score,
                opponent_score=excluded.opponent_score,
                kills=excluded.kills,
                assists=excluded.assists,
                deaths=excluded.deaths,
                damage=excluded.damage,
                healing=excluded.healing,
                mitigation=excluded.mitigation,
                total_worth=excluded.total_worth,
                duration_sec=excluded.duration_sec,
                raw_entry_json=excluded.raw_entry_json,
                updated_at=excluded.updated_at
            "#,
            params![
                match_id,
                season.max(int_field(entry, "_dashenSeason")),
                bucket_for(&mode, &match_kind),
                mode,
                match_kind,
                result_label(int_field(entry, "matchRet")),
                map_guid,
                map_name,
                hero_guid,
                hero_name,
                normalize_timestamp(int_field(entry, "beginTs")),
                int_field(entry, "teamScore"),
                int_field(entry, "opponentScore"),
                int_field(entry, "kill"),
                int_field(entry, "assist"),
                int_field(entry, "death"),
                int_field(entry, "heroDamage"),
                int_field(entry, "cure"),
                int_field(entry, "resistDamage"),
                int_field(entry, "totalWorth"),
                duration_from_detail(detail),
                serde_json::to_string(entry)?,
                now,
            ],
        )?;
    }

    for detail in &details {
        let match_id = string_field(detail, "match_id").unwrap_or_default();
        if match_id.is_empty() {
            continue;
        }
        tx.execute(
            r#"
            INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(match_id) DO UPDATE SET
                match_kind=excluded.match_kind,
                raw_detail_json=excluded.raw_detail_json,
                updated_at=excluded.updated_at
            "#,
            params![
                match_id,
                string_field(detail, "match_kind").unwrap_or_default(),
                serde_json::to_string(detail)?,
                now,
            ],
        )?;
    }

    rebuild_match_players_for_season(&tx, season, &details, &lookups, Some(payload), &now)?;

    tx.execute("DELETE FROM sync_errors WHERE season = ?", [season])?;

    for error in &errors {
        tx.execute(
            r#"
            INSERT OR IGNORE INTO sync_errors (season, match_id, error_type, message, raw_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            params![
                season,
                string_field(error, "match_id"),
                string_field(error, "error_type"),
                string_field(error, "error")
                    .or_else(|| string_field(error, "message"))
                    .map(|message| redact_sensitive_text(&message)),
                serde_json::to_string(error)?,
                now,
            ],
        )?;
    }

    let latest_error_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sync_errors WHERE season = ?",
        [season],
        |row| row.get(0),
    )?;

    tx.execute(
        r#"
        INSERT INTO seasons (season, status, synced_at, matches_count, details_count, errors_count)
        VALUES (?, 'synced', ?, ?, ?, ?)
        ON CONFLICT(season) DO UPDATE SET
            status=excluded.status,
            synced_at=excluded.synced_at,
            matches_count=excluded.matches_count,
            details_count=excluded.details_count,
            errors_count=excluded.errors_count
        "#,
        params![
            season,
            now,
            entries.len() as i64,
            details.len() as i64,
            latest_error_count
        ],
    )?;

    let preset_matches: i64 = tx.query_row(
        "SELECT COUNT(*) FROM matches WHERE bucket = 'preset'",
        [],
        |row| row.get(0),
    )?;
    let stadium_matches: i64 = tx.query_row(
        "SELECT COUNT(*) FROM matches WHERE bucket = 'stadium'",
        [],
        |row| row.get(0),
    )?;
    let _ = refresh_match_evaluations_conn(&tx);
    tx.commit()?;

    let mut summary = ImportSummary {
        season,
        matches_seen: entries.len(),
        details_seen: details.len(),
        preset_matches,
        stadium_matches,
        imported_at: now,
        mmr_warning: None,
    };
    if crate::career_mmr::replay::rebuild_career_mmr(db).is_err() {
        summary.mmr_warning =
            Some("比赛已成功导入，但生涯分重建失败；已保留上一套完整结果。".to_string());
    }
    Ok(summary)
}

pub fn rebuild_match_players_from_existing_details(db: &OwDatabase) -> Result<()> {
    db.initialize()?;
    let conn = db.connect()?;
    let now = Utc::now().to_rfc3339();
    let tx = conn.unchecked_transaction()?;
    let lookups = load_lookup_from_db(&tx)?;
    let seasons = {
        let mut stmt = tx.prepare("SELECT DISTINCT season FROM matches ORDER BY season")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<i64>>>()?
    };
    for season in seasons {
        let details = {
            let mut stmt = tx.prepare(
                r#"
                SELECT d.raw_detail_json
                FROM match_details d
                JOIN matches m ON m.match_id = d.match_id
                WHERE m.season = ?
                "#,
            )?;
            let rows = stmt.query_map(params![season], |row| {
                let text: String = row.get(0)?;
                serde_json::from_str::<Value>(&text).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<Value>>>()?
        };
        rebuild_match_players_for_season(&tx, season, &details, &lookups, None, &now)?;
    }
    tx.commit()?;
    Ok(())
}

fn rebuild_match_players_for_season(
    tx: &rusqlite::Transaction<'_>,
    season: i64,
    details: &[Value],
    lookups: &Lookups,
    payload: Option<&Value>,
    now: &str,
) -> Result<()> {
    tx.execute(
        "DELETE FROM match_players WHERE season = ?",
        params![season],
    )?;
    tx.execute(
        "DELETE FROM arena_match_players WHERE season = ?",
        params![season],
    )?;
    let current_bnet = payload
        .and_then(|item| item.pointer("/profile_card/data/bnetId"))
        .and_then(value_to_string)
        .or_else(|| infer_current_bnet(details))
        .unwrap_or_default();

    for detail in details {
        let match_id = string_field(detail, "match_id").unwrap_or_default();
        if match_id.is_empty() {
            continue;
        }
        let bucket: Option<String> = tx
            .query_row(
                "SELECT bucket FROM matches WHERE match_id = ?",
                params![match_id],
                |row| row.get(0),
            )
            .ok();
        if bucket.as_deref() != Some("preset") {
            if string_field(detail, "match_kind").as_deref() == Some("fight") {
                let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
                insert_arena_player_index_rows(tx, &match_id, season, data, now)?;
            }
            continue;
        }
        if string_field(detail, "match_kind").as_deref() != Some("normal") {
            continue;
        }
        let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
        let teammate_rows = player_rows_for_side(data, "teammateList");
        let enemy_rows = player_rows_for_side(data, "enemyList");
        let friend_ids = current_player_friend_ids(teammate_rows, &current_bnet);
        insert_player_rows(
            tx,
            &match_id,
            season,
            "teammate",
            teammate_rows,
            &current_bnet,
            &friend_ids,
            lookups,
            now,
        )?;
        insert_player_rows(
            tx,
            &match_id,
            season,
            "enemy",
            enemy_rows,
            &current_bnet,
            &HashSet::new(),
            lookups,
            now,
        )?;
    }
    let key = format!("arena_player_index_v1:{season}");
    tx.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, 'complete', ?2) \
         ON CONFLICT(key) DO UPDATE SET value='complete', updated_at=excluded.updated_at",
        params![key, now],
    )?;
    Ok(())
}

fn insert_arena_player_index_rows(
    tx: &rusqlite::Transaction<'_>,
    match_id: &str,
    season: i64,
    data: &Value,
    now: &str,
) -> Result<()> {
    let name_map = arena_name_map(data);
    insert_arena_player_index_side(
        tx,
        match_id,
        season,
        "teammate",
        data.pointer("/totalCount/teammateList"),
        &name_map,
        now,
    )?;
    insert_arena_player_index_side(
        tx,
        match_id,
        season,
        "enemy",
        data.pointer("/totalCount/enemyList"),
        &name_map,
        now,
    )?;

    if let Some(rounds) = data.get("roundCountList").and_then(Value::as_array) {
        for round in rounds {
            insert_arena_player_index_side(
                tx,
                match_id,
                season,
                "teammate",
                round.get("teammateList"),
                &name_map,
                now,
            )?;
            insert_arena_player_index_side(
                tx,
                match_id,
                season,
                "enemy",
                round.get("enemyList"),
                &name_map,
                now,
            )?;
        }
    }

    Ok(())
}

fn insert_arena_player_index_side(
    tx: &rusqlite::Transaction<'_>,
    match_id: &str,
    season: i64,
    side: &str,
    value: Option<&Value>,
    name_map: &HashMap<String, String>,
    now: &str,
) -> Result<()> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(());
    };
    for item in items {
        if !item.is_object() {
            continue;
        }
        let bnet_id = string_field(item, "bnetId").unwrap_or_default();
        if bnet_id.is_empty() {
            continue;
        }
        tx.execute(
            r#"
            INSERT INTO arena_match_players (match_id, season, side, bnet_id, player_name, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(match_id, side, bnet_id) DO UPDATE SET
                season=excluded.season,
                player_name=excluded.player_name,
                updated_at=excluded.updated_at
            "#,
            params![
                match_id,
                season,
                side,
                bnet_id,
                arena_player_name(item, name_map),
                now
            ],
        )?;
    }
    Ok(())
}

fn arena_name_map(data: &Value) -> HashMap<String, String> {
    data.get("nameMap")
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(key, value)| {
                    let name = value.as_str()?.trim();
                    if key.trim().is_empty() || name.is_empty() {
                        None
                    } else {
                        Some((key.to_string(), name.to_string()))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn arena_player_name(item: &Value, name_map: &HashMap<String, String>) -> String {
    let bnet_id = string_field(item, "bnetId").unwrap_or_default();
    string_field(item, "name")
        .filter(|name| !looks_like_private_id(name))
        .or_else(|| {
            name_map
                .get(&bnet_id)
                .filter(|name| !looks_like_private_id(name))
                .cloned()
        })
        .unwrap_or_else(|| "未知玩家".to_string())
}

fn looks_like_private_id(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return !trimmed.is_empty();
    }
    let compact = trimmed.chars().filter(|ch| *ch != '-').collect::<String>();
    compact.len() == 32 && compact.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn player_rows_for_side<'a>(data: &'a Value, list_name: &str) -> Option<&'a Value> {
    data.pointer(&format!("/totalCount/{list_name}"))
        .filter(|value| {
            value
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        })
        .or_else(|| data.get(list_name))
}

fn upsert_profile_card(tx: &rusqlite::Transaction<'_>, payload: &Value, now: &str) -> Result<()> {
    let Some(profile) = payload
        .pointer("/profile_card/data")
        .filter(|value| value.is_object())
    else {
        return Ok(());
    };
    let bnet_id =
        value_to_string(profile.get("bnetId").unwrap_or(&Value::Null)).unwrap_or_default();
    if bnet_id.is_empty() {
        return Ok(());
    }
    let display_name = string_field(profile, "name").unwrap_or_else(|| bnet_id.clone());
    let redacted = redact_sensitive_json(profile);
    tx.execute(
        r#"
        INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(player_id) DO UPDATE SET
            bnet_id=excluded.bnet_id,
            display_name=excluded.display_name,
            raw_json=excluded.raw_json,
            updated_at=excluded.updated_at
        "#,
        params![
            bnet_id,
            bnet_id,
            display_name,
            serde_json::to_string(&redacted)?,
            now
        ],
    )?;
    Ok(())
}

fn current_player_friend_ids(value: Option<&Value>, current_bnet: &str) -> HashSet<String> {
    value
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                string_field(item, "bnetId")
                    .map(|bnet_id| bnet_id == current_bnet)
                    .unwrap_or(false)
            })
        })
        .and_then(|item| item.get("friendBnetIds"))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(value_to_string).collect())
        .unwrap_or_default()
}

fn infer_current_bnet(details: &[Value]) -> Option<String> {
    let mut counts = HashMap::<String, i64>::new();
    for detail in details {
        if string_field(detail, "match_kind").as_deref() != Some("normal") {
            continue;
        }
        let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
        let Some(items) = player_rows_for_side(data, "teammateList").and_then(Value::as_array)
        else {
            continue;
        };
        for item in items {
            if let Some(bnet_id) = string_field(item, "bnetId") {
                *counts.entry(bnet_id).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .max_by(|(left_id, left_count), (right_id, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_id.cmp(left_id))
        })
        .map(|(bnet_id, _)| bnet_id)
}

#[allow(clippy::too_many_arguments)] // SQL row mapping preserves the importer call boundary.
fn insert_player_rows(
    tx: &rusqlite::Transaction<'_>,
    match_id: &str,
    season: i64,
    side: &str,
    value: Option<&Value>,
    current_bnet: &str,
    friend_ids: &HashSet<String>,
    lookups: &Lookups,
    now: &str,
) -> Result<()> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(());
    };
    for item in items {
        if !item.is_object() {
            continue;
        }
        let bnet_id = string_field(item, "bnetId").unwrap_or_default();
        if bnet_id.is_empty() {
            continue;
        }
        let hero_guid = string_field(item, "heroGuid").unwrap_or_default();
        let role_type = string_field(item, "roleType");
        let is_current_player = !current_bnet.is_empty() && bnet_id == current_bnet;
        let is_friend = side == "teammate" && !is_current_player && friend_ids.contains(&bnet_id);
        tx.execute(
            r#"
            INSERT INTO match_players (
                match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
                is_current_player, is_friend, kill, assist, death, hero_damage, cure,
                resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
                role_type, raw_json, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(match_id, side, bnet_id) DO UPDATE SET
                season=excluded.season,
                player_name=excluded.player_name,
                hero_guid=excluded.hero_guid,
                hero_name=excluded.hero_name,
                is_current_player=excluded.is_current_player,
                is_friend=excluded.is_friend,
                kill=excluded.kill,
                assist=excluded.assist,
                death=excluded.death,
                hero_damage=excluded.hero_damage,
                cure=excluded.cure,
                resist_damage=excluded.resist_damage,
                healing_taken=excluded.healing_taken,
                damage_taken=excluded.damage_taken,
                final_hit=excluded.final_hit,
                target_competing_time=excluded.target_competing_time,
                role_type=excluded.role_type,
                raw_json=excluded.raw_json,
                updated_at=excluded.updated_at
            "#,
            params![
                match_id,
                season,
                side,
                bnet_id,
                string_field(item, "name").unwrap_or_else(|| bnet_id.clone()),
                hero_guid,
                lookups.heroes.get(&hero_guid).cloned(),
                if is_current_player { 1 } else { 0 },
                if is_friend { 1 } else { 0 },
                int_field(item, "kill"),
                int_field(item, "assist"),
                int_field(item, "death"),
                int_field(item, "heroDamage"),
                int_field(item, "cure"),
                int_field(item, "resistDamage"),
                int_field(item, "healingTaken"),
                int_field(item, "damageTaken"),
                int_field(item, "finalHit"),
                int_field(item, "targetCompetingTime"),
                role_type,
                serde_json::to_string(&strip_sensitive_json(item))?,
                now,
            ],
        )?;
    }
    Ok(())
}

fn bucket_for(mode: &str, match_kind: &str) -> &'static str {
    if match_kind == "normal" || PRESET_MODES.contains(&mode) {
        "preset"
    } else if match_kind == "fight" || mode.ends_with("Fight") {
        "stadium"
    } else {
        "preset"
    }
}

fn result_label(value: i64) -> &'static str {
    match value {
        1 => "win",
        -1 | 2 => "loss",
        _ => "draw",
    }
}

fn details_by_match_id(details: &[Value]) -> HashMap<String, &Value> {
    details
        .iter()
        .filter_map(|item| string_field(item, "match_id").map(|id| (id, item)))
        .collect()
}

fn duration_from_detail(detail: Option<&&Value>) -> i64 {
    let Some(detail) = detail else {
        return 0;
    };
    let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
    data.get("gameTimeSec")
        .and_then(Value::as_i64)
        .or_else(|| {
            data.pointer("/totalCount/gameTimeSec")
                .and_then(Value::as_i64)
        })
        .unwrap_or(0)
}

pub(crate) fn string_field(value: &Value, key: &str) -> Option<String> {
    let item = value.get(key)?;
    if let Some(text) = item.as_str() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    } else if item.is_number() {
        Some(item.to_string())
    } else {
        None
    }
}

pub(crate) fn int_field(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_f64().map(|value| value.round() as i64))
                .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
        })
        .unwrap_or(0)
}

fn normalize_timestamp(value: i64) -> i64 {
    if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    }
}

struct Lookups {
    heroes: HashMap<String, String>,
    maps: HashMap<String, String>,
    #[allow(dead_code)]
    attrs: HashMap<String, String>,
}

fn load_lookup(project_root: &Path) -> Lookups {
    let path = project_root.join("data").join("ow_config_cache.json");
    let parsed = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    parsed
        .as_ref()
        .map(parse_lookups)
        .unwrap_or_else(|| load_embedded_lookup().expect("embedded runtime lookup must be valid"))
}

fn load_embedded_lookup() -> Result<Lookups> {
    let parsed: Value = serde_json::from_str(include_str!("../../runtime-config/ow-lookups.json"))
        .context("failed to parse embedded runtime lookup")?;
    Ok(parse_lookups(&parsed))
}

fn parse_lookups(parsed: &Value) -> Lookups {
    Lookups {
        heroes: dict_names(parsed.get("heroes")),
        maps: dict_names(parsed.get("maps")),
        attrs: attr_names(parsed.get("attrs")),
    }
}

fn load_lookup_from_db(tx: &rusqlite::Transaction<'_>) -> Result<Lookups> {
    Ok(Lookups {
        heroes: load_dictionary_from_db(tx, "hero_dict")?,
        maps: load_dictionary_from_db(tx, "map_dict")?,
        attrs: HashMap::new(),
    })
}

fn attr_names(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(guid, item)| {
                    string_field(item, "text").map(|text| (guid.clone(), text))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn load_dictionary_from_db(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
) -> Result<HashMap<String, String>> {
    let sql = format!("SELECT guid, name FROM {table}");
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(Into::into)
}

fn dict_names(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(guid, item)| {
                    string_field(item, "name").map(|name| (guid.clone(), name))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn value_to_string(item: &Value) -> Option<String> {
    if let Some(text) = item.as_str() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    } else if item.is_number() {
        Some(item.to_string())
    } else {
        None
    }
}

fn strip_sensitive_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| !is_sensitive_key(key))
                .map(|(key, value)| (key.clone(), strip_sensitive_json(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(strip_sensitive_json).collect()),
        _ => value.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered.contains("token") || lowered.contains("cookie")
}

#[allow(clippy::items_after_test_module)] // Public importer helpers intentionally follow lookup unit tests.
#[cfg(test)]
mod portable_lookup_tests {
    use super::load_embedded_lookup;

    #[test]
    fn embedded_lookup_resolves_known_hero_map_and_attribute_without_project_root() {
        let lookup = load_embedded_lookup().unwrap();
        assert_eq!(
            lookup.heroes.get("207165582859043131").map(String::as_str),
            Some("安娜")
        );
        assert_eq!(
            lookup.maps.get("576460752303425122").map(String::as_str),
            Some("漓江塔")
        );
        assert_eq!(
            lookup.attrs.get("603482350067646509").map(String::as_str),
            Some("单独消灭")
        );
    }
}

fn upsert_dictionary(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    values: &HashMap<String, String>,
    now: &str,
) -> Result<()> {
    let sql = format!(
        "INSERT INTO {table} (guid, name, raw_json, updated_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT(guid) DO UPDATE SET name=excluded.name, raw_json=excluded.raw_json, updated_at=excluded.updated_at"
    );
    for (guid, name) in values {
        let raw = serde_json::json!({ "guid": guid, "name": name });
        tx.execute(&sql, params![guid, name, raw.to_string(), now])?;
    }
    Ok(())
}

pub fn existing_import_summary(db: &OwDatabase) -> Result<Option<ImportSummary>> {
    db.initialize()?;
    let conn = db.connect()?;
    let row = conn
        .query_row(
            r#"
            SELECT
                season,
                matches_count,
                details_count,
                synced_at,
                (SELECT COUNT(*) FROM matches WHERE bucket='preset') AS preset_matches,
                (SELECT COUNT(*) FROM matches WHERE bucket='stadium') AS stadium_matches
            FROM seasons
            ORDER BY season DESC
            LIMIT 1
            "#,
            [],
            |row| {
                Ok(ImportSummary {
                    season: row.get(0)?,
                    matches_seen: row.get::<_, i64>(1)? as usize,
                    details_seen: row.get::<_, i64>(2)? as usize,
                    imported_at: row.get(3)?,
                    preset_matches: row.get(4)?,
                    stadium_matches: row.get(5)?,
                    mmr_warning: None,
                })
            },
        )
        .optional()?;
    Ok(row)
}
