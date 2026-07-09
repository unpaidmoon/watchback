# Phase 8 Real Season Fetch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current guessed season-sync path with a Python-equivalent Rust fetch pipeline that can bind a target player, fetch complete season JSON for S21/S22/S23, save JSON first, and then import SQLite.

**Architecture:** Split the feature into three backend units: `dashen_client.rs` for protocol requests, `player_target.rs` for BattleTag/customerToken binding, and `season_fetch.rs` for full-season list/detail assembly. Keep `sync.rs` as orchestration: load credentials and target, fetch full JSON, write local JSON, then import. The command-center UI adds only a compact target-player binding area and keeps fixed S21/S22/S23 single-season cards.

**Tech Stack:** Rust/Tauri 2, reqwest, serde_json, SQLite importer, React/TypeScript, source-level tests under `scripts/`.

---

## File Structure

Create:

- `apps/owstats-desktop/src-tauri/src/dashen_client.rs`
  - Owns NetEase Dashen HTTP protocol equivalent to Python `DashenAPIClient`.
  - Contains request headers, endpoint methods, safe upstream error classification, and response parsing helpers.
- `apps/owstats-desktop/src-tauri/src/player_target.rs`
  - Owns target player binding state and `.dashen_player_target.json`.
  - Provides safe status DTOs that never expose `customerToken`.
- `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
  - Owns full-season JSON assembly equivalent to Python `fetch_full_season_player_data`.
  - Fetches profile/count/list/detail payloads and returns a sanitized full-season payload.
- `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
  - Focused backend integration tests for target binding, protocol parity, JSON save-first, and no token fallback.
- `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`
  - Source-level frontend assertions for target-player UI and no forbidden UI entries.

Modify:

- `apps/owstats-desktop/src-tauri/src/lib.rs`
  - Export new modules.
- `apps/owstats-desktop/src-tauri/src/models.rs`
  - Add request/response DTOs for player target binding and status.
- `apps/owstats-desktop/src-tauri/src/commands.rs`
  - Add Tauri commands for target-player status/binding/clearing.
  - Update `sync_season` to require credentials plus target player.
- `apps/owstats-desktop/src-tauri/src/sync.rs`
  - Remove account-token-as-customer-token fallback.
  - Delegate full-season fetching to `season_fetch`.
  - Keep JSON-first import behavior.
- `apps/owstats-desktop/src/shared/types.ts`
  - Add TypeScript DTOs for target-player status and bind result.
- `apps/owstats-desktop/src/shared/api.ts`
  - Add frontend API wrappers.
- `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
  - Add compact target-player binding UI.
  - Disable S21/S22/S23 sync until target player is bound.
- `apps/owstats-desktop/src/shared/phase6Shell.ts`
  - Add any small status-label helper needed by source tests.
- `apps/owstats-desktop/src/styles.css`
  - Add compact target-player binding styles.
- `apps/owstats-desktop/scripts/phase6Shell.test.ts`
  - Extend command-center source assertions if the existing test is the right place.

Do not modify:

- Real credential files.
- `data/chrome-dashen-profile/`.
- `dump_season_json` UI integration.
- Match/report/ranking/friend-analysis page layouts.

---

### Task 1: Stop Invalid Token Fallback

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/sync_client.rs`

- [ ] **Step 1: Write failing backend test for missing player customerToken**

Add this test to `apps/owstats-desktop/src-tauri/tests/sync_client.rs`:

```rust
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
    assert!(!message.contains("customer"));
}
```

- [ ] **Step 2: Run the failing test**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo test rust_sync_refuses_to_use_account_token_as_customer_token
```

Expected before implementation:

```text
FAILED
```

The failure should occur because current code falls back to `credentials.token`.

- [ ] **Step 3: Remove fallback in `sync.rs`**

In `fetch_season_parts_with_base_url`, replace the customer token extraction with:

```rust
let customer_token = credentials
    .customer_token
    .clone()
    .filter(|value| !value.trim().is_empty())
    .ok_or_else(|| anyhow!("请先绑定目标玩家，再同步赛季数据"))?;
```

Do not include the token value in the error.

- [ ] **Step 4: Run targeted test**

Run:

```powershell
cargo test rust_sync_refuses_to_use_account_token_as_customer_token
```

Expected:

```text
test rust_sync_refuses_to_use_account_token_as_customer_token ... ok
```

- [ ] **Step 5: Run sync client regression tests**

Run:

```powershell
cargo test --test sync_client
```

Expected:

```text
4 passed
```

If existing tests that write credentials without `customer_token` fail, update those fixtures only when the scenario genuinely needs a bound target player.

---

### Task 2: Add Target Player Binding Backend

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/player_target.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/models.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Create/Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Add DTOs in `models.rs`**

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTargetStatus {
    pub state: String,
    pub input_bnet: Option<String>,
    pub resolved_bnet: Option<String>,
    pub bound_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindPlayerTargetRequest {
    pub bnet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindPlayerTargetResult {
    pub status: PlayerTargetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearPlayerTargetResult {
    pub cleared: bool,
    pub status: PlayerTargetStatus,
}
```

- [ ] **Step 2: Write failing player target storage tests**

Create `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs` with:

```rust
use owstats_desktop_lib::player_target::{
    clear_player_target_for_path, player_target_status_for_path, save_player_target_for_path,
    PlayerTarget,
};

#[test]
fn player_target_status_does_not_expose_customer_token() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(".dashen_player_target.json");
    save_player_target_for_path(
        &path,
        &PlayerTarget {
            input_bnet: "Tester#12345".to_string(),
            resolved_bnet: Some("Tester#12345".to_string()),
            customer_token: "secret-customer-token".to_string(),
            bound_at: "2026-07-03T00:00:00Z".to_string(),
        },
    )
    .unwrap();

    let status = player_target_status_for_path(&path).unwrap();
    let status_text = serde_json::to_string(&status).unwrap();

    assert_eq!(status.state, "bound");
    assert_eq!(status.input_bnet.as_deref(), Some("Tester#12345"));
    assert!(!status_text.contains("secret-customer-token"));
    assert!(!status_text.contains("customerToken"));
}

#[test]
fn clearing_player_target_removes_saved_target() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(".dashen_player_target.json");
    save_player_target_for_path(
        &path,
        &PlayerTarget {
            input_bnet: "Tester#12345".to_string(),
            resolved_bnet: None,
            customer_token: "secret-customer-token".to_string(),
            bound_at: "2026-07-03T00:00:00Z".to_string(),
        },
    )
    .unwrap();

    assert!(clear_player_target_for_path(&path).unwrap());
    let status = player_target_status_for_path(&path).unwrap();

    assert_eq!(status.state, "missing");
    assert_eq!(status.input_bnet, None);
}
```

- [ ] **Step 3: Run failing storage tests**

Run:

```powershell
cargo test --test phase8_real_fetch player_target
```

Expected:

```text
FAILED
```

The failure should be unresolved module/functions.

- [ ] **Step 4: Implement `player_target.rs`**

Create `apps/owstats-desktop/src-tauri/src/player_target.rs`:

```rust
use crate::models::PlayerTargetStatus;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTarget {
    pub input_bnet: String,
    pub resolved_bnet: Option<String>,
    pub customer_token: String,
    pub bound_at: String,
}

pub fn player_target_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(".dashen_player_target.json"))
}

pub fn player_target_status_for_path(path: &Path) -> Result<PlayerTargetStatus> {
    if !path.exists() {
        return Ok(missing_status());
    }
    let text = std::fs::read_to_string(path).with_context(|| "读取目标玩家失败")?;
    let target: PlayerTarget = serde_json::from_str(&text).with_context(|| "解析目标玩家失败")?;
    if target.customer_token.trim().is_empty() {
        return Ok(missing_status());
    }
    Ok(PlayerTargetStatus {
        state: "bound".to_string(),
        input_bnet: Some(target.input_bnet),
        resolved_bnet: target.resolved_bnet,
        bound_at: Some(target.bound_at),
    })
}

pub fn read_player_target_for_path(path: &Path) -> Result<PlayerTarget> {
    let text = std::fs::read_to_string(path).with_context(|| "请先绑定目标玩家")?;
    let target: PlayerTarget = serde_json::from_str(&text).with_context(|| "目标玩家记录不可用")?;
    if target.customer_token.trim().is_empty() {
        anyhow::bail!("请先绑定目标玩家");
    }
    Ok(target)
}

pub fn save_player_target_for_path(path: &Path, target: &PlayerTarget) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(target)?;
    std::fs::write(path, text).with_context(|| "保存目标玩家失败")
}

pub fn clear_player_target_for_path(path: &Path) -> Result<bool> {
    if path.exists() {
        std::fs::remove_file(path).with_context(|| "清除目标玩家失败")?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn missing_status() -> PlayerTargetStatus {
    PlayerTargetStatus {
        state: "missing".to_string(),
        input_bnet: None,
        resolved_bnet: None,
        bound_at: None,
    }
}
```

- [ ] **Step 5: Export module in `lib.rs`**

Add:

```rust
pub mod player_target;
```

- [ ] **Step 6: Run storage tests**

Run:

```powershell
cargo test --test phase8_real_fetch player_target
```

Expected:

```text
2 passed
```

---

### Task 3: Add Dashen Protocol Client

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/dashen_client.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Write failing protocol test for headers and customerToken usage**

Append to `phase8_real_fetch.rs`:

```rust
use owstats_desktop_lib::dashen_client::{DashenClient, DashenCredentials};

#[tokio::test]
async fn dashen_client_uses_account_token_for_auth_and_customer_token_for_query() {
    let server = spawn_recording_server(vec![(
        "/customer/queryMatchList",
        r#"{"code":0,"data":{"matchList":[]}}"#,
    )]);
    let client = DashenClient::new(
        DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: None,
            server: 1,
        },
        server.base_url.clone(),
    )
    .unwrap();

    let _ = client
        .query_match_list("player-customer-token", "sport", 1, Some(23))
        .await
        .unwrap();

    let request = server.requests.lock().unwrap().remove(0);
    assert!(request.first_line.contains("/customer/queryMatchList"));
    assert!(request.first_line.contains("token=player-customer-token"));
    assert!(request.first_line.contains("gameMode=sport"));
    assert!(request.first_line.contains("page=1"));
    assert!(request.first_line.contains("season=23"));
    assert!(request.headers.contains("GL-Bigdata-Auth-Token: account-auth-token"));
    assert!(request.headers.contains("GL-Bigdata-Role-Id: 123"));
    assert!(request.headers.contains("GL-Bigdata-Server: 1"));
    assert!(request.headers.contains("GL-ClientType: 60"));
}
```

Also add a small local test server helper in the same file:

```rust
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
struct RecordedRequest {
    first_line: String,
    headers: String,
}

struct RecordingServer {
    base_url: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

fn spawn_recording_server(routes: Vec<(&'static str, &'static str)>) -> RecordingServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming().take(32) {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 8192];
            let Ok(read) = stream.read(&mut buffer) else { continue };
            if read == 0 { continue; }
            let request = String::from_utf8_lossy(&buffer[..read]);
            let mut lines = request.lines();
            let first_line = lines.next().unwrap_or_default().to_string();
            let headers = lines.take_while(|line| !line.is_empty()).collect::<Vec<_>>().join("\n");
            captured.lock().unwrap().push(RecordedRequest { first_line: first_line.clone(), headers });
            let body = routes
                .iter()
                .find(|(path, _)| first_line.contains(path))
                .map(|(_, body)| *body)
                .unwrap_or(r#"{"code":404,"message":"not found"}"#);
            let status = if body.contains("\"code\":404") { "404 Not Found" } else { "200 OK" };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    RecordingServer {
        base_url: format!("http://{addr}"),
        requests,
    }
}
```

- [ ] **Step 2: Run failing protocol test**

Run:

```powershell
cargo test dashen_client_uses_account_token_for_auth_and_customer_token_for_query
```

Expected:

```text
FAILED
```

The failure should be unresolved module/type.

- [ ] **Step 3: Implement `dashen_client.rs`**

Create `apps/owstats-desktop/src-tauri/src/dashen_client.rs`:

```rust
use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use serde_json::Value;

const CLIENT_TYPE: &str = "60";
const ORIGIN: &str = "https://act.ds.163.com";
const REFERER: &str = "https://act.ds.163.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36 app/df_client dfVersion/100111";
const BIGDATA_DTS: i64 = 2026;

#[derive(Debug, Clone)]
pub struct DashenCredentials {
    pub role_id: i64,
    pub token: String,
    pub dts: Option<i64>,
    pub server: i64,
}

pub struct DashenClient {
    client: reqwest::Client,
    base_url: String,
}

impl DashenClient {
    pub fn new(credentials: DashenCredentials, base_url: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .default_headers(auth_headers(&credentials)?)
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        Ok(Self { client, base_url })
    }

    pub async fn search_bnet_account(&self, bnet: &str) -> Result<Value> {
        self.get_json(
            &format!("{}/searchBnetAccount", self.base_url.trim_end_matches("/customer")),
            &[("bnetId", bnet.to_string())],
            "搜索目标玩家",
        )
        .await
    }

    pub async fn query_card(&self, customer_token: &str) -> Result<Value> {
        self.customer_get("queryCard", &[("token", customer_token.to_string())], "读取资料卡").await
    }

    pub async fn query_count_info(&self, customer_token: &str, game_mode: &str, season: Option<i64>) -> Result<Value> {
        let mut params = vec![("gameMode", game_mode.to_string()), ("token", customer_token.to_string())];
        push_season(&mut params, season);
        self.customer_get("queryCountInfo", &params, "读取统计").await
    }

    pub async fn query_match_list(&self, customer_token: &str, game_mode: &str, page: i64, season: Option<i64>) -> Result<Value> {
        let mut params = vec![
            ("token", customer_token.to_string()),
            ("gameMode", game_mode.to_string()),
            ("page", page.to_string()),
        ];
        push_season(&mut params, season);
        self.customer_get("queryMatchList", &params, "读取对局列表").await
    }

    pub async fn fight_query_match_list(&self, customer_token: &str, game_mode: &str, page: i64, season: Option<i64>) -> Result<Value> {
        let mut params = vec![
            ("token", customer_token.to_string()),
            ("gameMode", game_mode.to_string()),
            ("page", page.to_string()),
        ];
        push_season(&mut params, season);
        self.customer_get("fight/queryMatchList", &params, "读取角斗列表").await
    }

    pub async fn query_match_info(&self, customer_token: &str, match_id: &str) -> Result<Value> {
        self.customer_get(
            "queryMatchInfo",
            &[("matchId", match_id.to_string()), ("token", customer_token.to_string())],
            "读取对局详情",
        )
        .await
    }

    pub async fn fight_query_match_info(&self, customer_token: &str, match_id: &str) -> Result<Value> {
        self.customer_get(
            "fight/queryMatchInfo",
            &[("matchId", match_id.to_string()), ("token", customer_token.to_string())],
            "读取角斗详情",
        )
        .await
    }

    async fn customer_get(&self, path: &str, params: &[(&str, String)], context: &str) -> Result<Value> {
        self.get_json(&format!("{}/{}", self.base_url.trim_end_matches('/'), path), params, context).await
    }

    async fn get_json(&self, url: &str, params: &[(&str, String)], context: &str) -> Result<Value> {
        let response = self.client.get(url).query(params).send().await.map_err(|_| anyhow!("{context}请求失败"))?;
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(anyhow!("登录凭据失效，请重新登录"));
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(anyhow!("请求过快，请稍后重试"));
        }
        if !status.is_success() {
            return Err(anyhow!("{context}返回 HTTP {}", status.as_u16()));
        }
        response.json::<Value>().await.with_context(|| format!("{context}返回了无效 JSON"))
    }
}

fn auth_headers(credentials: &DashenCredentials) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/json, text/plain, */*"));
    headers.insert("GL-Bigdata-Auth-Token", HeaderValue::from_str(&credentials.token)?);
    headers.insert("GL-Bigdata-Dts", HeaderValue::from_str(&credentials.dts.unwrap_or(BIGDATA_DTS).to_string())?);
    headers.insert("GL-Bigdata-Role-Id", HeaderValue::from_str(&credentials.role_id.to_string())?);
    headers.insert("GL-Bigdata-Server", HeaderValue::from_str(&credentials.server.to_string())?);
    headers.insert("GL-ClientType", HeaderValue::from_static(CLIENT_TYPE));
    headers.insert("Origin", HeaderValue::from_static(ORIGIN));
    headers.insert("Referer", HeaderValue::from_static(REFERER));
    headers.insert("User-Agent", HeaderValue::from_static(USER_AGENT));
    Ok(headers)
}

fn push_season(params: &mut Vec<(&'static str, String)>, season: Option<i64>) {
    if let Some(season) = season {
        params.push(("season", season.to_string()));
    }
}
```

- [ ] **Step 4: Export module in `lib.rs`**

Add:

```rust
pub mod dashen_client;
```

- [ ] **Step 5: Run protocol test**

Run:

```powershell
cargo test dashen_client_uses_account_token_for_auth_and_customer_token_for_query
```

Expected:

```text
test dashen_client_uses_account_token_for_auth_and_customer_token_for_query ... ok
```

---

### Task 4: Implement Full-Season JSON Fetcher

**Files:**
- Create: `apps/owstats-desktop/src-tauri/src/season_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/sync.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Write failing JSON fetch test**

Add to `phase8_real_fetch.rs`:

```rust
use owstats_desktop_lib::season_fetch::{fetch_full_season_with_base_url, FullSeasonRequest};

#[tokio::test]
async fn full_season_fetch_saves_python_shaped_payload_without_secrets() {
    let server = spawn_full_season_server();
    let payload = fetch_full_season_with_base_url(FullSeasonRequest {
        credentials: DashenCredentials {
            role_id: 123,
            token: "account-auth-token".to_string(),
            dts: None,
            server: 1,
        },
        customer_token: "player-customer-token".to_string(),
        input_bnet: "Tester#12345".to_string(),
        resolved_bnet: Some("Tester#12345".to_string()),
        season: 23,
        customer_base_url: format!("{}/customer", server.base_url),
        pages_per_batch: 2,
        detail_concurrency: 2,
        include_fight: true,
    })
    .await
    .unwrap()
    .payload;

    let text = serde_json::to_string(&payload).unwrap();
    assert_eq!(payload["fetch_mode"], "full_season");
    assert_eq!(payload["season"], 23);
    assert_eq!(payload["matches"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(payload["matches"]["details"].as_array().unwrap().len(), 2);
    assert_eq!(payload["matches"]["detail_errors"].as_array().unwrap().len(), 1);
    assert!(!text.contains("account-auth-token"));
    assert!(!text.contains("player-customer-token"));
    assert!(!text.to_ascii_lowercase().contains("customertoken"));
}
```

Implement `spawn_full_season_server()` in the same test file using the recording server pattern. It must return:

- `/customer/queryCard` -> `{"code":0,"data":{"name":"Tester#12345"}}`
- `/customer/queryCountInfo` -> `{"code":0,"data":{"total":1}}`
- `/customer/queryMatchList` page 1 for `sport` -> two matches, page 2 -> empty
- `/customer/queryMatchList` page 1 for `leisure` -> empty
- `/customer/fight/queryMatchList` page 1 for `SportFight` -> one match, page 2 -> empty
- detail for two match IDs succeeds, one match ID returns HTTP 500

- [ ] **Step 2: Run failing JSON fetch test**

Run:

```powershell
cargo test full_season_fetch_saves_python_shaped_payload_without_secrets
```

Expected:

```text
FAILED
```

The failure should be unresolved module/function.

- [ ] **Step 3: Implement `season_fetch.rs`**

Create `apps/owstats-desktop/src-tauri/src/season_fetch.rs` with these public types:

```rust
use crate::dashen_client::{DashenClient, DashenCredentials};
use crate::importer::string_field;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashSet;

const NORMAL_HISTORY_MODES: &[&str] = &["sport", "leisure"];
const FIGHT_HISTORY_MODES: &[&str] = &["QuickFight", "LeisureFight", "SportFight"];
const MAX_PAGES_PER_MODE: i64 = 80;

pub struct FullSeasonRequest {
    pub credentials: DashenCredentials,
    pub customer_token: String,
    pub input_bnet: String,
    pub resolved_bnet: Option<String>,
    pub season: i64,
    pub customer_base_url: String,
    pub pages_per_batch: i64,
    pub detail_concurrency: usize,
    pub include_fight: bool,
}

pub struct FullSeasonFetch {
    pub payload: Value,
    pub entries: usize,
    pub details: usize,
    pub failed_details: usize,
}
```

Core implementation outline:

```rust
pub async fn fetch_full_season_with_base_url(request: FullSeasonRequest) -> Result<FullSeasonFetch> {
    if request.customer_token.trim().is_empty() {
        return Err(anyhow!("请先绑定目标玩家"));
    }
    let client = DashenClient::new(request.credentials, request.customer_base_url)?;

    let profile_card = client.query_card(&request.customer_token).await.unwrap_or_else(|_| json!({}));
    let sport_count = client.query_count_info(&request.customer_token, "sport", Some(request.season)).await.unwrap_or_else(|_| json!({}));
    let leisure_count = client.query_count_info(&request.customer_token, "leisure", Some(request.season)).await.unwrap_or_else(|_| json!({}));

    let mut entries = Vec::new();
    let mut list_pages = Vec::new();
    for mode in NORMAL_HISTORY_MODES {
        let (mode_entries, mode_pages) = fetch_history_for_mode(&client, &request.customer_token, mode, request.season, request.pages_per_batch, false).await?;
        entries.extend(mode_entries);
        list_pages.extend(mode_pages);
    }
    if request.include_fight {
        for mode in FIGHT_HISTORY_MODES {
            let (mode_entries, mode_pages) = fetch_history_for_mode(&client, &request.customer_token, mode, request.season, request.pages_per_batch, true).await?;
            entries.extend(mode_entries);
            list_pages.extend(mode_pages);
        }
    }
    entries = dedupe_entries(entries, request.season);

    let (details, detail_errors) = fetch_details(&client, &request.customer_token, &entries).await;
    let payload = build_payload(
        &request,
        profile_card,
        sport_count,
        leisure_count,
        entries,
        details,
        detail_errors,
        list_pages,
    );
    let entries_count = payload["matches"]["entries"].as_array().map(Vec::len).unwrap_or(0);
    let details_count = payload["matches"]["details"].as_array().map(Vec::len).unwrap_or(0);
    let failed_details = payload["matches"]["detail_errors"].as_array().map(Vec::len).unwrap_or(0);

    Ok(FullSeasonFetch {
        payload,
        entries: entries_count,
        details: details_count,
        failed_details,
    })
}
```

Implement helpers in the same file:

- `fetch_history_for_mode`
- `extract_match_entries`
- `dedupe_entries`
- `fetch_details`
- `is_fight_match`
- `build_payload`
- `strip_sensitive_fields`

Use existing logic from `sync.rs` where possible, but move it into this focused module.

- [ ] **Step 4: Export `season_fetch` in `lib.rs`**

Add:

```rust
pub mod season_fetch;
```

- [ ] **Step 5: Run JSON fetch test**

Run:

```powershell
cargo test full_season_fetch_saves_python_shaped_payload_without_secrets
```

Expected:

```text
test full_season_fetch_saves_python_shaped_payload_without_secrets ... ok
```

---

### Task 5: Wire JSON-First Sync to Target Player

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/sync.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/sync_client.rs`

- [ ] **Step 1: Write failing sync orchestration test**

Add to `phase8_real_fetch.rs`:

```rust
#[tokio::test]
async fn sync_writes_json_before_importing_sqlite() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(project_root.join("data")).unwrap();
    std::fs::create_dir_all(&app_data).unwrap();

    std::fs::write(
        app_data.join(".dashen_credentials.json"),
        r#"{"role_id":123,"token":"account-auth-token","dts":2026,"server":1}"#,
    )
    .unwrap();
    save_player_target_for_path(
        &app_data.join(".dashen_player_target.json"),
        &PlayerTarget {
            input_bnet: "Tester#12345".to_string(),
            resolved_bnet: Some("Tester#12345".to_string()),
            customer_token: "player-customer-token".to_string(),
            bound_at: "2026-07-03T00:00:00Z".to_string(),
        },
    )
    .unwrap();

    let server = spawn_full_season_server();
    let db = OwDatabase::new(temp.path().join("sync.sqlite3"));

    let summary = owstats_desktop_lib::sync_season_with_credentials_and_target_paths_base_url(
        &db,
        &project_root,
        &app_data.join(".dashen_credentials.json"),
        &app_data.join(".dashen_player_target.json"),
        23,
        &format!("{}/customer", server.base_url),
        |_| {},
    )
    .await
    .unwrap();

    let raw_path = project_root.join("data").join("players").join("local_s23_full_season.json");
    assert!(raw_path.exists());
    assert_eq!(summary.imported_matches, 3);
}
```

- [ ] **Step 2: Run failing orchestration test**

Run:

```powershell
cargo test sync_writes_json_before_importing_sqlite
```

Expected:

```text
FAILED
```

The failure should be unresolved function.

- [ ] **Step 3: Implement new sync function**

In `sync.rs`, add:

```rust
pub async fn sync_season_with_credentials_and_target_paths_base_url(
    db: &OwDatabase,
    project_root: &Path,
    credential_path: &Path,
    target_path: &Path,
    season: i64,
    customer_base_url: &str,
    mut progress: impl FnMut(SyncProgress),
) -> Result<SyncSummary> {
    db.initialize()?;
    let credentials = read_credentials(credential_path)?;
    let target = crate::player_target::read_player_target_for_path(target_path)?;
    let fetch = crate::season_fetch::fetch_full_season_with_base_url(crate::season_fetch::FullSeasonRequest {
        credentials: crate::dashen_client::DashenCredentials {
            role_id: credentials.role_id,
            token: credentials.token,
            dts: credentials.dts,
            server: credentials.server.unwrap_or(1),
        },
        customer_token: target.customer_token,
        input_bnet: target.input_bnet,
        resolved_bnet: target.resolved_bnet,
        season,
        customer_base_url: customer_base_url.to_string(),
        pages_per_batch: 5,
        detail_concurrency: 2,
        include_fight: true,
    })
    .await?;

    write_season_payload(&project_root.join("data").join("players"), season, &fetch.payload)?;
    if fetch.entries == 0 {
        return Err(anyhow!("未拉取到 S{season} 赛季对局；已保存本地 JSON，请确认该赛季有战绩，或稍后重新登录后再试。"));
    }

    let entries = fetch.payload.pointer("/matches/entries").and_then(Value::as_array).cloned().unwrap_or_default();
    let details = fetch.payload.pointer("/matches/details").and_then(Value::as_array).cloned().unwrap_or_default();
    let detail_errors = fetch.payload.pointer("/matches/detail_errors").and_then(Value::as_array).cloned().unwrap_or_default();

    progress(SyncProgress {
        season,
        phase: "写入 SQLite".to_string(),
        processed: fetch.entries,
        total: fetch.entries,
        current_match_id: None,
        failed_details: detail_errors.len(),
        latest_errors: Vec::new(),
    });

    sync_import_parts(db, project_root, season, &fetch.payload, entries, details, detail_errors, 0)
}
```

Keep old test-only functions until callers are migrated, but mark internal comments so future cleanup is clear.

- [ ] **Step 4: Update `commands.rs` sync path**

In `sync_season`, compute both paths:

```rust
let credential_path = crate::credentials::credential_path(&app).map_err(to_message)?;
let target_path = crate::player_target::player_target_path(&app).map_err(to_message)?;
sync_season_with_credentials_and_target_paths_base_url(
    &db,
    &root,
    &credential_path,
    &target_path,
    request.season,
    "https://datamsapi.ds.163.com/v1/a19ld5tool/customer",
    |progress| {
        let _ = app.emit("sync-progress", &progress);
    },
)
```

Preserve the existing progress emit event.

- [ ] **Step 5: Run orchestration test**

Run:

```powershell
cargo test sync_writes_json_before_importing_sqlite
```

Expected:

```text
test sync_writes_json_before_importing_sqlite ... ok
```

- [ ] **Step 6: Run backend regression**

Run:

```powershell
cargo test --tests
```

Expected:

```text
test result: ok
```

---

### Task 6: Add Bind Target Player Commands

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/lib.rs`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Add command functions**

In `commands.rs`, add Tauri commands:

```rust
#[tauri::command]
pub fn get_player_target_status(app: AppHandle) -> Result<PlayerTargetStatus, String> {
    let path = crate::player_target::player_target_path(&app).map_err(to_message)?;
    crate::player_target::player_target_status_for_path(&path).map_err(to_message)
}

#[tauri::command]
pub fn clear_player_target(app: AppHandle) -> Result<ClearPlayerTargetResult, String> {
    let path = crate::player_target::player_target_path(&app).map_err(to_message)?;
    let cleared = crate::player_target::clear_player_target_for_path(&path).map_err(to_message)?;
    let status = crate::player_target::player_target_status_for_path(&path).map_err(to_message)?;
    Ok(ClearPlayerTargetResult { cleared, status })
}
```

Do not add `bind_player_target` until `search_bnet_account` is wired to a real command in Task 7.

- [ ] **Step 2: Register commands in `lib.rs`**

Add to `invoke_handler`:

```rust
commands::get_player_target_status,
commands::clear_player_target,
```

- [ ] **Step 3: Run compile check**

Run:

```powershell
cargo test --tests
```

Expected:

```text
test result: ok
```

---

### Task 7: Add Frontend Target Player UI

**Files:**
- Modify: `apps/owstats-desktop/src/shared/types.ts`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/src/styles.css`
- Create: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`
- Modify: `apps/owstats-desktop/package.json`

- [ ] **Step 1: Add TypeScript DTOs**

In `types.ts`, add:

```ts
export type PlayerTargetStatus = {
  state: "missing" | "bound" | string;
  inputBnet?: string | null;
  resolvedBnet?: string | null;
  boundAt?: string | null;
};

export type BindPlayerTargetResult = {
  status: PlayerTargetStatus;
};

export type ClearPlayerTargetResult = {
  cleared: boolean;
  status: PlayerTargetStatus;
};
```

- [ ] **Step 2: Add API wrappers**

In `api.ts`, add:

```ts
export function getPlayerTargetStatus() {
  return invoke<PlayerTargetStatus>("get_player_target_status");
}

export function clearPlayerTarget() {
  return invoke<ClearPlayerTargetResult>("clear_player_target");
}
```

Do not add `bindPlayerTarget` until backend binding command is implemented.

- [ ] **Step 3: Add source-level frontend test**

Create `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strict as assert } from "node:assert";

const root = process.cwd();
const onboarding = readFileSync(join(root, "src/pages/OnboardingPage.tsx"), "utf8");
const api = readFileSync(join(root, "src/shared/api.ts"), "utf8");
const types = readFileSync(join(root, "src/shared/types.ts"), "utf8");

assert.match(types, /PlayerTargetStatus/, "types should define PlayerTargetStatus");
assert.match(api, /get_player_target_status/, "api should expose player target status command");
assert.match(api, /clear_player_target/, "api should expose clear player target command");
assert.match(onboarding, /目标玩家/, "command center should show target player area");
assert.match(onboarding, /请先绑定目标玩家/, "sync cards should explain missing target player");
assert.doesNotMatch(onboarding, /同步全部/, "command center must not add sync-all");
assert.doesNotMatch(onboarding, /dump_season_json/, "dump command must not be wired into UI");
```

- [ ] **Step 4: Run failing frontend source test**

Run:

```powershell
cd apps\owstats-desktop
npx tsx scripts/phase8RealFetch.test.ts
```

Expected:

```text
AssertionError
```

- [ ] **Step 5: Update `OnboardingPage.tsx`**

Add state:

```tsx
const [playerTarget, setPlayerTarget] = useState<PlayerTargetStatus | null>(null);
```

Load it with onboarding status:

```tsx
const [statusResult, reviewResult, targetResult] = await Promise.all([
  getOnboardingStatus(),
  getPersonalReview(DEFAULT_REVIEW_SEASON).catch(() => null),
  getPlayerTargetStatus().catch(() => null),
]);
setPlayerTarget(targetResult);
```

Add a compact target block near login/status:

```tsx
<section className="commandSection targetPlayerSection" aria-label="目标玩家">
  <div>
    <p className="eyebrow">目标玩家</p>
    <h2>{playerTarget?.state === "bound" ? playerTarget.resolvedBnet ?? playerTarget.inputBnet : "未绑定目标玩家"}</h2>
    <p className="muted">{playerTarget?.state === "bound" ? "目标玩家已绑定，可同步赛季数据" : "请先绑定目标玩家"}</p>
  </div>
  <button type="button" className="secondaryButton" onClick={runClearPlayerTarget} disabled={playerTarget?.state !== "bound"}>
    清除目标玩家
  </button>
</section>
```

For now this exposes status and clear only. Binding UI lands in the next task when backend search is implemented.

Update season card disabled condition:

```tsx
const playerTargetReady = playerTarget?.state === "bound";
const syncDisabled = !credentialReady || !playerTargetReady || isSyncBlocked || isSyncing;
```

Card status text:

```tsx
const statusLabel = !credentialReady
  ? "请先登录网易大神"
  : !playerTargetReady
    ? "请先绑定目标玩家"
    : isSyncing
      ? "同步中"
      : "登录凭据可用";
```

- [ ] **Step 6: Run frontend source test**

Run:

```powershell
npx tsx scripts/phase8RealFetch.test.ts
```

Expected: command exits 0.

- [ ] **Step 7: Add test to `npm test`**

Add `&& tsx scripts/phase8RealFetch.test.ts` to the `test` script in `apps/owstats-desktop/package.json` after the existing command-center/source-level checks.

- [ ] **Step 8: Run frontend checks**

Run:

```powershell
npm test
npm run build
```

Expected:

```text
test exits 0
vite build exits 0
```

---

### Task 8: Add BattleTag Search and Bind Command

**Files:**
- Modify: `apps/owstats-desktop/src-tauri/src/dashen_client.rs`
- Modify: `apps/owstats-desktop/src-tauri/src/commands.rs`
- Modify: `apps/owstats-desktop/src/shared/api.ts`
- Modify: `apps/owstats-desktop/src/pages/OnboardingPage.tsx`
- Modify: `apps/owstats-desktop/scripts/phase8RealFetch.test.ts`
- Modify: `apps/owstats-desktop/src-tauri/tests/phase8_real_fetch.rs`

- [ ] **Step 1: Write backend bind test**

Add a command-level helper test if commands are hard to call directly, or add a pure function in `player_target.rs`:

```rust
#[test]
fn extracts_customer_token_from_search_payload() {
    let payload = serde_json::json!({
        "code": 0,
        "data": {
            "customerToken": "player-customer-token",
            "bnetId": "Tester#12345"
        }
    });

    let parsed = owstats_desktop_lib::player_target::target_from_search_payload(
        "Tester#12345",
        &payload,
        "2026-07-03T00:00:00Z",
    )
    .unwrap();

    assert_eq!(parsed.input_bnet, "Tester#12345");
    assert_eq!(parsed.resolved_bnet.as_deref(), Some("Tester#12345"));
    assert_eq!(parsed.customer_token, "player-customer-token");
}
```

- [ ] **Step 2: Implement `target_from_search_payload`**

In `player_target.rs`, add:

```rust
pub fn target_from_search_payload(input_bnet: &str, payload: &serde_json::Value, bound_at: &str) -> Result<PlayerTarget> {
    let data = payload.get("data").and_then(serde_json::Value::as_object).ok_or_else(|| anyhow::anyhow!("未找到目标玩家"))?;
    let customer_token = data
        .get("customerToken")
        .or_else(|| data.get("customer_token"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("搜索结果缺少玩家标识"))?;
    let resolved_bnet = data
        .get("bnetId")
        .or_else(|| data.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Ok(PlayerTarget {
        input_bnet: input_bnet.trim().to_string(),
        resolved_bnet,
        customer_token: customer_token.to_string(),
        bound_at: bound_at.to_string(),
    })
}
```

- [ ] **Step 3: Add `bind_player_target` command**

In `commands.rs`:

```rust
#[tauri::command]
pub async fn bind_player_target(app: AppHandle, request: BindPlayerTargetRequest) -> Result<BindPlayerTargetResult, String> {
    let bnet = request.bnet.trim();
    if bnet.is_empty() {
        return Err("请输入 BattleTag".to_string());
    }
    let credential_path = crate::credentials::credential_path(&app).map_err(to_message)?;
    let credentials = crate::sync::read_credentials_for_command(&credential_path).map_err(to_message)?;
    let client = crate::dashen_client::DashenClient::new(
        crate::dashen_client::DashenCredentials {
            role_id: credentials.role_id,
            token: credentials.token,
            dts: credentials.dts,
            server: credentials.server.unwrap_or(1),
        },
        "https://datamsapi.ds.163.com/v1/a19ld5tool/customer".to_string(),
    )
    .map_err(to_message)?;
    let payload = client.search_bnet_account(bnet).await.map_err(to_message)?;
    let target = crate::player_target::target_from_search_payload(bnet, &payload, &chrono::Utc::now().to_rfc3339()).map_err(to_message)?;
    let path = crate::player_target::player_target_path(&app).map_err(to_message)?;
    crate::player_target::save_player_target_for_path(&path, &target).map_err(to_message)?;
    let status = crate::player_target::player_target_status_for_path(&path).map_err(to_message)?;
    Ok(BindPlayerTargetResult { status })
}
```

If `sync::read_credentials` is private, expose a narrow `read_credentials_for_command` that returns a non-secret struct for internal command use only. Do not print the credentials.

- [ ] **Step 4: Register command**

In `lib.rs`, add:

```rust
commands::bind_player_target,
```

- [ ] **Step 5: Add frontend API**

In `api.ts`:

```ts
export function bindPlayerTarget(bnet: string) {
  return invoke<BindPlayerTargetResult>("bind_player_target", { request: { bnet } });
}
```

- [ ] **Step 6: Add UI input**

In `OnboardingPage.tsx`, add:

```tsx
const [targetBnetInput, setTargetBnetInput] = useState("");
const [targetBinding, setTargetBinding] = useState(false);

async function runBindPlayerTarget() {
  setTargetBinding(true);
  try {
    const result = await bindPlayerTarget(targetBnetInput);
    setPlayerTarget(result.status);
  } finally {
    setTargetBinding(false);
  }
}
```

Render:

```tsx
<input
  className="targetPlayerInput"
  value={targetBnetInput}
  onChange={(event) => setTargetBnetInput(event.target.value)}
  placeholder="玩家名#12345"
/>
<button type="button" className="primaryButton" onClick={runBindPlayerTarget} disabled={!credentialReady || targetBinding}>
  解析并绑定
</button>
```

- [ ] **Step 7: Extend frontend source test**

Add assertions:

```ts
assert.match(api, /bind_player_target/, "api should expose bind player target command");
assert.match(onboarding, /解析并绑定/, "target player UI should bind BattleTag");
assert.match(onboarding, /玩家名#12345/, "target player input should show Chinese placeholder example");
```

- [ ] **Step 8: Run full backend and frontend checks**

Run:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
cargo build --bin dump_season_json
cd ..\
npm test
npm run build
```

Expected:

```text
all commands exit 0
```

---

## Final Verification

Run from repo root or listed subdirectories:

```powershell
cd apps\owstats-desktop\src-tauri
cargo fmt --check
cargo test --tests
cargo build --bin dump_season_json
cd ..\
npm test
npm run build
cd ..\..\
git diff --check
```

Expected:

- `cargo fmt --check` exits 0.
- `cargo test --tests` exits 0.
- `cargo build --bin dump_season_json` exits 0.
- `npm test` exits 0.
- `npm run build` exits 0, allowing the existing Vite large chunk warning.
- `git diff --check` exits 0, allowing LF/CRLF warnings.

## Manual Verification

Manual real upstream verification is optional and must be explicitly approved by the main thread because it uses live credentials and network.

If approved:

1. Start the Tauri app.
2. Log in with NetEase Dashen.
3. Bind a BattleTag.
4. Click `同步 S23`.
5. Confirm `apps/owstats-desktop/data/players/local_s23_full_season.json` exists.
6. Inspect only the sanitized JSON structure and counts; do not print credentials, cookies, or browser storage.
7. Confirm S23 card count updates after import.

## Scope Gaps Left Deliberately Out

- Multi-player target history.
- Sync-all.
- Global current season.
- Direct Python runtime dependency.
- `dump_season_json` UI integration.
- Broad layout changes outside the command center.
