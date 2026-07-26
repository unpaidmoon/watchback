use anyhow::Result;
use serde_json::Value;
use std::{future::Future, pin::Pin};

pub type UpstreamFuture<'a> = Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;

pub trait PlayerQueryUpstream: Send + Sync {
    fn search<'a>(&'a self, battle_tag: &'a str) -> UpstreamFuture<'a>;
    fn card<'a>(&'a self, customer_token: &'a str) -> UpstreamFuture<'a>;
    fn count_info<'a>(
        &'a self,
        customer_token: &'a str,
        mode: &'a str,
        season: Option<i64>,
    ) -> UpstreamFuture<'a>;
    fn fight_count<'a>(&'a self, _: &'a str, _: Option<i64>) -> UpstreamFuture<'a> {
        Box::pin(async { anyhow::bail!("fight count unavailable") })
    }
    fn leisure_fight_card<'a>(&'a self, _: &'a str, _: Option<i64>) -> UpstreamFuture<'a> {
        Box::pin(async { anyhow::bail!("leisure fight card unavailable") })
    }
    fn match_list<'a>(
        &'a self,
        customer_token: &'a str,
        mode: &'a str,
        page: u32,
        season: Option<i64>,
        arena: bool,
    ) -> UpstreamFuture<'a>;
    fn match_detail<'a>(
        &'a self,
        customer_token: &'a str,
        match_id: &'a str,
        arena: bool,
    ) -> UpstreamFuture<'a>;
    fn image<'a>(&'a self, _url: &'a str, _max_bytes: usize) -> UpstreamFuture<'a> {
        Box::pin(async { anyhow::bail!("image unavailable") })
    }
}

impl PlayerQueryUpstream for crate::dashen_client::DashenClient {
    fn search<'a>(&'a self, battle_tag: &'a str) -> UpstreamFuture<'a> {
        Box::pin(self.search_bnet_account(battle_tag))
    }
    fn card<'a>(&'a self, token: &'a str) -> UpstreamFuture<'a> {
        Box::pin(self.query_card(token))
    }
    fn count_info<'a>(
        &'a self,
        token: &'a str,
        mode: &'a str,
        season: Option<i64>,
    ) -> UpstreamFuture<'a> {
        Box::pin(self.query_count_info(token, mode, season))
    }
    fn fight_count<'a>(&'a self, token: &'a str, season: Option<i64>) -> UpstreamFuture<'a> {
        Box::pin(self.fight_query_count(token, season))
    }
    fn leisure_fight_card<'a>(&'a self, token: &'a str, season: Option<i64>) -> UpstreamFuture<'a> {
        Box::pin(self.fight_leisure_role_card(token, season))
    }
    fn match_list<'a>(
        &'a self,
        token: &'a str,
        mode: &'a str,
        page: u32,
        season: Option<i64>,
        arena: bool,
    ) -> UpstreamFuture<'a> {
        if arena {
            Box::pin(self.fight_query_match_list(token, mode, page.into(), season))
        } else {
            Box::pin(self.query_match_list(token, mode, page.into(), season))
        }
    }
    fn match_detail<'a>(&'a self, token: &'a str, id: &'a str, arena: bool) -> UpstreamFuture<'a> {
        if arena {
            Box::pin(self.fight_query_match_info(token, id))
        } else {
            Box::pin(self.query_match_info(token, id))
        }
    }
}

/// Production adapter: game data shares one Dashen request gate while public images are
/// downloaded separately into bounded memory and are never handed to the WebView as URLs.
pub struct DashenPlayerQueryUpstream {
    dashen: crate::dashen_client::DashenClient,
    images: reqwest::Client,
}
impl DashenPlayerQueryUpstream {
    pub fn new(dashen: crate::dashen_client::DashenClient) -> Result<Self> {
        Ok(Self {
            dashen,
            images: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::limited(3))
                .build()?,
        })
    }
}
impl PlayerQueryUpstream for DashenPlayerQueryUpstream {
    fn search<'a>(&'a self, v: &'a str) -> UpstreamFuture<'a> {
        Box::pin(self.dashen.search_bnet_account(v))
    }
    fn card<'a>(&'a self, v: &'a str) -> UpstreamFuture<'a> {
        Box::pin(self.dashen.query_card(v))
    }
    fn count_info<'a>(&'a self, t: &'a str, m: &'a str, s: Option<i64>) -> UpstreamFuture<'a> {
        Box::pin(self.dashen.query_count_info(t, m, s))
    }
    fn fight_count<'a>(&'a self, token: &'a str, season: Option<i64>) -> UpstreamFuture<'a> {
        Box::pin(self.dashen.fight_query_count(token, season))
    }
    fn leisure_fight_card<'a>(&'a self, token: &'a str, season: Option<i64>) -> UpstreamFuture<'a> {
        Box::pin(self.dashen.fight_leisure_role_card(token, season))
    }
    fn match_list<'a>(
        &'a self,
        t: &'a str,
        m: &'a str,
        p: u32,
        s: Option<i64>,
        a: bool,
    ) -> UpstreamFuture<'a> {
        if a {
            Box::pin(self.dashen.fight_query_match_list(t, m, p.into(), s))
        } else {
            Box::pin(self.dashen.query_match_list(t, m, p.into(), s))
        }
    }
    fn match_detail<'a>(&'a self, t: &'a str, id: &'a str, a: bool) -> UpstreamFuture<'a> {
        if a {
            Box::pin(self.dashen.fight_query_match_info(t, id))
        } else {
            Box::pin(self.dashen.query_match_info(t, id))
        }
    }
    fn image<'a>(&'a self, url: &'a str, max: usize) -> UpstreamFuture<'a> {
        Box::pin(async move {
            let response = self.images.get(url).send().await?;
            if !response.status().is_success()
                || response.content_length().is_some_and(|n| n > max as u64)
            {
                anyhow::bail!("image unavailable")
            }
            let mime = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .split(';')
                .next()
                .unwrap_or_default()
                .to_string();
            let bytes = response.bytes().await?;
            if bytes.len() > max {
                anyhow::bail!("image unavailable")
            }
            Ok(serde_json::json!({"mimeType":mime,"bytes":bytes.to_vec()}))
        })
    }
}
