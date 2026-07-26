use super::{models::*, upstream::PlayerQueryUpstream};
use crate::sync_coordinator::QuerySessionPermit;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
};

pub(crate) struct Session {
    pub battle_tag: String,
    pub customer_token: String,
    pub generation: u64,
    pub upstream: Arc<dyn PlayerQueryUpstream>,
    pub _permit: QuerySessionPermit,
}
pub(crate) struct Sessions {
    inner: Mutex<HashMap<String, Session>>,
    next: AtomicU64,
}
impl Default for Sessions {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
        }
    }
}
impl Sessions {
    pub fn generation(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
    pub fn insert(&self, id: String, s: Session) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, s);
    }
    pub fn snapshot(
        &self,
        id: &str,
    ) -> QueryResult<(String, String, u64, Arc<dyn PlayerQueryUpstream>)> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|s| {
                (
                    s.battle_tag.clone(),
                    s.customer_token.clone(),
                    s.generation,
                    Arc::clone(&s.upstream),
                )
            })
            .ok_or_else(|| QueryError::SessionInvalid("查询会话已结束".into()))
    }
    pub fn remove(&self, id: &str) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }
    pub fn clear_more(&self, id: &str) -> QueryResult<()> {
        self.snapshot(id).map(|_| ())
    }
}
