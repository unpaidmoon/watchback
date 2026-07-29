use anyhow::{anyhow, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OwDatabase {
    path: PathBuf,
    backups: Option<PathBuf>,
    initialized: Option<Arc<Mutex<bool>>>,
}

impl OwDatabase {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            backups: None,
            initialized: None,
        }
    }

    pub fn for_account(paths: &crate::app_paths::AccountPaths) -> Self {
        Self {
            path: paths.database(),
            backups: Some(paths.backups()),
            initialized: Some(Arc::new(Mutex::new(false))),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn connect(&self) -> Result<Connection> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&self.path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(Duration::from_secs(5))?;
        if self.path.is_file() {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "synchronous", "NORMAL")?;
        }
        Ok(conn)
    }

    pub fn initialize(&self) -> Result<()> {
        let mut initialized = self
            .initialized
            .as_ref()
            .map(|state| state.lock().map_err(|_| anyhow!("database initialization lock poisoned")))
            .transpose()?;
        if initialized.as_deref() == Some(&true) {
            return Ok(());
        }
        let database_preexisted = self.path.is_file()
            && std::fs::metadata(&self.path)
                .map(|metadata| metadata.len() > 0)
                .unwrap_or(false);
        let conn = self.connect()?;
        let schema_version: i64 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if schema_version == crate::database::migrations::CURRENT_SCHEMA_VERSION {
            if let Some(state) = initialized.as_deref_mut() {
                *state = true;
            }
            return Ok(());
        }
        if database_preexisted {
            if let Some(backups) = self.backups.as_ref() {
                crate::database::backup::backup_before_schema_migration(
                    &conn,
                    &self.path,
                    backups,
                    crate::database::migrations::CURRENT_SCHEMA_VERSION,
                )?;
            } else if let Some(app_data_dir) = self.path.parent() {
                crate::database::backup::backup_before_schema_migration(
                    &conn,
                    &self.path,
                    &app_data_dir.join("backups"),
                    crate::database::migrations::CURRENT_SCHEMA_VERSION,
                )?;
            }
        }
        initialize_connection(&conn)?;
        if let Some(state) = initialized.as_deref_mut() {
            *state = true;
        }
        Ok(())
    }
}

pub fn initialize_connection(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS players (
            player_id TEXT PRIMARY KEY,
            bnet_id TEXT,
            display_name TEXT,
            raw_json TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS seasons (
            season INTEGER PRIMARY KEY,
            status TEXT NOT NULL,
            synced_at TEXT NOT NULL,
            matches_count INTEGER NOT NULL DEFAULT 0,
            details_count INTEGER NOT NULL DEFAULT 0,
            errors_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS matches (
            match_id TEXT PRIMARY KEY,
            season INTEGER NOT NULL,
            bucket TEXT NOT NULL,
            game_mode TEXT NOT NULL,
            match_kind TEXT NOT NULL,
            result TEXT NOT NULL,
            map_guid TEXT NOT NULL,
            map_name TEXT,
            hero_guid TEXT NOT NULL,
            hero_name TEXT,
            begin_ts INTEGER NOT NULL,
            team_score INTEGER NOT NULL,
            opponent_score INTEGER NOT NULL,
            kills INTEGER NOT NULL,
            assists INTEGER NOT NULL,
            deaths INTEGER NOT NULL,
            damage INTEGER NOT NULL,
            healing INTEGER NOT NULL,
            mitigation INTEGER NOT NULL,
            total_worth INTEGER NOT NULL,
            duration_sec INTEGER NOT NULL,
            raw_entry_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS match_details (
            match_id TEXT PRIMARY KEY REFERENCES matches(match_id) ON DELETE CASCADE,
            match_kind TEXT NOT NULL,
            raw_detail_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS match_players (
            match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
            season INTEGER NOT NULL,
            side TEXT NOT NULL,
            bnet_id TEXT NOT NULL,
            player_name TEXT NOT NULL,
            hero_guid TEXT NOT NULL,
            hero_name TEXT,
            is_current_player INTEGER NOT NULL DEFAULT 0,
            is_friend INTEGER NOT NULL DEFAULT 0,
            kill INTEGER NOT NULL DEFAULT 0,
            assist INTEGER NOT NULL DEFAULT 0,
            death INTEGER NOT NULL DEFAULT 0,
            hero_damage INTEGER NOT NULL DEFAULT 0,
            cure INTEGER NOT NULL DEFAULT 0,
            resist_damage INTEGER NOT NULL DEFAULT 0,
            healing_taken INTEGER NOT NULL DEFAULT 0,
            damage_taken INTEGER NOT NULL DEFAULT 0,
            final_hit INTEGER NOT NULL DEFAULT 0,
            target_competing_time INTEGER NOT NULL DEFAULT 0,
            role_type TEXT,
            raw_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (match_id, side, bnet_id)
        );

        CREATE TABLE IF NOT EXISTS match_evaluations (
            match_id TEXT PRIMARY KEY REFERENCES matches(match_id) ON DELETE CASCADE,
            mode TEXT NOT NULL,
            rule_version INTEGER NOT NULL,
            grade TEXT NOT NULL,
            primary_tag TEXT NOT NULL,
            evidence_tags_json TEXT NOT NULL,
            reasons_json TEXT NOT NULL,
            confidence REAL NOT NULL,
            computed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS hero_dict (
            guid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            role TEXT,
            icon_path TEXT,
            raw_json TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS map_dict (
            guid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            mode TEXT,
            image_path TEXT,
            raw_json TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS metric_dict (
            guid TEXT PRIMARY KEY,
            label TEXT NOT NULL,
            value_type TEXT,
            raw_json TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_errors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            season INTEGER,
            match_id TEXT,
            error_type TEXT,
            message TEXT,
            raw_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_matches_bucket_begin ON matches(bucket, begin_ts DESC);
        CREATE INDEX IF NOT EXISTS idx_matches_filters ON matches(season, hero_guid, map_guid, result, game_mode, begin_ts);
        CREATE INDEX IF NOT EXISTS idx_match_players_friend ON match_players(season, side, is_friend, is_current_player);
        CREATE INDEX IF NOT EXISTS idx_match_players_bnet ON match_players(bnet_id, season);
        CREATE INDEX IF NOT EXISTS idx_match_evaluations_version ON match_evaluations(rule_version);
        CREATE INDEX IF NOT EXISTS idx_match_evaluations_primary_tag ON match_evaluations(primary_tag);
        "#,
    )?;
    conn.execute(
        r#"
        UPDATE matches
        SET result = 'loss'
        WHERE result = 'draw'
          AND (
            team_score < opponent_score
            OR raw_entry_json LIKE '%"matchRet":-1%'
            OR raw_entry_json LIKE '%"matchRet": -1%'
          )
        "#,
        [],
    )?;
    crate::database::migrations::run_migrations(conn)?;
    Ok(())
}
