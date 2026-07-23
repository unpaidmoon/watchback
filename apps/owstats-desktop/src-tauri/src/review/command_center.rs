use super::*;

pub(super) const MIN_SYNC_SEASON: i64 = 15;
pub(super) const DEFAULT_SYNC_SEASON: i64 = 23;

pub(super) fn get_onboarding_status_impl(
    db: &OwDatabase,
    request: OnboardingStatusRequest,
) -> Result<OnboardingStatus> {
    db.initialize()?;
    let conn = db.connect()?;
    let available_seasons = {
        let mut stmt = conn.prepare("SELECT DISTINCT season FROM matches ORDER BY season")?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        rows
    };
    let selected = match request.season {
        Some(season) if season < MIN_SYNC_SEASON => {
            anyhow::bail!("最低支持 S{MIN_SYNC_SEASON}");
        }
        Some(season) => season,
        None => available_seasons
            .iter()
            .copied()
            .filter(|season| *season >= MIN_SYNC_SEASON)
            .max()
            .unwrap_or(DEFAULT_SYNC_SEASON),
    };
    let status_seasons = available_seasons
        .iter()
        .copied()
        .filter(|season| *season >= MIN_SYNC_SEASON)
        .rev()
        .collect::<Vec<_>>();
    let season_statuses = command_center_season_statuses(db, &status_seasons)?;
    let (preset_matches, detail_matches, last_seen) = conn.query_row(
        r#"
            SELECT
                COUNT(m.match_id),
                COUNT(d.match_id),
                MAX(m.updated_at)
            FROM matches m
            LEFT JOIN match_details d ON d.match_id = m.match_id
            WHERE m.bucket='preset' AND m.season=?
            "#,
        params![selected],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let can_enter_review = preset_matches > 0;
    let mut warnings = Vec::new();
    if preset_matches == 0 {
        warnings.push("没有可读取的本地预设/开放赛季数据。".to_string());
    } else if detail_matches < preset_matches {
        warnings.push("部分比赛详情缺失，但仍可进入复盘。".to_string());
    }

    Ok(OnboardingStatus {
        selected_season: Some(selected),
        minimum_sync_season: MIN_SYNC_SEASON,
        default_season: DEFAULT_SYNC_SEASON,
        available_seasons,
        can_enter_review,
        credential_status: CredentialStatus {
            state: "external".to_string(),
            message: "沿用第二阶段外部凭证流程。".to_string(),
            exposes_secret: false,
            role_id: None,
            saved_at: None,
        },
        local_data: LocalDataStatus {
            data_source: "本地 SQLite".to_string(),
            season: Some(selected),
            preset_matches,
            detail_matches,
            last_synced_or_imported_at: last_seen,
            freshness_risk: if can_enter_review {
                "未知".to_string()
            } else {
                "没有本地数据".to_string()
            },
        },
        season_statuses,
        health: DataHealthStatus {
            readable: can_enter_review,
            warnings,
        },
    })
}

pub(super) fn command_center_season_statuses(
    db: &OwDatabase,
    seasons: &[i64],
) -> Result<Vec<SeasonDataStatus>> {
    seasons
        .iter()
        .map(|season| command_center_season_status(db, *season))
        .collect()
}

pub(super) fn command_center_season_status(
    db: &OwDatabase,
    season: i64,
) -> Result<SeasonDataStatus> {
    let conn = db.connect()?;
    let (match_count, normal_match_count, arena_match_count, match_updated_at): (
        i64,
        i64,
        i64,
        Option<String>,
    ) = conn.query_row(
        r#"
        SELECT
            COUNT(*),
            COALESCE(SUM(CASE WHEN bucket='preset' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN bucket='stadium' OR match_kind='fight' OR game_mode IN ('LeisureFight', 'SportFight') THEN 1 ELSE 0 END), 0),
            MAX(updated_at)
        FROM matches
        WHERE season=?
        "#,
        params![season],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let (detail_count, detail_updated_at): (i64, Option<String>) = conn.query_row(
        r#"
        SELECT COUNT(d.match_id), MAX(d.updated_at)
        FROM match_details d
        JOIN matches m ON m.match_id=d.match_id
        WHERE m.season=?
        "#,
        params![season],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let failed_detail_count = (match_count - detail_count).max(0);
    let coverage_percent = if match_count > 0 {
        ((detail_count * 100) / match_count).clamp(0, 100)
    } else {
        0
    };

    Ok(SeasonDataStatus {
        season,
        match_count,
        detail_count,
        normal_match_count,
        arena_match_count,
        failed_detail_count,
        last_synced_or_imported_at: latest_timestamp(match_updated_at, detail_updated_at),
        coverage_percent,
    })
}

pub(super) fn latest_timestamp(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if left >= right { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
