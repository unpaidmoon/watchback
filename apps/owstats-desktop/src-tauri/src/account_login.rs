use crate::account_context::AccountContextManager;
use crate::account_migration::LegacyMigrationState;
use crate::account_store::{
    read_manifest, write_manifest_atomic, AccountId, AccountManifest, AccountRegistryEntry,
};
use crate::credential_store::{save_credentials_atomic, DashenCredentials};
use crate::models::AccountLoginResult;
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const SAFE_LOGIN_ERROR: &str = "账号登录处理失败，请重试。";

#[derive(Debug, Clone)]
pub struct LoginCapture {
    pub credentials: DashenCredentials,
    pub battle_tag: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingOperation<'a> {
    version: u32,
    kind: &'a str,
    account_id: Option<&'a AccountId>,
}

struct PendingLogin {
    root: PathBuf,
    pending_root: PathBuf,
}

impl PendingLogin {
    fn create(manager: &AccountContextManager) -> Result<Self> {
        let pending_root = manager.shared_paths().pending_logins();
        ensure_safe_pending_parent(&pending_root)?;
        fs::create_dir_all(&pending_root)?;
        ensure_not_link_or_reparse(&pending_root)?;
        let operation_id = AccountId::new();
        let root = pending_root.join(operation_id.as_str());
        fs::create_dir(&root)?;
        ensure_not_link_or_reparse(&root)?;
        Ok(Self { root, pending_root })
    }

    fn account_root(&self, account_id: &AccountId) -> PathBuf {
        self.root.join("account").join(account_id.as_str())
    }

    fn write_metadata(&self, kind: &str, account_id: Option<&AccountId>) -> Result<()> {
        write_json_atomic(
            &self.root.join("operation.json"),
            &PendingOperation {
                version: 1,
                kind,
                account_id,
            },
        )
    }

    fn cleanup(&self) {
        let _ = remove_pending_operation(&self.root, &self.pending_root);
    }
}

impl Drop for PendingLogin {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub fn publish_add_account(
    manager: &AccountContextManager,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    let result = publish_add_account_inner(manager, capture);
    result.map_err(|_| anyhow!(SAFE_LOGIN_ERROR))
}

fn publish_add_account_inner(
    manager: &AccountContextManager,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    validate_capture(&capture)?;
    let _publication = manager.lock_login_publication()?;
    ensure_safe_accounts_root(manager)?;
    let pending = PendingLogin::create(manager)?;
    let registry = manager.registry_snapshot()?;
    let matching = find_account_by_role(manager, &registry.accounts, capture.credentials.role_id)?;
    if let Some(account_id) = matching {
        pending.write_metadata("updateExisting", Some(&account_id))?;
        update_existing(manager, &pending, &registry, &account_id, capture, true)
    } else {
        let account_id = AccountId::new();
        pending.write_metadata("addNew", Some(&account_id))?;
        add_new(manager, &pending, registry, account_id, capture)
    }
}

pub fn publish_relogin_account(
    manager: &AccountContextManager,
    expected_account_id: &AccountId,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    let result = publish_relogin_account_inner(manager, expected_account_id, capture);
    result.map_err(|_| anyhow!(SAFE_LOGIN_ERROR))
}

fn publish_relogin_account_inner(
    manager: &AccountContextManager,
    expected_account_id: &AccountId,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    validate_capture(&capture)?;
    let _publication = manager.lock_login_publication()?;
    ensure_safe_accounts_root(manager)?;
    let pending = PendingLogin::create(manager)?;
    pending.write_metadata("relogin", Some(expected_account_id))?;
    let registry = manager.registry_snapshot()?;
    if !registry
        .accounts
        .iter()
        .any(|entry| &entry.account_id == expected_account_id)
    {
        bail!("selected account is absent")
    }
    let expected_paths = manager.shared_paths().account(expected_account_id);
    ensure_not_link_or_reparse(expected_paths.root())?;
    let expected_manifest = read_manifest(&expected_paths.manifest(), expected_account_id)?;
    let expected_entry = registry
        .accounts
        .iter()
        .find(|entry| &entry.account_id == expected_account_id)
        .context("selected account summary is absent")?;
    validate_stored_manifest(&expected_manifest, expected_entry)?;
    stage_capture(
        &pending.account_root(expected_account_id),
        expected_account_id,
        &capture,
        &expected_manifest.created_at,
    )?;
    if expected_manifest.role_id == capture.credentials.role_id {
        return update_existing(
            manager,
            &pending,
            &registry,
            expected_account_id,
            capture,
            false,
        );
    }

    if let Some(other_id) =
        find_account_by_role(manager, &registry.accounts, capture.credentials.role_id)?
    {
        let entry = registry
            .accounts
            .iter()
            .find(|entry| entry.account_id == other_id)
            .context("matched account summary is absent")?;
        return Ok(AccountLoginResult {
            kind: "differentExistingAccount".to_string(),
            account_id: Some(other_id.as_str().to_string()),
            battle_tag: Some(entry.battle_tag.clone()),
            message: "检测到另一个已保存账号，可切换后重新登录。".to_string(),
            should_offer_switch: true,
        });
    }

    Ok(AccountLoginResult {
        kind: "differentNewAccount".to_string(),
        account_id: None,
        battle_tag: Some(capture.battle_tag),
        message: "本次登录属于另一个新账号，未覆盖当前账号。".to_string(),
        should_offer_switch: false,
    })
}

fn add_new(
    manager: &AccountContextManager,
    pending: &PendingLogin,
    mut registry: crate::account_store::AccountRegistry,
    account_id: AccountId,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    let now = Utc::now().to_rfc3339();
    let pending_account_root = pending.account_root(&account_id);
    stage_capture(&pending_account_root, &account_id, &capture, &now)?;
    let staged_manifest = pending_account_root.join("account.json");
    let staged_credentials = pending_account_root.join("credentials.json");
    validate_staged_account(&staged_manifest, &staged_credentials, &account_id)?;

    let final_paths = manager.shared_paths().account(&account_id);
    ensure_safe_accounts_root(manager)?;
    if path_entry_exists(final_paths.root())? {
        bail!("account publication target already exists")
    }
    fs::rename(&pending_account_root, final_paths.root())?;

    registry.accounts.push(AccountRegistryEntry {
        account_id: account_id.clone(),
        battle_tag: capture.battle_tag.clone(),
        created_at: now.clone(),
        last_used_at: now,
        credential_state: "ready".to_string(),
    });
    if registry.active_account_id.is_none() {
        registry.active_account_id = Some(account_id.clone());
    }
    if manager.replace_registry(registry).is_err() {
        remove_published_account(final_paths.root())?;
        bail!("registry publication failed")
    }

    let migration_warning = adopt_legacy_if_waiting(manager, &account_id).is_err();
    Ok(AccountLoginResult {
        kind: "newAccountAdded".to_string(),
        account_id: Some(account_id.as_str().to_string()),
        battle_tag: Some(capture.battle_tag),
        message: if migration_warning {
            "账号已添加，旧数据暂待恢复。".to_string()
        } else {
            "账号已添加。".to_string()
        },
        should_offer_switch: false,
    })
}

fn update_existing(
    manager: &AccountContextManager,
    pending: &PendingLogin,
    registry: &crate::account_store::AccountRegistry,
    account_id: &AccountId,
    capture: LoginCapture,
    from_add: bool,
) -> Result<AccountLoginResult> {
    let paths = manager.shared_paths().account(account_id);
    let old_manifest = read_manifest(&paths.manifest(), account_id)?;
    let old_manifest_bytes = fs::read(paths.manifest())?;
    let old_credentials = read_optional_bytes(&paths.credentials())?;
    let now = Utc::now().to_rfc3339();
    let new_manifest = AccountManifest {
        version: 1,
        account_id: account_id.clone(),
        role_id: capture.credentials.role_id,
        battle_tag: capture.battle_tag.clone(),
        created_at: old_manifest.created_at,
        updated_at: now.clone(),
    };

    let staged_root = pending.account_root(account_id);
    stage_capture(&staged_root, account_id, &capture, &new_manifest.created_at)?;
    write_bytes_atomic(&pending.root.join("old-manifest.json"), &old_manifest_bytes)?;
    if let Some(bytes) = &old_credentials {
        write_bytes_atomic(&pending.root.join("old-credentials.json"), bytes)?;
    }

    save_credentials_atomic(&paths.credentials(), &capture.credentials)?;
    if let Err(error) = write_manifest_atomic(&paths.manifest(), &new_manifest) {
        restore_optional_file(&paths.credentials(), old_credentials.as_deref())?;
        return Err(error);
    }
    let mut updated_registry = registry.clone();
    let entry = updated_registry
        .accounts
        .iter_mut()
        .find(|entry| &entry.account_id == account_id)
        .context("account summary is absent")?;
    entry.battle_tag = capture.battle_tag.clone();
    entry.credential_state = "ready".to_string();
    entry.last_used_at = now;
    if let Err(error) = manager.replace_registry(updated_registry) {
        restore_optional_file(&paths.credentials(), old_credentials.as_deref())?;
        write_bytes_atomic(&paths.manifest(), &old_manifest_bytes)?;
        return Err(error);
    }

    let migration_warning = from_add && adopt_legacy_if_waiting(manager, account_id).is_err();
    Ok(AccountLoginResult {
        kind: "existingAccountUpdated".to_string(),
        account_id: Some(account_id.as_str().to_string()),
        battle_tag: Some(capture.battle_tag),
        message: if migration_warning {
            "账号登录已更新，旧数据暂待恢复。".to_string()
        } else {
            "账号登录已更新。".to_string()
        },
        should_offer_switch: registry.active_account_id.as_ref() != Some(account_id),
    })
}

fn find_account_by_role(
    manager: &AccountContextManager,
    entries: &[AccountRegistryEntry],
    role_id: i64,
) -> Result<Option<AccountId>> {
    let mut matching = None;
    for entry in entries {
        let account_paths = manager.shared_paths().account(&entry.account_id);
        ensure_not_link_or_reparse(account_paths.root())?;
        let manifest = read_manifest(&account_paths.manifest(), &entry.account_id)?;
        validate_stored_manifest(&manifest, entry)?;
        if manifest.role_id == role_id {
            if matching.is_some() {
                bail!("duplicate internal account identity")
            }
            matching = Some(entry.account_id.clone());
        }
    }
    Ok(matching)
}

fn validate_capture(capture: &LoginCapture) -> Result<()> {
    if capture.credentials.role_id <= 0 || capture.credentials.token.trim().is_empty() {
        bail!("captured credentials are incomplete")
    }
    validate_battle_tag(&capture.battle_tag)
}

fn validate_battle_tag(battle_tag: &str) -> Result<()> {
    let battle_tag = battle_tag.trim();
    let (name, discriminator) = battle_tag
        .split_once('#')
        .context("captured battle tag is unstable")?;
    if name.trim().is_empty()
        || discriminator.trim().is_empty()
        || discriminator.contains('#')
        || !discriminator
            .chars()
            .all(|character| character.is_ascii_digit())
        || battle_tag.len() > 128
    {
        bail!("captured battle tag is unstable")
    }
    Ok(())
}

fn validate_stored_manifest(
    manifest: &AccountManifest,
    entry: &AccountRegistryEntry,
) -> Result<()> {
    if manifest.role_id <= 0
        || manifest.created_at.trim().is_empty()
        || manifest.updated_at.trim().is_empty()
        || manifest.battle_tag != entry.battle_tag
    {
        bail!("stored account manifest is incomplete")
    }
    validate_battle_tag(&manifest.battle_tag)
}

fn validate_staged_account(
    manifest_path: &Path,
    credentials_path: &Path,
    account_id: &AccountId,
) -> Result<()> {
    let manifest = read_manifest(manifest_path, account_id)?;
    let credentials: DashenCredentials = serde_json::from_slice(&fs::read(credentials_path)?)?;
    if manifest.account_id != *account_id || manifest.role_id != credentials.role_id {
        bail!("staged account identity mismatch")
    }
    validate_capture(&LoginCapture {
        credentials,
        battle_tag: manifest.battle_tag,
    })
}

fn stage_capture(
    staged_root: &Path,
    account_id: &AccountId,
    capture: &LoginCapture,
    created_at: &str,
) -> Result<()> {
    fs::create_dir_all(staged_root)?;
    let manifest = AccountManifest {
        version: 1,
        account_id: account_id.clone(),
        role_id: capture.credentials.role_id,
        battle_tag: capture.battle_tag.clone(),
        created_at: created_at.to_string(),
        updated_at: Utc::now().to_rfc3339(),
    };
    write_manifest_atomic(&staged_root.join("account.json"), &manifest)?;
    save_credentials_atomic(&staged_root.join("credentials.json"), &capture.credentials)?;
    validate_staged_account(
        &staged_root.join("account.json"),
        &staged_root.join("credentials.json"),
        account_id,
    )
}

fn adopt_legacy_if_waiting(manager: &AccountContextManager, account_id: &AccountId) -> Result<()> {
    if manager.migration_state()? == LegacyMigrationState::WaitingForAccount {
        manager.adopt_legacy(&manager.snapshot_for(account_id))?;
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    write_bytes_atomic(path, &serde_json::to_vec_pretty(value)?)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("atomic path has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("data"),
        Uuid::new_v4()
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn restore_optional_file(path: &Path, bytes: Option<&[u8]>) -> Result<()> {
    if let Some(bytes) = bytes {
        write_bytes_atomic(path, bytes)
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn ensure_safe_pending_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if path_entry_exists(parent)? {
            ensure_not_link_or_reparse(parent)?;
        } else {
            fs::create_dir_all(parent)?;
            ensure_not_link_or_reparse(parent)?;
        }
    }
    if path_entry_exists(path)? {
        ensure_not_link_or_reparse(path)?;
    }
    Ok(())
}

fn ensure_safe_accounts_root(manager: &AccountContextManager) -> Result<()> {
    let accounts_root = manager.shared_paths().accounts_root();
    if path_entry_exists(&accounts_root)? {
        ensure_not_link_or_reparse(&accounts_root)?;
        if !fs::metadata(&accounts_root)?.is_dir() {
            bail!("account storage root is not a directory")
        }
    } else {
        fs::create_dir_all(&accounts_root)?;
        ensure_not_link_or_reparse(&accounts_root)?;
    }
    Ok(())
}

fn ensure_not_link_or_reparse(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
        bail!("unsafe account operation path")
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn path_entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn remove_pending_operation(root: &Path, pending_root: &Path) -> Result<()> {
    let operation_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .context("pending operation has no id")?;
    AccountId::parse(operation_name).context("pending operation id is invalid")?;
    if root.parent() != Some(pending_root) {
        bail!("pending operation escaped its root")
    }
    if path_entry_exists(root)? {
        ensure_not_link_or_reparse(root)?;
        fs::remove_dir_all(root)?;
    }
    if path_entry_exists(pending_root)? {
        ensure_not_link_or_reparse(pending_root)?;
        if fs::read_dir(pending_root)?.next().is_none() {
            fs::remove_dir(pending_root)?;
        }
    }
    Ok(())
}

fn remove_published_account(root: &Path) -> Result<()> {
    ensure_not_link_or_reparse(root)?;
    fs::remove_dir_all(root)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_new_rejects_existing_publication_target_without_overwriting_it() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let pending = PendingLogin::create(&manager).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let final_paths = shared.account(&account_id);
        fs::create_dir_all(final_paths.root()).unwrap();
        fs::write(final_paths.root().join("marker"), b"keep").unwrap();
        let capture = LoginCapture {
            credentials: DashenCredentials {
                role_id: 101101,
                token: "synthetic-secret".to_string(),
                customer_token: None,
                dts: None,
                server: 1,
                saved_at: "2026-07-16T00:00:00Z".to_string(),
            },
            battle_tag: "Synthetic#1011".to_string(),
        };

        assert!(add_new(
            &manager,
            &pending,
            manager.registry_snapshot().unwrap(),
            account_id,
            capture,
        )
        .is_err());
        assert_eq!(
            fs::read(final_paths.root().join("marker")).unwrap(),
            b"keep"
        );
        assert!(manager.registry_snapshot().unwrap().accounts.is_empty());
        drop(pending);
        assert!(!shared.pending_logins().exists());
    }
}
