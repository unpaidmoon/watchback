use anyhow::{bail, Context, Result};
use owstats_desktop_lib::{benchmark_snapshot, OwDatabase};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let database = args.next().ok_or_else(|| {
        anyhow::anyhow!("usage: benchmark_season_snapshot <database> <season> <output>")
    })?;
    let season = args
        .next()
        .ok_or_else(|| {
            anyhow::anyhow!("usage: benchmark_season_snapshot <database> <season> <output>")
        })?
        .to_string_lossy()
        .parse::<i64>()
        .context("season must be an integer")?;
    let output = args.next().ok_or_else(|| {
        anyhow::anyhow!("usage: benchmark_season_snapshot <database> <season> <output>")
    })?;
    if args.next().is_some() {
        bail!("usage: benchmark_season_snapshot <database> <season> <output>");
    }

    let result = benchmark_snapshot(
        &OwDatabase::new(PathBuf::from(database)),
        season,
        PathBuf::from(output).as_path(),
    )?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
