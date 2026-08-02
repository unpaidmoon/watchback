use crate::account_context::AccountContextManager;
use crate::background_sync::BackgroundSeasonSyncManager;
use crate::live_season_update::{LiveSeasonKey, LiveSeasonUpdateManager};
use crate::sync_coordinator::SyncCoordinator;
use crate::OwDatabase;
use anyhow::Result;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const DASHEN_CUSTOMER_BASE: &str = "https://datamsapi.ds.163.com/v1/a19ld5tool/customer";
pub(crate) fn spawn(
    app: AppHandle,
    accounts: AccountContextManager,
    operation_coordinator: SyncCoordinator,
    background_sync: BackgroundSeasonSyncManager,
    live_updates: LiveSeasonUpdateManager,
) {
    // Pin the trigger to the account that was active when it was requested.
    // A later account switch creates another keyed request instead of replacing
    // or retargeting this one.
    let Ok(triggered_account) = accounts.active_snapshot() else {
        crate::diagnostics::record(
            crate::diagnostics::Level::Warn,
            "auto_update",
            "skipped",
            &[("reason", "noActiveAccount".to_string())],
        );
        return;
    };
    let triggered_account_id = triggered_account.account_id().clone();
    drop(triggered_account);
    tauri::async_runtime::spawn(async move {
        crate::diagnostics::record(
            crate::diagnostics::Level::Info,
            "auto_update",
            "check_started",
            &[],
        );
        let result = run_once(
            &app,
            &accounts,
            &operation_coordinator,
            &background_sync,
            &live_updates,
            &triggered_account_id,
        )
        .await;
        match result {
            Ok(started) => crate::diagnostics::record(
                crate::diagnostics::Level::Info,
                "auto_update",
                "check_completed",
                &[("updateStarted", started.to_string())],
            ),
            Err(error) => {
                crate::diagnostics::record_error("auto_update", "check_failed", &error, &[])
            }
        }
    });
}

async fn run_once(
    app: &AppHandle,
    accounts: &AccountContextManager,
    operation_coordinator: &SyncCoordinator,
    background_sync: &BackgroundSeasonSyncManager,
    live_updates: &LiveSeasonUpdateManager,
    triggered_account_id: &crate::account_store::AccountId,
) -> Result<bool> {
    let settings =
        crate::settings::get_user_settings_for_path(&accounts.shared_paths().user_settings())?;
    if !settings.auto_update_season {
        return Ok(false);
    }

    let account_status = accounts.status()?;
    let credential_ready = account_status.accounts.iter().any(|account| {
        account.account_id == triggered_account_id.as_str() && account.credential_state == "ready"
    });
    if !credential_ready {
        return Ok(false);
    }

    let account = accounts.snapshot_for(triggered_account_id)?;
    let db = account.database();
    let local_season = latest_local_season(&db)?;
    // Dashen currently exposes no independently verified current-season metadata endpoint.
    // Load the versioned packaged resource at runtime; if it is missing, malformed, or stale,
    // the resolver safely limits automatic updates to the newest local season.
    let runtime_config = crate::runtime_config::load_packaged_current_season_config(app).ok();
    let resolution =
        crate::runtime_config::resolve_current_season(None, runtime_config, local_season);
    let Some(season) = resolution.season else {
        return Ok(false);
    };
    let live_key = LiveSeasonKey::new(account.account_id().as_str(), season);
    let Some(_live_run) = live_updates.admit(live_key.clone()).await else {
        return Ok(false);
    };
    background_sync
        .wait_for_idle(account.account_id().as_str(), season)
        .await;
    let _permit = operation_coordinator.acquire_sync().await;

    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())?;
    let target_path = account.paths().player_target();
    if crate::account_sync_identity::ensure_ready(&credentials, &target_path).is_err() {
        return Ok(false);
    }

    let revision_db = db.clone();
    let revision_app = app.clone();
    let event_manager = live_updates.clone();
    let event_key = live_key.clone();
    let update = crate::quick_list_update_with_credentials_and_target_paths_base_url_observed(
        &db,
        &account.paths().credentials(),
        &target_path,
        season,
        DASHEN_CUSTOMER_BASE,
        move |commit| {
            if let Ok(Some(event)) =
                event_manager.list_commit_event(&revision_db, &event_key, commit)
            {
                let _ = revision_app.emit("season-data-revision", event);
            }
        },
    )
    .await;
    if let Err(error) = update {
        let failure = crate::account_context::CredentialUseFailure::from_upstream_error(&error);
        record_authenticated_failure(accounts, account.account_id(), failure)?;
        return Err(error);
    }
    drop(_permit);

    let event_app = app.clone();
    let events: crate::BackgroundEventSink = Arc::new(move |event, payload| {
        let _ = event_app.emit(event, payload);
    });
    let observed_accounts = accounts.clone();
    let observed_account_id = account.account_id().clone();
    background_sync.wake_observed(
        account.account_id().as_str(),
        account.paths().clone(),
        season,
        events,
        Some(Arc::new(move |error| {
            let failure = crate::account_context::CredentialUseFailure::from_upstream_error(error);
            let _ = observed_accounts.record_credential_use_failure(&observed_account_id, failure);
        })),
    );
    Ok(true)
}

fn record_authenticated_failure(
    accounts: &AccountContextManager,
    account_id: &crate::account_store::AccountId,
    failure: crate::account_context::CredentialUseFailure,
) -> Result<bool> {
    accounts.record_credential_use_failure(account_id, failure)
}

pub(crate) fn latest_local_season(db: &OwDatabase) -> Result<Option<i64>> {
    db.initialize()?;
    let conn = db.connect()?;
    conn.query_row("SELECT MAX(season) FROM matches", [], |row| row.get(0))
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{latest_local_season, record_authenticated_failure};
    use crate::account_context::{AccountContextManager, CredentialUseFailure};
    use crate::account_store::{
        write_manifest_atomic, write_registry_atomic, AccountId, AccountManifest, AccountRegistry,
        AccountRegistryEntry,
    };
    use crate::app_paths::SharedAppPaths;
    use crate::OwDatabase;

    #[test]
    fn latest_local_season_requires_existing_match_data() {
        let temp = tempfile::tempdir().unwrap();
        let db = OwDatabase::new(temp.path().join("owstats.db"));
        db.initialize().unwrap();
        assert_eq!(latest_local_season(&db).unwrap(), None);

        let conn = db.connect().unwrap();
        for (match_id, season) in [("older", 21), ("latest", 23), ("middle", 22)] {
            conn.execute(
                "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,?2,'preset','SportPreset','preset','win','map','hero',1,0,0,0,0,0,0,0,0,0,1,'{}','now')",
                rusqlite::params![match_id, season],
            )
            .unwrap();
        }
        assert_eq!(latest_local_season(&db).unwrap(), Some(23));
    }

    #[test]
    fn automatic_sync_only_stales_credentials_for_401_or_403() {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let paths = shared.account(&id);
        std::fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(
            &paths.manifest(),
            &AccountManifest {
                version: 1,
                account_id: id.clone(),
                role_id: 101101,
                battle_tag: "Synthetic#1000".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            },
        )
        .unwrap();
        std::fs::write(paths.database(), b"synthetic-db").unwrap();
        write_registry_atomic(
            &shared.registry(),
            &AccountRegistry {
                version: 1,
                active_account_id: Some(id.clone()),
                accounts: vec![AccountRegistryEntry {
                    account_id: id.clone(),
                    battle_tag: "Synthetic#1000".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    last_used_at: "2026-01-01T00:00:00Z".into(),
                    credential_state: "ready".into(),
                }],
            },
        )
        .unwrap();
        let manager = AccountContextManager::load(shared).unwrap();

        record_authenticated_failure(&manager, &id, CredentialUseFailure::Transient).unwrap();
        assert_eq!(
            manager.list_accounts().unwrap()[0].credential_state,
            "ready"
        );
        record_authenticated_failure(&manager, &id, CredentialUseFailure::Forbidden).unwrap();
        assert_eq!(
            manager.list_accounts().unwrap()[0].credential_state,
            "stale"
        );
    }
}
