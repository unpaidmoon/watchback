use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection, Transaction};
use serde_json::Value;

pub const CURRENT_SCHEMA_VERSION: i64 = 5;

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
