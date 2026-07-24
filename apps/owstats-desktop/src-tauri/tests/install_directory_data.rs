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

    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(data.join(".install-data-migration-v1.json")).unwrap())
            .unwrap();
    let marker = marker.as_object().unwrap();
    assert_eq!(marker.len(), 2);
    assert_eq!(marker.get("version").unwrap(), 1);
    assert_eq!(marker.get("sourceRetained").unwrap(), true);
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
    assert_eq!(fs::read(data.join("new.txt")).unwrap(), b"new");
}

#[test]
fn no_source_creates_an_empty_data_directory() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");

    let outcome = prepare_install_data(None, &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Created);
    assert!(data.is_dir());
}

#[test]
fn existing_data_wins_when_the_source_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("missing");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::write(data.join("existing.txt"), b"existing").unwrap();
    fs::write(staging.join("stale.txt"), b"stale").unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert_eq!(fs::read(data.join("existing.txt")).unwrap(), b"existing");
    assert_eq!(fs::read(staging.join("stale.txt")).unwrap(), b"stale");
}

#[test]
fn existing_data_wins_when_the_source_is_an_unsupported_file() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("unsupported.txt");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::write(&source, b"unsupported").unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::write(data.join("existing.txt"), b"existing").unwrap();
    fs::write(staging.join("stale.txt"), b"stale").unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert_eq!(fs::read(data.join("existing.txt")).unwrap(), b"existing");
    assert_eq!(fs::read(staging.join("stale.txt")).unwrap(), b"stale");
}

#[cfg(windows)]
#[test]
fn existing_data_wins_when_the_source_is_a_symlink() {
    use std::os::windows::fs::symlink_file;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("linked.txt");
    let external = temp.path().join("external.txt");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::write(&external, b"external").unwrap();
    if symlink_file(&external, &source).is_err() {
        return;
    }
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::write(data.join("existing.txt"), b"existing").unwrap();
    fs::write(staging.join("stale.txt"), b"stale").unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert_eq!(fs::read(data.join("existing.txt")).unwrap(), b"existing");
    assert_eq!(fs::read(staging.join("stale.txt")).unwrap(), b"stale");
    assert_eq!(fs::read(&external).unwrap(), b"external");
}

#[test]
fn existing_data_wins_when_the_source_overlaps_staging() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&data).unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::write(data.join("existing.txt"), b"existing").unwrap();
    fs::write(staging.join("stale.txt"), b"stale").unwrap();

    let outcome =
        prepare_install_data(Some(&staging), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert_eq!(fs::read(data.join("existing.txt")).unwrap(), b"existing");
    assert_eq!(fs::read(staging.join("stale.txt")).unwrap(), b"stale");
}

#[test]
fn rejects_a_source_that_is_the_staging_directory_without_mutating_it() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("source.txt"), b"source").unwrap();

    assert!(prepare_install_data(Some(&staging), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(fs::read(staging.join("source.txt")).unwrap(), b"source");
    assert!(!data.exists());
}

#[test]
fn rejects_a_source_that_contains_the_staging_directory_without_mutating_it() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("install");
    let data = source.join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert!(!data.exists());
    assert!(!data.with_file_name("data.migrating").exists());
}

#[test]
fn rejects_a_source_inside_the_staging_directory_without_mutating_it() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let source = staging.join("nested-source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert!(!data.exists());
    assert!(staging.is_dir());
}

#[cfg(unix)]
#[test]
fn rejects_a_symlink_after_copying_without_publishing_data_or_mutating_the_source() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let external = temp.path().join("external.txt");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("a-copied.txt"), b"copied").unwrap();
    fs::write(&external, b"external").unwrap();
    symlink(&external, source.join("z-linked.txt")).unwrap();

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(fs::read(&external).unwrap(), b"external");
    assert!(source.join("z-linked.txt").symlink_metadata().is_ok());
    assert_eq!(fs::read(source.join("a-copied.txt")).unwrap(), b"copied");
    assert!(!data.exists());
    assert_eq!(
        fs::read(data.with_file_name("data.migrating").join("a-copied.txt")).unwrap(),
        b"copied"
    );
}

#[cfg(unix)]
#[test]
fn rejects_a_special_file_without_publishing_data_or_mutating_the_source() {
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::net::UnixListener;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("ordinary.txt"), b"ordinary").unwrap();
    let socket = UnixListener::bind(source.join("socket")).unwrap();

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert!(source
        .join("socket")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_socket());
    assert_eq!(fs::read(source.join("ordinary.txt")).unwrap(), b"ordinary");
    assert!(!data.exists());
    drop(socket);
}

#[cfg(windows)]
#[test]
fn rejects_a_windows_junction_without_mutating_its_target_or_publishing_data() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let external = temp.path().join("external");
    let junction = source.join("junction");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("external.txt"), b"external").unwrap();

    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&external)
        .status()
        .unwrap();
    if !status.success() {
        return;
    }

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(
        fs::read(external.join("external.txt")).unwrap(),
        b"external"
    );
    assert!(junction.symlink_metadata().is_ok());
    assert!(!data.exists());
}

#[cfg(windows)]
#[test]
fn rejects_a_windows_symlink_without_mutating_its_target_or_publishing_data() {
    use std::os::windows::fs::symlink_file;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let external = temp.path().join("external.txt");
    let linked = source.join("linked.txt");
    fs::create_dir_all(&source).unwrap();
    fs::write(&external, b"external").unwrap();
    if symlink_file(&external, &linked).is_err() {
        return;
    }

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(fs::read(&external).unwrap(), b"external");
    assert!(linked.symlink_metadata().is_ok());
    assert!(!data.exists());
}

#[cfg(unix)]
#[test]
fn rejects_a_malicious_staging_symlink_without_touching_its_target() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let external = temp.path().join("external");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&external).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    fs::write(external.join("external.txt"), b"external").unwrap();
    fs::create_dir_all(staging.parent().unwrap()).unwrap();
    symlink(&external, &staging).unwrap();

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(
        fs::read(external.join("external.txt")).unwrap(),
        b"external"
    );
    assert!(staging.symlink_metadata().is_ok());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert!(!data.exists());
}

#[test]
fn resets_only_the_exact_staging_directory_before_publishing() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    fs::write(staging.join("stale.txt"), b"stale").unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("source.txt")).unwrap(), b"source");
    assert!(!data.join("stale.txt").exists());
    assert!(!staging.exists());
}
