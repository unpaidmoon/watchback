use std::{env, fs, path::Path};

const DEEPSEEK_KEY_NAME: &str = "DEEPSEEK_API_KEY";

fn read_local_deepseek_key(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    contents.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (name, value) = line.split_once('=')?;
        if name.trim() != DEEPSEEK_KEY_NAME {
            return None;
        }
        let value = value.trim().trim_matches(['"', '\'']);
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn main() {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .expect("Cargo must set CARGO_MANIFEST_DIR");
    let local_env = manifest_dir.join("../../..").join(".env");
    println!("cargo:rerun-if-changed={}", local_env.display());
    println!("cargo:rerun-if-env-changed={DEEPSEEK_KEY_NAME}");

    let bundled_key = env::var(DEEPSEEK_KEY_NAME)
        .ok()
        .filter(|key| !key.trim().is_empty())
        .or_else(|| read_local_deepseek_key(&local_env));
    let generated = format!(
        "const BUNDLED_DEEPSEEK_API_KEY: Option<&'static str> = {:?};\n",
        bundled_key.as_deref()
    );
    let out_dir = env::var_os("OUT_DIR")
        .map(std::path::PathBuf::from)
        .expect("Cargo must set OUT_DIR");
    fs::write(out_dir.join("bundled_deepseek_api_key.rs"), generated)
        .expect("failed to generate bundled DeepSeek key configuration");

    tauri_build::build();
}
