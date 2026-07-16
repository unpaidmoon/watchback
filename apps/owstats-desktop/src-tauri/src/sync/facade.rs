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
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    let credentials = read_credentials(credential_path)?;
    let target = crate::player_target::read_player_target_for_path(target_path)?;
    if !crate::player_target::target_matches_login_role(&target, credentials.role_id) {
        return Err(anyhow!(crate::login_target::SYNC_TARGET_CONFIRMATION_ERROR));
    }
    sync_incremental_season_with_request(
        db,
        season_output_dir,
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
        progress,
    )
    .await
}

pub async fn sync_season_with_current_account_credentials_path_output_dir_base_url(
    db: &OwDatabase,
    season_output_dir: &Path,
    credential_path: &Path,
    season: i64,
    customer_base_url: &str,
    progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    let credentials = read_credentials(credential_path)?;
    let customer_token = credentials
        .customer_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("当前登录账号尚未识别，无法同步本人数据，请重新登录。"))?;
    sync_incremental_season_with_request(
        db,
        season_output_dir,
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
        progress,
    )
    .await
}

async fn sync_incremental_season_with_request(
    db: &OwDatabase,
    season_output_dir: &Path,
    request: crate::season_fetch::FullSeasonRequest,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    let season = request.season;
    progress(SyncProgress {
        season,
        phase: "正在检查本地数据".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    let inventory = load_season_sync_inventory(db, season)?;
    progress(SyncProgress {
        season,
        phase: "正在获取赛季对局列表".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    let fetch =
        crate::season_fetch::fetch_season_increment_with_base_url(request, &inventory.detail_ids)
            .await?;
    if fetch.entries == 0 {
        return Err(anyhow!(
            "未拉取到 S{season} 赛季对局；未更新本地数据库或快照，请确认该赛季有战绩，或稍后重新登录后再试。"
        ));
    }
    let entries = fetch
        .payload
        .pointer("/matches/entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selection = select_detail_entries(&entries, &inventory);
    progress(SyncProgress {
        season,
        phase: format!(
            "发现 {} 场新对局，{} 场缺少详情",
            selection.new_matches, selection.missing_details
        ),
        processed: 0,
        total: fetch.details_requested,
        current_match_id: None,
        failed_details: fetch.failed_details,
        latest_errors: Vec::new(),
    });
    let details = fetch
        .payload
        .pointer("/matches/details")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    progress(SyncProgress {
        season,
        phase: "正在获取详情".to_string(),
        processed: fetch.details_requested,
        total: fetch.details_requested,
        current_match_id: None,
        failed_details: fetch.failed_details,
        latest_errors: Vec::new(),
    });
    let detail_errors = fetch
        .payload
        .pointer("/matches/detail_errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let list_pages = fetch
        .payload
        .pointer("/matches/list_pages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let page_diagnostics = build_page_diagnostics(&list_pages, &inventory);
    let attempted_detail_ids = selection
        .entries
        .iter()
        .filter_map(|entry| string_field(entry, "matchId"))
        .collect::<HashSet<_>>();
    let repair_candidates = attempted_detail_ids
        .iter()
        .filter(|match_id| {
            inventory.match_ids.contains(*match_id) && !inventory.detail_ids.contains(*match_id)
        })
        .cloned()
        .collect::<HashSet<_>>();
    let repaired_details = details
        .iter()
        .filter_map(|detail| string_field(detail, "match_id").map(|match_id| (match_id, detail)))
        .filter(|(match_id, detail)| {
            repair_candidates.contains(match_id)
                && crate::importer::is_valid_match_detail_for(detail, match_id)
        })
        .map(|(match_id, _)| match_id)
        .collect::<HashSet<_>>()
        .len();
    let persist_started = std::time::Instant::now();
    progress(SyncProgress {
        season,
        phase: "正在更新本地数据库".to_string(),
        processed: fetch.details_requested.saturating_sub(fetch.failed_details),
        total: fetch.details_requested,
        current_match_id: None,
        failed_details: detail_errors.len(),
        latest_errors: Vec::new(),
    });
    let mut summary = persist::sync_import_incremental_parts(
        db,
        season,
        &fetch.payload,
        entries,
        details,
        detail_errors,
        attempted_detail_ids,
    )?;
    let (database_matches, database_details) =
        incremental::query_season_database_totals(db, season)?;
    let complete_payload =
        rebuild_complete_season_payload(db, season, &fetch.payload).map_err(|error| {
            anyhow!(
                "数据库更新成功，但本地快照更新失败：{}",
                crate::redaction::redact_sensitive_text(&error.to_string())
            )
        })?;
    write_season_payload(season_output_dir, season, &complete_payload).map_err(|error| {
        anyhow!(
            "数据库更新成功，但本地快照更新失败：{}",
            crate::redaction::redact_sensitive_text(&error.to_string())
        )
    })?;
    summary.list_entries_seen = fetch.entries;
    summary.new_matches = selection.new_matches;
    summary.missing_details = selection.missing_details;
    summary.details_requested = fetch.details_requested;
    summary.repaired_details = repaired_details;
    summary.database_matches = database_matches;
    summary.database_details = database_details;
    summary.diagnostics = crate::models::SyncDiagnostics {
        list_duration_ms: fetch.list_duration_ms,
        detail_duration_ms: fetch.detail_duration_ms,
        persist_duration_ms: persist_started.elapsed().as_millis() as u64,
        pages: page_diagnostics,
    };
    progress(SyncProgress {
        season,
        phase: "同步完成".to_string(),
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
