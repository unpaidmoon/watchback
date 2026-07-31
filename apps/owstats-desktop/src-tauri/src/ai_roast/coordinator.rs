use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::client::{
    AiRoastStreamEvent, DeepSeekRoastClient, DeepSeekRoastError, MODEL, PROMPT_VERSION,
};
use super::knowledge::{embedded_knowledge, knowledge_for_match};
use super::snapshot::{build_roast_snapshot, INPUT_SCHEMA_VERSION};
use super::store::{replace_roast, roast_for_match, save_roast_once, AiRoastRecord};
use super::task::task_knowledge_for_players;
use crate::deepseek_key::DeepSeekKeyManager;
use crate::importer::is_valid_match_detail_for;
use crate::{get_match_detail, AiMatchRoastView, OwDatabase};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use thiserror::Error;

const INVALID_MATCH_MESSAGE: &str = "\u{5bf9}\u{5c40}\u{6807}\u{8bc6}\u{65e0}\u{6548}\u{3002}";
const MISSING_KEY_MESSAGE: &str = "\u{8bf7}\u{5148}\u{914d}\u{7f6e} DeepSeek API Key\u{3002}";
const INCOMPLETE_DETAIL_MESSAGE: &str = "\u{5bf9}\u{5c40}\u{8be6}\u{60c5}\u{5c1a}\u{672a}\u{5b8c}\u{6574}\u{540c}\u{6b65}\u{ff0c}\u{6682}\u{65f6}\u{65e0}\u{6cd5}\u{751f}\u{6210} AI \u{9510}\u{8bc4}\u{3002}";
const MATERIAL_MESSAGE: &str = "\u{8fd9}\u{573a}\u{5bf9}\u{5c40}\u{6682}\u{65f6}\u{6ca1}\u{6709}\u{8db3}\u{591f}\u{7684}\u{53ef}\u{6838}\u{9a8c}\u{6570}\u{636e}\u{751f}\u{6210} AI \u{9510}\u{8bc4}\u{3002}";
const STORAGE_MESSAGE: &str = "AI \u{9510}\u{8bc4}\u{4fdd}\u{5b58}\u{5931}\u{8d25}\u{ff0c}\u{8bf7}\u{7a0d}\u{540e}\u{518d}\u{8bd5}\u{3002}";
const LENGTH_MESSAGE: &str = "\u{9510}\u{8bc4}\u{8f93}\u{51fa}\u{8fbe}\u{5230}\u{957f}\u{5ea6}\u{4e0a}\u{9650}\u{ff0c}\u{672a}\u{80fd}\u{5b8c}\u{6574}\u{751f}\u{6210}\u{3002}";
const CONTENT_FILTER_MESSAGE: &str = "\u{9510}\u{8bc4}\u{5185}\u{5bb9}\u{88ab} DeepSeek \u{5b89}\u{5168}\u{89c4}\u{5219}\u{62e6}\u{622a}\u{3002}";
const INVALID_OUTPUT_MESSAGE: &str =
    "DeepSeek \u{8fd4}\u{56de}\u{7684}\u{9510}\u{8bc4}\u{683c}\u{5f0f}\u{65e0}\u{6548}\u{3002}";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiRoastGenerationResult {
    pub ai_roast: AiMatchRoastView,
}

#[derive(Clone)]
pub struct CapturedRoastRequest {
    database: OwDatabase,
    match_id: String,
    regenerate: bool,
}

impl CapturedRoastRequest {
    pub fn new(database: OwDatabase, match_id: String) -> Result<Self, String> {
        validate_match_id(&match_id)?;
        Ok(Self {
            database,
            match_id,
            regenerate: false,
        })
    }

    pub fn regenerate(database: OwDatabase, match_id: String) -> Result<Self, String> {
        validate_match_id(&match_id)?;
        Ok(Self {
            database,
            match_id,
            regenerate: true,
        })
    }
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
        self.generate_streaming(request, keys, client, |_| {}).await
    }

    pub async fn generate_streaming<F>(
        &self,
        request: CapturedRoastRequest,
        keys: &DeepSeekKeyManager,
        client: &DeepSeekRoastClient,
        on_event: F,
    ) -> Result<AiRoastGenerationResult, String>
    where
        F: FnMut(AiRoastStreamEvent),
    {
        self.run(|| generate_captured(request, keys, client, on_event))
            .await
            .map_err(|error| error.to_string())?
    }
}

async fn generate_captured<F>(
    request: CapturedRoastRequest,
    keys: &DeepSeekKeyManager,
    client: &DeepSeekRoastClient,
    on_event: F,
) -> Result<AiRoastGenerationResult, String>
where
    F: FnMut(AiRoastStreamEvent),
{
    request.database.initialize().map_err(|_| STORAGE_MESSAGE)?;
    let conn = request.database.connect().map_err(|_| STORAGE_MESSAGE)?;
    if !request.regenerate {
        if let Some(ai_roast) =
            roast_for_match(&conn, &request.match_id).map_err(|_| STORAGE_MESSAGE)?
        {
            return Ok(AiRoastGenerationResult { ai_roast });
        }
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
    let knowledge = embedded_knowledge().map_err(|_| MATERIAL_MESSAGE)?;
    let knowledge_version = knowledge.version;
    let context = knowledge_for_match(
        &knowledge,
        detail
            .teammates
            .iter()
            .chain(&detail.enemies)
            .map(|row| row.hero_name.clone()),
        detail.map_name.as_deref(),
    );
    let mut snapshot = build_roast_snapshot(&conn, &request.match_id, &detail, context)
        .map_err(|_| MATERIAL_MESSAGE)?;
    snapshot.task_knowledge = task_knowledge_for_players(
        &knowledge,
        &snapshot.scoreboard.team,
        &snapshot.scoreboard.opponent,
    );
    drop(conn);

    let output = client
        .generate_streaming(&api_key, &snapshot, on_event)
        .await
        .map_err(map_client_error)?;
    let conn = request.database.connect().map_err(|_| STORAGE_MESSAGE)?;
    let record = AiRoastRecord {
        match_id: request.match_id,
        award: output.title.clone(),
        commentary: output.commentary.clone(),
        report: Some(super::report::StoredRoastPayload::Narrative(output)),
        used_fact_ids: Vec::new(),
        model: MODEL.to_string(),
        evaluation_rule_version: 0,
        material_version: INPUT_SCHEMA_VERSION,
        prompt_version: PROMPT_VERSION,
        report_schema_version: Some(super::report::REPORT_SCHEMA_VERSION),
        input_schema_version: Some(INPUT_SCHEMA_VERSION),
        knowledge_version: Some(knowledge_version),
        created_at: Utc::now().to_rfc3339(),
    };
    let ai_roast = if request.regenerate {
        replace_roast(&conn, &record)
    } else {
        save_roast_once(&conn, &record)
    }
    .map_err(|_| STORAGE_MESSAGE)?;
    Ok(AiRoastGenerationResult { ai_roast })
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
        DeepSeekRoastError::Length => LENGTH_MESSAGE,
        DeepSeekRoastError::ContentFilter => CONTENT_FILTER_MESSAGE,
        DeepSeekRoastError::InvalidOutput => INVALID_OUTPUT_MESSAGE,
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

    #[test]
    fn client_failures_keep_distinct_safe_messages() {
        assert_eq!(
            map_client_error(DeepSeekRoastError::InsufficientBalance),
            "DeepSeek \u{4f59}\u{989d}\u{4e0d}\u{8db3}\u{3002}"
        );
        assert_eq!(
            map_client_error(DeepSeekRoastError::Length),
            "\u{9510}\u{8bc4}\u{8f93}\u{51fa}\u{8fbe}\u{5230}\u{957f}\u{5ea6}\u{4e0a}\u{9650}\u{ff0c}\u{672a}\u{80fd}\u{5b8c}\u{6574}\u{751f}\u{6210}\u{3002}"
        );
        assert_eq!(
            map_client_error(DeepSeekRoastError::ContentFilter),
            "\u{9510}\u{8bc4}\u{5185}\u{5bb9}\u{88ab} DeepSeek \u{5b89}\u{5168}\u{89c4}\u{5219}\u{62e6}\u{622a}\u{3002}"
        );
        assert_eq!(
            map_client_error(DeepSeekRoastError::InvalidOutput),
            "DeepSeek \u{8fd4}\u{56de}\u{7684}\u{9510}\u{8bc4}\u{683c}\u{5f0f}\u{65e0}\u{6548}\u{3002}"
        );
    }
}
