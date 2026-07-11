use owstats_desktop_lib::OwDatabase;
use rusqlite::Connection;

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
}
