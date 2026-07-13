use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MARKER_KEY: &str = "privacy_sanitization_v1";
const MARKER_VALUE: &str = "complete";

/// Backs up and removes sensitive fields from legacy app-data payloads.
///
/// This deliberately limits its filesystem scope to the database and `seasons/`.
/// Credentials and the retired player-target file are neither read nor copied.
pub fn migrate_legacy_persistence(
    conn: &Connection,
    app_data_dir: &Path,
    database_path: &Path,
) -> Result<()> {
    if migration_complete(conn)? {
        return Ok(());
    }

    let backup_dir = app_data_dir
        .join("privacy-backups")
        .join(Utc::now().format("%Y%m%dT%H%M%S%.fZ").to_string());
    create_backup(app_data_dir, database_path, &backup_dir)?;
    sanitize_season_files(&app_data_dir.join("seasons"))?;
    sanitize_sqlite_rows(conn)?;

    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
        (MARKER_KEY, MARKER_VALUE, Utc::now().to_rfc3339()),
    )?;
    Ok(())
}

fn migration_complete(conn: &Connection) -> Result<bool> {
    let value = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [MARKER_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(value.as_deref() == Some(MARKER_VALUE))
}

fn create_backup(app_data_dir: &Path, database_path: &Path, backup_dir: &Path) -> Result<()> {
    fs::create_dir_all(backup_dir)?;
    if database_path.exists() {
        fs::copy(database_path, backup_dir.join("owstats.sqlite3"))
            .context("failed to back up the legacy database")?;
    }
    let seasons = app_data_dir.join("seasons");
    if seasons.exists() {
        copy_json_tree(&seasons, &backup_dir.join("seasons"))?;
    }
    Ok(())
}

fn copy_json_tree(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        return Err(anyhow!("legacy seasons path is not a directory"));
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            copy_json_tree(&path, &target)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            fs::create_dir_all(destination)?;
            fs::copy(&path, target)?;
        }
    }
    Ok(())
}

fn sanitize_season_files(seasons_dir: &Path) -> Result<()> {
    if !seasons_dir.exists() {
        return Ok(());
    }
    if !seasons_dir.is_dir() {
        return Err(anyhow!("legacy seasons path is not a directory"));
    }
    for entry in fs::read_dir(seasons_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            sanitize_season_files(&path)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            let text = fs::read_to_string(&path)?;
            let sanitized = match serde_json::from_str::<Value>(&text) {
                Ok(value) => {
                    serde_json::to_vec_pretty(&crate::redaction::sanitize_persisted_json(&value))?
                }
                Err(_) => crate::redaction::redact_sensitive_text(&text).into_bytes(),
            };
            write_atomically(&path, &sanitized)?;
        }
    }
    Ok(())
}

fn sanitize_sqlite_rows(conn: &Connection) -> Result<()> {
    let transaction = conn.unchecked_transaction()?;
    for (table, column) in [
        ("players", "raw_json"),
        ("matches", "raw_entry_json"),
        ("match_details", "raw_detail_json"),
        ("match_players", "raw_json"),
        ("hero_dict", "raw_json"),
        ("map_dict", "raw_json"),
        ("metric_dict", "raw_json"),
        ("sync_errors", "raw_json"),
        ("sync_errors", "message"),
    ] {
        sanitize_column(&transaction, table, column)?;
    }
    transaction.commit()?;
    Ok(())
}

fn sanitize_column(conn: &Connection, table: &str, column: &str) -> Result<()> {
    let sql = format!("SELECT rowid, {column} FROM {table} WHERE {column} IS NOT NULL");
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let update = format!("UPDATE {table} SET {column} = ?1 WHERE rowid = ?2");
    for (row_id, raw) in rows {
        let sanitized = match serde_json::from_str::<Value>(&raw) {
            Ok(value) => serde_json::to_string(&crate::redaction::sanitize_persisted_json(&value))?,
            Err(_) => crate::redaction::redact_sensitive_text(&raw),
        };
        conn.execute(&update, (&sanitized, row_id))?;
    }
    Ok(())
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = temporary_path(path);
    {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.privacy-tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::initialize_connection;
    use rusqlite::params;

    #[test]
    fn backs_up_and_sanitizes_legacy_json_and_sqlite_without_touching_target_file() {
        let temp = tempfile::tempdir().unwrap();
        let database_path = temp.path().join("owstats.sqlite3");
        let conn = Connection::open(&database_path).unwrap();
        initialize_connection(&conn).unwrap();
        conn.execute("DELETE FROM app_settings WHERE key = ?1", [MARKER_KEY])
            .unwrap();
        fs::create_dir_all(temp.path().join("seasons")).unwrap();
        fs::write(
            temp.path().join("seasons").join("22.json"),
            r#"{"safe":"ok","token":"legacy-json-secret"}"#,
        )
        .unwrap();
        let target = temp.path().join(".dashen_player_target.json");
        fs::write(&target, "legacy-target-must-remain").unwrap();
        conn.execute(
            "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, 22, 'normal', 'mode', 'normal', 'win', 'map', 'hero', 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, ?2, 'now')",
            params!["legacy-match", r#"{"customerToken":"legacy-sql-secret","safe":true}"#],
        ).unwrap();
        conn.execute(
            "INSERT INTO sync_errors (season, match_id, error_type, message, raw_json, created_at) VALUES (22, 'legacy-match', 'detail', ?1, ?2, 'now')",
            params!["token=legacy-message-secret", r#"{"authorization":"legacy-error-secret"}"#],
        ).unwrap();

        migrate_legacy_persistence(&conn, temp.path(), &database_path).unwrap();

        let season = fs::read_to_string(temp.path().join("seasons").join("22.json")).unwrap();
        let entry: String = conn
            .query_row("SELECT raw_entry_json FROM matches", [], |row| row.get(0))
            .unwrap();
        let error: String = conn
            .query_row("SELECT raw_json FROM sync_errors", [], |row| row.get(0))
            .unwrap();
        let message: String = conn
            .query_row("SELECT message FROM sync_errors", [], |row| row.get(0))
            .unwrap();
        for secret in [
            "legacy-json-secret",
            "legacy-sql-secret",
            "legacy-error-secret",
            "legacy-message-secret",
        ] {
            assert!(!season.contains(secret));
            assert!(!entry.contains(secret));
            assert!(!error.contains(secret));
            assert!(!message.contains(secret));
        }
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            "legacy-target-must-remain"
        );
        let backup_dirs = fs::read_dir(temp.path().join("privacy-backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(backup_dirs.len(), 1);
        assert!(backup_dirs[0].join("owstats.sqlite3").is_file());
        assert!(backup_dirs[0].join("seasons").join("22.json").is_file());
        assert!(migration_complete(&conn).unwrap());
        migrate_legacy_persistence(&conn, temp.path(), &database_path).unwrap();
        assert_eq!(
            fs::read_dir(temp.path().join("privacy-backups"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn failure_does_not_write_completion_marker() {
        let temp = tempfile::tempdir().unwrap();
        let database_path = temp.path().join("owstats.sqlite3");
        let conn = Connection::open(&database_path).unwrap();
        initialize_connection(&conn).unwrap();
        conn.execute("DELETE FROM app_settings WHERE key = ?1", [MARKER_KEY])
            .unwrap();
        fs::write(temp.path().join("seasons"), "not a directory").unwrap();

        assert!(migrate_legacy_persistence(&conn, temp.path(), &database_path).is_err());
        assert!(!migration_complete(&conn).unwrap());
    }

    #[test]
    fn atomic_write_replaces_file_without_temporary_residue() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("22.json");
        fs::write(&path, "old").unwrap();
        write_atomically(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert!(!temporary_path(&path).exists());
    }
}
