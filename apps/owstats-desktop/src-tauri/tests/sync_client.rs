use owstats_desktop_lib::{sync_season_with_base_url, OwDatabase};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

#[tokio::test]
async fn rust_sync_client_imports_http_payloads_and_keeps_going_after_detail_failure() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let data_dir = root.join("data");
    let players_dir = data_dir.join("players");
    std::fs::create_dir_all(&players_dir).unwrap();
    std::fs::write(
        data_dir.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"dummy-auth-token","dts":2026,"server":1}"#,
    )
    .unwrap();
    std::fs::write(
        players_dir.join("fixture.json"),
        r#"{
          "input_bnet":"Tester#1000",
          "resolved_bnet":"Tester#1000",
          "customer_token":"dummy-customer-token",
          "profile_card":{"data":{"bnetId":1000,"name":"Tester#1000","customerToken":"dummy-customer-token"}}
        }"#,
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

    assert_eq!(first.imported_matches, 2);
    assert_eq!(first.imported_details, 1);
    assert_eq!(first.failed_details, 1);
    assert_eq!(second.imported_matches, 2);
    assert_eq!(second.imported_details, 1);
    assert_eq!(matches_count, 2);
    assert_eq!(details_count, 1);
    assert_eq!(projection_rows, 3);
    assert!(!latest_error.contains("dummy-customer-token"));
    assert!(!latest_error.contains("dummy-auth-token"));
}

fn spawn_fixture_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().take(24) {
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

fn fixture_response(request_line: &str) -> (&'static str, &'static str) {
    if request_line.contains("/queryMatchList")
        && request_line.contains("gameMode=sport")
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
    if request_line.contains("/queryMatchInfo") && request_line.contains("matchId=fixture-ok") {
        return (
            "200 OK",
            r#"{"code":0,"data":{
              "matchRet":1,
              "mapGuid":"map-a",
              "gameTimeSec":600,
              "teammateList":[
                {"bnetId":1000,"name":"Tester#1000","heroGuid":"hero-current","friendBnetIds":[2000],"kill":1,"assist":2,"death":3,"healingTaken":10,"damageTaken":20},
                {"bnetId":2000,"name":"Friend#2000","heroGuid":"hero-friend","kill":4,"assist":5,"death":1,"healingTaken":100,"damageTaken":200}
              ],
              "enemyList":[
                {"bnetId":3000,"name":"Enemy#3000","heroGuid":"hero-enemy","kill":6,"assist":0,"death":2,"healingTaken":30,"damageTaken":40}
              ]
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
