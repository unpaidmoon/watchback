use crate::{
    account_store::{read_manifest, read_registry},
    app_paths::SharedAppPaths,
};
use anyhow::{anyhow, bail, Result};
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::HashSet,
    fs,
    ops::Deref,
    path::{Path, PathBuf},
};

pub struct TrainingSourceSpec {
    pub account_group: String,
    database_path: PathBuf,
    expected_canonical_account_root: PathBuf,
    expected_database_identity: DatabaseIdentity,
}

#[derive(PartialEq, Eq)]
struct DatabaseIdentity {
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    #[cfg(windows)]
    file_size: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(any(windows, unix)))]
    file_size: u64,
    #[cfg(not(any(windows, unix)))]
    modified: Option<std::time::SystemTime>,
}

impl TrainingSourceSpec {
    pub(crate) fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn conflicts_with_output(&self, output: &Path) -> bool {
        output == self.database_path
            || (output.exists()
                && (fs::canonicalize(output).ok() == fs::canonicalize(&self.database_path).ok()
                    || fs::metadata(output).ok().is_some_and(|metadata| {
                        database_identity(&metadata) == self.expected_database_identity
                    })))
    }
}

pub struct QueryOnlyConnection {
    connection: Connection,
    _database_guard: fs::File,
}

impl Deref for QueryOnlyConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

pub fn open_query_only(source: &TrainingSourceSpec) -> Result<QueryOnlyConnection> {
    let result = (|| -> Result<QueryOnlyConnection> {
        revalidate_source_path(source)?;
        let database_guard = open_pinned_database(source.database_path())?;
        validate_pinned_database(source, &database_guard)?;
        revalidate_source_path(source)?;
        let conn = Connection::open_with_flags(
            source.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
        )?;
        conn.pragma_update(None, "query_only", "ON")?;
        validate_pinned_database(source, &database_guard)?;
        revalidate_source_path(source)?;
        Ok(QueryOnlyConnection {
            connection: conn,
            _database_guard: database_guard,
        })
    })();
    result.map_err(|_| anyhow!("无法只读打开训练数据源"))
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
                "hero_guid",
                "duration_sec",
                "raw_entry_json",
            ],
        ),
        ("hero_dict", &["guid", "role"]),
        (
            "match_players",
            &["match_id", "is_current_player", "raw_json"],
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
            expected_canonical_account_root: canonical_account_root,
            expected_database_identity: database_identity(&fs::metadata(&canonical_database)?),
        });
    }
    Ok(sources)
}

fn revalidate_source_path(source: &TrainingSourceSpec) -> Result<()> {
    let account_root = source
        .database_path
        .parent()
        .ok_or_else(|| anyhow!("missing account root"))?;
    ensure_safe_directory(account_root)?;
    let canonical_account_root = fs::canonicalize(account_root)?;
    if canonical_account_root != source.expected_canonical_account_root {
        bail!("account root changed");
    }
    ensure_safe_file(source.database_path())?;
    let canonical_database = fs::canonicalize(source.database_path())?;
    if canonical_database.parent() != Some(source.expected_canonical_account_root.as_path()) {
        bail!("database boundary changed");
    }
    Ok(())
}

#[cfg(windows)]
fn open_pinned_database(path: &Path) -> Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    Ok(options.open(path)?)
}

#[cfg(not(windows))]
fn open_pinned_database(path: &Path) -> Result<fs::File> {
    Ok(fs::OpenOptions::new().read(true).open(path)?)
}

fn validate_pinned_database(source: &TrainingSourceSpec, guard: &fs::File) -> Result<()> {
    let metadata = guard.metadata()?;
    if is_windows_reparse(&metadata) || !metadata.is_file() {
        bail!("unsafe pinned database");
    }
    if database_identity(&metadata) != source.expected_database_identity {
        bail!("database identity changed");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current = fs::metadata(source.database_path())?;
        if current.dev() != metadata.dev() || current.ino() != metadata.ino() {
            bail!("database changed during open");
        }
    }
    Ok(())
}

fn database_identity(metadata: &fs::Metadata) -> DatabaseIdentity {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        DatabaseIdentity {
            creation_time: metadata.creation_time(),
            last_write_time: metadata.last_write_time(),
            file_size: metadata.file_size(),
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        DatabaseIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        DatabaseIdentity {
            file_size: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
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
