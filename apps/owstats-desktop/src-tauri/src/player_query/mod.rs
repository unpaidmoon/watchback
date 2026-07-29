pub mod mapper;
pub mod models;
mod session;
pub mod upstream;
use models::*;
use session::*;
use std::sync::Arc;
use upstream::PlayerQueryUpstream;

pub struct PlayerQueryService {
    coordinator: crate::sync_coordinator::SyncCoordinator,
    sessions: Sessions,
}
impl PlayerQueryService {
    pub fn new(coordinator: crate::sync_coordinator::SyncCoordinator) -> Self {
        Self {
            coordinator,
            sessions: Sessions::default(),
        }
    }
    pub async fn start_query(
        &self,
        battle_tag: &str,
        upstream: Arc<dyn PlayerQueryUpstream>,
    ) -> QueryResult<SessionStart> {
        let normalized = normalize_battle_tag(battle_tag)?;
        let permit = self
            .coordinator
            .try_acquire_query_session()
            .map_err(|e| QueryError::Busy(e.into()))?;
        let response = upstream.search(&normalized).await.map_err(classify)?;
        let (returned, token) = mapper::identity(&response)?;
        if let Some(returned) = returned {
            if normalize_battle_tag(&returned)? != normalized {
                return Err(QueryError::NotFound("目标身份不匹配".into()));
            }
        }
        let generation = self.sessions.generation();
        let id = format!("pq-{generation}");
        self.sessions.insert(
            id.clone(),
            Session {
                battle_tag: normalized.clone(),
                customer_token: token,
                generation,
                upstream,
                _permit: permit,
            },
        );
        Ok(SessionStart {
            session_id: id,
            generation,
            battle_tag: normalized,
        })
    }
    pub async fn load_card(&self, id: &str) -> QueryResult<PlayerQueryCard> {
        let (tag, token, g, u) = self.sessions.snapshot(id)?;
        let v = u.card(&token).await.map_err(|e| self.error(id, e))?;
        self.ensure_generation(id, g)?;
        mapper::card(&v, tag)
    }
    pub async fn load_card_images(&self, id: &str) -> QueryResult<PlayerQueryCardImages> {
        let (_, token, g, u) = self.sessions.snapshot(id)?;
        let v = u.card(&token).await.map_err(|e| self.error(id, e))?;
        self.ensure_generation(id, g)?;
        mapper::card_images(&v, u.as_ref()).await
    }
    pub async fn load_season(&self, id: &str, season: Season) -> QueryResult<SeasonOverview> {
        let (_, token, g, u) = self.sessions.snapshot(id)?;
        let (sport, fight) = tokio::join!(
            u.count_info(&token, "sport", season.upstream()),
            u.fight_count(&token, season.upstream())
        );
        let competitive = sport
            .as_ref()
            .ok()
            .and_then(|value| mapper::competitive_ranks(value).ok())
            .unwrap_or_default();
        let open_competitive = sport
            .as_ref()
            .ok()
            .and_then(|value| mapper::open_competitive_rank(value).ok())
            .unwrap_or_default();
        let stadium = fight
            .as_ref()
            .ok()
            .and_then(|value| mapper::stadium_rank(value).ok())
            .unwrap_or_default();
        self.ensure_generation(id, g)?;
        Ok(SeasonOverview {
            competitive,
            open_competitive,
            stadium,
        })
    }
    pub async fn load_mode_page(
        &self,
        id: &str,
        season: Season,
        mode: QueryMode,
        page: u32,
    ) -> QueryResult<ModePage> {
        if page == 0 {
            return Err(QueryError::InvalidInput("页码必须从 1 开始".into()));
        }
        let (_, token, g, u) = self.sessions.snapshot(id)?;
        let arena = mode.kind() == MatchKind::Arena;
        let list = u
            .match_list(&token, mode.list_upstream(), page, season.upstream(), arena)
            .await;
        let list = list.map_err(|e| self.error(id, e))?;
        let (rows, raw) = if arena {
            mapper::matches_for_mode(&list, mode.kind(), mode.upstream())?
        } else {
            mapper::matches(&list, mode.kind())?
        };
        self.ensure_generation(id, g)?;
        Ok(ModePage {
            list_scope_key: mode.list_scope_key().to_owned(),
            page,
            matches: rows,
            end_reached: raw < PAGE_SIZE,
        })
    }
    pub async fn load_mode_heroes(
        &self,
        id: &str,
        season: Season,
        mode: QueryMode,
    ) -> QueryResult<Vec<HeroUsage>> {
        let (_, token, g, u) = self.sessions.snapshot(id)?;
        let count = match mode {
            QueryMode::SportFight => u.fight_count(&token, season.upstream()).await,
            QueryMode::LeisureFight => u.leisure_fight_card(&token, season.upstream()).await,
            _ => {
                u.count_info(&token, mode.count_upstream(), season.upstream())
                    .await
            }
        }
        .map_err(|e| self.error(id, e))?;
        let heroes = mapper::heroes_for_mode(&count, mode)?;
        self.ensure_generation(id, g)?;
        Ok(heroes)
    }
    pub async fn load_match_detail(
        &self,
        id: &str,
        kind: MatchKind,
        match_id: &str,
    ) -> QueryResult<MatchDetailView> {
        if match_id.trim().is_empty() {
            return Err(QueryError::InvalidInput("缺少对局 ID".into()));
        }
        let (_, token, g, u) = self.sessions.snapshot(id)?;
        let v = u
            .match_detail(&token, match_id, kind == MatchKind::Arena)
            .await
            .map_err(|e| self.error(id, e))?;
        self.ensure_generation(id, g)?;
        mapper::detail(v, kind)
    }
    pub fn release_more_data(&self, id: &str) -> QueryResult<()> {
        self.sessions.clear_more(id)
    }
    pub fn end_query(&self, id: &str) {
        self.sessions.remove(id)
    }
    fn ensure_generation(&self, id: &str, g: u64) -> QueryResult<()> {
        if self.sessions.snapshot(id)?.2 == g {
            Ok(())
        } else {
            Err(QueryError::SessionInvalid("查询目标已变更".into()))
        }
    }
    fn error(&self, id: &str, e: anyhow::Error) -> QueryError {
        let q = classify(e);
        if matches!(q, QueryError::SessionInvalid(_)) {
            self.sessions.remove(id)
        }
        q
    }
}
fn normalize_battle_tag(value: &str) -> QueryResult<String> {
    let value = value.replace('＃', "#").trim().to_string();
    let Some((name, suffix)) = value.rsplit_once('#') else {
        return Err(QueryError::InvalidInput(
            "BattleTag 格式应为 名称#数字".into(),
        ));
    };
    if name.trim().is_empty() || suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return Err(QueryError::InvalidInput(
            "BattleTag 格式应为 名称#数字".into(),
        ));
    }
    Ok(format!("{}#{}", name.trim(), suffix))
}
fn classify(error: anyhow::Error) -> QueryError {
    let safe = error.to_string().to_ascii_lowercase();
    if safe.contains("401")
        || safe.contains("403")
        || safe.contains("unauthorized")
        || safe.contains("forbidden")
    {
        QueryError::SessionInvalid("登录凭证已失效，请在指挥台添加账号以更新登录".into())
    } else {
        QueryError::Upstream("查询暂时失败，请重试".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use upstream::UpstreamFuture;

    #[derive(Default)]
    struct CountingUpstream {
        lists: AtomicUsize,
        details: AtomicUsize,
        counts: AtomicUsize,
        images: AtomicUsize,
        active_images: AtomicUsize,
        max_active_images: AtomicUsize,
        failed_count_mode: Option<&'static str>,
    }

    impl PlayerQueryUpstream for CountingUpstream {
        fn search<'a>(&'a self, battle_tag: &'a str) -> UpstreamFuture<'a> {
            Box::pin(async move {
                Ok(
                    json!({"code": 0, "data": {"battleTag": battle_tag, "accountKey": "memory-only"}}),
                )
            })
        }
        fn card<'a>(&'a self, _: &'a str) -> UpstreamFuture<'a> {
            Box::pin(async {
                Ok(
                    json!({"code": 0, "data": {"level": 123, "gameTime": 90, "title": "title", "icon": "https://images/icon.png", "titleIcon": "https://images/title.png"}}),
                )
            })
        }
        fn count_info<'a>(
            &'a self,
            _: &'a str,
            mode: &'a str,
            _: Option<i64>,
        ) -> UpstreamFuture<'a> {
            self.counts.fetch_add(1, Ordering::SeqCst);
            let should_fail = self.failed_count_mode == Some(mode);
            Box::pin(async move {
                if should_fail {
                    anyhow::bail!("unsupported count mode")
                }
                Ok(
                    json!({"code": 0, "data": {"guideCountData": [{"roleType":"tank","matchSum":1},{"roleType":"open","matchSum":1}], "heroList": [{"heroGuid": "hero", "heroName": "英雄", "gameTimeSec": 60}]}}),
                )
            })
        }
        fn fight_count<'a>(&'a self, _: &'a str, _: Option<i64>) -> UpstreamFuture<'a> {
            self.counts.fetch_add(1, Ordering::SeqCst);
            let should_fail = self.failed_count_mode == Some("fight");
            Box::pin(async move {
                if should_fail {
                    anyhow::bail!("unsupported fight count")
                }
                Ok(
                    json!({"code":0,"data":{"roleTypeCountData":[{"roleType":"tank","matchSum":1,"winRate":"100","kda":"2.0","lastRankInfo":{"rankName":"Gold","rankSubTier":2}}],"heroUseSummaryList":[]}}),
                )
            })
        }
        fn leisure_fight_card<'a>(&'a self, _: &'a str, _: Option<i64>) -> UpstreamFuture<'a> {
            self.counts.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Ok(
                    json!({"code":0,"data":{"roleDataList":[],"heroDataList":[{"heroGuid":"hero","gameTime":60}]}}),
                )
            })
        }
        fn match_list<'a>(
            &'a self,
            _: &'a str,
            _: &'a str,
            page: u32,
            _: Option<i64>,
            _: bool,
        ) -> UpstreamFuture<'a> {
            self.lists.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(
                    json!({"code": 0, "data": {"matchList": [{"matchId": format!("match-{page}")}]}}),
                )
            })
        }
        fn match_detail<'a>(&'a self, _: &'a str, _: &'a str, _: bool) -> UpstreamFuture<'a> {
            self.details.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(Value::Null) })
        }
        fn image<'a>(&'a self, url: &'a str, _: usize) -> UpstreamFuture<'a> {
            Box::pin(async move {
                self.images.fetch_add(1, Ordering::SeqCst);
                let active = self.active_images.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_active_images.fetch_max(active, Ordering::SeqCst);
                tokio::task::yield_now().await;
                self.active_images.fetch_sub(1, Ordering::SeqCst);
                if url.contains("title") {
                    anyhow::bail!("one image failed")
                }
                Ok(json!({"mimeType":"image/png","bytes":[1,2,3]}))
            })
        }
    }

    async fn session(upstream: Arc<CountingUpstream>) -> (PlayerQueryService, String) {
        let service = PlayerQueryService::new(crate::sync_coordinator::SyncCoordinator::default());
        let started = service.start_query("玩家#1234", upstream).await.unwrap();
        (service, started.session_id)
    }

    #[tokio::test]
    async fn mode_list_and_heroes_have_independent_request_boundaries() -> Result<()> {
        let upstream = Arc::new(CountingUpstream::default());
        let (service, id) = session(Arc::clone(&upstream)).await;

        service
            .load_mode_page(&id, Season::S23, QueryMode::LeisurePreset, 1)
            .await
            .unwrap();
        assert_eq!(upstream.lists.load(Ordering::SeqCst), 1);
        assert_eq!(upstream.counts.load(Ordering::SeqCst), 0);

        service
            .load_mode_heroes(&id, Season::S23, QueryMode::LeisurePreset)
            .await
            .unwrap();
        assert_eq!(upstream.lists.load(Ordering::SeqCst), 1);
        assert_eq!(upstream.counts.load(Ordering::SeqCst), 1);

        service
            .load_mode_page(&id, Season::S23, QueryMode::LeisurePreset, 2)
            .await
            .unwrap();
        assert_eq!(upstream.lists.load(Ordering::SeqCst), 2);
        assert_eq!(upstream.counts.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[tokio::test]
    async fn arena_list_page_does_not_fetch_match_details() -> Result<()> {
        let upstream = Arc::new(CountingUpstream::default());
        let (service, id) = session(Arc::clone(&upstream)).await;

        service
            .load_mode_page(&id, Season::S23, QueryMode::SportFight, 1)
            .await
            .unwrap();

        assert_eq!(upstream.lists.load(Ordering::SeqCst), 1);
        assert_eq!(upstream.details.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[tokio::test]
    async fn card_text_never_fetches_images_and_card_images_fetches_only_the_avatar() {
        let upstream = Arc::new(CountingUpstream::default());
        let (service, id) = session(Arc::clone(&upstream)).await;

        let card = service.load_card(&id).await.unwrap();
        assert_eq!(card.level, Some(123));
        assert_eq!(upstream.images.load(Ordering::SeqCst), 0);

        let images = service.load_card_images(&id).await.unwrap();
        assert_eq!(upstream.images.load(Ordering::SeqCst), 1);
        assert_eq!(upstream.max_active_images.load(Ordering::SeqCst), 1);
        assert!(images.icon.is_some());
    }

    #[tokio::test]
    async fn season_rank_groups_fail_independently() {
        for failed in ["sport", "fight"] {
            let upstream = Arc::new(CountingUpstream {
                failed_count_mode: Some(failed),
                ..Default::default()
            });
            let (service, id) = session(upstream).await;
            let overview = service.load_season(&id, Season::S23).await.unwrap();
            assert_eq!(overview.competitive.is_empty(), failed == "sport");
            assert_eq!(overview.open_competitive.is_empty(), failed == "sport");
            assert_eq!(overview.stadium.is_empty(), failed == "fight");
        }
    }
}
