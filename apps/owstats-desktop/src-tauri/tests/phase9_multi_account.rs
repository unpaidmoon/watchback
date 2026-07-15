use owstats_desktop_lib::account_context::AccountContextManager;
use owstats_desktop_lib::account_migration::{
    LegacyMigrationState, MigrationJournal, MigrationPhase,
};
use owstats_desktop_lib::account_store::{
    read_manifest, read_registry, write_manifest_atomic, write_registry_atomic, AccountId,
    AccountManifest, AccountRegistry, AccountRegistryEntry,
};
use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::{list_matches, CredentialStatus, MatchFilters, PlayerProfile};
use rusqlite::params;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

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
        let staging = fixture.staging(&account_id);
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("partial"), "synthetic").unwrap();
        fixture.write_journal(&account_id, phase);
        fs::remove_file(fixture.shared.registry()).unwrap();
        fs::remove_file(account.paths().manifest()).unwrap();
        fs::remove_file(account.paths().credentials()).unwrap();

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
    let snapshot = fixture.manager.active_snapshot().unwrap();

    assert!(fixture.manager.adopt_legacy(&snapshot).is_err());
    assert!(fixture.shared.legacy_database().is_file());
    assert!(!external.join("owstats.sqlite3").exists());
}

#[cfg(windows)]
fn create_directory_symlink(source: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::windows::fs::symlink_dir(source, target).is_ok()
}

#[cfg(unix)]
fn create_directory_symlink(source: &std::path::Path, target: &std::path::Path) -> bool {
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
fn account_database_privacy_migration_uses_explicit_seasons_and_backups_paths() {
    let temp = tempfile::tempdir().unwrap();
    let shared = SharedAppPaths::from_app_data_dir(temp.path());
    let account_id = AccountId::parse("d59caecf-8e74-4b6c-b690-45e75f7fed9c").unwrap();
    let account = shared.account(&account_id);
    fs::create_dir_all(account.seasons()).unwrap();
    fs::write(
        account.seasons().join("22.json"),
        r#"{"safe":"ok","token":"synthetic-season-secret"}"#,
    )
    .unwrap();

    let db = owstats_desktop_lib::OwDatabase::for_account(&account);
    db.initialize().unwrap();

    assert!(
        !account.root().join("privacy-backups").exists(),
        "account initialization must not derive a privacy-backups sibling"
    );
    let backup_dirs = fs::read_dir(account.backups())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    assert_eq!(backup_dirs.len(), 1);
    assert!(backup_dirs[0].join("seasons").join("22.json").is_file());

    let sanitized = fs::read_to_string(account.seasons().join("22.json")).unwrap();
    assert!(!sanitized.contains("synthetic-season-secret"));
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

#[test]
fn account_mutating_commands_acquire_permit_before_resolving_snapshot() {
    let source = include_str!("../src/commands.rs");
    for command in [
        "refresh_match_evaluations",
        "logout",
        "start_dashen_login",
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
