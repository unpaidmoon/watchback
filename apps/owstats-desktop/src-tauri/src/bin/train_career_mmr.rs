use owstats_desktop_lib::{
    app_paths::SharedAppPaths,
    career_mmr::{
        input::Role,
        performance_forward::ModelVariant,
        performance_stability::{
            evaluate_stability_with_progress, PromotedCandidate, StabilityConfig, TrainingStage,
        },
        shadow::simulate_shadow_replay,
        training::build_multi_account_projection,
        training_report::{build_training_outputs, validate_report_boundary},
        training_sources::discover_training_sources,
    },
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Screen,
    Candidate,
    Final,
}

impl Stage {
    fn training_stage(self) -> TrainingStage {
        match self {
            Self::Screen => TrainingStage::Screen100,
            Self::Candidate => TrainingStage::Candidate1000,
            Self::Final => TrainingStage::Final10000,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Candidate => "candidate",
            Self::Final => "final",
        }
    }
}

#[derive(Debug)]
struct CliArgs {
    app_data_dir: PathBuf,
    stage: Stage,
    report: PathBuf,
    artifact: PathBuf,
    promote_from: Option<PathBuf>,
    seed: u64,
}

fn parse_args_from(args: impl IntoIterator<Item = OsString>) -> Result<CliArgs, &'static str> {
    let mut args = args.into_iter();
    let mut seen = BTreeSet::new();
    let mut app_data_dir = None;
    let mut stage = None;
    let mut report = None;
    let mut artifact = None;
    let mut promote_from = None;
    let mut seed = None;
    while let Some(option) = args.next() {
        let option = option.to_str().ok_or("命令行选项必须是有效文本")?;
        if !matches!(
            option,
            "--app-data-dir" | "--stage" | "--report" | "--artifact" | "--promote-from" | "--seed"
        ) {
            return Err("存在无法识别的命令行选项");
        }
        if !seen.insert(option.to_owned()) {
            return Err("命令行选项不能重复");
        }
        let value = args.next().ok_or("命令行选项缺少数值")?;
        if value.to_str().is_some_and(|value| value.starts_with("--")) {
            return Err("命令行选项缺少数值");
        }
        match option {
            "--app-data-dir" => app_data_dir = Some(PathBuf::from(value)),
            "--stage" => {
                stage = Some(match value.to_str() {
                    Some("screen") => Stage::Screen,
                    Some("candidate") => Stage::Candidate,
                    Some("final") => Stage::Final,
                    _ => return Err("训练阶段必须是 screen、candidate 或 final"),
                })
            }
            "--report" => report = Some(PathBuf::from(value)),
            "--artifact" => artifact = Some(PathBuf::from(value)),
            "--promote-from" => promote_from = Some(PathBuf::from(value)),
            "--seed" => {
                seed = Some(
                    value
                        .to_str()
                        .ok_or("随机种子必须是非负整数")?
                        .parse()
                        .map_err(|_| "随机种子必须是非负整数")?,
                )
            }
            _ => unreachable!(),
        }
    }
    let stage = stage.ok_or("缺少必需的命令行选项")?;
    if matches!(stage, Stage::Candidate | Stage::Final) && promote_from.is_none() {
        return Err("candidate 和 final 阶段必须提供上一阶段报告");
    }
    if stage == Stage::Screen && promote_from.is_some() {
        return Err("screen 阶段不能提供上一阶段报告");
    }
    Ok(CliArgs {
        app_data_dir: app_data_dir.ok_or("缺少必需的命令行选项")?,
        stage,
        report: report.ok_or("缺少必需的命令行选项")?,
        artifact: artifact.ok_or("缺少必需的命令行选项")?,
        promote_from,
        seed: seed.unwrap_or(0x5eed),
    })
}

#[derive(Serialize)]
struct ShadowAggregate {
    account_index: usize,
    role_matches: BTreeMap<String, usize>,
    total_cap_hits: usize,
}

fn run() -> Result<(), &'static str> {
    let args = parse_args_from(std::env::args_os().skip(1))?;
    validate_paths(&args)?;
    let shared = SharedAppPaths::from_app_data_dir(&args.app_data_dir);
    let sources = discover_training_sources(&shared).map_err(|_| "无法发现训练账号数据库")?;
    if sources.is_empty() {
        return Err("没有可用的训练账号数据库");
    }
    if sources.iter().any(|source| {
        source.conflicts_with_output(&args.report)
            || source.conflicts_with_output(&args.artifact)
            || args
                .promote_from
                .as_deref()
                .is_some_and(|path| source.conflicts_with_output(path))
    }) {
        return Err("输入输出不能覆盖训练源数据库");
    }
    let projection =
        build_multi_account_projection(&sources).map_err(|_| "无法聚合多账号训练数据")?;
    let promoted_candidates =
        load_promoted_candidates(args.promote_from.as_deref(), args.stage, args.seed)?;
    let mut config = StabilityConfig::for_stage(args.stage.training_stage());
    config.seed = args.seed;
    config.promoted_candidates = promoted_candidates.clone();
    let stage_label = args.stage.label();
    let evaluation = evaluate_stability_with_progress(&projection.rows, config, |event| {
        eprintln!(
            "stage={} completed={} total={} elapsed_ms={} eta_ms={}",
            stage_label,
            event.completed,
            event.total,
            event.elapsed_ms,
            event
                .eta_ms
                .map_or_else(|| "unknown".into(), |value| value.to_string())
        );
    });
    let (artifact, report) = build_training_outputs(
        &projection.rows,
        projection.public_audit(),
        &evaluation,
        args.seed,
    )
    .map_err(|_| "无法生成训练产物")?;
    let mut shadow = Vec::new();
    let account_groups = projection
        .rows
        .iter()
        .map(|row| row.account_group.clone())
        .collect::<BTreeSet<_>>();
    for (account_index, account_group) in account_groups.into_iter().enumerate() {
        let rows = projection
            .rows
            .iter()
            .filter(|row| row.account_group == account_group)
            .cloned()
            .collect::<Vec<_>>();
        let summary =
            simulate_shadow_replay(&rows, &artifact).map_err(|_| "无法完成全账号影子重放")?;
        shadow.push(ShadowAggregate {
            account_index: account_index + 1,
            role_matches: summary
                .roles
                .into_iter()
                .map(|role| {
                    (
                        format!("{:?}", role.role).to_ascii_lowercase(),
                        role.matches,
                    )
                })
                .collect(),
            total_cap_hits: summary
                .cap_hits
                .values()
                .map(|hits| hits.upper + hits.lower)
                .sum(),
        });
    }
    // Empty datasets still get one aggregate shadow row per registered source.
    while shadow.len() < sources.len() {
        shadow.push(ShadowAggregate {
            account_index: shadow.len() + 1,
            role_matches: BTreeMap::new(),
            total_cap_hits: 0,
        });
    }
    let mut report_value = serde_json::to_value(report).map_err(|_| "无法序列化训练报告")?;
    report_value
        .as_object_mut()
        .ok_or("训练报告结构无效")?
        .insert(
            "shadow".into(),
            serde_json::to_value(shadow).map_err(|_| "无法序列化影子结果")?,
        );
    if let Some(candidates) = promoted_candidates.as_ref() {
        report_value
            .as_object_mut()
            .ok_or("训练报告结构无效")?
            .insert(
                "promotion".into(),
                serde_json::json!({"candidate_count": candidates.len()}),
            );
    }
    let artifact_value = serde_json::to_value(artifact).map_err(|_| "无法序列化模型产物")?;
    validate_report_boundary(&report_value).map_err(|_| "训练报告包含不允许的字段")?;
    validate_report_boundary(&artifact_value).map_err(|_| "模型产物包含不允许的字段")?;
    let report_bytes =
        serde_json::to_vec_pretty(&report_value).map_err(|_| "无法序列化训练报告")?;
    let artifact_bytes =
        serde_json::to_vec_pretty(&artifact_value).map_err(|_| "无法序列化模型产物")?;
    write_pair_atomic(&args.report, &report_bytes, &args.artifact, &artifact_bytes)?;
    Ok(())
}

fn load_promoted_candidates(
    path: Option<&Path>,
    stage: Stage,
    seed: u64,
) -> Result<Option<Vec<PromotedCandidate>>, &'static str> {
    if stage == Stage::Screen {
        if path.is_some() {
            return Err("screen 阶段不能提供上一阶段报告");
        }
        return Ok(None);
    }
    let path = path.ok_or("candidate 和 final 阶段必须提供上一阶段报告")?;
    let metadata = fs::symlink_metadata(path).map_err(|_| "上一阶段报告无效")?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("上一阶段报告无效");
    }
    let bytes = fs::read(path).map_err(|_| "上一阶段报告无效")?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "上一阶段报告无效")?;
    let expected_stage = if stage == Stage::Candidate {
        "screen_100"
    } else {
        "candidate_1000"
    };
    let hash = value
        .get("config_hash")
        .and_then(|value| value.as_str())
        .ok_or("上一阶段报告无效")?;
    if value.get("schema_version").and_then(|value| value.as_u64()) != Some(3)
        || value.get("seed").and_then(|value| value.as_u64()) != Some(seed)
        || value
            .pointer("/training_config/validation/stage")
            .and_then(|value| value.as_str())
            != Some(expected_stage)
        || hash.len() != 64
        || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value.pointer("/training_config/fit/variants")
            != Some(&serde_json::json!(["core", "role_full"]))
        || value.pointer("/training_config/fit/final_hit_multipliers")
            != Some(&serde_json::json!([1.0, 1.25, 1.5, 1.75, 2.0]))
        || value.pointer("/training_config/fit/regularization_candidates")
            != Some(&serde_json::json!([
                {"lambda_core": 0.02, "lambda_cohort": 0.05},
                {"lambda_core": 0.05, "lambda_cohort": 0.10}
            ]))
    {
        return Err("上一阶段报告无效");
    }
    let cells = value
        .get("cells")
        .and_then(|value| value.as_array())
        .ok_or("上一阶段报告无效")?;
    let mut by_role = BTreeMap::<Role, PromotedCandidate>::new();
    for cell in cells
        .iter()
        .filter(|cell| cell.get("eligible").and_then(|value| value.as_bool()) == Some(true))
    {
        let role = match cell.get("role").and_then(|value| value.as_str()) {
            Some("tank") => Role::Tank,
            Some("damage") => Role::Damage,
            Some("support") => Role::Support,
            _ => return Err("上一阶段报告无效"),
        };
        let reports = cell
            .pointer("/metrics/strategy_reports")
            .and_then(|value| value.as_array())
            .ok_or("上一阶段报告无效")?;
        let spec = cell.get("model_spec_id").ok_or("上一阶段报告无效")?;
        let strategy_index = index(spec, "strategy_index", 5)?;
        let variant_index = index(spec, "variant_index", 2)?;
        let multiplier_index = index(spec, "multiplier_index", 5)?;
        let regularization_index = index(spec, "regularization_index", 2)?;
        let strategies = [
            "none",
            "directnegative",
            "heronormalized",
            "excesshealing",
            "resourceconversiondebt",
        ];
        let variants = [ModelVariant::Core, ModelVariant::RoleFull];
        let multipliers = [1.0, 1.25, 1.5, 1.75, 2.0];
        let regularization = [(0.02, 0.05), (0.05, 0.10)];
        let strategy = strategies[strategy_index];
        let variant = variants[variant_index];
        let final_multiplier = multipliers[multiplier_index];
        let (lambda_core, lambda_cohort) = regularization[regularization_index];
        let report = reports
            .iter()
            .find(|report| {
                report.get("strategy").and_then(|value| value.as_str()) == Some(strategy)
            })
            .filter(|report| report.get("eligible").and_then(|value| value.as_bool()) == Some(true))
            .ok_or("上一阶段报告无效")?;
        if report
            .pointer("/forward/variant")
            .and_then(|value| value.as_str())
            != Some(match variant {
                ModelVariant::Core => "core",
                ModelVariant::RoleFull => "role_full",
            })
            || finite_number(report, "/forward/final_multiplier")? != final_multiplier
            || finite_number(report, "/forward/lambda_core")? != lambda_core
            || finite_number(report, "/forward/lambda_cohort")? != lambda_cohort
        {
            return Err("上一阶段报告无效");
        }
        let candidate = PromotedCandidate {
            role,
            strategy: strategy.into(),
            variant,
            final_multiplier,
            lambda_core,
            lambda_cohort,
        };
        if by_role
            .get(&role)
            .is_some_and(|existing| existing != &candidate)
        {
            return Err("上一阶段报告无效");
        }
        by_role.insert(role, candidate);
    }
    let candidates = by_role.into_values().collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err("上一阶段报告无效");
    }
    Ok(Some(candidates))
}

fn index(value: &serde_json::Value, field: &str, length: usize) -> Result<usize, &'static str> {
    let index = value
        .get(field)
        .and_then(|value| value.as_u64())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("上一阶段报告无效")?;
    (index < length).then_some(index).ok_or("上一阶段报告无效")
}

fn finite_number(value: &serde_json::Value, pointer: &str) -> Result<f64, &'static str> {
    value
        .pointer(pointer)
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
        .ok_or("上一阶段报告无效")
}

fn validate_paths(args: &CliArgs) -> Result<(), &'static str> {
    let same = |left: &Path, right: &Path| {
        let normalized = |path: &Path| {
            path.parent()
                .and_then(|parent| fs::canonicalize(parent).ok())
                .zip(path.file_name().map(|name| name.to_owned()))
                .map(|(parent, name)| parent.join(name))
        };
        left == right
            || normalized(left) == normalized(right)
            || (left.exists()
                && right.exists()
                && fs::canonicalize(left).ok() == fs::canonicalize(right).ok())
    };
    if same(&args.report, &args.artifact) {
        return Err("报告和模型产物不能使用同一路径");
    }
    if args
        .promote_from
        .as_deref()
        .is_some_and(|input| same(input, &args.report) || same(input, &args.artifact))
    {
        return Err("输入报告不能与输出路径冲突");
    }
    for output in [&args.report, &args.artifact] {
        let parent = output.parent().ok_or("输出目录无效")?;
        let metadata = fs::symlink_metadata(parent).map_err(|_| "输出目录不可写")?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
            return Err("输出目录不安全");
        }
        if output.exists() {
            let metadata = fs::symlink_metadata(output).map_err(|_| "输出路径无效")?;
            if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
                return Err("输出路径不安全");
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn write_pair_atomic(
    report: &Path,
    report_bytes: &[u8],
    artifact: &Path,
    artifact_bytes: &[u8],
) -> Result<(), &'static str> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "无法创建安全临时输出")?
        .as_nanos();
    let temp_path = |target: &Path, suffix: &str| {
        target.with_file_name(format!(".career-mmr-{}-{nonce}.tmp", suffix))
    };
    let report_temp = temp_path(report, "report");
    let artifact_temp = temp_path(artifact, "artifact");
    let report_backup = temp_path(report, "report-backup");
    let artifact_backup = temp_path(artifact, "artifact-backup");
    let result = (|| -> Result<(), &'static str> {
        write_synced_new(&report_temp, report_bytes)?;
        write_synced_new(&artifact_temp, artifact_bytes)?;
        let report_existed = report.exists();
        let artifact_existed = artifact.exists();
        if artifact_existed {
            fs::rename(artifact, &artifact_backup).map_err(|_| "无法准备模型产物原子替换")?;
        }
        if report_existed && fs::rename(report, &report_backup).is_err() {
            if artifact_existed {
                let _ = fs::rename(&artifact_backup, artifact);
            }
            return Err("无法准备训练报告原子替换");
        }
        if fs::rename(&artifact_temp, artifact).is_err() {
            if report_existed {
                let _ = fs::rename(&report_backup, report);
            }
            if artifact_existed {
                let _ = fs::rename(&artifact_backup, artifact);
            }
            return Err("无法提交模型产物");
        }
        if fs::rename(&report_temp, report).is_err() {
            let _ = fs::remove_file(artifact);
            if report_existed {
                let _ = fs::rename(&report_backup, report);
            }
            if artifact_existed {
                let _ = fs::rename(&artifact_backup, artifact);
            }
            return Err("无法提交训练报告");
        }
        sync_parent(report)?;
        if report.parent() != artifact.parent() {
            sync_parent(artifact)?;
        }
        if report_existed {
            fs::remove_file(&report_backup).map_err(|_| "无法清理训练报告事务备份")?;
        }
        if artifact_existed {
            fs::remove_file(&artifact_backup).map_err(|_| "无法清理模型产物事务备份")?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&report_temp);
        let _ = fs::remove_file(&artifact_temp);
        if report_backup.exists() && !report.exists() {
            let _ = fs::rename(&report_backup, report);
        }
        if artifact_backup.exists() && !artifact.exists() {
            let _ = fs::rename(&artifact_backup, artifact);
        }
        let _ = fs::remove_file(&report_backup);
        let _ = fs::remove_file(&artifact_backup);
    }
    result
}

fn write_synced_new(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "无法创建安全临时输出")?;
    file.write_all(bytes).map_err(|_| "无法写入临时输出")?;
    file.flush().map_err(|_| "无法刷新临时输出")?;
    file.sync_all().map_err(|_| "无法同步临时输出")
}

fn sync_parent(path: &Path) -> Result<(), &'static str> {
    #[cfg(not(windows))]
    fs::File::open(path.parent().ok_or("输出目录无效")?)
        .and_then(|file| file.sync_all())
        .map_err(|_| "无法同步输出目录")?;
    #[cfg(windows)]
    let _ = path;
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("错误：{message}");
        std::process::exit(1);
    }
}
