use owstats_desktop_lib::account_context::AccountContextManager;
use owstats_desktop_lib::account_store::{
    read_manifest, read_registry, write_manifest_atomic, write_registry_atomic, AccountId,
    AccountManifest, AccountRegistry, AccountRegistryEntry,
};
use owstats_desktop_lib::app_paths::SharedAppPaths;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::thread;

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
