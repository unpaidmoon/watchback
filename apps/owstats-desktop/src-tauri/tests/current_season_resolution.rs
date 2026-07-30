use owstats_desktop_lib::runtime_config::{
    load_current_season_config, resolve_current_season, CurrentSeasonConfig, SeasonResolutionSource,
};

#[test]
fn reliable_upstream_metadata_wins_over_runtime_config() {
    let metadata = serde_json::json!({"data": {"currentSeason": "25"}});
    let resolution = resolve_current_season(Some(&metadata), Some(config(2, 24)), Some(23));

    assert_eq!(resolution.season, Some(25));
    assert_eq!(resolution.source, SeasonResolutionSource::UpstreamMetadata);
}

#[test]
fn versioned_runtime_config_discovers_a_new_season() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("current-season.json");
    std::fs::write(
        &path,
        r#"{"schemaVersion":1,"configVersion":7,"currentSeason":24}"#,
    )
    .unwrap();

    let loaded = load_current_season_config(&path).unwrap();
    let resolution = resolve_current_season(None, Some(loaded), Some(23));

    assert_eq!(loaded.config_version, 7);
    assert_eq!(resolution.season, Some(24));
    assert_eq!(resolution.source, SeasonResolutionSource::RuntimeConfig);
}

#[test]
fn list_entries_are_never_inferred_as_current_season_metadata() {
    let list_payload = serde_json::json!({
        "data": {"list": [{"season": 99, "_dashenSeason": 99}]}
    });

    let resolution = resolve_current_season(Some(&list_payload), Some(config(3, 24)), Some(23));

    assert_eq!(resolution.season, Some(24));
    assert_eq!(resolution.source, SeasonResolutionSource::RuntimeConfig);
}

#[test]
fn stale_or_invalid_config_safely_falls_back_to_local_maximum() {
    let stale = resolve_current_season(None, Some(config(4, 22)), Some(23));
    assert_eq!(stale.season, Some(23));
    assert_eq!(
        stale.source,
        SeasonResolutionSource::SeasonDiscoveryUnavailable
    );

    let invalid = CurrentSeasonConfig {
        schema_version: 2,
        config_version: 5,
        current_season: 25,
    };
    let unsupported = resolve_current_season(None, Some(invalid), Some(23));
    assert_eq!(unsupported.season, Some(23));
    assert_eq!(
        unsupported.source,
        SeasonResolutionSource::SeasonDiscoveryUnavailable
    );
}

#[test]
fn missing_all_evidence_reports_unavailable_without_guessing() {
    let resolution = resolve_current_season(None, None, None);
    assert_eq!(resolution.season, None);
    assert_eq!(
        resolution.source,
        SeasonResolutionSource::SeasonDiscoveryUnavailable
    );
}

#[test]
fn malformed_or_unsupported_runtime_files_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let malformed = temp.path().join("malformed.json");
    std::fs::write(&malformed, "not-json").unwrap();
    assert!(load_current_season_config(&malformed).is_err());

    let unsupported = temp.path().join("unsupported.json");
    std::fs::write(
        &unsupported,
        r#"{"schemaVersion":2,"configVersion":8,"currentSeason":25}"#,
    )
    .unwrap();
    assert!(load_current_season_config(&unsupported).is_err());
}

#[test]
fn packaged_config_is_valid_and_versioned() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("current-season.json");
    let loaded = load_current_season_config(&path).unwrap();
    assert_eq!(loaded.schema_version, 1);
    assert!(loaded.config_version > 0);
    assert!(loaded.current_season > 0);
}

fn config(version: u64, season: i64) -> CurrentSeasonConfig {
    CurrentSeasonConfig {
        schema_version: 1,
        config_version: version,
        current_season: season,
    }
}
