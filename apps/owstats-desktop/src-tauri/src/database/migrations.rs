use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use serde_json::Value;

pub const CURRENT_SCHEMA_VERSION: i64 = 8;

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
        version: 6,
        name: "career role mmr",
        apply: migrate_v6,
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
];

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > CURRENT_SCHEMA_VERSION {
        return Err(anyhow!(
            "数据库版本高于当前程序，请升级 OWStatsLocal 后再打开此数据"
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
    let transaction = conn.unchecked_transaction()?;
    add_column_if_missing(
        &transaction,
        "career_role_mode_summary",
        "draws",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    transaction.commit()?;
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

fn migrate_v6(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS mmr_model_versions (
            version INTEGER PRIMARY KEY,
            algorithm TEXT NOT NULL,
            config_json TEXT NOT NULL,
            is_active INTEGER NOT NULL CHECK(is_active IN (0, 1)),
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS mmr_match_inputs (
            match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
            role TEXT NOT NULL CHECK(role IN ('tank', 'damage', 'support')),
            season INTEGER NOT NULL,
            begin_ts INTEGER NOT NULL,
            mode_family TEXT NOT NULL,
            result REAL NOT NULL CHECK(result BETWEEN 0.0 AND 1.0),
            duration_sec INTEGER NOT NULL,
            role_share REAL NOT NULL CHECK(role_share > 0.0 AND role_share <= 1.0),
            role_source TEXT NOT NULL,
            data_quality REAL NOT NULL CHECK(data_quality BETWEEN 0.0 AND 1.0),
            team_strength REAL,
            opponent_strength REAL,
            rank_coverage REAL NOT NULL CHECK(rank_coverage BETWEEN 0.0 AND 1.0),
            performance_features_json TEXT NOT NULL DEFAULT '{}',
            input_hash TEXT NOT NULL,
            PRIMARY KEY (match_id, role)
        );

        CREATE TABLE IF NOT EXISTS career_mmr_events (
            model_version INTEGER NOT NULL REFERENCES mmr_model_versions(version),
            match_id TEXT NOT NULL REFERENCES matches(match_id) ON DELETE CASCADE,
            role TEXT NOT NULL CHECK(role IN ('tank', 'damage', 'support')),
            season INTEGER NOT NULL,
            begin_ts INTEGER NOT NULL,
            mode_family TEXT NOT NULL,
            role_share REAL NOT NULL CHECK(role_share > 0.0 AND role_share <= 1.0),
            result REAL NOT NULL CHECK(result BETWEEN 0.0 AND 1.0),
            expected_win_rate REAL NOT NULL CHECK(expected_win_rate BETWEEN 0.0 AND 1.0),
            data_quality REAL NOT NULL CHECK(data_quality BETWEEN 0.0 AND 1.0),
            performance_adjustment REAL NOT NULL DEFAULT 0.0,
            mu_before REAL NOT NULL,
            sigma_before REAL NOT NULL,
            base_delta REAL NOT NULL,
            final_delta REAL NOT NULL,
            mu_after REAL NOT NULL,
            sigma_after REAL NOT NULL,
            confidence_after REAL NOT NULL CHECK(confidence_after BETWEEN 0.0 AND 1.0),
            input_hash TEXT NOT NULL,
            computed_at TEXT NOT NULL,
            PRIMARY KEY (model_version, match_id, role)
        );

        CREATE TABLE IF NOT EXISTS career_role_mmr (
            model_version INTEGER NOT NULL REFERENCES mmr_model_versions(version),
            role TEXT NOT NULL CHECK(role IN ('tank', 'damage', 'support')),
            mmr INTEGER NOT NULL,
            mu REAL NOT NULL,
            sigma REAL NOT NULL,
            confidence REAL NOT NULL CHECK(confidence BETWEEN 0.0 AND 1.0),
            effective_matches REAL NOT NULL,
            effective_hours REAL NOT NULL,
            peak_mmr INTEGER,
            peak_at INTEGER,
            last_played_at INTEGER,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (model_version, role)
        );

        CREATE TABLE IF NOT EXISTS career_role_mode_summary (
            model_version INTEGER NOT NULL REFERENCES mmr_model_versions(version),
            role TEXT NOT NULL CHECK(role IN ('tank', 'damage', 'support')),
            mode_family TEXT NOT NULL,
            matches_count INTEGER NOT NULL,
            duration_sec INTEGER NOT NULL,
            wins INTEGER NOT NULL,
            losses INTEGER NOT NULL,
            draws INTEGER NOT NULL DEFAULT 0,
            cumulative_delta REAL NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (model_version, role, mode_family)
        );

        CREATE TABLE IF NOT EXISTS career_role_season_summary (
            model_version INTEGER NOT NULL REFERENCES mmr_model_versions(version),
            season INTEGER NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('tank', 'damage', 'support')),
            matches_count INTEGER NOT NULL,
            start_mmr INTEGER NOT NULL,
            end_mmr INTEGER NOT NULL,
            peak_mmr INTEGER NOT NULL,
            net_delta REAL NOT NULL,
            first_match_at INTEGER,
            last_match_at INTEGER,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (model_version, season, role)
        );

        CREATE INDEX IF NOT EXISTS idx_mmr_match_inputs_replay
            ON mmr_match_inputs(begin_ts, match_id);
        CREATE INDEX IF NOT EXISTS idx_career_mmr_events_role_history
            ON career_mmr_events(model_version, role, begin_ts);
        CREATE INDEX IF NOT EXISTS idx_career_role_season_summary_lookup
            ON career_role_season_summary(model_version, season, role);
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
