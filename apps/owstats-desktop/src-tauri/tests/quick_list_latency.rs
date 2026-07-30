use anyhow::Result;
use owstats_desktop_lib::dashen_client::DashenCredentials;
use owstats_desktop_lib::season_fetch::{
    fetch_quick_list_pages_with_progress, FullSeasonRequest, QuickListProbe,
};
use owstats_desktop_lib::{commit_quick_list_probe, OwDatabase};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

#[tokio::test]
async fn fast_mode_is_committed_before_a_slow_mode_finishes() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server = thread::spawn(move || {
        let mut workers = Vec::new();
        for stream in listener.incoming().take(5) {
            let mut stream = stream.unwrap();
            workers.push(thread::spawn(move || {
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..read]);
                let slow = request.contains("gameMode=sport&") || request.contains("gameMode=sport%26");
                if slow {
                    thread::sleep(Duration::from_millis(1_200));
                }
                let body = if request.contains("gameMode=leisure") {
                    r#"{"code":0,"data":{"matchList":[{"matchId":"fast-new","beginTs":1700000200}]}}"#
                } else {
                    r#"{"code":0,"data":{"matchList":[]}}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
    });
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("latency.sqlite3"));
    let request = FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 1,
            token: "synthetic-secret".into(),
            dts: Some(2026),
            server: 1,
        },
        customer_token: "synthetic-player".into(),
        input_bnet: "Synthetic#1".into(),
        resolved_bnet: None,
        season: 22,
        customer_base_url: format!("http://{addr}/customer"),
        pages_per_batch: 5,
        detail_concurrency: 4,
        include_fight: true,
    };
    let started = Instant::now();
    let mut fast_commit_elapsed = None;
    fetch_quick_list_pages_with_progress(&request, |mode| {
        let summary = commit_quick_list_probe(
            &db,
            QuickListProbe {
                season: 22,
                modes: vec![mode.clone()],
                list_duration_ms: 0,
            },
        )?;
        if summary.new_list_matches == 1 {
            fast_commit_elapsed = Some(started.elapsed());
        }
        Ok(())
    })
    .await?;
    server.join().unwrap();

    assert!(fast_commit_elapsed.is_some_and(|elapsed| elapsed < Duration::from_millis(900)));
    assert_eq!(
        db.connect()?.query_row(
            "SELECT COUNT(*) FROM matches WHERE match_id='fast-new'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    Ok(())
}
