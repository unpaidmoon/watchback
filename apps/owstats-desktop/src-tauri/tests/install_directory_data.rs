use std::fs;

use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::install_data_migration::{
    prepare_install_data, InstallDataMigrationOutcome,
};

#[cfg(windows)]
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use owstats_desktop_lib::install_data_migration::{
    set_windows_migration_hook, WindowsMigrationHookPoint,
};

#[cfg(not(windows))]
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

#[cfg(not(windows))]
use owstats_desktop_lib::install_data_migration::{
    set_non_windows_migration_hook, NonWindowsMigrationHookPoint,
};

#[cfg(windows)]
#[derive(Debug)]
enum MigrationRace {
    ReplaceStaging {
        staging: PathBuf,
        displaced: PathBuf,
        external: PathBuf,
        rename_succeeded: Option<bool>,
        junction_created: Option<bool>,
    },
    CreateDestination {
        destination: PathBuf,
        invoked: bool,
    },
}

#[cfg(windows)]
thread_local! {
    static COPY_OPEN_RACE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static MIGRATION_RACE: RefCell<Option<MigrationRace>> = const { RefCell::new(None) };
}

#[cfg(windows)]
struct CopyOpenHookReset;

#[cfg(windows)]
impl Drop for CopyOpenHookReset {
    fn drop(&mut self) {
        owstats_desktop_lib::install_data_migration::set_windows_copy_open_hook(None);
        COPY_OPEN_RACE.with(|race| {
            race.borrow_mut().take();
        });
    }
}

#[cfg(windows)]
struct MigrationHookReset;

#[cfg(windows)]
impl Drop for MigrationHookReset {
    fn drop(&mut self) {
        set_windows_migration_hook(None);
        MIGRATION_RACE.with(|race| {
            race.borrow_mut().take();
        });
    }
}

#[cfg(windows)]
fn replace_selected_source_with_junction(path: &Path) {
    let external = COPY_OPEN_RACE.with(|race| race.borrow().clone());
    let Some(external) = external else {
        return;
    };
    if path.file_name().and_then(|name| name.to_str()) != Some("z-raced") {
        return;
    }
    fs::remove_file(path).unwrap();
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(path)
        .arg(&external)
        .output()
        .unwrap();
    assert!(output.status.success(), "无法创建合成 TOCTOU junction 输入");
}

#[cfg(windows)]
fn trigger_selected_migration_race(
    point: WindowsMigrationHookPoint,
    staging_path: &Path,
    destination_path: &Path,
) {
    MIGRATION_RACE.with(|race| {
        let mut race = race.borrow_mut();
        match race.as_mut() {
            Some(MigrationRace::ReplaceStaging {
                staging,
                displaced,
                external,
                rename_succeeded,
                junction_created,
            }) if point == WindowsMigrationHookPoint::BeforePublish && staging_path == staging => {
                let renamed = fs::rename(staging_path, displaced).is_ok();
                *rename_succeeded = Some(renamed);
                if renamed {
                    let output = std::process::Command::new("cmd")
                        .args(["/C", "mklink", "/J"])
                        .arg(staging_path)
                        .arg(external)
                        .output()
                        .unwrap();
                    *junction_created = Some(output.status.success());
                }
            }
            Some(MigrationRace::CreateDestination {
                destination,
                invoked,
            }) if point == WindowsMigrationHookPoint::BeforePublish
                && destination_path == destination =>
            {
                fs::create_dir(destination_path).unwrap();
                *invoked = true;
            }
            _ => {}
        }
    });
}

#[cfg(not(windows))]
#[derive(Debug)]
struct NonWindowsMigrationRace {
    destination: PathBuf,
    invoked: bool,
}

#[cfg(not(windows))]
thread_local! {
    static NON_WINDOWS_MIGRATION_RACE: RefCell<Option<NonWindowsMigrationRace>> =
        const { RefCell::new(None) };
}

#[cfg(not(windows))]
struct NonWindowsMigrationHookReset;

#[cfg(not(windows))]
impl Drop for NonWindowsMigrationHookReset {
    fn drop(&mut self) {
        set_non_windows_migration_hook(None);
        NON_WINDOWS_MIGRATION_RACE.with(|race| {
            race.borrow_mut().take();
        });
    }
}

#[cfg(not(windows))]
fn create_destination_immediately_before_publish(
    point: NonWindowsMigrationHookPoint,
    _staging_path: &Path,
    destination_path: &Path,
) {
    NON_WINDOWS_MIGRATION_RACE.with(|race| {
        let mut race = race.borrow_mut();
        let Some(race) = race.as_mut() else {
            return;
        };
        if point == NonWindowsMigrationHookPoint::BeforePublish
            && destination_path == race.destination
        {
            fs::create_dir(destination_path).unwrap();
            race.invoked = true;
        }
    });
}

#[cfg(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
))]
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
    symlink_file(&external, &source).expect("当前 Windows 环境必须允许合成 symlink 测试输入");
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

    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&external)
        .output()
        .unwrap();
    assert!(output.status.success(), "无法创建合成 junction 测试输入");

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
    symlink_file(&external, &linked).expect("当前 Windows 环境必须允许合成 symlink 测试输入");

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(fs::read(&external).unwrap(), b"external");
    assert!(linked.symlink_metadata().is_ok());
    assert!(!data.exists());
}

#[cfg(windows)]
#[test]
fn rejects_a_reparse_point_created_between_enumeration_and_handle_open() {
    use owstats_desktop_lib::install_data_migration::set_windows_copy_open_hook;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let raced = source.join("z-raced");
    let external = temp.path().join("external");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&external).unwrap();
    fs::write(source.join("a-copied.txt"), b"copied").unwrap();
    fs::write(&raced, b"replace me").unwrap();
    fs::write(external.join("external.txt"), b"external").unwrap();
    COPY_OPEN_RACE.with(|race| {
        *race.borrow_mut() = Some(external.clone());
    });
    set_windows_copy_open_hook(Some(replace_selected_source_with_junction));
    let hook_reset = CopyOpenHookReset;

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    set_windows_copy_open_hook(None);
    COPY_OPEN_RACE.with(|race| {
        race.borrow_mut().take();
    });
    drop(hook_reset);
    assert!(result.is_err());
    assert_eq!(
        fs::read(external.join("external.txt")).unwrap(),
        b"external"
    );
    assert!(!data.exists());
    assert_eq!(fs::read(source.join("a-copied.txt")).unwrap(), b"copied");
}

#[cfg(all(
    not(windows),
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
#[test]
fn non_windows_publish_refuses_a_destination_created_after_copy_and_retries_safely() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    NON_WINDOWS_MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(NonWindowsMigrationRace {
            destination: data.clone(),
            invoked: false,
        });
    });
    set_non_windows_migration_hook(Some(create_destination_immediately_before_publish));
    let hook_reset = NonWindowsMigrationHookReset;

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    set_non_windows_migration_hook(None);
    let race = NON_WINDOWS_MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    assert!(race.invoked);
    assert!(result.is_err());
    assert!(data.is_dir());
    assert_eq!(fs::read_dir(&data).unwrap().count(), 0);
    assert!(!data.join("source.txt").exists());
    assert_eq!(fs::read(staging.join("source.txt")).unwrap(), b"source");
    assert!(staging.join(".install-data-migration-v1.json").is_file());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert!(staging.is_dir());

    fs::remove_dir(&data).unwrap();
    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("source.txt")).unwrap(), b"source");
    assert!(!staging.exists());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
}

#[cfg(not(windows))]
#[test]
fn non_windows_no_source_refuses_a_destination_created_after_checks() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    NON_WINDOWS_MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(NonWindowsMigrationRace {
            destination: data.clone(),
            invoked: false,
        });
    });
    set_non_windows_migration_hook(Some(create_destination_immediately_before_publish));
    let hook_reset = NonWindowsMigrationHookReset;

    let result = prepare_install_data(None, &SharedAppPaths::from_data_root(&data));

    set_non_windows_migration_hook(None);
    let race = NON_WINDOWS_MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    assert!(race.invoked);
    assert!(result.is_err());
    assert!(data.is_dir());
    assert_eq!(fs::read_dir(&data).unwrap().count(), 0);
    assert!(!staging.exists());

    let outcome = prepare_install_data(None, &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
}

#[cfg(all(
    not(windows),
    not(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    ))
))]
#[test]
fn unsupported_non_windows_publish_fails_safely_and_retains_source_and_staging() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    assert!(result.is_err());
    assert!(!data.exists());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert_eq!(fs::read(staging.join("source.txt")).unwrap(), b"source");
    assert!(staging.join(".install-data-migration-v1.json").is_file());
}

#[cfg(windows)]
#[test]
fn keeps_staging_fixed_until_handle_publish() {
    use std::os::windows::fs::MetadataExt;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let displaced = data.with_file_name("displaced-staging");
    let external = temp.path().join("external");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&external).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    fs::write(external.join("external.txt"), b"external").unwrap();
    MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(MigrationRace::ReplaceStaging {
            staging: staging.clone(),
            displaced: displaced.clone(),
            external: external.clone(),
            rename_succeeded: None,
            junction_created: None,
        });
    });
    set_windows_migration_hook(Some(trigger_selected_migration_race));
    let hook_reset = MigrationHookReset;

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    set_windows_migration_hook(None);
    let race = MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    let MigrationRace::ReplaceStaging {
        rename_succeeded,
        junction_created,
        ..
    } = race
    else {
        panic!("暂存目录替换测试状态错误");
    };
    assert_eq!(result.unwrap(), InstallDataMigrationOutcome::Migrated);
    assert_eq!(rename_succeeded, Some(false));
    assert_eq!(junction_created, None);
    assert!(!displaced.exists());
    assert_eq!(
        fs::symlink_metadata(&data).unwrap().file_attributes() & 0x400,
        0
    );
    assert_eq!(fs::read(data.join("source.txt")).unwrap(), b"source");
    assert!(data.join(".install-data-migration-v1.json").is_file());
    assert_eq!(
        fs::read(external.join("external.txt")).unwrap(),
        b"external"
    );
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
}

#[cfg(windows)]
#[test]
fn refuses_a_destination_created_immediately_before_handle_publish_and_retries_safely() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(MigrationRace::CreateDestination {
            destination: data.clone(),
            invoked: false,
        });
    });
    set_windows_migration_hook(Some(trigger_selected_migration_race));
    let hook_reset = MigrationHookReset;

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    set_windows_migration_hook(None);
    let race = MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    let MigrationRace::CreateDestination { invoked, .. } = race else {
        panic!("发布竞态测试状态错误");
    };
    assert!(invoked);
    assert!(result.is_err());
    assert!(data.is_dir());
    assert_eq!(fs::read_dir(&data).unwrap().count(), 0);
    assert!(!data.join("source.txt").exists());
    assert_eq!(fs::read(staging.join("source.txt")).unwrap(), b"source");
    assert!(staging.join(".install-data-migration-v1.json").is_file());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
    assert!(staging.is_dir());

    fs::remove_dir(&data).unwrap();
    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("source.txt")).unwrap(), b"source");
    assert!(!staging.exists());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
}

#[cfg(windows)]
#[test]
fn no_source_refuses_a_destination_created_after_checks() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(MigrationRace::CreateDestination {
            destination: data.clone(),
            invoked: false,
        });
    });
    set_windows_migration_hook(Some(trigger_selected_migration_race));
    let hook_reset = MigrationHookReset;

    let result = prepare_install_data(None, &SharedAppPaths::from_data_root(&data));

    set_windows_migration_hook(None);
    let race = MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    let MigrationRace::CreateDestination { invoked, .. } = race else {
        panic!("空数据目录竞态测试状态错误");
    };
    assert!(invoked);
    assert!(result.is_err());
    assert!(data.is_dir());
    assert_eq!(fs::read_dir(&data).unwrap().count(), 0);
    assert!(!staging.exists());

    let outcome = prepare_install_data(None, &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::ExistingData);
}

#[cfg(windows)]
#[test]
fn rejects_a_reparse_point_in_existing_staging_without_touching_its_target() {
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
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&staging)
        .arg(&external)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "无法创建合成 staging junction 输入"
    );

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert_eq!(
        fs::read(external.join("external.txt")).unwrap(),
        b"external"
    );
    assert!(staging.symlink_metadata().is_ok());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
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

#[cfg(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
))]
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
