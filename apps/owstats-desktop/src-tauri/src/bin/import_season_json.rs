use anyhow::{bail, Result};
use owstats_desktop_lib::{import_full_season_payload, OwDatabase};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let database = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: import_season_json <database> <season-json>"))?;
    let payload = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: import_season_json <database> <season-json>"))?;
    if args.next().is_some() {
        bail!("usage: import_season_json <database> <season-json>");
    }

    let summary = import_full_season_payload(&OwDatabase::new(database), payload)?;
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}
