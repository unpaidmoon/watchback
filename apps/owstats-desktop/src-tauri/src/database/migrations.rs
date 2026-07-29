use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use serde_json::Value;

pub const CURRENT_SCHEMA_VERSION: i64 = 14;

struct Migration {
    version: i64,
    name: &'static str,
    apply: fn(&Transaction<'_>) -> Result<()>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "baseline schema",
        apply: migrate_v1,
    },
    Migration {
        version: 2,
        name: "match player role type",
        apply: migrate_v2,
    },
    Migration {
        version: 3,
        name: "arena player projection",
        apply: migrate_v3,
    },
    Migration {
        version: 4,
        name: "sync error latest snapshot",
        apply: migrate_v4,
    },
    Migration {
        version: 5,
        name: "built-in map dictionary backfill",
        apply: migrate_v5,
    },
    Migration {
        version: 7,
        name: "normal match mode normalization",
        apply: migrate_v7,
    },
    Migration {
        version: 8,
        name: "match favorites and notes",
        apply: migrate_v8,
    },
    Migration {
        version: 9,
        name: "durable season sync state",
        apply: migrate_v9,
    },
    Migration {
        version: 11,
        name: "permanent match AI roasts",
        apply: migrate_v11,
    },
    Migration {
        version: 12,
        name: "structured AI roast reports",
        apply: migrate_v12,
    },
    Migration {
        version: 13,
        name: "remove abandoned rating prototype",
        apply: migrate_v13,
    },
    Migration {
        version: 14,
        name: "match timeline index",
        apply: migrate_v14,
    },
];

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > CURRENT_SCHEMA_VERSION {
        return Err(anyhow!(
            "数据库版本高于当前程序，请升级 Watchback 后再打开此数据"
        ));
    }
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current)
    {
        let transaction = conn.unchecked_transaction()?;
        (migration.apply)(&transaction).map_err(|error| {
            anyhow!(
                "数据库迁移 v{}（{}）失败：{error}",
                migration.version,
                migration.name
            )
        })?;
        transaction.pragma_update(None, "user_version", migration.version)?;
        transaction.commit()?;
    }
    Ok(())
}

fn migrate_v1(_: &Transaction<'_>) -> Result<()> {
    Ok(())
}

fn migrate_v2(transaction: &Transaction<'_>) -> Result<()> {
    add_column_if_missing(transaction, "match_players", "role_type", "TEXT")?;
    transaction.execute("CREATE INDEX IF NOT EXISTS idx_match_players_role ON match_players(season, is_current_player, role_type)", [])?;
    Ok(())
}

fn migrate_v3(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS arena_match_players (
            match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
            season INTEGER NOT NULL,
            side TEXT NOT NULL,
            bnet_id TEXT NOT NULL,
            player_name TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (match_id, side, bnet_id)
        );
        CREATE INDEX IF NOT EXISTS idx_arena_match_players_bnet ON arena_match_players(bnet_id, season);
        CREATE INDEX IF NOT EXISTS idx_arena_match_players_name ON arena_match_players(player_name, season);
    "#)?;
    Ok(())
}

fn migrate_v4(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        DELETE FROM sync_errors
        WHERE id NOT IN (
            SELECT MIN(id)
            FROM sync_errors
            GROUP BY season, match_id, error_type, message
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_errors_snapshot_unique
            ON sync_errors(
                IFNULL(season, -1),
                IFNULL(match_id, ''),
                IFNULL(error_type, ''),
                IFNULL(message, '')
            );
        "#,
    )?;
    Ok(())
}

fn migrate_v7(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute(
        r#"
        UPDATE matches
        SET game_mode = CASE
            WHEN json_valid(raw_entry_json) THEN CASE
                LOWER(TRIM(game_mode)) || ':' || LOWER(TRIM(json_extract(raw_entry_json, '$.instanceType')))
                WHEN 'leisure:it_unranked' THEN 'LeisurePreset'
                WHEN 'leisure:it_ruleset' THEN 'Leisure6v6'
                WHEN 'sport:it_ranked' THEN 'SportPreset'
                WHEN 'sport:it_ruleset' THEN 'SportOpen'
                ELSE game_mode
            END
            ELSE game_mode
        END
        WHERE LOWER(TRIM(game_mode)) IN ('leisure', 'sport')
        "#,
        [],
    )?;
    Ok(())
}

fn migrate_v8(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS match_favorites (
            match_id TEXT PRIMARY KEY REFERENCES matches(match_id) ON DELETE CASCADE,
            note TEXT NOT NULL DEFAULT '' CHECK(length(note) <= 500),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_match_favorites_updated
            ON match_favorites(updated_at DESC);
        "#,
    )?;
    Ok(())
}

fn migrate_v9(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS season_sync_state (
            season INTEGER PRIMARY KEY,
            season_revision INTEGER NOT NULL DEFAULT 0,
            snapshot_revision INTEGER NOT NULL DEFAULT 0,
            last_checked_at TEXT,
            background_status TEXT NOT NULL DEFAULT 'idle',
            pending_detail_count INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS season_sync_cursors (
            season INTEGER NOT NULL,
            match_kind TEXT NOT NULL,
            game_mode TEXT NOT NULL,
            request_shape TEXT NOT NULL CHECK(request_shape IN ('implicitCurrentSeason', 'explicitSeason')),
            latest_begin_ts INTEGER,
            empty_confirmed INTEGER NOT NULL DEFAULT 0 CHECK(empty_confirmed IN (0, 1)),
            last_checked_at TEXT,
            last_full_fetch_at TEXT,
            last_stop_reason TEXT,
            PRIMARY KEY (season, match_kind, game_mode)
        );

        CREATE TABLE IF NOT EXISTS season_sync_jobs (
            job_id TEXT PRIMARY KEY,
            season INTEGER NOT NULL,
            target_revision INTEGER NOT NULL,
            phase TEXT NOT NULL,
            status TEXT NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            next_retry_at TEXT,
            error_class TEXT,
            created_at TEXT NOT NULL,
            started_at TEXT,
            completed_at TEXT,
            updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_season_sync_one_active_job
            ON season_sync_jobs(season)
            WHERE status IN ('queued', 'running', 'retryable');

        CREATE TABLE IF NOT EXISTS season_sync_job_modes (
            job_id TEXT NOT NULL REFERENCES season_sync_jobs(job_id) ON DELETE CASCADE,
            match_kind TEXT NOT NULL,
            game_mode TEXT NOT NULL,
            request_shape TEXT NOT NULL CHECK(request_shape IN ('implicitCurrentSeason', 'explicitSeason')),
            next_page INTEGER NOT NULL,
            max_page INTEGER NOT NULL DEFAULT 5,
            stop_reason TEXT,
            status TEXT NOT NULL,
            PRIMARY KEY (job_id, match_kind, game_mode)
        );

        CREATE TABLE IF NOT EXISTS season_sync_job_matches (
            job_id TEXT NOT NULL REFERENCES season_sync_jobs(job_id) ON DELETE CASCADE,
            match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
            detail_status TEXT NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            error_class TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (job_id, match_id)
        );
        "#,
    )?;

    let now = Utc::now().to_rfc3339();
    transaction.execute(
        r#"
        INSERT INTO season_sync_state (
            season, season_revision, snapshot_revision, background_status,
            pending_detail_count, updated_at
        )
        SELECT season, 0, 0, 'idle', 0, ?1
        FROM matches
        GROUP BY season
        ON CONFLICT(season) DO NOTHING
        "#,
        [&now],
    )?;
    transaction.execute(
        r#"
        INSERT INTO season_sync_cursors (
            season, match_kind, game_mode, request_shape,
            latest_begin_ts, empty_confirmed, last_stop_reason
        )
        SELECT
            season,
            CASE
                WHEN game_mode IN ('sport', 'SportPreset', 'SportOpen') THEN 'normal'
                WHEN game_mode IN ('leisure', 'LeisurePreset', 'Leisure6v6') THEN 'normal'
                ELSE match_kind
            END,
            CASE
                WHEN game_mode IN ('sport', 'SportPreset', 'SportOpen') THEN 'sport'
                WHEN game_mode IN ('leisure', 'LeisurePreset', 'Leisure6v6') THEN 'leisure'
                ELSE game_mode
            END,
            CASE WHEN season = ?1 THEN 'implicitCurrentSeason' ELSE 'explicitSeason' END,
            MAX(begin_ts),
            0,
            'migratedWatermark'
        FROM matches
        WHERE game_mode IN (
            'sport', 'SportPreset', 'SportOpen',
            'leisure', 'LeisurePreset', 'Leisure6v6',
            'QuickFight', 'LeisureFight', 'SportFight'
        )
        GROUP BY season, 2, 3
        ON CONFLICT(season, match_kind, game_mode) DO UPDATE SET
            latest_begin_ts = MAX(season_sync_cursors.latest_begin_ts, excluded.latest_begin_ts)
        "#,
        [crate::season_fetch::DASHEN_CURRENT_SEASON],
    )?;
    Ok(())
}

fn migrate_v11(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS match_ai_roasts (
            match_id TEXT PRIMARY KEY REFERENCES matches(match_id) ON DELETE CASCADE,
            award TEXT NOT NULL CHECK(length(award) BETWEEN 1 AND 32),
            commentary TEXT NOT NULL CHECK(length(commentary) BETWEEN 1 AND 720),
            used_fact_ids_json TEXT NOT NULL CHECK(length(used_fact_ids_json) > 0),
            model TEXT NOT NULL CHECK(length(model) > 0),
            evaluation_rule_version INTEGER NOT NULL,
            material_version INTEGER NOT NULL,
            prompt_version INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}

fn migrate_v12(transaction: &Transaction<'_>) -> Result<()> {
    add_column_if_missing(transaction, "match_ai_roasts", "report_json", "TEXT")?;
    add_column_if_missing(
        transaction,
        "match_ai_roasts",
        "report_schema_version",
        "INTEGER",
    )?;
    add_column_if_missing(
        transaction,
        "match_ai_roasts",
        "input_schema_version",
        "INTEGER",
    )?;
    add_column_if_missing(
        transaction,
        "match_ai_roasts",
        "knowledge_version",
        "INTEGER",
    )?;
    Ok(())
}

fn migrate_v14(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute(
        "CREATE INDEX IF NOT EXISTS idx_matches_timeline ON matches(begin_ts DESC, match_id)",
        [],
    )?;
    Ok(())
}

fn migrate_v5(transaction: &Transaction<'_>) -> Result<()> {
    let lookup: Value =
        serde_json::from_str(include_str!("../../../runtime-config/ow-lookups.json"))?;
    let maps = lookup
        .get("maps")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("内置地图字典缺少 maps"))?;
    let now = Utc::now().to_rfc3339();
    for (guid, item) in maps {
        let Some(name) = item
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        transaction.execute(
            r#"
            INSERT INTO map_dict (guid, name, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(guid) DO UPDATE SET name=excluded.name, updated_at=excluded.updated_at
            "#,
            params![guid, name, now],
        )?;
    }
    transaction.execute(
        r#"
        UPDATE matches
        SET map_name = (
            SELECT name
            FROM map_dict
            WHERE map_dict.guid = matches.map_guid
        )
        WHERE (map_name IS NULL OR TRIM(map_name) = '' OR map_name = map_guid)
          AND EXISTS (
            SELECT 1
            FROM map_dict
            WHERE map_dict.guid = matches.map_guid
          )
        "#,
        [],
    )?;
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|item| item == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn migrate_v13(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        DROP TABLE IF EXISTS career_mmr_model_artifacts;
        DROP TABLE IF EXISTS career_mmr_events;
        DROP TABLE IF EXISTS career_role_mode_summary;
        DROP TABLE IF EXISTS career_role_season_summary;
        DROP TABLE IF EXISTS career_role_mmr;
        DROP TABLE IF EXISTS mmr_match_inputs;
        DROP TABLE IF EXISTS mmr_model_versions;
        "#,
    )?;
    Ok(())
}
