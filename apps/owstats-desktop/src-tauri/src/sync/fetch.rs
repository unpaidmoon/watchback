use super::*;

pub(super) async fn fetch_season_parts_with_base_url(
    credential_path: &Path,
    season: i64,
    customer_base_url: &str,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<SeasonFetchParts> {
    if season <= 0 {
        return Err(anyhow!("season must be greater than 0"));
    }
    progress(SyncProgress {
        season,
        phase: "读取本地凭证".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    let credentials = read_credentials(credential_path)?;
    let customer_token = credentials
        .customer_token
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("请先绑定目标玩家，再同步赛季数据"))?;
    let cached_player = json!({
        "input_bnet": Value::Null,
        "resolved_bnet": Value::Null,
        "profile_card": Value::Null
    });
    let client = reqwest::Client::builder()
        .default_headers(auth_headers(&credentials)?)
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()?;

    let mut entries = Vec::<Value>::new();
    let mut list_pages = Vec::<Value>::new();
    progress(SyncProgress {
        season,
        phase: "拉取赛季列表".to_string(),
        processed: 0,
        total: 0,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    for mode in NORMAL_MODES {
        let fetched = fetch_mode_pages(
            &client,
            customer_base_url,
            &customer_token,
            mode,
            season,
            false,
        )
        .await?;
        entries.extend(fetched.entries);
        list_pages.extend(fetched.list_pages);
    }
    for mode in FIGHT_MODES {
        let fetched = fetch_mode_pages(
            &client,
            customer_base_url,
            &customer_token,
            mode,
            season,
            true,
        )
        .await?;
        entries.extend(fetched.entries);
        list_pages.extend(fetched.list_pages);
    }
    let before_dedupe = entries.len();
    entries = dedupe_entries(entries, season);
    let skipped_duplicates = before_dedupe.saturating_sub(entries.len());

    let mut details = Vec::<Value>::new();
    let mut detail_errors = Vec::<Value>::new();
    let total = entries.len();
    progress(SyncProgress {
        season,
        phase: "拉取单局详情".to_string(),
        processed: 0,
        total,
        current_match_id: None,
        failed_details: 0,
        latest_errors: Vec::new(),
    });
    for entry in &entries {
        let match_id = string_field(entry, "matchId").unwrap_or_default();
        if match_id.is_empty() {
            continue;
        }
        match fetch_detail(&client, customer_base_url, &customer_token, entry).await {
            Ok(detail) => details.push(detail),
            Err(error) => detail_errors.push(json!({
                "match_id": match_id,
                "error_type": "DetailFetchError",
                "message": error.to_string(),
            })),
        }
        let latest_errors = detail_errors
            .iter()
            .rev()
            .take(5)
            .filter_map(|item| string_field(item, "message"))
            .collect::<Vec<_>>();
        progress(SyncProgress {
            season,
            phase: "拉取单局详情".to_string(),
            processed: details.len() + detail_errors.len(),
            total,
            current_match_id: Some(match_id),
            failed_details: detail_errors.len(),
            latest_errors,
        });
    }

    Ok(SeasonFetchParts {
        cached_player,
        entries,
        details,
        detail_errors,
        list_pages,
        skipped_duplicates,
    })
}
