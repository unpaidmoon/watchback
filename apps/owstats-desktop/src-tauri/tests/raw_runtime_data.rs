use anyhow::Result;
use owstats_desktop_lib::{
    account_store::AccountId, app_paths::SharedAppPaths, benchmark_snapshot,
    import_payload_value_embedded, runtime_error_message, write_season_payload, OwDatabase,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[test]
fn command_error_conversion_preserves_original_text() {
    let source =
        "request failed token=synthetic-error-token Authorization: Bearer synthetic-header";

    assert_eq!(runtime_error_message(anyhow::anyhow!(source)), source,);
}

#[test]
fn importer_and_direct_season_file_preserve_natural_fields() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    let payload = json!({
        "season": 22,
        "profile_card": {
            "data": {
                "bnetId": "synthetic-player",
                "name": "匿名账号",
                "customerToken": "synthetic-profile-token"
            }
        },
        "matches": {
            "entries": [{
                "matchId": "raw-contract-match",
                "gameMode": "sport",
                "beginTs": 1720000000,
                "token": "synthetic-entry-token"
            }],
            "details": [{
                "match_id": "raw-contract-match",
                "match_kind": "normal",
                "payload": {
                    "code": 0,
                    "data": {
                        "cookie": "synthetic-detail-cookie",
                        "totalCount": {
                            "teammateList": [],
                            "enemyList": []
                        }
                    }
                }
            }],
            "detail_errors": [{
                "match_id": "raw-error-match",
                "error_type": "DetailFetchError",
                "message": "Authorization: Bearer synthetic-error-authorization",
                "authorization": "synthetic-error-authorization"
            }]
        }
    });

    import_payload_value_embedded(&db, &payload)?;

    let conn = db.connect()?;
    let raw_entry_json: String = conn.query_row(
        "SELECT raw_entry_json FROM matches WHERE match_id='raw-contract-match'",
        [],
        |row| row.get(0),
    )?;
    let raw_detail_json: String = conn.query_row(
        "SELECT raw_detail_json FROM match_details WHERE match_id='raw-contract-match'",
        [],
        |row| row.get(0),
    )?;
    let raw_error_json: String = conn.query_row(
        "SELECT raw_json FROM sync_errors WHERE season=22 AND match_id='raw-error-match'",
        [],
        |row| row.get(0),
    )?;
    let raw_profile_json: String = conn.query_row(
        "SELECT raw_json FROM players WHERE player_id='synthetic-player'",
        [],
        |row| row.get(0),
    )?;
    drop(conn);

    assert_eq!(
        serde_json::from_str::<Value>(&raw_entry_json)?["token"],
        "synthetic-entry-token"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&raw_detail_json)?["payload"]["data"]["cookie"],
        "synthetic-detail-cookie"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&raw_error_json)?["authorization"],
        "synthetic-error-authorization"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&raw_profile_json)?["customerToken"],
        "synthetic-profile-token"
    );

    let written = write_season_payload(temp.path().join("seasons").as_path(), 22, &payload)?;
    let written_payload: Value = serde_json::from_str(&std::fs::read_to_string(written)?)?;

    assert_eq!(
        written_payload["matches"]["entries"][0]["token"],
        "synthetic-entry-token"
    );
    assert_eq!(
        written_payload["matches"]["details"][0]["payload"]["data"]["cookie"],
        "synthetic-detail-cookie"
    );
    assert_eq!(
        written_payload["matches"]["detail_errors"][0]["authorization"],
        "synthetic-error-authorization"
    );

    Ok(())
}

#[test]
fn database_initialize_does_not_rewrite_existing_raw_json_or_season_files() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111")?;
    let account = shared.account(&account_id);
    std::fs::create_dir_all(account.seasons())?;

    let db = OwDatabase::for_account(&account);
    db.initialize()?;
    let raw_entry_json = r#"{"matchId":"legacy-raw-match","token":"synthetic-legacy-token"}"#;
    let season_payload = r#"{"season":22,"cookie":"synthetic-legacy-cookie"}"#;
    let conn = db.connect()?;
    conn.execute(
        "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES ('legacy-raw-match', 22, 'preset', 'SportPreset', 'normal', 'win', 'map', 'hero', 100, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, ?1, 'now')",
        [raw_entry_json],
    )?;
    // Databases created before the retired migration have no completion marker.
    conn.execute(
        "DELETE FROM app_settings WHERE key = 'privacy_sanitization_v1'",
        [],
    )?;
    drop(conn);
    let season_file = account.seasons().join("local_s22_full_season.json");
    std::fs::write(&season_file, season_payload)?;

    db.initialize()?;

    let persisted_raw_entry_json: String = db.connect()?.query_row(
        "SELECT raw_entry_json FROM matches WHERE match_id = 'legacy-raw-match'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(persisted_raw_entry_json, raw_entry_json);
    assert_eq!(std::fs::read_to_string(season_file)?, season_payload);
    Ok(())
}

#[test]
fn benchmark_snapshot_reports_only_aggregate_snapshot_metrics() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    let sentinel = "benchmark-sentinel-not-in-report";
    import_payload_value_embedded(
        &db,
        &json!({
            "season": 22,
            "matches": {
                "entries": [{
                    "matchId": "benchmark-match",
                    "gameMode": "sport",
                    "beginTs": 1720000000,
                    "benchmarkSentinel": sentinel
                }],
                "details": [{
                    "match_id": "benchmark-match",
                    "match_kind": "normal",
                    "payload": {"code": 0, "data": {}}
                }],
                "detail_errors": [{
                    "match_id": "benchmark-error",
                    "error_type": "DetailFetchError",
                    "message": "benchmark failure"
                }]
            }
        }),
    )?;

    let output = temp.path().join("benchmark.json");
    let benchmark = benchmark_snapshot(&db, 22, &output)?;

    assert_eq!(benchmark.season, 22);
    assert_eq!(benchmark.entries, 1);
    assert_eq!(benchmark.details, 1);
    assert_eq!(benchmark.errors, 1);
    assert_eq!(benchmark.output_bytes, std::fs::metadata(&output)?.len());
    let serialized_snapshot = std::fs::read_to_string(&output)?;
    assert!(serialized_snapshot.contains(sentinel));
    let serialized_benchmark = serde_json::to_string(&benchmark)?;
    let benchmark_keys = serde_json::from_str::<Value>(&serialized_benchmark)?
        .as_object()
        .expect("benchmark must serialize as an object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        benchmark_keys,
        BTreeSet::from([
            "season".to_string(),
            "entries".to_string(),
            "details".to_string(),
            "errors".to_string(),
            "assembleMs".to_string(),
            "serializeMs".to_string(),
            "writeAndSyncMs".to_string(),
            "totalMs".to_string(),
            "outputBytes".to_string(),
        ])
    );
    assert!(!serialized_benchmark.contains(sentinel));
    Ok(())
}
