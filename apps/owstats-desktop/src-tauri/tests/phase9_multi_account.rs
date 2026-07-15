use owstats_desktop_lib::account_context::AccountContextManager;
use owstats_desktop_lib::account_store::{
    read_manifest, read_registry, write_manifest_atomic, write_registry_atomic, AccountId,
    AccountManifest, AccountRegistry, AccountRegistryEntry,
};
use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::{list_matches, CredentialStatus, MatchFilters, PlayerProfile};
use rusqlite::params;
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

fn command_source<'a>(source: &'a str, command: &str) -> &'a str {
    let sync_signature = format!("pub async fn {command}");
    let regular_signature = format!("pub fn {command}");
    let start = source
        .find(&sync_signature)
        .or_else(|| source.find(&regular_signature))
        .unwrap_or_else(|| panic!("missing command {command}"));
    let rest = &source[start..];
    let end = rest.find("\n#[tauri::command]").unwrap_or(rest.len());
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
