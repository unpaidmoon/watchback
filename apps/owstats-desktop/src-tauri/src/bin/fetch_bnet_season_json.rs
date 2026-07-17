use anyhow::{anyhow, Context, Result};
use owstats_desktop_lib::dashen_client::{DashenClient, DashenCredentials, DashenRateLimitConfig};
use owstats_desktop_lib::player_target::target_from_search_payload;
use owstats_desktop_lib::season_fetch::{
    fetch_full_season_with_rate_limit_base_url, FullSeasonRequest,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const DASHEN_CUSTOMER_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool/customer";

#[derive(Debug, Deserialize)]
struct LocalCredentials {
    #[serde(alias = "roleId")]
    role_id: i64,
    token: String,
    dts: Option<i64>,
    server: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
struct FetchArgs {
    bnet: String,
    season: i64,
    credential_path: Option<PathBuf>,
    cached_player_path: Option<PathBuf>,
    customer_token: Option<String>,
    output_path: Option<PathBuf>,
    pages_per_batch: i64,
    detail_concurrency: usize,
    account_rps: usize,
    account_window_ms: u64,
    datamsapi_concurrency: usize,
    max_http_attempts: usize,
    retry_base_ms: u64,
    include_fight: bool,
}

impl FetchArgs {
    fn parse_from<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let tokens = args.into_iter().map(Into::into).collect::<Vec<_>>();
        if tokens.iter().any(|token| token.starts_with("--")) {
            return Self::parse_flags(tokens);
        }
        Self::parse_legacy(tokens)
    }

    fn parse_legacy(tokens: Vec<String>) -> Result<Self> {
        let mut args = tokens.into_iter();
        let bnet = args.next().ok_or_else(|| anyhow!(usage()))?;
        let season = args
            .next()
            .ok_or_else(|| anyhow!("missing season"))?
            .parse::<i64>()
            .with_context(|| "invalid season")?;
        let credential_path = PathBuf::from(
            args.next()
                .ok_or_else(|| anyhow!("missing credential path"))?,
        );
        Ok(Self {
            bnet,
            season,
            credential_path: Some(credential_path),
            cached_player_path: args.next().map(PathBuf::from),
            customer_token: None,
            output_path: None,
            pages_per_batch: 5,
            detail_concurrency: 2,
            account_rps: 5,
            account_window_ms: 1_000,
            datamsapi_concurrency: 2,
            max_http_attempts: 3,
            retry_base_ms: 5_000,
            include_fight: true,
        })
    }

    fn parse_flags(tokens: Vec<String>) -> Result<Self> {
        let mut bnet = None;
        let mut season = None;
        let mut credential_path = None;
        let mut cached_player_path = None;
        let mut customer_token = None;
        let mut output_path = None;
        let mut pages_per_batch = 5_i64;
        let mut detail_concurrency = 2_usize;
        let mut account_rps = 5_usize;
        let mut account_window_ms = 1_000_u64;
        let mut datamsapi_concurrency = 2_usize;
        let mut max_http_attempts = 3_usize;
        let mut retry_base_ms = 5_000_u64;
        let mut include_fight = true;
        let mut index = 0_usize;
        while index < tokens.len() {
            let token = &tokens[index];
            match token.as_str() {
                "--bnet" => bnet = Some(next_value(&tokens, &mut index, "--bnet")?),
                "--season" => {
                    season = Some(
                        next_value(&tokens, &mut index, "--season")?
                            .parse::<i64>()
                            .with_context(|| "invalid season")?,
                    )
                }
                "--credential-path" | "--credentials" => {
                    credential_path = Some(PathBuf::from(next_value(
                        &tokens,
                        &mut index,
                        "--credential-path",
                    )?))
                }
                "--cached-player" | "--cached-player-path" => {
                    cached_player_path =
                        Some(PathBuf::from(next_value(&tokens, &mut index, token)?))
                }
                "--customer-token" => {
                    customer_token = Some(next_value(&tokens, &mut index, "--customer-token")?)
                }
                "--out" | "--output" => {
                    output_path = Some(PathBuf::from(next_value(&tokens, &mut index, token)?))
                }
                "--pages-per-batch" => {
                    pages_per_batch = next_value(&tokens, &mut index, "--pages-per-batch")?
                        .parse::<i64>()
                        .with_context(|| "invalid pages-per-batch")?
                }
                "--detail-concurrency" => {
                    detail_concurrency = next_value(&tokens, &mut index, "--detail-concurrency")?
                        .parse::<usize>()
                        .with_context(|| "invalid detail-concurrency")?
                }
                "--account-rps" => {
                    account_rps = next_value(&tokens, &mut index, "--account-rps")?
                        .parse::<usize>()
                        .with_context(|| "invalid account-rps")?
                }
                "--account-window-ms" => {
                    account_window_ms = next_value(&tokens, &mut index, "--account-window-ms")?
                        .parse::<u64>()
                        .with_context(|| "invalid account-window-ms")?
                }
                "--datamsapi-concurrency" => {
                    datamsapi_concurrency =
                        next_value(&tokens, &mut index, "--datamsapi-concurrency")?
                            .parse::<usize>()
                            .with_context(|| "invalid datamsapi-concurrency")?
                }
                "--max-http-attempts" => {
                    max_http_attempts = next_value(&tokens, &mut index, "--max-http-attempts")?
                        .parse::<usize>()
                        .with_context(|| "invalid max-http-attempts")?
                }
                "--retry-base-ms" => {
                    retry_base_ms = next_value(&tokens, &mut index, "--retry-base-ms")?
                        .parse::<u64>()
                        .with_context(|| "invalid retry-base-ms")?
                }
                "--full-season" => {}
                "--no-fight" => include_fight = false,
                "--help" | "-h" => return Err(anyhow!(usage())),
                other => return Err(anyhow!("unknown argument: {other}\n{}", usage())),
            }
            index += 1;
        }
        Ok(Self {
            bnet: bnet.ok_or_else(|| anyhow!("missing --bnet"))?,
            season: season.ok_or_else(|| anyhow!("missing --season"))?,
            credential_path,
            cached_player_path,
            customer_token,
            output_path,
            pages_per_batch,
            detail_concurrency,
            account_rps,
            account_window_ms,
            datamsapi_concurrency,
            max_http_attempts,
            retry_base_ms,
            include_fight,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = FetchArgs::parse_from(std::env::args().skip(1))?;
    if args.season <= 0 {
        return Err(anyhow!("season must be greater than 0"));
    }

    let credentials = resolve_credentials(args.credential_path.as_deref())?;
    let dashen_credentials = DashenCredentials {
        role_id: credentials.role_id,
        token: credentials.token,
        dts: credentials.dts,
        server: credentials.server.unwrap_or(1),
    };
    let client = DashenClient::new(dashen_credentials.clone(), DASHEN_CUSTOMER_BASE.to_string())?;
    let target = if let Some(customer_token) = args.customer_token.clone() {
        owstats_desktop_lib::player_target::PlayerTarget {
            input_bnet: args.bnet.clone(),
            resolved_bnet: Some(args.bnet.clone()),
            customer_token,
            bound_at: chrono::Utc::now().to_rfc3339(),
            source_role_id: None,
        }
    } else if let Some(cached_player_path) = args.cached_player_path.clone() {
        let customer_token = read_cached_customer_token(&cached_player_path)?;
        owstats_desktop_lib::player_target::PlayerTarget {
            input_bnet: args.bnet.clone(),
            resolved_bnet: Some(args.bnet.clone()),
            customer_token,
            bound_at: chrono::Utc::now().to_rfc3339(),
            source_role_id: None,
        }
    } else {
        let search_payload = client.search_bnet_account(&args.bnet).await?;
        target_from_search_payload(
            &args.bnet,
            &search_payload,
            credentials.role_id,
            &chrono::Utc::now().to_rfc3339(),
        )
        .with_context(|| search_payload_diagnostic(&search_payload))?
    };

    let fetch = fetch_full_season_with_rate_limit_base_url(
        FullSeasonRequest {
            credentials: dashen_credentials,
            customer_token: target.customer_token,
            input_bnet: target.input_bnet,
            resolved_bnet: target.resolved_bnet,
            season: args.season,
            customer_base_url: DASHEN_CUSTOMER_BASE.to_string(),
            pages_per_batch: args.pages_per_batch,
            detail_concurrency: args.detail_concurrency,
            include_fight: args.include_fight,
        },
        DashenRateLimitConfig {
            datamsapi_max_concurrent: args.datamsapi_concurrency,
            account_max_requests_per_window: args.account_rps,
            account_window_ms: args.account_window_ms,
            max_http_attempts: args.max_http_attempts,
            retry_base_ms: args.retry_base_ms,
        },
    )
    .await?;

    let output_path = match args.output_path {
        Some(path) => path,
        None => {
            let output_dir = project_root()?.join("data").join("players");
            std::fs::create_dir_all(&output_dir)
                .with_context(|| format!("failed to create {}", output_dir.display()))?;
            output_dir.join(format!("local_s{}_full_season.json", args.season))
        }
    };
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&fetch.payload)?;
    std::fs::write(&output_path, text)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    println!(
        "wrote {} entries={} details={} failed_details={}",
        output_path.display(),
        fetch.entries,
        fetch.details,
        fetch.failed_details
    );
    Ok(())
}

fn next_value(tokens: &[String], index: &mut usize, flag: &str) -> Result<String> {
    *index += 1;
    tokens
        .get(*index)
        .cloned()
        .filter(|value| !value.starts_with("--"))
        .ok_or_else(|| anyhow!("{flag} requires a value"))
}

fn usage() -> &'static str {
    "usage: fetch_bnet_season_json --bnet <BattleTag> --season <season> [--credential-path <path>] [--customer-token <token>] [--out <path>] [--pages-per-batch 5] [--detail-concurrency 2] [--no-fight]\nlegacy: fetch_bnet_season_json <BattleTag> <season> <credential_path> [cached_player_path]"
}

fn resolve_credentials(path: Option<&Path>) -> Result<LocalCredentials> {
    if let Some(path) = path {
        return read_credentials(path);
    }
    if let Some(credentials) = read_credentials_from_env()? {
        return Ok(credentials);
    }
    read_credentials(
        &project_root()?
            .join("data")
            .join(".dashen_credentials.json"),
    )
}

fn read_credentials_from_env() -> Result<Option<LocalCredentials>> {
    let role_id = match std::env::var("OW_DASHEN_ROLE_ID")
        .or_else(|_| std::env::var("DASHEN_ROLE_ID"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        Some(value) => value
            .parse::<i64>()
            .with_context(|| "OW_DASHEN_ROLE_ID must be an integer")?,
        None => return Ok(None),
    };
    let token = match std::env::var("OW_DASHEN_TOKEN")
        .or_else(|_| std::env::var("DASHEN_TOKEN"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        Some(value) => value,
        None => return Ok(None),
    };
    let dts = std::env::var("OW_DASHEN_DTS")
        .or_else(|_| std::env::var("DASHEN_DTS"))
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok());
    let server = std::env::var("OW_DASHEN_SERVER")
        .or_else(|_| std::env::var("DASHEN_SERVER"))
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or(Some(1));
    Ok(Some(LocalCredentials {
        role_id,
        token,
        dts,
        server,
    }))
}

fn read_credentials(path: &Path) -> Result<LocalCredentials> {
    let text = std::fs::read_to_string(path).with_context(|| "local credentials not found")?;
    let credentials: LocalCredentials =
        serde_json::from_str(&text).with_context(|| "failed to parse local credentials")?;
    if credentials.role_id <= 0 || credentials.token.trim().is_empty() {
        return Err(anyhow!("local credentials are incomplete"));
    }
    Ok(credentials)
}

fn read_cached_customer_token(path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path).with_context(|| "cached player file not found")?;
    let payload: serde_json::Value =
        serde_json::from_str(&text).with_context(|| "failed to parse cached player file")?;
    let customer_token = payload
        .get("customer_token")
        .or_else(|| payload.get("customerToken"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("cached player file does not contain customer token"))?;
    Ok(customer_token.to_string())
}

fn search_payload_diagnostic(payload: &serde_json::Value) -> String {
    let code = payload
        .get("code")
        .map(|value| value.to_string())
        .unwrap_or_else(|| "missing".to_string());
    let message = payload
        .get("message")
        .or_else(|| payload.get("msg"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "missing".to_string());
    let data = payload.get("data");
    let data_kind = match data {
        Some(serde_json::Value::Object(_)) => "object",
        Some(serde_json::Value::Array(_)) => "array",
        Some(serde_json::Value::Null) => "null",
        Some(serde_json::Value::String(_)) => "string",
        Some(serde_json::Value::Bool(_)) => "bool",
        Some(serde_json::Value::Number(_)) => "number",
        None => "missing",
    };
    let data_keys = data
        .and_then(serde_json::Value::as_object)
        .map(|map| {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys.join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "-".to_string());
    let top_keys = payload
        .as_object()
        .map(|map| {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys.join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "-".to_string());
    format!("search payload summary: code={code} message={message} top_keys={top_keys} data_kind={data_kind} data_keys={data_keys}")
}

fn project_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("..")
        .canonicalize()
        .with_context(|| "failed to resolve project root")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_python_style_full_season_arguments() {
        let args = FetchArgs::parse_from([
            "--bnet",
            "Tester#12345",
            "--season",
            "22",
            "--credential-path",
            "creds.json",
            "--customer-token",
            "player-token",
            "--out",
            "out.json",
            "--pages-per-batch",
            "5",
            "--detail-concurrency",
            "3",
            "--account-rps",
            "4",
            "--account-window-ms",
            "1200",
            "--datamsapi-concurrency",
            "2",
            "--max-http-attempts",
            "4",
            "--retry-base-ms",
            "250",
            "--no-fight",
        ])
        .unwrap();

        assert_eq!(args.bnet, "Tester#12345");
        assert_eq!(args.season, 22);
        assert_eq!(args.credential_path, Some(PathBuf::from("creds.json")));
        assert_eq!(args.customer_token.as_deref(), Some("player-token"));
        assert_eq!(args.output_path, Some(PathBuf::from("out.json")));
        assert_eq!(args.pages_per_batch, 5);
        assert_eq!(args.detail_concurrency, 3);
        assert_eq!(args.account_rps, 4);
        assert_eq!(args.account_window_ms, 1200);
        assert_eq!(args.datamsapi_concurrency, 2);
        assert_eq!(args.max_http_attempts, 4);
        assert_eq!(args.retry_base_ms, 250);
        assert!(!args.include_fight);
    }

    #[test]
    fn keeps_legacy_positional_arguments_working() {
        let args =
            FetchArgs::parse_from(["Tester#12345", "22", "creds.json", "cached-player.json"])
                .unwrap();

        assert_eq!(args.bnet, "Tester#12345");
        assert_eq!(args.season, 22);
        assert_eq!(args.credential_path, Some(PathBuf::from("creds.json")));
        assert_eq!(
            args.cached_player_path,
            Some(PathBuf::from("cached-player.json"))
        );
        assert_eq!(args.pages_per_batch, 5);
        assert_eq!(args.detail_concurrency, 2);
        assert_eq!(args.account_rps, 5);
        assert_eq!(args.account_window_ms, 1000);
        assert_eq!(args.datamsapi_concurrency, 2);
        assert_eq!(args.max_http_attempts, 3);
        assert_eq!(args.retry_base_ms, 5000);
        assert!(args.include_fight);
    }

    #[test]
    fn search_payload_diagnostic_preserves_message_and_schema_keys() {
        let payload = serde_json::json!({
            "code": 401,
            "message": "token=synthetic-diagnostic-value",
            "customer_token": "synthetic-customer-value",
            "data": {
                "cookie": "synthetic-cookie-value",
                "safe": true
            }
        });

        let diagnostic = search_payload_diagnostic(&payload);

        assert!(diagnostic.contains("token=synthetic-diagnostic-value"));
        assert!(diagnostic.contains("customer_token"));
        assert!(diagnostic.contains("cookie,safe"));
    }
}
