use owstats_desktop_lib::{sync_season_with_progress, OwDatabase};

fn project_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

#[tokio::test]
#[ignore = "uses real local Dashen credentials and upstream API"]
async fn live_rust_sync_imports_a_complete_season() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("live-sync.sqlite3"));
    let root = project_root();

    let first = sync_season_with_progress(&db, &root, 22, |progress| {
        eprintln!(
            "phase={} processed={}/{} failed={}",
            progress.phase, progress.processed, progress.total, progress.failed_details
        );
    })
    .await
    .unwrap();
    let conn = db.connect().unwrap();
    let matches_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM matches", [], |row| row.get(0))
        .unwrap();
    let details_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_details", [], |row| row.get(0))
        .unwrap();
    let projection_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))
        .unwrap();

    assert!(first.imported_matches > 0);
    assert!(first.imported_details > 0);
    assert_eq!(matches_count, first.imported_matches as i64);
    assert_eq!(details_count, first.imported_details as i64);
    assert!(projection_rows > 0);
}
