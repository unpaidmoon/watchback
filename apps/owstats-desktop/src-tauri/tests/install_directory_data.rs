use std::fs;

use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::install_data_migration::{
    prepare_install_data, InstallDataMigrationOutcome,
};

#[test]
fn copies_source_atomically_and_retains_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("user_settings.json"), b"{}").unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(source.join("user_settings.json").is_file());
    assert!(data.join("user_settings.json").is_file());
    assert!(data.join(".install-data-migration-v1.json").is_file());
    assert!(!data.with_file_name("data.migrating").exists());
}

#[test]
fn existing_data_is_never_overwritten() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::write(source.join("old.txt"), b"old").unwrap();
    fs::write(data.join("new.txt"), b"new").unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert!(!data.join("old.txt").exists());
}

#[test]
fn no_source_creates_an_empty_data_directory() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");

    let outcome = prepare_install_data(None, &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Created);
    assert!(data.is_dir());
}
