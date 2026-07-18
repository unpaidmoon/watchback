use anyhow::Result;
use owstats_desktop_lib::{
    account_store::{
        write_manifest_atomic, write_registry_atomic, AccountId, AccountManifest, AccountRegistry,
        AccountRegistryEntry,
    },
    app_paths::SharedAppPaths,
    career_mmr::{
        training::build_multi_account_projection,
        training_sources::{discover_training_sources, open_query_only, validate_training_schema},
    },
    OwDatabase,
};
use rusqlite::{params, Connection};
use std::fs;
use tempfile::TempDir;

fn synthetic_account(index: u8) -> Result<AccountId> {
    AccountId::parse(&format!("00000000-0000-4000-8000-{index:012}"))
}

fn registry_entry(account_id: AccountId, index: u8) -> AccountRegistryEntry {
    AccountRegistryEntry {
        account_id,
        battle_tag: format!("合成账号#{index:04}"),
        created_at: "2026-01-01T00:00:00Z".into(),
        last_used_at: "2026-01-01T00:00:00Z".into(),
        credential_state: "ready".into(),
    }
}

fn manifest(account_id: AccountId, index: u8) -> AccountManifest {
    AccountManifest {
        version: 1,
        account_id,
        role_id: i64::from(index),
        battle_tag: format!("合成账号#{index:04}"),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn registry_fixture() -> Result<(TempDir, SharedAppPaths)> {
    let dir = tempfile::tempdir()?;
    let paths = SharedAppPaths::from_app_data_dir(dir.path());
    let mut entries = Vec::new();
    for index in [2, 1] {
        let account_id = synthetic_account(index)?;
        let account_paths = paths.account(&account_id);
        fs::create_dir_all(account_paths.root())?;
        write_manifest_atomic(
            &account_paths.manifest(),
            &manifest(account_id.clone(), index),
        )?;
        OwDatabase::new(account_paths.database()).initialize()?;
        let conn = OwDatabase::new(account_paths.database()).connect()?;
        insert_source_rows(&conn, index)?;
        entries.push(registry_entry(account_id, index));
    }
    write_registry_atomic(
        &paths.registry(),
        &AccountRegistry {
            version: 1,
            active_account_id: None,
            accounts: entries,
        },
    )?;
    let backup_dir = dir.path().join("backups");
    fs::create_dir_all(&backup_dir)?;
    fs::write(
        backup_dir.join("unregistered.sqlite3"),
        b"must never be inspected",
    )?;
    Ok((dir, paths))
}

fn insert_source_rows(conn: &Connection, account_index: u8) -> Result<()> {
    conn.execute(
        "INSERT INTO hero_dict(guid,name,role,updated_at) VALUES('synthetic-hero','合成英雄','damage','now') ON CONFLICT(guid) DO NOTHING",
        [],
    )?;
    let own_begin = 1_000 + i64::from(account_index) * 1_000;
    insert_match(
        conn,
        &format!("source-{account_index}-own"),
        own_begin,
        &format!("synthetic-map-{account_index}"),
        "win",
        1,
        0,
    )?;
    let (result, team_score, opponent_score) = if account_index == 1 {
        ("win", 3, 2)
    } else {
        ("loss", 2, 3)
    };
    insert_match(
        conn,
        &format!("source-{account_index}-shared"),
        9_000,
        "synthetic-shared-map",
        result,
        team_score,
        opponent_score,
    )?;
    if account_index == 1 {
        conn.execute(
            "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES('source-1-shared','normal',?1,'now')",
            [r#"{"payload":{"data":{"heroList":[{"heroId":"synthetic-hero","userTimeSec":600,"useTimeRate":1.0,"statMap":{}},{"heroId":"synthetic-hero","userTimeSec":600,"useTimeRate":1.0,"statMap":{}}]}}}"#],
        )?;
    }
    Ok(())
}

fn insert_match(
    conn: &Connection,
    source_id: &str,
    begin_ts: i64,
    map_guid: &str,
    result: &str,
    team_score: i64,
    opponent_score: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,24,'preset','SportPreset','normal',?2,?3,'synthetic-hero',?4,?5,?6,4,3,2,1000,0,0,0,600,'{\"roleType\":\"damage\",\"heroPlayTimeList\":[{\"heroGuid\":\"synthetic-hero\",\"durationSec\":600}]}','now')",
        params![source_id, result, map_guid, begin_ts, team_score, opponent_score],
    )?;
    conn.execute(
        "INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,kill,assist,death,hero_damage,raw_json,updated_at) VALUES(?1,24,'teammate',?2,'合成当前玩家','synthetic-hero',1,4,3,2,1000,'{}','now')",
        params![source_id, format!("synthetic-current-{source_id}")],
    )?;
    conn.execute(
        "INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,kill,assist,death,hero_damage,raw_json,updated_at) VALUES(?1,24,'teammate',?2,'合成高统计队友','synthetic-hero',0,99,99,0,99999,'{}','now')",
        params![source_id, format!("synthetic-peer-{source_id}")],
    )?;
    Ok(())
}

#[test]
fn discovers_only_registry_accounts_with_stable_anonymous_groups() -> Result<()> {
    let (_dir, paths) = registry_fixture()?;

    let sources = discover_training_sources(&paths)?;

    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].account_group, "dataset-001");
    assert_eq!(sources[1].account_group, "dataset-002");
    Ok(())
}

#[test]
fn opens_sources_query_only_and_validates_only_training_schema() -> Result<()> {
    let (_dir, paths) = registry_fixture()?;
    let sources = discover_training_sources(&paths)?;

    let conn = open_query_only(&sources[0])?;
    validate_training_schema(&conn)?;
    assert_eq!(conn.total_changes(), 0);
    assert_eq!(
        conn.pragma_query_value(None, "query_only", |row| row.get::<_, i64>(0))?,
        1
    );
    assert!(conn.execute("DELETE FROM matches", []).is_err());
    assert_eq!(conn.total_changes(), 0);
    Ok(())
}

#[test]
fn aggregates_accounts_without_splitting_shared_matches_or_emitting_peer_rows() -> Result<()> {
    let (dir, paths) = registry_fixture()?;
    let sources = discover_training_sources(&paths)?;
    let source_state = [1, 2]
        .into_iter()
        .map(|index| -> Result<_> {
            let database = paths.account(&synthetic_account(index)?).database();
            let metadata = fs::metadata(&database)?;
            let count = Connection::open(&database)?.query_row(
                "SELECT COUNT(*) FROM matches",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            Ok((database, metadata.len(), metadata.modified()?, count))
        })
        .collect::<Result<Vec<_>>>()?;
    let backup = dir.path().join("backups/unregistered.sqlite3");
    let backup_before = fs::read(&backup)?;

    let projection = build_multi_account_projection(&sources)?;

    assert_eq!(projection.audit.source_accounts, 2);
    assert_eq!(projection.audit.projected_rows, 4);
    assert_eq!(projection.rows.len(), 4);
    assert_eq!(
        projection
            .rows
            .iter()
            .find(|row| row.begin_ts == 2_000)
            .map(|row| row.account_group.as_str()),
        Some("dataset-001")
    );
    assert_eq!(
        projection
            .rows
            .iter()
            .find(|row| row.begin_ts == 3_000)
            .map(|row| row.account_group.as_str()),
        Some("dataset-002")
    );
    let shared = projection
        .rows
        .iter()
        .filter(|row| row.begin_ts == 9_000)
        .collect::<Vec<_>>();
    assert_eq!(shared.len(), 2);
    assert_ne!(shared[0].account_group, shared[1].account_group);
    assert_eq!(shared[0].match_group, shared[1].match_group);
    assert_ne!(shared[0].source_match_group, shared[1].source_match_group);
    assert!(projection.rows.iter().all(|row| row.role == "damage"));
    let serialized_audit = serde_json::to_string(projection.public_audit())?;
    for private_value in [
        paths.root().to_string_lossy().to_string(),
        synthetic_account(1)?.as_str().to_owned(),
        "合成账号#0001".to_owned(),
        "source-1-shared".to_owned(),
        "raw_json".to_owned(),
    ] {
        assert!(!serialized_audit.contains(&private_value));
    }
    for (database, len, modified, count) in source_state {
        let metadata = fs::metadata(&database)?;
        assert_eq!(metadata.len(), len);
        assert_eq!(metadata.modified()?, modified);
        assert_eq!(
            Connection::open(&database)?
                .query_row("SELECT COUNT(*) FROM matches", [], |row| row
                    .get::<_, i64>(0),)?,
            count
        );
    }
    assert_eq!(fs::read(backup)?, backup_before);
    Ok(())
}

#[test]
fn rejects_one_incompatible_source_as_a_privacy_safe_aggregate_error() -> Result<()> {
    let (_dir, paths) = registry_fixture()?;
    let account_id = synthetic_account(2)?;
    let account_paths = paths.account(&account_id);
    Connection::open(account_paths.database())?.execute("DROP TABLE match_details", [])?;
    let sources = discover_training_sources(&paths)?;

    let error = build_multi_account_projection(&sources)
        .expect_err("一个源失败时必须拒绝整个聚合")
        .to_string();

    assert_eq!(error, "无法聚合多账号训练数据");
    for private_value in [
        account_paths.database().to_string_lossy().to_string(),
        account_id.as_str().to_owned(),
        "合成账号#0002".to_owned(),
        "source-2-shared".to_owned(),
        "raw_json".to_owned(),
    ] {
        assert!(!error.contains(&private_value));
    }
    Ok(())
}

#[test]
fn rejects_registry_manifest_identity_mismatch_without_exposing_identity() -> Result<()> {
    let (_dir, paths) = registry_fixture()?;
    let account_id = synthetic_account(1)?;
    let account_paths = paths.account(&account_id);
    write_manifest_atomic(
        &account_paths.manifest(),
        &AccountManifest {
            battle_tag: "不一致的合成身份".into(),
            ..manifest(account_id.clone(), 1)
        },
    )?;

    let error = match discover_training_sources(&paths) {
        Ok(_) => panic!("manifest 与 registry 不一致时必须拒绝"),
        Err(error) => error.to_string(),
    };

    assert_eq!(error, "无法发现多账号训练数据源");
    assert!(!error.contains(account_id.as_str()));
    assert!(!error.contains("不一致的合成身份"));
    Ok(())
}

#[test]
fn rejects_a_registered_database_replaced_by_a_filesystem_link() -> Result<()> {
    let (dir, paths) = registry_fixture()?;
    let database = paths.account(&synthetic_account(1)?).database();
    fs::remove_file(&database)?;
    create_file_symlink(&dir.path().join("backups/unregistered.sqlite3"), &database)?;

    let error = match discover_training_sources(&paths) {
        Ok(_) => panic!("训练数据库替换为链接时必须拒绝"),
        Err(error) => error.to_string(),
    };

    assert_eq!(error, "无法发现多账号训练数据源");
    Ok(())
}

#[cfg(windows)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}
