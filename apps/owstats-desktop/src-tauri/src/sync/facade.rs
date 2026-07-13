use super::*;

pub async fn sync_season(db: &OwDatabase, project_root: &Path, season: i64) -> Result<SyncSummary> {
    sync_season_with_progress(db, project_root, season, |_| {}).await
}

pub async fn sync_season_with_progress(
    db: &OwDatabase,
    project_root: &Path,
    season: i64,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    let credential_path = default_project_credential_path(project_root);
    sync_season_with_credentials_path_base_url(
        db,
        project_root,
        &credential_path,
        season,
        DASHEN_CUSTOMER_BASE,
        progress,
    )
    .await
}

pub async fn sync_season_with_credentials_path_progress(
    db: &OwDatabase,
    project_root: &Path,
    credential_path: &Path,
    season: i64,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    sync_season_with_credentials_path_base_url(
        db,
        project_root,
        credential_path,
        season,
        DASHEN_CUSTOMER_BASE,
        progress,
    )
    .await
}

pub async fn sync_season_with_base_url(
    db: &OwDatabase,
    project_root: &Path,
    season: i64,
    customer_base_url: &str,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    let credential_path = default_project_credential_path(project_root);
    sync_season_with_credentials_path_base_url(
        db,
        project_root,
        &credential_path,
        season,
        customer_base_url,
        progress,
    )
    .await
}

pub async fn sync_season_with_credentials_path_base_url(
    db: &OwDatabase,
    project_root: &Path,
    credential_path: &Path,
    season: i64,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    db.initialize()?;
    let parts =
        fetch_season_parts_with_base_url(credential_path, season, customer_base_url, &mut progress)
            .await?;
    let total = parts.entries.len();
    let payload = build_full_season_payload(
        season,
        &parts.cached_player,
        parts.entries,
        parts.details,
        parts.detail_errors,
        parts.list_pages,
    );
    write_season_payload(&project_root.join("data").join("players"), season, &payload)?;
    if total == 0 {
        return Err(anyhow!(
            "未拉取到 S{season} 赛季对局；已保存本地 JSON，请确认该赛季有战绩，或稍后重新登录后再试。"
        ));
    }
    let entries = payload
        .pointer("/matches/entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let details = payload
        .pointer("/matches/details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_errors = payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    progress(SyncProgress {
        season,
        phase: "写入 SQLite".to_string(),
        processed: total,
        total,
        current_match_id: None,
        failed_details: detail_errors.len(),
        latest_errors: detail_errors
            .iter()
            .rev()
            .take(5)
            .filter_map(|item| string_field(item, "message"))
            .collect(),
    });
    let summary = sync_import_parts(
        db,
        project_root,
        season,
        &payload,
        entries,
        details,
        detail_errors,
        parts.skipped_duplicates,
    )?;
    progress(SyncProgress {
        season,
        phase: "完成".to_string(),
        processed: summary.processed,
        total: summary.total,
        current_match_id: None,
        failed_details: summary.failed_details,
        latest_errors: summary.latest_errors.clone(),
    });
    Ok(summary)
}

pub async fn sync_season_with_credentials_and_target_paths_base_url(
    db: &OwDatabase,
    project_root: &Path,
    credential_path: &Path,
    target_path: &Path,
    season: i64,
    customer_base_url: &str,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    sync_season_with_credentials_and_target_paths_output_dir_base_url(
        db,
        &project_root.join("data").join("players"),
        credential_path,
        target_path,
        season,
        customer_base_url,
        progress,
    )
    .await
}

pub async fn sync_season_with_credentials_and_target_paths_output_dir_base_url(
    db: &OwDatabase,
    season_output_dir: &Path,
    credential_path: &Path,
    target_path: &Path,
    season: i64,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    db.initialize()?;
    let credentials = read_credentials(credential_path)?;
    let target = crate::player_target::read_player_target_for_path(target_path)?;
    progress(SyncProgress {
        season,
        phase: "拉取赛季 JSON".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    let fetch = crate::season_fetch::fetch_full_season_with_base_url(
        crate::season_fetch::FullSeasonRequest {
            credentials: crate::dashen_client::DashenCredentials {
                role_id: credentials.role_id,
                token: credentials.token,
                dts: credentials.dts,
                server: credentials.server.unwrap_or(1),
            },
            customer_token: target.customer_token,
            input_bnet: target.input_bnet,
            resolved_bnet: target.resolved_bnet,
            season,
            customer_base_url: customer_base_url.to_string(),
            pages_per_batch: 5,
            detail_concurrency: 2,
            include_fight: true,
        },
    )
    .await?;

    write_season_payload(season_output_dir, season, &fetch.payload)?;
    if fetch.entries == 0 {
        return Err(anyhow!(
            "未拉取到 S{season} 赛季对局；已保存本地 JSON，请确认该赛季有战绩，或稍后重新登录后再试。"
        ));
    }

    let entries = fetch
        .payload
        .pointer("/matches/entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let details = fetch
        .payload
        .pointer("/matches/details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_errors = fetch
        .payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    progress(SyncProgress {
        season,
        phase: "写入 SQLite".to_string(),
        processed: fetch.entries,
        total: fetch.entries,
        current_match_id: None,
        failed_details: detail_errors.len(),
        latest_errors: Vec::new(),
    });
    let summary = sync_import_parts(
        db,
        season_output_dir,
        season,
        &fetch.payload,
        entries,
        details,
        detail_errors,
        0,
    )?;
    progress(SyncProgress {
        season,
        phase: "完成".to_string(),
        processed: summary.processed,
        total: summary.total,
        current_match_id: None,
        failed_details: summary.failed_details,
        latest_errors: summary.latest_errors.clone(),
    });
    Ok(summary)
}

pub async fn sync_season_with_current_account_credentials_path_output_dir_base_url(
    db: &OwDatabase,
    season_output_dir: &Path,
    credential_path: &Path,
    season: i64,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    db.initialize()?;
    let credentials = read_credentials(credential_path)?;
    let customer_token = credentials
        .customer_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("当前登录账号尚未识别，无法同步本人数据，请重新登录。"))?;
    progress(SyncProgress {
        season,
        phase: "拉取赛季 JSON".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    let fetch = crate::season_fetch::fetch_full_season_with_base_url(
        crate::season_fetch::FullSeasonRequest {
            credentials: crate::dashen_client::DashenCredentials {
                role_id: credentials.role_id,
                token: credentials.token,
                dts: credentials.dts,
                server: credentials.server.unwrap_or(1),
            },
            customer_token: customer_token.to_string(),
            input_bnet: "当前登录账号".to_string(),
            resolved_bnet: None,
            season,
            customer_base_url: customer_base_url.to_string(),
            pages_per_batch: 5,
            detail_concurrency: 2,
            include_fight: true,
        },
    )
    .await?;
    write_season_payload(season_output_dir, season, &fetch.payload)?;
    if fetch.entries == 0 {
        return Err(anyhow!(
            "未拉取到 S{season} 赛季对局；已保存本地 JSON，请确认该赛季有战绩，或稍后重新登录后再试。"
        ));
    }
    let entries = fetch
        .payload
        .pointer("/matches/entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let details = fetch
        .payload
        .pointer("/matches/details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_errors = fetch
        .payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    progress(SyncProgress {
        season,
        phase: "写入 SQLite".to_string(),
        processed: fetch.entries,
        total: fetch.entries,
        current_match_id: None,
        failed_details: detail_errors.len(),
        latest_errors: Vec::new(),
    });
    let summary = sync_import_parts(
        db,
        season_output_dir,
        season,
        &fetch.payload,
        entries,
        details,
        detail_errors,
        0,
    )?;
    progress(SyncProgress {
        season,
        phase: "完成".to_string(),
        processed: summary.processed,
        total: summary.total,
        current_match_id: None,
        failed_details: summary.failed_details,
        latest_errors: summary.latest_errors.clone(),
    });
    Ok(summary)
}

pub async fn dump_season_json(
    project_root: &Path,
    season: i64,
    output_dir: &Path,
    progress: impl FnMut(SyncProgress),
) -> Result<SeasonJsonDump> {
    let credential_path = default_project_credential_path(project_root);
    dump_season_json_with_base_url(
        project_root,
        &credential_path,
        season,
        output_dir,
        DASHEN_CUSTOMER_BASE,
        progress,
    )
    .await
}

pub async fn dump_season_json_with_base_url(
    _project_root: &Path,
    credential_path: &Path,
    season: i64,
    output_dir: &Path,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SeasonJsonDump> {
    let parts =
        fetch_season_parts_with_base_url(credential_path, season, customer_base_url, &mut progress)
            .await?;
    let entries = parts.entries.len();
    let details = parts.details.len();
    let failed_details = parts.detail_errors.len();
    let skipped_duplicates = parts.skipped_duplicates;
    let payload = build_full_season_payload(
        season,
        &parts.cached_player,
        parts.entries,
        parts.details,
        parts.detail_errors,
        parts.list_pages,
    );
    let path = write_season_payload(output_dir, season, &payload)?;
    Ok(SeasonJsonDump {
        path,
        entries,
        details,
        failed_details,
        skipped_duplicates,
    })
}
