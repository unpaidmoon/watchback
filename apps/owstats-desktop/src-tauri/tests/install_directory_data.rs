use std::fs;

#[cfg(windows)]
use std::{
    process::Command,
    time::{Duration, Instant},
};

use rusqlite::Connection;

use owstats_desktop_lib::app_paths::SharedAppPaths;
use owstats_desktop_lib::install_data_migration::{
    prepare_install_data, InstallDataMigrationOutcome,
};

#[cfg(windows)]
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
};

#[cfg(windows)]
use owstats_desktop_lib::install_data_migration::{
    set_windows_cleanup_enumeration_hook, set_windows_cleanup_limits_for_test,
    set_windows_copy_enumeration_hook, set_windows_copy_limits_for_test,
    set_windows_migration_hook, set_windows_parent_guard_hook, WindowsCleanupLimits,
    WindowsCopyLimits, WindowsMigrationHookPoint,
};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{LocalFree, ERROR_SUCCESS, HANDLE, INVALID_HANDLE_VALUE},
    Security::{
        Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
            SE_FILE_OBJECT,
        },
        CreateRestrictedToken, CreateWellKnownSid, GetSecurityDescriptorControl,
        ImpersonateLoggedOnUser, RevertToSelf, WinBuiltinAdministratorsSid, WinLocalSystemSid,
        DACL_SECURITY_INFORMATION, DISABLE_MAX_PRIVILEGE, SECURITY_MAX_SID_SIZE, SE_DACL_PROTECTED,
        SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_QUERY, WELL_KNOWN_SID_TYPE,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING, READ_CONTROL,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
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
    LockStagedFile {
        relative: PathBuf,
        lock: Option<fs::File>,
        invoked: bool,
    },
    ReplaceValidatedFile {
        relative: PathBuf,
        replacement: Vec<u8>,
        write_succeeded: Option<bool>,
    },
    ProbeInitialStagingAccess {
        relative: PathBuf,
        write_succeeded: Option<bool>,
    },
}

#[cfg(windows)]
#[derive(Debug)]
struct InstallParentGuardRace {
    install: PathBuf,
    displaced: PathBuf,
    rename_succeeded: Option<bool>,
}

#[cfg(windows)]
thread_local! {
    static INSTALL_PARENT_GUARD_RACE: RefCell<Option<InstallParentGuardRace>> = const { RefCell::new(None) };
}

#[cfg(windows)]
thread_local! {
    static ENUMERATED_CLEANUP_ENTRIES: Cell<usize> = const { Cell::new(0) };
    static COPY_OPEN_RACE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static ENUMERATED_SOURCE_ENTRIES: Cell<usize> = const { Cell::new(0) };
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
struct CopyLimitsReset;

#[cfg(windows)]
impl Drop for CopyLimitsReset {
    fn drop(&mut self) {
        set_windows_copy_limits_for_test(None);
    }
}

#[cfg(windows)]
fn install_copy_limits(limits: WindowsCopyLimits) -> CopyLimitsReset {
    set_windows_copy_limits_for_test(Some(limits));
    CopyLimitsReset
}

#[cfg(windows)]
struct CleanupLimitsReset;

#[cfg(windows)]
impl Drop for CleanupLimitsReset {
    fn drop(&mut self) {
        set_windows_cleanup_limits_for_test(None);
    }
}

#[cfg(windows)]
fn install_cleanup_limits(limits: WindowsCleanupLimits) -> CleanupLimitsReset {
    set_windows_cleanup_limits_for_test(Some(limits));
    CleanupLimitsReset
}

#[cfg(windows)]
struct CleanupEnumerationHookReset;

#[cfg(windows)]
impl Drop for CleanupEnumerationHookReset {
    fn drop(&mut self) {
        set_windows_cleanup_enumeration_hook(None);
        ENUMERATED_CLEANUP_ENTRIES.with(|count| count.set(0));
    }
}

#[cfg(windows)]
fn count_enumerated_cleanup_entry() {
    ENUMERATED_CLEANUP_ENTRIES.with(|count| count.set(count.get() + 1));
}

#[cfg(windows)]
fn install_cleanup_enumeration_counter() -> CleanupEnumerationHookReset {
    ENUMERATED_CLEANUP_ENTRIES.with(|count| count.set(0));
    set_windows_cleanup_enumeration_hook(Some(count_enumerated_cleanup_entry));
    CleanupEnumerationHookReset
}

#[cfg(windows)]
struct EnumerationHookReset;

#[cfg(windows)]
impl Drop for EnumerationHookReset {
    fn drop(&mut self) {
        set_windows_copy_enumeration_hook(None);
        ENUMERATED_SOURCE_ENTRIES.with(|count| count.set(0));
    }
}

#[cfg(windows)]
fn count_enumerated_source_entry() {
    ENUMERATED_SOURCE_ENTRIES.with(|count| count.set(count.get() + 1));
}

#[cfg(windows)]
fn install_enumeration_counter() -> EnumerationHookReset {
    ENUMERATED_SOURCE_ENTRIES.with(|count| count.set(0));
    set_windows_copy_enumeration_hook(Some(count_enumerated_source_entry));
    EnumerationHookReset
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
struct LocalAllocation(*mut c_void);

#[cfg(windows)]
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(self.0);
        }
    }
}

#[cfg(windows)]
struct ImpersonationReset;

#[cfg(windows)]
impl Drop for ImpersonationReset {
    fn drop(&mut self) {
        unsafe {
            let _ = RevertToSelf();
        }
    }
}

#[cfg(windows)]
fn owned_handle(raw: HANDLE) -> OwnedHandle {
    assert!(!raw.is_null());
    assert_ne!(raw, INVALID_HANDLE_VALUE);
    unsafe { OwnedHandle::from_raw_handle(raw as _) }
}

#[cfg(windows)]
fn well_known_sid(kind: WELL_KNOWN_SID_TYPE) -> Vec<u8> {
    let mut sid = vec![0_u8; SECURITY_MAX_SID_SIZE as usize];
    let mut length = SECURITY_MAX_SID_SIZE;
    let created = unsafe {
        CreateWellKnownSid(
            kind,
            std::ptr::null_mut(),
            sid.as_mut_ptr().cast(),
            &mut length,
        )
    };
    assert_ne!(created, 0);
    sid.truncate(length as usize);
    sid
}

#[cfg(windows)]
fn with_privileged_sids_disabled<T>(operation: impl FnOnce() -> T) -> T {
    let mut process_token = std::ptr::null_mut();
    let opened = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY,
            &mut process_token,
        )
    };
    assert_ne!(opened, 0);
    let process_token = owned_handle(process_token);

    let mut administrators = well_known_sid(WinBuiltinAdministratorsSid);
    let mut system = well_known_sid(WinLocalSystemSid);
    let disabled = [
        SID_AND_ATTRIBUTES {
            Sid: administrators.as_mut_ptr().cast(),
            Attributes: 0,
        },
        SID_AND_ATTRIBUTES {
            Sid: system.as_mut_ptr().cast(),
            Attributes: 0,
        },
    ];
    let mut restricted_token = std::ptr::null_mut();
    let restricted = unsafe {
        CreateRestrictedToken(
            process_token.as_raw_handle() as _,
            DISABLE_MAX_PRIVILEGE,
            disabled.len() as u32,
            disabled.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            &mut restricted_token,
        )
    };
    assert_ne!(restricted, 0);
    let restricted_token = owned_handle(restricted_token);
    let impersonated = unsafe { ImpersonateLoggedOnUser(restricted_token.as_raw_handle() as _) };
    assert_ne!(impersonated, 0);
    let reset = ImpersonationReset;

    let result = operation();

    let reverted = unsafe { RevertToSelf() };
    assert_ne!(reverted, 0);
    std::mem::forget(reset);
    result
}

#[cfg(windows)]
fn protected_dacl_sddl(path: &Path) -> (u16, String) {
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    let handle = owned_handle(raw);
    let mut descriptor = std::ptr::null_mut();
    let status = unsafe {
        GetSecurityInfo(
            handle.as_raw_handle() as _,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    assert_eq!(status, ERROR_SUCCESS);
    assert!(!descriptor.is_null());
    let descriptor_guard = LocalAllocation(descriptor);
    let mut control = 0_u16;
    let mut revision = 0_u32;
    let read_control =
        unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) };
    assert_ne!(read_control, 0);

    let mut sddl = std::ptr::null_mut();
    let mut length = 0_u32;
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut sddl,
            &mut length,
        )
    };
    assert_ne!(converted, 0);
    assert!(!sddl.is_null());
    let sddl_guard = LocalAllocation(sddl.cast());
    let value =
        String::from_utf16(unsafe { std::slice::from_raw_parts(sddl, length as usize) }).unwrap();
    drop(sddl_guard);
    drop(descriptor_guard);
    (control, value)
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
            Some(MigrationRace::LockStagedFile {
                relative,
                lock,
                invoked,
            }) if point == WindowsMigrationHookPoint::BeforeValidation => {
                *lock = Some(
                    fs::OpenOptions::new()
                        .read(true)
                        .share_mode(0)
                        .open(staging_path.join(relative))
                        .unwrap(),
                );
                *invoked = true;
            }
            Some(MigrationRace::ReplaceValidatedFile {
                relative,
                replacement,
                write_succeeded,
            }) if point == WindowsMigrationHookPoint::BeforePublish => {
                let target = staging_path.join(relative);
                let result = with_privileged_sids_disabled(|| fs::write(target, replacement));
                *write_succeeded = Some(result.is_ok());
            }
            Some(MigrationRace::ProbeInitialStagingAccess {
                relative,
                write_succeeded,
            }) if point == WindowsMigrationHookPoint::StagingCreated => {
                let target = staging_path.join(relative);
                let result =
                    with_privileged_sids_disabled(|| fs::write(target, b"attacker controlled"));
                *write_succeeded = Some(result.is_ok());
            }
            _ => {}
        }
    });
}

#[cfg(windows)]
fn replace_install_parent_after_authorization(_parent: &Path) {
    INSTALL_PARENT_GUARD_RACE.with(|race| {
        let mut race = race.borrow_mut();
        let Some(race) = race.as_mut() else {
            return;
        };
        race.rename_succeeded = Some(fs::rename(&race.install, &race.displaced).is_ok());
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

fn write_valid_account_metadata(source: &Path) -> (String, PathBuf) {
    let account_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned();
    let account = source.join("accounts").join(&account_id);
    fs::create_dir_all(&account).unwrap();
    fs::write(
        source.join("accounts.json"),
        format!(
            r#"{{"version":1,"activeAccountId":"{account_id}","accounts":[{{"accountId":"{account_id}","battleTag":"合成账号#0000","createdAt":"2026-01-01T00:00:00Z","lastUsedAt":"2026-01-01T00:00:00Z","credentialState":"ready"}}]}}"#
        ),
    )
    .unwrap();
    fs::write(
        account.join("account.json"),
        format!(
            r#"{{"version":1,"accountId":"{account_id}","roleId":1,"battleTag":"合成账号#0000","createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z"}}"#
        ),
    )
    .unwrap();
    (account_id, account)
}

fn create_valid_sqlite(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch("CREATE TABLE synthetic_records (value INTEGER); INSERT INTO synthetic_records VALUES (1);")
        .unwrap();
}

fn assert_no_validation_artifacts(root: &Path) {
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        assert!(
            !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".install-data-validation-"),
            "发布树中残留数据库验证目录"
        );
        if entry.file_type().unwrap().is_dir() {
            assert_no_validation_artifacts(&entry.path());
        }
    }
}

#[test]
fn valid_account_metadata_and_sqlite_are_published() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    let (account_id, account) = write_valid_account_metadata(&source);
    create_valid_sqlite(&account.join("owstats.sqlite3"));

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(data
        .join("accounts")
        .join(account_id)
        .join("account.json")
        .is_file());
    assert!(data
        .join("accounts")
        .join("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
        .join("owstats.sqlite3")
        .is_file());
}

#[test]
fn two_accounts_with_independent_manifests_are_published() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let first_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let second_id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("accounts.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "version": 1,
            "activeAccountId": first_id,
            "accounts": [
                {
                    "accountId": first_id,
                    "battleTag": "合成账号甲#0000",
                    "createdAt": "2026-01-01T00:00:00Z",
                    "lastUsedAt": "2026-01-02T00:00:00Z",
                    "credentialState": "ready"
                },
                {
                    "accountId": second_id,
                    "battleTag": "合成账号乙#0000",
                    "createdAt": "2026-01-01T00:00:00Z",
                    "lastUsedAt": "2026-01-01T00:00:00Z",
                    "credentialState": "ready"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    for (account_id, role_id, battle_tag) in [
        (first_id, 1_u64, "合成账号甲#0000"),
        (second_id, 2_u64, "合成账号乙#0000"),
    ] {
        let account = source.join("accounts").join(account_id);
        fs::create_dir_all(&account).unwrap();
        fs::write(
            account.join("account.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 1,
                "accountId": account_id,
                "roleId": role_id,
                "battleTag": battle_tag,
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-02T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();
    }
    create_valid_sqlite(
        &source
            .join("accounts")
            .join(first_id)
            .join("owstats.sqlite3"),
    );

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    for account_id in [first_id, second_id] {
        assert!(data
            .join("accounts")
            .join(account_id)
            .join("account.json")
            .is_file());
    }
    assert!(data
        .join("accounts")
        .join(first_id)
        .join("owstats.sqlite3")
        .is_file());
    assert!(!data
        .join("accounts")
        .join(second_id)
        .join("owstats.sqlite3")
        .exists());
}

#[test]
fn valid_wal_database_is_published_without_validation_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    let (account_id, account) = write_valid_account_metadata(&source);
    let database = account.join("owstats.sqlite3");
    let writer = Connection::open(&database).unwrap();
    writer
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE synthetic_wal (value INTEGER); INSERT INTO synthetic_wal VALUES (1);",
        )
        .unwrap();
    let mut persist_wal = 1;
    let result = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            writer.handle(),
            b"main\0".as_ptr().cast(),
            rusqlite::ffi::SQLITE_FCNTL_PERSIST_WAL,
            (&mut persist_wal as *mut i32).cast(),
        )
    };
    assert_eq!(result, rusqlite::ffi::SQLITE_OK);
    drop(writer);
    assert!(database.with_file_name("owstats.sqlite3-wal").is_file());

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(data
        .join("accounts")
        .join(account_id)
        .join("owstats.sqlite3-wal")
        .is_file());
    assert_no_validation_artifacts(&data);
}

#[test]
fn quick_check_corruption_is_not_published_and_retains_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&source).unwrap();
    let (_, account) = write_valid_account_metadata(&source);
    let database = account.join("owstats.sqlite3");
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "PRAGMA page_size=512; VACUUM; CREATE TABLE synthetic_corruption (value BLOB); INSERT INTO synthetic_corruption VALUES (zeroblob(4096));",
        )
        .unwrap();
    drop(connection);
    let mut bytes = fs::read(&database).unwrap();
    bytes[512] = 0;
    fs::write(&database, bytes).unwrap();

    let error = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data))
        .unwrap_err()
        .to_string();

    assert!(error.contains("旧账户数据库完整性校验失败"));
    assert!(!data.exists());
    assert!(database.is_file());
    assert!(!staging.join(".install-data-migration-v1.json").exists());
}

#[test]
fn invalid_registry_is_not_published_and_source_is_retained() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("accounts.json"), b"{invalid").unwrap();

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert!(!data.exists());
    assert!(source.join("accounts.json").is_file());
}

#[test]
fn empty_valid_registry_does_not_require_an_accounts_directory() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("accounts.json"),
        br#"{"version":1,"activeAccountId":null,"accounts":[]}"#,
    )
    .unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(data.join("accounts.json").is_file());
}

#[test]
fn invalid_sqlite_is_not_published_and_source_is_retained() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let account_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let account = source.join("accounts").join(account_id);
    fs::create_dir_all(&account).unwrap();
    fs::write(
        source.join("accounts.json"),
        format!(
            r#"{{"version":1,"activeAccountId":"{account_id}","accounts":[{{"accountId":"{account_id}","battleTag":"合成账号#0000","createdAt":"2026-01-01T00:00:00Z","lastUsedAt":"2026-01-01T00:00:00Z","credentialState":"ready"}}]}}"#
        ),
    )
    .unwrap();
    fs::write(
        account.join("account.json"),
        format!(
            r#"{{"version":1,"accountId":"{account_id}","roleId":1,"battleTag":"合成账号#0000","createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z"}}"#
        ),
    )
    .unwrap();
    fs::write(account.join("owstats.sqlite3"), b"not sqlite").unwrap();

    assert!(prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).is_err());

    assert!(!data.exists());
    assert_eq!(
        fs::read(account.join("owstats.sqlite3")).unwrap(),
        b"not sqlite"
    );
}

#[test]
fn account_without_a_main_database_is_valid() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    let (account_id, _) = write_valid_account_metadata(&source);

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(data
        .join("accounts")
        .join(account_id)
        .join("account.json")
        .is_file());
    assert!(!data
        .join("accounts")
        .join("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
        .join("owstats.sqlite3")
        .exists());
}

#[cfg(windows)]
fn assert_locked_optional_validation_file_is_rejected(setup: impl FnOnce(&Path) -> PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    fs::create_dir_all(&source).unwrap();
    let relative = setup(&source);
    MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(MigrationRace::LockStagedFile {
            relative,
            lock: None,
            invoked: false,
        });
    });
    set_windows_migration_hook(Some(trigger_selected_migration_race));
    let hook_reset = MigrationHookReset;

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    set_windows_migration_hook(None);
    let race = MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    let MigrationRace::LockStagedFile { lock, invoked, .. } = race else {
        panic!("验证锁测试状态错误");
    };
    assert!(invoked);
    assert!(lock.is_some());
    assert!(result.is_err());
    assert!(!data.exists());
    assert!(!staging.join(".install-data-migration-v1.json").exists());
    drop(lock);
}

#[cfg(windows)]
#[test]
fn locked_registry_is_not_treated_as_absent() {
    assert_locked_optional_validation_file_is_rejected(|source| {
        fs::write(
            source.join("accounts.json"),
            br#"{"version":1,"activeAccountId":null,"accounts":[]}"#,
        )
        .unwrap();
        PathBuf::from("accounts.json")
    });
}

#[cfg(windows)]
#[test]
fn locked_wal_is_not_treated_as_absent() {
    assert_locked_optional_validation_file_is_rejected(|source| {
        let (account_id, account) = write_valid_account_metadata(source);
        create_valid_sqlite(&account.join("owstats.sqlite3"));
        fs::write(account.join("owstats.sqlite3-wal"), b"synthetic wal").unwrap();
        PathBuf::from("accounts")
            .join(account_id)
            .join("owstats.sqlite3-wal")
    });
}

#[cfg(windows)]
#[test]
fn locked_shm_is_not_treated_as_absent() {
    assert_locked_optional_validation_file_is_rejected(|source| {
        let (account_id, account) = write_valid_account_metadata(source);
        create_valid_sqlite(&account.join("owstats.sqlite3"));
        fs::write(account.join("owstats.sqlite3-shm"), b"synthetic shm").unwrap();
        PathBuf::from("accounts")
            .join(account_id)
            .join("owstats.sqlite3-shm")
    });
}

#[cfg(windows)]
#[test]
fn migrated_data_keeps_a_protected_admin_only_staging_dacl() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("payload.bin"), b"synthetic").unwrap();

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    let (control, sddl) = protected_dacl_sddl(&data);
    assert_ne!(control & SE_DACL_PROTECTED, 0);
    assert!(sddl.contains("FA;;;SY)"));
    assert!(sddl.contains("FA;;;BA)"));
    assert!(!sddl.contains(";;;BU)"));
    assert!(!sddl.contains(";;;AU)"));
    assert_eq!(sddl.matches('(').count(), 2);
}

#[cfg(windows)]
#[test]
fn staging_is_protected_when_it_first_becomes_observable() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let injected = PathBuf::from("pre-protection-write.bin");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("payload.bin"), b"synthetic").unwrap();
    MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(MigrationRace::ProbeInitialStagingAccess {
            relative: injected.clone(),
            write_succeeded: None,
        });
    });
    set_windows_migration_hook(Some(trigger_selected_migration_race));
    let hook_reset = MigrationHookReset;

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    set_windows_migration_hook(None);
    let race = MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    let MigrationRace::ProbeInitialStagingAccess {
        write_succeeded, ..
    } = race
    else {
        panic!("初始临时目录权限测试状态错误");
    };
    assert_eq!(write_succeeded, Some(false));
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(!data.join(injected).exists());
}

#[cfg(windows)]
#[test]
fn ordinary_user_cannot_replace_validated_content_before_publish() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let original = br#"{"version":1,"activeAccountId":null,"accounts":[]}"#;
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("accounts.json"), original).unwrap();
    MIGRATION_RACE.with(|race| {
        *race.borrow_mut() = Some(MigrationRace::ReplaceValidatedFile {
            relative: PathBuf::from("accounts.json"),
            replacement: b"{attacker controlled".to_vec(),
            write_succeeded: None,
        });
    });
    set_windows_migration_hook(Some(trigger_selected_migration_race));
    let hook_reset = MigrationHookReset;

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();

    set_windows_migration_hook(None);
    let race = MIGRATION_RACE.with(|race| race.borrow_mut().take().unwrap());
    drop(hook_reset);
    let MigrationRace::ReplaceValidatedFile {
        write_succeeded, ..
    } = race
    else {
        panic!("发布替换测试状态错误");
    };
    assert_eq!(write_succeeded, Some(false));
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("accounts.json")).unwrap(), original);
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

#[cfg(windows)]
#[test]
fn rejects_empty_directories_after_the_global_entry_budget_and_retries_safely() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let deepest = source.join("first").join("second").join("third");
    fs::create_dir_all(&deepest).unwrap();
    let limits = install_copy_limits(WindowsCopyLimits::new(64, 2, 1024));

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    drop(limits);
    assert!(result.is_err());
    assert!(!data.exists());
    assert!(deepest.is_dir());
    assert!(staging.is_dir());

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert!(data.join("first").join("second").join("third").is_dir());
    assert!(!staging.exists());
    assert!(deepest.is_dir());
}

#[cfg(windows)]
#[test]
fn rejects_a_flat_directory_when_enumerated_entries_exceed_the_budget() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let names = [
        "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth",
    ];
    for name in names {
        fs::create_dir_all(source.join(name)).unwrap();
    }
    let enumeration = install_enumeration_counter();
    let limits = install_copy_limits(WindowsCopyLimits::new(64, 2, 1024));

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    drop(limits);
    set_windows_copy_enumeration_hook(None);
    let enumerated = ENUMERATED_SOURCE_ENTRIES.with(Cell::get);
    drop(enumeration);
    assert!(result.is_err());
    assert_eq!(enumerated, 3);
    assert!(!data.exists());
    assert!(staging.is_dir());
    for name in names {
        assert!(source.join(name).is_dir());
    }

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    for name in names {
        assert!(data.join(name).is_dir());
        assert!(source.join(name).is_dir());
    }
    assert!(!staging.exists());
}

#[cfg(windows)]
#[test]
fn rejects_actual_read_bytes_over_the_budget_before_writing_them_and_retries_safely() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let payload = b"12345";
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("payload.bin"), payload).unwrap();
    let limits = install_copy_limits(WindowsCopyLimits::new(64, 8, 4));

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    drop(limits);
    assert!(result.is_err());
    assert!(!data.exists());
    assert_eq!(fs::read(source.join("payload.bin")).unwrap(), payload);
    assert_eq!(fs::metadata(staging.join("payload.bin")).unwrap().len(), 0);

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("payload.bin")).unwrap(), payload);
    assert_eq!(fs::read(source.join("payload.bin")).unwrap(), payload);
    assert!(!staging.exists());
}

#[cfg(windows)]
#[test]
fn rejects_a_source_with_a_live_writer_and_retries_after_the_writer_closes() {
    use std::io::Write;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let payload_path = source.join("payload.bin");
    fs::create_dir_all(&source).unwrap();
    fs::write(&payload_path, b"start").unwrap();
    let mut writer = fs::OpenOptions::new()
        .append(true)
        .open(&payload_path)
        .unwrap();
    writer.write_all(b"-grown").unwrap();
    writer.sync_all().unwrap();
    let limits = install_copy_limits(WindowsCopyLimits::new(64, 8, 1024));

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    drop(limits);
    assert!(result.is_err());
    assert!(!data.exists());
    assert!(staging.is_dir());
    drop(writer);
    assert_eq!(fs::read(&payload_path).unwrap(), b"start-grown");

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("payload.bin")).unwrap(), b"start-grown");
    assert_eq!(fs::read(&payload_path).unwrap(), b"start-grown");
    assert!(!staging.exists());
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

#[cfg(windows)]
#[test]
fn stops_streaming_a_flat_staging_directory_at_the_cleanup_entry_budget_and_retries() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let sibling = data.with_file_name("keep.txt");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&staging).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    fs::write(&sibling, b"keep").unwrap();
    for index in 0..8 {
        fs::write(staging.join(format!("stale-{index}.txt")), b"stale").unwrap();
    }
    let enumeration = install_cleanup_enumeration_counter();
    let limits = install_cleanup_limits(WindowsCleanupLimits::new(64, 2));

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    drop(limits);
    set_windows_cleanup_enumeration_hook(None);
    let enumerated = ENUMERATED_CLEANUP_ENTRIES.with(Cell::get);
    drop(enumeration);
    assert!(result.is_err());
    assert_eq!(enumerated, 3);
    assert!(!data.exists());
    assert!(staging.is_dir());
    assert!(fs::read_dir(&staging).unwrap().count() > 0);
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert_eq!(fs::read(&sibling).unwrap(), b"keep");

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("source.txt")).unwrap(), b"source");
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert_eq!(fs::read(&sibling).unwrap(), b"keep");
    assert!(!staging.exists());
}

#[cfg(windows)]
#[test]
fn rejects_staging_deeper_than_the_cleanup_depth_budget_and_retries() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let data = temp.path().join("install").join("data");
    let staging = data.with_file_name("data.migrating");
    let sibling = data.with_file_name("keep.txt");
    let deepest = staging.join("first").join("second").join("third");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&deepest).unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    fs::write(deepest.join("stale.txt"), b"stale").unwrap();
    fs::write(&sibling, b"keep").unwrap();
    let limits = install_cleanup_limits(WindowsCleanupLimits::new(2, 32));

    let result = prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data));

    drop(limits);
    assert!(result.is_err());
    assert!(!data.exists());
    assert!(deepest.join("stale.txt").is_file());
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert_eq!(fs::read(&sibling).unwrap(), b"keep");

    let outcome =
        prepare_install_data(Some(&source), &SharedAppPaths::from_data_root(&data)).unwrap();
    assert_eq!(outcome, InstallDataMigrationOutcome::Migrated);
    assert_eq!(fs::read(data.join("source.txt")).unwrap(), b"source");
    assert_eq!(fs::read(source.join("source.txt")).unwrap(), b"source");
    assert_eq!(fs::read(&sibling).unwrap(), b"keep");
    assert!(!staging.exists());
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

#[test]
fn migration_cli_accepts_source_and_destination() {
    let mode = owstats_desktop_lib::parse_startup_mode([
        "Watchback.exe",
        "--prepare-install-data",
        r"D:\\旧 数据",
        r"D:\\新 数据",
    ])
    .unwrap();

    assert_eq!(
        mode,
        owstats_desktop_lib::StartupMode::PrepareInstallData {
            source: Some(std::path::PathBuf::from(r"D:\\旧 数据")),
            destination: std::path::PathBuf::from(r"D:\\新 数据"),
        }
    );
}

#[test]
fn migration_cli_uses_dash_for_an_empty_source() {
    let mode = owstats_desktop_lib::parse_startup_mode([
        "Watchback.exe",
        "--prepare-install-data",
        "-",
        r"D:\\new\\data",
    ])
    .unwrap();

    assert!(matches!(
        mode,
        owstats_desktop_lib::StartupMode::PrepareInstallData { source: None, .. }
    ));
}

#[test]
fn migration_cli_rejects_missing_or_unknown_arguments() {
    for args in [
        vec!["Watchback.exe", "--prepare-install-data"],
        vec!["Watchback.exe", "--prepare-install-data", "-"],
        vec!["Watchback.exe", "--unknown"],
        vec![
            "Watchback.exe",
            "--prepare-install-data",
            "-",
            r"D:\\new\\data",
            "extra",
        ],
    ] {
        assert!(owstats_desktop_lib::parse_startup_mode(args).is_err());
    }
}

#[test]
fn migration_execution_accepts_only_the_executable_data_directory() {
    let temp = tempfile::tempdir().unwrap();
    let install = temp.path().join("install");
    let executable = install.join("Watchback.exe");
    let data = install.join("data");
    fs::create_dir_all(&install).unwrap();
    fs::write(&executable, b"synthetic executable").unwrap();

    owstats_desktop_lib::run_install_data_mode(&executable, None, &data).unwrap();

    assert!(data.is_dir());
}

#[test]
fn migration_execution_rejects_outside_relative_and_dot_destinations_without_creating_files() {
    let temp = tempfile::tempdir().unwrap();
    let install = temp.path().join("install");
    let executable = install.join("Watchback.exe");
    let outside = temp.path().join("outside").join("data");
    let relative = std::path::PathBuf::from("data");
    let dot = install.join("nested").join("..").join("data");
    fs::create_dir_all(&install).unwrap();
    fs::write(&executable, b"synthetic executable").unwrap();

    for destination in [&outside, &relative, &dot] {
        assert!(
            owstats_desktop_lib::run_install_data_mode(&executable, None, destination).is_err()
        );
    }

    assert!(!outside.exists());
    assert!(!install.join("data").exists());
    assert!(!install.join("nested").exists());
}

#[cfg(windows)]
#[test]
fn migration_execution_keeps_the_authorized_install_parent_fixed_until_publish() {
    let temp = tempfile::tempdir().unwrap();
    let install = temp.path().join("install");
    let executable = install.join("Watchback.exe");
    let source = temp.path().join("legacy");
    let data = install.join("data");
    let displaced = temp.path().join("displaced-install");
    fs::create_dir_all(&install).unwrap();
    fs::create_dir_all(&source).unwrap();
    fs::write(&executable, b"synthetic executable").unwrap();
    fs::write(source.join("source.txt"), b"source").unwrap();
    INSTALL_PARENT_GUARD_RACE.with(|race| {
        *race.borrow_mut() = Some(InstallParentGuardRace {
            install: install.clone(),
            displaced: displaced.clone(),
            rename_succeeded: None,
        });
    });
    set_windows_parent_guard_hook(Some(replace_install_parent_after_authorization));

    let result = owstats_desktop_lib::run_install_data_mode(&executable, Some(&source), &data);

    set_windows_parent_guard_hook(None);
    let race = INSTALL_PARENT_GUARD_RACE.with(|race| race.borrow_mut().take().unwrap());
    assert!(result.is_ok());
    assert_eq!(race.rename_succeeded, Some(false));
    assert_eq!(fs::read(data.join("source.txt")).unwrap(), b"source");
    assert!(!displaced.exists());
}

#[cfg(windows)]
fn copied_migration_binary(temp: &tempfile::TempDir) -> PathBuf {
    let install = temp.path().join("安装 空格");
    let executable = install.join("Watchback.exe");
    fs::create_dir_all(&install).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_owstats-desktop"), &executable).unwrap();
    executable
}

#[cfg(windows)]
fn assert_silent_exit_two(output: std::process::Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(windows)]
#[test]
fn migration_binary_exits_without_starting_tauri_and_enforces_the_cli_contract() {
    let temp = tempfile::tempdir().unwrap();
    let executable = copied_migration_binary(&temp);
    let install = executable.parent().unwrap();
    let data = install.join("data");
    let uppercase_data = install.join("DATA");
    let started = Instant::now();
    let success = Command::new(&executable)
        .arg("--prepare-install-data")
        .arg("-")
        .arg(&uppercase_data)
        .output()
        .unwrap();
    assert!(success.status.success());
    assert!(success.stdout.is_empty());
    assert!(success.stderr.is_empty());
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(data.is_dir());

    let unknown = Command::new(&executable).arg("--unknown").output().unwrap();
    assert_silent_exit_two(unknown);
    let missing = Command::new(&executable)
        .args(["--prepare-install-data", "-"])
        .output()
        .unwrap();
    assert_silent_exit_two(missing);
    let outside = temp.path().join("outside").join("data");
    let outside_output = Command::new(&executable)
        .arg("--prepare-install-data")
        .arg("-")
        .arg(&outside)
        .output()
        .unwrap();
    assert_silent_exit_two(outside_output);
    assert!(!outside.exists());
}

#[cfg(windows)]
#[test]
fn migration_execution_rejects_an_ill_formed_utf16_data_name() {
    use std::os::windows::ffi::OsStringExt;

    let temp = tempfile::tempdir().unwrap();
    let install = temp.path().join("install");
    let executable = install.join("Watchback.exe");
    fs::create_dir_all(&install).unwrap();
    fs::write(&executable, b"synthetic executable").unwrap();
    let malformed = std::ffi::OsString::from_wide(&[
        b'd' as u16,
        b'a' as u16,
        b't' as u16,
        b'a' as u16,
        0xd800,
    ]);
    let destination = install.join(malformed);

    assert!(owstats_desktop_lib::run_install_data_mode(&executable, None, &destination).is_err());
    assert!(!install.join("data").exists());
}
