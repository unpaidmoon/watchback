use owstats_desktop_lib::account_store::{
    read_manifest, read_registry, write_manifest_atomic, write_registry_atomic, AccountId,
    AccountManifest, AccountRegistry, AccountRegistryEntry,
};
use owstats_desktop_lib::app_paths::SharedAppPaths;
use serde_json::json;
use std::fs;

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
fn account_paths_are_isolated_and_reject_traversal() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let first_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let second_id = AccountId::parse("9a494190-fdee-46f2-abd2-7de46f4f4376").unwrap();
    let generated_id = AccountId::new();
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
        json!({ "version": 2, "active_account_id": null, "accounts": [] }),
        json!({
            "version": 1,
            "active_account_id": null,
            "accounts": [
                { "account_id": id, "battle_tag": "Synthetic#1000", "created_at": "a", "last_used_at": "b", "credential_state": "ready" },
                { "account_id": id, "battle_tag": "Synthetic#2000", "created_at": "a", "last_used_at": "b", "credential_state": "missing" }
            ]
        }),
        json!({
            "version": 1,
            "active_account_id": other,
            "accounts": [
                { "account_id": id, "battle_tag": "Synthetic#1000", "created_at": "a", "last_used_at": "b", "credential_state": "ready" }
            ]
        }),
        json!({
            "version": 1,
            "active_account_id": null,
            "accounts": [
                { "account_id": "../outside", "battle_tag": "Synthetic#1000", "created_at": "a", "last_used_at": "b", "credential_state": "ready" }
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
        "account_id": expected.as_str(),
        "role_id": 10001,
        "battle_tag": "Synthetic#1000",
        "created_at": "a",
        "updated_at": "b"
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
