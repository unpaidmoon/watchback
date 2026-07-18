use owstats_desktop_lib::{
    app_paths::SharedAppPaths,
    career_mmr::{
        performance_stability::{
            evaluate_stability_with_progress, CellDecision, PromotedCandidate, StabilityConfig,
            StabilityEvaluation, TrainingStage,
        },
        shadow::simulate_shadow_replay,
        training::build_multi_account_projection,
        training_report::{
            build_training_outputs, validate_promotion_report, validate_report_boundary,
        },
        training_sources::discover_training_sources,
    },
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
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
    validate_paths(&args)?;
    let projection =
        build_multi_account_projection(&sources).map_err(|_| "无法聚合多账号训练数据")?;
    let promoted_candidates =
        load_promoted_candidates(args.promote_from.as_deref(), args.stage, args.seed)?;
    let mut config = promoted_candidates.clone().map_or_else(
        || StabilityConfig::for_stage(args.stage.training_stage()),
        |candidates| {
            StabilityConfig::for_stage_with_promoted(args.stage.training_stage(), candidates)
        },
    );
    config.seed = args.seed;
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
    emit_evaluation_summary(&evaluation);
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

const FIXED_REJECTION_CODES: [&str; 26] = [
    "account_coverage_limited",
    "average_adjustment_above_0_10_mmr",
    "bootstrap_brier_ci_not_below_zero",
    "bootstrap_completion_below_90_percent",
    "bootstrap_iterations_zero",
    "cap_hit_rate_above_5_percent",
    "critical_coefficient_ci_crosses_zero",
    "critical_direction_stability_below_90_percent",
    "ece_worse_than_0_005",
    "final_weight_not_strictly_above_nonfinal",
    "hero_loo_mean_brier_delta_above_zero",
    "hero_loo_row_coverage_below_80_percent",
    "hero_loo_valid_groups_below_5",
    "hero_loo_worsened_fraction_above_25_percent",
    "improved_folds_below_4",
    "no_eligible_strategy",
    "no_promoted_candidate",
    "p95_single_feature_contribution_above_35_percent",
    "resource_debt_ci_upper_not_below_zero",
    "role_core_rows_below_500",
    "season_loo_mean_brier_delta_above_zero",
    "season_loo_row_coverage_below_80_percent",
    "season_loo_valid_groups_below_2",
    "season_loo_worsened_fraction_above_25_percent",
    "selection_degraded",
    "zero_reference_coefficient_unstable",
];

fn evaluation_summary(evaluation: &StabilityEvaluation) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::from([
        ("worker_errors", evaluation.worker_errors.len()),
        (
            "cell_shadow",
            evaluation
                .cells
                .iter()
                .filter(|cell| cell.decision == CellDecision::Shadow)
                .count(),
        ),
        (
            "cell_outcome_only",
            evaluation
                .cells
                .iter()
                .filter(|cell| cell.decision == CellDecision::OutcomeOnly)
                .count(),
        ),
        ("other_rejected_reason", 0),
    ]);
    for code in FIXED_REJECTION_CODES {
        counts.insert(code, 0);
    }
    for reason in evaluation
        .cells
        .iter()
        .flat_map(|cell| cell.rejected_reasons.iter())
    {
        let code = FIXED_REJECTION_CODES
            .iter()
            .copied()
            .find(|code| *code == reason)
            .unwrap_or("other_rejected_reason");
        *counts.entry(code).or_default() += 1;
    }
    counts
}

fn emit_evaluation_summary(evaluation: &StabilityEvaluation) {
    for (reason, count) in evaluation_summary(evaluation) {
        eprintln!("summary reason={reason} count={count}");
    }
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
    let bytes = read_pinned_promotion(path, || Ok(())).map_err(|_| "上一阶段报告无效")?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "上一阶段报告无效")?;
    let expected_stage = if stage == Stage::Candidate {
        "screen_100"
    } else {
        "candidate_1000"
    };
    let candidates =
        validate_promotion_report(&value, expected_stage, seed).map_err(|_| "上一阶段报告无效")?;
    Ok(Some(candidates))
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
    let report_parent = fs::canonicalize(args.report.parent().ok_or("输出目录无效")?)
        .map_err(|_| "输出目录不可写")?;
    let artifact_parent = fs::canonicalize(args.artifact.parent().ok_or("输出目录无效")?)
        .map_err(|_| "输出目录不可写")?;
    if report_parent != artifact_parent {
        return Err("报告和模型产物必须位于同一目录");
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
            return Err("输出文件已存在，拒绝覆盖");
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

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    #[cfg(windows)]
    size: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(any(windows, unix)))]
    size: u64,
}

fn file_identity(metadata: &fs::Metadata) -> FileIdentity {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        FileIdentity {
            creation_time: metadata.creation_time(),
            last_write_time: metadata.last_write_time(),
            size: metadata.file_size(),
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        FileIdentity {
            size: metadata.len(),
        }
    }
}

fn directory_identity(metadata: &fs::Metadata) -> FileIdentity {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        FileIdentity {
            creation_time: metadata.creation_time(),
            last_write_time: 0,
            size: 0,
        }
    }
    #[cfg(not(windows))]
    {
        file_identity(metadata)
    }
}

#[cfg(windows)]
fn open_pinned_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(windows))]
fn open_pinned_file(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new().read(true).open(path)
}

#[cfg(windows)]
fn open_pinned_directory(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(windows))]
fn open_pinned_directory(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new().read(true).open(path)
}

fn read_pinned_promotion(
    path: &Path,
    after_open: impl FnOnce() -> std::io::Result<()>,
) -> Result<Vec<u8>, &'static str> {
    let path_metadata = fs::symlink_metadata(path).map_err(|_| "上一阶段报告无效")?;
    if path_metadata.file_type().is_symlink()
        || is_reparse(&path_metadata)
        || !path_metadata.is_file()
    {
        return Err("上一阶段报告无效");
    }
    let canonical = fs::canonicalize(path).map_err(|_| "上一阶段报告无效")?;
    let mut file = open_pinned_file(path).map_err(|_| "上一阶段报告无效")?;
    let identity = file_identity(&file.metadata().map_err(|_| "上一阶段报告无效")?);
    after_open().map_err(|_| "上一阶段报告无效")?;
    revalidate_pinned_file(path, &canonical, identity)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "上一阶段报告无效")?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| "上一阶段报告无效")?;
    if file_identity(&file.metadata().map_err(|_| "上一阶段报告无效")?) != identity {
        return Err("上一阶段报告无效");
    }
    revalidate_pinned_file(path, &canonical, identity)?;
    Ok(bytes)
}

fn revalidate_pinned_file(
    path: &Path,
    canonical: &Path,
    identity: FileIdentity,
) -> Result<(), &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "上一阶段报告无效")?;
    if metadata.file_type().is_symlink()
        || is_reparse(&metadata)
        || !metadata.is_file()
        || file_identity(&metadata) != identity
        || fs::canonicalize(path).map_err(|_| "上一阶段报告无效")? != canonical
    {
        return Err("上一阶段报告无效");
    }
    Ok(())
}

struct PinnedOutputDirectory {
    canonical: PathBuf,
    identity: FileIdentity,
    _guard: fs::File,
}

impl PinnedOutputDirectory {
    fn open(parent: &Path) -> Result<Self, &'static str> {
        let metadata = fs::symlink_metadata(parent).map_err(|_| "输出目录不可写")?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
            return Err("输出目录不安全");
        }
        let canonical = fs::canonicalize(parent).map_err(|_| "输出目录不可写")?;
        let guard = open_pinned_directory(parent).map_err(|_| "输出目录不安全")?;
        let guard_metadata = guard.metadata().map_err(|_| "输出目录不安全")?;
        if is_reparse(&guard_metadata) || !guard_metadata.is_dir() {
            return Err("输出目录不安全");
        }
        let identity = directory_identity(&guard_metadata);
        let pinned = Self {
            canonical,
            identity,
            _guard: guard,
        };
        pinned.revalidate(parent)?;
        Ok(pinned)
    }

    fn revalidate(&self, parent: &Path) -> Result<(), &'static str> {
        let metadata = fs::symlink_metadata(parent).map_err(|_| "输出目录不安全")?;
        if metadata.file_type().is_symlink()
            || is_reparse(&metadata)
            || !metadata.is_dir()
            || directory_identity(&metadata) != self.identity
            || fs::canonicalize(parent).map_err(|_| "输出目录不安全")? != self.canonical
        {
            return Err("输出目录不安全");
        }
        Ok(())
    }
}

fn write_pair_atomic(
    report: &Path,
    report_bytes: &[u8],
    artifact: &Path,
    artifact_bytes: &[u8],
) -> Result<(), &'static str> {
    write_pair_atomic_with_hook(report, report_bytes, artifact, artifact_bytes, |_| Ok(()))
}

fn write_pair_atomic_with_hook(
    report: &Path,
    report_bytes: &[u8],
    artifact: &Path,
    artifact_bytes: &[u8],
    mut hook: impl FnMut(&str) -> std::io::Result<()>,
) -> Result<(), &'static str> {
    let parent = report.parent().ok_or("输出目录无效")?;
    let artifact_parent = artifact.parent().ok_or("输出目录无效")?;
    if fs::canonicalize(parent).map_err(|_| "输出目录不可写")?
        != fs::canonicalize(artifact_parent).map_err(|_| "输出目录不可写")?
    {
        return Err("报告和模型产物必须位于同一目录");
    }
    let pinned = PinnedOutputDirectory::open(parent)?;
    hook("after_pin").map_err(|_| "输出目录在写入前发生变化")?;
    pinned.revalidate(parent)?;
    let report = pinned
        .canonical
        .join(report.file_name().ok_or("输出路径无效")?);
    let artifact = pinned
        .canonical
        .join(artifact.file_name().ok_or("输出路径无效")?);
    ensure_missing(&report)?;
    ensure_missing(&artifact)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "无法创建安全临时输出")?
        .as_nanos();
    let temp_path = |target: &Path, suffix: &str| {
        target.with_file_name(format!(".career-mmr-{}-{nonce}.tmp", suffix))
    };
    let report_temp = temp_path(&report, "report");
    let artifact_temp = temp_path(&artifact, "artifact");
    let mut artifact_committed = false;
    let mut report_committed = false;
    let mut artifact_guard = None;
    let result = (|| -> Result<(), &'static str> {
        write_synced_new(&report_temp, report_bytes)?;
        write_synced_new(&artifact_temp, artifact_bytes)?;
        hook("after_temps").map_err(|_| "输出目录在提交前发生变化")?;
        pinned.revalidate(parent)?;
        ensure_missing(&report)?;
        ensure_missing(&artifact)?;
        pinned.revalidate(parent)?;
        if fs::rename(&artifact_temp, &artifact).is_err() {
            return Err("无法提交模型产物");
        }
        artifact_committed = true;
        artifact_guard = Some(open_pinned_file(&artifact).map_err(|_| "无法锁定已提交的模型产物")?);
        hook("after_artifact").map_err(|_| "输出目录在提交期间发生变化")?;
        pinned.revalidate(parent)?;
        ensure_missing(&report)?;
        pinned.revalidate(parent)?;
        fs::rename(&report_temp, &report).map_err(|_| "无法提交训练报告")?;
        report_committed = true;
        sync_parent(&report)?;
        Ok(())
    })();
    if result.is_err() {
        drop(artifact_guard.take());
        let _ = fs::remove_file(&report_temp);
        let _ = fs::remove_file(&artifact_temp);
        if report_committed && fs::remove_file(&report).is_err() {
            return Err("严重错误：输出回滚不完整");
        }
        if artifact_committed && fs::remove_file(&artifact).is_err() {
            return Err("严重错误：输出回滚不完整");
        }
    }
    result
}

fn ensure_missing(path: &Path) -> Result<(), &'static str> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err("输出文件已存在，拒绝覆盖"),
        Err(_) => Err("无法确认输出路径状态"),
    }
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

#[cfg(test)]
mod io_race_tests {
    use super::*;

    #[test]
    fn evaluation_summary_uses_only_fixed_reason_codes_and_counts() {
        let mut config = StabilityConfig::for_stage(TrainingStage::Screen100);
        config.bootstrap_iterations = 0;
        let mut evaluation =
            owstats_desktop_lib::career_mmr::performance_stability::evaluate_stability(&[], config);
        evaluation.worker_errors = vec!["private-player-guid-path".into()];
        evaluation.cells[0].decision = CellDecision::OutcomeOnly;
        evaluation.cells[0].rejected_reasons = vec![
            "role_core_rows_below_500".into(),
            "private-player-guid-path".into(),
        ];

        let summary = evaluation_summary(&evaluation);
        assert_eq!(summary["worker_errors"], 1);
        assert_eq!(summary["cell_outcome_only"], 1);
        assert!(summary["role_core_rows_below_500"] >= 1);
        assert!(summary["other_rejected_reason"] >= 1);
        assert!(!summary.keys().any(|code| code.contains("private")));
        assert!(!summary.keys().any(|code| code.contains("guid")));
        assert!(!summary.keys().any(|code| code.contains("path")));
    }

    #[test]
    fn promotion_regular_file_replacement_after_open_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let promotion = dir.path().join("promotion.json");
        let replacement = dir.path().join("replacement.json");
        let moved = dir.path().join("moved.json");
        fs::write(&promotion, b"original").unwrap();
        fs::write(&replacement, b"replacement").unwrap();
        let result = read_pinned_promotion(&promotion, || {
            fs::rename(&promotion, &moved)?;
            fs::rename(&replacement, &promotion)
        });
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn promotion_symlink_replacement_after_open_fails_closed() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let promotion = dir.path().join("promotion.json");
        let replacement = dir.path().join("replacement.json");
        let moved = dir.path().join("moved.json");
        fs::write(&promotion, b"original").unwrap();
        fs::write(&replacement, b"replacement").unwrap();
        let result = read_pinned_promotion(&promotion, || {
            fs::rename(&promotion, &moved)?;
            symlink(&replacement, &promotion)
        });
        assert!(result.is_err());
    }

    #[test]
    fn output_parent_replacement_after_pin_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("outputs");
        let moved = root.path().join("moved");
        fs::create_dir(&parent).unwrap();
        let report = parent.join("report.json");
        let artifact = parent.join("artifact.json");
        let result =
            write_pair_atomic_with_hook(&report, b"report", &artifact, b"artifact", |stage| {
                if stage == "after_pin" {
                    fs::rename(&parent, &moved)?;
                    fs::create_dir(&parent)?;
                }
                Ok(())
            });
        assert!(result.is_err());
        assert!(!report.exists());
        assert!(!artifact.exists());
    }

    #[test]
    fn output_collision_after_temp_creation_leaves_no_writer_half_or_temp() {
        let dir = tempfile::tempdir().unwrap();
        let report = dir.path().join("report.json");
        let artifact = dir.path().join("artifact.json");
        let result =
            write_pair_atomic_with_hook(&report, b"report", &artifact, b"artifact", |stage| {
                if stage == "after_temps" {
                    fs::write(&report, b"racer")?;
                }
                Ok(())
            });
        assert!(result.is_err());
        assert_eq!(fs::read(&report).unwrap(), b"racer");
        assert!(!artifact.exists());
        assert!(fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .all(|name| !name.starts_with(".career-mmr-")));
    }

    #[test]
    fn failure_after_first_rename_removes_the_first_formal_output() {
        let dir = tempfile::tempdir().unwrap();
        let report = dir.path().join("report.json");
        let artifact = dir.path().join("artifact.json");
        let result =
            write_pair_atomic_with_hook(&report, b"report", &artifact, b"artifact", |stage| {
                if stage == "after_artifact" {
                    return Err(std::io::Error::other("synthetic stop"));
                }
                Ok(())
            });
        assert!(result.is_err());
        assert!(!report.exists());
        assert!(!artifact.exists());
        assert!(fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .all(|name| !name.starts_with(".career-mmr-")));
    }

    #[test]
    fn racer_report_after_artifact_commit_is_preserved_and_writer_artifact_is_rolled_back() {
        let dir = tempfile::tempdir().unwrap();
        let report = dir.path().join("report.json");
        let artifact = dir.path().join("artifact.json");
        let result =
            write_pair_atomic_with_hook(&report, b"report", &artifact, b"artifact", |stage| {
                if stage == "after_artifact" {
                    fs::write(&report, b"racer")?;
                }
                Ok(())
            });
        assert!(result.is_err());
        assert_eq!(fs::read(&report).unwrap(), b"racer");
        assert!(!artifact.exists());
        assert!(fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .all(|name| !name.starts_with(".career-mmr-")));
    }

    #[test]
    fn parent_change_attempt_after_artifact_commit_rolls_back_writer_artifact() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("outputs");
        let moved = root.path().join("moved");
        fs::create_dir(&parent).unwrap();
        let report = parent.join("report.json");
        let artifact = parent.join("artifact.json");
        let result =
            write_pair_atomic_with_hook(&report, b"report", &artifact, b"artifact", |stage| {
                if stage == "after_artifact" {
                    fs::rename(&parent, &moved)?;
                    fs::create_dir(&parent)?;
                }
                Ok(())
            });
        assert!(result.is_err());
        assert!(!report.exists());
        assert!(!artifact.exists());
        assert!(fs::read_dir(&parent)
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .all(|name| !name.starts_with(".career-mmr-")));
    }
}
