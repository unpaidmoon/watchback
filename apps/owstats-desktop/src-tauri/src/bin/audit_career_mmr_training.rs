use anyhow::{bail, Result};
use owstats_desktop_lib::career_mmr::training::build_training_projection;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

fn main() -> Result<()> {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        bail!("用法：audit_career_mmr_training <数据库路径>");
    };
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "query_only", true)?;
    let projection = build_training_projection(&conn)?;
    println!(
        "{}",
        serde_json::to_string_pretty(projection.public_audit())?
    );
    Ok(())
}
