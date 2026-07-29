use owstats_desktop_lib::account_context::AccountContextManager;
use owstats_desktop_lib::account_login::{
    publish_add_account, publish_relogin_account, LoginCapture,
};
use owstats_desktop_lib::account_migration::{
    LegacyMigrationState, MigrationJournal, MigrationPhase,
};
use owstats_desktop_lib::account_store::{
    read_manifest, read_registry, write_manifest_atomic, write_registry_atomic, AccountId,
    AccountManifest, AccountRegistry, AccountRegistryEntry,
};
use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::credential_store::DashenCredentials;
use owstats_desktop_lib::{
    list_matches, AccountManagerStatus, CredentialStatus, MatchFilters, PlayerProfile,
    RemoveAccountResult,
};
use rusqlite::params;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

fn synthetic_capture(role_id: i64, battle_tag: &str, token: &str) -> LoginCapture {
    LoginCapture {
        credentials: DashenCredentials {
            role_id,
            token: token.to_string(),
            customer_token: Some(format!("customer-{role_id}")),
            dts: Some("synthetic-dts".to_string()),
            server: 1,
            saved_at: "2026-07-16T00:00:00Z".to_string(),
        },
        battle_tag: battle_tag.to_string(),
    }
}

fn saved_credentials(shared: &SharedAppPaths, account_id: &AccountId) -> DashenCredentials {
    serde_json::from_slice(&fs::read(shared.account(account_id).credentials()).unwrap()).unwrap()
}

fn install_account(
    manager: &AccountContextManager,
    account_id: AccountId,
    role_id: i64,
    battle_tag: &str,
    token: &str,
    active: bool,
) {
    let paths = manager.shared_paths().account(&account_id);
    fs::create_dir_all(paths.root()).unwrap();
    let mut account_manifest = manifest(account_id.clone(), battle_tag);
    account_manifest.role_id = role_id;
    write_manifest_atomic(&paths.manifest(), &account_manifest).unwrap();
    owstats_desktop_lib::credential_store::save_credentials_atomic(
        &paths.credentials(),
        &synthetic_capture(role_id, battle_tag, token).credentials,
    )
    .unwrap();
    let mut registry = manager.registry_snapshot().unwrap();
    registry
        .accounts
        .push(entry(account_id.clone(), battle_tag));
    if active {
        registry.active_account_id = Some(account_id);
    }
    manager.replace_registry(registry).unwrap();
}

#[test]
fn add_account_creates_new_or_updates_existing_role_without_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();

    let added = publish_add_account(
        &manager,
        synthetic_capture(101101, "First#1011", "synthetic-first-secret"),
    )
    .unwrap();
    assert_eq!(added.kind, "newAccountAdded");
    let account_id = AccountId::parse(added.account_id.as_deref().unwrap()).unwrap();
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
    assert_eq!(
        manager.registry_snapshot().unwrap().active_account_id,
        Some(account_id.clone())
    );

    let updated = publish_add_account(
        &manager,
        synthetic_capture(101101, "Renamed#1011", "synthetic-renamed-secret"),
    )
    .unwrap();
    assert_eq!(updated.kind, "existingAccountUpdated");
    assert_eq!(updated.message, "账号已更新。");
    assert_eq!(updated.account_id.as_deref(), Some(account_id.as_str()));
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
    assert_eq!(
        read_manifest(&shared.account(&account_id).manifest(), &account_id)
            .unwrap()
            .battle_tag,
        "Renamed#1011"
    );
    assert_eq!(
        saved_credentials(&shared, &account_id).token,
        "synthetic-renamed-secret"
    );
    assert!(!shared.pending_logins().exists());
}

#[test]
fn relogin_with_another_role_never_overwrites_selected_account() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let selected = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    install_account(
        &manager,
        selected.clone(),
        101101,
        "First#1011",
        "synthetic-old-secret",
        true,
    );

    let result = publish_relogin_account(
        &manager,
        &selected,
        synthetic_capture(202202, "Other#2022", "synthetic-other-secret"),
    )
    .unwrap();
    assert_eq!(result.kind, "differentNewAccount");
    assert_eq!(result.account_id, None);
    assert!(!result.should_offer_switch);
    assert_eq!(
        saved_credentials(&shared, &selected).token,
        "synthetic-old-secret"
    );
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
    assert!(!shared.pending_logins().exists());
}

#[test]
fn relogin_mismatch_existing_returns_safe_switch_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let selected = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let other = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    install_account(
        &manager,
        selected.clone(),
        101101,
        "First#1011",
        "old-one",
        true,
    );
    install_account(
        &manager,
        other.clone(),
        202202,
        "Other#2022",
        "old-two",
        false,
    );
    let before = manager.registry_snapshot().unwrap();

    let result = publish_relogin_account(
        &manager,
        &selected,
        synthetic_capture(202202, "OtherRenamed#2022", "new-two"),
    )
    .unwrap();
    assert_eq!(result.kind, "differentExistingAccount");
    assert_eq!(result.account_id.as_deref(), Some(other.as_str()));
    assert_eq!(result.battle_tag.as_deref(), Some("Other#2022"));
    assert!(result.should_offer_switch);
    assert_eq!(manager.registry_snapshot().unwrap(), before);
    assert_eq!(saved_credentials(&shared, &selected).token, "old-one");
    assert_eq!(saved_credentials(&shared, &other).token, "old-two");
}

#[test]
fn relogin_same_role_atomically_updates_selected_account() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let selected = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    install_account(
        &manager,
        selected.clone(),
        101101,
        "First#1011",
        "synthetic-old-secret",
        true,
    );

    let result = publish_relogin_account(
        &manager,
        &selected,
        synthetic_capture(101101, "Renamed#1011", "synthetic-new-secret"),
    )
    .unwrap();
    assert_eq!(result.kind, "existingAccountUpdated");
    assert_eq!(result.account_id.as_deref(), Some(selected.as_str()));
    assert_eq!(
        saved_credentials(&shared, &selected).token,
        "synthetic-new-secret"
    );
    assert_eq!(
        read_manifest(&shared.account(&selected).manifest(), &selected)
            .unwrap()
            .battle_tag,
        "Renamed#1011"
    );
    assert_eq!(
        manager.registry_snapshot().unwrap().accounts[0].battle_tag,
        "Renamed#1011"
    );
}

#[test]
fn add_account_keeps_published_login_when_eligible_legacy_migration_fails() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    fs::write(
        shared.legacy_credentials(),
        r#"{"roleId":909090,"token":"synthetic-legacy-secret","server":1,"savedAt":"2026-01-01T00:00:00Z"}"#,
    )
    .unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::WaitingForAccount
    );

    let result = publish_add_account(
        &manager,
        synthetic_capture(101101, "First#1011", "synthetic-current-secret"),
    )
    .unwrap();
    let account_id = AccountId::parse(result.account_id.as_deref().unwrap()).unwrap();
    assert_eq!(result.kind, "newAccountAdded");
    assert!(result.message.contains("待恢复"));
    assert_eq!(
        saved_credentials(&shared, &account_id).token,
        "synthetic-current-secret"
    );
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
}

#[test]
fn add_account_allows_duplicate_battle_tag_for_different_roles_and_serializes_safely() {
    let temp = tempfile::tempdir().unwrap();
    let manager =
        AccountContextManager::load(SharedAppPaths::from_app_data_dir(temp.path())).unwrap();
    publish_add_account(
        &manager,
        synthetic_capture(101101, "Same#1000", "first-secret"),
    )
    .unwrap();
    let result = publish_add_account(
        &manager,
        synthetic_capture(202202, "Same#1000", "second-secret"),
    )
    .unwrap();
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 2);
    let json = serde_json::to_string(&result).unwrap().to_ascii_lowercase();
    for forbidden in [
        "roleid",
        "role_id",
        "token",
        "customertoken",
        "cookie",
        "authorization",
        "credential",
        temp.path().to_string_lossy().as_ref(),
    ] {
        assert!(
            !json.contains(forbidden),
            "result exposed forbidden content"
        );
    }
}

#[test]
fn invalid_capture_does_not_publish_and_cleans_pending() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    for capture in [
        synthetic_capture(0, "Valid#1000", "secret"),
        synthetic_capture(101101, "Valid#1000", " "),
        synthetic_capture(101101, "unstable", "secret"),
        synthetic_capture(101101, "#1000", "secret"),
        synthetic_capture(101101, "Invalid#suffix", "secret"),
    ] {
        let error = publish_add_account(&manager, capture)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("secret"));
        assert!(manager.registry_snapshot().unwrap().accounts.is_empty());
        assert!(!shared.pending_logins().exists());
    }
}

#[test]
fn incomplete_stored_manifest_never_allows_duplicate_publication() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    install_account(
        &manager,
        account_id.clone(),
        101101,
        "First#1011",
        "synthetic-old-secret",
        true,
    );
    let mut incomplete = manifest(account_id.clone(), "First#1011");
    incomplete.role_id = 0;
    write_manifest_atomic(&shared.account(&account_id).manifest(), &incomplete).unwrap();

    assert!(publish_add_account(
        &manager,
        synthetic_capture(101101, "Renamed#1011", "synthetic-new-secret")
    )
    .is_err());
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
    assert_eq!(
        saved_credentials(&shared, &account_id).token,
        "synthetic-old-secret"
    );
}

#[test]
fn add_account_never_publishes_through_linked_accounts_root() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    if !create_directory_symlink(outside.path(), &shared.accounts_root()) {
        return;
    }

    assert!(publish_add_account(
        &manager,
        synthetic_capture(101101, "First#1011", "synthetic-secret")
    )
    .is_err());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(manager.registry_snapshot().unwrap().accounts.is_empty());
}

#[test]
fn login_recovery_never_follows_a_linked_operations_parent() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let operation_id = "11111111-1111-4111-8111-111111111111";
    let external_operation = outside.path().join("pending-logins").join(operation_id);
    fs::create_dir_all(&external_operation).unwrap();
    let sentinel = external_operation.join("sentinel");
    fs::write(&sentinel, b"external-must-stay-unchanged").unwrap();
    if !create_directory_symlink(outside.path(), &shared.operations_root()) {
        return;
    }

    let error = AccountContextManager::load(shared).err().unwrap();

    assert_eq!(error.to_string(), "账号登录恢复失败，请重试。");
    assert_eq!(fs::read(sentinel).unwrap(), b"external-must-stay-unchanged");
}

#[test]
fn manager_load_rejects_a_linked_shared_account_storage_ancestor() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let linked_root = temp.path().join("linked-app-data");
    let sentinel = outside.path().join("sentinel");
    fs::write(&sentinel, b"external-must-stay-unchanged").unwrap();
    if !create_directory_symlink(outside.path(), &linked_root) {
        return;
    }

    let error = AccountContextManager::load(SharedAppPaths::from_app_data_dir(&linked_root))
        .err()
        .unwrap();

    assert_eq!(error.to_string(), "账号登录恢复失败，请重试。");
    assert_eq!(fs::read(sentinel).unwrap(), b"external-must-stay-unchanged");
}

#[test]
fn relogin_rejects_linked_manifest_and_credentials_without_touching_external_files() {
    for linked_name in ["account.json", "credentials.json"] {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
        install_account(
            &manager,
            account_id.clone(),
            101101,
            "Old#1011",
            "old-secret",
            true,
        );
        let external_account = outside.path().join(account_id.as_str());
        fs::create_dir_all(&external_account).unwrap();
        let external = external_account.join(linked_name);
        fs::copy(
            shared.account(&account_id).root().join(linked_name),
            &external,
        )
        .unwrap();
        let expected = fs::read(&external).unwrap();
        fs::remove_file(shared.account(&account_id).root().join(linked_name)).unwrap();
        if !create_file_symlink(
            &external,
            &shared.account(&account_id).root().join(linked_name),
        ) {
            return;
        }

        assert!(publish_relogin_account(
            &manager,
            &account_id,
            synthetic_capture(101101, "Old#1011", "new-secret")
        )
        .is_err());
        assert_eq!(fs::read(&external).unwrap(), expected);
    }
}

#[test]
fn orphan_recovery_quarantines_then_rejects_nested_reparse_without_external_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    write_formal_account(&shared, &account_id, 101101, "Crash#1011", "new-secret");
    let sentinel = outside.path().join("sentinel");
    fs::write(&sentinel, b"external-must-stay-unchanged").unwrap();
    if !create_directory_symlink(
        outside.path(),
        &shared.account(&account_id).root().join("nested-link"),
    ) {
        return;
    }
    write_login_operation_journal(
        &shared,
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "addNew",
        &account_id,
        "accountPublished",
        "Crash#1011",
    );

    let error = AccountContextManager::load(shared.clone()).err().unwrap();

    assert_eq!(error.to_string(), "账号登录恢复失败，请重试。");
    assert_eq!(fs::read(sentinel).unwrap(), b"external-must-stay-unchanged");
    assert!(shared.pending_logins().is_dir());
}

#[test]
fn add_account_registry_write_failure_rolls_back_existing_account() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    install_account(
        &manager,
        account_id.clone(),
        101101,
        "First#1011",
        "old-secret",
        true,
    );
    let old_manifest = fs::read(shared.account(&account_id).manifest()).unwrap();
    fs::remove_file(shared.registry()).unwrap();
    fs::create_dir(shared.registry()).unwrap();

    assert!(publish_add_account(
        &manager,
        synthetic_capture(101101, "Renamed#1011", "new-secret")
    )
    .is_err());
    assert_eq!(saved_credentials(&shared, &account_id).token, "old-secret");
    assert_eq!(
        fs::read(shared.account(&account_id).manifest()).unwrap(),
        old_manifest
    );
    assert!(!shared.pending_logins().exists());
}

#[test]
fn concurrent_add_same_role_never_creates_duplicate_accounts() {
    let temp = tempfile::tempdir().unwrap();
    let manager = Arc::new(
        AccountContextManager::load(SharedAppPaths::from_app_data_dir(temp.path())).unwrap(),
    );
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for suffix in ["One#1000", "Two#1000"] {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            publish_add_account(
                &manager,
                synthetic_capture(101101, suffix, "synthetic-secret"),
            )
            .unwrap()
        }));
    }
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
}

#[test]
fn account_login_commands_acquire_account_operation_before_capture_and_are_registered() {
    let commands = include_str!("../src/commands.rs");
    for command in ["add_account"] {
        let function = command_source(commands, command);
        let permit = function.find("try_acquire_account_operation()").unwrap();
        let capture = function.find("capture_dashen_login(").unwrap();
        assert!(
            permit < capture,
            "{command} captured before acquiring permit"
        );
    }
    let lib = include_str!("../src/lib.rs");
    assert!(lib.contains("commands::add_account"));
    assert!(!lib.contains("commands::relogin_account"));
}

fn login_operation_dir(shared: &SharedAppPaths, operation_id: &str) -> std::path::PathBuf {
    shared.pending_logins().join(operation_id)
}

fn write_login_operation_journal(
    shared: &SharedAppPaths,
    operation_id: &str,
    kind: &str,
    account_id: &AccountId,
    phase: &str,
    battle_tag: &str,
) -> std::path::PathBuf {
    let operation = login_operation_dir(shared, operation_id);
    fs::create_dir_all(&operation).unwrap();
    fs::write(
        operation.join("operation.json"),
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "operationId": operation_id,
            "kind": kind,
            "accountId": account_id.as_str(),
            "phase": phase,
            "battleTag": battle_tag,
            "startedAt": "2026-07-16T00:00:00Z",
            "updatedAt": "2026-07-16T00:00:01Z"
        }))
        .unwrap(),
    )
    .unwrap();
    operation
}

fn write_formal_account(
    shared: &SharedAppPaths,
    account_id: &AccountId,
    role_id: i64,
    battle_tag: &str,
    token: &str,
) {
    let paths = shared.account(account_id);
    fs::create_dir_all(paths.root()).unwrap();
    let mut value = manifest(account_id.clone(), battle_tag);
    value.role_id = role_id;
    write_manifest_atomic(&paths.manifest(), &value).unwrap();
    owstats_desktop_lib::credential_store::save_credentials_atomic(
        &paths.credentials(),
        &synthetic_capture(role_id, battle_tag, token).credentials,
    )
    .unwrap();
}

#[test]
fn interrupted_new_login_after_account_rename_is_rolled_back_on_restart() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    write_formal_account(&shared, &account_id, 101101, "Crash#1011", "new-secret");
    write_login_operation_journal(
        &shared,
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        "addNew",
        &account_id,
        "accountPublished",
        "Crash#1011",
    );

    let manager = AccountContextManager::load(shared.clone()).unwrap();
    assert!(manager.registry_snapshot().unwrap().accounts.is_empty());
    assert!(!shared.account(&account_id).root().exists());
    assert!(!shared.pending_logins().exists());
}

#[test]
fn interrupted_new_login_after_registry_commit_is_kept_and_cleaned_on_restart() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    write_formal_account(&shared, &account_id, 101101, "Crash#1011", "new-secret");
    write_registry_atomic(
        &shared.registry(),
        &AccountRegistry {
            version: 1,
            active_account_id: Some(account_id.clone()),
            accounts: vec![entry(account_id.clone(), "Crash#1011")],
        },
    )
    .unwrap();
    write_login_operation_journal(
        &shared,
        "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        "addNew",
        &account_id,
        "accountPublished",
        "Crash#1011",
    );

    let manager = AccountContextManager::load(shared.clone()).unwrap();
    assert_eq!(manager.registry_snapshot().unwrap().accounts.len(), 1);
    assert!(shared.account(&account_id).root().is_dir());
    assert!(!shared.pending_logins().exists());
}

fn interrupted_existing_login_fixture(
    phase: &str,
    registry_is_new: bool,
) -> (
    tempfile::TempDir,
    SharedAppPaths,
    AccountId,
    std::path::PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    write_formal_account(&shared, &account_id, 101101, "Old#1011", "old-secret");
    let old_manifest = fs::read(shared.account(&account_id).manifest()).unwrap();
    let old_credentials = fs::read(shared.account(&account_id).credentials()).unwrap();
    let old_registry = AccountRegistry {
        version: 1,
        active_account_id: Some(account_id.clone()),
        accounts: vec![entry(account_id.clone(), "Old#1011")],
    };
    write_registry_atomic(&shared.registry(), &old_registry).unwrap();
    let old_registry_bytes = fs::read(shared.registry()).unwrap();
    let operation = write_login_operation_journal(
        &shared,
        "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
        "updateExisting",
        &account_id,
        phase,
        "New#1011",
    );
    fs::write(operation.join("old-manifest.json"), old_manifest).unwrap();
    fs::write(operation.join("old-credentials.json"), old_credentials).unwrap();
    fs::write(operation.join("old-registry.json"), old_registry_bytes).unwrap();
    let intended_registry = AccountRegistry {
        version: 1,
        active_account_id: Some(account_id.clone()),
        accounts: vec![entry(account_id.clone(), "New#1011")],
    };
    write_registry_atomic(
        &operation.join("intended-registry.json"),
        &intended_registry,
    )
    .unwrap();
    let staged = operation.join("account").join(account_id.as_str());
    fs::create_dir_all(&staged).unwrap();
    let mut new_manifest = manifest(account_id.clone(), "New#1011");
    new_manifest.role_id = 101101;
    write_manifest_atomic(&staged.join("account.json"), &new_manifest).unwrap();
    owstats_desktop_lib::credential_store::save_credentials_atomic(
        &staged.join("credentials.json"),
        &synthetic_capture(101101, "New#1011", "new-secret").credentials,
    )
    .unwrap();
    if matches!(
        phase,
        "credentialsPublished" | "manifestPublished" | "registryCommitted"
    ) {
        owstats_desktop_lib::credential_store::save_credentials_atomic(
            &shared.account(&account_id).credentials(),
            &synthetic_capture(101101, "New#1011", "new-secret").credentials,
        )
        .unwrap();
    }
    if matches!(phase, "manifestPublished" | "registryCommitted") {
        write_manifest_atomic(&shared.account(&account_id).manifest(), &new_manifest).unwrap();
    }
    if registry_is_new {
        write_registry_atomic(&shared.registry(), &intended_registry).unwrap();
    }
    (temp, shared, account_id, operation)
}

#[test]
fn interrupted_existing_login_after_credentials_write_restores_old_state() {
    let (_temp, shared, account_id, _) =
        interrupted_existing_login_fixture("credentialsPublished", false);
    AccountContextManager::load(shared.clone()).unwrap();
    assert_eq!(saved_credentials(&shared, &account_id).token, "old-secret");
    assert_eq!(
        read_manifest(&shared.account(&account_id).manifest(), &account_id)
            .unwrap()
            .battle_tag,
        "Old#1011"
    );
    assert!(!shared.pending_logins().exists());
}

#[test]
fn interrupted_existing_login_after_manifest_write_restores_old_state() {
    let (_temp, shared, account_id, _) =
        interrupted_existing_login_fixture("manifestPublished", false);
    AccountContextManager::load(shared.clone()).unwrap();
    assert_eq!(saved_credentials(&shared, &account_id).token, "old-secret");
    assert_eq!(
        read_manifest(&shared.account(&account_id).manifest(), &account_id)
            .unwrap()
            .battle_tag,
        "Old#1011"
    );
    assert_eq!(
        read_registry(&shared.registry()).unwrap().accounts[0].battle_tag,
        "Old#1011"
    );
    assert!(!shared.pending_logins().exists());
}

#[test]
fn interrupted_existing_login_after_registry_write_keeps_new_state() {
    let (_temp, shared, account_id, _) =
        interrupted_existing_login_fixture("manifestPublished", true);
    AccountContextManager::load(shared.clone()).unwrap();
    assert_eq!(saved_credentials(&shared, &account_id).token, "new-secret");
    assert_eq!(
        read_manifest(&shared.account(&account_id).manifest(), &account_id)
            .unwrap()
            .battle_tag,
        "New#1011"
    );
    assert_eq!(
        read_registry(&shared.registry()).unwrap().accounts[0].battle_tag,
        "New#1011"
    );
    assert!(!shared.pending_logins().exists());
}

struct LegacyFixture {
    _temp: tempfile::TempDir,
    shared: SharedAppPaths,
    manager: AccountContextManager,
}

impl LegacyFixture {
    fn with_database_match(match_id: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        let legacy = owstats_desktop_lib::OwDatabase::new(shared.legacy_database());
        legacy.initialize().unwrap();
        legacy
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, 22, 'preset', 'SportPreset', 'normal', 'win', 'map', 'hero', 100, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, '{}', 'now')",
                params![match_id],
            )
            .unwrap();
        fs::write(
            shared.legacy_credentials(),
            r#"{"role_id":10001,"token":"synthetic-legacy-token","server":1,"saved_at":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        Self {
            _temp: temp,
            shared,
            manager,
        }
    }

    fn without_database() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let shared = SharedAppPaths::from_app_data_dir(temp.path());
        fs::write(shared.legacy_player_profile(), b"profile-only").unwrap();
        fs::write(
            shared.legacy_credentials(),
            r#"{"role_id":10001,"token":"synthetic-legacy-token","server":1,"saved_at":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        let manager = AccountContextManager::load(shared.clone()).unwrap();
        Self {
            _temp: temp,
            shared,
            manager,
        }
    }

    fn confirm_account(&self) -> owstats_desktop_lib::account_context::AccountContextSnapshot {
        let account_id = AccountId::parse("36be8ecb-ecc4-4a15-b4d5-7d1fa355ab9c").unwrap();
        let paths = self.shared.account(&account_id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(
            &paths.manifest(),
            &manifest(account_id.clone(), "Tester#5479"),
        )
        .unwrap();
        fs::write(
            paths.credentials(),
            r#"{"roleId":10001,"token":"synthetic-account-token","server":1,"savedAt":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        self.manager
            .replace_registry(AccountRegistry {
                version: 1,
                active_account_id: Some(account_id.clone()),
                accounts: vec![entry(account_id, "Tester#5479")],
            })
            .unwrap();
        self.manager.active_snapshot().unwrap()
    }

    fn match_ids(
        &self,
        account: &owstats_desktop_lib::account_context::AccountContextSnapshot,
    ) -> Vec<String> {
        let connection = account.database().connect().unwrap();
        let mut statement = connection
            .prepare("SELECT match_id FROM matches ORDER BY match_id")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn write_journal(&self, account: &AccountId, phase: MigrationPhase) {
        if matches!(
            phase,
            MigrationPhase::Publishing | MigrationPhase::CleaningLegacy
        ) {
            let staging = self.staging(account);
            fs::create_dir_all(&staging).unwrap();
            let database = if staging.join("owstats.sqlite3").is_file() {
                Some(fs::read(staging.join("owstats.sqlite3")).unwrap())
            } else if self.shared.account(account).database().is_file() {
                Some(fs::read(self.shared.account(account).database()).unwrap())
            } else {
                None
            };
            if let Some(database) = database {
                write_database_manifest(&self.shared, account, &database);
            } else {
                fs::write(
                    self.shared.migration_content_manifest(account),
                    r#"{"version":1,"databaseRequired":false,"entries":[]}"#,
                )
                .unwrap();
            }
        }
        fs::create_dir_all(self.shared.operations_root()).unwrap();
        fs::write(
            self.shared.migration_journal(),
            serde_json::to_vec_pretty(&MigrationJournal {
                version: 1,
                account_id: account.clone(),
                phase,
                started_at: "2026-01-01T00:00:00Z".to_string(),
            })
            .unwrap(),
        )
        .unwrap();
    }

    fn staging(&self, account: &AccountId) -> std::path::PathBuf {
        self.shared
            .accounts_root()
            .join(format!(".migration-{}", account.as_str()))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_database_manifest(shared: &SharedAppPaths, account: &AccountId, database: &[u8]) {
    fs::create_dir_all(shared.operations_root()).unwrap();
    fs::write(
        shared.migration_content_manifest(account),
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "databaseRequired": true,
            "entries": [{
                "relativePath": "owstats.sqlite3",
                "kind": "file",
                "size": database.len(),
                "sha256": sha256_hex(database)
            }]
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn legacy_database_is_adopted_only_after_confirmed_account_identity() {
    let fixture = LegacyFixture::with_database_match("legacy-match");

    assert_eq!(
        fixture.manager.migration_state().unwrap(),
        LegacyMigrationState::WaitingForAccount
    );
    assert!(fixture.shared.legacy_database().is_file());

    let account = fixture.confirm_account();
    fixture.manager.adopt_legacy(&account).unwrap();

    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
    assert!(!fixture.shared.legacy_database().exists());
    assert_eq!(
        fixture.manager.migration_state().unwrap(),
        LegacyMigrationState::Complete
    );
}

#[test]
fn synthetic_legacy_upgrade_is_one_time_complete_and_preserves_shared_settings() {
    let fixture = LegacyFixture::with_database_match("legacy-once-match");
    fs::write(
        fixture.shared.legacy_player_target(),
        br#"{"battleTag":"Synthetic#1000"}"#,
    )
    .unwrap();
    fs::write(
        fixture.shared.legacy_player_profile(),
        br#"{"battleTag":"Synthetic#1000"}"#,
    )
    .unwrap();
    fs::create_dir_all(fixture.shared.legacy_seasons()).unwrap();
    fs::write(
        fixture.shared.legacy_seasons().join("22.json"),
        br#"{"season":22}"#,
    )
    .unwrap();
    fs::write(fixture.shared.user_settings(), br#"{"theme":"system"}"#).unwrap();
    let account = fixture.confirm_account();

    fixture.manager.adopt_legacy(&account).unwrap();
    assert_eq!(fixture.match_ids(&account), vec!["legacy-once-match"]);
    assert!(account.paths().credentials().is_file());
    assert!(account.paths().player_target().is_file());
    assert!(account.paths().player_profile().is_file());
    assert!(account.paths().seasons().join("22.json").is_file());
    for source in [
        fixture.shared.legacy_database(),
        fixture.shared.legacy_credentials(),
        fixture.shared.legacy_player_target(),
        fixture.shared.legacy_player_profile(),
        fixture.shared.legacy_seasons(),
    ] {
        assert!(!source.exists(), "validated legacy source was not removed");
    }
    let settings_before = fs::read(fixture.shared.user_settings()).unwrap();
    let database_before = fs::read(account.paths().database()).unwrap();
    let registry_before = fs::read(fixture.shared.registry()).unwrap();

    let restarted = AccountContextManager::load(fixture.shared.clone()).unwrap();
    let restarted_account = restarted.active_snapshot().unwrap();
    restarted.adopt_legacy(&restarted_account).unwrap();
    assert_eq!(
        fixture.match_ids(&restarted_account),
        vec!["legacy-once-match"]
    );
    assert_eq!(
        fs::read(restarted_account.paths().database()).unwrap(),
        database_before
    );
    assert_eq!(
        fs::read(fixture.shared.registry()).unwrap(),
        registry_before
    );
    assert_eq!(
        fs::read(fixture.shared.user_settings()).unwrap(),
        settings_before
    );

    let public_json = serde_json::to_string(&restarted.status().unwrap())
        .unwrap()
        .to_ascii_lowercase();
    for forbidden in ["roleid", "role_id", "token", "synthetic-legacy-token"] {
        assert!(!public_json.contains(forbidden));
    }
}

#[test]
fn legacy_files_without_database_are_published_and_cleaned_to_complete() {
    let fixture = LegacyFixture::without_database();
    let account = fixture.confirm_account();

    fixture.manager.adopt_legacy(&account).unwrap();

    assert_eq!(
        fs::read(account.paths().player_profile()).unwrap(),
        b"profile-only"
    );
    assert!(!account.paths().database().exists());
    assert!(!fixture.shared.legacy_player_profile().exists());
    assert!(!fixture.shared.legacy_credentials().exists());
    assert_eq!(
        fixture.manager.migration_state().unwrap(),
        LegacyMigrationState::Complete
    );
    let journal: MigrationJournal =
        serde_json::from_slice(&fs::read(fixture.shared.migration_journal()).unwrap()).unwrap();
    assert_eq!(journal.phase, MigrationPhase::Complete);
}

#[test]
fn failed_or_repeated_legacy_migration_never_loses_or_duplicates_data() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let valid_database = fs::read(fixture.shared.legacy_database()).unwrap();
    fs::write(fixture.shared.legacy_player_profile(), "synthetic-profile").unwrap();
    fs::create_dir_all(fixture.shared.legacy_seasons()).unwrap();
    fs::write(
        fixture.shared.legacy_seasons().join("season.json"),
        "synthetic-season",
    )
    .unwrap();
    fs::write(fixture.shared.legacy_database(), b"not-a-sqlite-database").unwrap();
    fs::write(fixture.shared.legacy_database_wal(), b"synthetic-wal").unwrap();
    fs::write(fixture.shared.legacy_database_shm(), b"synthetic-shm").unwrap();
    let account = fixture.confirm_account();

    assert!(fixture.manager.adopt_legacy(&account).is_err());
    assert!(fixture.shared.legacy_database().is_file());
    assert!(fixture.shared.legacy_credentials().is_file());
    assert!(fixture.shared.legacy_player_profile().is_file());
    assert!(fixture
        .shared
        .legacy_seasons()
        .join("season.json")
        .is_file());
    assert!(fixture.shared.legacy_database_wal().is_file());
    assert!(fixture.shared.legacy_database_shm().is_file());
    fs::remove_file(fixture.shared.legacy_database_wal()).unwrap();
    fs::remove_file(fixture.shared.legacy_database_shm()).unwrap();
    fs::write(fixture.shared.legacy_database(), valid_database).unwrap();

    fixture.manager.adopt_legacy(&account).unwrap();
    fixture.manager.adopt_legacy(&account).unwrap();
    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
}

#[test]
fn recovery_from_copying_and_validating_rolls_back_without_touching_legacy_sources() {
    for phase in [MigrationPhase::Copying, MigrationPhase::Validating] {
        let fixture = LegacyFixture::with_database_match("legacy-match");
        let account = fixture.confirm_account();
        let staging = fixture.staging(account.account_id());
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("partial"), "synthetic").unwrap();
        fixture.write_journal(account.account_id(), phase);

        AccountContextManager::load(fixture.shared.clone()).unwrap();

        assert!(fixture.shared.legacy_database().is_file());
        assert!(!staging.exists());
        assert!(!fixture.shared.migration_journal().exists());
    }
}

#[test]
fn recovery_from_publishing_finishes_publish_before_cleaning_legacy() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    let staging = fixture.staging(account.account_id());
    fs::create_dir_all(&staging).unwrap();
    fs::copy(
        fixture.shared.legacy_database(),
        staging.join("owstats.sqlite3"),
    )
    .unwrap();
    fixture.write_journal(account.account_id(), MigrationPhase::Publishing);

    AccountContextManager::load(fixture.shared.clone()).unwrap();

    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
    assert!(!fixture.shared.legacy_database().exists());
    assert_eq!(
        fixture.manager.migration_state().unwrap(),
        LegacyMigrationState::Complete
    );
}

#[test]
fn recovery_from_cleaning_and_complete_never_loses_verified_target() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    fs::copy(fixture.shared.legacy_database(), account.paths().database()).unwrap();
    fixture.write_journal(account.account_id(), MigrationPhase::CleaningLegacy);

    AccountContextManager::load(fixture.shared.clone()).unwrap();
    AccountContextManager::load(fixture.shared.clone()).unwrap();

    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
    assert!(!fixture.shared.legacy_database().exists());
    assert_eq!(
        fixture.manager.migration_state().unwrap(),
        LegacyMigrationState::Complete
    );
}

#[test]
fn mismatched_legacy_identity_is_rejected_without_exposing_identifiers_or_secrets() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    fs::write(
        fixture.shared.legacy_credentials(),
        r#"{"role_id":99999,"token":"synthetic-private-token"}"#,
    )
    .unwrap();
    let account = fixture.confirm_account();

    let error = fixture
        .manager
        .adopt_legacy(&account)
        .unwrap_err()
        .to_string();

    assert!(fixture.shared.legacy_database().is_file());
    assert!(!account.paths().database().exists());
    for forbidden in ["99999", "10001", "synthetic-private-token", "token"] {
        assert!(!error.to_ascii_lowercase().contains(forbidden));
    }
}

#[test]
fn incomplete_confirmed_account_identity_never_starts_legacy_migration() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    fs::remove_file(account.paths().credentials()).unwrap();

    let error = fixture
        .manager
        .adopt_legacy(&account)
        .unwrap_err()
        .to_string();

    assert!(fixture.shared.legacy_database().is_file());
    assert!(!fixture.shared.migration_journal().exists());
    assert!(!error.contains("10001"));
    assert!(!error.to_ascii_lowercase().contains("token"));
}

#[test]
fn shared_user_settings_and_browser_profile_are_never_migrated_or_removed() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    fs::write(fixture.shared.user_settings(), "synthetic-settings").unwrap();
    fs::write(fixture.shared.root().join("unknown-root-file"), "keep").unwrap();
    fs::create_dir_all(fixture.shared.browser_profile()).unwrap();
    fs::write(
        fixture.shared.browser_profile().join("synthetic-profile"),
        "profile",
    )
    .unwrap();
    fs::write(fixture.shared.legacy_player_target(), "target").unwrap();
    fs::write(fixture.shared.legacy_player_profile(), "profile").unwrap();
    for (directory, file_name) in [
        (fixture.shared.legacy_seasons(), "season.json"),
        (fixture.shared.legacy_diagnostics(), "diagnostic.json"),
        (fixture.shared.legacy_backups(), "backup.sqlite3"),
    ] {
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(file_name), "synthetic-owned-data").unwrap();
    }
    let account = fixture.confirm_account();

    fixture.manager.adopt_legacy(&account).unwrap();

    assert_eq!(
        fs::read_to_string(fixture.shared.user_settings()).unwrap(),
        "synthetic-settings"
    );
    assert!(fixture
        .shared
        .browser_profile()
        .join("synthetic-profile")
        .is_file());
    assert!(!account.paths().root().join("user_settings.json").exists());
    assert!(!account
        .paths()
        .root()
        .join("chrome-dashen-profile")
        .exists());
    assert!(fixture.shared.root().join("unknown-root-file").is_file());
    assert_eq!(
        fs::read_to_string(account.paths().player_target()).unwrap(),
        "target"
    );
    assert_eq!(
        fs::read_to_string(account.paths().player_profile()).unwrap(),
        "profile"
    );
    assert!(account.paths().seasons().join("season.json").is_file());
    assert!(account
        .paths()
        .diagnostics()
        .join("diagnostic.json")
        .is_file());
    assert!(account.paths().backups().join("backup.sqlite3").is_file());
}

#[test]
fn damaged_or_invalid_migration_journal_is_never_guessed() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    fs::create_dir_all(fixture.shared.operations_root()).unwrap();
    fs::write(fixture.shared.migration_journal(), r#"{"version":2}"#).unwrap();

    assert_eq!(
        owstats_desktop_lib::account_migration::detect_legacy_state(&fixture.shared).unwrap(),
        LegacyMigrationState::RecoveryError
    );
    let manager = AccountContextManager::load(fixture.shared.clone()).unwrap();
    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::RecoveryError
    );
    assert!(fixture.shared.legacy_database().is_file());
}

#[test]
fn manager_load_survives_recovery_error_and_retries_after_journal_is_repaired() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    fs::create_dir_all(fixture.shared.operations_root()).unwrap();
    fs::write(
        fixture.shared.migration_journal(),
        r#"{"version":99,"token":"synthetic-must-not-escape"}"#,
    )
    .unwrap();

    let manager = AccountContextManager::load(fixture.shared.clone()).unwrap();

    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::RecoveryError
    );
    assert!(fixture.shared.legacy_database().is_file());
    assert!(fixture.shared.migration_journal().is_file());

    fixture.write_journal(account.account_id(), MigrationPhase::Copying);
    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::WaitingForAccount
    );
    assert!(!fixture.shared.migration_journal().exists());
    assert!(fixture.shared.legacy_database().is_file());
}

#[test]
fn copying_and_validating_recovery_do_not_require_registry_manifest_or_credentials() {
    for phase in [MigrationPhase::Copying, MigrationPhase::Validating] {
        let fixture = LegacyFixture::with_database_match("legacy-match");
        let account = fixture.confirm_account();
        let account_id = account.account_id().clone();
        let account_paths = account.paths().clone();
        drop(account);
        let staging = fixture.staging(&account_id);
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("partial"), "synthetic").unwrap();
        fixture.write_journal(&account_id, phase);
        fs::remove_file(fixture.shared.registry()).unwrap();
        fs::remove_file(account_paths.manifest()).unwrap();
        fs::remove_file(account_paths.credentials()).unwrap();

        AccountContextManager::load(fixture.shared.clone()).unwrap();

        assert!(fixture.shared.legacy_database().is_file());
        assert!(!staging.exists());
        assert!(!fixture.shared.migration_journal().exists());
    }
}

#[test]
fn publishing_never_overwrites_empty_or_different_target_database() {
    for existing in [Vec::new(), b"different-target".to_vec()] {
        let fixture = LegacyFixture::with_database_match("legacy-match");
        let account = fixture.confirm_account();
        fs::write(account.paths().database(), &existing).unwrap();

        assert!(fixture.manager.adopt_legacy(&account).is_err());

        assert_eq!(fs::read(account.paths().database()).unwrap(), existing);
        assert!(fixture.shared.legacy_database().is_file());
        let journal: MigrationJournal =
            serde_json::from_slice(&fs::read(fixture.shared.migration_journal()).unwrap()).unwrap();
        assert_eq!(journal.phase, MigrationPhase::Publishing);

        assert_eq!(
            fixture.manager.migration_state().unwrap(),
            LegacyMigrationState::RecoveryError
        );
        assert_eq!(fs::read(account.paths().database()).unwrap(), existing);
        assert!(fixture.shared.legacy_database().is_file());

        fs::remove_file(account.paths().database()).unwrap();
        assert_eq!(
            fixture.manager.migration_state().unwrap(),
            LegacyMigrationState::Complete
        );
        assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
        assert!(!fixture.shared.legacy_database().exists());
    }
}

#[test]
fn linked_account_root_is_rejected_before_identity_or_publish_access() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account_id = AccountId::parse("cd5e9796-e686-4298-92f4-0b49f499a727").unwrap();
    let external_parent = tempfile::tempdir().unwrap();
    let external = external_parent.path().join(account_id.as_str());
    fs::create_dir_all(&external).unwrap();
    write_manifest_atomic(
        &external.join("account.json"),
        &manifest(account_id.clone(), "Linked#1000"),
    )
    .unwrap();
    fs::write(
        external.join("credentials.json"),
        r#"{"roleId":10001,"token":"synthetic-linked-token"}"#,
    )
    .unwrap();
    fs::create_dir_all(fixture.shared.accounts_root()).unwrap();
    let linked = fixture.shared.account(&account_id);
    if !create_directory_symlink(&external, linked.root()) {
        return;
    }
    fixture
        .manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(account_id.clone()),
            accounts: vec![entry(account_id, "Linked#1000")],
        })
        .unwrap();
    assert!(fixture.manager.active_snapshot().is_err());
    assert!(fixture.shared.legacy_database().is_file());
    assert!(!external.join("owstats.sqlite3").exists());
}

#[cfg(windows)]
fn create_directory_symlink(source: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::windows::fs::symlink_dir(source, target).is_ok()
}

#[cfg(windows)]
fn create_file_symlink(source: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::windows::fs::symlink_file(source, target).is_ok()
}

#[cfg(unix)]
fn create_directory_symlink(source: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::unix::fs::symlink(source, target).is_ok()
}

#[cfg(unix)]
fn create_file_symlink(source: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::unix::fs::symlink(source, target).is_ok()
}

#[test]
fn legacy_wal_snapshot_includes_committed_uncheckpointed_rows_and_cleans_sidecars() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    seed_legacy_wal_snapshot(&shared, "wal-only-row");
    assert!(shared.legacy_database_wal().is_file());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let account = confirm_account_for(&manager, &shared);

    manager.adopt_legacy(&account).unwrap();

    let connection = account.database().connect().unwrap();
    let value: String = connection
        .query_row("SELECT value FROM wal_probe", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "wal-only-row");
    assert!(!shared.legacy_database().exists());
    assert!(!shared.legacy_database_wal().exists());
    assert!(!shared.legacy_database_shm().exists());
}

#[test]
fn failed_wal_migration_preserves_main_wal_and_shm_files() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    seed_legacy_wal_snapshot(&shared, "wal-only-row");
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let account = confirm_account_for(&manager, &shared);
    fs::write(account.paths().database(), b"target-conflict").unwrap();

    assert!(manager.adopt_legacy(&account).is_err());

    assert!(shared.legacy_database().is_file());
    assert!(shared.legacy_database_wal().is_file());
    assert!(shared.legacy_database_shm().is_file());
}

#[test]
fn concurrent_adopt_calls_are_serialized_and_publish_every_owned_file() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    fs::write(fixture.shared.legacy_player_profile(), "synthetic-profile").unwrap();
    fs::create_dir_all(fixture.shared.legacy_seasons()).unwrap();
    fs::write(
        fixture.shared.legacy_seasons().join("season.json"),
        "synthetic-season",
    )
    .unwrap();
    let account = fixture.confirm_account();
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let manager = fixture.manager.clone();
        let account = account.clone();
        let barrier = barrier.clone();
        handles.push(thread::spawn(move || {
            barrier.wait();
            manager.adopt_legacy(&account)
        }));
    }
    barrier.wait();

    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
    assert_eq!(
        fs::read_to_string(account.paths().player_profile()).unwrap(),
        "synthetic-profile"
    );
    assert!(account.paths().seasons().join("season.json").is_file());
    assert!(!fixture.shared.legacy_database().exists());
}

#[test]
fn existing_registry_never_makes_stray_legacy_data_eligible() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let legacy = owstats_desktop_lib::OwDatabase::new(shared.legacy_database());
    legacy.initialize().unwrap();
    let account_id = AccountId::parse("36be8ecb-ecc4-4a15-b4d5-7d1fa355ab9c").unwrap();
    let paths = shared.account(&account_id);
    fs::create_dir_all(paths.root()).unwrap();
    write_manifest_atomic(
        &paths.manifest(),
        &manifest(account_id.clone(), "Existing#1000"),
    )
    .unwrap();
    fs::write(
        paths.credentials(),
        r#"{"roleId":10001,"token":"synthetic-existing-token"}"#,
    )
    .unwrap();
    write_registry_atomic(
        &shared.registry(),
        &AccountRegistry {
            version: 1,
            active_account_id: Some(account_id.clone()),
            accounts: vec![entry(account_id, "Existing#1000")],
        },
    )
    .unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let account = manager.active_snapshot().unwrap();

    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::None
    );
    assert!(!shared.migration_eligibility().exists());
    assert!(manager.adopt_legacy(&account).is_err());
    assert!(shared.legacy_database().is_file());
}

#[test]
fn first_run_eligibility_survives_account_confirmation_and_restart() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    assert!(fixture.shared.migration_eligibility().is_file());
    let account = fixture.confirm_account();
    let restarted = AccountContextManager::load(fixture.shared.clone()).unwrap();

    restarted.adopt_legacy(&account).unwrap();

    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
}

#[test]
fn damaged_registry_never_creates_legacy_eligibility() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    owstats_desktop_lib::OwDatabase::new(shared.legacy_database())
        .initialize()
        .unwrap();
    fs::write(shared.registry(), b"damaged-registry").unwrap();

    assert!(AccountContextManager::load(shared.clone()).is_err());
    assert!(!shared.migration_eligibility().exists());
    assert!(shared.legacy_database().is_file());
}

#[test]
fn damaged_eligibility_is_reported_without_guessing_or_touching_legacy_data() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    owstats_desktop_lib::OwDatabase::new(shared.legacy_database())
        .initialize()
        .unwrap();
    fs::create_dir_all(shared.operations_root()).unwrap();
    fs::write(
        shared.migration_eligibility(),
        r#"{"version":99,"token":"synthetic-must-not-escape"}"#,
    )
    .unwrap();

    let manager = AccountContextManager::load(shared.clone()).unwrap();

    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::RecoveryError
    );
    assert!(shared.legacy_database().is_file());
    assert!(shared.migration_eligibility().is_file());
}

#[test]
fn cleaning_phase_with_partial_non_database_publish_keeps_all_legacy_sources() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    fs::write(fixture.shared.legacy_player_profile(), b"profile").unwrap();
    let account = fixture.confirm_account();
    fs::copy(fixture.shared.legacy_database(), account.paths().database()).unwrap();
    let staging = fixture.staging(account.account_id());
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("player_profile.json"), b"profile").unwrap();
    fixture.write_journal(account.account_id(), MigrationPhase::CleaningLegacy);
    fs::write(
        fixture
            .shared
            .migration_content_manifest(account.account_id()),
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "databaseRequired": true,
            "entries": [{
                "relativePath": "player_profile.json",
                "kind": "file",
                "size": 7,
                "sha256": sha256_hex(b"profile")
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    let manager = AccountContextManager::load(fixture.shared.clone()).unwrap();

    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::RecoveryError
    );
    assert!(fixture.shared.legacy_database().is_file());
    assert!(fixture.shared.legacy_player_profile().is_file());
    assert!(!account.paths().player_profile().exists());
}

#[test]
fn cleaning_rejects_valid_but_different_database_until_original_is_restored() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    fs::copy(fixture.shared.legacy_database(), account.paths().database()).unwrap();
    let expected_database = fs::read(account.paths().database()).unwrap();
    fixture.write_journal(account.account_id(), MigrationPhase::CleaningLegacy);
    write_database_manifest(&fixture.shared, account.account_id(), &expected_database);
    fs::remove_dir_all(fixture.staging(account.account_id())).unwrap();

    let replacement = fixture.shared.root().join("replacement.sqlite3");
    let replacement_database = owstats_desktop_lib::OwDatabase::new(&replacement);
    replacement_database.initialize().unwrap();
    replacement_database
        .connect()
        .unwrap()
        .execute(
            "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES ('replacement-match', 22, 'preset', 'SportPreset', 'normal', 'win', 'map', 'hero', 100, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, '{}', 'now')",
            [],
        )
        .unwrap();
    fs::copy(&replacement, account.paths().database()).unwrap();

    let manager = AccountContextManager::load(fixture.shared.clone()).unwrap();
    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::RecoveryError
    );
    assert!(fixture.shared.legacy_database().is_file());

    fs::write(account.paths().database(), expected_database).unwrap();
    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::Complete
    );
    assert!(!fixture.shared.legacy_database().exists());
}

#[test]
fn cleaning_without_legacy_or_staging_converges_using_stable_manifest() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    fs::copy(fixture.shared.legacy_database(), account.paths().database()).unwrap();
    let expected_database = fs::read(account.paths().database()).unwrap();
    fixture.write_journal(account.account_id(), MigrationPhase::CleaningLegacy);
    write_database_manifest(&fixture.shared, account.account_id(), &expected_database);
    fs::remove_dir_all(fixture.staging(account.account_id())).unwrap();
    fs::remove_file(fixture.shared.legacy_database()).unwrap();
    fs::remove_file(fixture.shared.legacy_credentials()).unwrap();

    let manager = AccountContextManager::load(fixture.shared.clone()).unwrap();

    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::Complete
    );
    assert!(!fixture
        .shared
        .migration_content_manifest(account.account_id())
        .exists());
}

#[test]
fn cleaning_without_stable_manifest_never_blind_completes() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    fs::copy(fixture.shared.legacy_database(), account.paths().database()).unwrap();
    fixture.write_journal(account.account_id(), MigrationPhase::CleaningLegacy);
    fs::remove_file(
        fixture
            .shared
            .migration_content_manifest(account.account_id()),
    )
    .unwrap();
    fs::remove_dir_all(fixture.staging(account.account_id())).unwrap();

    let manager = AccountContextManager::load(fixture.shared.clone()).unwrap();

    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::RecoveryError
    );
    assert!(fixture.shared.legacy_database().is_file());
    let journal: MigrationJournal =
        serde_json::from_slice(&fs::read(fixture.shared.migration_journal()).unwrap()).unwrap();
    assert_eq!(journal.phase, MigrationPhase::CleaningLegacy);
}

#[test]
fn complete_recovery_cleans_residual_staging_and_manifest_without_changing_state() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    let account = fixture.confirm_account();
    let staging = fixture.staging(account.account_id());
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("residual"), b"residual").unwrap();
    fixture.write_journal(account.account_id(), MigrationPhase::Complete);
    fs::write(
        fixture
            .shared
            .migration_content_manifest(account.account_id()),
        r#"{"version":1,"entries":[]}"#,
    )
    .unwrap();

    let manager = AccountContextManager::load(fixture.shared.clone()).unwrap();
    let journal: MigrationJournal =
        serde_json::from_slice(&fs::read(fixture.shared.migration_journal()).unwrap()).unwrap();

    assert_eq!(
        manager.migration_state().unwrap(),
        LegacyMigrationState::Complete
    );
    assert_eq!(journal.phase, MigrationPhase::Complete);
    assert!(!staging.exists());
    assert!(!fixture
        .shared
        .migration_content_manifest(account.account_id())
        .exists());
}

#[test]
fn eligible_first_run_can_use_confirmed_account_when_legacy_credentials_are_absent() {
    let fixture = LegacyFixture::with_database_match("legacy-match");
    assert!(fixture.shared.migration_eligibility().is_file());
    fs::remove_file(fixture.shared.legacy_credentials()).unwrap();
    let account = fixture.confirm_account();

    fixture.manager.adopt_legacy(&account).unwrap();

    assert_eq!(fixture.match_ids(&account), vec!["legacy-match"]);
    assert_eq!(
        fs::read_to_string(account.paths().credentials()).unwrap(),
        r#"{"roleId":10001,"token":"synthetic-account-token","server":1,"savedAt":"2026-01-01T00:00:00Z"}"#
    );
}

fn confirm_account_for(
    manager: &AccountContextManager,
    shared: &SharedAppPaths,
) -> owstats_desktop_lib::account_context::AccountContextSnapshot {
    let account_id = AccountId::parse("36be8ecb-ecc4-4a15-b4d5-7d1fa355ab9c").unwrap();
    let paths = shared.account(&account_id);
    fs::create_dir_all(paths.root()).unwrap();
    write_manifest_atomic(
        &paths.manifest(),
        &manifest(account_id.clone(), "Tester#5479"),
    )
    .unwrap();
    fs::write(
        paths.credentials(),
        r#"{"roleId":10001,"token":"synthetic-account-token"}"#,
    )
    .unwrap();
    manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(account_id.clone()),
            accounts: vec![entry(account_id, "Tester#5479")],
        })
        .unwrap();
    manager.active_snapshot().unwrap()
}

fn seed_legacy_wal_snapshot(shared: &SharedAppPaths, value: &str) {
    let source_temp = tempfile::tempdir().unwrap();
    let source_path = source_temp.path().join("source.sqlite3");
    let connection = rusqlite::Connection::open(&source_path).unwrap();
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .unwrap();
    connection
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    connection
        .execute_batch(
            "CREATE TABLE wal_probe (value TEXT NOT NULL); INSERT INTO wal_probe VALUES ('wal-only-row');",
        )
        .unwrap();
    fs::copy(&source_path, shared.legacy_database()).unwrap();
    fs::copy(
        source_path.with_extension("sqlite3-wal"),
        shared.legacy_database_wal(),
    )
    .unwrap();
    fs::copy(
        source_path.with_extension("sqlite3-shm"),
        shared.legacy_database_shm(),
    )
    .unwrap();
    drop(connection);
    assert_eq!(value, "wal-only-row");
}

fn entry(account_id: AccountId, battle_tag: &str) -> AccountRegistryEntry {
    AccountRegistryEntry {
        account_id,
        battle_tag: battle_tag.to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        last_used_at: "2026-01-02T00:00:00Z".to_string(),
        credential_state: "ready".to_string(),
    }
}

fn manifest(account_id: AccountId, battle_tag: &str) -> AccountManifest {
    AccountManifest {
        version: 1,
        account_id,
        role_id: 10001,
        battle_tag: battle_tag.to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-02T00:00:00Z".to_string(),
    }
}

fn registry_with_active(
    active_account_id: AccountId,
    first_id: AccountId,
    second_id: AccountId,
) -> AccountRegistry {
    AccountRegistry {
        version: 1,
        active_account_id: Some(active_account_id),
        accounts: vec![
            entry(first_id, "Synthetic#1000"),
            entry(second_id, "Synthetic#2000"),
        ],
    }
}

fn seed_match_for_active_account(manager: &AccountContextManager, match_id: &str) {
    let db = manager.active_snapshot().unwrap().database();
    db.initialize().unwrap();
    db.connect()
        .unwrap()
        .execute(
            "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, 22, 'preset', 'SportPreset', 'normal', 'win', 'map', 'hero', 100, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, '{}', 'now')",
            params![match_id],
        )
        .unwrap();
}

#[test]
fn switching_active_context_routes_queries_to_separate_databases() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let second_id = AccountId::parse("9a494190-fdee-46f2-abd2-7de46f4f4376").unwrap();
    for (id, tag) in [
        (&first_id, "Synthetic#1000"),
        (&second_id, "Synthetic#2000"),
    ] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    let manager = AccountContextManager::load(shared).unwrap();

    manager
        .replace_registry(registry_with_active(
            first_id.clone(),
            first_id.clone(),
            second_id.clone(),
        ))
        .unwrap();
    seed_match_for_active_account(&manager, "first-account-match");

    manager
        .replace_registry(registry_with_active(
            second_id.clone(),
            first_id.clone(),
            second_id,
        ))
        .unwrap();
    seed_match_for_active_account(&manager, "second-account-match");

    let second_rows = list_matches(
        &manager.active_snapshot().unwrap().database(),
        MatchFilters::default(),
    )
    .unwrap();
    assert_eq!(
        second_rows
            .into_iter()
            .map(|row| row.match_id)
            .collect::<Vec<_>>(),
        vec!["second-account-match"]
    );

    manager
        .replace_registry(registry_with_active(
            first_id.clone(),
            first_id,
            AccountId::parse("9a494190-fdee-46f2-abd2-7de46f4f4376").unwrap(),
        ))
        .unwrap();
    let first_rows = list_matches(
        &manager.active_snapshot().unwrap().database(),
        MatchFilters::default(),
    )
    .unwrap();
    assert_eq!(
        first_rows
            .into_iter()
            .map(|row| row.match_id)
            .collect::<Vec<_>>(),
        vec!["first-account-match"]
    );
}

#[test]
fn account_database_initialization_does_not_create_privacy_backups() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let account = shared.account(&account_id);
    fs::create_dir_all(account.seasons()).unwrap();
    fs::write(
        account.seasons().join("local_s22_full_season.json"),
        r#"{"safe":"ok","token":"synthetic-season-secret"}"#,
    )
    .unwrap();

    let db = owstats_desktop_lib::OwDatabase::for_account(&account);
    db.initialize().unwrap();

    assert!(
        !account.backups().exists(),
        "new account initialization must not create a privacy-* backup"
    );
    assert_eq!(
        fs::read_to_string(account.seasons().join("local_s22_full_season.json")).unwrap(),
        r#"{"safe":"ok","token":"synthetic-season-secret"}"#
    );
}

#[test]
fn public_status_and_profile_serialization_hide_internal_role_ids_and_secrets() {
    let value = json!({
        "credentialStatus": CredentialStatus {
            state: "ready".to_string(),
            message: "synthetic ready".to_string(),
            exposes_secret: false,
            role_id: Some(987654321),
            saved_at: Some("2026-01-01T00:00:00Z".to_string()),
        },
        "playerProfile": PlayerProfile {
            role_id: Some(987654321),
            player_id: Some("synthetic-player".to_string()),
            avatar_url: None,
            title_icon_url: None,
            endorsement_level: None,
            play_time_hours: None,
            title: None,
            refreshed_at: None,
        }
    });
    let encoded = serde_json::to_string(&value).unwrap().to_ascii_lowercase();

    for forbidden in [
        "roleid",
        "role_id",
        "987654321",
        "token",
        "customertoken",
        "cookie",
        "authorization",
        "credential\"",
    ] {
        assert!(
            !encoded.contains(forbidden),
            "public JSON exposed forbidden key or value {forbidden:?}: {encoded}"
        );
    }
}

#[test]
fn account_public_json_never_contains_private_identity_or_secrets() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared).unwrap();
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    install_account(
        &manager,
        account_id,
        101101,
        "Privacy#1000",
        "synthetic-private-secret",
        true,
    );

    let json = serde_json::to_string(&manager.status().unwrap())
        .unwrap()
        .to_ascii_lowercase();
    for forbidden in [
        "roleid",
        "role_id",
        "101101",
        "token",
        "customertoken",
        "cookie",
        "authorization",
        "credential\"",
        "synthetic-private-secret",
    ] {
        assert!(
            !json.contains(forbidden),
            "public account JSON exposed forbidden data"
        );
    }
}

#[test]
fn player_commands_resolve_exactly_one_active_snapshot_and_avoid_legacy_paths() {
    let source = include_str!("../src/commands.rs");
    for forbidden in [
        "credential_path(",
        "player_target_path(",
        "player_profile_path(",
        "AppPaths::from_app",
        "app_database(",
    ] {
        assert!(
            !source.contains(forbidden),
            "player command routing still uses legacy helper {forbidden}"
        );
    }

    for command in [
        "get_overview",
        "list_match_filters",
        "list_match_timeline",
        "search_match_player_options",
        "get_match_details",
        "refresh_match_evaluations",
        "get_friend_analytics",
        "get_friend_details",
        "get_onboarding_status",
        "get_personal_season_review",
        "get_credential_status",
        "get_player_profile",
        "get_player_target_status",
        "get_login_target_status",
        "confirm_login_target_switch",
        "keep_current_player_target",
        "retry_login_target_binding",
        "clear_player_target",
        "get_rankings",
        "get_season_analysis",
        "get_career_mmr_overview",
        "get_role_mmr_history",
        "get_role_mmr_mode_breakdown",
        "get_role_mmr_season_breakdown",
        "get_mmr_match_explanation",
        "get_recent_mmr_explanations",
        "rebuild_career_mmr",
        "get_career_mmr_build_status",
        "sync_season",
    ] {
        let function = command_source(source, command);
        assert!(
            function.contains("State<'_, AccountContextManager>"),
            "{command} does not receive account context state"
        );
        assert_eq!(
            function.matches("active_account(&accounts)?").count(),
            1,
            "{command} must resolve exactly one active snapshot at command entry"
        );
    }
}

#[test]
fn account_mutating_commands_acquire_permit_before_resolving_snapshot() {
    let source = include_str!("../src/commands.rs");
    for command in [
        "refresh_match_evaluations",
        "confirm_login_target_switch",
        "keep_current_player_target",
        "retry_login_target_binding",
        "clear_player_target",
        "rebuild_career_mmr",
        "sync_season",
    ] {
        let function = command_source(source, command);
        let permit = function
            .find("try_acquire_account_operation()")
            .unwrap_or_else(|| panic!("{command} does not acquire the account-operation permit"));
        let snapshot = function
            .find("active_account(&accounts)?")
            .unwrap_or_else(|| panic!("{command} does not resolve an account snapshot"));
        assert!(
            permit < snapshot,
            "{command} resolves the account before holding the operation permit"
        );
    }
}

fn command_source<'a>(source: &'a str, command: &str) -> &'a str {
    let sync_signature = format!("pub async fn {command}");
    let regular_signature = format!("pub fn {command}");
    let start = source
        .find(&sync_signature)
        .or_else(|| source.find(&regular_signature))
        .unwrap_or_else(|| panic!("missing command {command}"));
    let rest = &source[start..];
    let end = rest.find("\n#[tauri::command").unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn active_context_resolves_one_complete_account_without_exposing_role_id() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let account_entry = entry(account_id.clone(), "Synthetic#1000");
    let registry = AccountRegistry {
        version: 1,
        active_account_id: Some(account_id.clone()),
        accounts: vec![account_entry],
    };
    write_registry_atomic(&shared.registry(), &registry).unwrap();
    write_manifest_atomic(
        &shared.account(&account_id).manifest(),
        &manifest(account_id.clone(), "Synthetic#1000"),
    )
    .unwrap();

    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let snapshot = manager.active_snapshot().unwrap();
    assert_eq!(snapshot.account_id(), &account_id);
    assert_eq!(
        snapshot.paths().database(),
        shared.account(&account_id).database()
    );

    let encoded = serde_json::to_string(&manager.list_accounts().unwrap()).unwrap();
    assert!(encoded.contains("Synthetic#1000"));
    for forbidden in [
        "roleId",
        "role_id",
        "10001",
        "token",
        "customerToken",
        "cookie",
        "authorization",
        "credential\"",
    ] {
        assert!(
            !encoded
                .to_ascii_lowercase()
                .contains(&forbidden.to_ascii_lowercase()),
            "account summary exposed forbidden value or key {forbidden:?}: {encoded}"
        );
    }
}

#[test]
fn missing_active_account_returns_a_chinese_product_error() {
    let temp = tempfile::tempdir().unwrap();
    let manager = AccountContextManager::load(SharedAppPaths::from_app_data_dir(temp.path()))
        .expect("an absent registry should load as an empty account manager");

    assert_eq!(
        manager.active_snapshot().unwrap_err().to_string(),
        "尚未添加可用账号，请先在指挥台添加账号。"
    );
}

#[test]
fn empty_and_corrupt_registries_fail_safely_without_guessing_an_account() {
    let empty_temp = tempfile::tempdir().unwrap();
    let empty_shared = SharedAppPaths::from_app_data_dir(empty_temp.path());
    let empty_manager = AccountContextManager::load(empty_shared).unwrap();
    let status = empty_manager.status().unwrap();
    assert_eq!(status.active_account_id, None);
    assert!(status.accounts.is_empty());

    let corrupt_temp = tempfile::tempdir().unwrap();
    let corrupt_shared = SharedAppPaths::from_app_data_dir(corrupt_temp.path());
    fs::write(corrupt_shared.registry(), b"not-json").unwrap();
    let error = match AccountContextManager::load(corrupt_shared) {
        Ok(_) => panic!("corrupt registry must not load"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("账号索引"));
    for forbidden in ["not-json", "role_id", "token"] {
        assert!(!error.to_ascii_lowercase().contains(forbidden));
    }
}

#[test]
fn replace_registry_persists_before_updating_memory_and_supports_concurrent_readers() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = Arc::new(AccountContextManager::load(shared.clone()).unwrap());
    let account_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let valid = AccountRegistry {
        version: 1,
        active_account_id: Some(account_id.clone()),
        accounts: vec![entry(account_id, "Synthetic#1000")],
    };
    manager.replace_registry(valid.clone()).unwrap();
    assert_eq!(read_registry(&shared.registry()).unwrap(), valid);

    let mut readers = Vec::new();
    for _ in 0..4 {
        let manager = Arc::clone(&manager);
        readers.push(thread::spawn(move || manager.registry_snapshot().unwrap()));
    }
    for reader in readers {
        assert_eq!(reader.join().unwrap(), valid);
    }

    let invalid = AccountRegistry {
        version: 2,
        active_account_id: None,
        accounts: Vec::new(),
    };
    assert!(manager.replace_registry(invalid).is_err());
    assert_eq!(manager.registry_snapshot().unwrap(), valid);
    assert_eq!(read_registry(&shared.registry()).unwrap(), valid);
}

#[test]
fn account_paths_are_isolated_and_reject_traversal() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let second_id = AccountId::parse("9a494190-fdee-46f2-abd2-7de46f4f4376").unwrap();
    let generated_id = AccountId::new();
    fn assert_ord<T: Ord>() {}
    assert_ord::<AccountId>();
    assert_eq!(
        AccountId::parse(generated_id.as_str()).unwrap(),
        generated_id
    );
    let first = shared.account(&first_id);
    let second = shared.account(&second_id);

    assert_ne!(first.root(), second.root());
    assert_eq!(first.root(), shared.accounts_root().join(first_id.as_str()));
    assert_eq!(first.manifest().file_name().unwrap(), "account.json");
    assert_eq!(first.credentials().file_name().unwrap(), "credentials.json");
    assert_eq!(first.database().file_name().unwrap(), "owstats.sqlite3");
    assert!(first.manifest().starts_with(shared.accounts_root()));
    assert!(first.credentials().starts_with(first.root()));
    assert!(first.database().starts_with(first.root()));
    assert!(first.player_target().starts_with(first.root()));
    assert!(first.player_profile().starts_with(first.root()));
    assert!(first.seasons().starts_with(first.root()));
    assert!(first.diagnostics().starts_with(first.root()));
    assert!(first.backups().starts_with(first.root()));

    assert_eq!(shared.registry().file_name().unwrap(), "accounts.json");
    assert_eq!(
        shared.legacy_database(),
        temp.path().join("owstats.sqlite3")
    );
    assert!(shared.user_settings().starts_with(shared.root()));
    assert!(shared.browser_profile().starts_with(shared.root()));
    assert_eq!(
        shared.deletion_journal(),
        shared.operations_root().join("deletion-journal.json")
    );
    assert_eq!(
        shared.migration_journal(),
        shared.operations_root().join("migration-journal.json")
    );
    assert_eq!(
        shared.pending_logins(),
        shared.operations_root().join("pending-logins")
    );

    for invalid in [
        "../outside",
        "..\\outside",
        "not-a-uuid",
        "d59caecf-8e74-4b6c-b690-45e75f7fed9c/extra",
        "",
    ] {
        assert!(AccountId::parse(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn all_account_owned_paths_change_together_after_switch() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let first_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let second_id = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    for (id, tag, role_id) in [
        (&first_id, "First#1000", 101101),
        (&second_id, "Second#2000", 202202),
    ] {
        install_account(
            &manager,
            id.clone(),
            role_id,
            tag,
            "synthetic-secret",
            false,
        );
    }
    manager
        .switch_active(&first_id, "2026-07-16T10:00:00Z")
        .unwrap();
    let first = manager.active_snapshot().unwrap();
    manager
        .switch_active(&second_id, "2026-07-16T10:01:00Z")
        .unwrap();
    let second = manager.active_snapshot().unwrap();

    assert_ne!(first.paths().credentials(), second.paths().credentials());
    assert_ne!(
        first.paths().player_target(),
        second.paths().player_target()
    );
    assert_ne!(
        first.paths().player_profile(),
        second.paths().player_profile()
    );
    assert_ne!(first.paths().database(), second.paths().database());
    assert_ne!(first.paths().seasons(), second.paths().seasons());
    assert_ne!(first.paths().diagnostics(), second.paths().diagnostics());
    assert_ne!(first.paths().backups(), second.paths().backups());
}

#[test]
fn stale_credentials_do_not_hide_existing_local_account_data() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared).unwrap();
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let paths = manager.shared_paths().account(&account_id);
    fs::create_dir_all(paths.root()).unwrap();
    write_manifest_atomic(
        &paths.manifest(),
        &manifest(account_id.clone(), "Expired#1000"),
    )
    .unwrap();
    let mut stale = entry(account_id.clone(), "Expired#1000");
    stale.credential_state = "stale".to_string();
    manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(account_id),
            accounts: vec![stale],
        })
        .unwrap();
    seed_match_for_active_account(&manager, "expired-credential-local-match");

    assert_eq!(
        manager.status().unwrap().accounts[0].credential_state,
        "stale"
    );
    let rows = list_matches(
        &manager.active_snapshot().unwrap().database(),
        MatchFilters::default(),
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].match_id, "expired-credential-local-match");
}

#[test]
fn registry_round_trip_and_contains_no_credential_secret_fields() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let second_id = AccountId::parse("9a494190-fdee-46f2-abd2-7de46f4f4376").unwrap();
    let registry = AccountRegistry {
        version: 1,
        active_account_id: Some(first_id.clone()),
        accounts: vec![
            entry(first_id.clone(), "Synthetic#1000"),
            entry(second_id.clone(), "Synthetic#1000"),
        ],
    };

    write_registry_atomic(&shared.registry(), &registry).unwrap();
    let loaded = read_registry(&shared.registry()).unwrap();
    assert_eq!(loaded, registry);
    assert_eq!(loaded.accounts[0].battle_tag, loaded.accounts[1].battle_tag);
    assert_ne!(loaded.accounts[0].account_id, loaded.accounts[1].account_id);

    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(shared.registry()).unwrap()).unwrap();
    let object = value.as_object().unwrap();
    assert!(object.contains_key("activeAccountId"));
    assert!(!object.contains_key("active_account_id"));
    let first_account = object["accounts"][0].as_object().unwrap();
    for key in [
        "accountId",
        "battleTag",
        "createdAt",
        "lastUsedAt",
        "credentialState",
    ] {
        assert!(
            first_account.contains_key(key),
            "missing camelCase key {key}"
        );
    }
    for key in [
        "account_id",
        "battle_tag",
        "created_at",
        "last_used_at",
        "credential_state",
    ] {
        assert!(
            !first_account.contains_key(key),
            "wrote snake_case key {key}"
        );
    }
    let mut keys = Vec::new();
    collect_object_keys(&value, &mut keys);
    for forbidden in [
        "token",
        "customertoken",
        "cookie",
        "authorization",
        "credential",
        "role_id",
        "roleid",
    ] {
        assert!(
            !keys.iter().any(|key| key == forbidden),
            "registry exposed forbidden key {forbidden}"
        );
    }

    let replacement = AccountRegistry {
        version: 1,
        active_account_id: Some(second_id),
        accounts: registry.accounts,
    };
    write_registry_atomic(&shared.registry(), &replacement).unwrap();
    assert_eq!(read_registry(&shared.registry()).unwrap(), replacement);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
fn registry_rejects_unsupported_version_duplicates_and_missing_active_account() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("accounts.json");
    let id = "d59caecf-8e74-4b6c-b690-45e75f7fed9c";
    let other = "9a494190-fdee-46f2-abd2-7de46f4f4376";

    let cases = [
        json!({ "version": 2, "activeAccountId": null, "accounts": [] }),
        json!({
            "version": 1,
            "activeAccountId": null,
            "accounts": [
                { "accountId": id, "battleTag": "Synthetic#1000", "createdAt": "a", "lastUsedAt": "b", "credentialState": "ready" },
                { "accountId": id, "battleTag": "Synthetic#2000", "createdAt": "a", "lastUsedAt": "b", "credentialState": "missing" }
            ]
        }),
        json!({
            "version": 1,
            "activeAccountId": other,
            "accounts": [
                { "accountId": id, "battleTag": "Synthetic#1000", "createdAt": "a", "lastUsedAt": "b", "credentialState": "ready" }
            ]
        }),
        json!({
            "version": 1,
            "activeAccountId": null,
            "accounts": [
                { "accountId": "../outside", "battleTag": "Synthetic#1000", "createdAt": "a", "lastUsedAt": "b", "credentialState": "ready" }
            ]
        }),
        json!({
            "version": 1,
            "activeAccountId": null,
            "accounts": [
                { "accountId": id, "battleTag": "Synthetic#1000", "createdAt": "a", "lastUsedAt": "b", "credentialState": "token=synthetic-secret" }
            ]
        }),
    ];

    for value in cases {
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(read_registry(&path).is_err());
    }
}

#[test]
fn manifest_round_trip_rejects_version_and_account_id_mismatch() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let expected = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let other = AccountId::parse("9a494190-fdee-46f2-abd2-7de46f4f4376").unwrap();
    let account = shared.account(&expected);

    write_manifest_atomic(
        &account.manifest(),
        &manifest(expected.clone(), "Synthetic#1000"),
    )
    .unwrap();
    assert_eq!(
        read_manifest(&account.manifest(), &expected).unwrap(),
        manifest(expected.clone(), "Synthetic#1000")
    );
    let manifest_value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(account.manifest()).unwrap()).unwrap();
    let manifest_object = manifest_value.as_object().unwrap();
    for key in ["accountId", "roleId", "battleTag", "createdAt", "updatedAt"] {
        assert!(
            manifest_object.contains_key(key),
            "missing camelCase key {key}"
        );
    }
    for key in [
        "account_id",
        "role_id",
        "battle_tag",
        "created_at",
        "updated_at",
    ] {
        assert!(
            !manifest_object.contains_key(key),
            "wrote snake_case key {key}"
        );
    }
    write_manifest_atomic(
        &account.manifest(),
        &manifest(expected.clone(), "Synthetic#1001"),
    )
    .unwrap();
    assert_eq!(
        read_manifest(&account.manifest(), &expected)
            .unwrap()
            .battle_tag,
        "Synthetic#1001"
    );

    let unsupported = json!({
        "version": 2,
        "accountId": expected.as_str(),
        "roleId": 10001,
        "battleTag": "Synthetic#1000",
        "createdAt": "a",
        "updatedAt": "b"
    });
    fs::write(
        account.manifest(),
        serde_json::to_vec(&unsupported).unwrap(),
    )
    .unwrap();
    assert!(read_manifest(&account.manifest(), &expected).is_err());

    fs::write(
        account.manifest(),
        serde_json::to_vec(&manifest(other, "Synthetic#2000")).unwrap(),
    )
    .unwrap();
    assert!(read_manifest(&account.manifest(), &expected).is_err());

    let wrong_directory =
        shared.account(&AccountId::parse("59dca10a-d0cb-4b15-80fb-aa5b45464cfa").unwrap());
    fs::create_dir_all(wrong_directory.root()).unwrap();
    fs::write(
        wrong_directory.manifest(),
        serde_json::to_vec(&manifest(expected.clone(), "Synthetic#1000")).unwrap(),
    )
    .unwrap();
    assert!(read_manifest(&wrong_directory.manifest(), &expected).is_err());
}

fn collect_object_keys(value: &serde_json::Value, keys: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                keys.push(key.to_ascii_lowercase());
                collect_object_keys(value, keys);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_object_keys(value, keys);
            }
        }
        _ => {}
    }
}

#[test]
fn switch_updates_active_account_and_recent_use_without_opening_account_data() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let second = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    for (id, tag) in [(&first, "First#1000"), (&second, "Second#2000")] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(first.clone()),
            accounts: vec![
                entry(first.clone(), "First#1000"),
                entry(second.clone(), "Second#2000"),
            ],
        })
        .unwrap();

    let now = "2026-07-16T12:34:56Z";
    let status = manager.switch_active(&second, now).unwrap();
    assert_eq!(status.active_account_id.as_deref(), Some(second.as_str()));
    assert_eq!(
        manager.registry_snapshot().unwrap().active_account_id,
        Some(second.clone())
    );
    let on_disk = read_registry(&shared.registry()).unwrap();
    assert_eq!(on_disk.active_account_id, Some(second.clone()));
    assert_eq!(
        on_disk
            .accounts
            .iter()
            .find(|entry| entry.account_id == second)
            .unwrap()
            .last_used_at,
        now
    );
    assert!(!shared.account(&first).database().exists());
    assert!(!shared.account(&second).database().exists());
    assert!(!shared.browser_profile().exists());
}

#[test]
fn switch_rejects_manifest_mismatch_without_registry_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let second = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    for (id, tag) in [(&first, "First#1000"), (&second, "Wrong#9999")] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    let before = registry_with_active(first.clone(), first, second.clone());
    manager.replace_registry(before.clone()).unwrap();

    assert_eq!(
        manager
            .switch_active(&second, "2026-07-16T12:34:56Z")
            .unwrap_err()
            .to_string(),
        "账号数据不完整，无法切换。"
    );
    assert_eq!(manager.registry_snapshot().unwrap(), before);
    assert_eq!(read_registry(&shared.registry()).unwrap(), before);
}

#[test]
fn switch_rejects_missing_account_directory_without_registry_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let missing = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let first_paths = shared.account(&first);
    fs::create_dir_all(first_paths.root()).unwrap();
    write_manifest_atomic(
        &first_paths.manifest(),
        &manifest(first.clone(), "First#1000"),
    )
    .unwrap();
    let before = registry_with_active(first.clone(), first, missing.clone());
    manager.replace_registry(before.clone()).unwrap();

    let error = manager
        .switch_active(&missing, "2026-07-16T12:34:56Z")
        .unwrap_err()
        .to_string();
    assert_eq!(error, "账号数据不完整，无法切换。");
    assert_eq!(manager.registry_snapshot().unwrap(), before);
    assert_eq!(read_registry(&shared.registry()).unwrap(), before);
}

#[test]
fn removing_active_account_deletes_only_it_and_selects_recent_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let second = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    let third = AccountId::parse("33333333-3333-4333-8333-333333333333").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let mut entries = Vec::new();
    for (id, tag, last, bytes) in [
        (
            &first,
            "First#1000",
            "2026-07-01T00:00:00Z",
            b"first".as_slice(),
        ),
        (
            &second,
            "Second#2000",
            "2026-07-02T00:00:00Z",
            b"second".as_slice(),
        ),
        (
            &third,
            "Third#3000",
            "2026-07-03T00:00:00Z",
            b"third".as_slice(),
        ),
    ] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
        fs::write(paths.database(), bytes).unwrap();
        let mut item = entry(id.clone(), tag);
        item.last_used_at = last.to_string();
        entries.push(item);
    }
    manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(first.clone()),
            accounts: entries,
        })
        .unwrap();

    let result = manager
        .remove_account(&first, "2026-07-16T13:00:00Z")
        .unwrap();
    assert!(result.removed);
    assert!(result.reload_required);
    assert_eq!(result.active_account_id.as_deref(), Some(third.as_str()));
    assert!(!shared.account(&first).root().exists());
    assert_eq!(
        fs::read(shared.account(&second).database()).unwrap(),
        b"second"
    );
    assert_eq!(
        fs::read(shared.account(&third).database()).unwrap(),
        b"third"
    );
    let registry = read_registry(&shared.registry()).unwrap();
    assert_eq!(registry.active_account_id, Some(third));
    assert_eq!(registry.accounts.len(), 2);
    assert!(!shared.deletion_journal().exists());
}

#[test]
fn remove_unknown_account_keeps_registry_and_safe_results_hide_private_fields() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let unknown = AccountId::parse("99999999-9999-4999-8999-999999999999").unwrap();
    let before = manager.registry_snapshot().unwrap();
    assert_eq!(
        manager
            .remove_account(&unknown, "2026-07-16T13:00:00Z")
            .unwrap_err()
            .to_string(),
        "账号不存在。"
    );
    assert_eq!(manager.registry_snapshot().unwrap(), before);

    let encoded = serde_json::to_string(&json!({
        "status": AccountManagerStatus { active_account_id: None, accounts: vec![], message: "暂无账号。".into() },
        "remove": RemoveAccountResult { removed: true, active_account_id: None, message: "账号已退出。".into(), reload_required: true }
    }))
    .unwrap()
    .to_ascii_lowercase();
    for forbidden in [
        "path",
        "roleid",
        "role_id",
        "token",
        "credential",
        "authorization",
    ] {
        assert!(
            !encoded.contains(forbidden),
            "public DTO exposed {forbidden}: {encoded}"
        );
    }
}

#[test]
fn removing_inactive_keeps_active_and_removing_last_clears_active() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let second = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    for (id, tag) in [(&first, "First#1000"), (&second, "Second#2000")] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(first.clone()),
            accounts: vec![
                entry(first.clone(), "First#1000"),
                entry(second.clone(), "Second#2000"),
            ],
        })
        .unwrap();

    let inactive = manager
        .remove_account(&second, "2026-07-16T13:00:00Z")
        .unwrap();
    assert_eq!(inactive.active_account_id.as_deref(), Some(first.as_str()));
    assert!(shared.account(&first).root().is_dir());
    let last = manager
        .remove_account(&first, "2026-07-16T13:01:00Z")
        .unwrap();
    assert_eq!(last.active_account_id, None);
    assert_eq!(
        read_registry(&shared.registry()).unwrap().active_account_id,
        None
    );
}

#[test]
fn removing_active_skips_corrupt_recent_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let active = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let valid = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    let corrupt = AccountId::parse("33333333-3333-4333-8333-333333333333").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    for (id, tag) in [(&active, "Active#1000"), (&valid, "Valid#2000")] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    fs::create_dir_all(shared.account(&corrupt).root()).unwrap();
    fs::write(shared.account(&corrupt).manifest(), b"not-json").unwrap();
    let mut active_entry = entry(active.clone(), "Active#1000");
    active_entry.last_used_at = "2026-07-01T00:00:00Z".into();
    let mut valid_entry = entry(valid.clone(), "Valid#2000");
    valid_entry.last_used_at = "2026-07-02T00:00:00Z".into();
    let mut corrupt_entry = entry(corrupt, "Corrupt#3000");
    corrupt_entry.last_used_at = "2026-07-99T00:00:00Z".into();
    manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(active.clone()),
            accounts: vec![active_entry, valid_entry, corrupt_entry],
        })
        .unwrap();

    let result = manager
        .remove_account(&active, "2026-07-16T13:00:00Z")
        .unwrap();
    assert_eq!(result.active_account_id.as_deref(), Some(valid.as_str()));
}

fn write_deletion_journal_fixture(
    shared: &SharedAppPaths,
    operation_id: &str,
    account_id: &AccountId,
    phase: &str,
    old_registry: &AccountRegistry,
    new_registry: &AccountRegistry,
) {
    fs::create_dir_all(shared.operations_root()).unwrap();
    fs::write(
        shared.deletion_journal(),
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "operationId": operation_id,
            "accountId": account_id,
            "phase": phase,
            "fallbackAccountId": new_registry.active_account_id,
            "startedAt": "2026-07-16T13:00:00Z",
            "oldRegistry": old_registry,
            "newRegistry": new_registry,
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn startup_recovery_rolls_back_quarantine_when_registry_is_old() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let removed = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let fallback = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    for (id, tag) in [(&removed, "Removed#1000"), (&fallback, "Fallback#2000")] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    let old_registry = AccountRegistry {
        version: 1,
        active_account_id: Some(removed.clone()),
        accounts: vec![
            entry(removed.clone(), "Removed#1000"),
            entry(fallback.clone(), "Fallback#2000"),
        ],
    };
    let new_registry = AccountRegistry {
        version: 1,
        active_account_id: Some(fallback),
        accounts: vec![old_registry.accounts[1].clone()],
    };
    write_registry_atomic(&shared.registry(), &old_registry).unwrap();
    let operation_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let quarantine = shared
        .operations_root()
        .join("deletions")
        .join(operation_id)
        .join(removed.as_str());
    fs::create_dir_all(quarantine.parent().unwrap()).unwrap();
    fs::rename(shared.account(&removed).root(), &quarantine).unwrap();
    write_deletion_journal_fixture(
        &shared,
        operation_id,
        &removed,
        "quarantined",
        &old_registry,
        &new_registry,
    );

    let manager = AccountContextManager::load(shared.clone()).unwrap();
    assert_eq!(manager.registry_snapshot().unwrap(), old_registry);
    assert!(shared.account(&removed).root().is_dir());
    assert!(!quarantine.exists());
    assert!(!shared.deletion_journal().exists());
}

#[test]
fn startup_recovery_finishes_delete_when_registry_is_new() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let removed = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let fallback = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    for (id, tag) in [(&removed, "Removed#1000"), (&fallback, "Fallback#2000")] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    let old_registry = AccountRegistry {
        version: 1,
        active_account_id: Some(removed.clone()),
        accounts: vec![
            entry(removed.clone(), "Removed#1000"),
            entry(fallback.clone(), "Fallback#2000"),
        ],
    };
    let new_registry = AccountRegistry {
        version: 1,
        active_account_id: Some(fallback),
        accounts: vec![old_registry.accounts[1].clone()],
    };
    write_registry_atomic(&shared.registry(), &new_registry).unwrap();
    let operation_id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    let quarantine = shared
        .operations_root()
        .join("deletions")
        .join(operation_id)
        .join(removed.as_str());
    fs::create_dir_all(quarantine.parent().unwrap()).unwrap();
    fs::rename(shared.account(&removed).root(), &quarantine).unwrap();
    write_deletion_journal_fixture(
        &shared,
        operation_id,
        &removed,
        "quarantined",
        &old_registry,
        &new_registry,
    );

    let manager = AccountContextManager::load(shared.clone()).unwrap();
    assert_eq!(manager.registry_snapshot().unwrap(), new_registry);
    assert!(!shared.account(&removed).root().exists());
    assert!(!quarantine.exists());
    assert!(!shared.deletion_journal().exists());
}

#[test]
fn remove_rejects_nested_link_without_touching_external_sentinel() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let paths = shared.account(&account_id);
    fs::create_dir_all(paths.root()).unwrap();
    write_manifest_atomic(
        &paths.manifest(),
        &manifest(account_id.clone(), "Safe#1000"),
    )
    .unwrap();
    let sentinel = outside.path().join("sentinel");
    fs::write(&sentinel, b"external-unchanged").unwrap();
    if !create_directory_symlink(outside.path(), &paths.root().join("nested-link")) {
        return;
    }
    let registry = AccountRegistry {
        version: 1,
        active_account_id: Some(account_id.clone()),
        accounts: vec![entry(account_id.clone(), "Safe#1000")],
    };
    manager.replace_registry(registry.clone()).unwrap();

    assert_eq!(
        manager
            .remove_account(&account_id, "2026-07-16T13:00:00Z")
            .unwrap_err()
            .to_string(),
        "账号数据不完整，无法退出。"
    );
    assert_eq!(fs::read(&sentinel).unwrap(), b"external-unchanged");
    assert_eq!(manager.registry_snapshot().unwrap(), registry);
}

#[cfg(windows)]
#[test]
fn remove_deterministically_rejects_windows_junction_without_external_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("44444444-4444-4444-8444-444444444444").unwrap();
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let paths = shared.account(&account_id);
    fs::create_dir_all(paths.root()).unwrap();
    write_manifest_atomic(
        &paths.manifest(),
        &manifest(account_id.clone(), "Junction#4000"),
    )
    .unwrap();
    let sentinel = outside.path().join("sentinel");
    fs::write(&sentinel, b"junction-external-unchanged").unwrap();
    let junction = paths.root().join("junction");
    let output = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&junction)
        .arg(outside.path())
        .output()
        .expect("cmd /c mklink /J must be available on Windows");
    assert!(
        output.status.success(),
        "deterministic junction setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let registry = AccountRegistry {
        version: 1,
        active_account_id: Some(account_id.clone()),
        accounts: vec![entry(account_id.clone(), "Junction#4000")],
    };
    manager.replace_registry(registry.clone()).unwrap();

    assert_eq!(
        manager
            .remove_account(&account_id, "2026-07-16T13:00:00Z")
            .unwrap_err()
            .to_string(),
        "账号数据不完整，无法退出。"
    );
    assert_eq!(fs::read(sentinel).unwrap(), b"junction-external-unchanged");
    assert_eq!(manager.registry_snapshot().unwrap(), registry);
    std::fs::remove_dir(&junction).unwrap();
}

#[test]
fn switch_and_add_account_share_one_lifecycle_mutex() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    let second = AccountId::parse("22222222-2222-4222-8222-222222222222").unwrap();
    let manager = Arc::new(AccountContextManager::load(shared.clone()).unwrap());
    for (id, tag) in [(&first, "First#1000"), (&second, "Second#2000")] {
        let paths = shared.account(id);
        fs::create_dir_all(paths.root()).unwrap();
        write_manifest_atomic(&paths.manifest(), &manifest(id.clone(), tag)).unwrap();
    }
    manager
        .replace_registry(AccountRegistry {
            version: 1,
            active_account_id: Some(first.clone()),
            accounts: vec![
                entry(first, "First#1000"),
                entry(second.clone(), "Second#2000"),
            ],
        })
        .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let switcher = {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        let second = second.clone();
        thread::spawn(move || {
            barrier.wait();
            manager
                .switch_active(&second, "2026-07-16T13:00:00Z")
                .unwrap();
        })
    };
    let adder = {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            publish_add_account(
                &manager,
                synthetic_capture(303303, "Third#3000", "synthetic-third-secret"),
            )
            .unwrap();
        })
    };
    barrier.wait();
    switcher.join().unwrap();
    adder.join().unwrap();
    let registry = manager.registry_snapshot().unwrap();
    assert_eq!(registry.accounts.len(), 3);
    assert_eq!(registry.active_account_id, Some(second));
    assert_eq!(read_registry(&shared.registry()).unwrap(), registry);
}

#[test]
fn account_management_commands_hold_shared_operation_permit() {
    let source = include_str!("../src/commands.rs");
    for command in ["switch_account", "remove_account"] {
        let function = command_source(source, command);
        assert!(function.contains("try_acquire_account_operation()"));
        assert!(function.contains("AccountId::parse"));
    }
    let lib = include_str!("../src/lib.rs");
    for command in ["list_accounts", "switch_account", "remove_account"] {
        assert!(lib.contains(&format!("commands::{command}")));
    }
}

#[test]
fn pending_deletion_recovery_gate_blocks_every_account_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let manager = AccountContextManager::load(shared.clone()).unwrap();
    let account_id = AccountId::parse("11111111-1111-4111-8111-111111111111").unwrap();
    install_account(
        &manager,
        account_id.clone(),
        101101,
        "First#1000",
        "synthetic-existing-secret",
        true,
    );
    fs::create_dir_all(shared.operations_root()).unwrap();
    fs::write(shared.deletion_journal(), b"{}").unwrap();
    let expected = "账号退出恢复失败，请重试。";

    assert_eq!(
        manager
            .switch_active(&account_id, "2026-07-16T13:00:00Z")
            .unwrap_err()
            .to_string(),
        expected
    );
    assert_eq!(
        manager
            .remove_account(&account_id, "2026-07-16T13:00:00Z")
            .unwrap_err()
            .to_string(),
        expected
    );
    assert_eq!(
        publish_add_account(
            &manager,
            synthetic_capture(202202, "Second#2000", "synthetic-new-secret"),
        )
        .unwrap_err()
        .to_string(),
        expected
    );
    assert_eq!(
        publish_relogin_account(
            &manager,
            &account_id,
            synthetic_capture(101101, "First#1000", "synthetic-refresh-secret"),
        )
        .unwrap_err()
        .to_string(),
        expected
    );
    let snapshot = manager.active_snapshot().unwrap();
    assert_eq!(
        manager.adopt_legacy(&snapshot).unwrap_err().to_string(),
        expected
    );

    fs::remove_file(shared.deletion_journal()).unwrap();
    assert!(manager
        .switch_active(&account_id, "2026-07-16T13:01:00Z")
        .is_ok());
}
