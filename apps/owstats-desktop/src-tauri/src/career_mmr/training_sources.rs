use crate::{
    account_store::{read_manifest, read_registry},
    app_paths::SharedAppPaths,
};
use anyhow::{anyhow, bail, Result};
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub struct TrainingSourceSpec {
    pub account_group: String,
    database_path: PathBuf,
}

impl TrainingSourceSpec {
    pub(crate) fn database_path(&self) -> &Path {
        &self.database_path
    }
}

pub fn open_query_only(source: &TrainingSourceSpec) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        source.database_path(),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|_| anyhow!("无法只读打开训练数据源"))?;
    conn.pragma_update(None, "query_only", "ON")
        .map_err(|_| anyhow!("无法启用训练数据源只读模式"))?;
    Ok(conn)
}

pub fn validate_training_schema(conn: &Connection) -> Result<()> {
    validate_training_schema_inner(conn).map_err(|_| anyhow!("训练数据源结构不兼容"))
}

fn validate_training_schema_inner(conn: &Connection) -> Result<()> {
    const REQUIRED: &[(&str, &[&str])] = &[
        (
            "matches",
            &[
                "match_id",
                "season",
                "begin_ts",
                "bucket",
                "game_mode",
                "result",
                "map_guid",
                "hero_guid",
                "team_score",
                "opponent_score",
                "duration_sec",
                "raw_entry_json",
            ],
        ),
        ("hero_dict", &["guid", "role"]),
        (
            "match_players",
            &["match_id", "is_current_player", "bnet_id", "raw_json"],
        ),
        ("match_details", &["match_id", "raw_detail_json"]),
    ];
    for (table, required_columns) in REQUIRED {
        let sql = format!("SELECT name FROM pragma_table_info('{table}')");
        let mut stmt = conn.prepare(&sql)?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<HashSet<_>>>()?;
        if !required_columns
            .iter()
            .all(|column| columns.contains(*column))
        {
            bail!("missing training schema");
        }
    }
    Ok(())
}

pub fn discover_training_sources(paths: &SharedAppPaths) -> Result<Vec<TrainingSourceSpec>> {
    discover_training_sources_inner(paths).map_err(|_| anyhow!("无法发现多账号训练数据源"))
}

fn discover_training_sources_inner(paths: &SharedAppPaths) -> Result<Vec<TrainingSourceSpec>> {
    ensure_safe_directory(paths.root())?;
    let mut accounts = read_registry(&paths.registry())?.accounts;
    accounts.sort_by(|left, right| left.account_id.cmp(&right.account_id));

    let accounts_root = paths.accounts_root();
    ensure_safe_directory(&accounts_root)?;
    let canonical_accounts_root = fs::canonicalize(&accounts_root)?;
    let mut sources = Vec::with_capacity(accounts.len());
    for (index, entry) in accounts.into_iter().enumerate() {
        let account_paths = paths.account(&entry.account_id);
        ensure_safe_directory(account_paths.root())?;
        let canonical_account_root = fs::canonicalize(account_paths.root())?;
        if !canonical_account_root.starts_with(&canonical_accounts_root) {
            bail!("unsafe account boundary");
        }

        let manifest = read_manifest(&account_paths.manifest(), &entry.account_id)?;
        if manifest.battle_tag != entry.battle_tag {
            bail!("account identity mismatch");
        }

        let database_path = account_paths.database();
        ensure_safe_file(&database_path)?;
        let canonical_database = fs::canonicalize(&database_path)?;
        if canonical_database.parent() != Some(canonical_account_root.as_path()) {
            bail!("unsafe database boundary");
        }
        sources.push(TrainingSourceSpec {
            account_group: format!("dataset-{:03}", index + 1),
            database_path,
        });
    }
    Ok(sources)
}

fn ensure_safe_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) || !metadata.is_dir() {
        bail!("unsafe training source directory");
    }
    Ok(())
}

fn ensure_safe_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) || !metadata.is_file() {
        bail!("unsafe training source file");
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse(_metadata: &fs::Metadata) -> bool {
    false
}
