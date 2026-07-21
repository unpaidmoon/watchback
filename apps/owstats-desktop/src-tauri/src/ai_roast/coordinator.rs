use std::collections::HashSet;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::arena::{assert_arena_material_privacy, build_arena_roast_material, ArenaRoastMaterial};
use super::client::{
    DeepSeekRoastClient, DeepSeekRoastError, RoastPromptMaterial, MODEL, PROMPT_VERSION,
};
use super::material::{assert_material_privacy, build_roast_material, RoastMaterial};
use super::store::{roast_for_match, save_roast_once, AiRoastRecord};
use super::validator::RoastValidationVocabulary;
use crate::deepseek_key::DeepSeekKeyManager;
use crate::importer::is_valid_match_detail_for;
use crate::{get_match_detail, AiMatchRoastView, MatchDetailView, OwDatabase};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use thiserror::Error;

const INVALID_MATCH_MESSAGE: &str = "\u{5bf9}\u{5c40}\u{6807}\u{8bc6}\u{65e0}\u{6548}\u{3002}";
const MISSING_KEY_MESSAGE: &str = "\u{8bf7}\u{5148}\u{914d}\u{7f6e} DeepSeek API Key\u{3002}";
const INCOMPLETE_DETAIL_MESSAGE: &str = "\u{5bf9}\u{5c40}\u{8be6}\u{60c5}\u{5c1a}\u{672a}\u{5b8c}\u{6574}\u{540c}\u{6b65}\u{ff0c}\u{6682}\u{65f6}\u{65e0}\u{6cd5}\u{751f}\u{6210} AI \u{9510}\u{8bc4}\u{3002}";
const MATERIAL_MESSAGE: &str = "\u{8fd9}\u{573a}\u{5bf9}\u{5c40}\u{6682}\u{65f6}\u{6ca1}\u{6709}\u{8db3}\u{591f}\u{7684}\u{5b89}\u{5168}\u{7d20}\u{6750}\u{751f}\u{6210} AI \u{9510}\u{8bc4}\u{3002}";
const STORAGE_MESSAGE: &str = "AI \u{9510}\u{8bc4}\u{4fdd}\u{5b58}\u{5931}\u{8d25}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}\u{3002}";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiRoastGenerationResult {
    pub ai_roast: AiMatchRoastView,
}

#[derive(Clone)]
pub struct CapturedRoastRequest {
    database: OwDatabase,
    match_id: String,
}

impl CapturedRoastRequest {
    pub fn new(database: OwDatabase, match_id: String) -> Result<Self, String> {
        validate_match_id(&match_id)?;
        Ok(Self { database, match_id })
    }
}

enum PreparedMaterial {
    Match(RoastMaterial),
    Arena(ArenaRoastMaterial),
}

impl PreparedMaterial {
    fn prompt(&self) -> RoastPromptMaterial<'_> {
        match self {
            Self::Match(material) => RoastPromptMaterial::Match(material),
            Self::Arena(material) => RoastPromptMaterial::Arena(material),
        }
    }

    fn material_version(&self) -> i64 {
        match self {
            Self::Match(material) => material.material_version,
            Self::Arena(_) => super::material::MATERIAL_VERSION,
        }
    }

    fn allowed_terms(&self) -> HashSet<String> {
        let mut terms = HashSet::new();
        match self {
            Self::Match(material) => {
                terms.insert(material.evaluation.primary_tag.clone());
                terms.extend(material.evaluation.evidence_tags.iter().cloned());
                add_fact_terms(&mut terms, &material.selected_facts);
                if let Some(relationship) = &material.relationship {
                    add_fact_terms(&mut terms, &relationship.facts);
                    add_fact_terms(&mut terms, &relationship.counter_facts);
                }
            }
            Self::Arena(material) => add_fact_terms(&mut terms, &material.candidate_facts),
        }
        terms
    }
}

fn add_fact_terms(terms: &mut HashSet<String>, facts: &[super::material::RoastFact]) {
    terms.extend(facts.iter().map(|fact| fact.text.clone()));
}

#[derive(Clone, Default)]
pub struct AiRoastCoordinator {
    active: Arc<AtomicBool>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AiRoastCoordinatorError {
    #[error("\u{5df2}\u{6709}\u{4e00}\u{573a} AI \u{9510}\u{8bc4}\u{6b63}\u{5728}\u{751f}\u{6210}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}")]
    Busy,
}

struct ActiveRoastGuard {
    active: Arc<AtomicBool>,
}

impl Drop for ActiveRoastGuard {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

impl AiRoastCoordinator {
    #[cfg(test)]
    fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub async fn run<T, F, Fut>(&self, operation: F) -> Result<T, AiRoastCoordinatorError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| AiRoastCoordinatorError::Busy)?;
        let _guard = ActiveRoastGuard {
            active: Arc::clone(&self.active),
        };
        let result = operation().await;
        Ok(result)
    }

    pub async fn generate(
        &self,
        request: CapturedRoastRequest,
        keys: &DeepSeekKeyManager,
        client: &DeepSeekRoastClient,
    ) -> Result<AiRoastGenerationResult, String> {
        self.run(|| generate_captured(request, keys, client))
            .await
            .map_err(|error| error.to_string())?
    }
}

async fn generate_captured(
    request: CapturedRoastRequest,
    keys: &DeepSeekKeyManager,
    client: &DeepSeekRoastClient,
) -> Result<AiRoastGenerationResult, String> {
    request.database.initialize().map_err(|_| STORAGE_MESSAGE)?;
    let conn = request.database.connect().map_err(|_| STORAGE_MESSAGE)?;
    if let Some(ai_roast) =
        roast_for_match(&conn, &request.match_id).map_err(|_| STORAGE_MESSAGE)?
    {
        return Ok(AiRoastGenerationResult { ai_roast });
    }

    let api_key = keys.backend_api_key().map_err(|_| MISSING_KEY_MESSAGE)?;
    let raw_detail = conn
        .query_row(
            "SELECT raw_detail_json FROM match_details WHERE match_id=?1",
            params![request.match_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| INCOMPLETE_DETAIL_MESSAGE)?
        .ok_or(INCOMPLETE_DETAIL_MESSAGE)?;
    let raw_detail = serde_json::from_str(&raw_detail).map_err(|_| INCOMPLETE_DETAIL_MESSAGE)?;
    if !is_valid_match_detail_for(&raw_detail, &request.match_id) {
        return Err(INCOMPLETE_DETAIL_MESSAGE.to_string());
    }
    let detail = get_match_detail(&request.database, &request.match_id)
        .map_err(|_| INCOMPLETE_DETAIL_MESSAGE)?;
    let evaluation_rule_version = evaluation_rule_version(&conn, &request.match_id)?;
    let material = prepare_material(&conn, &request.match_id, &detail)?;
    let vocabulary = RoastValidationVocabulary::new(known_terms(&detail), material.allowed_terms());
    let material_version = material.material_version();
    drop(conn);

    let output = client
        .generate(&api_key, material.prompt(), &vocabulary)
        .await
        .map_err(map_client_error)?;
    let conn = request.database.connect().map_err(|_| STORAGE_MESSAGE)?;
    let ai_roast = save_roast_once(
        &conn,
        &AiRoastRecord {
            match_id: request.match_id,
            award: output.award,
            commentary: output.commentary,
            used_fact_ids: output.used_fact_ids,
            model: MODEL.to_string(),
            evaluation_rule_version,
            material_version,
            prompt_version: PROMPT_VERSION,
            created_at: Utc::now().to_rfc3339(),
        },
    )
    .map_err(|_| STORAGE_MESSAGE)?;
    Ok(AiRoastGenerationResult { ai_roast })
}

fn prepare_material(
    conn: &rusqlite::Connection,
    match_id: &str,
    detail: &MatchDetailView,
) -> Result<PreparedMaterial, String> {
    if let Some(arena_detail) = &detail.arena_detail {
        let current_bnet_id = conn
            .query_row(
                "SELECT bnet_id FROM match_players WHERE match_id=?1 AND is_current_player=1 LIMIT 1",
                params![match_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| INCOMPLETE_DETAIL_MESSAGE)?
            .filter(|value| !value.trim().is_empty())
            .ok_or(INCOMPLETE_DETAIL_MESSAGE)?;
        let material = build_arena_roast_material(arena_detail, &current_bnet_id);
        if material.candidate_facts.len() < 2 {
            return Err(INCOMPLETE_DETAIL_MESSAGE.to_string());
        }
        assert_arena_material_privacy(&material).map_err(|_| MATERIAL_MESSAGE)?;
        Ok(PreparedMaterial::Arena(material))
    } else {
        let material = build_roast_material(conn, match_id).map_err(|_| MATERIAL_MESSAGE)?;
        assert_material_privacy(&material).map_err(|_| MATERIAL_MESSAGE)?;
        Ok(PreparedMaterial::Match(material))
    }
}

fn evaluation_rule_version(conn: &rusqlite::Connection, match_id: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT rule_version FROM match_evaluations WHERE match_id=?1",
        params![match_id],
        |row| row.get(0),
    )
    .optional()
    .map(|value| value.unwrap_or(0))
    .map_err(|_| MATERIAL_MESSAGE.to_string())
}

fn known_terms(detail: &MatchDetailView) -> HashSet<String> {
    let mut terms = HashSet::new();
    for value in [
        detail.map_guid.as_ref(),
        detail.map_name.as_ref(),
        detail.hero_guid.as_ref(),
        detail.hero_name.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if !value.trim().is_empty() {
            terms.insert(value.clone());
        }
    }
    for player in detail.teammates.iter().chain(&detail.enemies) {
        for value in [
            &player.name,
            &player.bnet_id,
            &player.hero_guid,
            &player.hero_name,
        ] {
            if !value.trim().is_empty() {
                terms.insert(value.clone());
            }
        }
    }
    terms
}

fn validate_match_id(match_id: &str) -> Result<(), String> {
    let valid = !match_id.is_empty()
        && match_id.len() <= 256
        && match_id.trim() == match_id
        && !match_id
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | '?' | '#'));
    valid
        .then_some(())
        .ok_or_else(|| INVALID_MATCH_MESSAGE.to_string())
}

fn map_client_error(error: DeepSeekRoastError) -> String {
    match error {
        DeepSeekRoastError::InvalidKey => "DeepSeek API Key \u{65e0}\u{6548}\u{3002}",
        DeepSeekRoastError::InsufficientBalance => "DeepSeek \u{4f59}\u{989d}\u{4e0d}\u{8db3}\u{3002}",
        DeepSeekRoastError::TooFrequent => "\u{8bf7}\u{6c42}\u{8fc7}\u{4e8e}\u{9891}\u{7e41}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}\u{3002}",
        DeepSeekRoastError::Busy => "DeepSeek \u{670d}\u{52a1}\u{6682}\u{65f6}\u{7e41}\u{5fd9}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}\u{3002}",
        DeepSeekRoastError::Timeout => "\u{751f}\u{6210}\u{8d85}\u{65f6}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}\u{3002}",
        DeepSeekRoastError::Transport => "\u{65e0}\u{6cd5}\u{8fde}\u{63a5} DeepSeek \u{670d}\u{52a1}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}\u{3002}",
        DeepSeekRoastError::Length
        | DeepSeekRoastError::ContentFilter
        | DeepSeekRoastError::InvalidOutput => MATERIAL_MESSAGE,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;
    use std::panic::AssertUnwindSafe;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn second_concurrent_request_returns_safe_busy_error() {
        let coordinator = AiRoastCoordinator::default();
        let first = coordinator.clone();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            first
                .run(|| async move {
                    started_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    1
                })
                .await
        });
        started_rx.await.unwrap();

        let second = coordinator.run(|| async { 2 }).await;
        assert_eq!(second, Err(AiRoastCoordinatorError::Busy));
        release_tx.send(()).unwrap();
        assert_eq!(task.await.unwrap().unwrap(), 1);
    }

    #[tokio::test]
    async fn guard_releases_after_error_result() {
        let coordinator = AiRoastCoordinator::default();
        let result = coordinator
            .run(|| async { Err::<(), _>("synthetic") })
            .await;
        assert_eq!(result.unwrap().unwrap_err(), "synthetic");
        assert!(!coordinator.is_active());
        assert!(coordinator.run(|| async { 3 }).await.is_ok());
    }

    #[tokio::test]
    async fn guard_releases_after_panic_unwind() {
        let coordinator = AiRoastCoordinator::default();
        let task = tokio::spawn({
            let coordinator = coordinator.clone();
            async move {
                AssertUnwindSafe(coordinator.run(|| async { panic!("synthetic") }))
                    .catch_unwind()
                    .await
                    .is_err()
            }
        });
        assert!(task.await.unwrap());
        assert!(!coordinator.is_active());
        assert!(coordinator.run(|| async { 4 }).await.is_ok());
    }
}
