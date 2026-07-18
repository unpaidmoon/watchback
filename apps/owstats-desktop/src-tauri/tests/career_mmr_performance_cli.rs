use owstats_desktop_lib::career_mmr::performance_stability::{
    aggregate_progress_schedule, aggregate_progress_schedule_with_outcome, evaluate_stability,
    PromotedCandidate, StabilityConfig, TrainingStage,
};
use owstats_desktop_lib::{
    account_store::{
        write_manifest_atomic, write_registry_atomic, AccountId, AccountManifest, AccountRegistry,
        AccountRegistryEntry,
    },
    app_paths::SharedAppPaths,
    career_mmr::{
        training::TrainingAudit,
        training_report::{build_training_outputs, recompute_promotion_hash},
    },
};
use std::fs;
use std::process::Command;

const SOURCE: &str = include_str!("../src/bin/evaluate_career_mmr_performance.rs");

fn run_trainer(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .args(args)
        .output()
        .expect("run staged trainer")
}

#[test]
fn staged_trainer_rejects_missing_promotion_and_never_echoes_values() {
    let secret = "Z:\\synthetic-secret-player-guid\\app-data";
    let output = run_trainer(&[
        "--app-data-dir",
        secret,
        "--stage",
        "candidate",
        "--report",
        "synthetic-report.json",
        "--artifact",
        "synthetic-artifact.json",
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr,
        "错误：candidate 和 final 阶段必须提供上一阶段报告\n"
    );
    assert!(!stderr.contains(secret));
}

#[test]
fn staged_trainer_missing_account_store_is_nonzero_and_does_not_echo_path() {
    let holder = tempfile::tempdir().unwrap();
    let secret = holder.path().join("synthetic-private-guid-app-data");
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(&secret)
        .args(["--stage", "screen", "--report"])
        .arg(holder.path().join("report.json"))
        .arg("--artifact")
        .arg(holder.path().join("artifact.json"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr, "错误：无法发现训练账号数据库\n");
    assert!(!stderr.contains(secret.to_str().unwrap()));
    assert!(!stderr.to_ascii_lowercase().contains("guid"));
}

#[test]
fn staged_trainer_has_strict_fixed_chinese_argument_errors() {
    let cases: &[(&[&str], &str)] = &[
        (&["--unknown", "value"], "错误：存在无法识别的命令行选项\n"),
        (&["--stage"], "错误：命令行选项缺少数值\n"),
        (
            &["--stage", "screen", "--stage", "final"],
            "错误：命令行选项不能重复\n",
        ),
        (
            &["--seed", "not-a-number"],
            "错误：随机种子必须是非负整数\n",
        ),
        (&["--stage", "--report"], "错误：命令行选项缺少数值\n"),
        (
            &["--stage", "invalid"],
            "错误：训练阶段必须是 screen、candidate 或 final\n",
        ),
    ];
    for (args, expected) in cases {
        let output = run_trainer(args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8(output.stderr).unwrap(), *expected);
    }
}

fn synthetic_app_data() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(dir.path());
    fs::create_dir_all(shared.accounts_root()).unwrap();
    let ids = [
        AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap(),
        AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap(),
    ];
    let tags = ["SyntheticAlpha#1000", "SyntheticBeta#2000"];
    let entries = ids
        .iter()
        .zip(tags)
        .map(|(id, tag)| {
            let paths = shared.account(id);
            fs::create_dir_all(paths.root()).unwrap();
            write_manifest_atomic(
                &paths.manifest(),
                &AccountManifest {
                    version: 1,
                    account_id: id.clone(),
                    role_id: 1,
                    battle_tag: tag.into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    updated_at: "2026-01-01T00:00:00Z".into(),
                },
            )
            .unwrap();
            let conn = rusqlite::Connection::open(paths.database()).unwrap();
            conn.execute_batch(
                "CREATE TABLE matches(match_id TEXT, season INTEGER, begin_ts INTEGER, hero_guid TEXT, result TEXT, game_mode TEXT, duration_sec INTEGER, raw_entry_json TEXT, bucket TEXT);
                 CREATE TABLE hero_dict(guid TEXT, role TEXT);
                 CREATE TABLE match_players(match_id TEXT, is_current_player INTEGER, raw_json TEXT);
                 CREATE TABLE match_details(match_id TEXT, raw_detail_json TEXT);",
            )
            .unwrap();
            AccountRegistryEntry {
                account_id: id.clone(),
                battle_tag: tag.into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                last_used_at: "2026-01-01T00:00:00Z".into(),
                credential_state: "ready".into(),
            }
        })
        .collect();
    write_registry_atomic(
        &shared.registry(),
        &AccountRegistry {
            version: 1,
            active_account_id: Some(ids[0].clone()),
            accounts: entries,
        },
    )
    .unwrap();
    dir
}

#[test]
fn screen_stage_runs_multi_account_pipeline_with_safe_aggregate_outputs() {
    let app = synthetic_app_data();
    let report = app.path().join("screen-report.json");
    let artifact = app.path().join("screen-artifact.json");
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(app.path())
        .args(["--stage", "screen", "--report"])
        .arg(&report)
        .arg("--artifact")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    for field in [
        "stage=screen",
        "completed=",
        "total=",
        "elapsed_ms=",
        "eta_ms=",
        "summary reason=worker_errors count=",
        "summary reason=cell_shadow count=",
        "summary reason=cell_outcome_only count=",
    ] {
        assert!(stderr.contains(field), "missing {field}: {stderr}");
    }
    for line in stderr
        .lines()
        .filter(|line| !line.contains("completed=1515 "))
    {
        assert!(!line.ends_with("eta_ms=0"), "premature zero ETA: {line}");
    }
    let report_json: serde_json::Value =
        serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    let artifact_json: serde_json::Value =
        serde_json::from_slice(&fs::read(artifact).unwrap()).unwrap();
    assert_eq!(
        report_json["training_config"]["validation"]["bootstrap_iterations"],
        100
    );
    assert_eq!(
        artifact_json["training_config"]["validation"]["bootstrap_iterations"],
        100
    );
    assert_eq!(report_json["dataset"]["account_datasets"], 2);
    assert!(report_json["shadow"].is_array());
    let combined = format!("{report_json}{artifact_json}{stderr}").to_ascii_lowercase();
    for forbidden in [
        "syntheticalpha",
        "syntheticbeta",
        "11111111-1111",
        "22222222-2222",
        "match_id",
        "app-data",
    ] {
        assert!(!combined.contains(forbidden), "leaked {forbidden}");
    }
}

fn promotion_report(stage: &str, seed: u64, eligible: bool) -> serde_json::Value {
    let training_stage = match stage {
        "screen_100" => TrainingStage::Screen100,
        "candidate_1000" => TrainingStage::Candidate1000,
        _ => TrainingStage::Final10000,
    };
    let prior_candidates = ["tank", "damage", "support"]
        .into_iter()
        .map(|role| {
            serde_json::from_value::<PromotedCandidate>(serde_json::json!({
                "role": role, "strategy": "none", "variant": "core",
                "final_multiplier": 1.0, "lambda_core": 0.02, "lambda_cohort": 0.05
            }))
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut config = if training_stage == TrainingStage::Screen100 {
        StabilityConfig::for_stage(training_stage)
    } else {
        StabilityConfig::for_stage_with_promoted(training_stage, prior_candidates)
    };
    config.seed = seed;
    let evaluation = evaluate_stability(&[], config);
    let (_, report) =
        build_training_outputs(&[], &TrainingAudit::default(), &evaluation, seed).unwrap();
    let mut value = serde_json::to_value(report).unwrap();
    if eligible {
        let mut candidates = Vec::new();
        for role in ["tank", "damage", "support"] {
            let cell = value["cells"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|cell| cell["role"] == role && cell["mode"] == "role_competitive")
                .unwrap();
            let strategy_report = &mut cell["metrics"]["strategy_reports"][0];
            strategy_report["eligible"] = serde_json::json!(true);
            strategy_report["decision"] = serde_json::json!("eligible");
            strategy_report["rejected_reasons"] = serde_json::json!([]);
            let strategy = strategy_report["strategy"].as_str().unwrap().to_owned();
            let variant = strategy_report["forward"]["variant"]
                .as_str()
                .unwrap()
                .to_owned();
            let multiplier = strategy_report["forward"]["final_multiplier"]
                .as_f64()
                .unwrap();
            let lambda_core = strategy_report["forward"]["lambda_core"].as_f64().unwrap();
            let lambda_cohort = strategy_report["forward"]["lambda_cohort"]
                .as_f64()
                .unwrap();
            let strategy_index = [
                "none",
                "directnegative",
                "heronormalized",
                "excesshealing",
                "resourceconversiondebt",
            ]
            .iter()
            .position(|value| *value == strategy)
            .unwrap();
            let variant_index = ["core", "role_full"]
                .iter()
                .position(|value| *value == variant)
                .unwrap();
            let multiplier_index = [1.0, 1.25, 1.5, 1.75, 2.0]
                .iter()
                .position(|value| *value == multiplier)
                .unwrap();
            let regularization_index = [(0.02, 0.05), (0.05, 0.10)]
                .iter()
                .position(|value| *value == (lambda_core, lambda_cohort))
                .unwrap();
            cell["eligible"] = serde_json::json!(true);
            cell["decision"] = serde_json::json!("eligible");
            cell["rejected_reasons"] = serde_json::json!([]);
            cell["model_spec_id"] = serde_json::json!({
                "strategy_index": strategy_index, "variant_index": variant_index,
                "multiplier_index": multiplier_index, "regularization_index": regularization_index
            });
            candidates.push(serde_json::json!({
                "role": role, "strategy": strategy, "variant": variant,
                "final_multiplier": multiplier, "lambda_core": lambda_core,
                "lambda_cohort": lambda_cohort
            }));
        }
        value["promotable_candidates"] = serde_json::Value::Array(candidates);
    }
    value["promotion_hash"] = serde_json::json!(recompute_promotion_hash(&value).unwrap());
    value
}

#[test]
fn candidate_stage_validates_promotion_and_reduces_actual_evaluation_work() {
    let app = synthetic_app_data();
    let promotion = app.path().join("promotion.json");
    fs::write(
        &promotion,
        serde_json::to_vec(&promotion_report("screen_100", 0x5eed, true)).unwrap(),
    )
    .unwrap();
    let report = app.path().join("candidate-report.json");
    let artifact = app.path().join("candidate-artifact.json");
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(app.path())
        .args(["--stage", "candidate", "--promote-from"])
        .arg(&promotion)
        .arg("--report")
        .arg(&report)
        .arg("--artifact")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("stage=candidate"));
    assert!(stderr.contains("total=3015"), "{stderr}");
    let report_json: serde_json::Value =
        serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(
        report_json["training_config"]["validation"]["bootstrap_iterations"],
        1_000
    );
    assert_eq!(report_json["promotion"]["candidate_count"], 3);
    let artifact_json: serde_json::Value =
        serde_json::from_slice(&fs::read(artifact).unwrap()).unwrap();
    let promoted = artifact_json["training_config"]["fit"]["promoted_candidates"]
        .as_array()
        .unwrap();
    assert_eq!(promoted.len(), 3);
    assert!(promoted.iter().all(|candidate| {
        candidate["strategy"] == "none"
            && candidate["variant"] == "core"
            && candidate["final_multiplier"] == 1.25
            && candidate["lambda_core"] == 0.02
            && candidate["lambda_cohort"] == 0.05
    }));
}

#[test]
fn final_stage_uses_ten_thousand_bootstraps_only_for_promoted_candidates() {
    let app = synthetic_app_data();
    let promotion = app.path().join("promotion.json");
    fs::write(
        &promotion,
        serde_json::to_vec(&promotion_report("candidate_1000", 0x5eed, true)).unwrap(),
    )
    .unwrap();
    let report = app.path().join("final-report.json");
    let artifact = app.path().join("final-artifact.json");
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(app.path())
        .args(["--stage", "final", "--promote-from"])
        .arg(&promotion)
        .arg("--report")
        .arg(&report)
        .arg("--artifact")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("stage=final"));
    assert!(stderr.contains("total=30015"), "{stderr}");
    let artifact: serde_json::Value = serde_json::from_slice(&fs::read(artifact).unwrap()).unwrap();
    assert_eq!(
        artifact["training_config"]["validation"]["bootstrap_iterations"],
        10_000
    );
    assert_eq!(
        artifact["training_config"]["fit"]["promoted_candidates"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn promotion_is_fail_closed_for_wrong_stage_seed_hash_schema_or_no_candidates() {
    let rehash = |mut value: serde_json::Value| {
        value["promotion_hash"] = serde_json::json!(recompute_promotion_hash(&value).unwrap());
        value
    };
    for (name, value) in [
        ("stage", promotion_report("candidate_1000", 0x5eed, true)),
        ("seed", promotion_report("screen_100", 9, true)),
        ("empty", promotion_report("screen_100", 0x5eed, false)),
        ("hash", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["config_hash"] = serde_json::json!("bad");
            value
        }),
        ("random-config-hash", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["config_hash"] = serde_json::json!("b".repeat(64));
            value
        }),
        ("random-promotion-hash", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["promotion_hash"] = serde_json::json!("c".repeat(64));
            value
        }),
        ("cell", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["cells"][0]["mode"] = serde_json::json!("role_quick");
            value
        }),
        ("eligible", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["cells"][0]["eligible"] = serde_json::json!(false);
            value
        }),
        ("config", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["training_config"]["fit"]["variants"][0] = serde_json::json!("role_full");
            value
        }),
        ("schema", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["schema_version"] = serde_json::json!(2);
            value
        }),
        ("spec", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["cells"][0]["model_spec_id"]["multiplier_index"] = serde_json::json!(4);
            value
        }),
        ("algorithm", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["algorithm"] = serde_json::json!("wrong");
            rehash(value)
        }),
        ("feature", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["feature_version"] = serde_json::json!("wrong");
            rehash(value)
        }),
        ("duplicate-cell", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["cells"][1] = value["cells"][0].clone();
            rehash(value)
        }),
        ("outcome-only", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["stadium_quick"]["decision"] = serde_json::json!("shadow");
            rehash(value)
        }),
        ("decision-mismatch", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["cells"][0]["decision"] = serde_json::json!("shadow");
            rehash(value)
        }),
        ("privacy", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["token"] = serde_json::json!("synthetic-secret");
            rehash(value)
        }),
        ("screen-with-promoted", {
            let mut value = promotion_report("screen_100", 0x5eed, true);
            value["training_config"]["fit"]["promoted_candidates"] =
                value["promotable_candidates"].clone();
            rehash(value)
        }),
    ] {
        let app = synthetic_app_data();
        let promotion = app.path().join(format!("{name}.json"));
        fs::write(&promotion, serde_json::to_vec(&value).unwrap()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
            .arg("--app-data-dir")
            .arg(app.path())
            .args(["--stage", "candidate", "--promote-from"])
            .arg(&promotion)
            .arg("--report")
            .arg(app.path().join("report.json"))
            .arg("--artifact")
            .arg(app.path().join("artifact.json"))
            .output()
            .unwrap();
        assert!(!output.status.success(), "{name}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "错误：上一阶段报告无效\n"
        );
    }
}

#[test]
fn candidate_and_final_reports_require_nonempty_prior_promoted_candidates() {
    use owstats_desktop_lib::career_mmr::training_report::validate_promotion_report;
    for stage in ["candidate_1000", "final_10000"] {
        let mut value = promotion_report(stage, 0x5eed, true);
        value["training_config"]["fit"]["promoted_candidates"] = serde_json::Value::Null;
        value["promotion_hash"] = serde_json::json!(recompute_promotion_hash(&value).unwrap());
        assert!(validate_promotion_report(&value, stage, 0x5eed).is_err());
    }
}

#[test]
fn existing_output_pair_is_rejected_unchanged_without_transaction_files() {
    let app = synthetic_app_data();
    let report = app.path().join("report.json");
    let artifact = app.path().join("artifact.json");
    fs::write(&report, b"old-report").unwrap();
    fs::write(&artifact, b"old-artifact").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(app.path())
        .args(["--stage", "screen", "--report"])
        .arg(&report)
        .arg("--artifact")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .ends_with("错误：输出文件已存在，拒绝覆盖\n"));
    assert_eq!(fs::read(report).unwrap(), b"old-report");
    assert_eq!(fs::read(artifact).unwrap(), b"old-artifact");
    let leftovers = fs::read_dir(app.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(".career-mmr-"))
        .collect::<Vec<_>>();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn output_pair_must_share_one_canonical_parent_directory() {
    let app = synthetic_app_data();
    let other = tempfile::tempdir().unwrap();
    let report = other.path().join("report.json");
    let artifact = app.path().join("artifact.json");
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(app.path())
        .args(["--stage", "screen", "--report"])
        .arg(&report)
        .arg("--artifact")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr, "错误：报告和模型产物必须位于同一目录\n");
    assert!(!stderr.contains(app.path().to_str().unwrap()));
    assert!(!report.exists());
    assert!(!artifact.exists());
}

#[test]
fn trainer_rejects_output_aliases_before_opening_sources() {
    let app = synthetic_app_data();
    let same = app.path().join("same.json");
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(app.path())
        .args(["--stage", "screen", "--report"])
        .arg(&same)
        .arg("--artifact")
        .arg(&same)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "错误：报告和模型产物不能使用同一路径\n"
    );
}

#[test]
fn trainer_refuses_to_overwrite_a_source_database() {
    let app = synthetic_app_data();
    let database = app
        .path()
        .join("accounts/11111111-1111-4111-8111-111111111111/owstats.sqlite3");
    let before = fs::read(&database).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_train_career_mmr"))
        .arg("--app-data-dir")
        .arg(app.path())
        .args(["--stage", "screen", "--report"])
        .arg(app.path().join("report.json"))
        .arg("--artifact")
        .arg(&database)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "错误：输入输出不能覆盖训练源数据库\n"
    );
    assert_eq!(fs::read(database).unwrap(), before);
}

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
        let total = iterations * 3 * 5 + 15;
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

    let failed = aggregate_progress_schedule_with_outcome(TrainingStage::Screen100, 137, 0, false);
    assert!(failed.last().unwrap().completed < failed.last().unwrap().total);
    assert_ne!(failed.last().unwrap().eta_ms, Some(0));
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
    assert_eq!(json["dataset_scope"], "single_dataset_research");
    assert_eq!(
        json["account_loo_status"],
        "coverage_limited_not_formal_multi_account"
    );
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
