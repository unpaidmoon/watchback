use anyhow::Result;
use owstats_desktop_lib::player_query::{
    models::{MatchKind, QueryMode, QueryResult, Season},
    upstream::{PlayerQueryUpstream, UpstreamFuture},
    PlayerQueryService,
};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const SYNTHETIC_TOKEN: &str = "synthetic-customer-secret";

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFact {
    len: u64,
    readonly: bool,
    sha256: String,
}

fn snapshot(root: &Path) -> Result<BTreeMap<PathBuf, FileFact>> {
    fn visit(root: &Path, directory: &Path, out: &mut BTreeMap<PathBuf, FileFact>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, out)?;
            } else {
                let bytes = fs::read(&path)?;
                let metadata = fs::metadata(&path)?;
                out.insert(
                    path.strip_prefix(root)?.to_path_buf(),
                    FileFact {
                        len: metadata.len(),
                        readonly: metadata.permissions().readonly(),
                        sha256: format!("{:x}", Sha256::digest(bytes)),
                    },
                );
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result)?;
    Ok(result)
}

#[derive(Default)]
struct Calls(Mutex<Vec<String>>);

impl Calls {
    fn push(&self, value: impl Into<String>) {
        self.0.lock().unwrap().push(value.into());
    }
    fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

struct MockUpstream {
    calls: Arc<Calls>,
    fail_detail: bool,
}

impl PlayerQueryUpstream for MockUpstream {
    fn search<'a>(&'a self, battle_tag: &'a str) -> UpstreamFuture<'a> {
        self.calls.push(format!("search:{battle_tag}"));
        Box::pin(async move {
            Ok(json!({"data":{"battleTag":battle_tag,"accountKey":SYNTHETIC_TOKEN}}))
        })
    }
    fn card<'a>(&'a self, token: &'a str) -> UpstreamFuture<'a> {
        assert_eq!(token, SYNTHETIC_TOKEN);
        self.calls.push("card");
        Box::pin(async {
            Ok(
                json!({"data":{"icon":"https://invalid.example/icon.png","level":123,"gameTime":61,"title":"先锋","titleIcon":"https://invalid.example/title.png"}}),
            )
        })
    }
    fn count_info<'a>(
        &'a self,
        token: &'a str,
        mode: &'a str,
        season: Option<i64>,
    ) -> UpstreamFuture<'a> {
        assert_eq!(token, SYNTHETIC_TOKEN);
        self.calls.push(format!("count:{mode}:{season:?}"));
        Box::pin(async {
            Ok(
                json!({"data":{"guideCountData":[],"heroList":[{"heroGuid":"hero","heroName":"英雄","gameTimeSec":60,"winCount":1,"gameCount":2}]}}),
            )
        })
    }
    fn fight_count<'a>(&'a self, token: &'a str, season: Option<i64>) -> UpstreamFuture<'a> {
        assert_eq!(token, SYNTHETIC_TOKEN);
        self.calls.push(format!("fight-count:{season:?}"));
        Box::pin(async { Ok(json!({"data":{"roleTypeCountData":[],"heroUseSummaryList":[]}})) })
    }
    fn leisure_fight_card<'a>(&'a self, token: &'a str, season: Option<i64>) -> UpstreamFuture<'a> {
        assert_eq!(token, SYNTHETIC_TOKEN);
        self.calls.push(format!("leisure-fight-card:{season:?}"));
        Box::pin(async { Ok(json!({"data":{"roleDataList":[],"heroDataList":[]}})) })
    }
    fn match_list<'a>(
        &'a self,
        token: &'a str,
        mode: &'a str,
        page: u32,
        season: Option<i64>,
        arena: bool,
    ) -> UpstreamFuture<'a> {
        assert_eq!(token, SYNTHETIC_TOKEN);
        self.calls
            .push(format!("list:{mode}:{page}:{season:?}:{arena}"));
        Box::pin(async move {
            Ok(
                json!({"data":{"matchList":[{"matchId":format!("match-{page}"),"mapName":"测试地图"}]}}),
            )
        })
    }
    fn match_detail<'a>(
        &'a self,
        token: &'a str,
        match_id: &'a str,
        arena: bool,
    ) -> UpstreamFuture<'a> {
        assert_eq!(token, SYNTHETIC_TOKEN);
        self.calls.push(format!("detail:{match_id}:{arena}"));
        if self.fail_detail {
            return Box::pin(async {
                anyhow::bail!("Authorization: Bearer synthetic-customer-secret Cookie=secret raw=https://secret.invalid")
            });
        }
        Box::pin(async move {
            Ok(
                json!({"data":{"matchId":match_id,"mapName":"测试地图","playerList":[],"roundList":[]}}),
            )
        })
    }
    fn image<'a>(&'a self, url: &'a str, max_bytes: usize) -> UpstreamFuture<'a> {
        assert!(max_bytes <= 256 * 1024);
        self.calls.push(format!("image:{url}"));
        Box::pin(async { Ok(json!({"mimeType":"image/png","bytes":[1,2,3]})) })
    }
}

fn serialized<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}

fn query<T>(result: QueryResult<T>) -> Result<T> {
    result.map_err(|error| anyhow::anyhow!("{error:?}"))
}

#[tokio::test]
async fn query_flow_is_memory_only_and_has_exact_request_boundaries() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    fs::create_dir_all(root.join("data/accounts/synthetic"))?;
    fs::write(
        root.join("data/accounts/synthetic/credentials.json"),
        b"{\"synthetic\":true}",
    )?;
    fs::write(root.join("data/player_target.json"), b"{\"sentinel\":true}")?;
    fs::write(root.join("season.json"), b"{\"matches\":[]}")?;
    let db_path = root.join("data/watchback.sqlite3");
    let db = Connection::open(&db_path)?;
    db.execute_batch(
        "CREATE TABLE sentinel(value TEXT); INSERT INTO sentinel VALUES ('unchanged');",
    )?;
    let before_files = snapshot(root)?;
    let before_data_version: i64 = db.query_row("PRAGMA data_version", [], |row| row.get(0))?;
    let before_page_count: i64 = db.query_row("PRAGMA page_count", [], |row| row.get(0))?;

    let calls = Arc::new(Calls::default());
    let upstream = Arc::new(MockUpstream {
        calls: Arc::clone(&calls),
        fail_detail: false,
    });
    let service = PlayerQueryService::new(Default::default());
    let started = query(service.start_query(" 测试玩家＃1234 ", upstream).await)?;
    let card = query(service.load_card(&started.session_id).await)?;
    let images = query(service.load_card_images(&started.session_id).await)?;
    let season = query(service.load_season(&started.session_id, Season::S23).await)?;
    let page1 = query(
        service
            .load_mode_page(
                &started.session_id,
                Season::S23,
                QueryMode::LeisurePreset,
                1,
            )
            .await,
    )?;
    let heroes = query(
        service
            .load_mode_heroes(&started.session_id, Season::S23, QueryMode::LeisurePreset)
            .await,
    )?;

    let home_calls = calls.all();
    assert_eq!(
        home_calls
            .iter()
            .filter(|v| v.starts_with("search:"))
            .count(),
        1
    );
    assert_eq!(
        home_calls.iter().filter(|v| *v == "card").count(),
        2,
        "text and image commands independently read the card endpoint"
    );
    assert_eq!(
        home_calls
            .iter()
            .filter(|v| v.starts_with("image:"))
            .count(),
        2
    );
    assert_eq!(
        home_calls
            .iter()
            .filter(|v| v == &&"count:sport:None".to_string())
            .count(),
        1
    );
    assert_eq!(
        home_calls
            .iter()
            .filter(|v| v == &&"fight-count:None".to_string())
            .count(),
        1
    );
    assert_eq!(
        home_calls
            .iter()
            .filter(|v| v == &&"count:leisure:None".to_string())
            .count(),
        1
    );
    assert_eq!(
        home_calls.iter().filter(|v| v.starts_with("list:")).count(),
        1
    );
    assert!(!home_calls
        .iter()
        .any(|v| v.starts_with("detail:") || v.contains(":2:")));
    assert!(!home_calls
        .iter()
        .any(|v| v.contains("Leisure6v6") || v.contains("LeisureFight")));

    let detail = query(
        service
            .load_match_detail(&started.session_id, MatchKind::Normal, "match-1")
            .await,
    )?;
    let page2 = query(
        service
            .load_mode_page(
                &started.session_id,
                Season::S23,
                QueryMode::LeisurePreset,
                2,
            )
            .await,
    )?;
    query(service.release_more_data(&started.session_id))?;
    service.end_query(&started.session_id);
    let all_calls = calls.all();
    assert_eq!(
        all_calls
            .iter()
            .filter(|v| v.starts_with("detail:"))
            .count(),
        1
    );
    assert_eq!(all_calls.iter().filter(|v| v.contains(":2:")).count(), 1);

    for output in [
        serialized(&started),
        serialized(&card),
        serialized(&images),
        serialized(&season),
        serialized(&page1),
        serialized(&heroes),
        serialized(&detail),
        serialized(&page2),
    ] {
        let lower = output.to_ascii_lowercase();
        for forbidden in [
            SYNTHETIC_TOKEN,
            "customertoken",
            "authorization",
            "cookie",
            "http://",
            "https://",
            "rawjson",
            "rawresponse",
        ] {
            assert!(
                !lower.contains(&forbidden.to_ascii_lowercase()),
                "WebView DTO leaked forbidden marker {forbidden}"
            );
        }
    }

    let failing = Arc::new(MockUpstream {
        calls: Arc::new(Calls::default()),
        fail_detail: true,
    });
    let failed = query(service.start_query("错误测试#9", failing).await)?;
    let safe_error = service
        .load_match_detail(&failed.session_id, MatchKind::Normal, "bad")
        .await
        .unwrap_err();
    service.end_query(&failed.session_id);
    let error_json = serialized(&safe_error).to_ascii_lowercase();
    for forbidden in [
        SYNTHETIC_TOKEN,
        "authorization",
        "cookie",
        "bearer",
        "https://",
        "raw=",
    ] {
        assert!(
            !error_json.contains(&forbidden.to_ascii_lowercase()),
            "serialized error leaked {forbidden}"
        );
    }

    assert_eq!(
        snapshot(root)?,
        before_files,
        "query flow changed the app-like directory tree"
    );
    assert_eq!(
        db.query_row::<i64, _, _>("PRAGMA data_version", [], |row| row.get(0))?,
        before_data_version
    );
    assert_eq!(
        db.query_row::<i64, _, _>("PRAGMA page_count", [], |row| row.get(0))?,
        before_page_count
    );
    Ok(())
}
