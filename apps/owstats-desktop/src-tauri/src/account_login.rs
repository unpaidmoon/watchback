use crate::account_context::AccountContextManager;
use crate::account_migration::LegacyMigrationState;
use crate::account_store::{
    read_manifest, write_manifest_atomic, AccountId, AccountManifest, AccountRegistryEntry,
};
use crate::credential_store::{save_credentials_atomic, DashenCredentials};
use crate::login_flow::{LoginPhase, LoginProgressReporter};
use crate::models::AccountLoginResult;
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const SAFE_LOGIN_ERROR: &str = "账号登录处理失败，请重试。";
const LOGIN_OPERATION_VERSION: u32 = 1;
const JOURNAL_FILE: &str = "operation.json";
const OLD_MANIFEST_FILE: &str = "old-manifest.json";
const OLD_CREDENTIALS_FILE: &str = "old-credentials.json";
const OLD_CREDENTIALS_MISSING_FILE: &str = "old-credentials-missing";
const OLD_REGISTRY_FILE: &str = "old-registry.json";
const INTENDED_REGISTRY_FILE: &str = "intended-registry.json";
const ORPHAN_QUARANTINE_DIR: &str = "orphan-quarantine";

#[derive(Debug, Clone)]
pub struct LoginCapture {
    pub credentials: DashenCredentials,
    pub battle_tag: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoginOperationKind {
    AddNew,
    UpdateExisting,
    Relogin,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoginOperationPhase {
    Staged,
    AccountPublished,
    CredentialsPublished,
    ManifestPublished,
    RegistryCommitted,
    CleanupPending,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginOperationJournal {
    pub version: u32,
    pub operation_id: AccountId,
    pub kind: LoginOperationKind,
    pub account_id: AccountId,
    pub phase: LoginOperationPhase,
    pub battle_tag: String,
    pub started_at: String,
    pub updated_at: String,
}

struct PendingLogin {
    root: PathBuf,
    pending_root: PathBuf,
    operation_id: AccountId,
    journal: Option<LoginOperationJournal>,
    cleaned: bool,
    preserve_for_recovery: bool,
}

impl PendingLogin {
    fn create(manager: &AccountContextManager) -> Result<Self> {
        let pending_root = manager.shared_paths().pending_logins();
        ensure_safe_pending_parent(&pending_root)?;
        fs::create_dir_all(&pending_root)?;
        ensure_safe_pending_parent(&pending_root)?;
        ensure_not_link_or_reparse(&pending_root)?;
        let operation_id = AccountId::new();
        let root = pending_root.join(operation_id.as_str());
        fs::create_dir(&root)?;
        ensure_not_link_or_reparse(&root)?;
        Ok(Self {
            root,
            pending_root,
            operation_id,
            journal: None,
            cleaned: false,
            preserve_for_recovery: false,
        })
    }

    fn account_root(&self, account_id: &AccountId) -> PathBuf {
        self.root.join("account").join(account_id.as_str())
    }

    fn initialize(
        &mut self,
        kind: LoginOperationKind,
        account_id: &AccountId,
        battle_tag: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let journal = LoginOperationJournal {
            version: LOGIN_OPERATION_VERSION,
            operation_id: self.operation_id.clone(),
            kind,
            account_id: account_id.clone(),
            phase: LoginOperationPhase::Staged,
            battle_tag: battle_tag.to_string(),
            started_at: now.clone(),
            updated_at: now,
        };
        write_json_atomic(&self.root.join(JOURNAL_FILE), &journal)?;
        self.journal = Some(journal);
        Ok(())
    }

    fn advance(&mut self, phase: LoginOperationPhase) -> Result<()> {
        let journal = self
            .journal
            .as_mut()
            .context("login operation journal is not initialized")?;
        journal.phase = phase;
        journal.updated_at = Utc::now().to_rfc3339();
        write_json_atomic(&self.root.join(JOURNAL_FILE), journal)
    }

    fn cleanup_explicit(&mut self) -> Result<()> {
        if self.cleaned {
            return Ok(());
        }
        if self.journal.is_some() {
            self.advance(LoginOperationPhase::CleanupPending)?;
        }
        remove_pending_operation(&self.root, &self.pending_root)?;
        self.cleaned = true;
        Ok(())
    }

    fn preserve(&mut self) {
        self.preserve_for_recovery = true;
    }

    fn cleanup(&self) {
        let _ = remove_pending_operation(&self.root, &self.pending_root);
    }
}

impl Drop for PendingLogin {
    fn drop(&mut self) {
        if !self.cleaned && !self.preserve_for_recovery {
            self.cleanup();
        }
    }
}

pub(crate) fn recover_pending_logins(shared: &crate::app_paths::SharedAppPaths) -> Result<()> {
    recover_pending_logins_with(shared, &NoRecoveryFault)
}

trait RecoveryFault {
    fn before_remove_published_account(&self, _account_id: &AccountId) -> Result<()> {
        Ok(())
    }

    fn before_cleanup_pending(&self, _operation_id: &AccountId) -> Result<()> {
        Ok(())
    }

    fn after_restore_old_credentials(&self, _account_id: &AccountId) -> Result<()> {
        Ok(())
    }
}

struct NoRecoveryFault;

impl RecoveryFault for NoRecoveryFault {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublicationFaultPoint {
    NewAccountPublished,
    NewRegistryCommitted,
    ExistingCredentialsPublished,
    ExistingManifestPublished,
    ExistingRegistryCommitted,
    CleanupPending,
}

trait PublicationFault {
    fn after(&self, _point: PublicationFaultPoint) -> Result<()> {
        Ok(())
    }
}

struct NoPublicationFault;

impl PublicationFault for NoPublicationFault {}

fn recover_pending_logins_with(
    shared: &crate::app_paths::SharedAppPaths,
    fault: &impl RecoveryFault,
) -> Result<()> {
    let pending_root = shared.pending_logins();
    ensure_safe_pending_parent(&pending_root)?;
    if !path_entry_exists(&pending_root)? {
        return Ok(());
    }
    ensure_not_link_or_reparse(&pending_root)?;
    if !fs::metadata(&pending_root)?.is_dir() {
        bail!("pending login root is not a directory")
    }
    let mut operations = fs::read_dir(&pending_root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    operations.sort();
    for operation_root in operations {
        let operation_name = operation_root
            .file_name()
            .and_then(|name| name.to_str())
            .context("pending login operation has no id")?;
        let operation_id = AccountId::parse(operation_name)?;
        ensure_not_link_or_reparse(&operation_root)?;
        if !fs::metadata(&operation_root)?.is_dir() {
            bail!("pending login operation is not a directory")
        }
        let journal_path = operation_root.join(JOURNAL_FILE);
        if !path_entry_exists(&journal_path)? {
            fault.before_cleanup_pending(&operation_id)?;
            remove_pending_operation(&operation_root, &pending_root)?;
            continue;
        }
        ensure_regular_file_no_reparse(&journal_path)?;
        let journal: LoginOperationJournal = serde_json::from_slice(&fs::read(&journal_path)?)?;
        validate_login_journal(&journal, &operation_id)?;
        recover_login_operation(shared, &operation_root, &journal, fault)?;
    }
    Ok(())
}

fn validate_login_journal(journal: &LoginOperationJournal, operation_id: &AccountId) -> Result<()> {
    if journal.version != LOGIN_OPERATION_VERSION
        || &journal.operation_id != operation_id
        || journal.started_at.trim().is_empty()
        || journal.updated_at.trim().is_empty()
    {
        bail!("invalid login operation journal")
    }
    validate_battle_tag(&journal.battle_tag)
}

fn recover_login_operation(
    shared: &crate::app_paths::SharedAppPaths,
    operation_root: &Path,
    journal: &LoginOperationJournal,
    fault: &impl RecoveryFault,
) -> Result<()> {
    let registry = read_optional_registry(shared)?;
    match journal.kind {
        LoginOperationKind::AddNew => {
            recover_new_login(shared, operation_root, journal, &registry, fault)
        }
        LoginOperationKind::UpdateExisting | LoginOperationKind::Relogin => {
            recover_existing_login(shared, operation_root, journal, &registry, fault)
        }
    }
}

fn recover_new_login(
    shared: &crate::app_paths::SharedAppPaths,
    operation_root: &Path,
    journal: &LoginOperationJournal,
    registry: &crate::account_store::AccountRegistry,
    fault: &impl RecoveryFault,
) -> Result<()> {
    let registered = registry
        .accounts
        .iter()
        .find(|entry| entry.account_id == journal.account_id);
    if let Some(entry) = registered {
        if entry.battle_tag != journal.battle_tag {
            bail!("recovered account summary does not match journal")
        }
        validate_formal_account(shared, &journal.account_id, &journal.battle_tag)?;
    } else {
        let final_root = shared.account(&journal.account_id);
        let quarantine_parent = operation_root.join(ORPHAN_QUARANTINE_DIR);
        let quarantined = quarantine_parent.join(journal.account_id.as_str());
        if path_entry_exists(&quarantined)? {
            ensure_directory_no_reparse(&quarantined)?;
            validate_tree_no_reparse(&quarantined)?;
            fs::remove_dir_all(&quarantined)?;
        }
        if path_entry_exists(final_root.root())? {
            ensure_safe_accounts_root_path(shared)?;
            ensure_directory_no_reparse(final_root.root())?;
            fault.before_remove_published_account(&journal.account_id)?;
            if !path_entry_exists(&quarantine_parent)? {
                fs::create_dir(&quarantine_parent)?;
            }
            ensure_directory_no_reparse(&quarantine_parent)?;
            fs::rename(final_root.root(), &quarantined)?;
            ensure_directory_no_reparse(&quarantined)?;
            validate_tree_no_reparse(&quarantined)?;
            fs::remove_dir_all(&quarantined)?;
        }
        if path_entry_exists(&quarantine_parent)? {
            ensure_directory_no_reparse(&quarantine_parent)?;
            if fs::read_dir(&quarantine_parent)?.next().is_none() {
                fs::remove_dir(&quarantine_parent)?;
            }
        }
    }
    fault.before_cleanup_pending(&journal.operation_id)?;
    remove_pending_operation(operation_root, &shared.pending_logins())
}

fn recover_existing_login(
    shared: &crate::app_paths::SharedAppPaths,
    operation_root: &Path,
    journal: &LoginOperationJournal,
    registry: &crate::account_store::AccountRegistry,
    fault: &impl RecoveryFault,
) -> Result<()> {
    let old_registry = read_operation_registry(operation_root, OLD_REGISTRY_FILE)
        .context("missing previous account registry")?;
    let intended_registry = read_operation_registry(operation_root, INTENDED_REGISTRY_FILE)
        .context("missing intended account registry")?;
    if registry == &intended_registry {
        validate_formal_account_matches_staged(shared, operation_root, &journal.account_id)?;
        fault.before_cleanup_pending(&journal.operation_id)?;
        return remove_pending_operation(operation_root, &shared.pending_logins());
    }
    if registry != &old_registry {
        if crate::account_removal::deletion_journal_proves_removed_account(
            shared,
            &journal.account_id,
            &old_registry,
            &intended_registry,
            registry,
        ) {
            fault.before_cleanup_pending(&journal.operation_id)?;
            return remove_pending_operation(operation_root, &shared.pending_logins());
        }
        bail!("account registry is neither previous nor committed state")
    }
    let paths = shared.account(&journal.account_id);
    ensure_safe_accounts_root_path(shared)?;
    ensure_not_link_or_reparse(paths.root())?;
    let old_credentials_path = operation_root.join(OLD_CREDENTIALS_FILE);
    let missing_marker = operation_root.join(OLD_CREDENTIALS_MISSING_FILE);
    if path_entry_exists(&old_credentials_path)? {
        ensure_not_link_or_reparse(&old_credentials_path)?;
        write_bytes_atomic(&paths.credentials(), &fs::read(old_credentials_path)?)?;
    } else if path_entry_exists(&missing_marker)? {
        ensure_not_link_or_reparse(&missing_marker)?;
        restore_optional_file(&paths.credentials(), None)?;
    } else {
        bail!("previous credential backup state is missing")
    }
    fault.after_restore_old_credentials(&journal.account_id)?;
    let old_manifest_path = operation_root.join(OLD_MANIFEST_FILE);
    ensure_regular_file_no_reparse(&old_manifest_path)?;
    let old_manifest = fs::read(old_manifest_path)?;
    write_bytes_atomic(&paths.manifest(), &old_manifest)?;
    fault.before_cleanup_pending(&journal.operation_id)?;
    remove_pending_operation(operation_root, &shared.pending_logins())
}

fn read_operation_registry(
    operation_root: &Path,
    file_name: &str,
) -> Result<crate::account_store::AccountRegistry> {
    let path = operation_root.join(file_name);
    ensure_not_link_or_reparse(&path)?;
    crate::account_store::read_registry(&path)
}

fn validate_formal_account_matches_staged(
    shared: &crate::app_paths::SharedAppPaths,
    operation_root: &Path,
    account_id: &AccountId,
) -> Result<()> {
    ensure_safe_accounts_root_path(shared)?;
    let formal = shared.account(account_id);
    ensure_not_link_or_reparse(formal.root())?;
    let staged_root = operation_root.join("account").join(account_id.as_str());
    ensure_not_link_or_reparse(&staged_root)?;
    let formal_manifest = read_manifest(&formal.manifest(), account_id)?;
    let staged_manifest = read_manifest(&staged_root.join("account.json"), account_id)?;
    ensure_regular_file_no_reparse(&formal.credentials())?;
    let staged_credentials_path = staged_root.join("credentials.json");
    ensure_regular_file_no_reparse(&staged_credentials_path)?;
    let formal_credentials: DashenCredentials =
        serde_json::from_slice(&fs::read(formal.credentials())?)?;
    let staged_credentials: DashenCredentials =
        serde_json::from_slice(&fs::read(staged_credentials_path)?)?;
    if formal_manifest != staged_manifest || formal_credentials != staged_credentials {
        bail!("formal account does not match intended staged account")
    }
    validate_formal_account(shared, account_id, &staged_manifest.battle_tag)
}

fn read_optional_registry(
    shared: &crate::app_paths::SharedAppPaths,
) -> Result<crate::account_store::AccountRegistry> {
    if path_entry_exists(&shared.registry())? {
        crate::account_store::read_registry(&shared.registry())
    } else {
        Ok(crate::account_store::AccountRegistry::default())
    }
}

fn validate_formal_account(
    shared: &crate::app_paths::SharedAppPaths,
    account_id: &AccountId,
    expected_battle_tag: &str,
) -> Result<()> {
    ensure_safe_accounts_root_path(shared)?;
    let paths = shared.account(account_id);
    ensure_not_link_or_reparse(paths.root())?;
    let manifest = read_manifest(&paths.manifest(), account_id)?;
    if manifest.role_id <= 0 || manifest.battle_tag != expected_battle_tag {
        bail!("formal account manifest is incomplete")
    }
    let credentials = crate::credential_store::read_credentials(&paths.credentials())?;
    if credentials.role_id <= 0
        || credentials.token.trim().is_empty()
        || credentials.role_id != manifest.role_id
    {
        bail!("formal account credentials are incomplete")
    }
    Ok(())
}

fn ensure_safe_accounts_root_path(shared: &crate::app_paths::SharedAppPaths) -> Result<()> {
    let accounts_root = shared.accounts_root();
    ensure_existing_directory_ancestors_safe(&accounts_root)?;
    if !path_entry_exists(&accounts_root)? {
        bail!("account storage root is missing")
    }
    ensure_not_link_or_reparse(&accounts_root)?;
    if !fs::metadata(&accounts_root)?.is_dir() {
        bail!("account storage root is not a directory")
    }
    Ok(())
}

pub fn publish_add_account(
    manager: &AccountContextManager,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    let mut result = publish_add_account_inner(manager, capture).map_err(publication_error)?;
    if let Some(account_id) = result
        .account_id
        .as_deref()
        .and_then(|value| AccountId::parse(value).ok())
    {
        if adopt_legacy_if_waiting(manager, &account_id).is_err() {
            result.message = if result.kind == "newAccountAdded" {
                "账号已添加，旧数据暂待恢复。".to_string()
            } else {
                "账号登录已更新，旧数据暂待恢复。".to_string()
            };
        }
    }
    Ok(result)
}

fn publish_add_account_inner(
    manager: &AccountContextManager,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    publish_add_account_inner_with_fault(manager, capture, &NoPublicationFault)
}

fn publish_add_account_inner_with_fault(
    manager: &AccountContextManager,
    capture: LoginCapture,
    fault: &impl PublicationFault,
) -> Result<AccountLoginResult> {
    validate_capture(&capture)?;
    let _publication = manager.lock_login_publication()?;
    ensure_safe_accounts_root(manager)?;
    let registry = manager.registry_snapshot()?;
    let matching = find_account_by_role(manager, &registry.accounts, capture.credentials.role_id)?;
    let mut pending = PendingLogin::create(manager)?;
    if let Some(account_id) = matching {
        if let Err(error) = prepare_existing_pending(
            manager,
            &mut pending,
            &registry,
            &account_id,
            &capture,
            LoginOperationKind::UpdateExisting,
        ) {
            return fail_before_commit(&mut pending, error);
        }
        update_existing(
            manager,
            &mut pending,
            &registry,
            &account_id,
            capture,
            true,
            fault,
        )
    } else {
        let account_id = AccountId::new();
        add_new(manager, &mut pending, registry, account_id, capture, fault)
    }
}

pub fn publish_relogin_account(
    manager: &AccountContextManager,
    expected_account_id: &AccountId,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    publish_relogin_account_inner(manager, expected_account_id, capture).map_err(publication_error)
}

fn publication_error(error: anyhow::Error) -> anyhow::Error {
    if error.to_string() == crate::account_context::ACCOUNT_DELETION_RECOVERY_ERROR {
        anyhow!(crate::account_context::ACCOUNT_DELETION_RECOVERY_ERROR)
    } else {
        anyhow!(SAFE_LOGIN_ERROR)
    }
}

pub(crate) async fn capture_and_publish_relogin<F, Fut, R, C>(
    manager: &AccountContextManager,
    expected_account_id: &AccountId,
    progress: &R,
    capture: F,
    was_cancelled: C,
) -> Result<AccountLoginResult>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<LoginCapture>>,
    R: LoginProgressReporter,
    C: FnOnce() -> bool,
{
    manager.prepare_login_capture().map_err(publication_error)?;
    let capture = match capture().await {
        Ok(capture) => capture,
        Err(_) if was_cancelled() => {
            progress.report(LoginPhase::Cancelled, "已取消登录等待。");
            bail!("登录已取消。")
        }
        Err(_) => {
            progress.report(LoginPhase::Failed, "登录失败，请重试。");
            bail!("账号登录处理失败，请重试。")
        }
    };
    progress.report(LoginPhase::Saving, "正在安全保存账号登录信息。");
    match publish_relogin_account(manager, expected_account_id, capture) {
        Ok(result) if result.kind == "existingAccountUpdated" => {
            progress.report(LoginPhase::Ready, "账号登录信息已更新。");
            Ok(result)
        }
        Ok(result) => {
            progress.report(LoginPhase::Failed, "登录账号与所选账号不一致。");
            Ok(result)
        }
        Err(error) => {
            progress.report(LoginPhase::Failed, "账号登录处理失败，请重试。");
            if error.to_string() == crate::account_context::ACCOUNT_DELETION_RECOVERY_ERROR {
                bail!(crate::account_context::ACCOUNT_DELETION_RECOVERY_ERROR)
            }
            bail!("账号登录处理失败，请重试。")
        }
    }
}

pub(crate) async fn capture_and_publish_add<F, Fut, R, C>(
    manager: &AccountContextManager,
    progress: &R,
    capture: F,
    was_cancelled: C,
) -> Result<AccountLoginResult>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<LoginCapture>>,
    R: LoginProgressReporter,
    C: FnOnce() -> bool,
{
    manager.prepare_login_capture().map_err(publication_error)?;
    let capture = match capture().await {
        Ok(capture) => capture,
        Err(_) if was_cancelled() => {
            progress.report(LoginPhase::Cancelled, "已取消登录等待。");
            bail!("登录已取消。")
        }
        Err(_) => {
            progress.report(LoginPhase::Failed, "登录失败，请重试。");
            bail!("账号登录处理失败，请重试。")
        }
    };
    progress.report(LoginPhase::Saving, "正在安全保存账号登录信息。");
    match publish_add_account(manager, capture) {
        Ok(result) => {
            progress.report(LoginPhase::Ready, "账号登录信息已保存。");
            Ok(result)
        }
        Err(error) => {
            progress.report(LoginPhase::Failed, "账号登录处理失败，请重试。");
            if error.to_string() == crate::account_context::ACCOUNT_DELETION_RECOVERY_ERROR {
                bail!(crate::account_context::ACCOUNT_DELETION_RECOVERY_ERROR)
            }
            bail!("账号登录处理失败，请重试。")
        }
    }
}

fn publish_relogin_account_inner(
    manager: &AccountContextManager,
    expected_account_id: &AccountId,
    capture: LoginCapture,
) -> Result<AccountLoginResult> {
    publish_relogin_account_inner_with_fault(
        manager,
        expected_account_id,
        capture,
        &NoPublicationFault,
    )
}

fn publish_relogin_account_inner_with_fault(
    manager: &AccountContextManager,
    expected_account_id: &AccountId,
    capture: LoginCapture,
    fault: &impl PublicationFault,
) -> Result<AccountLoginResult> {
    validate_capture(&capture)?;
    let _publication = manager.lock_login_publication()?;
    ensure_safe_accounts_root(manager)?;
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
    let other_match = if expected_manifest.role_id != capture.credentials.role_id {
        find_account_by_role(manager, &registry.accounts, capture.credentials.role_id)?
    } else {
        None
    };
    let mut pending = PendingLogin::create(manager)?;
    if let Err(error) = prepare_existing_pending(
        manager,
        &mut pending,
        &registry,
        expected_account_id,
        &capture,
        LoginOperationKind::Relogin,
    ) {
        return fail_before_commit(&mut pending, error);
    }
    if expected_manifest.role_id == capture.credentials.role_id {
        return update_existing(
            manager,
            &mut pending,
            &registry,
            expected_account_id,
            capture,
            false,
            fault,
        );
    }

    if let Some(other_id) = other_match {
        let entry = registry
            .accounts
            .iter()
            .find(|entry| entry.account_id == other_id)
            .context("matched account summary is absent")?;
        return finish_unmodified(
            &mut pending,
            AccountLoginResult {
                kind: "differentExistingAccount".to_string(),
                account_id: Some(other_id.as_str().to_string()),
                battle_tag: Some(entry.battle_tag.clone()),
                message: "检测到另一个已保存账号，可切换后重新登录。".to_string(),
                should_offer_switch: true,
            },
            fault,
        );
    }

    finish_unmodified(
        &mut pending,
        AccountLoginResult {
            kind: "differentNewAccount".to_string(),
            account_id: None,
            battle_tag: Some(capture.battle_tag),
            message: "本次登录属于另一个新账号，未覆盖当前账号。".to_string(),
            should_offer_switch: false,
        },
        fault,
    )
}

fn add_new(
    manager: &AccountContextManager,
    pending: &mut PendingLogin,
    mut registry: crate::account_store::AccountRegistry,
    account_id: AccountId,
    capture: LoginCapture,
    fault: &impl PublicationFault,
) -> Result<AccountLoginResult> {
    let now = Utc::now().to_rfc3339();
    let pending_account_root = pending.account_root(&account_id);
    if let Err(error) =
        stage_capture(&pending_account_root, &account_id, &capture, &now).and_then(|_| {
            pending.initialize(LoginOperationKind::AddNew, &account_id, &capture.battle_tag)
        })
    {
        return fail_before_commit(pending, error);
    }
    let staged_manifest = pending_account_root.join("account.json");
    let staged_credentials = pending_account_root.join("credentials.json");
    if let Err(error) = validate_staged_account(&staged_manifest, &staged_credentials, &account_id)
    {
        return fail_before_commit(pending, error);
    }

    let final_paths = manager.shared_paths().account(&account_id);
    if path_entry_exists(final_paths.root())? {
        return fail_before_commit(
            pending,
            anyhow!("account publication target already exists"),
        );
    }
    if let Err(error) = fs::rename(&pending_account_root, final_paths.root()) {
        return fail_before_commit(pending, error);
    }
    if let Err(error) = fault.after(PublicationFaultPoint::NewAccountPublished) {
        pending.preserve();
        return Err(error);
    }
    if let Err(error) = pending.advance(LoginOperationPhase::AccountPublished) {
        pending.preserve();
        return Err(error);
    }

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
    if let Err(error) = manager.replace_registry(registry) {
        return recover_failed_publication(manager, pending, error);
    }
    if let Err(error) = fault.after(PublicationFaultPoint::NewRegistryCommitted) {
        pending.preserve();
        return Err(error);
    }
    let mut result = AccountLoginResult {
        kind: "newAccountAdded".to_string(),
        account_id: Some(account_id.as_str().to_string()),
        battle_tag: Some(capture.battle_tag.clone()),
        message: "账号已添加。".to_string(),
        should_offer_switch: false,
    };
    if pending
        .advance(LoginOperationPhase::RegistryCommitted)
        .is_err()
    {
        pending.preserve();
        result.message = pending_cleanup_message();
        return Ok(result);
    }

    finish_committed(pending, result, fault)
}

fn update_existing(
    manager: &AccountContextManager,
    pending: &mut PendingLogin,
    registry: &crate::account_store::AccountRegistry,
    account_id: &AccountId,
    capture: LoginCapture,
    _from_add: bool,
    fault: &impl PublicationFault,
) -> Result<AccountLoginResult> {
    let paths = manager.shared_paths().account(account_id);
    let staged_root = pending.account_root(account_id);
    let staged_credentials = staged_root.join("credentials.json");
    let staged_manifest = staged_root.join("account.json");
    if let Err(error) = ensure_regular_file_no_reparse(&staged_credentials)
        .and_then(|_| ensure_regular_file_no_reparse(&staged_manifest))
    {
        return fail_before_commit(pending, error);
    }
    let staged_credentials_bytes = match fs::read(&staged_credentials) {
        Ok(bytes) => bytes,
        Err(error) => return fail_before_commit(pending, error),
    };
    let staged_manifest_bytes = match fs::read(&staged_manifest) {
        Ok(bytes) => bytes,
        Err(error) => return fail_before_commit(pending, error),
    };
    let updated_registry = match read_operation_registry(&pending.root, INTENDED_REGISTRY_FILE) {
        Ok(registry) => registry,
        Err(error) => return fail_before_commit(pending, error),
    };

    if let Err(error) = write_bytes_atomic(&paths.credentials(), &staged_credentials_bytes) {
        return recover_failed_publication(manager, pending, error);
    }
    if let Err(error) = fault.after(PublicationFaultPoint::ExistingCredentialsPublished) {
        pending.preserve();
        return Err(error);
    }
    if let Err(error) = pending.advance(LoginOperationPhase::CredentialsPublished) {
        pending.preserve();
        return Err(error);
    }
    if let Err(error) = write_bytes_atomic(&paths.manifest(), &staged_manifest_bytes) {
        return recover_failed_publication(manager, pending, error);
    }
    if let Err(error) = fault.after(PublicationFaultPoint::ExistingManifestPublished) {
        pending.preserve();
        return Err(error);
    }
    if let Err(error) = pending.advance(LoginOperationPhase::ManifestPublished) {
        pending.preserve();
        return Err(error);
    }
    if let Err(error) = manager.replace_registry(updated_registry) {
        return recover_failed_publication(manager, pending, error);
    }
    if let Err(error) = fault.after(PublicationFaultPoint::ExistingRegistryCommitted) {
        pending.preserve();
        return Err(error);
    }

    let mut result = AccountLoginResult {
        kind: "existingAccountUpdated".to_string(),
        account_id: Some(account_id.as_str().to_string()),
        battle_tag: Some(capture.battle_tag.clone()),
        message: "账号登录已更新。".to_string(),
        should_offer_switch: registry.active_account_id.as_ref() != Some(account_id),
    };
    if pending
        .advance(LoginOperationPhase::RegistryCommitted)
        .is_err()
    {
        pending.preserve();
        result.message = pending_cleanup_message();
        return Ok(result);
    }
    finish_committed(pending, result, fault)
}

fn prepare_existing_pending(
    manager: &AccountContextManager,
    pending: &mut PendingLogin,
    registry: &crate::account_store::AccountRegistry,
    account_id: &AccountId,
    capture: &LoginCapture,
    kind: LoginOperationKind,
) -> Result<()> {
    let paths = manager.shared_paths().account(account_id);
    ensure_not_link_or_reparse(paths.root())?;
    let old_manifest = read_manifest(&paths.manifest(), account_id)?;
    stage_capture(
        &pending.account_root(account_id),
        account_id,
        capture,
        &old_manifest.created_at,
    )?;
    write_bytes_atomic(
        &pending.root.join(OLD_MANIFEST_FILE),
        &read_optional_bytes(&paths.manifest())?.context("account manifest backup is missing")?,
    )?;
    match read_optional_bytes(&paths.credentials())? {
        Some(bytes) => write_bytes_atomic(&pending.root.join(OLD_CREDENTIALS_FILE), &bytes)?,
        None => write_bytes_atomic(&pending.root.join(OLD_CREDENTIALS_MISSING_FILE), b"")?,
    }
    write_bytes_atomic(
        &pending.root.join(OLD_REGISTRY_FILE),
        &read_optional_bytes(&manager.shared_paths().registry())?
            .context("account registry backup is missing")?,
    )?;
    let staged_manifest = read_manifest(
        &pending.account_root(account_id).join("account.json"),
        account_id,
    )?;
    let mut intended_registry = registry.clone();
    let entry = intended_registry
        .accounts
        .iter_mut()
        .find(|entry| &entry.account_id == account_id)
        .context("account summary is absent")?;
    entry.battle_tag = capture.battle_tag.clone();
    entry.credential_state = "ready".to_string();
    entry.last_used_at = staged_manifest.updated_at;
    crate::account_store::write_registry_atomic(
        &pending.root.join(INTENDED_REGISTRY_FILE),
        &intended_registry,
    )?;
    pending.initialize(kind, account_id, &capture.battle_tag)
}

fn fail_before_commit(
    pending: &mut PendingLogin,
    error: impl Into<anyhow::Error>,
) -> Result<AccountLoginResult> {
    if pending.cleanup_explicit().is_err() {
        pending.preserve();
    }
    Err(error.into())
}

fn recover_failed_publication(
    manager: &AccountContextManager,
    pending: &mut PendingLogin,
    error: impl Into<anyhow::Error>,
) -> Result<AccountLoginResult> {
    let recovery = pending
        .journal
        .as_ref()
        .cloned()
        .context("login operation journal is not initialized")
        .and_then(|journal| {
            recover_login_operation(
                manager.shared_paths(),
                &pending.root,
                &journal,
                &NoRecoveryFault,
            )
        });
    if recovery.is_ok() {
        pending.cleaned = true;
    } else {
        pending.preserve();
    }
    Err(error.into())
}

fn finish_unmodified(
    pending: &mut PendingLogin,
    mut result: AccountLoginResult,
    fault: &impl PublicationFault,
) -> Result<AccountLoginResult> {
    if fault.after(PublicationFaultPoint::CleanupPending).is_err() {
        pending.preserve();
        result.message = pending_cleanup_message();
        return Ok(result);
    }
    if pending.cleanup_explicit().is_err() {
        pending.preserve();
        result.message = pending_cleanup_message();
    }
    Ok(result)
}

fn finish_committed(
    pending: &mut PendingLogin,
    mut result: AccountLoginResult,
    fault: &impl PublicationFault,
) -> Result<AccountLoginResult> {
    if fault.after(PublicationFaultPoint::CleanupPending).is_err() {
        pending.preserve();
        result.message = pending_cleanup_message();
        return Ok(result);
    }
    if pending.cleanup_explicit().is_err() {
        pending.preserve();
        result.message = pending_cleanup_message();
    }
    Ok(result)
}

fn pending_cleanup_message() -> String {
    "账号已处理，临时登录数据将在重启后继续清理。".to_string()
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
    let credentials = crate::credential_store::read_credentials(credentials_path)?;
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
    for path in staged_root.ancestors().take(3) {
        ensure_not_link_or_reparse(path)?;
    }
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
        manager.adopt_legacy(&manager.snapshot_for(account_id)?)?;
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    write_bytes_atomic(path, &serde_json::to_vec_pretty(value)?)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("atomic path has no parent")?;
    ensure_existing_directory_ancestors_safe(parent)?;
    if path_entry_exists(path)? {
        ensure_regular_file_no_reparse(path)?;
    }
    fs::create_dir_all(parent)?;
    ensure_directory_no_reparse(parent)?;
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
    if path_entry_exists(path)? {
        ensure_regular_file_no_reparse(path)?;
        Ok(Some(fs::read(path)?))
    } else {
        Ok(None)
    }
}

fn ensure_safe_pending_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_existing_directory_ancestors_safe(parent)?;
    }
    if path_entry_exists(path)? {
        ensure_not_link_or_reparse(path)?;
    }
    Ok(())
}

fn ensure_existing_directory_ancestors_safe(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) {
                    bail!("unsafe account operation path")
                }
                if !metadata.is_dir() {
                    bail!("account operation ancestor is not a directory")
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn ensure_safe_accounts_root(manager: &AccountContextManager) -> Result<()> {
    let accounts_root = manager.shared_paths().accounts_root();
    ensure_existing_directory_ancestors_safe(&accounts_root)?;
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

fn ensure_directory_no_reparse(path: &Path) -> Result<()> {
    ensure_not_link_or_reparse(path)?;
    if !fs::symlink_metadata(path)?.is_dir() {
        bail!("unsafe account operation directory")
    }
    Ok(())
}

fn ensure_regular_file_no_reparse(path: &Path) -> Result<()> {
    ensure_not_link_or_reparse(path)?;
    if !fs::symlink_metadata(path)?.is_file() {
        bail!("unsafe account operation file")
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
        validate_tree_no_reparse(root)?;
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

fn validate_tree_no_reparse(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        ensure_not_link_or_reparse(&path)?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            validate_tree_no_reparse(&path)?;
        } else if !file_type.is_file() {
            bail!("unsafe account operation tree entry")
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::login_flow::{LoginPhase, LoginProgressReporter};
    use std::sync::Mutex;

    struct CrashAfter(PublicationFaultPoint);

    impl PublicationFault for CrashAfter {
        fn after(&self, point: PublicationFaultPoint) -> Result<()> {
            if point == self.0 {
                bail!("injected crash")
            }
            Ok(())
        }
    }

    struct FailPublishedDelete;

    impl RecoveryFault for FailPublishedDelete {
        fn before_remove_published_account(&self, _account_id: &AccountId) -> Result<()> {
            bail!("injected Windows-style delete failure")
        }
    }

    struct FailAfterCredentialRollback;

    impl RecoveryFault for FailAfterCredentialRollback {
        fn after_restore_old_credentials(&self, _account_id: &AccountId) -> Result<()> {
            bail!("injected recovery interruption")
        }
    }

    fn capture(battle_tag: &str, token: &str) -> LoginCapture {
        LoginCapture {
            credentials: DashenCredentials {
                role_id: 101101,
                token: token.to_string(),
                customer_token: None,
                dts: None,
                server: 1,
                saved_at: "2026-07-16T00:00:00Z".to_string(),
            },
            battle_tag: battle_tag.to_string(),
        }
    }

    #[derive(Default)]
    struct RecordingProgress(Mutex<Vec<LoginPhase>>);

    impl LoginProgressReporter for RecordingProgress {
        fn report(&self, phase: LoginPhase, _message: &str) {
            self.0.lock().unwrap().push(phase);
        }
    }

    #[tokio::test]
    async fn capture_and_publish_relogin_reports_order_and_never_overwrites_on_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_existing(&manager, &account_id);
        let old_manifest = fs::read(shared.account(&account_id).manifest()).unwrap();
        let old_registry = fs::read(shared.registry()).unwrap();
        let progress = RecordingProgress::default();
        progress.report(LoginPhase::WaitingForLogin, "waiting");
        progress.report(LoginPhase::RoleFound, "captured");

        let result = capture_and_publish_relogin(
            &manager,
            &account_id,
            &progress,
            || async {
                let mut other = capture("Other#2022", "other-secret");
                other.credentials.role_id = 202202;
                Ok(other)
            },
            || false,
        )
        .await
        .unwrap();

        assert_eq!(result.kind, "differentNewAccount");
        assert_eq!(
            *progress.0.lock().unwrap(),
            vec![
                LoginPhase::WaitingForLogin,
                LoginPhase::RoleFound,
                LoginPhase::Saving,
                LoginPhase::Failed,
            ]
        );
        assert_eq!(
            fs::read(shared.account(&account_id).manifest()).unwrap(),
            old_manifest
        );
        assert_eq!(fs::read(shared.registry()).unwrap(), old_registry);
        assert_eq!(
            serde_json::from_slice::<DashenCredentials>(
                &fs::read(shared.account(&account_id).credentials()).unwrap()
            )
            .unwrap()
            .token,
            "old-secret"
        );
    }

    #[tokio::test]
    async fn capture_and_publish_relogin_reports_ready_failed_and_cancelled_terminal_states() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_existing(&manager, &account_id);

        let ready = RecordingProgress::default();
        ready.report(LoginPhase::WaitingForLogin, "waiting");
        ready.report(LoginPhase::RoleFound, "captured");
        capture_and_publish_relogin(
            &manager,
            &account_id,
            &ready,
            || async { Ok(capture("Old#1011", "new-secret")) },
            || false,
        )
        .await
        .unwrap();
        assert_eq!(
            *ready.0.lock().unwrap(),
            vec![
                LoginPhase::WaitingForLogin,
                LoginPhase::RoleFound,
                LoginPhase::Saving,
                LoginPhase::Ready,
            ]
        );
        assert_eq!(
            serde_json::from_slice::<DashenCredentials>(
                &fs::read(shared.account(&account_id).credentials()).unwrap()
            )
            .unwrap()
            .token,
            "new-secret"
        );
        assert!(!shared.pending_logins().exists());

        let failed = RecordingProgress::default();
        assert!(capture_and_publish_relogin(
            &manager,
            &account_id,
            &failed,
            || async { Err(anyhow!("token=must-not-escape")) },
            || false,
        )
        .await
        .is_err());
        assert_eq!(*failed.0.lock().unwrap(), vec![LoginPhase::Failed]);

        let cancelled = RecordingProgress::default();
        let before_cancel = fs::read(shared.account(&account_id).credentials()).unwrap();
        assert!(capture_and_publish_relogin(
            &manager,
            &account_id,
            &cancelled,
            || async { Err(anyhow!("cancelled token=must-not-escape")) },
            || true,
        )
        .await
        .is_err());
        assert_eq!(*cancelled.0.lock().unwrap(), vec![LoginPhase::Cancelled]);
        assert_eq!(
            fs::read(shared.account(&account_id).credentials()).unwrap(),
            before_cancel
        );
    }

    #[tokio::test]
    async fn capture_and_publish_add_reports_saving_ready_and_safe_failure() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared).unwrap();
        let ready = RecordingProgress::default();

        capture_and_publish_add(
            &manager,
            &ready,
            || async { Ok(capture("New#1011", "new-secret")) },
            || false,
        )
        .await
        .unwrap();
        assert_eq!(
            *ready.0.lock().unwrap(),
            vec![LoginPhase::Saving, LoginPhase::Ready]
        );

        let failed = RecordingProgress::default();
        assert!(capture_and_publish_add(
            &manager,
            &failed,
            || async { Err(anyhow!("token=must-not-escape")) },
            || false,
        )
        .await
        .is_err());
        assert_eq!(*failed.0.lock().unwrap(), vec![LoginPhase::Failed]);
    }

    fn install_existing(manager: &AccountContextManager, account_id: &AccountId) {
        let paths = manager.shared_paths().account(account_id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(
            &paths.manifest(),
            &AccountManifest {
                version: 1,
                account_id: account_id.clone(),
                role_id: 101101,
                battle_tag: "Old#1011".to_string(),
                created_at: "2026-07-16T00:00:00Z".to_string(),
                updated_at: "2026-07-16T00:00:00Z".to_string(),
            },
        )
        .unwrap();
        save_credentials_atomic(
            &paths.credentials(),
            &capture("Old#1011", "old-secret").credentials,
        )
        .unwrap();
        manager
            .replace_registry(crate::account_store::AccountRegistry {
                version: 1,
                active_account_id: Some(account_id.clone()),
                accounts: vec![AccountRegistryEntry {
                    account_id: account_id.clone(),
                    battle_tag: "Old#1011".to_string(),
                    created_at: "2026-07-16T00:00:00Z".to_string(),
                    last_used_at: "2026-07-16T00:00:00Z".to_string(),
                    credential_state: "ready".to_string(),
                }],
            })
            .unwrap();
    }

    #[test]
    fn every_new_publication_crash_point_recovers_from_disk_facts() {
        for point in [
            PublicationFaultPoint::NewAccountPublished,
            PublicationFaultPoint::NewRegistryCommitted,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
            let manager = AccountContextManager::load(shared.clone()).unwrap();
            assert!(publish_add_account_inner_with_fault(
                &manager,
                capture("New#1011", "new-secret"),
                &CrashAfter(point),
            )
            .is_err());
            assert!(shared.pending_logins().is_dir());
            drop(manager);

            let restarted = AccountContextManager::load(shared.clone()).unwrap();
            let registry = restarted.registry_snapshot().unwrap();
            if point == PublicationFaultPoint::NewAccountPublished {
                assert!(registry.accounts.is_empty());
                assert_eq!(fs::read_dir(shared.accounts_root()).unwrap().count(), 0);
            } else {
                assert_eq!(registry.accounts.len(), 1);
                validate_formal_account(&shared, &registry.accounts[0].account_id, "New#1011")
                    .unwrap();
            }
            assert!(!shared.pending_logins().exists());
        }
    }

    #[test]
    fn every_existing_publication_crash_point_recovers_from_disk_facts() {
        for point in [
            PublicationFaultPoint::ExistingCredentialsPublished,
            PublicationFaultPoint::ExistingManifestPublished,
            PublicationFaultPoint::ExistingRegistryCommitted,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
            let manager = AccountContextManager::load(shared.clone()).unwrap();
            let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
            install_existing(&manager, &account_id);
            assert!(publish_add_account_inner_with_fault(
                &manager,
                capture("New#1011", "new-secret"),
                &CrashAfter(point),
            )
            .is_err());
            assert!(shared.pending_logins().is_dir());
            drop(manager);

            let restarted = AccountContextManager::load(shared.clone()).unwrap();
            let registry = restarted.registry_snapshot().unwrap();
            let expected_tag = if point == PublicationFaultPoint::ExistingRegistryCommitted {
                "New#1011"
            } else {
                "Old#1011"
            };
            assert_eq!(registry.accounts[0].battle_tag, expected_tag);
            validate_formal_account(&shared, &account_id, expected_tag).unwrap();
            let credentials: DashenCredentials = serde_json::from_slice(
                &fs::read(shared.account(&account_id).credentials()).unwrap(),
            )
            .unwrap();
            let expected_token = if point == PublicationFaultPoint::ExistingRegistryCommitted {
                "new-secret"
            } else {
                "old-secret"
            };
            assert_eq!(credentials.token, expected_token);
            assert!(!shared.pending_logins().exists());
        }
    }

    #[test]
    fn unchanged_battle_tag_relogin_crashes_use_registry_snapshots_not_summary_fields() {
        for point in [
            PublicationFaultPoint::ExistingCredentialsPublished,
            PublicationFaultPoint::ExistingManifestPublished,
            PublicationFaultPoint::ExistingRegistryCommitted,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
            let manager = AccountContextManager::load(shared.clone()).unwrap();
            let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
            install_existing(&manager, &account_id);
            assert!(publish_relogin_account_inner_with_fault(
                &manager,
                &account_id,
                capture("Old#1011", "new-secret"),
                &CrashAfter(point),
            )
            .is_err());
            let operation = fs::read_dir(shared.pending_logins())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert!(operation.join("old-registry.json").is_file());
            assert!(operation.join("intended-registry.json").is_file());
            drop(manager);

            let restarted = AccountContextManager::load(shared.clone()).unwrap();
            let registry = restarted.registry_snapshot().unwrap();
            assert_eq!(registry.accounts[0].battle_tag, "Old#1011");
            let credentials: DashenCredentials = serde_json::from_slice(
                &fs::read(shared.account(&account_id).credentials()).unwrap(),
            )
            .unwrap();
            let expected_token = if point == PublicationFaultPoint::ExistingRegistryCommitted {
                "new-secret"
            } else {
                "old-secret"
            };
            assert_eq!(credentials.token, expected_token);
            assert!(!shared.pending_logins().exists());
        }
    }

    #[test]
    fn interrupted_existing_rollback_retries_from_old_registry_fact() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_existing(&manager, &account_id);
        assert!(publish_relogin_account_inner_with_fault(
            &manager,
            &account_id,
            capture("New#1011", "new-secret"),
            &CrashAfter(PublicationFaultPoint::ExistingManifestPublished),
        )
        .is_err());
        drop(manager);

        assert!(recover_pending_logins_with(&shared, &FailAfterCredentialRollback).is_err());
        assert_eq!(
            serde_json::from_slice::<DashenCredentials>(
                &fs::read(shared.account(&account_id).credentials()).unwrap()
            )
            .unwrap()
            .token,
            "old-secret"
        );
        assert_eq!(
            read_manifest(&shared.account(&account_id).manifest(), &account_id)
                .unwrap()
                .battle_tag,
            "New#1011"
        );
        assert!(shared.pending_logins().is_dir());

        let restarted = AccountContextManager::load(shared.clone()).unwrap();
        assert_eq!(
            restarted.registry_snapshot().unwrap().accounts[0].battle_tag,
            "Old#1011"
        );
        assert_eq!(
            read_manifest(&shared.account(&account_id).manifest(), &account_id)
                .unwrap()
                .battle_tag,
            "Old#1011"
        );
        assert_eq!(
            serde_json::from_slice::<DashenCredentials>(
                &fs::read(shared.account(&account_id).credentials()).unwrap()
            )
            .unwrap()
            .token,
            "old-secret"
        );
        assert!(!shared.pending_logins().exists());
    }

    #[test]
    fn intended_registry_requires_exact_staged_manifest_and_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_existing(&manager, &account_id);
        assert!(publish_relogin_account_inner_with_fault(
            &manager,
            &account_id,
            capture("Old#1011", "new-secret"),
            &CrashAfter(PublicationFaultPoint::ExistingRegistryCommitted),
        )
        .is_err());
        drop(manager);
        save_credentials_atomic(
            &shared.account(&account_id).credentials(),
            &capture("Old#1011", "tampered-secret").credentials,
        )
        .unwrap();

        let error = AccountContextManager::load(shared.clone()).err().unwrap();

        assert_eq!(error.to_string(), "账号登录恢复失败，请重试。");
        assert!(shared.pending_logins().is_dir());
        let rendered = format!("{error:#}").to_ascii_lowercase();
        assert!(!rendered.contains("tampered-secret"));
        assert!(!rendered.contains("new-secret"));
    }

    #[test]
    fn failed_recovery_delete_keeps_journal_and_next_restart_retries() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        assert!(publish_add_account_inner_with_fault(
            &manager,
            capture("New#1011", "new-secret"),
            &CrashAfter(PublicationFaultPoint::NewAccountPublished),
        )
        .is_err());
        drop(manager);

        assert!(recover_pending_logins_with(&shared, &FailPublishedDelete).is_err());
        assert!(shared.pending_logins().is_dir());
        assert_eq!(fs::read_dir(shared.accounts_root()).unwrap().count(), 1);

        let restarted = AccountContextManager::load(shared.clone()).unwrap();
        assert!(restarted.registry_snapshot().unwrap().accounts.is_empty());
        assert_eq!(fs::read_dir(shared.accounts_root()).unwrap().count(), 0);
        assert!(!shared.pending_logins().exists());
    }

    #[test]
    fn committed_cleanup_failure_returns_success_and_restart_cleans() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let result = publish_add_account_inner_with_fault(
            &manager,
            capture("New#1011", "new-secret"),
            &CrashAfter(PublicationFaultPoint::CleanupPending),
        )
        .unwrap();
        assert_eq!(result.message, pending_cleanup_message());
        assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
        assert!(shared.pending_logins().is_dir());
        drop(manager);

        let restarted = AccountContextManager::load(shared.clone()).unwrap();
        assert_eq!(restarted.registry_snapshot().unwrap().accounts.len(), 1);
        assert!(!shared.pending_logins().exists());
    }

    #[test]
    fn remove_recovers_committed_relogin_cleanup_before_deleting_account() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_existing(&manager, &account_id);
        let result = publish_relogin_account_inner_with_fault(
            &manager,
            &account_id,
            capture("Old#1011", "new-secret"),
            &CrashAfter(PublicationFaultPoint::CleanupPending),
        )
        .unwrap();
        assert_eq!(result.message, pending_cleanup_message());
        assert!(shared.pending_logins().is_dir());

        let removed = manager
            .remove_account(&account_id, "2026-07-16T13:00:00Z")
            .unwrap();
        assert!(removed.removed);
        assert!(!shared.pending_logins().exists());
        drop(manager);

        let restarted = AccountContextManager::load(shared).unwrap();
        assert!(restarted.registry_snapshot().unwrap().accounts.is_empty());
    }

    #[test]
    fn restart_clears_stale_login_only_when_pending_deletion_proves_removal() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_existing(&manager, &account_id);
        publish_relogin_account_inner_with_fault(
            &manager,
            &account_id,
            capture("Old#1011", "new-secret"),
            &CrashAfter(PublicationFaultPoint::CleanupPending),
        )
        .unwrap();
        let committed_registry = manager.registry_snapshot().unwrap();
        let removal = crate::account_removal::remove_account_with_test_fault(
            &shared,
            &committed_registry,
            &account_id,
            "2026-07-16T13:00:00Z",
            crate::account_removal::TestRemovalFault::RollbackRestoreCommitsRemoval,
        )
        .unwrap();
        assert!(removal.registry.accounts.is_empty());
        assert!(shared.pending_logins().is_dir());
        assert!(shared.deletion_journal().is_file());
        drop(manager);

        let restarted = AccountContextManager::load(shared.clone()).unwrap();
        assert!(restarted.registry_snapshot().unwrap().accounts.is_empty());
        assert!(!shared.pending_logins().exists());
        assert!(!shared.deletion_journal().exists());
    }

    #[tokio::test]
    async fn capture_wrapper_recovers_before_starting_browser_work() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        fs::create_dir_all(shared.pending_logins()).unwrap();
        fs::create_dir_all(
            shared
                .pending_logins()
                .join("11111111-1111-4111-8111-111111111111"),
        )
        .unwrap();
        fs::write(
            shared
                .pending_logins()
                .join("11111111-1111-4111-8111-111111111111")
                .join(JOURNAL_FILE),
            b"{}",
        )
        .unwrap();
        let capture_started = std::sync::atomic::AtomicBool::new(false);

        assert!(capture_and_publish_add(
            &manager,
            &RecordingProgress::default(),
            || async {
                capture_started.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(capture("Never#1000", "never-secret"))
            },
            || false,
        )
        .await
        .is_err());
        assert!(!capture_started.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn mismatch_cleanup_failure_returns_safe_result_and_restart_cleans() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_existing(&manager, &account_id);
        let mut other_capture = capture("Other#2022", "other-secret");
        other_capture.credentials.role_id = 202202;
        let result = publish_relogin_account_inner_with_fault(
            &manager,
            &account_id,
            other_capture,
            &CrashAfter(PublicationFaultPoint::CleanupPending),
        )
        .unwrap();
        assert_eq!(result.kind, "differentNewAccount");
        assert_eq!(result.message, pending_cleanup_message());
        assert!(shared.pending_logins().is_dir());
        drop(manager);

        let restarted = AccountContextManager::load(shared.clone()).unwrap();
        assert_eq!(
            restarted.registry_snapshot().unwrap().accounts[0].battle_tag,
            "Old#1011"
        );
        assert!(!shared.pending_logins().exists());
    }

    #[test]
    fn login_operation_journal_is_safe_camel_case_and_contains_no_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        assert!(publish_add_account_inner_with_fault(
            &manager,
            capture("New#1011", "synthetic-secret"),
            &CrashAfter(PublicationFaultPoint::NewAccountPublished),
        )
        .is_err());
        let operation = fs::read_dir(shared.pending_logins())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let journal = fs::read_to_string(operation.join(JOURNAL_FILE)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&journal).unwrap();
        let object = value.as_object().unwrap();
        for key in [
            "operationId",
            "kind",
            "accountId",
            "phase",
            "battleTag",
            "startedAt",
            "updatedAt",
        ] {
            assert!(object.contains_key(key));
        }
        let lower = journal.to_ascii_lowercase();
        for forbidden in [
            "synthetic-secret",
            "roleid",
            "role_id",
            "token",
            "customertoken",
            "cookie",
            "authorization",
            "credential",
        ] {
            assert!(!lower.contains(forbidden));
        }
    }

    #[test]
    fn add_new_rejects_existing_publication_target_without_overwriting_it() {
        let temp = tempfile::tempdir().unwrap();
        let shared = crate::app_paths::SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let mut pending = PendingLogin::create(&manager).unwrap();
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
            &mut pending,
            manager.registry_snapshot().unwrap(),
            account_id,
            capture,
            &NoPublicationFault,
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
