use owstats_desktop_lib::OwDatabase;
use rusqlite::Connection;
use std::fs;
use std::time::Duration;

#[test]
fn database_rejects_a_schema_newer_than_the_running_application() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("future.sqlite3");
    let conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 999_i64).unwrap();
    drop(conn);

    let error = OwDatabase::new(&path).initialize().unwrap_err().to_string();

    assert!(error.contains("高于当前程序"), "unexpected error: {error}");
}

#[test]
fn every_database_connection_waits_for_brief_lock_contention() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));
    let conn = db.connect().unwrap();
    let timeout: i64 = conn
        .pragma_query_value(None, "busy_timeout", |row| row.get(0))
        .unwrap();

    assert_eq!(timeout, 5_000);
    let journal_mode: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");
}

#[test]
fn second_writer_waits_for_the_first_writer_to_finish() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("owstats.sqlite3");
    let db = OwDatabase::new(&path);
    db.initialize().unwrap();
    let first = db.connect().unwrap();
    first.execute_batch("BEGIN IMMEDIATE;").unwrap();

    let worker_path = path.clone();
    let worker = std::thread::spawn(move || {
        let second = OwDatabase::new(worker_path).connect().unwrap();
        second.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES ('stab005-lock', 'ok', 'now')",
            [],
        )
    });
    std::thread::sleep(Duration::from_millis(100));
    first.execute_batch("COMMIT;").unwrap();

    worker.join().unwrap().unwrap();
}

#[test]
fn legacy_database_is_backed_up_before_schema_migration() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("owstats.sqlite3");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE legacy_data (id INTEGER); PRAGMA user_version = 2;")
        .unwrap();
    drop(conn);

    OwDatabase::new(&path).initialize().unwrap();

    let backups = fs::read_dir(temp.path().join("backups"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), 1);
    let backup = Connection::open(&backups[0]).unwrap();
    let version: i64 = backup
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
}

#[test]
fn new_empty_database_does_not_create_a_schema_backup() {
    let temp = tempfile::tempdir().unwrap();
    OwDatabase::new(temp.path().join("owstats.sqlite3"))
        .initialize()
        .unwrap();
    assert!(!temp.path().join("backups").exists());
}

#[test]
fn backup_failure_preserves_the_legacy_database_before_schema_changes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("owstats.sqlite3");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE legacy_data (id INTEGER); PRAGMA user_version = 2;")
        .unwrap();
    drop(conn);
    fs::write(temp.path().join("backups"), "not a directory").unwrap();

    assert!(OwDatabase::new(&path).initialize().is_err());
    let source = Connection::open(&path).unwrap();
    let version: i64 = source
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
}
