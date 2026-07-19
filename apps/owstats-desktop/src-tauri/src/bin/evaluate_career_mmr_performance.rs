use owstats_desktop_lib::career_mmr::{
    performance_stability::{evaluate_stability, StabilityConfig, StabilityEvaluation},
    training::{build_training_projection, TrainingAudit},
};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::{path::PathBuf, time::Instant};

#[derive(Debug)]
struct CliArgs {
    database: PathBuf,
    bootstrap_iterations: usize,
    seed: u64,
}

#[derive(Serialize)]
struct AlgorithmConfig {
    bootstrap_iterations: usize,
    seed: u64,
    model_fit_steps: usize,
}

#[derive(Serialize)]
struct EvaluationReport<'a> {
    audit: &'a TrainingAudit,
    dataset_scope: &'static str,
    account_loo_status: &'static str,
    algorithm_config: AlgorithmConfig,
    elapsed_ms: u128,
    #[serde(flatten)]
    evaluation: StabilityEvaluation,
}

fn parse_args() -> Result<CliArgs, &'static str> {
    let mut args = std::env::args_os().skip(1);
    let database = args.next().map(PathBuf::from).ok_or(
        "用法：evaluate_career_mmr_performance <只读SQLite路径> [--bootstrap-iterations N] [--seed N]",
    )?;
    let mut parsed = CliArgs {
        database,
        bootstrap_iterations: 1000,
        seed: StabilityConfig::default().seed,
    };
    while let Some(option) = args.next() {
        let value = args.next().ok_or("命令行选项缺少数值")?;
        let value = value.to_str().ok_or("命令行数值必须是有效文本")?;
        match option.to_str() {
            Some("--bootstrap-iterations") => {
                parsed.bootstrap_iterations = value
                    .parse()
                    .map_err(|_| "bootstrap 迭代次数必须是非负整数")?;
            }
            Some("--seed") => {
                parsed.seed = value.parse().map_err(|_| "随机种子必须是非负整数")?;
            }
            _ => return Err("存在无法识别的命令行选项"),
        }
    }
    Ok(parsed)
}

fn run() -> Result<String, &'static str> {
    let args = parse_args()?;
    let conn = Connection::open_with_flags(
        &args.database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "无法只读打开数据库")?;
    conn.pragma_update(None, "query_only", true)
        .map_err(|_| "无法启用数据库只读保护")?;
    let projection = build_training_projection(&conn).map_err(|_| "无法构建训练数据聚合")?;
    let started = Instant::now();
    let config = StabilityConfig {
        bootstrap_iterations: args.bootstrap_iterations,
        seed: args.seed,
        ..StabilityConfig::default()
    };
    let algorithm_config = AlgorithmConfig {
        bootstrap_iterations: config.bootstrap_iterations,
        seed: config.seed,
        model_fit_steps: owstats_desktop_lib::career_mmr::performance_model::MODEL_FIT_STEPS,
    };
    let evaluation = evaluate_stability(&projection.rows, config);
    let report = EvaluationReport {
        audit: projection.public_audit(),
        dataset_scope: "single_dataset_research",
        account_loo_status: "coverage_limited_not_formal_multi_account",
        algorithm_config,
        elapsed_ms: started.elapsed().as_millis(),
        evaluation,
    };
    serde_json::to_string_pretty(&report).map_err(|_| "无法序列化聚合评估结果")
}

fn main() {
    match run() {
        Ok(json) => println!("{json}"),
        Err(message) => {
            eprintln!("错误：{message}");
            std::process::exit(1);
        }
    }
}
