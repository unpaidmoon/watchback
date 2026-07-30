use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use tauri::Manager;

const CURRENT_SEASON_RESOURCE: &str = "current-season.json";

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentSeasonConfig {
    pub schema_version: u32,
    pub config_version: u64,
    pub current_season: i64,
}

impl CurrentSeasonConfig {
    fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            self.schema_version == 1,
            "unsupported current-season config schema"
        );
        anyhow::ensure!(
            self.config_version > 0,
            "current-season config version must be positive"
        );
        anyhow::ensure!(self.current_season > 0, "current season must be positive");
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeasonResolutionSource {
    UpstreamMetadata,
    RuntimeConfig,
    SeasonDiscoveryUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentSeasonResolution {
    pub season: Option<i64>,
    pub source: SeasonResolutionSource,
}

/// Resolves the season only from explicit evidence. Match-list entries are deliberately
/// absent from this interface because their `season` field is not reliable current-season
/// metadata.
pub fn resolve_current_season(
    upstream_metadata: Option<&serde_json::Value>,
    runtime_config: Option<CurrentSeasonConfig>,
    latest_local: Option<i64>,
) -> CurrentSeasonResolution {
    if let Some(season) = upstream_metadata.and_then(extract_upstream_current_season) {
        return CurrentSeasonResolution {
            season: Some(season),
            source: SeasonResolutionSource::UpstreamMetadata,
        };
    }

    if let Some(config) = runtime_config.filter(|config| {
        config.validate().is_ok() && latest_local.is_none_or(|local| config.current_season >= local)
    }) {
        return CurrentSeasonResolution {
            season: Some(config.current_season),
            source: SeasonResolutionSource::RuntimeConfig,
        };
    }

    CurrentSeasonResolution {
        season: latest_local,
        source: SeasonResolutionSource::SeasonDiscoveryUnavailable,
    }
}

pub fn load_current_season_config(path: &Path) -> Result<CurrentSeasonConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read current-season config {}", path.display()))?;
    serde_json::from_str::<CurrentSeasonConfig>(&raw)
        .context("failed to parse current-season config")?
        .validate()
}

pub(crate) fn load_packaged_current_season_config(
    app: &tauri::AppHandle,
) -> Result<CurrentSeasonConfig> {
    let path = app
        .path()
        .resolve(
            CURRENT_SEASON_RESOURCE,
            tauri::path::BaseDirectory::Resource,
        )
        .context("failed to resolve packaged current-season config")?;
    load_current_season_config(&path)
}

fn extract_upstream_current_season(payload: &serde_json::Value) -> Option<i64> {
    [
        payload.pointer("/data/currentSeason"),
        payload.get("currentSeason"),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
            .filter(|season| *season > 0)
    })
}
