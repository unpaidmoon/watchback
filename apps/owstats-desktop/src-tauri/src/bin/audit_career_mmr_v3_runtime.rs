use owstats_desktop_lib::{
    app_paths::SharedAppPaths,
    career_mmr::{
        input::{ModeFamily, Role},
        performance_runtime::NormalizedPerformanceRow,
        training::build_multi_account_projection,
        training_report::{validate_parsed_v3_artifact, PerformanceModelArtifact},
        training_sources::discover_training_sources,
    },
};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn role(value: &str) -> Option<Role> {
    match value {
        "tank" => Some(Role::Tank),
        "damage" => Some(Role::Damage),
        "support" => Some(Role::Support),
        _ => None,
    }
}

fn main() {
    let app_data_dir = PathBuf::from(env::args_os().nth(1).expect("missing app data dir"));
    let artifact_path = PathBuf::from(env::args_os().nth(2).expect("missing artifact"));
    let packaged: PerformanceModelArtifact =
        serde_json::from_slice(&fs::read(artifact_path).expect("artifact read failed"))
            .expect("artifact json failed");
    let artifact = packaged.v3_runtime.expect("missing v3 runtime");
    validate_parsed_v3_artifact(&artifact).expect("invalid v3 runtime");
    let sources = discover_training_sources(&SharedAppPaths::from_app_data_dir(app_data_dir))
        .expect("source discovery failed");
    let projection = build_multi_account_projection(&sources).expect("projection failed");
    let mut total = 0usize;
    let mut nonzero = 0usize;
    let mut nonfinite = 0usize;
    let mut cap_violations = 0usize;
    let mut direction_reversals = 0usize;
    let mut by_mode = BTreeMap::<ModeFamily, usize>::new();
    for source in projection.rows.iter().filter(|row| row.allow_performance) {
        let Some(role) = role(&source.role) else {
            continue;
        };
        let Some(model) = artifact.selected_models.get(&role) else {
            continue;
        };
        let duration_min = source.duration_sec.max(1.0) / 60.0;
        let per10 = |value: Option<f64>| value.unwrap_or(0.0) / duration_min;
        let final_hit = per10(source.final_hit).tanh();
        let kill = per10(source.kill);
        let row = NormalizedPerformanceRow {
            role,
            mode: source.mode_family,
            final_hit,
            non_final_eliminations: (kill - per10(source.final_hit)).max(0.0).tanh(),
            assist: per10(source.assist).tanh(),
            death: -per10(source.death).tanh(),
        };
        let base_delta = source.base_delta.unwrap_or(0.0);
        let correction = model.evaluate_sign_safe(&row, base_delta);
        total += 1;
        *by_mode.entry(source.mode_family).or_default() += 1;
        nonzero += usize::from(correction.abs() > 1.0e-12);
        nonfinite += usize::from(!correction.is_finite());
        cap_violations += usize::from(correction.abs() > model.cap_for(source.mode_family) + 1e-9);
        direction_reversals += usize::from(
            base_delta != 0.0 && (base_delta + correction).signum() != base_delta.signum(),
        );
    }
    println!("rows={total}");
    println!("nonzero={nonzero}");
    println!("nonfinite={nonfinite}");
    println!("cap_violations={cap_violations}");
    println!("direction_reversals={direction_reversals}");
    for (mode, count) in by_mode {
        println!("mode={} rows={count}", mode.as_str());
    }
}
