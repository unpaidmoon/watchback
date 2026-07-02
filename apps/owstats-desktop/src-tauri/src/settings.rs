use crate::db::OwDatabase;
use crate::models::{PlayerProfile, UpdateCheckResult, UserSettings};
use crate::redaction::redact_sensitive_json;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

pub fn get_player_profile_for_path(path: &Path, role_id: Option<i64>) -> Result<PlayerProfile> {
    let mut profile = read_json::<PlayerProfile>(path)?.unwrap_or(PlayerProfile {
        role_id,
        player_id: role_id.map(|id| format!("角色 {id}")),
        avatar_url: None,
        endorsement_level: None,
        play_time_hours: None,
        title: None,
        refreshed_at: None,
    });
    if profile.role_id.is_none() {
        profile.role_id = role_id;
    }
    if profile.player_id.is_none() {
        profile.player_id = profile.role_id.map(|id| format!("角色 {id}"));
    }
    Ok(profile)
}

pub fn get_player_profile_with_project_root(
    db: &OwDatabase,
    path: &Path,
    role_id: Option<i64>,
    project_root: &Path,
) -> Result<PlayerProfile> {
    if let Some(profile) = get_player_profile_from_db(db, role_id)? {
        return Ok(profile);
    }
    if let Some((profile, raw)) = get_player_profile_from_project_payloads(project_root, role_id)? {
        upsert_player_profile(db, &profile, &raw)?;
        return Ok(profile);
    }

    get_player_profile_for_path(path, role_id)
}

fn get_player_profile_from_db(
    db: &OwDatabase,
    role_id: Option<i64>,
) -> Result<Option<PlayerProfile>> {
    db.initialize()?;
    let conn = db.connect()?;
    let row = if let Some(role_id) = role_id {
        conn.query_row(
            "SELECT bnet_id, display_name, raw_json, updated_at FROM players WHERE bnet_id = ? ORDER BY updated_at DESC LIMIT 1",
            params![role_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
    } else {
        conn.query_row(
            "SELECT bnet_id, display_name, raw_json, updated_at FROM players ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
    };

    let Some((bnet_id, display_name, raw_json, updated_at)) = row else {
        return Ok(None);
    };
    let raw: Value = serde_json::from_str(&raw_json)?;
    Ok(Some(profile_from_raw(
        &raw,
        role_id,
        bnet_id,
        display_name,
        Some(updated_at),
    )))
}

fn get_player_profile_from_project_payloads(
    project_root: &Path,
    role_id: Option<i64>,
) -> Result<Option<(PlayerProfile, Value)>> {
    let player_dir = project_root.join("data").join("players");
    if !player_dir.exists() {
        return Ok(None);
    }
    let role_text = role_id.map(|id| id.to_string());
    for entry in std::fs::read_dir(player_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|item| item.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let payload: Value = serde_json::from_str(&text)?;
        let Some(raw) = payload
            .pointer("/profile_card/data")
            .filter(|value| value.is_object())
        else {
            continue;
        };
        let bnet_id = json_i64(raw, "bnetId").map(|id| id.to_string());
        if role_text.as_deref().is_some() && bnet_id.as_deref() != role_text.as_deref() {
            continue;
        }
        let profile = profile_from_raw(
            raw,
            role_id,
            bnet_id.clone(),
            json_string(raw, "name"),
            None,
        );
        return Ok(Some((profile, raw.clone())));
    }
    Ok(None)
}

fn upsert_player_profile(db: &OwDatabase, profile: &PlayerProfile, raw: &Value) -> Result<()> {
    let Some(role_id) = profile.role_id else {
        return Ok(());
    };
    db.initialize()?;
    let conn = db.connect()?;
    let bnet_id = role_id.to_string();
    let display_name = profile.player_id.clone().unwrap_or_else(|| bnet_id.clone());
    let redacted = redact_sensitive_json(raw);
    conn.execute(
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
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

fn profile_from_raw(
    raw: &Value,
    role_id: Option<i64>,
    bnet_id: Option<String>,
    display_name: Option<String>,
    refreshed_at: Option<String>,
) -> PlayerProfile {
    let role_id = role_id.or_else(|| {
        bnet_id
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok())
            .or_else(|| json_i64(&raw, "bnetId"))
    });
    let player_id = json_string(&raw, "name")
        .or(display_name)
        .or(bnet_id.clone());

    PlayerProfile {
        role_id,
        player_id,
        avatar_url: json_string(&raw, "icon")
            .or_else(|| json_string(&raw, "headIcon"))
            .or_else(|| json_string(&raw, "avatar"))
            .or_else(|| json_string(&raw, "avatarUrl")),
        endorsement_level: json_i64(&raw, "level").or_else(|| json_i64(&raw, "endorsementLevel")),
        play_time_hours: json_f64(&raw, "gameTime").or_else(|| json_f64(&raw, "playTimeHours")),
        title: json_string(&raw, "title"),
        refreshed_at,
    }
}

pub fn get_user_settings_for_path(path: &Path) -> Result<UserSettings> {
    Ok(read_json::<UserSettings>(path)?.unwrap_or_default())
}

pub fn save_user_settings_for_path(path: &Path, settings: UserSettings) -> Result<UserSettings> {
    let normalized = UserSettings {
        theme: normalize_theme(&settings.theme).to_string(),
    };
    write_json(path, &normalized)?;
    Ok(normalized)
}

pub fn check_app_update_result(current_version: &str) -> UpdateCheckResult {
    UpdateCheckResult {
        status: "notConfigured".to_string(),
        current_version: current_version.to_string(),
        latest_version: None,
        message: "版本检查接口已预留，更新源尚未接入。".to_string(),
    }
}

fn normalize_theme(theme: &str) -> &str {
    match theme {
        "light" => "light",
        "dark" => "dark",
        _ => "system",
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&text)?))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text)?;
    Ok(())
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
}

fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|item| {
        item.as_i64()
            .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
    })
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|item| {
        item.as_f64()
            .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_to_system_theme_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        let settings = get_user_settings_for_path(&temp.path().join("settings.json")).unwrap();

        assert_eq!(settings.theme, "system");
    }

    #[test]
    fn settings_normalize_unknown_theme_to_system() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");

        let saved = save_user_settings_for_path(
            &path,
            UserSettings {
                theme: "solarized".to_string(),
            },
        )
        .unwrap();

        assert_eq!(saved.theme, "system");
        assert_eq!(get_user_settings_for_path(&path).unwrap().theme, "system");
    }

    #[test]
    fn profile_prefers_database_profile_card_fields() {
        let temp = tempfile::tempdir().unwrap();
        let db = OwDatabase::new(temp.path().join("owstats.db"));
        db.initialize().unwrap();
        let conn = db.connect().unwrap();
        conn.execute(
            r#"
            INSERT INTO players (player_id, bnet_id, display_name, raw_json, updated_at)
            VALUES ('595945364', '595945364', '风偏偏呢#5479', ?, '2026-07-01T00:00:00Z')
            "#,
            [r#"{
              "bnetId": 595945364,
              "name": "风偏偏呢#5479",
              "icon": "https://example.test/avatar.png",
              "title": "┻━┻ ︵ \\(°□°)/ ︵ ┻━┻",
              "level": 4,
              "gameTime": "1031.5"
            }"#],
        )
        .unwrap();

        let profile = get_player_profile_with_project_root(
            &db,
            &temp.path().join("profile.json"),
            Some(595945364),
            temp.path(),
        )
        .unwrap();

        assert_eq!(profile.role_id, Some(595945364));
        assert_eq!(profile.player_id.as_deref(), Some("风偏偏呢#5479"));
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://example.test/avatar.png")
        );
        assert_eq!(profile.title.as_deref(), Some("┻━┻ ︵ \\(°□°)/ ︵ ┻━┻"));
        assert_eq!(profile.endorsement_level, Some(4));
        assert_eq!(profile.play_time_hours, Some(1031.5));
    }

    #[test]
    fn profile_can_fallback_to_project_player_payloads() {
        let temp = tempfile::tempdir().unwrap();
        let db = OwDatabase::new(temp.path().join("owstats.db"));
        db.initialize().unwrap();
        let project_root = temp.path().join("project");
        let player_dir = project_root.join("data").join("players");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(
            player_dir.join("5479_s22_full_season.json"),
            r#"{
              "profile_card": {
                "data": {
                  "bnetId": 595945364,
                  "name": "风偏偏呢#5479",
                  "icon": "https://example.test/avatar.png",
                  "title": "桌面清空者",
                  "level": 4,
                  "gameTime": "1031.5"
                }
              }
            }"#,
        )
        .unwrap();

        let profile = get_player_profile_with_project_root(
            &db,
            &temp.path().join("profile.json"),
            Some(595945364),
            &project_root,
        )
        .unwrap();

        assert_eq!(profile.player_id.as_deref(), Some("风偏偏呢#5479"));
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://example.test/avatar.png")
        );
        assert_eq!(profile.title.as_deref(), Some("桌面清空者"));
        assert_eq!(profile.endorsement_level, Some(4));
        assert_eq!(profile.play_time_hours, Some(1031.5));

        let conn = db.connect().unwrap();
        let saved: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM players WHERE bnet_id = '595945364'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(saved, 1);
    }
}
