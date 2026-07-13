use anyhow::{bail, Context, Result};
use owstats_desktop_lib::career_mmr::input::audit_career_mmr_inputs;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next() else {
        bail!("用法: audit_career_mmr_inputs <数据库路径>");
    };
    if args.next().is_some() {
        bail!("仅接受一个明确的数据库路径参数");
    }
    let path = PathBuf::from(path);
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("无法只读打开数据库: {}", path.display()))?;
    let audit = audit_career_mmr_inputs(&conn)?;
    println!("{}", serde_json::to_string_pretty(&audit)?);
    Ok(())
}
