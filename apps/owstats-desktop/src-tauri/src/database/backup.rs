use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

pub fn backup_before_schema_migration(
    conn: &Connection,
    database_path: &Path,
    backup_dir: &Path,
    target_version: i64,
) -> Result<Option<PathBuf>> {
    if !database_path.is_file() || fs::metadata(database_path)?.len() == 0 {
        return Ok(None);
    }
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current >= target_version {
        return Ok(None);
    }

    fs::create_dir_all(backup_dir).context("failed to create database backup directory")?;
    let name = format!(
        "owstats-v{current}-to-v{target_version}-{}.sqlite3",
        Utc::now().format("%Y%m%dT%H%M%S%.fZ")
    );
    let destination = backup_dir.join(name);
    let escaped = destination.display().to_string().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .context("failed to create consistent database backup")?;
    Ok(Some(destination))
}
