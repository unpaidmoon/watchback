use crate::db::OwDatabase;
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestShape {
    ImplicitCurrentSeason,
    ExplicitSeason,
}

impl RequestShape {
    pub fn for_season(season: i64) -> Self {
        if season == crate::season_fetch::DASHEN_CURRENT_SEASON {
            Self::ImplicitCurrentSeason
        } else {
            Self::ExplicitSeason
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImplicitCurrentSeason => "implicitCurrentSeason",
            Self::ExplicitSeason => "explicitSeason",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "implicitCurrentSeason" => Ok(Self::ImplicitCurrentSeason),
            "explicitSeason" => Ok(Self::ExplicitSeason),
            _ => Err(anyhow!("invalid season request shape")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeWork {
    pub match_kind: String,
    pub game_mode: String,
    pub request_shape: RequestShape,
    pub next_page: i64,
    pub max_page: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedSeasonJob {
    pub job_id: String,
    pub season: i64,
    pub target_revision: i64,
    pub phase: String,
    pub modes: Vec<ModeWork>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SeasonBackgroundStatus {
    pub season: i64,
    pub phase: String,
    pub status: String,
    pub pending_details: usize,
    pub retryable: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct SeasonSyncStore {
    db: OwDatabase,
}

impl SeasonSyncStore {
    pub fn new(db: OwDatabase) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &OwDatabase {
        &self.db
    }

    pub fn enqueue_or_merge(
        &self,
        season: i64,
        target_revision: i64,
        modes: &[ModeWork],
        match_ids: &[String],
    ) -> Result<String> {
        self.db.initialize()?;
        let mut conn = self.db.connect()?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        ensure_season_state(&tx, season, &now)?;
        let existing = tx
            .query_row(
                r#"
                SELECT job_id
                FROM season_sync_jobs
                WHERE season=?1 AND status IN ('queued', 'running', 'retryable')
                ORDER BY created_at
                LIMIT 1
                "#,
                [season],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let job_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        tx.execute(
            r#"
            INSERT INTO season_sync_jobs (
                job_id, season, target_revision, phase, status,
                retry_count, created_at, updated_at
            ) VALUES (?1, ?2, ?3, 'continuePages', 'queued', 0, ?4, ?4)
            ON CONFLICT(job_id) DO UPDATE SET
                target_revision=MAX(target_revision, excluded.target_revision),
                updated_at=excluded.updated_at
            "#,
            params![job_id, season, target_revision, now],
        )?;
        for mode in modes {
            tx.execute(
                r#"
                INSERT INTO season_sync_job_modes (
                    job_id, match_kind, game_mode, request_shape,
                    next_page, max_page, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued')
                ON CONFLICT(job_id, match_kind, game_mode) DO UPDATE SET
                    next_page=MIN(next_page, excluded.next_page),
                    max_page=MAX(max_page, excluded.max_page),
                    request_shape=excluded.request_shape,
                    status=CASE WHEN status='completed' THEN status ELSE 'queued' END
                "#,
                params![
                    job_id,
                    mode.match_kind,
                    mode.game_mode,
                    mode.request_shape.as_str(),
                    mode.next_page,
                    mode.max_page
                ],
            )?;
        }
        for match_id in match_ids {
            tx.execute(
                r#"
                INSERT INTO season_sync_job_matches (
                    job_id, match_id, detail_status, updated_at
                ) VALUES (?1, ?2, 'queued', ?3)
                ON CONFLICT(job_id, match_id) DO NOTHING
                "#,
                params![job_id, match_id, now],
            )?;
        }
        tx.execute(
            r#"
            UPDATE season_sync_state
            SET background_status='queued',
                pending_detail_count=(
                    SELECT COUNT(*) FROM season_sync_job_matches
                    WHERE job_id=?2 AND detail_status IN ('queued', 'retryable')
                ),
                updated_at=?3
            WHERE season=?1
            "#,
            params![season, job_id, now],
        )?;
        tx.commit()?;
        Ok(job_id)
    }

    pub fn claim_next(&self, season: i64) -> Result<Option<ClaimedSeasonJob>> {
        self.db.initialize()?;
        let mut conn = self.db.connect()?;
        let tx = conn.transaction()?;
        let row = tx
            .query_row(
                r#"
                SELECT job_id, target_revision, phase
                FROM season_sync_jobs
                WHERE season=?1 AND status IN ('queued', 'retryable')
                ORDER BY created_at
                LIMIT 1
                "#,
                [season],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((job_id, target_revision, phase)) = row else {
            tx.commit()?;
            return Ok(None);
        };
        let now = Utc::now().to_rfc3339();
        let claimed = tx.execute(
            r#"
            UPDATE season_sync_jobs
            SET status='running', started_at=COALESCE(started_at, ?2), updated_at=?2
            WHERE job_id=?1 AND status IN ('queued', 'retryable')
            "#,
            params![job_id, now],
        )?;
        if claimed != 1 {
            tx.commit()?;
            return Ok(None);
        }
        let modes = {
            let mut statement = tx.prepare(
                r#"
                SELECT match_kind, game_mode, request_shape, next_page, max_page
                FROM season_sync_job_modes
                WHERE job_id=?1 AND status != 'completed'
                ORDER BY match_kind, game_mode
                "#,
            )?;
            let rows = statement.query_map([&job_id], |row| {
                let raw_shape = row.get::<_, String>(2)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    raw_shape,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?;
            let mut modes = Vec::new();
            for row in rows {
                let (match_kind, game_mode, raw_shape, next_page, max_page) = row?;
                modes.push(ModeWork {
                    match_kind,
                    game_mode,
                    request_shape: RequestShape::parse(&raw_shape)?,
                    next_page,
                    max_page,
                });
            }
            modes
        };
        tx.execute(
            "UPDATE season_sync_state SET background_status=?2, updated_at=?3 WHERE season=?1",
            params![season, phase, now],
        )?;
        tx.commit()?;
        Ok(Some(ClaimedSeasonJob {
            job_id,
            season,
            target_revision,
            phase,
            modes,
        }))
    }

    pub fn requeue_orphaned_running(&self) -> Result<usize> {
        self.db.initialize()?;
        let conn = self.db.connect()?;
        let now = Utc::now().to_rfc3339();
        Ok(conn.execute(
            "UPDATE season_sync_jobs SET status='queued', started_at=NULL, updated_at=?1 WHERE status='running'",
            [now],
        )?)
    }

    pub fn active_job_count(&self, season: i64) -> Result<usize> {
        self.db.initialize()?;
        let conn = self.db.connect()?;
        let count = conn.query_row(
            "SELECT COUNT(*) FROM season_sync_jobs WHERE season=?1 AND status IN ('queued','running','retryable')",
            [season],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count as usize)
    }

    pub fn retry_season(&self, season: i64) -> Result<bool> {
        self.db.initialize()?;
        let mut conn = self.db.connect()?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            r#"
            UPDATE season_sync_jobs
            SET status='queued', error_class=NULL, next_retry_at=NULL, updated_at=?2
            WHERE season=?1 AND status='retryable'
            "#,
            params![season, now],
        )?;
        if changed > 0 {
            tx.execute(
                "UPDATE season_sync_state SET background_status='queued', updated_at=?2 WHERE season=?1",
                params![season, now],
            )?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn reconcile_full_fetch(&self, season: i64) -> Result<()> {
        self.db.initialize()?;
        let mut conn = self.db.connect()?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        ensure_season_state(&tx, season, &now)?;
        tx.execute(
            r#"
            UPDATE season_sync_jobs
            SET status='superseded', phase='completed', completed_at=?2, updated_at=?2
            WHERE season=?1 AND status IN ('queued','running','retryable')
            "#,
            params![season, now],
        )?;
        tx.execute(
            r#"
            INSERT INTO season_sync_cursors (
                season, match_kind, game_mode, request_shape,
                latest_begin_ts, empty_confirmed, last_stop_reason,
                last_checked_at, last_full_fetch_at
            )
            SELECT season, match_kind, game_mode, ?2,
                   MAX(begin_ts), 0, 'fullFetch', ?3, ?3
            FROM matches
            WHERE season=?1
            GROUP BY season, match_kind, game_mode
            ON CONFLICT(season, match_kind, game_mode) DO UPDATE SET
                request_shape=excluded.request_shape,
                latest_begin_ts=excluded.latest_begin_ts,
                last_stop_reason='fullFetch',
                last_checked_at=excluded.last_checked_at,
                last_full_fetch_at=excluded.last_full_fetch_at
            "#,
            params![season, RequestShape::for_season(season).as_str(), now],
        )?;
        tx.execute(
            r#"
            UPDATE season_sync_state
            SET season_revision=season_revision+1,
                snapshot_revision=season_revision+1,
                background_status='completed', pending_detail_count=0,
                last_checked_at=?2, updated_at=?2
            WHERE season=?1
            "#,
            params![season, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn season_status(&self, season: i64) -> Result<SeasonBackgroundStatus> {
        self.db.initialize()?;
        let conn = self.db.connect()?;
        let row = conn
            .query_row(
                r#"
                SELECT s.background_status, s.pending_detail_count, s.updated_at,
                       COALESCE(j.phase, 'idle'), COALESCE(j.status, 'idle')
                FROM season_sync_state s
                LEFT JOIN season_sync_jobs j ON j.season=s.season
                    AND j.status IN ('queued','running','retryable')
                WHERE s.season=?1
                ORDER BY j.created_at DESC
                LIMIT 1
                "#,
                [season],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((background, pending, updated_at, phase, job_status)) = row else {
            return Ok(SeasonBackgroundStatus {
                season,
                phase: "idle".to_string(),
                status: "idle".to_string(),
                pending_details: 0,
                retryable: false,
                updated_at: String::new(),
            });
        };
        let retryable = job_status == "retryable";
        Ok(SeasonBackgroundStatus {
            season,
            phase,
            status: if job_status == "idle" {
                background
            } else {
                job_status
            },
            pending_details: pending.max(0) as usize,
            retryable,
            updated_at,
        })
    }
}

pub(crate) fn ensure_season_state(tx: &Transaction<'_>, season: i64, now: &str) -> Result<()> {
    tx.execute(
        r#"
        INSERT INTO season_sync_state (season, updated_at)
        VALUES (?1, ?2)
        ON CONFLICT(season) DO NOTHING
        "#,
        params![season, now],
    )?;
    Ok(())
}
