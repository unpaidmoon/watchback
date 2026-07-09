use owstats_desktop_lib::{
    dump_season_json_with_base_url, sync_season_with_base_url,
    sync_season_with_credentials_path_base_url, OwDatabase,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

#[tokio::test]
async fn rust_sync_refuses_to_use_account_token_as_customer_token() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(project_root.join("data")).unwrap();
    std::fs::create_dir_all(&app_data).unwrap();
    std::fs::write(
        app_data.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"dummy-auth-token","dts":2026,"server":1}"#,
    )
    .unwrap();
    let base_url = spawn_fixture_server();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));

    let error = sync_season_with_credentials_path_base_url(
        &db,
        &project_root,
        &app_data.join(".dashen_credentials.json"),
        23,
        &base_url,
        |_| {},
    )
    .await
    .expect_err("sync must require a target player customerToken");

    let message = error.to_string();
    assert!(message.contains("目标玩家"));
    assert!(!message.contains("dummy-auth-token"));
    assert!(!message.contains("customerToken"));
}

#[tokio::test]
async fn rust_sync_client_imports_http_payloads_and_keeps_going_after_detail_failure() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(
        data_dir.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"dummy-auth-token","customer_token":"dummy-customer-token","dts":2026,"server":1}"#,
    )
    .unwrap();
    let base_url = spawn_fixture_server();
    let db = OwDatabase::new(root.join("sync.sqlite3"));

    let first = sync_season_with_base_url(&db, root, 22, &base_url, |_| {})
        .await
        .unwrap();
    let second = sync_season_with_base_url(&db, root, 22, &base_url, |_| {})
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
    let latest_error: String = conn
        .query_row(
            "SELECT raw_json FROM sync_errors ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(first.imported_matches, 3);
    assert_eq!(first.imported_details, 2);
    assert_eq!(first.failed_details, 1);
    assert_eq!(second.imported_matches, 3);
    assert_eq!(second.imported_details, 2);
    assert_eq!(matches_count, 3);
    assert_eq!(details_count, 2);
    assert_eq!(projection_rows, 3);
    assert!(!latest_error.contains("dummy-customer-token"));
    assert!(!latest_error.contains("dummy-auth-token"));

    let raw_path = data_dir.join("players").join("local_s22_full_season.json");
    let raw_text = std::fs::read_to_string(&raw_path).expect("sync keeps raw season json");
    let raw_payload: serde_json::Value = serde_json::from_str(&raw_text).unwrap();
    assert_eq!(raw_payload["season"], 22);
    assert_eq!(
        raw_payload["matches"]["entries"].as_array().unwrap().len(),
        3
    );
    assert!(!raw_text.contains("dummy-customer-token"));
    assert!(!raw_text.contains("dummy-auth-token"));
}

#[tokio::test]
async fn desktop_sync_uses_app_data_credentials_instead_of_project_data_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(project_root.join("data")).unwrap();
    std::fs::create_dir_all(&app_data).unwrap();
    std::fs::write(
        app_data.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"dummy-auth-token","customer_token":"dummy-customer-token","dts":2026,"server":1}"#,
    )
    .unwrap();
    let base_url = spawn_fixture_server();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));

    let summary = sync_season_with_credentials_path_base_url(
        &db,
        &project_root,
        &app_data.join(".dashen_credentials.json"),
        22,
        &base_url,
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(summary.imported_matches, 3);
    assert_eq!(summary.imported_details, 2);
}

#[tokio::test]
async fn rust_sync_client_rejects_empty_season_fetch_instead_of_reporting_success() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(project_root.join("data")).unwrap();
    std::fs::create_dir_all(&app_data).unwrap();
    std::fs::write(
        app_data.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"dummy-auth-token","customer_token":"dummy-customer-token","dts":2026,"server":1}"#,
    )
    .unwrap();
    let base_url = spawn_empty_fixture_server();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));

    let error = sync_season_with_credentials_path_base_url(
        &db,
        &project_root,
        &app_data.join(".dashen_credentials.json"),
        23,
        &base_url,
        |_| {},
    )
    .await
    .expect_err("empty season fetch should not be a successful sync");

    assert!(error.to_string().contains("S23"));
    assert!(error.to_string().contains("未拉取到"));

    let raw_path = project_root
        .join("data")
        .join("players")
        .join("local_s23_full_season.json");
    let raw_text = std::fs::read_to_string(&raw_path)
        .expect("empty season fetch should still keep raw json evidence");
    let raw_payload: serde_json::Value = serde_json::from_str(&raw_text).unwrap();
    assert_eq!(raw_payload["season"], 23);
    assert_eq!(
        raw_payload["matches"]["entries"].as_array().unwrap().len(),
        0
    );
    let list_pages = raw_payload["matches"]["list_pages"].as_array().unwrap();
    assert!(
        !list_pages.is_empty(),
        "empty fetch should preserve upstream list response evidence"
    );
    assert_eq!(list_pages[0]["page"], 1);
    assert_eq!(list_pages[0]["payload"]["code"], 0);
}

#[tokio::test]
async fn rust_sync_client_dumps_complete_season_json_without_importing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(
        data_dir.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"dummy-auth-token","customer_token":"dummy-customer-token","dts":2026,"server":1}"#,
    )
    .unwrap();
    let output_dir = data_dir.join("players");
    let base_url = spawn_fixture_server();

    let dump = dump_season_json_with_base_url(
        root,
        &data_dir.join(".dashen_credentials.json"),
        22,
        &output_dir,
        &base_url,
        |_| {},
    )
    .await
    .unwrap();
    let text = std::fs::read_to_string(&dump.path).unwrap();
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();

    assert_eq!(dump.path.file_name().unwrap(), "local_s22_full_season.json");
    assert_eq!(dump.entries, 3);
    assert_eq!(dump.details, 2);
    assert_eq!(dump.failed_details, 1);
    assert_eq!(payload["season"], 22);
    assert_eq!(payload["fetch_mode"], "rust_sync");
    assert_eq!(payload["matches"]["total"], 3);
    assert_eq!(payload["matches"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(payload["matches"]["details"].as_array().unwrap().len(), 2);
    assert_eq!(
        payload["matches"]["detail_errors"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(payload["matches"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["matchId"] == "fixture-fight"));
    assert!(payload["matches"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .any(|detail| detail["match_kind"] == "fight" && detail["match_id"] == "fixture-fight"));
    let lower_text = text.to_ascii_lowercase();
    assert!(!text.contains("dummy-customer-token"));
    assert!(!text.contains("dummy-auth-token"));
    assert!(!text.contains("detail-customer-secret"));
    assert!(!text.contains("detail-nested-secret"));
    assert!(!text.contains("detail-cookie-secret"));
    assert!(!lower_text.contains("customertoken"));
    assert!(!lower_text.contains("nestedtoken"));
    assert!(!lower_text.contains("token"));
    assert!(!lower_text.contains("cookie"));
    assert!(!lower_text.contains("authorization"));
    assert!(!lower_text.contains("credential"));
}

fn spawn_fixture_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().take(64) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = [0_u8; 4096];
            let Ok(read) = stream.read(&mut buffer) else {
                continue;
            };
            let request = String::from_utf8_lossy(&buffer[..read]);
            let first_line = request.lines().next().unwrap_or_default();
            let (status, body) = fixture_response(first_line);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}")
}

fn spawn_empty_fixture_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().take(32) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = [0_u8; 4096];
            let Ok(read) = stream.read(&mut buffer) else {
                continue;
            };
            if read == 0 {
                continue;
            }
            let body = r#"{"code":0,"data":{"matchList":[]}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}")
}

fn fixture_response(request_line: &str) -> (&'static str, &'static str) {
    if request_line.contains("/queryMatchList")
        && request_line.contains("gameMode=SportPreset")
        && request_line.contains("page=1")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"matchList":[
              {"matchId":"fixture-ok","matchRet":1,"mapGuid":"map-a","heroGuid":"hero-current","beginTs":1700000000000,"gameMode":"SportPreset","_dashenSeason":22},
              {"matchId":"fixture-fail","matchRet":2,"mapGuid":"map-b","heroGuid":"hero-current","beginTs":1699999900000,"gameMode":"SportPreset","_dashenSeason":22}
            ]}}"#,
        );
    }
    if request_line.contains("/fight/queryMatchList")
        && request_line.contains("gameMode=QuickFight")
        && request_line.contains("page=1")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{"matchList":[
              {"matchId":"fixture-fight","matchRet":1,"mapGuid":"arena-a","heroGuid":"hero-current","beginTs":1699999800000}
            ]}}"#,
        );
    }
    if request_line.contains("/queryMatchInfo") && request_line.contains("matchId=fixture-ok") {
        return (
            "200 OK",
            r#"{"code":0,"data":{
              "matchRet":1,
              "mapGuid":"map-a",
              "gameTimeSec":600,
              "customerToken":"detail-customer-secret",
              "teammateList":[
                {"bnetId":1000,"name":"Tester#1000","heroGuid":"hero-current","friendBnetIds":[2000],"kill":1,"assist":2,"death":3,"healingTaken":10,"damageTaken":20,"nested":{"nestedToken":"detail-nested-secret","cookie":"detail-cookie-secret"}},
                {"bnetId":2000,"name":"Friend#2000","heroGuid":"hero-friend","kill":4,"assist":5,"death":1,"healingTaken":100,"damageTaken":200}
              ],
              "enemyList":[
                {"bnetId":3000,"name":"Enemy#3000","heroGuid":"hero-enemy","kill":6,"assist":0,"death":2,"healingTaken":30,"damageTaken":40}
              ]
            }}"#,
        );
    }
    if request_line.contains("/fight/queryMatchInfo")
        && request_line.contains("matchId=fixture-fight")
    {
        return (
            "200 OK",
            r#"{"code":0,"data":{
              "totalCount":{"matchRet":1,"mapGuid":"arena-a","gameTimeSec":420},
              "roundCountList":[{"matchRet":1,"mapGuid":"arena-round-a","gameTimeSec":210}]
            }}"#,
        );
    }
    if request_line.contains("/queryMatchInfo") && request_line.contains("matchId=fixture-fail") {
        return (
            "500 Internal Server Error",
            r#"{"code":500,"message":"detail failed"}"#,
        );
    }
    ("200 OK", r#"{"code":0,"data":{"matchList":[]}}"#)
}
