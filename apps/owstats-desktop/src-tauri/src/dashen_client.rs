use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, OwnedSemaphorePermit, Semaphore};

const CLIENT_TYPE: &str = "60";
const ORIGIN: &str = "https://act.ds.163.com";
const REFERER: &str = "https://act.ds.163.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36 app/df_client dfVersion/100111";
const BIGDATA_DTS: i64 = 2026;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashenCredentials {
    pub role_id: i64,
    pub token: String,
    pub dts: Option<i64>,
    pub server: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashenRateLimitConfig {
    pub datamsapi_max_concurrent: usize,
    pub account_max_requests_per_window: usize,
    pub account_window_ms: u64,
    pub max_http_attempts: usize,
    pub retry_base_ms: u64,
}

impl Default for DashenRateLimitConfig {
    fn default() -> Self {
        Self {
            datamsapi_max_concurrent: 2,
            account_max_requests_per_window: 5,
            account_window_ms: 1_000,
            max_http_attempts: 3,
            retry_base_ms: 5_000,
        }
    }
}

pub struct DashenClient {
    client: reqwest::Client,
    base_url: String,
    credentials: DashenCredentials,
    request_gate: Arc<DashenRequestGate>,
}

impl DashenClient {
    pub fn new(credentials: DashenCredentials, base_url: String) -> Result<Self> {
        Self::new_with_rate_limit(credentials, base_url, DashenRateLimitConfig::default())
    }

    pub fn new_with_rate_limit(
        credentials: DashenCredentials,
        base_url: String,
        rate_limit: DashenRateLimitConfig,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .default_headers(auth_headers(&credentials)?)
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            credentials,
            request_gate: Arc::new(DashenRequestGate::new(rate_limit)),
        })
    }

    pub async fn search_bnet_account(&self, bnet: &str) -> Result<Value> {
        let payload = json!({
            "token": self.credentials.token,
            "roleId": self.credentials.role_id,
            "dts": self.credentials.dts.unwrap_or(BIGDATA_DTS),
            "server": self.credentials.server,
            "name": normalize_bnet(bnet),
        });
        self.post_json(
            &format!("{}/searchBnetAccount", self.root_base_url()),
            &payload,
            "搜索目标玩家",
        )
        .await
    }

    pub async fn query_card(&self, customer_token: &str) -> Result<Value> {
        self.customer_get(
            "queryCard",
            &[("token", customer_token.to_string())],
            "读取资料卡",
        )
        .await
    }

    pub async fn query_count_info(
        &self,
        customer_token: &str,
        game_mode: &str,
        season: Option<i64>,
    ) -> Result<Value> {
        let mut params = vec![
            ("gameMode", game_mode.to_string()),
            ("token", customer_token.to_string()),
        ];
        push_season(&mut params, season);
        self.customer_get("queryCountInfo", &params, "读取统计")
            .await
    }

    pub async fn query_match_list(
        &self,
        customer_token: &str,
        game_mode: &str,
        page: i64,
        season: Option<i64>,
    ) -> Result<Value> {
        let mut params = vec![
            ("token", customer_token.to_string()),
            ("gameMode", game_mode.to_string()),
            ("page", page.to_string()),
        ];
        push_season(&mut params, season);
        self.customer_get("queryMatchList", &params, "读取对局列表")
            .await
    }

    pub async fn fight_query_match_list(
        &self,
        customer_token: &str,
        game_mode: &str,
        page: i64,
        season: Option<i64>,
    ) -> Result<Value> {
        let mut params = vec![
            ("token", customer_token.to_string()),
            ("gameMode", game_mode.to_string()),
            ("page", page.to_string()),
        ];
        push_season(&mut params, season);
        self.customer_get("fight/queryMatchList", &params, "读取斗技列表")
            .await
    }

    pub async fn query_match_info(&self, customer_token: &str, match_id: &str) -> Result<Value> {
        self.customer_get(
            "queryMatchInfo",
            &[
                ("matchId", match_id.to_string()),
                ("token", customer_token.to_string()),
            ],
            "读取对局详情",
        )
        .await
    }

    pub async fn fight_query_match_info(
        &self,
        customer_token: &str,
        match_id: &str,
    ) -> Result<Value> {
        self.customer_get(
            "fight/queryMatchInfo",
            &[
                ("matchId", match_id.to_string()),
                ("token", customer_token.to_string()),
            ],
            "读取斗技详情",
        )
        .await
    }

    async fn customer_get(
        &self,
        method: &str,
        params: &[(&str, String)],
        context: &str,
    ) -> Result<Value> {
        self.get_json(
            &format!("{}/{}", self.customer_base_url(), method),
            params,
            context,
        )
        .await
    }

    async fn get_json(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        context: &str,
    ) -> Result<Value> {
        for attempt in 1..=self.request_gate.max_http_attempts() {
            let _permit = self.request_gate.acquire().await;
            let response = self
                .client
                .get(endpoint)
                .query(params)
                .send()
                .await
                .map_err(|error| anyhow!("{context}: {error}"))?;
            let status = response.status();
            if status == StatusCode::TOO_MANY_REQUESTS
                && attempt < self.request_gate.max_http_attempts()
            {
                tokio::time::sleep(self.request_gate.retry_delay(attempt, &response)).await;
                continue;
            }
            if !status.is_success() {
                let error = response.error_for_status_ref().unwrap_err();
                return Err(anyhow!("{context}: {error}"));
            }
            return response
                .json::<Value>()
                .await
                .map_err(|error| anyhow!("{context}: {error}"));
        }
        Err(anyhow!("{context} 请求失败"))
    }

    async fn post_json(&self, endpoint: &str, payload: &Value, context: &str) -> Result<Value> {
        for attempt in 1..=self.request_gate.max_http_attempts() {
            let _permit = self.request_gate.acquire().await;
            let response = self
                .client
                .post(endpoint)
                .json(payload)
                .send()
                .await
                .map_err(|error| anyhow!("{context}: {error}"))?;
            let status = response.status();
            if status == StatusCode::TOO_MANY_REQUESTS
                && attempt < self.request_gate.max_http_attempts()
            {
                tokio::time::sleep(self.request_gate.retry_delay(attempt, &response)).await;
                continue;
            }
            if !status.is_success() {
                let error = response.error_for_status_ref().unwrap_err();
                return Err(anyhow!("{context}: {error}"));
            }
            return response
                .json::<Value>()
                .await
                .map_err(|error| anyhow!("{context}: {error}"));
        }
        Err(anyhow!("{context} 请求失败"))
    }

    fn root_base_url(&self) -> String {
        self.base_url
            .strip_suffix("/customer")
            .unwrap_or(&self.base_url)
            .to_string()
    }

    fn customer_base_url(&self) -> String {
        if self.base_url.ends_with("/customer") {
            self.base_url.clone()
        } else {
            format!("{}/customer", self.base_url)
        }
    }
}

struct DashenRequestGate {
    config: DashenRateLimitConfig,
    datamsapi: Arc<Semaphore>,
    account: SlidingWindowRateLimiter,
}

impl DashenRequestGate {
    fn new(config: DashenRateLimitConfig) -> Self {
        let normalized = DashenRateLimitConfig {
            datamsapi_max_concurrent: config.datamsapi_max_concurrent.max(1),
            account_max_requests_per_window: config.account_max_requests_per_window.max(1),
            account_window_ms: config.account_window_ms.max(10),
            max_http_attempts: config.max_http_attempts.max(1),
            retry_base_ms: config.retry_base_ms.max(1),
        };
        Self {
            config: normalized,
            datamsapi: Arc::new(Semaphore::new(normalized.datamsapi_max_concurrent)),
            account: SlidingWindowRateLimiter::new(
                normalized.account_max_requests_per_window,
                Duration::from_millis(normalized.account_window_ms),
            ),
        }
    }

    async fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        let permit = self.datamsapi.clone().acquire_owned().await.ok();
        self.account.acquire().await;
        permit
    }

    fn max_http_attempts(&self) -> usize {
        self.config.max_http_attempts
    }

    fn retry_delay(&self, attempt: usize, response: &reqwest::Response) -> Duration {
        response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(parse_retry_after)
            .unwrap_or_else(|| {
                Duration::from_millis(self.config.retry_base_ms.saturating_mul(attempt as u64))
            })
    }
}

struct SlidingWindowRateLimiter {
    limit: usize,
    window: Duration,
    timestamps: AsyncMutex<VecDeque<Instant>>,
}

impl SlidingWindowRateLimiter {
    fn new(limit: usize, window: Duration) -> Self {
        Self {
            limit,
            window,
            timestamps: AsyncMutex::new(VecDeque::new()),
        }
    }

    async fn acquire(&self) {
        loop {
            let mut wait = Duration::ZERO;
            {
                let mut timestamps = self.timestamps.lock().await;
                let now = Instant::now();
                let cutoff = now.checked_sub(self.window).unwrap_or(now);
                while timestamps
                    .front()
                    .map(|timestamp| *timestamp <= cutoff)
                    .unwrap_or(false)
                {
                    timestamps.pop_front();
                }
                if timestamps.len() < self.limit {
                    timestamps.push_back(now);
                    return;
                }
                if let Some(oldest) = timestamps.front() {
                    wait = self
                        .window
                        .saturating_sub(now.saturating_duration_since(*oldest));
                }
            }
            tokio::time::sleep(wait.max(Duration::from_millis(1))).await;
        }
    }
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    let seconds = value.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds))
}

fn normalize_bnet(value: &str) -> String {
    value.replace("锛?", "#").trim().to_string()
}

fn push_season(params: &mut Vec<(&str, String)>, season: Option<i64>) {
    if let Some(season) = season {
        params.push(("season", season.to_string()));
    }
}

fn auth_headers(credentials: &DashenCredentials) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        "GL-Bigdata-Auth-Token",
        HeaderValue::from_str(&credentials.token)?,
    );
    headers.insert(
        "GL-Bigdata-Dts",
        HeaderValue::from_str(&credentials.dts.unwrap_or(BIGDATA_DTS).to_string())?,
    );
    headers.insert(
        "GL-Bigdata-Role-Id",
        HeaderValue::from_str(&credentials.role_id.to_string())?,
    );
    headers.insert(
        "GL-Bigdata-Server",
        HeaderValue::from_str(&credentials.server.to_string())?,
    );
    headers.insert("GL-ClientType", HeaderValue::from_static(CLIENT_TYPE));
    headers.insert("Origin", HeaderValue::from_static(ORIGIN));
    headers.insert("Referer", HeaderValue::from_static(REFERER));
    headers.insert("User-Agent", HeaderValue::from_static(USER_AGENT));
    Ok(headers)
}
