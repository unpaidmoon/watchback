use crate::db::OwDatabase;
use crate::importer::{import_payload_value_embedded, string_field};
use crate::models::{SyncProgress, SyncSummary};
use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DASHEN_CUSTOMER_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool/customer";
const CLIENT_TYPE: &str = "60";
const ORIGIN: &str = "https://act.ds.163.com";
const REFERER: &str = "https://act.ds.163.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36 app/df_client dfVersion/100111";
const NORMAL_MODES: &[&str] = &[
    "SportPreset",
    "SportOpen",
    "LeisurePreset",
    "Leisure6v6",
    "sport",
    "leisure",
];
const FIGHT_MODES: &[&str] = &["QuickFight", "LeisureFight", "SportFight"];
const MAX_PAGES_PER_MODE: i64 = 80;
const MAX_HTTP_ATTEMPTS: usize = 5;
const REMOTE_REQUEST_SPACING_MS: u64 = 350;
const HTTP_TIMEOUT_SECONDS: u64 = 20;

#[derive(Debug, Deserialize)]
pub(crate) struct Credentials {
    #[serde(alias = "roleId")]
    pub(crate) role_id: i64,
    pub(crate) token: String,
    #[serde(default, alias = "customerToken")]
    pub(crate) customer_token: Option<String>,
    pub(crate) dts: Option<i64>,
    pub(crate) server: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonJsonDump {
    pub path: PathBuf,
    pub entries: usize,
    pub details: usize,
    pub failed_details: usize,
    pub skipped_duplicates: usize,
}

struct SeasonFetchParts {
    cached_player: Value,
    entries: Vec<Value>,
    details: Vec<Value>,
    detail_errors: Vec<Value>,
    list_pages: Vec<Value>,
    skipped_duplicates: usize,
}

struct ModeFetchPages {
    entries: Vec<Value>,
    list_pages: Vec<Value>,
}

mod background_worker;
mod client;
mod facade;
mod fetch;
mod incremental;
mod payload;
mod persist;
mod quick_list;
mod snapshot;
mod state;

use client::*;
use fetch::*;
use payload::*;

pub use background_worker::{run_background_job_once, BackgroundRunSummary};
pub use facade::{
    dump_season_json, dump_season_json_with_base_url,
    quick_list_update_with_credentials_and_target_paths_base_url, sync_season,
    sync_season_with_base_url, sync_season_with_credentials_and_target_paths_base_url,
    sync_season_with_credentials_and_target_paths_output_dir_base_url,
    sync_season_with_credentials_and_target_paths_output_dir_with_mode_base_url,
    sync_season_with_credentials_and_target_paths_with_mode_base_url,
    sync_season_with_credentials_path_base_url, sync_season_with_credentials_path_progress,
    sync_season_with_current_account_credentials_path_output_dir_base_url,
    sync_season_with_current_account_credentials_path_output_dir_with_mode_base_url,
    sync_season_with_progress,
};
pub use incremental::{
    build_page_diagnostics, classify_quick_page, load_season_sync_inventory, select_detail_entries,
    DetailSelection, ModeSyncInventory, QuickPageDecision, SeasonModeKey, SeasonSyncInventory,
};
pub use persist::sync_import_parts;
#[doc(hidden)]
pub use persist::write_season_payload;
pub use quick_list::commit_quick_list_probe;
#[doc(hidden)]
pub use snapshot::{
    rebuild_complete_season_payload, rebuild_snapshot_for_revision,
    rebuild_snapshot_for_revision_with_hook, SnapshotOutcome,
};
pub use state::{
    ClaimedSeasonJob, ModeWork, RequestShape, SeasonBackgroundStatus, SeasonSyncStore,
};
