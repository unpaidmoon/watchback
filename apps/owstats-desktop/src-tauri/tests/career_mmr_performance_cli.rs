use owstats_desktop_lib::career_mmr::performance_stability::{
    aggregate_progress_schedule, TrainingStage,
};
use std::process::Command;

const SOURCE: &str = include_str!("../src/bin/evaluate_career_mmr_performance.rs");

#[test]
fn cli_source_uses_read_only_query_only_and_safe_aggregate_output() {
    assert!(SOURCE.contains("SQLITE_OPEN_READ_ONLY"));
    assert!(SOURCE.contains("SQLITE_OPEN_NO_MUTEX"));
    assert!(SOURCE.contains("query_only"));
    assert!(SOURCE.contains("build_training_projection"));
    assert!(SOURCE.contains("evaluate_stability"));
    assert!(!SOURCE.contains("println!(\"{:?}\""));
    for forbidden in ["match_id", "hero_group", "player_id", "guid"] {
        assert!(!SOURCE.to_ascii_lowercase().contains(forbidden));
    }
}

#[test]
fn cli_defaults_to_one_thousand_bootstrap_iterations() {
    assert!(SOURCE.contains("bootstrap_iterations: 1000"));
}

#[test]
fn staged_progress_uses_exact_aggregate_work_and_safe_eta_batches() {
    for (stage, iterations) in [
        (TrainingStage::Screen100, 100usize),
        (TrainingStage::Candidate1000, 1_000),
        (TrainingStage::Final10000, 10_000),
    ] {
        assert_eq!(stage.bootstrap_iterations(), iterations);
        let events = aggregate_progress_schedule(stage, 137);
        let total = iterations * 3 * 5;
        assert_eq!(events.first().unwrap().completed, 0);
        assert_eq!(events.first().unwrap().total, total);
        assert_eq!(events.last().unwrap().completed, total);
        assert_eq!(events.last().unwrap().eta_ms, Some(0));
        assert!(events
            .windows(2)
            .all(|pair| pair[0].completed < pair[1].completed));
        assert!(events
            .iter()
            .skip(1)
            .take(events.len().saturating_sub(2))
            .all(|event| event.eta_ms.is_some_and(|eta| eta > 0)));
        let json = serde_json::to_string(&events).unwrap().to_ascii_lowercase();
        for forbidden in ["account", "match", "hero", "player", "guid", "path"] {
            assert!(!json.contains(forbidden));
        }
    }
}

#[test]
fn nonexistent_database_error_is_chinese_and_does_not_echo_path() {
    let secret_path = "Z:\\private-player-guid-123\\missing.sqlite";
    let output = Command::new(env!("CARGO_BIN_EXE_evaluate_career_mmr_performance"))
        .arg(secret_path)
        .output()
        .expect("run performance evaluator");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    assert!(stderr.contains("无法只读打开数据库"));
    assert!(!stderr.contains(secret_path));
    assert!(!stderr.to_ascii_lowercase().contains("guid"));
}

#[test]
fn serialized_json_contains_only_aggregate_audit_and_keeps_performance_zero() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("empty.sqlite");
    let conn = rusqlite::Connection::open(&database).unwrap();
    conn.execute_batch(
        "CREATE TABLE matches(match_id TEXT, season INTEGER, begin_ts INTEGER, hero_guid TEXT, result TEXT, game_mode TEXT, duration_sec INTEGER, raw_entry_json TEXT, bucket TEXT);
         CREATE TABLE hero_dict(guid TEXT, role TEXT);
         CREATE TABLE match_players(match_id TEXT, is_current_player INTEGER, raw_json TEXT);
         CREATE TABLE match_details(match_id TEXT, raw_detail_json TEXT);",
    )
    .unwrap();
    drop(conn);

    let output = Command::new(env!("CARGO_BIN_EXE_evaluate_career_mmr_performance"))
        .args([
            database.as_os_str(),
            "--bootstrap-iterations".as_ref(),
            "0".as_ref(),
            "--seed".as_ref(),
            "7".as_ref(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["algorithm_config"]["bootstrap_iterations"], 0);
    assert_eq!(json["algorithm_config"]["seed"], 7);
    assert_eq!(json["audit"]["ordinary_candidates"], 0);
    assert!(json["elapsed_ms"].is_u64());
    assert!(json["worker_errors"].as_array().is_some());
    assert!(json["reports"].as_array().unwrap().iter().all(|report| {
        report["performance_adjustment"].as_f64() == Some(0.0)
            && report["forward"]["performance_adjustment"].as_f64() == Some(0.0)
    }));
    let serialized = String::from_utf8(output.stdout)
        .unwrap()
        .to_ascii_lowercase();
    for forbidden in ["match_id", "hero_group", "player_id", "guid", "database"] {
        assert!(!serialized.contains(forbidden));
    }
}
