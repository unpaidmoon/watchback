use crate::account_context::AccountContextManager;
use crate::background_sync::BackgroundSeasonSyncManager;
use crate::login_target::LoginTargetCoordinator;
use crate::sync::SeasonSyncStore;
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
    login_target_coordinator: LoginTargetCoordinator,
    background_sync: BackgroundSeasonSyncManager,
) {
    tauri::async_runtime::spawn(async move {
        let _ = run_once(
            &app,
            &accounts,
            &operation_coordinator,
            &login_target_coordinator,
            &background_sync,
        )
        .await;
    });
}

async fn run_once(
    app: &AppHandle,
    accounts: &AccountContextManager,
    operation_coordinator: &SyncCoordinator,
    login_target_coordinator: &LoginTargetCoordinator,
    background_sync: &BackgroundSeasonSyncManager,
) -> Result<bool> {
    let _permit = match operation_coordinator.try_acquire_sync() {
        Ok(permit) => permit,
        Err(_) => return Ok(false),
    };
    let settings =
        crate::settings::get_user_settings_for_path(&accounts.shared_paths().user_settings())?;
    if !settings.auto_update_season {
        return Ok(false);
    }

    let account_status = accounts.status()?;
    let Some(active_account_id) = account_status.active_account_id.as_deref() else {
        return Ok(false);
    };
    let credential_ready = account_status.accounts.iter().any(|account| {
        account.account_id == active_account_id && account.credential_state == "ready"
    });
    if !credential_ready {
        return Ok(false);
    }

    let account = accounts.active_snapshot()?;
    let db = account.database();
    let Some(season) = latest_local_season(&db)? else {
        return Ok(false);
    };
    let store = SeasonSyncStore::new(db.clone());
    if store.active_job_count(season)? > 0 {
        return Ok(false);
    }

    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())?;
    let target_path = account.paths().player_target();
    if crate::login_target::ensure_sync_target_ready(
        login_target_coordinator,
        account.account_id(),
        &credentials,
        &target_path,
    )
    .is_err()
    {
        return Ok(false);
    }

    crate::quick_list_update_with_credentials_and_target_paths_base_url(
        &db,
        &account.paths().credentials(),
        &target_path,
        season,
        DASHEN_CUSTOMER_BASE,
    )
    .await?;
    drop(_permit);

    let event_app = app.clone();
    let events: crate::BackgroundEventSink = Arc::new(move |event, payload| {
        let _ = event_app.emit(event, payload);
    });
    background_sync.wake(
        account.account_id().as_str(),
        account.paths().clone(),
        season,
        events,
    );
    Ok(true)
}

pub(crate) fn latest_local_season(db: &OwDatabase) -> Result<Option<i64>> {
    db.initialize()?;
    let conn = db.connect()?;
    conn.query_row("SELECT MAX(season) FROM matches", [], |row| row.get(0))
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::latest_local_season;
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
}
