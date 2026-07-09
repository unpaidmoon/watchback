use anyhow::{anyhow, Context, Result};
use owstats_desktop_lib::dump_season_json;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() -> Result<()> {
    let season = parse_season(std::env::args().nth(1))?;
    let project_root = project_root()?;
    let output_dir = project_root.join("data").join("players");

    let dump = dump_season_json(&project_root, season, &output_dir, |progress| {
        eprintln!(
            "phase={} processed={}/{} failed={}",
            progress.phase, progress.processed, progress.total, progress.failed_details
        );
    })
    .await?;

    println!(
        "wrote {} entries={} details={} failed_details={} skipped_duplicates={}",
        dump.path.display(),
        dump.entries,
        dump.details,
        dump.failed_details,
        dump.skipped_duplicates
    );
    Ok(())
}

fn parse_season(value: Option<String>) -> Result<i64> {
    let value =
        value.ok_or_else(|| anyhow!("usage: cargo run --bin dump_season_json -- <season>"))?;
    let season = value
        .parse::<i64>()
        .with_context(|| format!("invalid season {value}"))?;
    if season <= 0 {
        return Err(anyhow!("season must be greater than 0"));
    }
    Ok(season)
}

fn project_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir
        .join("..")
        .join("..")
        .join("..")
        .canonicalize()
        .with_context(|| "failed to resolve project root")?)
}
