use crate::account_context::AccountContextSnapshot;
use crate::account_store::{read_manifest, read_registry, AccountId};
use crate::app_paths::{AccountPaths, SharedAppPaths};
use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MIGRATION_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationJournal {
    pub version: u32,
    pub account_id: AccountId,
    pub phase: MigrationPhase,
    pub started_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MigrationPhase {
    Copying,
    Validating,
    Publishing,
    CleaningLegacy,
    Complete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LegacyMigrationState {
    WaitingForAccount,
    InProgress,
    Complete,
    None,
    RecoveryError,
}

pub fn detect_legacy_state(shared: &SharedAppPaths) -> Result<LegacyMigrationState> {
    if path_entry_exists(&shared.migration_journal())? {
        return match read_journal(shared) {
            Ok(journal) if journal.phase == MigrationPhase::Complete => {
                Ok(LegacyMigrationState::Complete)
            }
            Ok(_) => Ok(LegacyMigrationState::InProgress),
            Err(_) => Ok(LegacyMigrationState::RecoveryError),
        };
    }
    if legacy_sources_exist(shared)? {
        Ok(LegacyMigrationState::WaitingForAccount)
    } else {
        Ok(LegacyMigrationState::None)
    }
}

pub fn adopt_legacy_account(
    shared: &SharedAppPaths,
    account: &AccountContextSnapshot,
) -> Result<()> {
    validate_confirmed_account(shared, account.account_id(), account.paths())?;

    if path_entry_exists(&shared.migration_journal())? {
        let journal = read_journal(shared)?;
        if journal.account_id != *account.account_id() {
            bail!("旧数据迁移已绑定其他已确认账号，无法继续")
        }
        if journal.phase == MigrationPhase::Complete {
            return Ok(());
        }
        recover_interrupted_migration(shared)?;
    }
    if !legacy_sources_exist(shared)? {
        return Ok(());
    }

    validate_legacy_identity(shared, account.paths())?;
    let mut journal = MigrationJournal {
        version: MIGRATION_VERSION,
        account_id: account.account_id().clone(),
        phase: MigrationPhase::Copying,
        started_at: Utc::now().to_rfc3339(),
    };
    write_journal(shared, &journal)?;

    let staging = staging_root(shared, account.account_id());
    remove_staging_if_present(&staging)?;
    fs::create_dir_all(&staging).map_err(|_| anyhow!("无法准备旧数据迁移目录"))?;
    copy_legacy_to_staging(shared, &staging)?;

    set_phase(shared, &mut journal, MigrationPhase::Validating)?;
    validate_staging_database(&staging)?;

    set_phase(shared, &mut journal, MigrationPhase::Publishing)?;
    publish_staging(&staging, account.paths())?;
    validate_published_account(shared, account.account_id(), account.paths())?;

    set_phase(shared, &mut journal, MigrationPhase::CleaningLegacy)?;
    clean_legacy_sources(shared)?;
    remove_staging_if_present(&staging)?;
    set_phase(shared, &mut journal, MigrationPhase::Complete)
}

pub fn recover_interrupted_migration(shared: &SharedAppPaths) -> Result<()> {
    if !path_entry_exists(&shared.migration_journal())? {
        return Ok(());
    }
    let mut journal = read_journal(shared)?;
    let staging = staging_root(shared, &journal.account_id);

    match journal.phase {
        MigrationPhase::Copying | MigrationPhase::Validating => {
            remove_staging_if_present(&staging)?;
            fs::remove_file(shared.migration_journal())
                .map_err(|_| anyhow!("无法回滚未完成的旧数据迁移"))?;
            Ok(())
        }
        MigrationPhase::Publishing => {
            let paths = shared.account(&journal.account_id);
            validate_confirmed_account_paths(shared, &journal.account_id, &paths)?;
            validate_staging_database(&staging)?;
            publish_staging(&staging, &paths)?;
            validate_published_account(shared, &journal.account_id, &paths)?;
            set_phase(shared, &mut journal, MigrationPhase::CleaningLegacy)?;
            clean_legacy_sources(shared)?;
            remove_staging_if_present(&staging)?;
            set_phase(shared, &mut journal, MigrationPhase::Complete)
        }
        MigrationPhase::CleaningLegacy => {
            let paths = shared.account(&journal.account_id);
            validate_confirmed_account_paths(shared, &journal.account_id, &paths)?;
            validate_published_account(shared, &journal.account_id, &paths)?;
            clean_legacy_sources(shared)?;
            remove_staging_if_present(&staging)?;
            set_phase(shared, &mut journal, MigrationPhase::Complete)
        }
        MigrationPhase::Complete => Ok(()),
    }
}

fn validate_confirmed_account(
    shared: &SharedAppPaths,
    account_id: &AccountId,
    paths: &AccountPaths,
) -> Result<()> {
    if shared.account(account_id).root() != paths.root() {
        bail!("已确认账号目录不匹配")
    }
    validate_confirmed_account_paths(shared, account_id, paths)
}

fn validate_confirmed_account_paths(
    shared: &SharedAppPaths,
    account_id: &AccountId,
    paths: &AccountPaths,
) -> Result<()> {
    require_safe_directory(paths.root())?;
    require_safe_regular_file(&paths.manifest())?;
    let manifest = read_manifest(&paths.manifest(), account_id)
        .map_err(|_| anyhow!("已确认账号信息不完整，无法迁移旧数据"))?;
    let role_id = read_complete_credential_role(&paths.credentials())?;
    if role_id != manifest.role_id {
        bail!("已确认账号身份不一致，无法迁移旧数据")
    }
    let registry =
        read_registry(&shared.registry()).map_err(|_| anyhow!("账号索引不完整，无法迁移旧数据"))?;
    let entry = registry
        .accounts
        .iter()
        .find(|entry| entry.account_id == *account_id)
        .ok_or_else(|| anyhow!("账号索引不包含已确认账号"))?;
    if entry.battle_tag != manifest.battle_tag
        || registry.active_account_id.as_ref() != Some(account_id)
    {
        bail!("账号索引与已确认账号不一致")
    }
    Ok(())
}

fn validate_legacy_identity(shared: &SharedAppPaths, account: &AccountPaths) -> Result<()> {
    if !path_entry_exists(&shared.legacy_credentials())? {
        return Ok(());
    }
    let manifest = read_manifest(
        &account.manifest(),
        &AccountId::parse(
            account
                .root()
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| anyhow!("已确认账号目录无效"))?,
        )?,
    )
    .map_err(|_| anyhow!("已确认账号信息不完整，无法迁移旧数据"))?;
    let legacy_role_id = read_complete_credential_role(&shared.legacy_credentials())?;
    if legacy_role_id != manifest.role_id {
        bail!("旧数据账号身份与已确认账号不一致，迁移已取消")
    }
    Ok(())
}

fn read_complete_credential_role(path: &Path) -> Result<i64> {
    require_safe_regular_file(path)?;
    let bytes = fs::read(path).map_err(|_| anyhow!("账号凭据不完整，请重新登录后再迁移"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow!("账号凭据不完整，请重新登录后再迁移"))?;
    let role_id = value
        .get("role_id")
        .or_else(|| value.get("roleId"))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        });
    let token_ready = value
        .get("token")
        .and_then(|value| value.as_str())
        .is_some_and(|token| !token.trim().is_empty());
    match (role_id, token_ready) {
        (Some(role_id), true) => Ok(role_id),
        _ => bail!("账号凭据不完整，请重新登录后再迁移"),
    }
}

fn copy_legacy_to_staging(shared: &SharedAppPaths, staging: &Path) -> Result<()> {
    let mappings = [
        (shared.legacy_database(), staging.join("owstats.sqlite3")),
        (
            shared.legacy_player_target(),
            staging.join("player_target.json"),
        ),
        (
            shared.legacy_player_profile(),
            staging.join("player_profile.json"),
        ),
        (shared.legacy_seasons(), staging.join("seasons")),
        (shared.legacy_diagnostics(), staging.join("diagnostics")),
        (shared.legacy_backups(), staging.join("backups")),
    ];
    for (source, destination) in mappings {
        if path_entry_exists(&source)? {
            copy_path_checked(&source, &destination)?;
        }
    }
    Ok(())
}

fn validate_staging_database(staging: &Path) -> Result<()> {
    let path = staging.join("owstats.sqlite3");
    match safe_destination_metadata(&path)? {
        None => return Ok(()),
        Some(metadata) if metadata.is_file() => {}
        Some(_) => bail!("旧数据库文件类型无效，源数据保持不变"),
    }
    crate::OwDatabase::new(&path)
        .initialize()
        .map_err(|_| anyhow!("旧数据库校验失败，源数据保持不变"))?;
    let connection =
        Connection::open(&path).map_err(|_| anyhow!("旧数据库校验失败，源数据保持不变"))?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| anyhow!("旧数据库校验失败，源数据保持不变"))?;
    if integrity != "ok" {
        bail!("旧数据库校验失败，源数据保持不变")
    }
    connection
        .query_row("SELECT COUNT(*) FROM matches", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|_| anyhow!("旧数据库校验失败，源数据保持不变"))?;
    Ok(())
}

fn publish_staging(staging: &Path, account: &AccountPaths) -> Result<()> {
    if !staging.is_dir() {
        bail!("旧数据迁移暂存目录缺失，源数据保持不变")
    }
    let mappings = [
        (staging.join("owstats.sqlite3"), account.database()),
        (staging.join("player_target.json"), account.player_target()),
        (
            staging.join("player_profile.json"),
            account.player_profile(),
        ),
        (staging.join("seasons"), account.seasons()),
        (staging.join("diagnostics"), account.diagnostics()),
        (staging.join("backups"), account.backups()),
    ];
    for (source, destination) in mappings {
        if path_entry_exists(&source)? {
            publish_path_no_overwrite(&source, &destination)?;
        }
    }
    Ok(())
}

fn validate_published_account(
    shared: &SharedAppPaths,
    account_id: &AccountId,
    paths: &AccountPaths,
) -> Result<()> {
    validate_confirmed_account_paths(shared, account_id, paths)?;
    if let Some(metadata) = safe_destination_metadata(&paths.database())? {
        if !metadata.is_file() {
            bail!("迁移后的账号数据库类型无效")
        }
        let connection = Connection::open(paths.database())
            .map_err(|_| anyhow!("迁移后的账号数据库校验失败"))?;
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|_| anyhow!("迁移后的账号数据库校验失败"))?;
        if integrity != "ok" {
            bail!("迁移后的账号数据库校验失败")
        }
        connection
            .query_row("SELECT COUNT(*) FROM matches", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|_| anyhow!("迁移后的账号数据库校验失败"))?;
    }
    Ok(())
}

fn clean_legacy_sources(shared: &SharedAppPaths) -> Result<()> {
    for path in legacy_sources(shared) {
        if !path_entry_exists(&path)? {
            continue;
        }
        reject_links_recursively(&path)?;
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|_| anyhow!("无法清理已迁移的旧数据"))?;
        } else {
            fs::remove_file(&path).map_err(|_| anyhow!("无法清理已迁移的旧数据"))?;
        }
    }
    Ok(())
}

fn legacy_sources(shared: &SharedAppPaths) -> Vec<PathBuf> {
    vec![
        shared.legacy_database(),
        shared.legacy_credentials(),
        shared.legacy_player_target(),
        shared.legacy_player_profile(),
        shared.legacy_seasons(),
        shared.legacy_diagnostics(),
        shared.legacy_backups(),
    ]
}

fn legacy_sources_exist(shared: &SharedAppPaths) -> Result<bool> {
    for path in legacy_sources(shared) {
        if path_entry_exists(&path)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn path_entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => bail!("无法检查旧数据迁移路径"),
    }
}

fn staging_root(shared: &SharedAppPaths, account_id: &AccountId) -> PathBuf {
    shared
        .accounts_root()
        .join(format!(".migration-{}", account_id.as_str()))
}

fn set_phase(
    shared: &SharedAppPaths,
    journal: &mut MigrationJournal,
    phase: MigrationPhase,
) -> Result<()> {
    journal.phase = phase;
    write_journal(shared, journal)
}

fn read_journal(shared: &SharedAppPaths) -> Result<MigrationJournal> {
    require_safe_regular_file(&shared.migration_journal())?;
    let bytes =
        fs::read(shared.migration_journal()).map_err(|_| anyhow!("无法读取旧数据迁移状态"))?;
    let journal: MigrationJournal = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow!("旧数据迁移状态损坏，请保留数据并联系支持"))?;
    if journal.version != MIGRATION_VERSION {
        bail!("旧数据迁移状态版本不受支持，请保留数据并联系支持")
    }
    Ok(journal)
}

fn write_journal(shared: &SharedAppPaths, journal: &MigrationJournal) -> Result<()> {
    if journal.version != MIGRATION_VERSION {
        bail!("旧数据迁移状态版本不受支持")
    }
    let bytes =
        serde_json::to_vec_pretty(journal).map_err(|_| anyhow!("无法保存旧数据迁移状态"))?;
    write_atomic(&shared.migration_journal(), &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| anyhow!("迁移状态路径无效"))?;
    if let Some(metadata) = safe_destination_metadata(path)? {
        if !metadata.is_file() {
            bail!("迁移状态文件类型无效")
        }
    }
    fs::create_dir_all(parent).map_err(|_| anyhow!("无法创建迁移状态目录"))?;
    let temporary = parent.join(format!(".migration-journal-{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file =
            fs::File::create(&temporary).map_err(|_| anyhow!("无法创建迁移状态临时文件"))?;
        file.write_all(bytes)
            .map_err(|_| anyhow!("无法写入迁移状态"))?;
        file.flush().map_err(|_| anyhow!("无法刷新迁移状态"))?;
        file.sync_all().map_err(|_| anyhow!("无法同步迁移状态"))?;
        fs::rename(&temporary, path).map_err(|_| anyhow!("无法发布迁移状态"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn copy_path_checked(source: &Path, destination: &Path) -> Result<()> {
    reject_link(source)?;
    let metadata = fs::symlink_metadata(source).map_err(|_| anyhow!("无法读取旧数据"))?;
    if metadata.is_file() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| anyhow!("无法创建迁移暂存目录"))?;
        }
        fs::copy(source, destination).map_err(|_| anyhow!("无法复制旧数据"))?;
        return Ok(());
    }
    if !metadata.is_dir() {
        bail!("旧数据包含不支持的文件类型")
    }
    fs::create_dir_all(destination).map_err(|_| anyhow!("无法创建迁移暂存目录"))?;
    for entry in fs::read_dir(source).map_err(|_| anyhow!("无法读取旧数据目录"))? {
        let entry = entry.map_err(|_| anyhow!("无法读取旧数据目录"))?;
        copy_path_checked(&entry.path(), &destination.join(entry.file_name()))?;
    }
    Ok(())
}

fn publish_path_no_overwrite(source: &Path, destination: &Path) -> Result<()> {
    let source_metadata =
        fs::symlink_metadata(source).map_err(|_| anyhow!("无法读取迁移暂存数据"))?;
    reject_metadata(&source_metadata)?;
    let destination_metadata = safe_destination_metadata(destination)?;
    if source_metadata.is_dir() {
        if destination_metadata
            .as_ref()
            .is_some_and(|metadata| !metadata.is_dir())
        {
            bail!("账号目录已有冲突数据，迁移已停止")
        }
        fs::create_dir_all(destination).map_err(|_| anyhow!("无法发布迁移数据"))?;
        for entry in fs::read_dir(source).map_err(|_| anyhow!("无法读取迁移暂存数据"))? {
            let entry = entry.map_err(|_| anyhow!("无法读取迁移暂存数据"))?;
            publish_path_no_overwrite(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if !source_metadata.is_file() {
        bail!("迁移暂存数据包含不支持的文件类型")
    }
    if let Some(metadata) = destination_metadata {
        if metadata.is_file() {
            let source_bytes = fs::read(source).map_err(|_| anyhow!("无法读取迁移暂存数据"))?;
            let destination_bytes =
                fs::read(destination).map_err(|_| anyhow!("无法读取账号现有数据"))?;
            if source_bytes == destination_bytes {
                return Ok(());
            }
        }
        bail!("账号目录已有不同数据，迁移不会覆盖")
    }
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("迁移目标路径无效"))?;
    fs::create_dir_all(parent).map_err(|_| anyhow!("无法创建账号数据目录"))?;
    let temporary = parent.join(format!(".migration-publish-{}.tmp", Uuid::new_v4()));
    let publish_result = (|| -> Result<()> {
        fs::copy(source, &temporary).map_err(|_| anyhow!("无法写入账号迁移数据"))?;
        fs::rename(&temporary, destination).map_err(|_| anyhow!("无法发布账号迁移数据"))?;
        Ok(())
    })();
    if publish_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    publish_result
}

fn remove_staging_if_present(staging: &Path) -> Result<()> {
    if !path_entry_exists(staging)? {
        return Ok(());
    }
    reject_links_recursively(staging)?;
    fs::remove_dir_all(staging).map_err(|_| anyhow!("无法清理旧数据迁移暂存目录"))
}

fn reject_links_recursively(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| anyhow!("无法检查迁移文件类型"))?;
    reject_metadata(&metadata)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| anyhow!("无法检查迁移目录"))? {
            let entry = entry.map_err(|_| anyhow!("无法检查迁移目录"))?;
            reject_links_recursively(&entry.path())?;
        }
    }
    Ok(())
}

fn reject_link(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| anyhow!("无法检查迁移文件类型"))?;
    reject_metadata(&metadata)
}

fn reject_metadata(metadata: &fs::Metadata) -> Result<()> {
    if metadata.file_type().is_symlink() {
        bail!("旧数据包含链接，迁移已停止")
    }
    if !metadata.is_file() && !metadata.is_dir() {
        bail!("迁移路径包含不支持的文件类型")
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x0400 != 0 {
            bail!("旧数据包含重解析点，迁移已停止")
        }
    }
    Ok(())
}

fn safe_destination_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    let mut ancestor = path.parent();
    while let Some(parent) = ancestor {
        match fs::symlink_metadata(parent) {
            Ok(metadata) => {
                reject_metadata(&metadata)?;
                if !metadata.is_dir() {
                    bail!("迁移目标父级不是目录")
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => bail!("无法检查迁移目标路径"),
        }
        ancestor = parent.parent();
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            reject_metadata(&metadata)?;
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => bail!("无法检查迁移目标路径"),
    }
}

fn require_safe_directory(path: &Path) -> Result<()> {
    match safe_destination_metadata(path)? {
        Some(metadata) if metadata.is_dir() => Ok(()),
        _ => bail!("已确认账号目录类型无效"),
    }
}

fn require_safe_regular_file(path: &Path) -> Result<()> {
    match safe_destination_metadata(path)? {
        Some(metadata) if metadata.is_file() => Ok(()),
        _ => bail!("账号文件不完整或类型无效"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_journal_is_camel_case_versioned_and_atomic() {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let account_id = AccountId::parse("24512330-a396-461e-98dd-14685f03f5ed").unwrap();
        let mut journal = MigrationJournal {
            version: 1,
            account_id,
            phase: MigrationPhase::Copying,
            started_at: "2026-01-01T00:00:00Z".to_string(),
        };

        write_journal(&shared, &journal).unwrap();
        journal.phase = MigrationPhase::Validating;
        write_journal(&shared, &journal).unwrap();

        assert_eq!(read_journal(&shared).unwrap(), journal);
        let encoded = fs::read_to_string(shared.migration_journal()).unwrap();
        assert!(encoded.contains("\"accountId\""));
        assert!(encoded.contains("\"startedAt\""));
        assert!(encoded.contains("\"validating\""));
        assert!(!encoded.contains("roleId"));
        assert!(!encoded.contains("token"));
        assert!(fs::read_dir(shared.operations_root())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")));
    }

    #[test]
    fn migration_journal_rejects_unsupported_version_and_invalid_account_id() {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        fs::create_dir_all(shared.operations_root()).unwrap();
        fs::write(
            shared.migration_journal(),
            r#"{"version":2,"accountId":"24512330-a396-461e-98dd-14685f03f5ed","phase":"copying","startedAt":"now"}"#,
        )
        .unwrap();
        assert!(read_journal(&shared).is_err());

        fs::write(
            shared.migration_journal(),
            r#"{"version":1,"accountId":"../escape","phase":"copying","startedAt":"now"}"#,
        )
        .unwrap();
        assert!(read_journal(&shared).is_err());
    }

    #[test]
    fn publish_file_conflict_matrix_never_overwrites_existing_data() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::write(&source, b"verified-data").unwrap();
        let absent = temp.path().join("absent");

        publish_path_no_overwrite(&source, &absent).unwrap();
        publish_path_no_overwrite(&source, &absent).unwrap();
        assert_eq!(fs::read(&absent).unwrap(), b"verified-data");

        for (name, existing) in [("empty", &b""[..]), ("different", &b"different"[..])] {
            let destination = temp.path().join(name);
            fs::write(&destination, existing).unwrap();
            assert!(publish_path_no_overwrite(&source, &destination).is_err());
            assert_eq!(fs::read(destination).unwrap(), existing);
        }

        let directory_source = temp.path().join("directory-source");
        fs::create_dir(&directory_source).unwrap();
        let file_destination = temp.path().join("file-destination");
        fs::write(&file_destination, b"keep").unwrap();
        assert!(publish_path_no_overwrite(&directory_source, &file_destination).is_err());

        let file_source = temp.path().join("file-source");
        fs::write(&file_source, b"data").unwrap();
        let directory_destination = temp.path().join("directory-destination");
        fs::create_dir(&directory_destination).unwrap();
        assert!(publish_path_no_overwrite(&file_source, &directory_destination).is_err());
    }

    #[test]
    fn publish_rejects_linked_destination_directory_without_writing_through_it() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("payload"), b"data").unwrap();
        let external = temp.path().join("external");
        fs::create_dir(&external).unwrap();
        let linked = temp.path().join("linked");
        if !create_test_directory_symlink(&external, &linked) {
            return;
        }

        assert!(publish_path_no_overwrite(&source, &linked).is_err());
        assert!(!external.join("payload").exists());

        let file_source = temp.path().join("file-source");
        fs::write(&file_source, b"data").unwrap();
        assert!(publish_path_no_overwrite(&file_source, &linked.join("nested")).is_err());
        assert!(!external.join("nested").exists());
    }

    #[test]
    fn copy_rejects_link_source_as_unknown_special_type() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("real");
        fs::write(&real, b"data").unwrap();
        let linked = temp.path().join("linked");
        if !create_test_file_symlink(&real, &linked) {
            return;
        }
        assert!(copy_path_checked(&linked, &temp.path().join("copy")).is_err());
    }

    #[cfg(windows)]
    fn create_test_directory_symlink(source: &Path, target: &Path) -> bool {
        std::os::windows::fs::symlink_dir(source, target).is_ok()
    }

    #[cfg(unix)]
    fn create_test_directory_symlink(source: &Path, target: &Path) -> bool {
        std::os::unix::fs::symlink(source, target).is_ok()
    }

    #[cfg(windows)]
    fn create_test_file_symlink(source: &Path, target: &Path) -> bool {
        std::os::windows::fs::symlink_file(source, target).is_ok()
    }

    #[cfg(unix)]
    fn create_test_file_symlink(source: &Path, target: &Path) -> bool {
        std::os::unix::fs::symlink(source, target).is_ok()
    }
}
