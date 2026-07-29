use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use owstats_desktop_lib::account_context::AccountContextManager;
use owstats_desktop_lib::ai_roast::client::{
    take_validation_failure_category, DeepSeekRoastClient,
};
use owstats_desktop_lib::ai_roast::coordinator::{AiRoastCoordinator, CapturedRoastRequest};
use owstats_desktop_lib::ai_roast::knowledge::{embedded_knowledge, knowledge_for_match};
use owstats_desktop_lib::ai_roast::snapshot::build_roast_snapshot;
use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::deepseek_key::DeepSeekKeyManager;
use owstats_desktop_lib::{get_match_detail, MatchDetailView, OwDatabase};
use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};

#[derive(Clone)]
struct Candidate {
    match_id: String,
    hero_signature: String,
    simple_priority: bool,
}

struct CandidateSet {
    standard: Vec<Candidate>,
    arena: Vec<Candidate>,
}

#[derive(Debug, Clone, Copy)]
enum FailureCategory {
    Arguments,
    Account,
    Key,
    Backup,
    Candidates,
    Generate,
    GenerateTimeout,
    GenerateInvalidOutput,
    GenerateEvidence,
    GenerateNumber,
    GenerateClaim,
    GenerateJson,
    GenerateSections,
    GenerateText,
    GenerateSafety,
    GenerateTextTotal,
    GenerateTextField,
    GenerateTextEmpty,
    GenerateNumberPercent,
    GenerateNumberOther,
    GenerateResponseShape,
    GenerateResponseLength,
    GenerateResponseFilter,
    GenerateFinishReason,
    GenerateNumberAscii,
    GenerateNumberChinese,
    GenerateService,
    ValidateStructure,
}

impl FailureCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Arguments => "arguments",
            Self::Account => "account",
            Self::Key => "key",
            Self::Backup => "backup",
            Self::Candidates => "candidates",
            Self::Generate => "generate",
            Self::GenerateTimeout => "generate_timeout",
            Self::GenerateInvalidOutput => "generate_invalid_output",
            Self::GenerateEvidence => "generate_evidence",
            Self::GenerateNumber => "generate_number",
            Self::GenerateClaim => "generate_claim",
            Self::GenerateJson => "generate_json",
            Self::GenerateSections => "generate_sections",
            Self::GenerateText => "generate_text",
            Self::GenerateSafety => "generate_safety",
            Self::GenerateTextTotal => "generate_text_total",
            Self::GenerateTextField => "generate_text_field",
            Self::GenerateTextEmpty => "generate_text_empty",
            Self::GenerateNumberPercent => "generate_number_percent",
            Self::GenerateNumberOther => "generate_number_other",
            Self::GenerateResponseShape => "generate_response_shape",
            Self::GenerateResponseLength => "generate_response_length",
            Self::GenerateResponseFilter => "generate_response_filter",
            Self::GenerateFinishReason => "generate_finish_reason",
            Self::GenerateNumberAscii => "generate_number_ascii",
            Self::GenerateNumberChinese => "generate_number_chinese",
            Self::GenerateService => "generate_service",
            Self::ValidateStructure => "validate_structure",
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(category) = run().await {
        println!("summary passed=0 failed=1 category={}", category.label());
        std::process::exit(1);
    }
}

async fn run() -> Result<(), FailureCategory> {
    let mut args = std::env::args_os();
    let _program = args.next();
    let Some(app_data_dir) = args.next() else {
        print_help();
        return Err(FailureCategory::Arguments);
    };
    if app_data_dir == "--help" || app_data_dir == "-h" {
        print_help();
        return Ok(());
    }
    if args.next().is_some() {
        print_help();
        return Err(FailureCategory::Arguments);
    }

    let shared = SharedAppPaths::from_app_data_dir(Path::new(&app_data_dir));
    let accounts = AccountContextManager::load(shared).map_err(|_| FailureCategory::Account)?;
    let active = accounts
        .active_snapshot()
        .map_err(|_| FailureCategory::Account)?;
    let keys = DeepSeekKeyManager::new().map_err(|_| FailureCategory::Key)?;
    let status = keys.status().map_err(|_| FailureCategory::Key)?;
    if status.state != "ready" {
        return Err(FailureCategory::Key);
    }

    let temp = tempfile::TempDir::new().map_err(|_| FailureCategory::Backup)?;
    let temp_database_path = temp.path().join("validation.sqlite3");
    backup_database(active.database().path(), &temp_database_path)
        .map_err(|_| FailureCategory::Backup)?;
    let database = OwDatabase::new(&temp_database_path);
    database.initialize().map_err(|_| FailureCategory::Backup)?;
    let candidates = select_candidates(&database).map_err(|_| FailureCategory::Candidates)?;
    println!("eligible_standard={}", candidates.standard.len());
    println!("eligible_arena={}", candidates.arena.len());
    if candidates.standard.len() < 2 || candidates.arena.is_empty() {
        return Err(FailureCategory::Candidates);
    }

    let standard_one = candidates.standard[0].clone();
    let standard_two = candidates
        .standard
        .iter()
        .find(|candidate| candidate.hero_signature != standard_one.hero_signature)
        .cloned()
        .ok_or(FailureCategory::Candidates)?;
    let cases = [
        (standard_one.clone(), false),
        (standard_two, false),
        (candidates.arena[0].clone(), false),
        (standard_one, true),
    ];
    {
        let conn = database.connect().map_err(|_| FailureCategory::Backup)?;
        for (candidate, _) in cases.iter().take(3) {
            conn.execute(
                "DELETE FROM match_ai_roasts WHERE match_id=?1",
                [candidate.match_id.as_str()],
            )
            .map_err(|_| FailureCategory::Backup)?;
        }
    }
    let coordinator = AiRoastCoordinator::default();
    let client = DeepSeekRoastClient::new().map_err(|_| FailureCategory::Generate)?;
    let mut first_ending = None;
    let mut passed = 0;

    for (index, (candidate, regenerate)) in cases.into_iter().enumerate() {
        let started = Instant::now();
        let request = if regenerate {
            CapturedRoastRequest::regenerate(database.clone(), candidate.match_id.clone())
        } else {
            CapturedRoastRequest::new(database.clone(), candidate.match_id.clone())
        }
        .map_err(|_| FailureCategory::Generate)?;
        let generated = coordinator
            .generate(request, &keys, &client)
            .await
            .map_err(|message| classify_generation_failure(&message))?;
        let title = generated.ai_roast.award.trim();
        let commentary = generated.ai_roast.commentary.trim();
        if title.is_empty()
            || commentary.is_empty()
            || title.chars().count() > 36
            || commentary.chars().count() > 650
            || generated.ai_roast.report.is_some()
        {
            return Err(FailureCategory::ValidateStructure);
        }
        let report_chars = title.chars().count() + commentary.chars().count();
        let variation = if index == 3 {
            first_ending
                .as_ref()
                .is_some_and(|(first_title, first_commentary)| {
                    first_title != title || first_commentary != commentary
                })
                || first_ending.is_none()
        } else {
            if index == 0 {
                first_ending = Some((title.to_string(), commentary.to_string()));
            }
            false
        };
        passed += 1;
        let stage = if regenerate { "regenerate" } else { "generate" };
        if regenerate {
            println!(
                "case={} stage={} status=pass elapsed_ms={} report_chars={} variation={}",
                index + 1,
                stage,
                started.elapsed().as_millis(),
                report_chars,
                variation
            );
        } else {
            println!(
                "case={} stage={} status=pass elapsed_ms={} report_chars={}",
                index + 1,
                stage,
                started.elapsed().as_millis(),
                report_chars
            );
        }
    }
    println!("summary passed={passed} failed=0");
    Ok(())
}

fn classify_generation_failure(message: &str) -> FailureCategory {
    if message.contains("超时") {
        FailureCategory::GenerateTimeout
    } else if message.contains("可核验数据") {
        match take_validation_failure_category() {
            Some("evidence") => FailureCategory::GenerateEvidence,
            Some("number") => FailureCategory::GenerateNumber,
            Some("claim") => FailureCategory::GenerateClaim,
            Some("json") => FailureCategory::GenerateJson,
            Some("sections") => FailureCategory::GenerateSections,
            Some("text") => FailureCategory::GenerateText,
            Some("safety") => FailureCategory::GenerateSafety,
            Some("text_total") => FailureCategory::GenerateTextTotal,
            Some("text_field") => FailureCategory::GenerateTextField,
            Some("text_empty") => FailureCategory::GenerateTextEmpty,
            Some("number_percent") => FailureCategory::GenerateNumberPercent,
            Some("number_other") => FailureCategory::GenerateNumberOther,
            Some("response_shape") => FailureCategory::GenerateResponseShape,
            Some("response_length") => FailureCategory::GenerateResponseLength,
            Some("response_filter") => FailureCategory::GenerateResponseFilter,
            Some("finish_reason") => FailureCategory::GenerateFinishReason,
            Some("number_ascii") => FailureCategory::GenerateNumberAscii,
            Some("number_chinese") => FailureCategory::GenerateNumberChinese,
            _ => FailureCategory::GenerateInvalidOutput,
        }
    } else if message.contains("频繁") || message.contains("繁忙") || message.contains("连接")
    {
        FailureCategory::GenerateService
    } else {
        FailureCategory::Generate
    }
}

fn print_help() {
    println!("usage: validate_ai_roast_live <app_data_dir>");
    println!("在临时数据库副本中执行匿名 AI 锐评验收。");
}

fn backup_database(source_path: &Path, destination_path: &Path) -> Result<()> {
    let source = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut destination = Connection::open(destination_path)?;
    let backup = Backup::new(&source, &mut destination)?;
    backup.run_to_completion(128, Duration::from_millis(10), None)?;
    Ok(())
}

fn select_candidates(database: &OwDatabase) -> Result<CandidateSet> {
    let conn = database.connect()?;
    let knowledge = embedded_knowledge()?;
    let mut statement = conn.prepare(
        "SELECT m.match_id, m.match_kind
         FROM matches m
         JOIN match_details d ON d.match_id=m.match_id
         WHERE d.raw_detail_json IS NOT NULL AND length(d.raw_detail_json)>2
         ORDER BY m.begin_ts DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut standard = Vec::new();
    let mut arena = Vec::new();
    for row in rows {
        let (match_id, match_kind) = row?;
        let Ok(detail) = get_match_detail(database, &match_id) else {
            continue;
        };
        if detail.teammates.is_empty() || detail.enemies.is_empty() {
            continue;
        }
        let context = knowledge_for_match(
            &knowledge,
            detail
                .teammates
                .iter()
                .chain(&detail.enemies)
                .map(|player| player.hero_name.clone()),
            detail.map_name.as_deref(),
        );
        if build_roast_snapshot(&conn, &match_id, &detail, context).is_err() {
            continue;
        }
        let mut heroes = detail
            .teammates
            .iter()
            .chain(&detail.enemies)
            .map(|player| player.hero_name.clone())
            .collect::<Vec<_>>();
        heroes.sort();
        let candidate = Candidate {
            match_id,
            hero_signature: heroes.join("|"),
            simple_priority: current_player_is_not_objective_leader(&detail),
        };
        if match_kind == "fight" {
            arena.push(candidate);
        } else {
            standard.push(candidate);
        }
    }
    standard.sort_by_key(|candidate| !candidate.simple_priority);
    arena.sort_by_key(|candidate| !candidate.simple_priority);
    Ok(CandidateSet { standard, arena })
}

fn current_player_is_not_objective_leader(detail: &MatchDetailView) -> bool {
    let Some(hero_guid) = detail.hero_guid.as_deref() else {
        return false;
    };
    let current = detail
        .teammates
        .iter()
        .filter(|player| player.hero_guid == hero_guid)
        .collect::<Vec<_>>();
    if current.len() != 1 {
        return false;
    }
    let team_max = detail
        .teammates
        .iter()
        .map(|player| player.target_time_sec)
        .max();
    team_max.is_some_and(|maximum| current[0].target_time_sec < maximum)
}
