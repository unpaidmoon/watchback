use anyhow::Result;
use owstats_desktop_lib::{
    benchmark_snapshot, import_payload_value_embedded, rebuild_complete_season_payload,
    write_season_payload, OwDatabase,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[test]
fn importer_snapshot_and_season_file_preserve_natural_fields() -> Result<()> {
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

    let rebuilt = rebuild_complete_season_payload(
        &db,
        22,
        &json!({
            "season": 22,
            "matches": {}
        }),
    )?;
    let written = write_season_payload(temp.path().join("seasons").as_path(), 22, &rebuilt)?;
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
    assert_eq!(benchmark.output_bytes, std::fs::metadata(output)?.len());
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
