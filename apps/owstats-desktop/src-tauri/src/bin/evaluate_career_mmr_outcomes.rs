use anyhow::{bail, Result};
use owstats_desktop_lib::career_mmr::{
    evaluation::evaluate_outcome_baselines, training::build_training_projection,
};
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

fn main() -> Result<()> {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        bail!("用法：evaluate_career_mmr_outcomes <数据库路径>");
    };
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "query_only", true)?;
    let projection = build_training_projection(&conn)?;
    let report = evaluate_outcome_baselines(&projection.rows);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
